extern crate malbox_plugin_sdk as malbox;

use malbox::prelude::*;
use std::sync::Mutex;

/// Demonstrates plugin result chaining - a second-pass plugin that reacts
/// to results produced by upstream plugins and produces derived output.
///
/// The flow:
/// 1. An upstream plugin (e.g. host-file-info) processes a task and pushes
///    results like "file_info" containing hashes and sizes.
/// 2. The daemon delivers a `PluginResultAvailable` event to this plugin.
/// 3. This plugin's `on_result_available` handler records which results have
///    arrived from which source plugins.
/// 4. When a task completes, `on_task_complete` builds a summary report
///    aggregating everything seen and pushes its own result.
///
/// This pattern lets you build pipelines: file-info -> enrichment -> scoring
/// without any plugin needing direct knowledge of the others.
///
/// Note: for the daemon to deliver `PluginResultAvailable` events from a
/// specific plugin, the runtime must be constructed with that plugin ID in
/// its `subscribed_plugins` list. The macro currently passes an empty list;
/// in production, the daemon's plugin manager wires subscriptions based on
/// the plugin manifest's declared dependencies.
#[malbox::host_plugin]
struct ResultChainPlugin {
    upstream_results: Mutex<Vec<UpstreamResult>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpstreamResult {
    source_plugin: String,
    result_name: String,
}

#[derive(Serialize)]
struct ChainSummary {
    upstream_count: usize,
    sources: Vec<String>,
    results: Vec<UpstreamResult>,
    verdict: &'static str,
}

#[malbox::handlers]
impl ResultChainPlugin {
    #[malbox::on_start]
    fn init(&self) -> Result<()> {
        info!("ResultChain plugin started - waiting for upstream results");
        Ok(())
    }

    /// Called each time an upstream plugin produces a result.
    ///
    /// The event carries `source` (the plugin ID) and `result_name` (which
    /// result was produced). The actual result data is accessible through
    /// the result pub/sub channel at the transport layer.
    #[malbox::on_event(PluginResultAvailable)]
    fn on_result_available(&self) -> Result<()> {
        // In a real plugin, you'd extract source and result_name from the
        // event payload. For now, track that we saw a result.
        if let Ok(mut results) = self.upstream_results.lock() {
            results.push(UpstreamResult {
                source_plugin: "upstream".into(),
                result_name: "unknown".into(),
            });
        }
        info!("Upstream result received - queued for aggregation");
        Ok(())
    }

    /// Process a task by building a summary of all upstream results.
    ///
    /// In a real chaining scenario, this handler would:
    /// 1. Read the actual result data from upstream plugins
    /// 2. Correlate and enrich the data
    /// 3. Produce derived results (e.g. a combined threat score)
    #[malbox::on_task]
    fn aggregate(&self, ctx: &Context) -> Result<()> {
        let results: Vec<UpstreamResult> = self
            .upstream_results
            .lock()
            .map(|mut r| std::mem::take(&mut *r))
            .unwrap_or_default();

        let sources: Vec<String> = results
            .iter()
            .map(|r| r.source_plugin.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let upstream_count = results.len();
        let verdict = if upstream_count > 0 {
            "enriched"
        } else {
            "no-upstream-data"
        };

        ctx.progress(0.5, &format!("aggregating {} upstream results", upstream_count))?;

        let summary = ChainSummary {
            upstream_count,
            sources,
            results,
            verdict,
        };

        ctx.results()
            .push(PluginResult::json("chain_summary", &summary)?)?;

        let report = ReportBuilder::new("host-result-chain", "0.1.0")
            .display_name("Result Chain Aggregation")
            .summary(&format!(
                "Aggregated {} results from upstream plugins",
                upstream_count
            ))
            .labels(["meta-analysis", "chaining"])
            .section("summary", "Chain Summary", |s| {
                s.block(Block::Kv {
                    pairs: vec![
                        KvPair {
                            key: "Upstream results".into(),
                            value: upstream_count.to_string(),
                            mono: false,
                        },
                        KvPair {
                            key: "Verdict".into(),
                            value: verdict.into(),
                            mono: false,
                        },
                    ],
                })
            })
            .build();

        ctx.results().push(report.into_plugin_result()?)?;

        info!(
            task_id = ctx.task().id(),
            upstream_count, verdict, "Chain aggregation complete"
        );
        Ok(())
    }

    #[malbox::on_event(TaskCompleted)]
    fn on_task_complete(&self) {
        let count = self
            .upstream_results
            .lock()
            .map(|r| r.len())
            .unwrap_or(0);
        info!(
            pending_results = count,
            "Task completed - upstream results flushed"
        );
    }

    #[malbox::on_stop]
    fn shutdown(&self) -> Result<()> {
        info!("ResultChain plugin shutting down");
        Ok(())
    }
}
