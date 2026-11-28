pub(crate) mod context;
pub(crate) mod event;
pub(crate) mod health;
pub(crate) mod report;
pub(crate) mod result;

use pyo3::prelude::*;

/// Register every SDK Python class, exception, and helper into the native module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<context::PyContext>()?;
    m.add_class::<context::PyTaskInfo>()?;
    m.add_class::<context::PyResultSink>()?;
    m.add(
        "ArtifactLimitError",
        m.py().get_type::<context::ArtifactLimitError>(),
    )?;
    m.add_function(wrap_pyfunction!(context::validate_artifact_args, m)?)?;
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
