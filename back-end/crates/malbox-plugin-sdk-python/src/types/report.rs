use pyo3::prelude::*;

use malbox_plugin_sdk::report::{
    ArtifactRef, Block, CalloutLevel, Classification, Column, Confidence, GraphEdge, GraphNode,
    Indicator, KvPair, TimelineEvent, TreeNode, Ttp,
};

// ─── Enums ───────────────────────────────────────────────────────────────────

#[pyclass(name = "Classification", module = "malbox_plugin_sdk", eq, eq_int)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PyClassification {
    Clean = 0,
    Suspicious = 1,
    Malicious = 2,
    Unknown = 3,
}

impl From<PyClassification> for Classification {
    fn from(val: PyClassification) -> Self {
        match val {
            PyClassification::Clean => Classification::Clean,
            PyClassification::Suspicious => Classification::Suspicious,
            PyClassification::Malicious => Classification::Malicious,
            PyClassification::Unknown => Classification::Unknown,
        }
    }
}

#[pyclass(name = "Confidence", module = "malbox_plugin_sdk", eq, eq_int)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PyConfidence {
    Low = 0,
    Medium = 1,
    High = 2,
}

impl From<PyConfidence> for Confidence {
    fn from(val: PyConfidence) -> Self {
        match val {
            PyConfidence::Low => Confidence::Low,
            PyConfidence::Medium => Confidence::Medium,
            PyConfidence::High => Confidence::High,
        }
    }
}

// ─── Indicator ───────────────────────────────────────────────────────────────

#[pyclass(name = "Indicator", module = "malbox_plugin_sdk")]
#[derive(Clone)]
pub struct PyIndicator {
    pub(crate) inner: Indicator,
}

#[pymethods]
impl PyIndicator {
    #[new]
    #[pyo3(signature = (kind, value, context=None, first_seen=None))]
    fn new(
        kind: String,
        value: String,
        context: Option<String>,
        first_seen: Option<String>,
    ) -> Self {
        Self {
            inner: Indicator {
                kind,
                value,
                context,
                first_seen,
            },
        }
    }

    #[getter]
    fn kind(&self) -> &str {
        &self.inner.kind
    }

    #[getter]
    fn value(&self) -> &str {
        &self.inner.value
    }

    #[getter]
    fn context(&self) -> Option<&str> {
        self.inner.context.as_deref()
    }

    #[getter]
    fn first_seen(&self) -> Option<&str> {
        self.inner.first_seen.as_deref()
    }
}

impl From<PyIndicator> for Indicator {
    fn from(val: PyIndicator) -> Self {
        val.inner
    }
}

// ─── Ttp ─────────────────────────────────────────────────────────────────────

#[pyclass(name = "Ttp", module = "malbox_plugin_sdk")]
#[derive(Clone)]
pub struct PyTtp {
    pub(crate) inner: Ttp,
}

#[pymethods]
impl PyTtp {
    #[new]
    #[pyo3(signature = (id, name, evidence=None))]
    fn new(id: String, name: String, evidence: Option<String>) -> Self {
        Self {
            inner: Ttp { id, name, evidence },
        }
    }

    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn evidence(&self) -> Option<&str> {
        self.inner.evidence.as_deref()
    }
}

impl From<PyTtp> for Ttp {
    fn from(val: PyTtp) -> Self {
        val.inner
    }
}

// ─── ArtifactRef ─────────────────────────────────────────────────────────────

#[pyclass(name = "ArtifactRef", module = "malbox_plugin_sdk")]
#[derive(Clone)]
pub struct PyArtifactRef {
    pub(crate) inner: ArtifactRef,
}

#[pymethods]
impl PyArtifactRef {
    #[new]
    #[pyo3(signature = (result_name, kind, description=None))]
    fn new(result_name: String, kind: String, description: Option<String>) -> Self {
        Self {
            inner: ArtifactRef {
                result_name,
                kind,
                description,
            },
        }
    }

    #[getter]
    fn result_name(&self) -> &str {
        &self.inner.result_name
    }

    #[getter]
    fn kind(&self) -> &str {
        &self.inner.kind
    }

    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }
}

impl From<PyArtifactRef> for ArtifactRef {
    fn from(val: PyArtifactRef) -> Self {
        val.inner
    }
}

// ─── KvPair ──────────────────────────────────────────────────────────────────

#[pyclass(name = "KvPair", module = "malbox_plugin_sdk")]
#[derive(Clone)]
pub struct PyKvPair {
    pub(crate) inner: KvPair,
}

#[pymethods]
impl PyKvPair {
    #[new]
    #[pyo3(signature = (key, value, mono=false))]
    fn new(key: String, value: String, mono: bool) -> Self {
        Self {
            inner: KvPair { key, value, mono },
        }
    }

    #[getter]
    fn key(&self) -> &str {
        &self.inner.key
    }

    #[getter]
    fn value(&self) -> &str {
        &self.inner.value
    }

    #[getter]
    fn mono(&self) -> bool {
        self.inner.mono
    }
}

impl From<PyKvPair> for KvPair {
    fn from(val: PyKvPair) -> Self {
        val.inner
    }
}

// ─── Column ──────────────────────────────────────────────────────────────────

#[pyclass(name = "Column", module = "malbox_plugin_sdk")]
#[derive(Clone)]
pub struct PyColumn {
    pub(crate) inner: Column,
}

#[pymethods]
impl PyColumn {
    #[new]
    #[pyo3(signature = (key, label, r#type="string".to_owned()))]
    fn new(key: String, label: String, r#type: String) -> Self {
        Self {
            inner: Column { key, label, r#type },
        }
    }

    #[getter]
    fn key(&self) -> &str {
        &self.inner.key
    }

    #[getter]
    fn label(&self) -> &str {
        &self.inner.label
    }

    #[getter]
    fn r#type(&self) -> &str {
        &self.inner.r#type
    }
}

impl From<PyColumn> for Column {
    fn from(val: PyColumn) -> Self {
        val.inner
    }
}

// ─── TreeNode ────────────────────────────────────────────────────────────────

#[pyclass(name = "TreeNode", module = "malbox_plugin_sdk")]
#[derive(Clone)]
pub struct PyTreeNode {
    pub(crate) inner: TreeNode,
}

#[pymethods]
impl PyTreeNode {
    #[new]
    #[pyo3(signature = (label, children=vec![], meta=None))]
    fn new(
        py: Python<'_>,
        label: String,
        children: Vec<PyTreeNode>,
        meta: Option<PyObject>,
    ) -> PyResult<Self> {
        let meta_value = match meta {
            Some(obj) => py_obj_to_json_value(py, &obj)?,
            None => serde_json::Value::Null,
        };
        Ok(Self {
            inner: TreeNode {
                label,
                children: children.into_iter().map(|c| c.inner).collect(),
                meta: meta_value,
            },
        })
    }

    #[getter]
    fn label(&self) -> &str {
        &self.inner.label
    }

    #[getter]
    fn children(&self) -> Vec<PyTreeNode> {
        self.inner
            .children
            .iter()
            .map(|c| PyTreeNode { inner: c.clone() })
            .collect()
    }

    #[getter]
    fn meta(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_value_to_py_obj(py, &self.inner.meta)
    }
}

impl From<PyTreeNode> for TreeNode {
    fn from(val: PyTreeNode) -> Self {
        val.inner
    }
}

// ─── TimelineEvent ───────────────────────────────────────────────────────────

#[pyclass(name = "TimelineEvent", module = "malbox_plugin_sdk")]
#[derive(Clone)]
pub struct PyTimelineEvent {
    pub(crate) inner: TimelineEvent,
}

#[pymethods]
impl PyTimelineEvent {
    #[new]
    #[pyo3(signature = (ts, label, severity=None, meta=None))]
    fn new(
        py: Python<'_>,
        ts: String,
        label: String,
        severity: Option<String>,
        meta: Option<PyObject>,
    ) -> PyResult<Self> {
        let meta_value = match meta {
            Some(obj) => py_obj_to_json_value(py, &obj)?,
            None => serde_json::Value::Null,
        };
        Ok(Self {
            inner: TimelineEvent {
                ts,
                label,
                severity,
                meta: meta_value,
            },
        })
    }

    #[getter]
    fn ts(&self) -> &str {
        &self.inner.ts
    }

    #[getter]
    fn label(&self) -> &str {
        &self.inner.label
    }

    #[getter]
    fn severity(&self) -> Option<&str> {
        self.inner.severity.as_deref()
    }

    #[getter]
    fn meta(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_value_to_py_obj(py, &self.inner.meta)
    }
}

impl From<PyTimelineEvent> for TimelineEvent {
    fn from(val: PyTimelineEvent) -> Self {
        val.inner
    }
}

// ─── GraphNode ───────────────────────────────────────────────────────────────

#[pyclass(name = "GraphNode", module = "malbox_plugin_sdk")]
#[derive(Clone)]
pub struct PyGraphNode {
    pub(crate) inner: GraphNode,
}

#[pymethods]
impl PyGraphNode {
    #[new]
    #[pyo3(signature = (id, label, meta=None))]
    fn new(py: Python<'_>, id: String, label: String, meta: Option<PyObject>) -> PyResult<Self> {
        let meta_value = match meta {
            Some(obj) => py_obj_to_json_value(py, &obj)?,
            None => serde_json::Value::Null,
        };
        Ok(Self {
            inner: GraphNode {
                id,
                label,
                meta: meta_value,
            },
        })
    }

    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    #[getter]
    fn label(&self) -> &str {
        &self.inner.label
    }

    #[getter]
    fn meta(&self, py: Python<'_>) -> PyResult<PyObject> {
        json_value_to_py_obj(py, &self.inner.meta)
    }
}

impl From<PyGraphNode> for GraphNode {
    fn from(val: PyGraphNode) -> Self {
        val.inner
    }
}

// ─── GraphEdge ───────────────────────────────────────────────────────────────

#[pyclass(name = "GraphEdge", module = "malbox_plugin_sdk")]
#[derive(Clone)]
pub struct PyGraphEdge {
    pub(crate) inner: GraphEdge,
}

#[pymethods]
impl PyGraphEdge {
    #[new]
    #[pyo3(signature = (from_node, to_node, label=None))]
    fn new(from_node: String, to_node: String, label: Option<String>) -> Self {
        Self {
            inner: GraphEdge {
                from: from_node,
                to: to_node,
                label,
            },
        }
    }

    #[getter]
    #[allow(clippy::wrong_self_convention)]
    fn from_node(&self) -> &str {
        &self.inner.from
    }

    #[getter]
    fn to_node(&self) -> &str {
        &self.inner.to
    }

    #[getter]
    fn label(&self) -> Option<&str> {
        self.inner.label.as_deref()
    }
}

impl From<PyGraphEdge> for GraphEdge {
    fn from(val: PyGraphEdge) -> Self {
        val.inner
    }
}

// ─── Block ───────────────────────────────────────────────────────────────────

#[pyclass(name = "Block", module = "malbox_plugin_sdk")]
#[derive(Clone)]
pub struct PyBlock {
    pub(crate) inner: Block,
}

#[pymethods]
impl PyBlock {
    #[staticmethod]
    fn markdown(text: String) -> Self {
        Self {
            inner: Block::Markdown { text },
        }
    }

    #[staticmethod]
    fn callout(level: &str, text: String) -> PyResult<Self> {
        let lvl = match level {
            "info" => CalloutLevel::Info,
            "success" => CalloutLevel::Success,
            "warn" => CalloutLevel::Warn,
            "error" => CalloutLevel::Error,
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Invalid callout level '{}'. Must be one of: info, success, warn, error",
                    level
                )));
            }
        };
        Ok(Self {
            inner: Block::Callout { level: lvl, text },
        })
    }

    #[staticmethod]
    fn heading(level: u8, text: String) -> PyResult<Self> {
        if !(1..=6).contains(&level) {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "heading level must be 1-6",
            ));
        }
        Ok(Self {
            inner: Block::Heading { level, text },
        })
    }

    #[staticmethod]
    fn divider() -> Self {
        Self {
            inner: Block::Divider,
        }
    }

    #[staticmethod]
    fn kv(pairs: Vec<PyKvPair>) -> Self {
        Self {
            inner: Block::Kv {
                pairs: pairs.into_iter().map(|p| p.inner).collect(),
            },
        }
    }

    #[staticmethod]
    #[pyo3(signature = (columns, rows, sortable=true, searchable=false))]
    fn table(
        py: Python<'_>,
        columns: Vec<PyColumn>,
        rows: PyObject,
        sortable: bool,
        searchable: bool,
    ) -> PyResult<Self> {
        let json_mod = py.import("json")?;
        let json_str = json_mod
            .call_method1("dumps", (rows.bind(py),))?
            .extract::<String>()?;
        let rows_value: Vec<serde_json::Value> = serde_json::from_str(&json_str)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self {
            inner: Block::Table {
                columns: columns.into_iter().map(|c| c.inner).collect(),
                rows: rows_value,
                sortable,
                searchable,
            },
        })
    }

    #[staticmethod]
    fn code(language: String, text: String) -> Self {
        Self {
            inner: Block::Code { language, text },
        }
    }

    #[staticmethod]
    #[pyo3(signature = (data, collapsed=true))]
    fn json(py: Python<'_>, data: PyObject, collapsed: bool) -> PyResult<Self> {
        let json_mod = py.import("json")?;
        let json_str = json_mod
            .call_method1("dumps", (data.bind(py),))?
            .extract::<String>()?;
        let json_value: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self {
            inner: Block::Json {
                data: json_value,
                collapsed,
            },
        })
    }

    #[staticmethod]
    #[pyo3(signature = (bytes_b64, offset=0))]
    fn hex(bytes_b64: String, offset: u64) -> Self {
        Self {
            inner: Block::Hex { bytes_b64, offset },
        }
    }

    #[staticmethod]
    #[pyo3(signature = (artifact, caption=None))]
    fn image(artifact: String, caption: Option<String>) -> Self {
        Self {
            inner: Block::Image { artifact, caption },
        }
    }

    #[staticmethod]
    fn download(artifact: String, label: String) -> Self {
        Self {
            inner: Block::Download { artifact, label },
        }
    }

    #[staticmethod]
    fn iocs(items: Vec<PyIndicator>) -> Self {
        Self {
            inner: Block::Iocs {
                items: items.into_iter().map(|i| i.inner).collect(),
            },
        }
    }

    #[staticmethod]
    fn ttps(items: Vec<PyTtp>) -> Self {
        Self {
            inner: Block::Ttps {
                items: items.into_iter().map(|t| t.inner).collect(),
            },
        }
    }

    #[staticmethod]
    fn tree(nodes: Vec<PyTreeNode>) -> Self {
        Self {
            inner: Block::Tree {
                nodes: nodes.into_iter().map(|n| n.inner).collect(),
            },
        }
    }

    #[staticmethod]
    fn timeline(events: Vec<PyTimelineEvent>) -> Self {
        Self {
            inner: Block::Timeline {
                events: events.into_iter().map(|e| e.inner).collect(),
            },
        }
    }

    #[staticmethod]
    fn graph(nodes: Vec<PyGraphNode>, edges: Vec<PyGraphEdge>) -> Self {
        Self {
            inner: Block::Graph {
                nodes: nodes.into_iter().map(|n| n.inner).collect(),
                edges: edges.into_iter().map(|e| e.inner).collect(),
            },
        }
    }
}

impl From<PyBlock> for Block {
    fn from(val: PyBlock) -> Self {
        val.inner
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Convert a Python object to a serde_json::Value via Python's json module.
fn py_obj_to_json_value(py: Python<'_>, obj: &PyObject) -> PyResult<serde_json::Value> {
    let json_mod = py.import("json")?;
    let json_str = json_mod
        .call_method1("dumps", (obj.bind(py),))?
        .extract::<String>()?;
    serde_json::from_str(&json_str)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Convert a serde_json::Value back to a Python object via Python's json module.
fn json_value_to_py_obj(py: Python<'_>, value: &serde_json::Value) -> PyResult<PyObject> {
    let json_str = serde_json::to_string(value)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    let json_mod = py.import("json")?;
    let obj = json_mod.call_method1("loads", (json_str,))?;
    Ok(obj.into())
}
