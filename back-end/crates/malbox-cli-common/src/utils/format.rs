//! Styled terminal output: brand colors, tables, detail views, and prompts.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "CLI writes user-facing output to the terminal"
)]

use chrono::{DateTime, Local, NaiveDateTime, Utc};
use console::{Style, Term};

/// Malbox color palette used for all styled CLI output.
#[derive(Debug)]
pub struct Brand;

impl Brand {
    /// Highlight color for headers, prompts, and active items.
    pub fn accent() -> Style {
        Style::new().color256(63)
    }

    /// Color for successful outcomes.
    pub fn success() -> Style {
        Style::new().color256(77)
    }

    /// Color for errors and failures.
    pub fn error() -> Style {
        Style::new().color256(196)
    }

    /// Color for pending states and warnings.
    pub fn warning() -> Style {
        Style::new().color256(220)
    }

    /// Muted color for secondary information.
    pub fn dim() -> Style {
        Style::new().color256(245)
    }

    /// Bright color for primary text.
    pub fn primary() -> Style {
        Style::new().color256(231)
    }
}

/// Picks the brand style matching a task, machine, or job status string.
pub fn status_style(status: &str) -> Style {
    match status.to_lowercase().as_str() {
        "running" | "provisioning" | "analyzing" => Brand::accent(),
        "completed" | "success" | "reported" => Brand::success(),
        "failed" | "error" => Brand::error(),
        "pending" | "queued" => Brand::warning(),
        _ => Brand::dim(),
    }
}

/// Renders a status string colored by [`status_style`].
pub fn styled_status(status: &str) -> String {
    let style = status_style(status);
    style.apply_to(status).to_string()
}

fn parse_timestamp(timestamp: &str) -> Option<DateTime<Utc>> {
    timestamp
        .parse::<DateTime<Utc>>()
        .or_else(|_| DateTime::parse_from_rfc3339(timestamp).map(|dt| dt.to_utc()))
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S%.f")
                .or_else(|_| NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%dT%H:%M:%S%.f"))
                .ok()
                .map(|dt| dt.and_utc())
        })
}

fn relative_ago(dt: DateTime<Utc>) -> String {
    let duration = Utc::now().signed_duration_since(dt);

    if duration.num_seconds() < 60 {
        return "just now".to_string();
    }
    if duration.num_minutes() < 60 {
        let m = duration.num_minutes();
        return format!("{m} min ago");
    }
    if duration.num_hours() < 24 {
        let h = duration.num_hours();
        return format!("{h}h ago");
    }
    if duration.num_days() < 30 {
        let d = duration.num_days();
        return format!("{d}d ago");
    }
    if duration.num_days() < 365 {
        let months = duration.num_days() / 30;
        return format!("{months}mo ago");
    }
    let years = duration.num_days() / 365;
    format!("{years}y ago")
}

/// Formats a timestamp as local date/time plus a dimmed relative age.
///
/// Unparseable input is returned unchanged.
pub fn format_time(timestamp: &str) -> String {
    let Some(dt) = parse_timestamp(timestamp) else {
        return timestamp.to_string();
    };
    let date = dt.format("%Y-%m-%d %H:%M:%S").to_string();
    let ago = relative_ago(dt);
    let dim = Brand::dim();
    format!("{} {}", date, dim.apply_to(format!("({ago})")))
}

/// Width of the stdout terminal in columns.
pub fn terminal_width() -> usize {
    usize::from(Term::stdout().size().1)
}

/// Shortens `s` to `max_width` visible columns, ending in `...`.
///
/// Strings already fitting, or widths below 4, are returned unchanged.
pub fn truncate(s: &str, max_width: usize) -> String {
    if max_width < 4 {
        return s.to_string();
    }
    let visible = console::measure_text_width(s);
    if visible <= max_width {
        return s.to_string();
    }
    let stripped = console::strip_ansi_codes(s);
    format!("{}...", &stripped[..max_width - 3])
}

/// Prints the malbox name and tagline banner to stdout.
pub fn brand_header() {
    let accent = Brand::accent();
    let dim = Brand::dim();
    println!(
        "  {} {}",
        accent.apply_to("malbox"),
        dim.apply_to("- malware analysis sandbox")
    );
    println!();
}

/// Prints an accent-colored section title to stdout.
pub fn section_header(title: &str) {
    let accent = Brand::accent();
    println!("\n  {}", accent.apply_to(title));
}

/// Prints a horizontal divider with a title to stderr.
///
/// Falls back to a timestamped plain line when stderr is not a terminal.
pub fn section_divider(title: &str) {
    if Term::stderr().is_term() {
        let dim = Brand::dim();
        let width = terminal_width().min(56);
        let used = 2 + 3 + title.len() + 1;
        let suffix_len = width.saturating_sub(used).max(3);
        eprintln!();
        eprintln!(
            "  {}",
            dim.apply_to(format!(
                "\u{2500}\u{2500} {} {}",
                title,
                "\u{2500}".repeat(suffix_len),
            ))
        );
        eprintln!();
    } else {
        eprintln!("[{}] --- {title} ---", timestamp_hms());
    }
}

/// Current local time as `HH:MM:SS`.
pub fn timestamp_hms() -> String {
    Local::now().format("%H:%M:%S").to_string()
}

fn pad_ansi(s: &str, width: usize) -> String {
    let visible = console::measure_text_width(s);
    if visible >= width {
        return s.to_string();
    }
    format!("{}{}", s, " ".repeat(width - visible))
}

/// Renders a JSON value for display; strings appear without quotes.
pub fn display_json(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Renders an optional JSON value via [`display_json`], or `-` when absent.
pub fn display_value(val: &Option<serde_json::Value>) -> String {
    match val {
        Some(v) => display_json(v),
        None => "-".into(),
    }
}

/// Column-aligned table printed to stdout, shrinking columns to fit the terminal.
#[derive(Debug)]
pub struct Table {
    headers: Vec<&'static str>,
    rows: Vec<Vec<String>>,
    indent: usize,
    gap: usize,
    /// Columns never shrunk to fit the terminal (e.g. IDs users copy).
    pinned: Vec<usize>,
}

impl Table {
    /// Creates an empty table with the given column headers.
    pub fn new(headers: &[&'static str]) -> Self {
        Self {
            headers: headers.to_vec(),
            rows: Vec::new(),
            indent: 0,
            gap: 2,
            pinned: Vec::new(),
        }
    }

    /// Sets the number of spaces printed before every line.
    pub fn set_indent(&mut self, indent: usize) {
        self.indent = indent;
    }

    /// Never truncate column `col` when the table is wider than the terminal.
    pub fn pin(&mut self, col: usize) {
        self.pinned.push(col);
    }

    /// Appends a row; cells beyond the header count are not measured.
    pub fn add_row(&mut self, cells: Vec<String>) {
        self.rows.push(cells);
    }

    /// Prints the header, a separator, and all rows to stdout.
    pub fn print(&self) {
        let col_count = self.headers.len();
        let mut widths: Vec<usize> = self.headers.iter().map(|h| h.len()).collect();

        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < col_count {
                    let visible = console::measure_text_width(cell);
                    widths[i] = widths[i].max(visible);
                }
            }
        }

        let term_width = terminal_width();
        let total_gap = self.gap * col_count.saturating_sub(1);
        let total_used: usize = widths.iter().sum::<usize>() + total_gap + self.indent;

        if total_used > term_width && term_width > 40 {
            let budget = term_width.saturating_sub(total_gap + self.indent);
            shrink_columns(&mut widths, budget, &self.pinned);
        }

        let header_style = Brand::accent().bold();
        let dim = Brand::dim();
        let prefix = " ".repeat(self.indent);
        let gap_str = " ".repeat(self.gap);

        let header: String = self
            .headers
            .iter()
            .zip(widths.iter())
            .map(|(name, width)| format!("{:<width$}", name, width = *width))
            .collect::<Vec<_>>()
            .join(&gap_str);
        println!("{}{}", prefix, header_style.apply_to(&header));

        let sep_width: usize = widths.iter().sum::<usize>() + total_gap;
        println!("{}{}", prefix, dim.apply_to("\u{2500}".repeat(sep_width)));

        for row in &self.rows {
            let line: String = row
                .iter()
                .enumerate()
                .map(|(i, cell)| {
                    let w = widths.get(i).copied().unwrap_or(0);
                    let visible = console::measure_text_width(cell);
                    if visible > w {
                        truncate(cell, w)
                    } else {
                        pad_ansi(cell, w)
                    }
                })
                .collect::<Vec<_>>()
                .join(&gap_str);
            println!("{prefix}{line}");
        }
    }
}

fn shrink_columns(widths: &mut [usize], budget: usize, pinned: &[usize]) {
    let current: usize = widths.iter().sum();
    if current <= budget {
        return;
    }
    let excess = current - budget;
    let mut shrinkable: Vec<(usize, usize)> = widths
        .iter()
        .enumerate()
        .filter(|&(i, w)| *w > 4 && !pinned.contains(&i))
        .map(|(i, w)| (i, *w))
        .collect();
    shrinkable.sort_by_key(|b| std::cmp::Reverse(b.1));

    let mut remaining = excess;
    for (i, w) in &shrinkable {
        if remaining == 0 {
            break;
        }
        let min = 4.max(widths[*i].min(6));
        let can_shrink = w.saturating_sub(min);
        let take = can_shrink.min(remaining);
        widths[*i] -= take;
        remaining -= take;
    }
}

/// Aligned `label: value` list printed to stdout.
#[derive(Debug)]
pub struct Detail {
    fields: Vec<(String, String)>,
}

impl Default for Detail {
    fn default() -> Self {
        Self::new()
    }
}

impl Detail {
    /// Creates an empty detail list.
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Appends a labeled value.
    pub fn field(&mut self, label: &str, value: impl std::fmt::Display) -> &mut Self {
        self.fields.push((label.to_string(), value.to_string()));
        self
    }

    /// Appends a labeled value only when `value` is `Some`.
    pub fn field_opt(&mut self, label: &str, value: Option<impl std::fmt::Display>) -> &mut Self {
        if let Some(v) = value {
            self.fields.push((label.to_string(), v.to_string()));
        }
        self
    }

    /// Appends a status value colored by [`status_style`].
    pub fn field_status(&mut self, label: &str, status: &str) -> &mut Self {
        self.fields.push((label.to_string(), styled_status(status)));
        self
    }

    /// Prints all fields with labels padded to a common width; prints nothing when empty.
    pub fn print(&self) {
        if self.fields.is_empty() {
            return;
        }

        let dim = Brand::dim();
        let max_label = self.fields.iter().map(|(l, _)| l.len()).max().unwrap_or(0);

        for (label, value) in &self.fields {
            let padded = format!("{:<width$}", format!("{}:", label), width = max_label + 1);
            println!("  {} {}", dim.apply_to(&padded), value);
        }
    }
}

/// Prints a check-marked success message to stdout.
pub fn success(msg: impl std::fmt::Display) {
    let style = Brand::success();
    println!("{} {}", style.apply_to("\u{2713}"), msg);
}

/// Prints a cross-marked error message to stderr.
pub fn error(msg: impl std::fmt::Display) {
    let style = Brand::error();
    eprintln!("{} {}", style.apply_to("\u{2717}"), msg);
}

/// Prints a dimmed "nothing to show" message to stdout.
pub fn empty(msg: impl std::fmt::Display) {
    let dim = Brand::dim();
    println!("{}", dim.apply_to(msg.to_string()));
}

/// Prints a dimmed empty-state message followed by an arrowed hint to stdout.
pub fn empty_with_hint(msg: impl std::fmt::Display, hint: impl std::fmt::Display) {
    let dim = Brand::dim();
    let accent = Brand::accent();
    println!("{}", dim.apply_to(msg.to_string()));
    println!("  {} {}", accent.apply_to("\u{2192}"), hint);
}

/// Prints a dimmed `Total: N label(s)` footer to stdout.
pub fn total(count: usize, label: &str) {
    let dim = Brand::dim();
    let suffix = if count == 1 { "" } else { "s" };
    println!(
        "\n{}",
        dim.apply_to(format!("Total: {count} {label}{suffix}"))
    );
}

/// Formats a byte count with binary units (`B`, `KB`, ... `TB`); negative sizes render as `-`.
#[expect(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    reason = "approximate value only used for one-decimal display"
)]
pub fn bytes(size: i64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    if size < 0 {
        return "-".to_string();
    }
    let mut value = size as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", size, UNITS[0])
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}

/// Dialoguer theme in the malbox palette, rendered on stderr.
pub fn malbox_theme() -> dialoguer::theme::ColorfulTheme {
    dialoguer::theme::ColorfulTheme {
        defaults_style: Style::new().for_stderr().color256(245),
        prompt_style: Style::new().for_stderr().bold(),
        prompt_prefix: console::style("  \u{203a}".to_string())
            .for_stderr()
            .color256(63),
        prompt_suffix: console::style("\u{2500}".to_string())
            .for_stderr()
            .color256(237),
        success_prefix: console::style("  \u{203a}".to_string())
            .for_stderr()
            .color256(245),
        success_suffix: console::style("\u{2500}".to_string())
            .for_stderr()
            .color256(237),
        error_prefix: console::style("  \u{2718}".to_string())
            .for_stderr()
            .color256(196),
        error_style: Style::new().for_stderr().color256(196),
        hint_style: Style::new().for_stderr().color256(245),
        values_style: Style::new().for_stderr().color256(63),
        active_item_style: Style::new().for_stderr().color256(63),
        inactive_item_style: Style::new().for_stderr().color256(245),
        active_item_prefix: console::style("    \u{276f}".to_string())
            .for_stderr()
            .color256(63),
        inactive_item_prefix: console::style("     ".to_string()).for_stderr(),
        checked_item_prefix: console::style("    \u{25cf}".to_string())
            .for_stderr()
            .color256(63),
        unchecked_item_prefix: console::style("    \u{25cb}".to_string())
            .for_stderr()
            .color256(237),
        picked_item_prefix: console::style("    \u{276f}".to_string())
            .for_stderr()
            .color256(63),
        unpicked_item_prefix: console::style("     ".to_string()).for_stderr(),
        fuzzy_cursor_style: Style::new().for_stderr().color256(231).on_color256(63),
        fuzzy_match_highlight_style: Style::new().for_stderr().bold().color256(63),
    }
}

/// Asks a yes/no question on stderr, defaulting to "no".
///
/// Returns `true` without asking when stderr is not a terminal.
pub fn confirm(prompt: &str) -> bool {
    if !std::io::IsTerminal::is_terminal(&std::io::stderr()) {
        return true;
    }
    let theme = malbox_theme();
    dialoguer::Confirm::with_theme(&theme)
        .with_prompt(prompt)
        .default(false)
        .interact()
        .unwrap_or(false)
}

/// Shows a fuzzy-searchable selection list and returns the chosen index.
///
/// Returns `None` when stderr is not a terminal, `items` is empty, or the user cancels.
pub fn fuzzy_select(prompt: &str, items: &[String]) -> Option<usize> {
    if !std::io::IsTerminal::is_terminal(&std::io::stderr()) {
        return None;
    }
    if items.is_empty() {
        return None;
    }
    let theme = malbox_theme();
    dialoguer::FuzzySelect::with_theme(&theme)
        .with_prompt(prompt)
        .items(items)
        .interact_opt()
        .ok()
        .flatten()
}
