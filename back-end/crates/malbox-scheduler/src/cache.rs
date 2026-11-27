//! Host-job result cache keys.

use std::collections::BTreeMap;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::params::SubmissionParams;
use crate::routing::AnalyzerSpec;

/// Lowercase hex SHA-256 over everything that influences a host job's output.
pub fn cache_key(
    artifact_sha256: &str,
    plugin: &str,
    plugin_version: &str,
    params: &Map<String, Value>,
    deep_scan: bool,
    ignore_filtering: bool,
) -> String {
    let sorted: BTreeMap<&String, &Value> = params.iter().collect();
    let params = serde_json::to_string(&sorted).unwrap_or_default();
    let input = format!(
        "v1|{artifact_sha256}|{plugin}|{plugin_version}|{params}|deep_scan={deep_scan}|ignore_filtering={ignore_filtering}"
    );
    hex::encode(Sha256::digest(input.as_bytes()))
}

pub fn is_cacheable(spec: &AnalyzerSpec, params: &SubmissionParams) -> bool {
    spec.cacheable
        && !spec.uses_indicators
        && spec.monitored_keys.is_empty()
        && !params.ignore_cache
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn key_is_stable() {
        let p = map(json!({"a": 1}));
        let k1 = cache_key("abc", "zip", "1.0.0", &p, false, false);
        let k2 = cache_key("abc", "zip", "1.0.0", &p, false, false);
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 64);
        assert!(
            k1.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_ne!(k1, cache_key("abc", "zip", "1.0.1", &p, false, false));
        assert_ne!(k1, cache_key("abc", "zip", "1.0.0", &p, true, false));
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
            cache_key("s", "p", "1", &a, false, false),
            cache_key("s", "p", "1", &b, false, false)
        );
    }
}
