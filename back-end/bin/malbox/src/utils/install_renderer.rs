use console::{Style, StyledObject, Term};
use malbox_cli_common::utils::format::{Brand, timestamp_hms};
use malbox_installer::progress::ProgressObserver;
use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;
use std::io::Write;
use std::sync::{Mutex, MutexGuard};
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
    recovery_hints: HashMap<String, Vec<String>>,
    last_nontty_pct: usize,
}

/// Shared styles used while drawing the step list.
struct Palette {
    accent: Style,
    success: Style,
    error: Style,
    dim: Style,
    primary: Style,
    border_dim: Style,
}

impl Palette {
    fn new() -> Self {
        Self {
            accent: Brand::accent(),
            success: Brand::success(),
            error: Brand::error(),
            dim: Brand::dim(),
            primary: Brand::primary(),
            border_dim: Style::new().color256(237),
        }
    }
}

pub(crate) struct InstallRenderer {
    is_tty: bool,
    state: Mutex<RendererState>,
}

impl InstallRenderer {
    pub(crate) fn new(header: impl Into<String>) -> Self {
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
                recovery_hints: HashMap::new(),
                last_nontty_pct: 0,
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, RendererState> {
        self.state
            .lock()
            .expect("renderer state mutex poisoned by a panicking observer")
    }

    /// Redraws the TTY view and records how many lines it now occupies.
    fn redraw(&self, state: &mut RendererState) {
        self.render(state);
        state.rendered_lines = count_lines(state);
    }

    fn render(&self, state: &RendererState) {
        if !self.is_tty {
            return;
        }

        let mut lines: Vec<String> = Vec::new();
        let palette = Palette::new();

        let total_steps = state.steps.len();
        for (idx, step) in state.steps.iter().enumerate() {
            let num = format!("[{}/{}]", idx + 1, total_steps);
            match &step.status {
                StepStatus::Done { detail, elapsed } => {
                    lines.push(done_line(&palette, &num, step, detail, *elapsed));
                }
                StepStatus::Active => active_lines(&palette, &num, step, state, &mut lines),
                StepStatus::Failed { error: _, elapsed } => {
                    failed_lines(&palette, &num, step, *elapsed, state, &mut lines);
                }
                StepStatus::Pending => {
                    lines.push(format!(
                        "  {} {}",
                        palette.dim.apply_to(&num),
                        palette.dim.apply_to(&step.label),
                    ));
                }
            }
        }

        let mut stderr = std::io::stderr().lock();
        if state.rendered_lines > 0 {
            write!(stderr, "\x1b[{}A", state.rendered_lines).ok();
        }
        for line in &lines {
            write!(stderr, "\r\x1b[2K{line}\n").ok();
        }
        // Clear orphaned lines from previous render when output shrinks
        if lines.len() < state.rendered_lines {
            for _ in 0..state.rendered_lines - lines.len() {
                write!(stderr, "\r\x1b[2K\n").ok();
            }
            // Move cursor back up to end of actual content
            let extra = state.rendered_lines - lines.len();
            write!(stderr, "\x1b[{extra}A").ok();
        }
        stderr.flush().ok();
    }

    fn step_pos(steps: &[Step], label: &str) -> (usize, usize) {
        let total = steps.len();
        let idx = steps.iter().position(|s| s.label == label).unwrap_or(0);
        (idx, total)
    }

    fn render_non_tty(idx: usize, total: usize, step: &str, status: &str) {
        eprintln!(
            "[{}] [{}/{}] {}... {}",
            timestamp_hms(),
            idx + 1,
            total,
            step,
            status,
        );
    }

    fn render_non_tty_detail(msg: &str) {
        eprintln!("[{}]       {}", timestamp_hms(), msg);
    }

    pub(crate) fn add_recovery_hints(&self, step: &str, hints: &[&str]) {
        let mut state = self.lock();
        let entry = state.recovery_hints.entry(step.to_string()).or_default();
        entry.extend(hints.iter().map(ToString::to_string));
    }

    pub(crate) fn add_pending_steps(&self, steps: &[&str]) {
        let mut state = self.lock();
        for &step in steps {
            if !state.steps.iter().any(|s| s.label == step) {
                state.steps.push(Step {
                    label: step.to_string(),
                    status: StepStatus::Pending,
                });
            }
        }
        if self.is_tty {
            self.redraw(&mut state);
        }
    }

    pub(crate) fn finish_line(&self, msg: &str) {
        if self.is_tty {
            let accent = Brand::accent();
            eprintln!();
            eprintln!("  {}", accent.bold().apply_to(msg));
        } else {
            Self::render_non_tty_detail(&format!("Done: {msg}"));
        }
    }
}

/// Appends ` · part · part` to `line` when there are any detail parts.
fn push_detail_suffix(line: &mut String, dim: &Style, detail_parts: &[String]) {
    if !detail_parts.is_empty() {
        let _ = write!(
            line,
            " {}",
            dim.apply_to(format!("\u{b7} {}", detail_parts.join(" \u{b7} ")))
        );
    }
}

fn done_line(
    palette: &Palette,
    num: &str,
    step: &Step,
    detail: &str,
    elapsed: Option<f64>,
) -> String {
    let mut line = format!(
        "  {} {}",
        palette.success.apply_to(num),
        palette.dim.apply_to(&step.label)
    );
    let mut detail_parts = Vec::new();
    if !detail.is_empty() {
        detail_parts.push(detail.to_owned());
    }
    if let Some(secs) = elapsed {
        detail_parts.push(format_elapsed(secs));
    }
    push_detail_suffix(&mut line, &palette.dim, &detail_parts);
    line
}

fn active_lines(
    palette: &Palette,
    num: &str,
    step: &Step,
    state: &RendererState,
    lines: &mut Vec<String>,
) {
    let line = match &state.progress {
        Some((_, _, label)) if !label.is_empty() => format!(
            "  {} {} {}",
            palette.accent.apply_to(num),
            &step.label,
            palette.dim.apply_to(label),
        ),
        _ => format!("  {} {}", palette.accent.apply_to(num), &step.label),
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
            palette.accent.apply_to("\u{2501}".repeat(filled)),
            palette.border_dim.apply_to("\u{2501}".repeat(empty)),
        );
        lines.push(format!(
            "    {} {}",
            bar,
            palette.primary.apply_to(format!("{pct}%")),
        ));
    }

    let tail_len = state.log_tail.len();
    for (i, line_text) in state.log_tail.iter().enumerate() {
        let is_newest = i == tail_len - 1;
        let styled_text: StyledObject<&String> = if is_newest {
            palette.primary.apply_to(line_text)
        } else {
            palette.dim.apply_to(line_text)
        };
        lines.push(format!(
            "    {} {}",
            palette.border_dim.apply_to("\u{2502}"),
            styled_text,
        ));
    }
}

fn failed_lines(
    palette: &Palette,
    num: &str,
    step: &Step,
    elapsed: Option<f64>,
    state: &RendererState,
    lines: &mut Vec<String>,
) {
    let mut line = format!(
        "  {} {}",
        palette.error.apply_to(num),
        palette.error.apply_to(&step.label)
    );
    let mut detail_parts = Vec::new();
    if let Some((compiled, total, _)) = &state.progress
        && *total > 0
    {
        detail_parts.push(format!("{compiled}/{total} crates"));
    }
    if let Some(secs) = elapsed {
        detail_parts.push(format_elapsed(secs));
    }
    push_detail_suffix(&mut line, &palette.dim, &detail_parts);
    lines.push(line);

    for err_line in &state.error_lines {
        lines.push(format!(
            "    {} {}",
            palette.error.apply_to("\u{2502}"),
            palette.dim.apply_to(err_line),
        ));
    }

    if let Some(hints) = state.recovery_hints.get(&step.label)
        && !hints.is_empty()
    {
        lines.push(format!("    {}", palette.border_dim.apply_to("\u{2502}")));
        for hint in hints {
            lines.push(format!(
                "    {} {}",
                palette.border_dim.apply_to("\u{2502}"),
                palette.dim.apply_to(format!("Hint: {hint}")),
            ));
        }
    }
}

/// Number of terminal lines `render` produces for `state`.
fn count_lines(state: &RendererState) -> usize {
    let mut count = 0;
    for step in &state.steps {
        count += 1;
        match &step.status {
            StepStatus::Active => {
                if state.progress.as_ref().is_some_and(|(_, t, _)| *t > 0) {
                    count += 1;
                }
                count += state.log_tail.len();
            }
            StepStatus::Failed { .. } => {
                count += state.error_lines.len();
                if let Some(hints) = state.recovery_hints.get(&step.label)
                    && !hints.is_empty()
                {
                    count += 1 + hints.len();
                }
            }
            _ => {}
        }
    }
    count
}

impl ProgressObserver for InstallRenderer {
    fn step_started(&self, step: &str) {
        let mut state = self.lock();
        state.log_tail.clear();
        state.progress = None;
        state.error_lines.clear();
        state.step_start = Some(Instant::now());
        state.last_nontty_pct = 0;

        if let Some(s) = state.steps.iter_mut().find(|s| s.label == step) {
            s.status = StepStatus::Active;
        } else {
            state.steps.push(Step {
                label: step.to_string(),
                status: StepStatus::Active,
            });
        }

        if self.is_tty {
            self.redraw(&mut state);
        } else {
            let (idx, total) = Self::step_pos(&state.steps, step);
            Self::render_non_tty(idx, total, step, "started");
        }
    }

    fn step_completed(&self, step: &str, detail: &str) {
        let mut state = self.lock();
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
            self.redraw(&mut state);
        } else {
            let (idx, total) = Self::step_pos(&state.steps, step);
            let mut parts = Vec::new();
            if !detail.is_empty() {
                parts.push(detail.to_string());
            }
            if let Some(secs) = elapsed {
                parts.push(format_elapsed(secs));
            }
            let msg = if parts.is_empty() {
                "done".to_string()
            } else {
                format!("done ({})", parts.join(", "))
            };
            Self::render_non_tty(idx, total, step, &msg);
        }
    }

    fn step_failed(&self, step: &str, error: &str) {
        let mut state = self.lock();
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

        let pos = Self::step_pos(&state.steps, step);

        state
            .steps
            .retain(|s| !matches!(s.status, StepStatus::Pending));

        if self.is_tty {
            self.redraw(&mut state);
        } else {
            let (idx, total) = pos;
            Self::render_non_tty(idx, total, step, &format!("FAILED: {error}"));
            if let Some(hints) = state.recovery_hints.get(step) {
                for hint in hints {
                    Self::render_non_tty_detail(&format!("Hint: {hint}"));
                }
            }
        }
    }

    fn build_progress(&self, compiled: usize, total: usize, crate_name: &str) {
        let mut state = self.lock();
        state.progress = Some((compiled, total, crate_name.to_string()));

        if self.is_tty {
            self.redraw(&mut state);
        } else if let Some(pct) = (compiled * 100).checked_div(total) {
            let threshold = (pct / 25) * 25;
            if threshold > state.last_nontty_pct || compiled >= total {
                state.last_nontty_pct = threshold;
                Self::render_non_tty_detail(&format!(
                    "{compiled}/{total} crates ({pct}%) - {crate_name}",
                ));
            }
        }
    }

    fn build_output(&self, line: &str) {
        let mut state = self.lock();
        if state.log_tail.len() >= LOG_TAIL_SIZE {
            state.log_tail.pop_front();
        }
        state.log_tail.push_back(line.to_string());

        if self.is_tty {
            self.redraw(&mut state);
        }
    }

    fn download_progress(&self, done: u64, total: Option<u64>, label: &str) {
        let mut state = self.lock();
        let byte_info = if let Some(total_bytes) = total.filter(|&t| t > 0) {
            let clamped = done.min(total_bytes);
            state.progress = Some((
                usize::try_from(clamped).unwrap_or(usize::MAX),
                usize::try_from(total_bytes).unwrap_or(usize::MAX),
                label.to_string(),
            ));
            Some((clamped, total_bytes))
        } else {
            None
        };

        if self.is_tty {
            self.redraw(&mut state);
        } else if let Some((clamped, total_bytes)) = byte_info {
            // clamped <= total_bytes, so the percentage is at most 100.
            let pct = usize::try_from((clamped * 100) / total_bytes).unwrap_or(100);
            let threshold = (pct / 25) * 25;
            if threshold > state.last_nontty_pct || clamped >= total_bytes {
                state.last_nontty_pct = threshold;
                Self::render_non_tty_detail(&format!(
                    "{} / {} ({pct}%)",
                    format_bytes(clamped),
                    format_bytes(total_bytes),
                ));
            }
        }
    }
}

fn format_elapsed(secs: f64) -> String {
    if secs < 60.0 {
        format!("{secs:.1}s")
    } else {
        let mins = (secs / 60.0).floor();
        let remaining = secs - mins * 60.0;
        format!("{mins}m{remaining:.0}s")
    }
}

#[expect(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    reason = "byte counts are shown rounded to one decimal; precision loss is irrelevant"
)]
fn format_bytes(n: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
