//! Orchestrator end-to-end against a real Postgres, with fake runners.
//!
//! Gated on `MALBOX_TEST_DATABASE_URL` (an admin URL); each test creates and
//! drops its own `malbox_test_{uuid}` database.

#![expect(clippy::unwrap_used, reason = "test code: failures should panic")]
#![expect(
    clippy::print_stdout,
    reason = "test harness reports skipped database tests on stdout"
)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use malbox_config::OrchestrationConfig;
use malbox_database::connect_and_migrate;
use malbox_database::repositories::artifacts::{self, TaskArtifact};
use malbox_database::repositories::jobs::{self, Job, NewJob};
use malbox_database::repositories::machinery::MachinePlatform;
use malbox_database::repositories::samples::{self, Sample};
use malbox_database::repositories::tasks::{self, Task, TaskState};
use malbox_plugin_internal::manager::handle::{OutputFormat, PluginOutput};
use malbox_plugin_manifest::Stage;
use malbox_plugin_transport::artifact::{ArtifactMeta, ArtifactRelation};
use malbox_plugin_transport::job_config;
use malbox_scheduler::routing::anchored;
use malbox_scheduler::{
    AcquireResult, AnalyzerSpec, ChildDetonation, DescendantPolicy, DetonationJobRequest,
    DetonationProfile, DetonationRunner, HostJobRequest, HostJobRunner, JobRunError, Lease,
    NewSubmission, OrchestratorCore, OrchestratorHandle, PluginOutputs, RoutingTable,
    SubmissionParams, create_task, start_with_runners,
};
use malbox_utils::identify::Identification;
use malbox_utils::{ResultStore, SampleStore, hashing};
use serde_json::json;
use sqlx::{AssertSqlSafe, Connection, PgConnection, PgPool};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

// ----- database -----------------------------------------------------------

struct TestDb {
    admin_url: String,
    name: String,
    pool: PgPool,
}

impl TestDb {
    async fn create() -> Option<Self> {
        let Ok(admin_url) = std::env::var("MALBOX_TEST_DATABASE_URL") else {
            println!("skipping: MALBOX_TEST_DATABASE_URL not set");
            return None;
        };
        let name = format!("malbox_test_{}", Uuid::new_v4().simple());
        let mut admin = PgConnection::connect(&admin_url)
            .await
            .expect("admin connect");
        sqlx::query(AssertSqlSafe(format!("create database \"{name}\"")))
            .execute(&mut admin)
            .await
            .expect("create database");
        admin.close().await.expect("close admin");
        let (base, _) = admin_url.rsplit_once('/').expect("url has a database path");
        let pool = connect_and_migrate(&format!("{base}/{name}"))
            .await
            .expect("connect and migrate");
        Some(Self {
            admin_url,
            name,
            pool,
        })
    }

    async fn drop(self) {
        self.pool.close().await;
        let mut admin = PgConnection::connect(&self.admin_url)
            .await
            .expect("admin connect");
        sqlx::query(AssertSqlSafe(format!(
            "drop database if exists \"{}\" with (force)",
            self.name
        )))
        .execute(&mut admin)
        .await
        .expect("drop database");
    }
}

// ----- fakes ----------------------------------------------------------------

fn fake_identify(_data: &[u8], name: Option<&str>) -> Identification {
    let ext = name.and_then(|n| n.rsplit_once('.')).map(|(_, e)| e);
    let (artifact_type, mime) = match ext {
        Some("zip") => ("archive/zip", "application/zip"),
        Some("exe") => ("executable/windows/pe32", "application/x-dosexec"),
        Some("sh") => ("code/shell", "text/x-shellscript"),
        _ => ("text/plain", "text/plain"),
    };
    Identification {
        artifact_type: artifact_type.into(),
        mime: mime.into(),
        description: format!("fake {artifact_type}"),
    }
}

enum Reply {
    Outputs(Vec<PluginOutput>),
    Fail(JobRunError),
    /// Run until cancelled.
    Block,
}

type HostScript = dyn Fn(&str, &str, u32) -> Reply + Send + Sync;

/// Scripted host runner: `script(analyzer, artifact name, call index)`.
struct FakeHostRunner {
    script: Box<HostScript>,
    calls: Mutex<HashMap<(String, String), u32>>,
}

impl FakeHostRunner {
    fn new(script: impl Fn(&str, &str, u32) -> Reply + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            script: Box::new(script),
            calls: Mutex::default(),
        })
    }

    fn calls(&self, analyzer: &str, name: &str) -> u32 {
        self.calls
            .lock()
            .unwrap()
            .get(&(analyzer.to_string(), name.to_string()))
            .copied()
            .unwrap_or(0)
    }
}

#[async_trait]
impl HostJobRunner for FakeHostRunner {
    async fn run(
        &self,
        req: HostJobRequest,
        cancel: CancellationToken,
    ) -> Result<Vec<PluginOutput>, JobRunError> {
        let name = req.config[job_config::ARTIFACT_NAME].clone();
        let n = {
            let mut calls = self.calls.lock().unwrap();
            let c = calls
                .entry((req.spec.name.clone(), name.clone()))
                .or_default();
            *c += 1;
            *c - 1
        };
        match (self.script)(&req.spec.name, &name, n) {
            Reply::Outputs(o) => Ok(o),
            Reply::Fail(e) => Err(e),
            Reply::Block => {
                cancel.cancelled().await;
                Err(JobRunError::Canceled)
            }
        }
    }
}

type DetonationScript = dyn Fn(&str) -> Vec<PluginOutputs> + Send + Sync;

/// In-memory machine slots with scripted per-plugin outputs:
/// `script(artifact name)`.
struct FakeDetonationRunner {
    lease: Option<Lease>,
    free: Mutex<usize>,
    unserviceable: Option<String>,
    script: Box<DetonationScript>,
    runs: Mutex<Vec<String>>,
}

impl FakeDetonationRunner {
    fn idle() -> Self {
        Self {
            lease: None,
            free: Mutex::new(0),
            unserviceable: None,
            script: Box::new(|_| Vec::new()),
            runs: Mutex::default(),
        }
    }

    /// Never called: for tests without detonation profiles.
    fn none() -> Arc<Self> {
        Arc::new(Self::idle())
    }

    fn unserviceable(message: &str) -> Arc<Self> {
        Arc::new(Self {
            unserviceable: Some(message.to_string()),
            ..Self::idle()
        })
    }

    fn slots(
        lease: Lease,
        slots: usize,
        script: impl Fn(&str) -> Vec<PluginOutputs> + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            lease: Some(lease),
            free: Mutex::new(slots),
            unserviceable: None,
            script: Box::new(script),
            runs: Mutex::default(),
        })
    }

    fn runs(&self) -> Vec<String> {
        self.runs.lock().unwrap().clone()
    }
}

#[async_trait]
impl DetonationRunner for FakeDetonationRunner {
    async fn acquire(
        &self,
        _request: &malbox_scheduler::DetonationRequest,
        _task_id: i32,
    ) -> AcquireResult {
        if let Some(m) = &self.unserviceable {
            return AcquireResult::Unserviceable(m.clone());
        }
        let mut free = self.free.lock().unwrap();
        match &self.lease {
            Some(lease) if *free > 0 => {
                *free -= 1;
                AcquireResult::Acquired(lease.clone())
            }
            _ => AcquireResult::Busy,
        }
    }

    async fn abandon(&self, _lease: Lease) {
        *self.free.lock().unwrap() += 1;
    }

    async fn run(
        &self,
        _lease: Lease,
        req: DetonationJobRequest,
        _cancel: CancellationToken,
    ) -> Result<Vec<PluginOutputs>, JobRunError> {
        let name = req.base_config[job_config::ARTIFACT_NAME].clone();
        self.runs.lock().unwrap().push(name.clone());
        let out = (self.script)(&name);
        *self.free.lock().unwrap() += 1;
        Ok(out)
    }
}

// ----- outputs and routing ----------------------------------------------------

fn artifact(name: &str, data: &[u8], relation: ArtifactRelation) -> PluginOutput {
    PluginOutput {
        result_name: name.to_string(),
        data: data.to_vec(),
        format: OutputFormat::Bytes,
        artifact: Some(ArtifactMeta {
            relation,
            analyze: relation.default_analyze(),
            allow_dynamic_recursion: false,
        }),
    }
}

fn report(plugin: &str, labels: &[&str]) -> PluginOutput {
    let body = json!({
        "schema_version": 1,
        "plugin": {"id": plugin, "version": "1.0.0"},
        "verdict": {"classification": "unknown", "labels": labels},
    });
    PluginOutput {
        result_name: "report".into(),
        data: serde_json::to_vec(&body).unwrap(),
        format: OutputFormat::Json,
        artifact: None,
    }
}

fn shared(value: &serde_json::Value) -> PluginOutput {
    PluginOutput {
        result_name: "malbox.shared".into(),
        data: serde_json::to_vec(value).unwrap(),
        format: OutputFormat::Json,
        artifact: None,
    }
}

fn spec(name: &str, stage: Stage, accepts: &str) -> AnalyzerSpec {
    AnalyzerSpec {
        name: name.into(),
        version: "1.0.0".into(),
        stage,
        category: stage.as_str().into(),
        accepts: anchored(accepts).unwrap(),
        rejects: None,
        sources_exclude: vec![],
        timeout: Duration::from_secs(30),
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
        platform: MachinePlatform::Linux,
        arch: None,
        snapshot: None,
        tags: vec![],
        guest_plugins: vec!["mon".into()],
        timeout: Duration::from_secs(10),
    }
}

fn routing(analyzers: Vec<AnalyzerSpec>, profiles: Vec<DetonationProfile>) -> RoutingTable {
    RoutingTable::new(analyzers, BTreeSet::from(["mon".to_string()]), profiles)
}

fn mon(outputs: Vec<PluginOutput>) -> Vec<PluginOutputs> {
    vec![PluginOutputs {
        plugin: "mon".into(),
        result: Ok(outputs),
    }]
}

// ----- environment --------------------------------------------------------------

struct Env {
    db: TestDb,
    _dir: tempfile::TempDir,
    core: OrchestratorCore,
}

struct Running {
    handle: OrchestratorHandle,
    join: JoinHandle<()>,
    token: CancellationToken,
}

impl Env {
    async fn new(adjust: impl FnOnce(&mut OrchestrationConfig)) -> Option<Self> {
        let db = TestDb::create().await?;
        let dir = tempfile::tempdir().expect("tempdir");
        let mut settings = OrchestrationConfig {
            retry_backoff_secs: 0,
            detonation_decision_window_secs: 1,
            ..Default::default()
        };
        adjust(&mut settings);
        let core = OrchestratorCore {
            db: db.pool.clone(),
            sample_store: Arc::new(SampleStore::new(dir.path())),
            result_store: Arc::new(ResultStore::new(dir.path())),
            settings: Arc::new(settings),
            data_dir: dir.path().to_path_buf(),
            identify: fake_identify,
            events: Arc::new(()),
        };
        Some(Self {
            db,
            _dir: dir,
            core,
        })
    }

    fn pool(&self) -> &PgPool {
        &self.db.pool
    }

    async fn start(
        &self,
        routing: RoutingTable,
        host: Arc<FakeHostRunner>,
        detonation: Arc<FakeDetonationRunner>,
    ) -> Running {
        let token = CancellationToken::new();
        let (handle, join) = start_with_runners(
            self.core.clone(),
            Arc::new(routing),
            host,
            detonation,
            token.clone(),
        )
        .await
        .expect("start orchestrator");
        Running {
            handle,
            join,
            token,
        }
    }

    /// `insert_sample` + `create_task`, without submitting.
    async fn create(
        &self,
        data: &[u8],
        name: &str,
        params: SubmissionParams,
    ) -> (Task, TaskArtifact) {
        let sha256 = hashing::get_sha256(data);
        self.core
            .sample_store
            .store(&sha256, data)
            .await
            .expect("store sample");
        let sample = samples::insert_sample(
            self.pool(),
            Sample {
                file_size: i64::try_from(data.len()).expect("test sample fits in i64"),
                file_type: "fake".into(),
                md5: hashing::get_md5(data),
                crc32: hashing::get_crc32(data),
                sha1: hashing::get_sha1(data),
                sha256,
                sha512: hashing::get_sha512(data),
                ssdeep: "not-available".into(),
            },
        )
        .await
        .expect("insert sample");
        create_task(
            &self.core,
            NewSubmission {
                sample,
                identification: fake_identify(data, Some(name)),
                target: name.into(),
                plugins: vec![],
                excluded_plugins: vec![],
                timeout: 60,
                priority: 1,
                platform: None,
                snapshot_id: None,
                machine_id: None,
                owner: None,
                tags: None,
                params,
                parent_task_id: None,
            },
        )
        .await
        .expect("create task")
    }

    async fn submit(
        &self,
        run: &Running,
        data: &[u8],
        name: &str,
        params: SubmissionParams,
    ) -> i32 {
        let (task, _) = self.create(data, name, params).await;
        run.handle.submit(task.id);
        task.id
    }

    /// Seed a ready machine with one snapshot so leases satisfy foreign keys.
    async fn seed_machine(&self) -> Lease {
        let machine_id: i32 = sqlx::query_scalar(
            "insert into machines (name, arch, platform, status) \
             values ('m1', 'x64', 'linux', 'ready') returning id",
        )
        .fetch_one(self.pool())
        .await
        .expect("insert machine");
        let snapshot_id: Uuid = sqlx::query_scalar(
            "insert into machine_snapshots (machine_id, name, provider_snapshot_id, guest_plugins, is_active) \
             values ($1, 's1', 's1', '[\"mon\"]', true) returning id",
        )
        .bind(machine_id)
        .fetch_one(self.pool())
        .await
        .expect("insert snapshot");
        Lease {
            machine_id,
            snapshot_id,
        }
    }

    async fn wait_terminal(&self, task_id: i32) -> Task {
        for _ in 0..300 {
            let task = tasks::fetch_task(self.pool(), task_id)
                .await
                .unwrap()
                .unwrap();
            if task.status.is_terminal() {
                return task;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let jobs = jobs::fetch_task_jobs(self.pool(), task_id).await.unwrap();
        let states: Vec<_> = jobs
            .iter()
            .map(|j| (j.analyzer.clone(), j.artifact_id, j.state.clone()))
            .collect();
        panic!("task {task_id} did not finish within 30s; jobs: {states:?}");
    }

    async fn finish(self, run: Running) {
        run.token.cancel();
        run.join.await.expect("orchestrator join");
        self.db.drop().await;
    }

    /// Artifacts keyed by name: earliest incoming edge name, `"root"` for the root.
    async fn artifacts(&self, task_id: i32) -> HashMap<String, TaskArtifact> {
        let all = artifacts::fetch_task_artifacts(self.pool(), task_id)
            .await
            .unwrap();
        let edges = artifacts::fetch_task_edges(self.pool(), task_id)
            .await
            .unwrap();
        all.into_iter()
            .map(|a| {
                let name = edges
                    .iter()
                    .filter(|e| e.child_artifact_id == a.id && e.parent_artifact_id != a.id)
                    .min_by_key(|e| e.created_on)
                    .and_then(|e| e.name.clone())
                    .unwrap_or_else(|| "root".into());
                (name, a)
            })
            .collect()
    }

    async fn jobs(&self, task_id: i32) -> Vec<Job> {
        jobs::fetch_task_jobs(self.pool(), task_id).await.unwrap()
    }
}

fn jobs_on<'a>(jobs: &'a [Job], artifact: &TaskArtifact) -> Vec<&'a Job> {
    jobs.iter()
        .filter(|j| j.artifact_id == artifact.id)
        .collect()
}

fn job<'a>(jobs: &'a [Job], artifact: &TaskArtifact, analyzer: &str) -> &'a Job {
    jobs.iter()
        .find(|j| j.artifact_id == artifact.id && j.analyzer == analyzer)
        .unwrap_or_else(|| panic!("no {analyzer} job on artifact {}", artifact.id))
}

fn zip_routing() -> RoutingTable {
    routing(
        vec![
            spec("extractor", Stage::Extract, "archive/zip"),
            spec("pe", Stage::Static, "executable/.*"),
        ],
        vec![],
    )
}

fn extract_two(analyzer: &str, name: &str, _n: u32) -> Reply {
    if analyzer == "extractor" && name == "a.zip" {
        Reply::Outputs(vec![
            artifact("x.exe", b"MZ x", ArtifactRelation::Extracted),
            artifact("y.txt", b"hello", ArtifactRelation::Extracted),
        ])
    } else {
        Reply::Outputs(vec![])
    }
}

const ZIP: &[u8] = b"PK zip root";

// ----- tests ------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn zip_children_are_analysed() {
    let Some(env) = Env::new(|_| {}).await else {
        return;
    };
    let run = env
        .start(
            zip_routing(),
            FakeHostRunner::new(extract_two),
            FakeDetonationRunner::none(),
        )
        .await;
    let task_id = env
        .submit(&run, ZIP, "a.zip", SubmissionParams::default())
        .await;
    let task = env.wait_terminal(task_id).await;

    assert_eq!(task.status, TaskState::Completed);
    assert_eq!(task.artifact_count, 3);
    let arts = env.artifacts(task_id).await;
    let (x, y) = (&arts["x.exe"], &arts["y.txt"]);
    assert_eq!((x.depth, y.depth), (1, 1));
    assert!(x.analyze && y.analyze);

    let jobs = env.jobs(task_id).await;
    let pe: Vec<_> = jobs.iter().filter(|j| j.analyzer == "pe").collect();
    assert_eq!(pe.len(), 1);
    assert_eq!(pe[0].artifact_id, x.id);

    let edges = artifacts::fetch_task_edges(env.pool(), task_id)
        .await
        .unwrap();
    assert_eq!(edges.len(), 2);
    for e in &edges {
        assert_eq!(e.relation, "extracted");
        assert_eq!(e.source_plugin.as_deref(), Some("extractor"));
        assert_eq!(e.parent_artifact_id, arts["root"].id);
    }
    env.finish(run).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn depth_limit_drops() {
    let Some(env) = Env::new(|_| {}).await else {
        return;
    };
    let host = FakeHostRunner::new(|analyzer, name, _| {
        if analyzer == "extractor" {
            Reply::Outputs(vec![artifact(
                &format!("in-{name}"),
                format!("zip:{name}").as_bytes(),
                ArtifactRelation::Extracted,
            )])
        } else {
            Reply::Outputs(vec![])
        }
    });
    let run = env
        .start(zip_routing(), host, FakeDetonationRunner::none())
        .await;
    let params = SubmissionParams {
        max_depth: Some(2),
        ..Default::default()
    };
    let task_id = env.submit(&run, ZIP, "a.zip", params).await;
    let task = env.wait_terminal(task_id).await;

    assert_eq!(task.status, TaskState::Completed);
    assert!(task.partial);
    let arts = env.artifacts(task_id).await;
    let level1 = &arts["in-a.zip"];
    assert!(level1.analyze);
    assert_eq!(level1.depth, 1);
    let level2 = &arts["in-in-a.zip"];
    assert_eq!(level2.depth, 2);
    assert_eq!(level2.state, "dropped");
    assert_eq!(level2.drop_reason.as_deref(), Some("max_depth"));
    assert!(!level2.analyze);
    assert_eq!(arts.len(), 3);
    env.finish(run).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn self_extraction_breaks_loop() {
    let Some(env) = Env::new(|_| {}).await else {
        return;
    };
    let host = FakeHostRunner::new(|analyzer, _, _| {
        if analyzer == "extractor" {
            Reply::Outputs(vec![artifact("copy.zip", ZIP, ArtifactRelation::Extracted)])
        } else {
            Reply::Outputs(vec![])
        }
    });
    let run = env
        .start(
            zip_routing(),
            Arc::clone(&host),
            FakeDetonationRunner::none(),
        )
        .await;
    let task_id = env
        .submit(&run, ZIP, "a.zip", SubmissionParams::default())
        .await;
    let task = env.wait_terminal(task_id).await;

    assert_eq!(task.status, TaskState::Completed);
    let all = artifacts::fetch_task_artifacts(env.pool(), task_id)
        .await
        .unwrap();
    assert_eq!(all.len(), 1);
    let edges = artifacts::fetch_task_edges(env.pool(), task_id)
        .await
        .unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].parent_artifact_id, all[0].id);
    assert_eq!(edges[0].child_artifact_id, all[0].id);
    assert_eq!(host.calls("extractor", "a.zip"), 1);
    env.finish(run).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn recoverable_retry() {
    let Some(env) = Env::new(|c| c.max_attempts = 3).await else {
        return;
    };
    let host = FakeHostRunner::new(|_, _, n| {
        if n < 2 {
            Reply::Fail(JobRunError::Recoverable("flaky".into()))
        } else {
            Reply::Outputs(vec![])
        }
    });
    let run = env
        .start(
            routing(vec![spec("pe", Stage::Static, "executable/.*")], vec![]),
            host,
            FakeDetonationRunner::none(),
        )
        .await;
    let task_id = env
        .submit(&run, b"MZ r", "r.exe", SubmissionParams::default())
        .await;
    let task = env.wait_terminal(task_id).await;

    assert_eq!(task.status, TaskState::Completed);
    let jobs = env.jobs(task_id).await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].state, "completed");
    assert_eq!(jobs[0].attempts, 3);
    env.finish(run).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nonrecoverable_fails_job_not_task() {
    let Some(env) = Env::new(|_| {}).await else {
        return;
    };
    let host = FakeHostRunner::new(|analyzer, name, n| {
        if analyzer == "pe" {
            Reply::Fail(JobRunError::Nonrecoverable("bad".into()))
        } else {
            extract_two(analyzer, name, n)
        }
    });
    let run = env
        .start(zip_routing(), host, FakeDetonationRunner::none())
        .await;
    let task_id = env
        .submit(&run, ZIP, "a.zip", SubmissionParams::default())
        .await;
    let task = env.wait_terminal(task_id).await;

    assert_eq!(task.status, TaskState::Completed);
    assert_eq!(task.error_count, 1);
    let arts = env.artifacts(task_id).await;
    let jobs = env.jobs(task_id).await;
    let pe = job(&jobs, &arts["x.exe"], "pe");
    assert_eq!(pe.state, "failed");
    assert_eq!(pe.error_class.as_deref(), Some("nonrecoverable"));
    assert_eq!(pe.attempts, 1);
    env.finish(run).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn root_all_failed_fails_task() {
    let Some(env) = Env::new(|_| {}).await else {
        return;
    };
    let host =
        FakeHostRunner::new(|_, _, _| Reply::Fail(JobRunError::Nonrecoverable("bad".into())));
    let run = env
        .start(
            routing(vec![spec("pe", Stage::Static, "executable/.*")], vec![]),
            host,
            FakeDetonationRunner::none(),
        )
        .await;
    let task_id = env
        .submit(&run, b"MZ r", "r.exe", SubmissionParams::default())
        .await;
    let task = env.wait_terminal(task_id).await;

    assert_eq!(task.status, TaskState::Failed);
    assert_eq!(task.error_count, 1);
    env.finish(run).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cache_hit_replays_children() {
    let Some(env) = Env::new(|_| {}).await else {
        return;
    };
    let host = FakeHostRunner::new(extract_two);
    let run = env
        .start(
            zip_routing(),
            Arc::clone(&host),
            FakeDetonationRunner::none(),
        )
        .await;

    let first = env
        .submit(&run, ZIP, "a.zip", SubmissionParams::default())
        .await;
    assert_eq!(env.wait_terminal(first).await.status, TaskState::Completed);
    let second = env
        .submit(&run, ZIP, "a.zip", SubmissionParams::default())
        .await;
    let task = env.wait_terminal(second).await;
    assert_eq!(task.status, TaskState::Completed);

    let first_arts = env.artifacts(first).await;
    let first_jobs = env.jobs(first).await;
    let first_extractor = job(&first_jobs, &first_arts["root"], "extractor");
    assert_eq!(first_extractor.state, "completed");

    let arts = env.artifacts(second).await;
    let jobs = env.jobs(second).await;
    let extractor = job(&jobs, &arts["root"], "extractor");
    assert_eq!(extractor.state, "cached");
    assert_eq!(extractor.cached_from_job_id, Some(first_extractor.id));

    let shas = |a: &HashMap<String, TaskArtifact>| -> BTreeSet<String> {
        a.values().map(|x| x.sha256.clone()).collect()
    };
    assert_eq!(arts.len(), 3);
    assert_eq!(shas(&arts), shas(&first_arts));
    assert_eq!(task.artifact_count, 3);
    assert_eq!(host.calls("extractor", "a.zip"), 1);
    env.finish(run).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancel_child_keeps_siblings() {
    let Some(env) = Env::new(|_| {}).await else {
        return;
    };
    let host = FakeHostRunner::new(|analyzer, name, n| match (analyzer, name) {
        ("pe", "x.exe") => Reply::Block,
        _ => extract_two(analyzer, name, n),
    });
    let routing = routing(
        vec![
            spec("extractor", Stage::Extract, "archive/zip"),
            spec("pe", Stage::Static, "executable/.*"),
            spec("strings", Stage::Static, "text/.*"),
        ],
        vec![],
    );
    let run = env.start(routing, host, FakeDetonationRunner::none()).await;
    let task_id = env
        .submit(&run, ZIP, "a.zip", SubmissionParams::default())
        .await;

    let x = loop {
        let arts = env.artifacts(task_id).await;
        if let Some(x) = arts.get("x.exe") {
            let jobs = env.jobs(task_id).await;
            if jobs_on(&jobs, x)
                .iter()
                .any(|j| j.analyzer == "pe" && j.state == "running")
            {
                break x.clone();
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    run.handle
        .cancel_artifact(task_id, x.id, DescendantPolicy::Cancel)
        .await
        .expect("cancel artifact");
    let task = env.wait_terminal(task_id).await;

    assert_eq!(task.status, TaskState::Completed);
    assert!(task.partial);
    let arts = env.artifacts(task_id).await;
    assert_eq!(arts["x.exe"].state, "canceled");
    assert_eq!(arts["x.exe"].canceled_by.as_deref(), Some("user"));
    let jobs = env.jobs(task_id).await;
    assert_eq!(job(&jobs, &arts["x.exe"], "pe").state, "canceled");
    assert_eq!(arts["y.txt"].state, "done");
    assert_eq!(job(&jobs, &arts["y.txt"], "strings").state, "completed");
    env.finish(run).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn detonation_budget_by_interest() {
    let Some(env) = Env::new(|_| {}).await else {
        return;
    };
    let lease = env.seed_machine().await;
    let host = FakeHostRunner::new(|analyzer, name, _| {
        if analyzer == "extractor" && name == "a.zip" {
            Reply::Outputs(vec![
                artifact("a.exe", b"MZ a", ArtifactRelation::Extracted),
                artifact("b.sh", b"#!/bin/sh b", ArtifactRelation::Extracted),
                artifact("c.txt", b"c", ArtifactRelation::Extracted),
                artifact("d.exe", b"MZ d", ArtifactRelation::Extracted),
            ])
        } else {
            Reply::Outputs(vec![])
        }
    });
    let detonation = FakeDetonationRunner::slots(lease.clone(), 4, |_| mon(vec![]));
    let routing = routing(
        vec![spec("extractor", Stage::Extract, "archive/zip")],
        vec![profile("dyn", "executable/.*|code/.*")],
    );
    let run = env.start(routing, host, Arc::clone(&detonation)).await;
    let params = SubmissionParams {
        max_detonations: Some(2),
        ..Default::default()
    };
    let task_id = env.submit(&run, ZIP, "a.zip", params).await;
    let task = env.wait_terminal(task_id).await;

    assert_eq!(task.status, TaskState::Completed);
    assert!(task.partial);
    let arts = env.artifacts(task_id).await;
    let jobs = env.jobs(task_id).await;
    assert!(
        jobs_on(&jobs, &arts["root"])
            .iter()
            .all(|j| j.kind == "host")
    );
    for name in ["a.exe", "d.exe"] {
        let j = job(&jobs, &arts[name], "dyn");
        assert_eq!(j.state, "completed", "{name}");
        assert_eq!(j.guest_plugins.as_deref(), Some(&["mon".to_string()][..]));
        assert_eq!(j.machine_id, Some(lease.machine_id));
    }
    let b = job(&jobs, &arts["b.sh"], "dyn");
    assert_eq!(b.state, "skipped");
    assert_eq!(
        b.error_message.as_deref(),
        Some("detonation budget reached")
    );
    assert!(jobs_on(&jobs, &arts["c.txt"]).is_empty());
    let mut runs = detonation.runs();
    runs.sort();
    assert_eq!(runs, vec!["a.exe", "d.exe"]);
    env.finish(run).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dropped_files_not_redetonated() {
    let Some(env) = Env::new(|_| {}).await else {
        return;
    };
    let lease = env.seed_machine().await;
    let host = FakeHostRunner::new(|_, _, _| Reply::Outputs(vec![]));
    let detonation = FakeDetonationRunner::slots(lease, 2, |name| {
        if name == "r.exe" {
            mon(vec![artifact(
                "drop.exe",
                b"MZ dropped",
                ArtifactRelation::Dropped,
            )])
        } else {
            mon(vec![])
        }
    });
    let routing = routing(
        vec![spec("pe", Stage::Static, "executable/.*")],
        vec![profile("dyn", "executable/.*")],
    );
    let run = env.start(routing, host, detonation).await;

    for (mode, expected_detonations) in [(ChildDetonation::Extracted, 0), (ChildDetonation::All, 1)]
    {
        let params = SubmissionParams {
            detonate_children: mode,
            ..Default::default()
        };
        let task_id = env.submit(&run, b"MZ root", "r.exe", params).await;
        let task = env.wait_terminal(task_id).await;
        assert_eq!(task.status, TaskState::Completed, "{mode:?}");

        let arts = env.artifacts(task_id).await;
        let jobs = env.jobs(task_id).await;
        assert_eq!(job(&jobs, &arts["root"], "dyn").state, "completed");
        let dropped = &arts["drop.exe"];
        assert!(dropped.analyze, "{mode:?}");
        let on_drop = jobs_on(&jobs, dropped);
        assert!(on_drop.iter().any(|j| j.analyzer == "pe"), "{mode:?}");
        let detonations: Vec<_> = on_drop.iter().filter(|j| j.kind == "detonation").collect();
        assert_eq!(detonations.len(), expected_detonations, "{mode:?}");
        for d in detonations {
            assert_eq!(d.state, "completed");
        }
        let edges = artifacts::fetch_task_edges(env.pool(), task_id)
            .await
            .unwrap();
        let edge = edges
            .iter()
            .find(|e| e.child_artifact_id == dropped.id)
            .unwrap();
        assert_eq!(edge.relation, "dropped");
        assert_eq!(edge.source_plugin.as_deref(), Some("mon"));
    }
    env.finish(run).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn monitored_key_rerun() {
    let Some(env) = Env::new(|_| {}).await else {
        return;
    };
    let host = FakeHostRunner::new(|analyzer, name, n| match (analyzer, name, n) {
        ("extractor", "a.zip", 0) => Reply::Outputs(vec![report("extractor", &["encrypted"])]),
        ("extractor", "a.zip", _) => Reply::Outputs(vec![artifact(
            "x.exe",
            b"MZ x",
            ArtifactRelation::Extracted,
        )]),
        ("pw", _, _) => Reply::Outputs(vec![shared(&json!({"passwords": ["x"]}))]),
        _ => Reply::Outputs(vec![]),
    });
    let routing = routing(
        vec![
            AnalyzerSpec {
                monitored_keys: vec!["passwords".into()],
                ..spec("extractor", Stage::Extract, "archive/zip")
            },
            spec("pw", Stage::Static, "archive/zip"),
        ],
        vec![],
    );
    let run = env
        .start(routing, Arc::clone(&host), FakeDetonationRunner::none())
        .await;
    let task_id = env
        .submit(&run, ZIP, "a.zip", SubmissionParams::default())
        .await;
    let task = env.wait_terminal(task_id).await;

    assert_eq!(task.status, TaskState::Completed);
    let arts = env.artifacts(task_id).await;
    assert!(arts.contains_key("x.exe"));
    let jobs = env.jobs(task_id).await;
    let extractor = job(&jobs, &arts["root"], "extractor");
    assert_eq!(extractor.reruns, 1);
    assert_eq!(extractor.state, "completed");
    assert_eq!(host.calls("extractor", "a.zip"), 2);
    assert_eq!(task.shared_data, json!({"passwords": ["x"]}));
    env.finish(run).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restart_recovers_running_job() {
    let Some(env) = Env::new(|_| {}).await else {
        return;
    };
    let (task, root) = env
        .create(b"MZ r", "r.exe", SubmissionParams::default())
        .await;
    let inserted = jobs::insert_job(
        env.pool(),
        &NewJob {
            task_id: task.id,
            artifact_id: root.id,
            kind: "host",
            analyzer: "pe",
            stage: "static",
            priority: 1,
            state: "running",
            max_attempts: 3,
            ignore_cache: false,
            cache_key: None,
            snapshot_id: None,
            guest_plugins: None,
            detonation: None,
        },
    )
    .await
    .expect("insert job");
    sqlx::query("update jobs set attempts = 1 where id = $1")
        .bind(inserted.id)
        .execute(env.pool())
        .await
        .unwrap();

    let host = FakeHostRunner::new(|_, _, _| Reply::Outputs(vec![]));
    let run = env
        .start(
            routing(vec![spec("pe", Stage::Static, "executable/.*")], vec![]),
            Arc::clone(&host),
            FakeDetonationRunner::none(),
        )
        .await;
    let finished = env.wait_terminal(task.id).await;

    assert_eq!(finished.status, TaskState::Completed);
    let jobs = env.jobs(task.id).await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, inserted.id);
    assert_eq!(jobs[0].state, "completed");
    assert_eq!(jobs[0].attempts, 2);
    assert_eq!(host.calls("pe", "r.exe"), 1);
    env.finish(run).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unserviceable_detonation() {
    let Some(env) = Env::new(|_| {}).await else {
        return;
    };
    let run = env
        .start(
            routing(vec![], vec![profile("dyn", ".*")]),
            FakeHostRunner::new(|_, _, _| Reply::Outputs(vec![])),
            FakeDetonationRunner::unserviceable("no machine"),
        )
        .await;
    let task_id = env
        .submit(&run, b"MZ r", "r.exe", SubmissionParams::default())
        .await;
    let task = env.wait_terminal(task_id).await;

    let jobs = env.jobs(task_id).await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].kind, "detonation");
    assert_eq!(jobs[0].state, "failed");
    assert_eq!(jobs[0].error_class.as_deref(), Some("nonrecoverable"));
    assert_eq!(jobs[0].error_message.as_deref(), Some("no machine"));
    assert_eq!(task.status, TaskState::Failed);
    env.finish(run).await;
}
