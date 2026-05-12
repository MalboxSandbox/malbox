//! Named results produced by plugin analysis.
//!
//! [`PluginResult`] represents a single output from a plugin. It can be
//! JSON data, raw bytes, or a file on disk. Each result carries a name
//! that identifies it in the task output (e.g. `"yara_matches"`).

use crate::error::Result;
use serde::Serialize;
use std::path::PathBuf;

/// A named result produced by plugin analysis.
///
/// Each variant carries a `name` that identifies the result in the task
/// report (e.g. `"yara_matches"`, `"extracted_pe"`).
#[derive(Debug)]
pub enum PluginResult {
    /// Structured data serialized as JSON.
    Json { name: String, data: Vec<u8> },
    /// Arbitrary binary data.
    Bytes { name: String, data: Vec<u8> },
    /// A file on disk - the runtime streams it back to the daemon.
    File { name: String, path: PathBuf },
}

impl PluginResult {
    /// Serialize a value as JSON and wrap it as a named result.
    pub fn json(name: impl Into<String>, value: &impl Serialize) -> Result<Self> {
        let data = serde_json::to_vec(value)?;
        Ok(PluginResult::Json {
            name: name.into(),
            data,
        })
    }

    /// Wrap raw bytes as a named result.
    pub fn bytes(name: impl Into<String>, data: Vec<u8>) -> Self {
        PluginResult::Bytes {
            name: name.into(),
            data,
        }
    }

    /// Reference a file on disk as a named result. The runtime reads and
    /// streams the file contents back to the daemon.
    pub fn file(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        PluginResult::File {
            name: name.into(),
            path: path.into(),
        }
    }

    /// The name that identifies this result in the task output.
    pub fn name(&self) -> &str {
        match self {
            PluginResult::Json { name, .. } => name,
            PluginResult::Bytes { name, .. } => name,
            PluginResult::File { name, .. } => name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestData {
        key: String,
        value: i32,
    }

    #[test]
    fn plugin_result_json_serializes_correctly() {
        let data = TestData {
            key: "hello".into(),
            value: 42,
        };
        let result = PluginResult::json("test_result", &data).unwrap();
        assert_eq!(result.name(), "test_result");
        match result {
            PluginResult::Json { data, .. } => {
                let parsed: serde_json::Value = serde_json::from_slice(&data).unwrap();
                assert_eq!(parsed["key"], "hello");
                assert_eq!(parsed["value"], 42);
            }
            _ => panic!("expected Json variant"),
        }
    }

    #[test]
    fn plugin_result_bytes_stores_raw_data() {
        let result = PluginResult::bytes("raw", vec![0xDE, 0xAD]);
        assert_eq!(result.name(), "raw");
        match result {
            PluginResult::Bytes { data, .. } => assert_eq!(data, vec![0xDE, 0xAD]),
            _ => panic!("expected Bytes variant"),
        }
    }

    #[test]
    fn plugin_result_file_stores_path() {
        let result = PluginResult::file("capture", "/tmp/out.pcap");
        assert_eq!(result.name(), "capture");
        match result {
            PluginResult::File { path, .. } => assert_eq!(path, PathBuf::from("/tmp/out.pcap")),
            _ => panic!("expected File variant"),
        }
    }

    #[test]
    fn plugin_result_constructors_accept_owned_string() {
        let owned = String::from("owned_name");
        let r = PluginResult::bytes(owned, vec![1, 2, 3]);
        assert_eq!(r.name(), "owned_name");

        let r = PluginResult::file(String::from("file_name"), "/tmp/x");
        assert_eq!(r.name(), "file_name");

        let r = PluginResult::json(String::from("json_name"), &42i32).unwrap();
        assert_eq!(r.name(), "json_name");
    }
}
