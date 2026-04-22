//! Ergonomic builder for the [`Report`] envelope.
//!
//! See the module-level docs on `super` for the envelope semantics. This
//! module is purely about a fluent construction API so `on_task` stays tight.

use super::{
    ArtifactRef, Block, CalloutLevel, Classification, Column, Confidence, GraphEdge, GraphNode,
    Indicator, KvPair, PluginInfo, Report, SCHEMA_VERSION, Section, TimelineEvent, TreeNode, Ttp,
    Verdict,
};

// ---------- Small constructors on the leaf types ----------

impl Indicator {
    pub fn new(kind: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            value: value.into(),
            context: None,
            first_seen: None,
        }
    }
    pub fn context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }
    pub fn first_seen(mut self, ts: impl Into<String>) -> Self {
        self.first_seen = Some(ts.into());
        self
    }
}

impl Ttp {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            evidence: None,
        }
    }
    pub fn evidence(mut self, ev: impl Into<String>) -> Self {
        self.evidence = Some(ev.into());
        self
    }
}

impl ArtifactRef {
    pub fn new(result_name: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            result_name: result_name.into(),
            kind: kind.into(),
            description: None,
        }
    }
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

// ---------- ReportBuilder ----------

/// Fluent builder for a [`Report`]. Finalise with [`ReportBuilder::build`].
pub struct ReportBuilder {
    report: Report,
}

impl ReportBuilder {
    /// Start a new report for a plugin. `id` and `version` typically come
    /// from `env!("CARGO_PKG_NAME")` / `env!("CARGO_PKG_VERSION")`.
    pub fn new(plugin_id: impl Into<String>, plugin_version: impl Into<String>) -> Self {
        Self {
            report: Report {
                schema_version: SCHEMA_VERSION,
                plugin: PluginInfo {
                    id: plugin_id.into(),
                    version: plugin_version.into(),
                    display_name: None,
                },
                verdict: None,
                indicators: Vec::new(),
                ttps: Vec::new(),
                artifacts: Vec::new(),
                summary: None,
                sections: Vec::new(),
                raw: None,
            },
        }
    }

    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.report.plugin.display_name = Some(name.into());
        self
    }

    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.report.summary = Some(summary.into());
        self
    }

    /// Set the verdict. `score` is optional (pass `None` when the plugin
    /// has no numeric score).
    pub fn verdict(
        mut self,
        classification: Classification,
        score: Option<u8>,
        confidence: Option<Confidence>,
    ) -> Self {
        self.report.verdict = Some(Verdict {
            classification,
            score,
            confidence,
            labels: self.report.verdict.map(|v| v.labels).unwrap_or_default(),
        });
        self
    }

    pub fn labels<I, S>(mut self, labels: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let labels: Vec<String> = labels.into_iter().map(Into::into).collect();
        match &mut self.report.verdict {
            Some(v) => v.labels.extend(labels),
            None => {
                self.report.verdict = Some(Verdict {
                    classification: Classification::Unknown,
                    score: None,
                    confidence: None,
                    labels,
                });
            }
        }
        self
    }

    pub fn indicator(mut self, ind: Indicator) -> Self {
        self.report.indicators.push(ind);
        self
    }

    pub fn ttp(mut self, ttp: Ttp) -> Self {
        self.report.ttps.push(ttp);
        self
    }

    pub fn artifact(mut self, artifact: ArtifactRef) -> Self {
        self.report.artifacts.push(artifact);
        self
    }

    /// Append a section. The closure receives a [`SectionBuilder`] and
    /// returns it — the typical shape is `|s| s.heading(2, "...").table(...)`.
    pub fn section<F>(mut self, id: impl Into<String>, title: impl Into<String>, build: F) -> Self
    where
        F: FnOnce(SectionBuilder) -> SectionBuilder,
    {
        let sb = SectionBuilder::new(id, title);
        self.report.sections.push(build(sb).build());
        self
    }

    /// Attach the plugin's native JSON as an escape hatch. Rarely needed —
    /// prefer blocks, but handy when wrapping a legacy pipeline.
    pub fn raw(mut self, value: impl serde::Serialize) -> Self {
        // If serialization somehow fails, drop silently — `raw` is optional.
        if let Ok(v) = serde_json::to_value(value) {
            self.report.raw = Some(v);
        }
        self
    }

    pub fn build(self) -> Report {
        self.report
    }
}

// ---------- SectionBuilder ----------

/// Builder for a single [`Section`]. One method per [`Block`] variant, plus
/// a `block(...)` escape hatch for forward compatibility.
pub struct SectionBuilder {
    section: Section,
}

impl SectionBuilder {
    pub(crate) fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            section: Section {
                id: id.into(),
                title: title.into(),
                blocks: Vec::new(),
            },
        }
    }

    /// Escape hatch — push any [`Block`] directly.
    pub fn block(mut self, block: Block) -> Self {
        self.section.blocks.push(block);
        self
    }

    pub fn markdown(self, text: impl Into<String>) -> Self {
        self.block(Block::Markdown { text: text.into() })
    }

    pub fn callout(self, level: CalloutLevel, text: impl Into<String>) -> Self {
        self.block(Block::Callout {
            level,
            text: text.into(),
        })
    }

    pub fn heading(self, level: u8, text: impl Into<String>) -> Self {
        self.block(Block::Heading {
            level,
            text: text.into(),
        })
    }

    pub fn divider(self) -> Self {
        self.block(Block::Divider)
    }

    pub fn kv(self, pairs: impl IntoIterator<Item = KvPair>) -> Self {
        self.block(Block::Kv {
            pairs: pairs.into_iter().collect(),
        })
    }

    pub fn table(
        self,
        columns: impl IntoIterator<Item = Column>,
        rows: impl IntoIterator<Item = serde_json::Value>,
    ) -> Self {
        self.block(Block::Table {
            columns: columns.into_iter().collect(),
            rows: rows.into_iter().collect(),
            sortable: true,
            searchable: false,
        })
    }

    pub fn code(self, language: impl Into<String>, text: impl Into<String>) -> Self {
        self.block(Block::Code {
            language: language.into(),
            text: text.into(),
        })
    }

    pub fn json(self, data: impl serde::Serialize) -> Self {
        let v = serde_json::to_value(data).unwrap_or(serde_json::Value::Null);
        self.block(Block::Json {
            data: v,
            collapsed: true,
        })
    }

    pub fn hex(self, bytes_b64: impl Into<String>, offset: u64) -> Self {
        self.block(Block::Hex {
            bytes_b64: bytes_b64.into(),
            offset,
        })
    }

    pub fn image(self, artifact: impl Into<String>, caption: Option<String>) -> Self {
        self.block(Block::Image {
            artifact: artifact.into(),
            caption,
        })
    }

    pub fn download(self, artifact: impl Into<String>, label: impl Into<String>) -> Self {
        self.block(Block::Download {
            artifact: artifact.into(),
            label: label.into(),
        })
    }

    pub fn iocs(self, items: impl IntoIterator<Item = Indicator>) -> Self {
        self.block(Block::Iocs {
            items: items.into_iter().collect(),
        })
    }

    pub fn ttps(self, items: impl IntoIterator<Item = Ttp>) -> Self {
        self.block(Block::Ttps {
            items: items.into_iter().collect(),
        })
    }

    pub fn tree(self, nodes: impl IntoIterator<Item = TreeNode>) -> Self {
        self.block(Block::Tree {
            nodes: nodes.into_iter().collect(),
        })
    }

    pub fn timeline(self, events: impl IntoIterator<Item = TimelineEvent>) -> Self {
        self.block(Block::Timeline {
            events: events.into_iter().collect(),
        })
    }

    pub fn graph(
        self,
        nodes: impl IntoIterator<Item = GraphNode>,
        edges: impl IntoIterator<Item = GraphEdge>,
    ) -> Self {
        self.block(Block::Graph {
            nodes: nodes.into_iter().collect(),
            edges: edges.into_iter().collect(),
        })
    }

    pub(crate) fn build(self) -> Section {
        self.section
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builder_produces_minimal_report() {
        let r = ReportBuilder::new("yara", "1.0.0").build();
        assert_eq!(r.schema_version, SCHEMA_VERSION);
        assert_eq!(r.plugin.id, "yara");
        assert!(r.verdict.is_none());
        assert!(r.sections.is_empty());
    }

    #[test]
    fn builder_sets_verdict_and_labels() {
        let r = ReportBuilder::new("yara", "1.0.0")
            .verdict(Classification::Malicious, Some(87), Some(Confidence::High))
            .labels(["trojan", "stealer"])
            .build();
        let v = r.verdict.unwrap();
        assert_eq!(v.classification, Classification::Malicious);
        assert_eq!(v.score, Some(87));
        assert_eq!(v.labels, vec!["trojan", "stealer"]);
    }

    #[test]
    fn labels_before_verdict_still_works() {
        let r = ReportBuilder::new("p", "0")
            .labels(["tagA"])
            .verdict(Classification::Suspicious, None, None)
            .build();
        let v = r.verdict.unwrap();
        assert_eq!(v.classification, Classification::Suspicious);
        assert_eq!(v.labels, vec!["tagA"]);
    }

    #[test]
    fn section_builder_adds_blocks_in_order() {
        let r = ReportBuilder::new("yara", "1.0.0")
            .section("overview", "Overview", |s| {
                s.heading(2, "Rules")
                    .markdown("1 match")
                    .code("yara", "rule r { condition: true }")
                    .divider()
            })
            .build();

        let sec = &r.sections[0];
        assert_eq!(sec.id, "overview");
        assert_eq!(sec.blocks.len(), 4);
        assert!(matches!(sec.blocks[0], Block::Heading { .. }));
        assert!(matches!(sec.blocks[3], Block::Divider));
    }

    #[test]
    fn indicator_and_ttp_helpers_work() {
        let r = ReportBuilder::new("p", "0")
            .indicator(Indicator::new("sha256", "abc").context("sample"))
            .indicator(Indicator::new("ipv4", "1.2.3.4"))
            .ttp(Ttp::new("T1055", "Process Injection").evidence("memdump"))
            .artifact(ArtifactRef::new("cap.pcap", "pcap").description("Full capture"))
            .build();

        assert_eq!(r.indicators.len(), 2);
        assert_eq!(r.indicators[0].context.as_deref(), Some("sample"));
        assert_eq!(r.ttps[0].evidence.as_deref(), Some("memdump"));
        assert_eq!(r.artifacts[0].kind, "pcap");
    }

    #[test]
    fn raw_escape_hatch_stores_json() {
        #[derive(serde::Serialize)]
        struct Native {
            hit: u32,
        }
        let r = ReportBuilder::new("p", "0").raw(Native { hit: 3 }).build();
        assert_eq!(r.raw.unwrap(), json!({"hit": 3}));
    }
}
