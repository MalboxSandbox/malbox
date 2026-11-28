//! Host-job result cache keys.

use std::collections::BTreeMap;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::params::SubmissionParams;
use crate::routing::AnalyzerSpec;

/// Submission switches that change what a host job produces and so belong in its cache key.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ScanFlags {
    /// Whether the submission requested a deep scan.
    pub(crate) deep_scan: bool,
    /// Whether the submission disabled artifact filtering.
    pub(crate) ignore_filtering: bool,
}

/// Lowercase hex SHA-256 over everything that influences a host job's output.
pub(crate) fn cache_key(
    artifact_sha256: &str,
    plugin: &str,
    plugin_version: &str,
    params: &Map<String, Value>,
    flags: ScanFlags,
) -> String {
    let ScanFlags {
        deep_scan,
        ignore_filtering,
    } = flags;
    let sorted: BTreeMap<&String, &Value> = params.iter().collect();
    let params = serde_json::to_string(&sorted).unwrap_or_default();
    let input = format!(
        "v1|{artifact_sha256}|{plugin}|{plugin_version}|{params}|deep_scan={deep_scan}|ignore_filtering={ignore_filtering}"
    );
    hex::encode(Sha256::digest(input.as_bytes()))
}

/// Whether a host job's result may be stored in and replayed from the result cache.
pub(crate) fn is_cacheable(spec: &AnalyzerSpec, params: &SubmissionParams) -> bool {
    spec.cacheable
        && !spec.uses_indicators
        && spec.monitored_keys.is_empty()
        && !params.ignore_cache
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map(v: &Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    const PLAIN: ScanFlags = ScanFlags {
        deep_scan: false,
        ignore_filtering: false,
    };
    const DEEP: ScanFlags = ScanFlags {
        deep_scan: true,
        ignore_filtering: false,
    };

    #[test]
    fn key_is_stable() {
        let p = map(&json!({"a": 1}));
        let k1 = cache_key("abc", "zip", "1.0.0", &p, PLAIN);
        let k2 = cache_key("abc", "zip", "1.0.0", &p, PLAIN);
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 64);
        assert!(
            k1.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_ne!(k1, cache_key("abc", "zip", "1.0.1", &p, PLAIN));
        assert_ne!(k1, cache_key("abc", "zip", "1.0.0", &p, DEEP));
    }

    #[test]
    fn key_ignores_param_order() {
        let mut a = Map::new();
        a.insert("x".into(), json!(1));
        a.insert("y".into(), json!([true]));
        let mut b = Map::new();
        b.insert("y".into(), json!([true]));
        b.insert("x".into(), json!(1));
        assert_eq!(
            cache_key("s", "p", "1", &a, PLAIN),
            cache_key("s", "p", "1", &b, PLAIN)
        );
    }
}
