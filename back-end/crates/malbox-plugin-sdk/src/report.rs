//! The `Report` envelope - a structured, frontend-renderable result.
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
//! See [`builder`] for the ergonomic [`ReportBuilder`] API.

use crate::error::Result;
use crate::result::PluginResult;
use serde::{Deserialize, Serialize};

pub mod builder;

pub use builder::{ReportBuilder, SectionBuilder};

/// Current schema version of the report envelope.
pub const SCHEMA_VERSION: u32 = 1;

/// The well-known `result_name` the scheduler and API use to identify a
/// report envelope among a task's outputs. Defined in `malbox-plugin-transport`
/// so the SDK and scheduler share the same constant without a direct dep.
pub use malbox_plugin_transport::REPORT_RESULT_NAME;

/// A plugin's structured analysis result for a single task.
///
/// Reports are the primary way plugins communicate findings to the
/// frontend. Build one with [`ReportBuilder`], then call
/// [`Report::into_plugin_result`] to turn it into a [`PluginResult`]
/// ready for [`ResultSink::push`](crate::context::ResultSink::push).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// Schema version for forward-compatible deserialization.
    pub schema_version: u32,
    /// Identity of the plugin that produced this report.
    pub plugin: PluginInfo,

    /// Overall verdict (classification, score, confidence, labels).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict: Option<Verdict>,

    /// Indicators of compromise extracted during analysis.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub indicators: Vec<Indicator>,

    /// MITRE ATT&CK techniques observed during analysis.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ttps: Vec<Ttp>,

    /// References to sibling [`PluginResult`]s (files, captures, etc.).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ArtifactRef>,

    /// Short human-readable summary of the analysis findings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,

    /// Presentation sections rendered by the frontend.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<Section>,

    /// Escape hatch for plugin-native JSON that doesn't fit the typed schema.
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

/// Identity of the plugin that produced a report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Unique plugin identifier (usually the crate name).
    pub id: String,
    /// SemVer version of the plugin binary.
    pub version: String,
    /// Optional human-friendly name shown in the frontend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

/// The plugin's overall assessment of the analyzed sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    /// Threat classification (clean, suspicious, malicious, unknown).
    pub classification: Classification,
    /// Optional numeric score (0-100). Not all plugins produce a score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<u8>,
    /// How confident the plugin is in its classification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,
    /// Free-form tags describing the threat (e.g. `"trojan"`, `"ransomware"`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
}

/// Threat classification assigned by a plugin's verdict.
///
/// When multiple plugins produce verdicts, the daemon aggregates them
/// using [`Classification::severity`] - worst wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    /// The sample appears safe.
    Clean,
    /// The sample shows potentially harmful behavior but is not conclusive.
    Suspicious,
    /// The sample is confirmed malicious.
    Malicious,
    /// The plugin could not determine a classification.
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

/// How confident a plugin is in its [`Classification`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// The classification is speculative or based on weak signals.
    Low,
    /// The classification is likely correct but not certain.
    Medium,
    /// The classification is highly reliable (strong signature match, etc.).
    High,
}

/// An indicator of compromise. `kind` is an open vocabulary so plugins can
/// emit kinds the SDK doesn't know about yet; the frontend renders any kind.
/// Common values: `sha256`, `md5`, `sha1`, `ipv4`, `ipv6`, `domain`, `url`,
/// `email`, `mutex`, `registry`, `filepath`, `yara_rule`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Indicator {
    /// IOC type (e.g. `"sha256"`, `"ipv4"`, `"domain"`, `"mutex"`).
    pub kind: String,
    /// The indicator value itself (a hash, IP, URL, etc.).
    pub value: String,
    /// Optional context describing where or how this IOC was observed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Optional ISO-8601 timestamp of when the IOC was first seen.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_seen: Option<String>,
}

/// A MITRE ATT&CK technique observation. `id` uses the canonical `T####`
/// (or `T####.###` for sub-techniques) form.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ttp {
    /// Technique ID in `T####` or `T####.###` form.
    pub id: String,
    /// Human-readable technique name (e.g. `"Process Injection"`).
    pub name: String,
    /// Optional free-text evidence supporting this observation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// A reference to a sibling `PluginResult` produced by the same plugin in
/// the same task - used by `Block::Image` / `Block::Download` to resolve
/// artifact URLs on the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRef {
    /// Name of the sibling [`PluginResult`] this references.
    pub result_name: String,
    /// Artifact type (e.g. `"pcap"`, `"screenshot"`, `"memdump"`).
    pub kind: String,
    /// Optional human-readable description of the artifact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A named section in the report's presentation layer. Each section
/// has a title and a list of renderable [`Block`]s.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    /// Machine-readable section identifier (used for anchoring/linking).
    pub id: String,
    /// Human-readable section title shown in the frontend.
    pub title: String,
    /// Ordered list of content blocks rendered inside this section.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<Block>,
}

/// A renderable block. The frontend dispatches on `type`; unknown types
/// are rendered as a JSON tree so additions are non-breaking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    /// Rendered markdown text.
    Markdown { text: String },
    /// Highlighted message box (info, success, warning, or error).
    Callout { level: CalloutLevel, text: String },
    /// Section heading. `level` maps to HTML heading levels (1-6).
    Heading { level: u8, text: String },
    /// Horizontal rule separating content.
    Divider,
    /// Key-value pairs displayed as a definition list.
    Kv { pairs: Vec<KvPair> },
    /// Tabular data with typed columns and JSON rows.
    Table {
        columns: Vec<Column>,
        rows: Vec<serde_json::Value>,
        #[serde(default)]
        sortable: bool,
        #[serde(default)]
        searchable: bool,
    },
    /// Syntax-highlighted source code.
    Code { language: String, text: String },
    /// Interactive JSON tree viewer.
    Json {
        data: serde_json::Value,
        #[serde(default)]
        collapsed: bool,
    },
    /// Hex dump of binary data. `bytes_b64` is base64-encoded.
    Hex {
        bytes_b64: String,
        #[serde(default)]
        offset: u64,
    },
    /// Inline image resolved from a sibling artifact result.
    Image {
        artifact: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        caption: Option<String>,
    },
    /// Download link resolved from a sibling artifact result.
    Download { artifact: String, label: String },
    /// Formatted list of indicators of compromise.
    Iocs { items: Vec<Indicator> },
    /// Formatted list of MITRE ATT&CK techniques.
    Ttps { items: Vec<Ttp> },
    /// Collapsible tree structure (e.g. process trees, file hierarchies).
    Tree { nodes: Vec<TreeNode> },
    /// Chronological event timeline.
    Timeline { events: Vec<TimelineEvent> },
    /// Node-and-edge graph (e.g. network connections, call graphs).
    Graph {
        nodes: Vec<GraphNode>,
        edges: Vec<GraphEdge>,
    },
}

/// Severity level for a [`Block::Callout`], controlling its color and icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalloutLevel {
    /// Neutral informational message.
    Info,
    /// Positive confirmation.
    Success,
    /// Something that deserves attention but is not an error.
    Warn,
    /// A problem that needs action.
    Error,
}

/// A single key-value pair for [`Block::Kv`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvPair {
    /// Label shown on the left.
    pub key: String,
    /// Content shown on the right.
    pub value: String,
    /// Render the value in a monospace font (useful for hashes, paths, etc.).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mono: bool,
}

/// Column definition for a [`Block::Table`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    /// JSON key used to look up this column's value in each row object.
    pub key: String,
    /// Human-readable column header.
    pub label: String,
    /// Rendering type hint: `"string"`, `"number"`, `"bool"`, `"datetime"`, etc.
    #[serde(default = "default_column_type")]
    pub r#type: String,
}

fn default_column_type() -> String {
    "string".to_string()
}

/// A node in a [`Block::Tree`] (e.g. a process or directory entry).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    /// Display text for this node.
    pub label: String,
    /// Child nodes rendered nested beneath this one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TreeNode>,
    /// Arbitrary metadata shown in a tooltip or detail pane.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub meta: serde_json::Value,
}

/// A single event on a [`Block::Timeline`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    /// Timestamp string (ISO-8601 or relative offset).
    pub ts: String,
    /// Short description of the event.
    pub label: String,
    /// Optional severity hint for color-coding (e.g. `"high"`, `"low"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    /// Arbitrary metadata shown on click or hover.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub meta: serde_json::Value,
}

/// A node in a [`Block::Graph`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    /// Unique identifier referenced by [`GraphEdge::from`] and [`GraphEdge::to`].
    pub id: String,
    /// Display label for this node.
    pub label: String,
    /// Arbitrary metadata shown on click or hover.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub meta: serde_json::Value,
}

/// A directed edge in a [`Block::Graph`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Source [`GraphNode::id`].
    pub from: String,
    /// Target [`GraphNode::id`].
    pub to: String,
    /// Optional label shown on the edge (e.g. `"connects to"`, `"spawns"`).
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
