//! Parsers that turn build tool output into progress information.

/// Extracts the target name from a `cargo --message-format=json` `compiler-artifact` line.
#[must_use]
pub fn parse_cargo_artifact(json_line: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(json_line).ok()?;
    if val.get("reason")?.as_str()? != "compiler-artifact" {
        return None;
    }
    Some(val.get("target")?.get("name")?.as_str()?.to_string())
}

/// Extracts the percentage from a `CMake` progress line such as `[ 42%] Building ...`.
#[must_use]
pub fn parse_cmake_progress(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('[') {
        return None;
    }
    let end = trimmed.find('%')?;
    trimmed[1..end].trim().parse::<usize>().ok()
}
