use serde::Deserialize;

/// Declarative snapshot XML structure that deserializes from libvirt XML format.
#[derive(Debug, Deserialize)]
#[serde(rename = "domainsnapshot")]
pub struct SnapshotXml {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub creationTime: Option<i64>,
    #[serde(default)]
    pub state: Option<String>,
}

impl SnapshotXml {
    /// Parse snapshot XML string into structured data.
    pub fn from_xml(xml: &str) -> Result<Self, quick_xml::DeError> {
        quick_xml::de::from_str(xml)
    }
}
