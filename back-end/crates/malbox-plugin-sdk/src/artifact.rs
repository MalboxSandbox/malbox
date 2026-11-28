//! Artifact pushes: files a plugin extracts, drops, unpacks, dumps or outputs.
//!
//! Use [`ResultSink::push_artifact`](crate::ResultSink::push_artifact) with
//! an [`ArtifactSource`] and [`ArtifactOpts`].

pub use malbox_plugin_transport::artifact::{
    ArtifactMeta, ArtifactRelation, UnknownArtifactRelation,
};

/// Where the artifact's bytes come from.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ArtifactSource {
    /// In-memory content.
    Bytes(Vec<u8>),
    /// A file on disk.
    File(std::path::PathBuf),
}

/// How the daemon should treat a pushed artifact.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct ArtifactOpts {
    /// How the artifact relates to the one being analysed.
    pub relation: ArtifactRelation,
    /// `None` uses the relation's default ([`ArtifactRelation::default_analyze`]).
    pub analyze: Option<bool>,
    /// Allow the artifact to be detonated even if recursion prevention
    /// would otherwise forbid it.
    pub allow_dynamic_recursion: bool,
}

impl ArtifactOpts {
    /// Options for `relation` with the relation's default analysis and no
    /// recursion override.
    pub fn new(relation: ArtifactRelation) -> Self {
        Self {
            relation,
            analyze: None,
            allow_dynamic_recursion: false,
        }
    }

    /// Override whether the daemon analyses the artifact.
    #[must_use]
    #[expect(
        clippy::fn_params_excessive_bools,
        reason = "builder setter for a bool field; callers forward runtime flags from FFI, Python and config"
    )]
    pub fn analyze(self, v: bool) -> Self {
        Self {
            analyze: Some(v),
            ..self
        }
    }

    /// Set whether the artifact may be detonated despite recursion prevention.
    #[must_use]
    #[expect(
        clippy::fn_params_excessive_bools,
        reason = "builder setter for a bool field; callers forward runtime flags from FFI, Python and config"
    )]
    pub fn allow_dynamic_recursion(self, v: bool) -> Self {
        Self {
            allow_dynamic_recursion: v,
            ..self
        }
    }

    /// Resolve to wire metadata, filling `analyze` from the relation's default.
    pub fn resolve(self) -> ArtifactMeta {
        ArtifactMeta {
            relation: self.relation,
            analyze: self.analyze.unwrap_or(self.relation.default_analyze()),
            allow_dynamic_recursion: self.allow_dynamic_recursion,
        }
    }
}
