//! Postcard encoding of [`ArtifactMeta`] for `ResponseKind::ArtifactMeta` frames.
//!
//! A host plugin sends one metadata frame immediately before the result
//! frames of an artifact; the daemon attaches it to the next completed output.

use serde::{Deserialize, Serialize};

use crate::artifact::{ArtifactMeta, ArtifactRelation};
use crate::error::TransportError;

#[derive(Serialize, Deserialize)]
struct ArtifactMetaWire {
    relation: u8,
    analyze: bool,
    allow_dynamic_recursion: bool,
}

pub fn encode_artifact_meta(meta: &ArtifactMeta) -> Result<Vec<u8>, TransportError> {
    postcard::to_allocvec(&ArtifactMetaWire {
        relation: meta.relation as u8,
        analyze: meta.analyze,
        allow_dynamic_recursion: meta.allow_dynamic_recursion,
    })
    .map_err(|e| TransportError::Ipc(Box::new(e)))
}

pub fn decode_artifact_meta(payload: &[u8]) -> Result<ArtifactMeta, TransportError> {
    let wire: ArtifactMetaWire =
        postcard::from_bytes(payload).map_err(|e| TransportError::Ipc(Box::new(e)))?;
    let relation =
        ArtifactRelation::try_from(wire.relation).map_err(|e| TransportError::Ipc(Box::new(e)))?;
    // Root is reserved for the daemon; reject it like the gRPC path does.
    if relation == ArtifactRelation::Root {
        return Err(TransportError::Ipc(
            "artifact relation root is reserved".into(),
        ));
    }
    Ok(ArtifactMeta {
        relation,
        analyze: wire.analyze,
        allow_dynamic_recursion: wire.allow_dynamic_recursion,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let meta = ArtifactMeta {
            relation: ArtifactRelation::Dropped,
            analyze: true,
            allow_dynamic_recursion: false,
        };
        let bytes = encode_artifact_meta(&meta).unwrap();
        assert_eq!(decode_artifact_meta(&bytes).unwrap(), meta);
    }

    #[test]
    fn unknown_relation_rejected() {
        let bytes = postcard::to_allocvec(&ArtifactMetaWire {
            relation: 42,
            analyze: false,
            allow_dynamic_recursion: false,
        })
        .unwrap();
        let err = decode_artifact_meta(&bytes).unwrap_err();
        assert!(err.to_string().contains("unknown artifact relation 42"));
    }

    #[test]
    fn root_relation_rejected() {
        let bytes = postcard::to_allocvec(&ArtifactMetaWire {
            relation: ArtifactRelation::Root as u8,
            analyze: true,
            allow_dynamic_recursion: false,
        })
        .unwrap();
        let err = decode_artifact_meta(&bytes).unwrap_err();
        assert!(err.to_string().contains("root is reserved"));
    }
}
