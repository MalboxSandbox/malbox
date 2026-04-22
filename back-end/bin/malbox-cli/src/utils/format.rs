use console::Style;

/// Extract a displayable string from a JSON value.
pub fn display_json(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Extract a displayable string from an optional JSON value, defaulting to "-".
pub fn display_value(val: &Option<serde_json::Value>) -> String {
    match val {
        Some(v) => display_json(v),
        None => "-".into(),
    }
}

/// A simple table renderer with consistent styling.
pub struct Table {
    columns: Vec<(&'static str, usize)>,
    rows: Vec<Vec<String>>,
    indent: usize,
}

impl Table {
    pub fn new(columns: &[(&'static str, usize)]) -> Self {
        Self {
            columns: columns.to_vec(),
            rows: Vec::new(),
            indent: 0,
        }
    }

    pub fn set_indent(&mut self, indent: usize) {
        self.indent = indent;
    }

    pub fn add_row(&mut self, cells: Vec<String>) {
        self.rows.push(cells);
    }

    pub fn print(&self) {
        let bold = Style::new().bold();
        let dim = Style::new().dim();
        let prefix = " ".repeat(self.indent);

        // Header
        let header: String = self
            .columns
            .iter()
            .map(|(name, width)| format!("{:<width$}", name, width = *width))
            .collect::<Vec<_>>()
            .join(" ");
        println!("{}{}", prefix, bold.apply_to(&header));

        // Separator
        let sep_width: usize = self.columns.iter().map(|(_, w)| w).sum::<usize>()
            + self.columns.len().saturating_sub(1);
        println!("{}{}", prefix, dim.apply_to("\u{2500}".repeat(sep_width)));

        // Rows
        for row in &self.rows {
            let line: String = row
                .iter()
                .zip(self.columns.iter())
                .map(|(cell, (_, width))| format!("{:<width$}", cell, width = *width))
                .collect::<Vec<_>>()
                .join(" ");
            println!("{}{}", prefix, line);
        }
    }
}

/// Display a key-value detail view with aligned labels.
pub struct Detail {
    fields: Vec<(String, String)>,
}

impl Default for Detail {
    fn default() -> Self {
        Self::new()
    }
}

impl Detail {
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Add a field that is always shown.
    pub fn field(&mut self, label: &str, value: impl std::fmt::Display) -> &mut Self {
        self.fields.push((label.to_string(), value.to_string()));
        self
    }

    /// Add a field only if the value is `Some`.
    pub fn field_opt(&mut self, label: &str, value: Option<impl std::fmt::Display>) -> &mut Self {
        if let Some(v) = value {
            self.fields.push((label.to_string(), v.to_string()));
        }
        self
    }

    pub fn print(&self) {
        if self.fields.is_empty() {
            return;
        }

        let dim = Style::new().dim();
        let max_label = self.fields.iter().map(|(l, _)| l.len()).max().unwrap_or(0);

        for (label, value) in &self.fields {
            let padded = format!("{:<width$}", format!("{}:", label), width = max_label + 1);
            println!("  {} {}", dim.apply_to(&padded), value);
        }
    }
}

/// Print a success message with a green check mark.
pub fn success(msg: impl std::fmt::Display) {
    let green = Style::new().green();
    println!("{} {}", green.apply_to("\u{2713}"), msg);
}

/// Print an empty state message.
pub fn empty(msg: impl std::fmt::Display) {
    let dim = Style::new().dim();
    println!("{}", dim.apply_to(msg.to_string()));
}

/// Print a total count line.
pub fn total(count: usize, label: &str) {
    let dim = Style::new().dim();
    println!(
        "\n{}",
        dim.apply_to(format!("Total: {} {}(s)", count, label))
    );
}

/// Render a byte count as a short human-readable string (e.g. "1.2 KB", "512 B").
pub fn bytes(size: i64) -> String {
    if size < 0 {
        return "-".to_string();
    }
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
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
