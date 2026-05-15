use malbox_plugin_sdk::report::{
    ArtifactRef, Block, Classification, Confidence, Indicator, Report, ReportBuilder, Ttp,
};
use malbox_plugin_sdk::result::PluginResult;
use pyo3::prelude::*;
use std::path::PathBuf;

use super::report::{PyArtifactRef, PyBlock, PyClassification, PyConfidence, PyIndicator, PyTtp};

// ─── PluginResult ───────────────────────────────────────────────────────────

#[pyclass(name = "PluginResult", module = "malbox_plugin_sdk")]
pub struct PyPluginResult {
    pub(crate) inner: PluginResult,
}

impl Clone for PyPluginResult {
    fn clone(&self) -> Self {
        let inner = match &self.inner {
            PluginResult::Json { name, data } => PluginResult::Json {
                name: name.clone(),
                data: data.clone(),
            },
            PluginResult::Bytes { name, data } => PluginResult::Bytes {
                name: name.clone(),
                data: data.clone(),
            },
            PluginResult::File { name, path } => PluginResult::File {
                name: name.clone(),
                path: path.clone(),
            },
        };
        Self { inner }
    }
}

#[pymethods]
impl PyPluginResult {
    #[staticmethod]
    fn json(py: Python<'_>, name: String, data: PyObject) -> PyResult<Self> {
        let json_str = py
            .import("json")?
            .call_method1("dumps", (data.bind(py),))?
            .extract::<String>()?;
        let json_bytes: Vec<u8> = json_str.into_bytes();
        Ok(Self {
            inner: PluginResult::Json {
                name,
                data: json_bytes,
            },
        })
    }

    #[staticmethod]
    fn bytes(name: String, data: Vec<u8>) -> Self {
        Self {
            inner: PluginResult::bytes(name, data),
        }
    }

    #[staticmethod]
    fn file(name: String, path: PathBuf) -> Self {
        Self {
            inner: PluginResult::file(name, path),
        }
    }

    #[getter]
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn __repr__(&self) -> String {
        format!("PluginResult(name='{}')", self.inner.name())
    }
}

impl From<PyPluginResult> for PluginResult {
    fn from(py: PyPluginResult) -> Self {
        py.inner
    }
}

// ─── ReportBuilder ──────────────────────────────────────────────────────────

#[pyclass(name = "ReportBuilder", module = "malbox_plugin_sdk")]
pub struct PyReportBuilder {
    plugin_id: String,
    plugin_version: String,
    display_name: Option<String>,
    summary: Option<String>,
    verdict: Option<(PyClassification, Option<u8>, Option<PyConfidence>)>,
    labels: Vec<String>,
    indicators: Vec<Indicator>,
    ttps: Vec<Ttp>,
    artifacts: Vec<ArtifactRef>,
    sections: Vec<(String, String, Vec<Block>)>,
    raw: Option<serde_json::Value>,
}

#[pymethods]
impl PyReportBuilder {
    #[new]
    fn new(plugin_id: String, plugin_version: String) -> Self {
        Self {
            plugin_id,
            plugin_version,
            display_name: None,
            summary: None,
            verdict: None,
            labels: Vec::new(),
            indicators: Vec::new(),
            ttps: Vec::new(),
            artifacts: Vec::new(),
            sections: Vec::new(),
            raw: None,
        }
    }

    fn display_name(mut slf: PyRefMut<'_, Self>, name: String) -> PyRefMut<'_, Self> {
        slf.display_name = Some(name);
        slf
    }

    fn summary(mut slf: PyRefMut<'_, Self>, summary: String) -> PyRefMut<'_, Self> {
        slf.summary = Some(summary);
        slf
    }

    #[pyo3(signature = (classification, score=None, confidence=None))]
    fn verdict(
        mut slf: PyRefMut<'_, Self>,
        classification: PyClassification,
        score: Option<u8>,
        confidence: Option<PyConfidence>,
    ) -> PyRefMut<'_, Self> {
        slf.verdict = Some((classification, score, confidence));
        slf
    }

    fn labels(mut slf: PyRefMut<'_, Self>, labels: Vec<String>) -> PyRefMut<'_, Self> {
        slf.labels = labels;
        slf
    }

    fn indicator(mut slf: PyRefMut<'_, Self>, indicator: PyIndicator) -> PyRefMut<'_, Self> {
        slf.indicators.push(indicator.inner);
        slf
    }

    fn ttp(mut slf: PyRefMut<'_, Self>, ttp: PyTtp) -> PyRefMut<'_, Self> {
        slf.ttps.push(ttp.inner);
        slf
    }

    fn artifact(mut slf: PyRefMut<'_, Self>, artifact: PyArtifactRef) -> PyRefMut<'_, Self> {
        slf.artifacts.push(artifact.inner);
        slf
    }

    fn section(
        mut slf: PyRefMut<'_, Self>,
        id: String,
        title: String,
        blocks: Vec<PyBlock>,
    ) -> PyRefMut<'_, Self> {
        let rust_blocks: Vec<Block> = blocks.into_iter().map(|b| b.inner).collect();
        slf.sections.push((id, title, rust_blocks));
        slf
    }

    fn raw(mut slf: PyRefMut<'_, Self>, value: PyObject) -> PyResult<PyRefMut<'_, Self>> {
        let json_val: serde_json::Value = Python::with_gil(|py| {
            let json_str = py
                .import("json")?
                .call_method1("dumps", (value.bind(py),))?
                .extract::<String>()?;
            serde_json::from_str(&json_str)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
        })?;
        slf.raw = Some(json_val);
        Ok(slf)
    }

    fn build(&self) -> PyResult<PyPluginResult> {
        let report = self.build_report()?;
        let result = report
            .into_plugin_result()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(PyPluginResult { inner: result })
    }
}

impl PyReportBuilder {
    fn build_report(&self) -> PyResult<Report> {
        let mut builder = ReportBuilder::new(&self.plugin_id, &self.plugin_version);

        if let Some(ref name) = self.display_name {
            builder = builder.display_name(name);
        }
        if let Some(ref s) = self.summary {
            builder = builder.summary(s);
        }
        if let Some((classification, score, confidence)) = self.verdict {
            builder = builder.verdict(
                Classification::from(classification),
                score,
                confidence.map(Confidence::from),
            );
        }
        if !self.labels.is_empty() {
            builder = builder.labels(self.labels.clone());
        }
        for ind in &self.indicators {
            builder = builder.indicator(ind.clone());
        }
        for ttp in &self.ttps {
            builder = builder.ttp(ttp.clone());
        }
        for art in &self.artifacts {
            builder = builder.artifact(art.clone());
        }
        for (id, title, blocks) in &self.sections {
            let blocks_clone: Vec<Block> = blocks.clone();
            builder = builder.section(id, title, |s| {
                let mut section = s;
                for block in blocks_clone {
                    section = section.block(block);
                }
                section
            });
        }
        if let Some(ref raw) = self.raw {
            builder = builder.raw(raw);
        }

        Ok(builder.build())
    }
}

// ─── SectionBuilder (Python convenience, not wrapping Rust SectionBuilder) ──

#[pyclass(name = "SectionBuilder", module = "malbox_plugin_sdk")]
pub struct PySectionBuilder {
    pub(crate) blocks: Vec<PyBlock>,
}

#[pymethods]
impl PySectionBuilder {
    #[new]
    fn new() -> Self {
        Self { blocks: Vec::new() }
    }

    fn block(mut slf: PyRefMut<'_, Self>, block: PyBlock) -> PyRefMut<'_, Self> {
        slf.blocks.push(block);
        slf
    }

    fn get_blocks(&self) -> Vec<PyBlock> {
        self.blocks.clone()
    }
}
