//! Pure job planning for one analysed artifact.

use std::collections::BTreeSet;

use malbox_database::repositories::machinery::{MachineArch, MachinePlatform};
use malbox_plugin_manifest::Stage;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::params::{DetonateMode, SubmissionParams};
use crate::recursion::DYNAMIC;
use crate::routing::{DetonationProfile, RoutingTable};

pub use malbox_config::orchestration::{SUBMISSION_PROFILE, UNMATCHED_PROFILE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKindPlan {
    Host,
    Detonation,
}

impl JobKindPlan {
    /// The `job_kind` label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Detonation => "detonation",
        }
    }

    pub fn from_db(kind: &str) -> Self {
        if kind == "detonation" {
            Self::Detonation
        } else {
            Self::Host
        }
    }
}

/// A resolved detonation, stored in `jobs.detonation`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetonationRequest {
    pub profile: String,
    pub platform: Option<MachinePlatform>,
    pub arch: Option<MachineArch>,
    pub machine_id: Option<i32>,
    pub snapshot_id: Option<Uuid>,
    pub snapshot_name: Option<String>,
    pub tags: Vec<String>,
    pub guest_plugins: Vec<String>,
    pub timeout_secs: u64,
    pub package: Option<String>,
    pub dll_function: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TaskSelection {
    pub selected: Vec<String>,
    pub excluded: Vec<String>,
}

pub struct PlanInput<'a> {
    pub artifact_type: &'a str,
    pub is_root: bool,
    pub task: &'a TaskSelection,
    pub params: &'a SubmissionParams,
    /// Categories this artifact may not run (`"dynamic"` = no detonation).
    pub forbidden: &'a BTreeSet<String>,
    /// Analyzers whose output produced this artifact.
    pub source_analyzers: &'a BTreeSet<String>,
    pub routing: &'a RoutingTable,
    /// Explicit root detonation from the submission.
    pub explicit: Option<&'a DetonationRequest>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedJob {
    pub kind: JobKindPlan,
    pub analyzer: String,
    pub stage: Stage,
    pub detonation: Option<DetonationRequest>,
}

/// Guest plugins named in the task selection.
pub fn selected_guest_plugins(selection: &TaskSelection, routing: &RoutingTable) -> Vec<String> {
    selection
        .selected
        .iter()
        .filter(|s| routing.is_guest_plugin(s))
        .cloned()
        .collect()
}

/// A detonation request from a configured profile.
pub fn request_from_profile(
    profile: &DetonationProfile,
    selection: &TaskSelection,
    routing: &RoutingTable,
    params: &SubmissionParams,
    is_root: bool,
) -> DetonationRequest {
    let selected = selected_guest_plugins(selection, routing);
    let guest_plugins = if selected.is_empty() {
        profile.guest_plugins.clone()
    } else if profile.guest_plugins.is_empty() {
        selected
    } else {
        let both: Vec<String> = profile
            .guest_plugins
            .iter()
            .filter(|p| selected.contains(p))
            .cloned()
            .collect();
        if both.is_empty() { selected } else { both }
    };
    let (package, dll_function) = if is_root {
        (
            params.detonation.package.clone(),
            params.detonation.dll_function.clone(),
        )
    } else {
        (None, None)
    };
    DetonationRequest {
        profile: profile.name.clone(),
        platform: Some(profile.platform.clone()),
        arch: profile.arch.clone(),
        machine_id: None,
        snapshot_id: None,
        snapshot_name: profile.snapshot.clone(),
        tags: profile.tags.clone(),
        guest_plugins,
        timeout_secs: profile.timeout.as_secs(),
        package,
        dll_function,
    }
}

pub fn plan_artifact(input: &PlanInput) -> Vec<PlannedJob> {
    let routing = input.routing;
    let selection = input.task;
    let selects_host = selection
        .selected
        .iter()
        .any(|s| routing.analyzer(s).is_some());

    let mut jobs: Vec<PlannedJob> = routing
        .analyzers()
        .filter(|a| !selects_host || selection.selected.contains(&a.name))
        .filter(|a| !selection.excluded.contains(&a.name))
        .filter(|a| !(input.params.ignore_filtering && a.stage == Stage::Filter))
        .filter(|a| !input.forbidden.contains(&a.category))
        .filter(|a| {
            !a.sources_exclude
                .iter()
                .any(|s| input.source_analyzers.contains(s))
        })
        .filter(|a| a.routes(input.artifact_type))
        .map(|a| PlannedJob {
            kind: JobKindPlan::Host,
            analyzer: a.name.clone(),
            stage: a.stage,
            detonation: None,
        })
        .collect();

    if let Some(request) = plan_detonation(input) {
        jobs.push(PlannedJob {
            kind: JobKindPlan::Detonation,
            analyzer: request.profile.clone(),
            stage: Stage::Dynamic,
            detonation: Some(request),
        });
    }
    jobs
}

fn plan_detonation(input: &PlanInput) -> Option<DetonationRequest> {
    let params = input.params;
    if params.detonate == DetonateMode::Never || input.forbidden.contains(DYNAMIC) {
        return None;
    }
    if input.is_root
        && let Some(explicit) = input.explicit
    {
        return Some(explicit.clone());
    }
    let profile = match (&params.detonation.profile, input.is_root) {
        (Some(name), true) => input.routing.profile(name),
        _ => input.routing.match_profile(input.artifact_type),
    };
    if let Some(profile) = profile {
        return Some(request_from_profile(
            profile,
            input.task,
            input.routing,
            params,
            input.is_root,
        ));
    }
    if input.is_root && params.detonate == DetonateMode::Always {
        return Some(DetonationRequest {
            profile: UNMATCHED_PROFILE.to_string(),
            platform: None,
            arch: None,
            machine_id: None,
            snapshot_id: None,
            snapshot_name: None,
            tags: Vec::new(),
            guest_plugins: selected_guest_plugins(input.task, input.routing),
            timeout_secs: 0,
            package: None,
            dll_function: None,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::{AnalyzerSpec, anchored};
    use std::collections::BTreeMap;
    use std::time::Duration;

    fn analyzer(name: &str, stage: Stage, accepts: &str) -> AnalyzerSpec {
        AnalyzerSpec {
            name: name.into(),
            version: "1.0.0".into(),
            stage,
            category: stage.as_str().into(),
            accepts: anchored(accepts).unwrap(),
            rejects: None,
            sources_exclude: vec![],
            timeout: Duration::from_secs(60),
            cacheable: true,
            recursion_prevention: vec![],
            uses_indicators: false,
            monitored_keys: vec![],
            parameters: BTreeMap::new(),
        }
    }

    fn profile(name: &str, accepts: &str) -> DetonationProfile {
        DetonationProfile {
            name: name.into(),
            accepts: anchored(accepts).unwrap(),
            platform: MachinePlatform::Windows,
            arch: None,
            snapshot: Some("snap".into()),
            tags: vec![],
            guest_plugins: vec!["mon".into()],
            timeout: Duration::from_secs(120),
        }
    }

    fn table(analyzers: Vec<AnalyzerSpec>, profiles: Vec<DetonationProfile>) -> RoutingTable {
        RoutingTable::new(analyzers, ["mon".to_string()].into(), profiles)
    }

    struct Fixture {
        routing: RoutingTable,
        selection: TaskSelection,
        params: SubmissionParams,
        forbidden: BTreeSet<String>,
        sources: BTreeSet<String>,
    }

    impl Fixture {
        fn new(routing: RoutingTable) -> Self {
            Self {
                routing,
                selection: TaskSelection::default(),
                params: SubmissionParams::default(),
                forbidden: BTreeSet::new(),
                sources: BTreeSet::new(),
            }
        }

        fn plan(
            &self,
            t: &str,
            is_root: bool,
            explicit: Option<&DetonationRequest>,
        ) -> Vec<PlannedJob> {
            plan_artifact(&PlanInput {
                artifact_type: t,
                is_root,
                task: &self.selection,
                params: &self.params,
                forbidden: &self.forbidden,
                source_analyzers: &self.sources,
                routing: &self.routing,
                explicit,
            })
        }

        fn names(&self, t: &str, is_root: bool) -> Vec<String> {
            self.plan(t, is_root, None)
                .into_iter()
                .map(|j| j.analyzer)
                .collect()
        }
    }

    fn base() -> Fixture {
        Fixture::new(table(
            vec![
                analyzer("filter", Stage::Filter, ".*"),
                analyzer("pe", Stage::Static, "executable/.*"),
                analyzer("zip", Stage::Extract, "archive/zip"),
            ],
            vec![],
        ))
    }

    #[test]
    fn routes_by_regex() {
        let f = base();
        assert_eq!(
            f.names("executable/windows/pe32", false),
            vec!["filter", "pe"]
        );
        assert_eq!(f.names("archive/zip", false), vec!["filter", "zip"]);
        // Anchored: a prefix match is not enough.
        assert_eq!(f.names("xexecutable/windows/pe32", false), vec!["filter"]);
    }

    #[test]
    fn rejects_excluded() {
        let mut f = base();
        f.routing = table(
            vec![AnalyzerSpec {
                rejects: Some(anchored("executable/linux/.*").unwrap()),
                ..analyzer("pe", Stage::Static, "executable/.*")
            }],
            vec![],
        );
        assert_eq!(f.names("executable/windows/pe32", false), vec!["pe"]);
        assert!(f.names("executable/linux/elf64", false).is_empty());
    }

    #[test]
    fn selection_limits_host_analyzers() {
        let mut f = base();
        f.selection.selected = vec!["pe".into(), "mon".into()];
        assert_eq!(f.names("executable/windows/pe32", false), vec!["pe"]);
        // Only guest plugins selected: every host analyzer is a candidate.
        f.selection.selected = vec!["mon".into()];
        assert_eq!(
            f.names("executable/windows/pe32", false),
            vec!["filter", "pe"]
        );
    }

    #[test]
    fn excluded_plugin_skipped() {
        let mut f = base();
        f.selection.excluded = vec!["pe".into()];
        assert_eq!(f.names("executable/windows/pe32", false), vec!["filter"]);
    }

    #[test]
    fn forbidden_category_skipped() {
        let mut f = base();
        f.forbidden.insert("static".into());
        assert_eq!(f.names("executable/windows/pe32", false), vec!["filter"]);
    }

    #[test]
    fn sources_exclude_skips() {
        let mut f = base();
        f.routing = table(
            vec![AnalyzerSpec {
                sources_exclude: vec!["zip".into()],
                ..analyzer("pe", Stage::Static, "executable/.*")
            }],
            vec![],
        );
        assert_eq!(f.names("executable/windows/pe32", false), vec!["pe"]);
        f.sources.insert("zip".into());
        assert!(f.names("executable/windows/pe32", false).is_empty());
    }

    #[test]
    fn ignore_filtering_drops_filter_stage() {
        let mut f = base();
        f.params.ignore_filtering = true;
        assert_eq!(f.names("executable/windows/pe32", false), vec!["pe"]);
    }

    #[test]
    fn detonate_never() {
        let mut f = Fixture::new(table(vec![], vec![profile("win", ".*")]));
        assert_eq!(f.names("executable/windows/pe32", true), vec!["win"]);
        f.params.detonate = DetonateMode::Never;
        assert!(f.names("executable/windows/pe32", true).is_empty());
    }

    #[test]
    fn dynamic_forbidden_skips_detonation() {
        let mut f = Fixture::new(table(vec![], vec![profile("win", ".*")]));
        f.forbidden.insert(DYNAMIC.into());
        assert!(f.names("executable/windows/pe32", false).is_empty());
    }

    #[test]
    fn explicit_root_request_wins() {
        let f = Fixture::new(table(vec![], vec![profile("win", ".*")]));
        let explicit = DetonationRequest {
            profile: SUBMISSION_PROFILE.into(),
            platform: Some(MachinePlatform::Linux),
            arch: None,
            machine_id: Some(3),
            snapshot_id: None,
            snapshot_name: None,
            tags: vec![],
            guest_plugins: vec![],
            timeout_secs: 30,
            package: None,
            dll_function: None,
        };
        let jobs = f.plan("executable/windows/pe32", true, Some(&explicit));
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].analyzer, SUBMISSION_PROFILE);
        assert_eq!(jobs[0].stage, Stage::Dynamic);
        assert_eq!(jobs[0].detonation.as_ref(), Some(&explicit));
        // Children never use the explicit request.
        let jobs = f.plan("executable/windows/pe32", false, Some(&explicit));
        assert_eq!(jobs[0].analyzer, "win");
    }

    #[test]
    fn first_profile_match() {
        let f = Fixture::new(table(
            vec![],
            vec![
                profile("docs", "document/.*"),
                profile("exe1", "executable/.*"),
                profile("exe2", "executable/.*"),
            ],
        ));
        let jobs = f.plan("executable/windows/pe32", false, None);
        assert_eq!(jobs.len(), 1);
        let req = jobs[0].detonation.as_ref().unwrap();
        assert_eq!(req.profile, "exe1");
        assert_eq!(req.platform, Some(MachinePlatform::Windows));
        assert_eq!(req.snapshot_name.as_deref(), Some("snap"));
        assert_eq!(req.guest_plugins, vec!["mon"]);
        assert_eq!(req.timeout_secs, 120);
        assert!(f.plan("text/plain", false, None).is_empty());
    }

    #[test]
    fn root_profile_override() {
        let mut f = Fixture::new(table(
            vec![],
            vec![profile("exe", "executable/.*"), profile("other", "never")],
        ));
        f.params.detonation.profile = Some("other".into());
        let jobs = f.plan("executable/windows/pe32", true, None);
        assert_eq!(jobs[0].analyzer, "other");
    }

    #[test]
    fn always_without_match_is_unmatched() {
        let mut f = Fixture::new(table(vec![], vec![profile("exe", "executable/.*")]));
        f.params.detonate = DetonateMode::Always;
        let jobs = f.plan("text/plain", true, None);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].analyzer, UNMATCHED_PROFILE);
        // Not for children.
        assert!(f.plan("text/plain", false, None).is_empty());
    }
}
