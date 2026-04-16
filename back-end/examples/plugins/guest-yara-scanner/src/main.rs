extern crate malbox_plugin_sdk as malbox;

use malbox::prelude::*;

struct Rule {
    name: &'static str,
    pattern: &'static [u8],
}

const RULES: &[Rule] = &[
    Rule { name: "MZ_Header", pattern: b"MZ" },
    Rule { name: "ELF_Header", pattern: b"\x7fELF" },
    Rule { name: "PDF_Header", pattern: b"%PDF" },
    Rule { name: "PK_Archive", pattern: b"PK\x03\x04" },
];

#[malbox::guest_plugin]
#[malbox(state = "ephemeral", execution = "parallel")]
struct YaraScanner;

#[malbox::handlers]
impl YaraScanner {
    #[malbox::on_start]
    fn init(&self) -> Result<()> {
        info!(rule_count = RULES.len(), "YARA scanner ready");
        Ok(())
    }

    #[malbox::on_stop]
    fn shutdown(&self) -> Result<()> {
        info!("YARA scanner shutting down");
        Ok(())
    }

    #[malbox::on_task]
    fn scan(&self, task: Task, ctx: &Context) -> Result<()> {
        ctx.emit_progress(0.25, "loading sample")?;
        let sample = task.sample_bytes()?;

        ctx.emit_progress(0.5, "scanning")?;
        let matches: Vec<&str> = RULES
            .iter()
            .filter(|rule| {
                sample
                    .windows(rule.pattern.len())
                    .any(|window| window == rule.pattern)
            })
            .map(|rule| rule.name)
            .collect();

        if matches.is_empty() {
            info!(task_id = task.id(), "No rules matched");
        } else {
            info!(task_id = task.id(), matched = ?matches, "Rules matched");
        }

        ctx.push_result(PluginResult::json("yara_matches", &matches)?)?;
        Ok(())
    }
}
