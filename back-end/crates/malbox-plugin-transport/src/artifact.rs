//! Artifact metadata shared by the plugin SDK and the daemon.
//!
//! An artifact is any file a plugin extracts, drops, unpacks, dumps or
//! outputs. The metadata tells the daemon how it relates to its parent and
//! whether the plugin wants it analysed.

use std::str::FromStr;

/// How an artifact relates to the artifact that produced it.
///
/// The discriminant is the wire value; [`as_str`](Self::as_str) is the
/// database label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum ArtifactRelation {
    /// The submitted sample. Reserved for the daemon; plugins never send it.
    Root = 0,
    /// Pulled out of an archive or container.
    Extracted = 1,
    /// Written to disk by the sample while it ran.
    Dropped = 2,
    /// Recovered by unpacking or deobfuscating its parent.
    Unpacked = 3,
    /// A process memory dump. Stored, not analysed by default.
    Memdump = 4,
    /// Fetched over the network by the sample.
    Downloaded = 5,
    /// A network capture.
    Pcap = 6,
    /// A plugin output such as a report or log. Stored, not analysed by default.
    Output = 7,
}

impl ArtifactRelation {
    /// Database label of this relation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Extracted => "extracted",
            Self::Dropped => "dropped",
            Self::Unpacked => "unpacked",
            Self::Memdump => "memdump",
            Self::Downloaded => "downloaded",
            Self::Pcap => "pcap",
            Self::Output => "output",
        }
    }

    /// Whether artifacts of this relation are analysed unless the plugin says otherwise.
    pub fn default_analyze(self) -> bool {
        !matches!(self, Self::Root | Self::Memdump | Self::Output)
    }
}

/// Error for a wire value or label that names no [`ArtifactRelation`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown artifact relation {0}")]
pub struct UnknownArtifactRelation(String);

impl TryFrom<u8> for ArtifactRelation {
    type Error = UnknownArtifactRelation;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        Ok(match v {
            0 => Self::Root,
            1 => Self::Extracted,
            2 => Self::Dropped,
            3 => Self::Unpacked,
            4 => Self::Memdump,
            5 => Self::Downloaded,
            6 => Self::Pcap,
            7 => Self::Output,
            _ => return Err(UnknownArtifactRelation(v.to_string())),
        })
    }
}

impl FromStr for ArtifactRelation {
    type Err = UnknownArtifactRelation;

    /// Parse a database label as produced by [`ArtifactRelation::as_str`].
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "root" => Self::Root,
            "extracted" => Self::Extracted,
            "dropped" => Self::Dropped,
            "unpacked" => Self::Unpacked,
            "memdump" => Self::Memdump,
            "downloaded" => Self::Downloaded,
            "pcap" => Self::Pcap,
            "output" => Self::Output,
            _ => return Err(UnknownArtifactRelation(s.to_owned())),
        })
    }
}

/// Metadata a plugin attaches to an artifact output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactMeta {
    /// How the artifact relates to the one the plugin was analysing.
    pub relation: ArtifactRelation,
    /// Whether the daemon should schedule analysis of the artifact.
    pub analyze: bool,
    /// Allow detonation even if recursion prevention would forbid it.
    pub allow_dynamic_recursion: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_roundtrip() {
        for v in 0..=7u8 {
            let r = ArtifactRelation::try_from(v).unwrap();
            assert_eq!(r as u8, v);
            assert_eq!(r.as_str().parse::<ArtifactRelation>().unwrap(), r);
        }
        assert_eq!(
            ArtifactRelation::try_from(8).unwrap_err().to_string(),
            "unknown artifact relation 8"
        );
        assert!("Extracted".parse::<ArtifactRelation>().is_err());
    }

    #[test]
    fn default_analyze_per_relation() {
        assert!(ArtifactRelation::Extracted.default_analyze());
        assert!(ArtifactRelation::Pcap.default_analyze());
        assert!(!ArtifactRelation::Memdump.default_analyze());
        assert!(!ArtifactRelation::Output.default_analyze());
    }
}
