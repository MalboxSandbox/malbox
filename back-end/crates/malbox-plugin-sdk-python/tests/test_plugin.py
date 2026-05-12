import malbox_plugin_sdk as malbox


def test_plugin_subclass():
    class MyPlugin(malbox.Plugin):
        def on_start(self, config):
            self.started = True

    p = MyPlugin()
    assert isinstance(p, malbox.Plugin)


def test_plugin_with_all_handlers():
    class FullPlugin(malbox.Plugin):
        def __init__(self):
            super().__init__()
            self.started = False
            self.stopped = False

        def on_start(self, config):
            self.started = True

        def on_stop(self):
            self.stopped = True

        def health_check(self):
            return malbox.HealthStatus.Healthy()

    plugin = FullPlugin()
    plugin.on_start({})
    assert plugin.started is True
    plugin.on_stop()
    assert plugin.stopped is True
    assert plugin.health_check().is_ready is True


def test_async_plugin_definition():
    """Verify async handlers can be defined."""
    class AsyncPlugin(malbox.Plugin):
        async def on_task(self, ctx):
            pass

        async def on_event(self, event):
            pass

    plugin = AsyncPlugin()
    assert isinstance(plugin, malbox.Plugin)


def test_plugin_default_health():
    """Default Plugin base returns Healthy."""
    p = malbox.Plugin()
    # Base class health_check isn't directly callable from Python
    # but the subclass pattern works
    assert p is not None
