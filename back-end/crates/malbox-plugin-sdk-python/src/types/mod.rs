pub mod context;
pub mod event;
pub mod health;
pub mod report;
pub mod result;

use pyo3::prelude::*;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<context::PyContext>()?;
    m.add_class::<context::PyTaskInfo>()?;
    m.add_class::<context::PyResultSink>()?;
    m.add_class::<event::PyEvent>()?;
    m.add_class::<result::PyPluginResult>()?;
    m.add_class::<result::PyReportBuilder>()?;
    m.add_class::<result::PySectionBuilder>()?;
    m.add_class::<health::PyHealthStatus>()?;
    m.add_class::<report::PyClassification>()?;
    m.add_class::<report::PyConfidence>()?;
    m.add_class::<report::PyIndicator>()?;
    m.add_class::<report::PyTtp>()?;
    m.add_class::<report::PyArtifactRef>()?;
    m.add_class::<report::PyKvPair>()?;
    m.add_class::<report::PyColumn>()?;
    m.add_class::<report::PyBlock>()?;
    m.add_class::<report::PyTreeNode>()?;
    m.add_class::<report::PyTimelineEvent>()?;
    m.add_class::<report::PyGraphNode>()?;
    m.add_class::<report::PyGraphEdge>()?;
    Ok(())
}
