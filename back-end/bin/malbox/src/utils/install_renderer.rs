use console::{Style, Term};
use malbox_cli_common::utils::format::Brand;
use malbox_installer::progress::ProgressObserver;
use std::collections::VecDeque;
use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;

const BAR_WIDTH: usize = 28;
const LOG_TAIL_SIZE: usize = 3;

#[derive(Clone)]
enum StepStatus {
    Pending,
    Active,
    Done {
        detail: String,
        elapsed: Option<f64>,
    },
    Failed {
        error: String,
        elapsed: Option<f64>,
    },
}

struct Step {
    label: String,
    status: StepStatus,
}

struct RendererState {
    steps: Vec<Step>,
    progress: Option<(usize, usize, String)>,
    log_tail: VecDeque<String>,
    error_lines: Vec<String>,
    rendered_lines: usize,
    step_start: Option<Instant>,
}

pub struct InstallRenderer {
    is_tty: bool,
    state: Mutex<RendererState>,
}

impl InstallRenderer {
    pub fn new(header: impl Into<String>) -> Self {
        let is_tty = Term::stderr().is_term();
        let header = header.into();
        if is_tty {
            let accent = Brand::accent();
            let mut stderr = std::io::stderr().lock();
            write!(stderr, "  {}\n\n", accent.bold().apply_to(&header)).ok();
            stderr.flush().ok();
        }
        Self {
            is_tty,
            state: Mutex::new(RendererState {
                steps: Vec::new(),
                progress: None,
                log_tail: VecDeque::with_capacity(LOG_TAIL_SIZE),
                error_lines: Vec::new(),
                rendered_lines: 0,
                step_start: None,
            }),
        }
    }

    fn render(&self, state: &RendererState) {
        if !self.is_tty {
            return;
        }

        let mut lines: Vec<String> = Vec::new();
        let accent = Brand::accent();
        let success = Brand::success();
        let error = Brand::error();
        let dim = Brand::dim();
        let primary = Brand::primary();
        let bar_empty = Style::new().color256(237);
        let border_dim = Style::new().color256(237);

        let total_steps = state.steps.len();
        for (idx, step) in state.steps.iter().enumerate() {
            let num = format!("[{}/{}]", idx + 1, total_steps);
            match &step.status {
                StepStatus::Done { detail, elapsed } => {
                    let mut line =
                        format!("  {} {}", success.apply_to(&num), dim.apply_to(&step.label),);
                    let mut detail_parts = Vec::new();
                    if !detail.is_empty() {
                        detail_parts.push(detail.clone());
                    }
                    if let Some(secs) = elapsed {
                        detail_parts.push(format_elapsed(*secs));
                    }
                    if !detail_parts.is_empty() {
                        line.push_str(&format!(
                            " {}",
                            dim.apply_to(format!("\u{b7} {}", detail_parts.join(" \u{b7} ")))
                        ));
                    }
                    lines.push(line);
                }
                StepStatus::Active => {
                    let line = if let Some((_, _, label)) = &state.progress {
                        if !label.is_empty() {
                            format!(
                                "  {} {} {}",
                                accent.apply_to(&num),
                                &step.label,
                                dim.apply_to(label),
                            )
                        } else {
                            format!("  {} {}", accent.apply_to(&num), &step.label)
                        }
                    } else {
                        format!("  {} {}", accent.apply_to(&num), &step.label)
                    };
                    lines.push(line);

                    if let Some((compiled, total, _)) = &state.progress
                        && *total > 0
                    {
                        let clamped = (*compiled).min(*total);
                        let pct = (clamped * 100) / *total;
                        let filled = (BAR_WIDTH * clamped) / *total;
                        let empty = BAR_WIDTH - filled;
                        let bar = format!(
                            "{}{}",
                            accent.apply_to("\u{2501}".repeat(filled)),
                            bar_empty.apply_to("\u{2501}".repeat(empty)),
                        );
                        lines.push(format!(
                            "    {} {}",
                            bar,
                            primary.apply_to(format!("{pct}%")),
                        ));
                    }

                    if !state.log_tail.is_empty() {
                        let tail_len = state.log_tail.len();
                        for (i, line_text) in state.log_tail.iter().enumerate() {
                            let is_newest = i == tail_len - 1;
                            let styled_text = if is_newest {
                                primary.apply_to(line_text).to_string()
                            } else {
                                dim.apply_to(line_text).to_string()
                            };
                            lines.push(format!(
                                "    {} {}",
                                border_dim.apply_to("\u{2502}"),
                                styled_text,
                            ));
                        }
                    }
                }
                StepStatus::Failed { error: _, elapsed } => {
                    let mut line =
                        format!("  {} {}", error.apply_to(&num), error.apply_to(&step.label),);
                    let mut detail_parts = Vec::new();
                    if let Some((compiled, total, _)) = &state.progress
                        && *total > 0
                    {
                        detail_parts.push(format!("{compiled}/{total} crates"));
                    }
                    if let Some(secs) = elapsed {
                        detail_parts.push(format_elapsed(*secs));
                    }
                    if !detail_parts.is_empty() {
                        line.push_str(&format!(
                            " {}",
                            dim.apply_to(format!("\u{b7} {}", detail_parts.join(" \u{b7} ")))
                        ));
                    }
                    lines.push(line);

                    for err_line in &state.error_lines {
                        lines.push(format!(
                            "    {} {}",
                            error.apply_to("\u{2502}"),
                            dim.apply_to(err_line),
                        ));
                    }
                }
                StepStatus::Pending => {
                    lines.push(format!(
                        "  {} {}",
                        dim.apply_to(&num),
                        dim.apply_to(&step.label),
                    ));
                }
            }
        }

        let mut stderr = std::io::stderr().lock();
        if state.rendered_lines > 0 {
            write!(stderr, "\x1b[{}A", state.rendered_lines).ok();
        }
        for line in &lines {
            write!(stderr, "\r\x1b[2K{}\n", line).ok();
        }
        // Clear orphaned lines from previous render when output shrinks
        if lines.len() < state.rendered_lines {
            for _ in 0..state.rendered_lines - lines.len() {
                write!(stderr, "\r\x1b[2K\n").ok();
            }
            // Move cursor back up to end of actual content
            let extra = state.rendered_lines - lines.len();
            write!(stderr, "\x1b[{}A", extra).ok();
        }
        stderr.flush().ok();
    }

    fn render_non_tty(step: &str, msg: &str) {
        eprintln!("[{step}] {msg}");
    }

    fn count_lines(&self, state: &RendererState) -> usize {
        let mut count = 0;
        for step in &state.steps {
            count += 1; // step line
            match &step.status {
                StepStatus::Active => {
                    if state.progress.as_ref().is_some_and(|(_, t, _)| *t > 0) {
                        count += 1; // progress bar
                    }
                    count += state.log_tail.len();
                }
                StepStatus::Failed { .. } => {
                    count += state.error_lines.len();
                }
                _ => {}
            }
        }
        count
    }

    pub fn add_pending_steps(&self, steps: &[&str]) {
        let mut state = self.state.lock().unwrap();
        for &step in steps {
            if !state.steps.iter().any(|s| s.label == step) {
                state.steps.push(Step {
                    label: step.to_string(),
                    status: StepStatus::Pending,
                });
            }
        }
        if self.is_tty {
            self.render(&state);
            state.rendered_lines = self.count_lines(&state);
        }
    }

    pub fn finish_line(&self, msg: &str) {
        if self.is_tty {
            let success = Brand::success();
            eprintln!();
            eprintln!("  {} {}", success.apply_to("\u{2713}"), msg);
        } else {
            eprintln!("[done] {msg}");
        }
    }
}

impl ProgressObserver for InstallRenderer {
    fn step_started(&self, step: &str) {
        let mut state = self.state.lock().unwrap();
        state.log_tail.clear();
        state.progress = None;
        state.error_lines.clear();
        state.step_start = Some(Instant::now());

        if let Some(s) = state.steps.iter_mut().find(|s| s.label == step) {
            s.status = StepStatus::Active;
        } else {
            state.steps.push(Step {
                label: step.to_string(),
                status: StepStatus::Active,
            });
        }

        if self.is_tty {
            self.render(&state);
            state.rendered_lines = self.count_lines(&state);
        } else {
            Self::render_non_tty(step, "started");
        }
    }

    fn step_completed(&self, step: &str, detail: &str) {
        let mut state = self.state.lock().unwrap();
        let elapsed = state.step_start.map(|s| s.elapsed().as_secs_f64());

        if let Some(s) = state.steps.iter_mut().find(|s| s.label == step) {
            s.status = StepStatus::Done {
                detail: detail.to_string(),
                elapsed,
            };
        }

        state.log_tail.clear();
        state.progress = None;
        state.step_start = None;

        if self.is_tty {
            self.render(&state);
            state.rendered_lines = self.count_lines(&state);
        } else {
            let msg = if detail.is_empty() {
                "done".to_string()
            } else {
                format!("done ({detail})")
            };
            Self::render_non_tty(step, &msg);
        }
    }

    fn step_failed(&self, step: &str, error: &str) {
        let mut state = self.state.lock().unwrap();
        let elapsed = state.step_start.map(|s| s.elapsed().as_secs_f64());

        state.error_lines = state.log_tail.drain(..).collect();
        if !error.is_empty() {
            for line in error.lines() {
                if !line.trim().is_empty() {
                    state.error_lines.push(line.to_string());
                }
            }
        }

        if let Some(s) = state.steps.iter_mut().find(|s| s.label == step) {
            s.status = StepStatus::Failed {
                error: error.to_string(),
                elapsed,
            };
        }

        state
            .steps
            .retain(|s| !matches!(s.status, StepStatus::Pending));

        if self.is_tty {
            self.render(&state);
            state.rendered_lines = self.count_lines(&state);
        } else {
            Self::render_non_tty(step, &format!("FAILED: {error}"));
        }
    }

    fn build_progress(&self, compiled: usize, total: usize, crate_name: &str) {
        let mut state = self.state.lock().unwrap();
        state.progress = Some((compiled, total, crate_name.to_string()));

        if self.is_tty {
            self.render(&state);
            state.rendered_lines = self.count_lines(&state);
        }
    }

    fn build_output(&self, line: &str) {
        let mut state = self.state.lock().unwrap();
        if state.log_tail.len() >= LOG_TAIL_SIZE {
            state.log_tail.pop_front();
        }
        state.log_tail.push_back(line.to_string());

        if self.is_tty {
            self.render(&state);
            state.rendered_lines = self.count_lines(&state);
        }
    }

    fn download_progress(&self, done: u64, total: Option<u64>, label: &str) {
        let mut state = self.state.lock().unwrap();
        if let Some(total) = total.filter(|&t| t > 0) {
            let clamped = done.min(total);
            state.progress = Some((clamped as usize, total as usize, label.to_string()));
        }

        if self.is_tty {
            self.render(&state);
            state.rendered_lines = self.count_lines(&state);
        }
    }
}

fn format_elapsed(secs: f64) -> String {
    if secs < 60.0 {
        format!("{:.1}s", secs)
    } else {
        let mins = (secs / 60.0).floor() as u64;
        let remaining = secs - (mins as f64 * 60.0);
        format!("{mins}m{:.0}s", remaining)
    }
}
