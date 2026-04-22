//! The `Report` envelope — a structured, frontend-renderable result.
//!
//! Plugins produce one `PluginResult::Json { name: "report", ... }` per task
//! containing a [`Report`]. The scheduler tags that row with `role='report'`
//! in the `task_results` table and the API aggregates it into the task view.
//!
//! The envelope has two layers:
//!
//! * A **semantic layer** (`verdict`, `indicators`, `ttps`, `artifacts`) the
//!   platform understands and can query across tasks.
//! * A **presentation layer** (`sections` of typed [`Block`]s) the frontend
//!   renders generically. Unknown block types degrade to a JSON tree view on
//!   the client, so adding new variants is never a breaking change.
//!
//! See `crates/malbox-plugin-sdk/src/types/report/builder.rs` for the
//! ergonomic [`ReportBuilder`] API.

use crate::error::Result;
use crate::types::PluginResult;
use serde::{Deserialize, Serialize};

pub mod builder;

pub use builder::{ReportBuilder, SectionBuilder};

/// Current schema version of the report envelope.
pub const SCHEMA_VERSION: u32 = 1;

/// The well-known `result_name` the scheduler and API use to identify a
/// report envelope among a task's outputs. Defined in `malbox-plugin-transport`
/// so the SDK and scheduler share the same constant without a direct dep.
pub use malbox_plugin_transport::REPORT_RESULT_NAME;

/// A plugin's structured, renderable result for a single task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: u32,
    pub plugin: PluginInfo,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict: Option<Verdict>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub indicators: Vec<Indicator>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ttps: Vec<Ttp>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ArtifactRef>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<Section>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

impl Report {
    /// Serialize this report into a [`PluginResult::Json`] with the
    /// well-known [`REPORT_RESULT_NAME`] name, ready for `ctx.push_result`.
    pub fn into_plugin_result(self) -> Result<PluginResult> {
        PluginResult::json(REPORT_RESULT_NAME, &self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    pub classification: Classification,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    Clean,
    Suspicious,
    Malicious,
    Unknown,
}

impl Classification {
    /// Precedence for aggregating plugin verdicts: worst wins.
    pub fn severity(self) -> u8 {
        match self {
            Classification::Clean => 0,
            Classification::Unknown => 1,
            Classification::Suspicious => 2,
            Classification::Malicious => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

/// An indicator of compromise. `kind` is an open vocabulary so plugins can
/// emit kinds the SDK doesn't know about yet; the frontend renders any kind.
/// Common values: `sha256`, `md5`, `sha1`, `ipv4`, `ipv6`, `domain`, `url`,
/// `email`, `mutex`, `registry`, `filepath`, `yara_rule`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Indicator {
    pub kind: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_seen: Option<String>,
}

/// A MITRE ATT&CK technique observation. `id` uses the canonical `T####`
/// (or `T####.###` for sub-techniques) form.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ttp {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// A reference to a sibling `PluginResult` produced by the same plugin in
/// the same task — used by `Block::Image` / `Block::Download` to resolve
/// artifact URLs on the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub result_name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<Block>,
}

/// A renderable block. The frontend dispatches on `type`; unknown types
/// are rendered as a JSON tree so additions are non-breaking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Markdown {
        text: String,
    },
    Callout {
        level: CalloutLevel,
        text: String,
    },
    Heading {
        level: u8,
        text: String,
    },
    Divider,
    Kv {
        pairs: Vec<KvPair>,
    },
    Table {
        columns: Vec<Column>,
        rows: Vec<serde_json::Value>,
        #[serde(default)]
        sortable: bool,
        #[serde(default)]
        searchable: bool,
    },
    Code {
        language: String,
        text: String,
    },
    Json {
        data: serde_json::Value,
        #[serde(default)]
        collapsed: bool,
    },
    Hex {
        bytes_b64: String,
        #[serde(default)]
        offset: u64,
    },
    Image {
        artifact: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        caption: Option<String>,
    },
    Download {
        artifact: String,
        label: String,
    },
    Iocs {
        items: Vec<Indicator>,
    },
    Ttps {
        items: Vec<Ttp>,
    },
    Tree {
        nodes: Vec<TreeNode>,
    },
    Timeline {
        events: Vec<TimelineEvent>,
    },
    Graph {
        nodes: Vec<GraphNode>,
        edges: Vec<GraphEdge>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalloutLevel {
    Info,
    Success,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvPair {
    pub key: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mono: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub key: String,
    pub label: String,
    /// Hint for rendering: `"string"`, `"number"`, `"bool"`, `"datetime"`, ...
    #[serde(default = "default_column_type")]
    pub r#type: String,
}

fn default_column_type() -> String {
    "string".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TreeNode>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub meta: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub ts: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub meta: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub meta: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn minimal_report() -> Report {
        Report {
            schema_version: SCHEMA_VERSION,
            plugin: PluginInfo {
                id: "yara".into(),
                version: "1.0.0".into(),
                display_name: None,
            },
            verdict: None,
            indicators: vec![],
            ttps: vec![],
            artifacts: vec![],
            summary: None,
            sections: vec![],
            raw: None,
        }
    }

    #[test]
    fn minimal_report_omits_optional_fields() {
        let v = serde_json::to_value(minimal_report()).unwrap();
        assert_eq!(v["schema_version"], 1);
        assert_eq!(v["plugin"]["id"], "yara");
        // Nones and empty vecs must not appear in the output.
        assert!(v.get("verdict").is_none());
        assert!(v.get("summary").is_none());
        assert!(v.get("indicators").is_none());
        assert!(v.get("ttps").is_none());
        assert!(v.get("sections").is_none());
        assert!(v.get("raw").is_none());
        // display_name also absent.
        assert!(v["plugin"].get("display_name").is_none());
    }

    #[test]
    fn classification_serializes_snake_case() {
        let v = serde_json::to_value(Classification::Malicious).unwrap();
        assert_eq!(v, json!("malicious"));
    }

    #[test]
    fn classification_severity_orders_correctly() {
        assert!(Classification::Malicious.severity() > Classification::Suspicious.severity());
        assert!(Classification::Suspicious.severity() > Classification::Unknown.severity());
        assert!(Classification::Unknown.severity() > Classification::Clean.severity());
    }

    #[test]
    fn block_is_tagged_with_type() {
        let b = Block::Heading {
            level: 2,
            text: "Matches".into(),
        };
        let v = serde_json::to_value(&b).unwrap();
        assert_eq!(v["type"], "heading");
        assert_eq!(v["level"], 2);
        assert_eq!(v["text"], "Matches");
    }

    #[test]
    fn divider_block_has_no_extra_fields() {
        let v = serde_json::to_value(Block::Divider).unwrap();
        assert_eq!(v, json!({"type": "divider"}));
    }

    #[test]
    fn full_report_round_trips() {
        let r = Report {
            schema_version: SCHEMA_VERSION,
            plugin: PluginInfo {
                id: "yara".into(),
                version: "1.0.0".into(),
                display_name: Some("YARA Scanner".into()),
            },
            verdict: Some(Verdict {
                classification: Classification::Malicious,
                score: Some(87),
                confidence: Some(Confidence::High),
                labels: vec!["trojan".into()],
            }),
            indicators: vec![Indicator {
                kind: "sha256".into(),
                value: "deadbeef".into(),
                context: Some("sample".into()),
                first_seen: None,
            }],
            ttps: vec![Ttp {
                id: "T1055".into(),
                name: "Process Injection".into(),
                evidence: None,
            }],
            artifacts: vec![ArtifactRef {
                result_name: "details.json".into(),
                kind: "other".into(),
                description: None,
            }],
            summary: Some("Matched 1 rule".into()),
            sections: vec![Section {
                id: "matches".into(),
                title: "Matches".into(),
                blocks: vec![
                    Block::Markdown {
                        text: "Details".into(),
                    },
                    Block::Table {
                        columns: vec![Column {
                            key: "rule".into(),
                            label: "Rule".into(),
                            r#type: "string".into(),
                        }],
                        rows: vec![json!({"rule": "r1"})],
                        sortable: true,
                        searchable: false,
                    },
                ],
            }],
            raw: Some(json!({"native": 1})),
        };

        let s = serde_json::to_string(&r).unwrap();
        let parsed: Report = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        assert_eq!(
            parsed.verdict.as_ref().unwrap().classification,
            Classification::Malicious
        );
        assert_eq!(parsed.indicators.len(), 1);
        assert_eq!(parsed.ttps[0].id, "T1055");
        assert_eq!(parsed.sections[0].blocks.len(), 2);
    }

    /// Parity golden: this string MUST remain byte-for-byte identical to
    /// `GOLDEN_JSON` in
    /// `crates/malbox-plugin-sdk-cpp/tests/cpp/test_report.cpp`. If either
    /// SDK's serializer drifts, one of these two tests will fail.
    const PARITY_GOLDEN: &str = concat!(
        "{",
        "\"schema_version\":1,",
        "\"plugin\":{\"id\":\"yara\",\"version\":\"1.0.0\",\"display_name\":\"YARA Scanner\"},",
        "\"verdict\":{\"classification\":\"malicious\",\"score\":87,\"confidence\":\"high\",\"labels\":[\"trojan\"]},",
        "\"indicators\":[{\"kind\":\"sha256\",\"value\":\"abc\",\"context\":\"sample\"}],",
        "\"ttps\":[{\"id\":\"T1055\",\"name\":\"Process Injection\"}],",
        "\"artifacts\":[{\"result_name\":\"details.json\",\"kind\":\"other\"}],",
        "\"summary\":\"Matched 1 rule\",",
        "\"sections\":[{\"id\":\"overview\",\"title\":\"Overview\",\"blocks\":[",
        "{\"type\":\"heading\",\"level\":2,\"text\":\"Rules\"},",
        "{\"type\":\"markdown\",\"text\":\"1 match\"},",
        "{\"type\":\"divider\"}",
        "]}]",
        "}"
    );

    #[test]
    fn parity_golden_matches_rust_serialization() {
        // Build the equivalent report via the Rust builder and compare.
        let r = ReportBuilder::new("yara", "1.0.0")
            .display_name("YARA Scanner")
            .summary("Matched 1 rule")
            .verdict(Classification::Malicious, Some(87), Some(Confidence::High))
            .labels(["trojan"])
            .indicator(Indicator::new("sha256", "abc").context("sample"))
            .ttp(Ttp::new("T1055", "Process Injection"))
            .artifact(ArtifactRef::new("details.json", "other"))
            .section("overview", "Overview", |s| {
                s.heading(2, "Rules").markdown("1 match").divider()
            })
            .build();

        let got = serde_json::to_string(&r).unwrap();
        assert_eq!(
            got, PARITY_GOLDEN,
            "Rust Report serialization drifted from parity golden.\n\
             If this is intentional, update BOTH:\n\
             - `PARITY_GOLDEN` in this file\n\
             - `GOLDEN_JSON` in malbox-plugin-sdk-cpp/tests/cpp/test_report.cpp"
        );
    }

    #[test]
    fn into_plugin_result_uses_well_known_name() {
        let r = minimal_report();
        let pr = r.into_plugin_result().expect("serialize");
        match pr {
            PluginResult::Json { name, data } => {
                assert_eq!(name, REPORT_RESULT_NAME);
                let parsed: serde_json::Value = serde_json::from_slice(&data).unwrap();
                assert_eq!(parsed["schema_version"], 1);
            }
            _ => panic!("expected Json variant"),
        }
    }
}
