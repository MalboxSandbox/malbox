import malbox_plugin_sdk as malbox


def test_health_status_healthy():
    status = malbox.HealthStatus.Healthy()
    assert status.is_ready is True
    assert status.reason == ""


def test_health_status_degraded():
    status = malbox.HealthStatus.Degraded("low memory")
    assert status.is_ready is False
    assert status.reason == "low memory"


def test_health_status_unhealthy():
    status = malbox.HealthStatus.Unhealthy("crashed")
    assert status.is_ready is False
    assert status.reason == "crashed"


def test_event_creation():
    event = malbox.Event.task_created(42)
    assert event.kind == "TaskCreated"
    assert event.id == 42


def test_event_no_id():
    event = malbox.Event.daemon_shutdown()
    assert event.kind == "DaemonShutdown"
    assert event.id is None


def test_event_all_variants():
    assert malbox.Event.task_starting(1).kind == "TaskStarting"
    assert malbox.Event.task_completed(2).kind == "TaskCompleted"
    assert malbox.Event.task_failed(3).kind == "TaskFailed"
    assert malbox.Event.task_canceled(4).kind == "TaskCanceled"
    assert malbox.Event.plugin_started(5).kind == "PluginStarted"
    assert malbox.Event.plugin_stopped(6).kind == "PluginStopped"
    assert malbox.Event.config_reloaded().kind == "ConfigReloaded"
    assert malbox.Event.sample_started(7).kind == "SampleStarted"
    assert malbox.Event.sample_stopped(8).kind == "SampleStopped"
    assert malbox.Event.sample_result_produced(9).kind == "SampleResultProduced"


def test_event_plugin_result_available():
    event = malbox.Event.plugin_result_available("yara-scanner", "matches")
    assert event.kind == "PluginResultAvailable"
    assert event.source == "yara-scanner"
    assert event.result_name == "matches"
    assert event.id is None


def test_event_sample_id():
    event = malbox.Event.sample_started(42)
    assert event.id == 42


def test_event_source_none_for_non_result_events():
    event = malbox.Event.task_created(1)
    assert event.source is None
    assert event.result_name is None


def test_plugin_result_json():
    result = malbox.PluginResult.json("test", {"key": "value"})
    assert result.name == "test"


def test_plugin_result_bytes():
    result = malbox.PluginResult.bytes("data", b"\x00\x01\x02")
    assert result.name == "data"


def test_plugin_result_file():
    result = malbox.PluginResult.file("capture", "/tmp/test.bin")
    assert result.name == "capture"


def test_indicator():
    ind = malbox.Indicator("ip", "1.2.3.4", context="C2 callback")
    assert ind.kind == "ip"
    assert ind.value == "1.2.3.4"
    assert ind.context == "C2 callback"
    assert ind.first_seen is None


def test_indicator_minimal():
    ind = malbox.Indicator("hash", "abc123")
    assert ind.kind == "hash"
    assert ind.value == "abc123"
    assert ind.context is None


def test_ttp():
    ttp = malbox.Ttp("T1059.001", "PowerShell", evidence="obfuscated script")
    assert ttp.id == "T1059.001"
    assert ttp.name == "PowerShell"
    assert ttp.evidence == "obfuscated script"


def test_artifact_ref():
    art = malbox.ArtifactRef("strings", "json", description="extracted strings")
    assert art.result_name == "strings"
    assert art.kind == "json"
    assert art.description == "extracted strings"


def test_kv_pair():
    pair = malbox.KvPair("size", "1024", mono=True)
    assert pair.key == "size"
    assert pair.value == "1024"
    assert pair.mono is True


def test_kv_pair_default():
    pair = malbox.KvPair("name", "test")
    assert pair.mono is False


def test_column():
    col = malbox.Column("name", "Name", "string")
    assert col.key == "name"
    assert col.label == "Name"


def test_block_markdown():
    block = malbox.Block.markdown("hello world")
    assert block is not None


def test_block_callout():
    block = malbox.Block.callout("warn", "careful!")
    assert block is not None


def test_block_callout_invalid():
    import pytest
    with pytest.raises(ValueError):
        malbox.Block.callout("invalid", "text")


def test_block_table():
    cols = [malbox.Column("k", "Key"), malbox.Column("v", "Value")]
    rows = [{"k": "a", "v": "1"}, {"k": "b", "v": "2"}]
    block = malbox.Block.table(cols, rows, sortable=True)
    assert block is not None


def test_block_code():
    block = malbox.Block.code("python", "print('hello')")
    assert block is not None


def test_block_divider():
    block = malbox.Block.divider()
    assert block is not None


def test_classification_enum():
    assert malbox.Classification.Clean != malbox.Classification.Malicious
    assert malbox.Classification.Suspicious != malbox.Classification.Unknown


def test_confidence_enum():
    assert malbox.Confidence.Low != malbox.Confidence.High


def test_report_builder():
    report = malbox.ReportBuilder("test-plugin", "0.1.0")
    report.display_name("Test Plugin")
    report.summary("A test summary")
    report.verdict(malbox.Classification.Malicious, score=85, confidence=malbox.Confidence.High)
    report.indicator(malbox.Indicator("hash", "abc123"))
    report.ttp(malbox.Ttp("T1059", "Scripting"))
    report.artifact(malbox.ArtifactRef("strings", "json"))
    report.section("overview", "Overview", [malbox.Block.markdown("test content")])
    result = report.build()
    assert result.name is not None


def test_report_builder_minimal():
    report = malbox.ReportBuilder("minimal", "1.0.0")
    result = report.build()
    assert result is not None


def test_graph_types():
    node = malbox.GraphNode("n1", "Node 1")
    assert node.id == "n1"
    assert node.label == "Node 1"

    edge = malbox.GraphEdge("n1", "n2", label="connects")
    assert edge.from_node == "n1"
    assert edge.to_node == "n2"
    assert edge.label == "connects"


def test_timeline_event():
    ev = malbox.TimelineEvent("2024-01-01T00:00:00Z", "Started")
    assert ev.ts == "2024-01-01T00:00:00Z"
    assert ev.label == "Started"
    assert ev.severity is None


def test_tree_node():
    child = malbox.TreeNode("child")
    parent = malbox.TreeNode("parent", children=[child])
    assert parent.label == "parent"
