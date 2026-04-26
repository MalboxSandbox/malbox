//! Guest-side YARA scanner.
//!
//! Runs inside the analysis VM, scans each sample with an embedded rule set
//! using `yara-x` (pure-Rust), and emits:
//!
//! * a structured `report` envelope via the malbox SDK report builder
//!   (picked up by `/v1/tasks/{id}/report`);
//! * a `yara_matches` JSON artifact for anyone who still wants raw data
//!   (matches the declaration in `plugin.toml`).
//!
//! The rules live in `src/rules.yar` and are compiled lazily at first scan;
//! subsequent scans in the same process reuse the compiled rule set.

extern crate malbox_plugin_sdk as malbox;

use std::sync::OnceLock;

use malbox::prelude::*;
use malbox::types::report::{
    ArtifactRef, Classification, Confidence, Indicator, ReportBuilder, Ttp,
};
use sha2::{Digest, Sha256};

const RULES_SRC: &str = include_str!("rules.yar");

/// Compile once per process (OK whether the plugin is `ephemeral` or
/// `persistent`) — the ruleset is small and yara-x compilation is fast,
/// but there's no reason to recompile on every scan.
fn rules() -> &'static yara_x::Rules {
    static RULES: OnceLock<yara_x::Rules> = OnceLock::new();
    RULES.get_or_init(|| yara_x::compile(RULES_SRC).expect("embedded YARA rules must compile"))
}

#[malbox::guest_plugin]
struct YaraScanner;

#[malbox::handlers]
impl YaraScanner {
    #[malbox::on_start]
    fn init(&self) -> Result<()> {
        // Touch the rules up front so a bad rules.yar surfaces at startup
        // rather than on the first task.
        let n = rules().iter().count();
        info!(rule_count = n, "YARA scanner ready");
        Ok(())
    }

    #[malbox::on_stop]
    fn shutdown(&self) -> Result<()> {
        info!("YARA scanner shutting down");
        Ok(())
    }

    #[malbox::on_task]
    fn scan(&self, task: Task, ctx: &Context) -> Result<()> {
        ctx.emit_progress(0.15, "reading sample")?;
        let sample = task.sample_bytes()?;
        let sample_size = sample.len();
        let sample_sha256 = hex::encode(Sha256::digest(&sample));

        ctx.emit_progress(0.45, "scanning")?;
        let mut scanner = yara_x::Scanner::new(rules());
        let scan_results = scanner
            .scan(&sample)
            .map_err(|e| SdkError::Plugin(Box::new(e)))?;

        let matches: Vec<MatchSummary> = scan_results
            .matching_rules()
            .map(MatchSummary::from_rule)
            .collect();

        info!(
            task_id = task.id(),
            matched = matches.len(),
            "YARA scan complete"
        );

        ctx.emit_progress(0.8, "building report")?;
        let report = build_report(&task, &sample_sha256, sample_size, &matches);

        // Emit the report envelope (recognised by the scheduler as a report)
        // and the raw matches JSON as a sibling artifact.
        ctx.push_result(report.into_plugin_result()?)?;
        ctx.push_result(PluginResult::json("yara_matches", &matches)?)?;

        ctx.emit_progress(1.0, "done")?;
        Ok(())
    }
}

/// Compact per-match info kept both in the `yara_matches` artifact and
/// inlined into the report's table rows.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct MatchSummary {
    rule: String,
    severity: Severity,
    description: Option<String>,
    mitre: Option<String>,
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Severity {
    Info,
    Suspicious,
    Malicious,
}

impl MatchSummary {
    fn from_rule(rule: yara_x::Rule) -> Self {
        let mut description = None;
        let mut mitre = None;
        let mut severity = Severity::Info;

        for (k, v) in rule.metadata() {
            match (k, v) {
                ("description", yara_x::MetaValue::String(s)) => description = Some(s.to_string()),
                ("mitre", yara_x::MetaValue::String(s)) => mitre = Some(s.to_string()),
                ("severity", yara_x::MetaValue::String(s)) => severity = parse_severity(s),
                _ => {}
            }
        }

        Self {
            rule: rule.identifier().to_string(),
            severity,
            description,
            mitre,
            tags: rule.tags().map(|t| t.identifier().to_string()).collect(),
        }
    }
}

fn parse_severity(s: &str) -> Severity {
    match s.to_ascii_lowercase().as_str() {
        "malicious" => Severity::Malicious,
        "suspicious" => Severity::Suspicious,
        _ => Severity::Info,
    }
}

fn classification_for(worst: Severity) -> Classification {
    match worst {
        Severity::Malicious => Classification::Malicious,
        Severity::Suspicious => Classification::Suspicious,
        Severity::Info => Classification::Clean,
    }
}

/// Rough 0–100 score: malicious 80 + 5 per additional malicious (cap 100),
/// suspicious 40 + 5 per additional suspicious (cap 70), otherwise 5 per
/// info match (cap 25). Deliberately simple — this is test data.
fn score_for(matches: &[MatchSummary]) -> u8 {
    let mal = matches
        .iter()
        .filter(|m| m.severity == Severity::Malicious)
        .count();
    let sus = matches
        .iter()
        .filter(|m| m.severity == Severity::Suspicious)
        .count();
    let info = matches
        .iter()
        .filter(|m| m.severity == Severity::Info)
        .count();
    if mal > 0 {
        (80 + 5 * (mal.saturating_sub(1))).min(100) as u8
    } else if sus > 0 {
        (40 + 5 * (sus.saturating_sub(1))).min(70) as u8
    } else {
        (5 * info).min(25) as u8
    }
}

fn build_report(
    task: &Task,
    sample_sha256: &str,
    sample_size: usize,
    matches: &[MatchSummary],
) -> malbox::types::report::Report {
    let worst = matches
        .iter()
        .map(|m| m.severity)
        .max()
        .unwrap_or(Severity::Info);
    let classification = classification_for(worst);
    let score = score_for(matches);

    let summary = if matches.is_empty() {
        "No YARA rules matched this sample.".to_string()
    } else {
        format!(
            "YARA matched {} rule{} ({}).",
            matches.len(),
            if matches.len() == 1 { "" } else { "s" },
            match classification {
                Classification::Malicious => "malicious",
                Classification::Suspicious => "suspicious",
                _ => "info",
            }
        )
    };

    let mut b = ReportBuilder::new("guest-yara-scanner", env!("CARGO_PKG_VERSION"))
        .display_name("YARA Scanner")
        .summary(summary)
        .verdict(classification, Some(score), Some(Confidence::Medium));

    // Labels: distinct rule names (short).
    let labels: Vec<String> = matches.iter().map(|m| m.rule.clone()).collect();
    if !labels.is_empty() {
        b = b.labels(labels);
    }

    // IOCs: the sample hash + one `yara_rule` IOC per match.
    b = b.indicator(Indicator::new("sha256", sample_sha256).context("sample"));
    for m in matches {
        b = b.indicator(
            Indicator::new("yara_rule", m.rule.clone())
                .context(m.description.clone().unwrap_or_else(|| "match".into())),
        );
    }

    // TTPs: any `mitre` meta on a match becomes a TTP.
    for m in matches {
        if let Some(id) = &m.mitre {
            b = b.ttp(Ttp::new(
                id.clone(),
                m.description.clone().unwrap_or_else(|| id.clone()),
            ));
        }
    }

    // Artifact ref for the raw JSON so the frontend can link to it.
    b = b.artifact(
        ArtifactRef::new("yara_matches", "yara").description("Full match details as JSON"),
    );

    // Overview section: sample stats + verdict callout.
    b = b.section("overview", "Overview", |s| {
        let size = sample_size.to_string();
        let s = s.kv(vec![
            malbox::types::report::KvPair {
                key: "SHA-256".into(),
                value: sample_sha256.to_string(),
                mono: true,
            },
            malbox::types::report::KvPair {
                key: "Size (bytes)".into(),
                value: size,
                mono: false,
            },
            malbox::types::report::KvPair {
                key: "Rules matched".into(),
                value: matches.len().to_string(),
                mono: false,
            },
        ]);
        if matches.is_empty() {
            s.callout(
                malbox::types::report::CalloutLevel::Success,
                "No rules matched — nothing suspicious observed.",
            )
        } else {
            s.callout(
                match classification {
                    Classification::Malicious => malbox::types::report::CalloutLevel::Error,
                    Classification::Suspicious => malbox::types::report::CalloutLevel::Warn,
                    _ => malbox::types::report::CalloutLevel::Info,
                },
                format!(
                    "{} matching rule{} — see the Matches section for details.",
                    matches.len(),
                    if matches.len() == 1 { "" } else { "s" }
                ),
            )
        }
    });

    // Matches section: a sortable table of rule / severity / mitre / description.
    if !matches.is_empty() {
        b = b.section("matches", "Matches", |s| {
            let cols = vec![
                malbox::types::report::Column {
                    key: "rule".into(),
                    label: "Rule".into(),
                    r#type: "string".into(),
                },
                malbox::types::report::Column {
                    key: "severity".into(),
                    label: "Severity".into(),
                    r#type: "string".into(),
                },
                malbox::types::report::Column {
                    key: "mitre".into(),
                    label: "ATT&CK".into(),
                    r#type: "string".into(),
                },
                malbox::types::report::Column {
                    key: "description".into(),
                    label: "Description".into(),
                    r#type: "string".into(),
                },
            ];
            let rows = matches
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "rule": m.rule,
                        "severity": severity_label(m.severity),
                        "mitre": m.mitre.clone().unwrap_or_default(),
                        "description": m.description.clone().unwrap_or_default(),
                    })
                })
                .collect::<Vec<_>>();
            s.table(cols, rows)
        });
    }

    // Raw escape hatch: stash the plugin-native match list too.
    b = b.raw(matches);

    let _ = task; // silence unused-warning if the above never references task
    b.build()
}

fn severity_label(s: Severity) -> &'static str {
    match s {
        Severity::Info => "info",
        Severity::Suspicious => "suspicious",
        Severity::Malicious => "malicious",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The EICAR test string is a standard AV test signature — our rules.yar
    /// matches it explicitly. Good smoke test that the yara-x API + rule
    /// compilation all work end-to-end.
    const EICAR: &[u8] = b"X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*";

    fn scan(sample: &[u8]) -> Vec<MatchSummary> {
        let mut scanner = yara_x::Scanner::new(rules());
        scanner
            .scan(sample)
            .unwrap()
            .matching_rules()
            .map(MatchSummary::from_rule)
            .collect()
    }

    #[test]
    fn rules_compile() {
        let _ = rules();
    }

    #[test]
    fn eicar_sample_matches_eicar_rule_and_is_malicious() {
        let matches = scan(EICAR);
        assert!(
            matches.iter().any(|m| m.rule == "EICAR_Test_String"),
            "expected EICAR_Test_String rule to match, got {:?}",
            matches.iter().map(|m| &m.rule).collect::<Vec<_>>()
        );
        let eicar = matches
            .iter()
            .find(|m| m.rule == "EICAR_Test_String")
            .unwrap();
        assert_eq!(eicar.severity, Severity::Malicious);
    }

    #[test]
    fn pe_header_matches_pe_file_rule() {
        // Minimal MZ + PE signature at the right offsets.
        let mut sample = vec![0u8; 0x100];
        sample[0] = b'M';
        sample[1] = b'Z';
        // e_lfanew at 0x3c points at 0x80 where we place "PE\0\0".
        sample[0x3c] = 0x80;
        sample[0x80] = b'P';
        sample[0x81] = b'E';
        // 0x82, 0x83 already zero.
        let matches = scan(&sample);
        assert!(
            matches.iter().any(|m| m.rule == "PE_File"),
            "expected PE_File rule to match, got {:?}",
            matches.iter().map(|m| &m.rule).collect::<Vec<_>>()
        );
    }

    #[test]
    fn clean_sample_produces_clean_verdict_in_report() {
        use malbox::types::report::Classification;
        let matches: Vec<MatchSummary> = vec![];
        assert_eq!(classification_for(Severity::Info), Classification::Clean);
        assert_eq!(score_for(&matches), 0);
    }

    #[test]
    fn malicious_match_builds_malicious_report() {
        let m = MatchSummary {
            rule: "Suspicious_Process_Injection_Imports".into(),
            severity: Severity::Malicious,
            description: Some("Process injection imports".into()),
            mitre: Some("T1055".into()),
            tags: vec![],
        };
        let score = score_for(&[m.clone()]);
        assert!(score >= 80, "expected malicious score >= 80, got {score}");
        assert_eq!(
            classification_for(Severity::Malicious),
            malbox::types::report::Classification::Malicious
        );
    }
}
