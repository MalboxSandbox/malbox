use serde::Deserialize;

/// Declarative snapshot XML structure that deserializes from libvirt XML format.
#[derive(Debug, Deserialize)]
#[serde(rename = "domainsnapshot")]
#[expect(
    dead_code,
    reason = "mirrors the libvirt snapshot schema; not every field is read"
)]
pub(crate) struct SnapshotXml {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
    #[serde(default, rename = "creationTime")]
    pub(crate) creation_time: Option<i64>,
    #[serde(default)]
    pub(crate) state: Option<String>,
}

impl SnapshotXml {
    /// Parse snapshot XML string into structured data.
    pub(crate) fn from_xml(xml: &str) -> Result<Self, quick_xml::DeError> {
        quick_xml::de::from_str(xml)
    }
}
