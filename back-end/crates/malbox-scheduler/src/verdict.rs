//! Sample-level verdict aggregation and task verdict selection.

use std::collections::BTreeSet;

use malbox_database::repositories::plugin_reports::{self, DbClassification, PluginReport};
use malbox_database::repositories::sample_verdicts::{self, UpsertSampleVerdict};
use tracing::error;

use crate::OrchestratorCore;

/// Recompute the latest-wins verdict of a sample after one of its artifacts
/// finished. Failures are logged; verdicts are a derived view.
pub async fn recompute_sample_verdict(core: &OrchestratorCore, sample_id: i64, task_id: i32) {
    let pool = &core.db;
    let reports =
        match plugin_reports::fetch_latest_plugin_reports_for_sample(pool, sample_id).await {
            Ok(r) => r,
            Err(e) => {
                error!(sample_id, error = %e, "Failed to fetch latest plugin reports for verdict");
                return;
            }
        };
    let task_count = match plugin_reports::count_reported_tasks_for_sample(pool, sample_id).await {
        Ok(n) => i32::try_from(n).unwrap_or(i32::MAX),
        Err(e) => {
            error!(sample_id, error = %e, "Failed to count reported tasks for verdict");
            return;
        }
    };

    let mut worst: Option<DbClassification> = None;
    let mut max_score: Option<i16> = None;
    let mut plugin_names: BTreeSet<String> = BTreeSet::new();
    let mut iocs: BTreeSet<(String, String)> = BTreeSet::new();
    let mut ttps: BTreeSet<String> = BTreeSet::new();

    for r in &reports {
        plugin_names.insert(r.plugin_name.clone());
        if let Some(c) = r.classification {
            worst = Some(match worst {
                Some(w) if w.severity() >= c.severity() => w,
                _ => c,
            });
        }
        if let Some(s) = r.score {
            max_score = Some(max_score.map_or(s, |m| m.max(s)));
        }
        if let Some(arr) = r.indicators.as_array() {
            for ind in arr {
                if let (Some(k), Some(v)) = (ind["kind"].as_str(), ind["value"].as_str()) {
                    iocs.insert((k.to_string(), v.to_string()));
                }
            }
        }
        if let Some(arr) = r.ttps.as_array() {
            for ttp in arr {
                if let Some(id) = ttp["id"].as_str() {
                    ttps.insert(id.to_string());
                }
            }
        }
    }

    let names: Vec<String> = plugin_names.into_iter().collect();
    let params = UpsertSampleVerdict {
        sample_id,
        classification: worst,
        score: max_score,
        indicator_count: iocs.len() as i32,
        ttp_count: ttps.len() as i32,
        plugin_names: &names,
        task_count,
        last_task_id: Some(task_id),
    };
    if let Err(e) = sample_verdicts::upsert_sample_verdict(pool, &params).await {
        error!(sample_id, error = %e, "Failed to upsert sample verdict");
    }
}

/// The report that decides a task's verdict: worst classification, then
/// higher score, then lower artifact id.
pub fn task_verdict(reports: &[PluginReport]) -> Option<&PluginReport> {
    reports
        .iter()
        .filter(|r| r.classification.is_some())
        .min_by_key(|r| {
            (
                std::cmp::Reverse(r.classification.map_or(0, |c| c.severity())),
                std::cmp::Reverse(r.score.map_or(i32::MIN, i32::from)),
                r.artifact_id,
            )
        })
}
