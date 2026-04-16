#include <malbox/plugin.hpp>
#include <cassert>
#include <cstdio>

int main() {
    // Enum size checks
    static_assert(sizeof(malbox::PluginType) == 1);
    static_assert(sizeof(malbox::PluginState) == 1);
    static_assert(sizeof(malbox::ExecutionContext) == 1);
    static_assert(sizeof(malbox::EventTag) == sizeof(int32_t));

    // PluginMeta construction
    malbox::PluginMeta meta{
        .name = "test-plugin",
        .version = "1.0.0",
        .description = "A test plugin",
        .authors = "Test",
        .plugin_type = malbox::PluginType::Guest,
        .state = malbox::PluginState::Ephemeral,
        .execution = malbox::ExecutionContext::Parallel,
    };
    assert(meta.name != nullptr);

    // HealthStatus
    auto ok = malbox::HealthStatus::ok();
    assert(ok.ready == true);

    auto bad = malbox::HealthStatus::not_ready("broken");
    assert(bad.ready == false);
    assert(bad.reason == "broken");

    // Flat Event construction and accessors
    auto evt = malbox::Event::task_created(42);
    assert(evt.tag() == malbox::EventTag::TaskCreated);
    assert(evt.task_id() == 42);

    auto daemon_evt = malbox::Event::daemon_shutdown();
    assert(daemon_evt.tag() == malbox::EventTag::DaemonShutdown);
    assert(daemon_evt.id() == 0);

    // Event equality
    assert(malbox::Event::task_created(1) == malbox::Event::task_created(1));
    assert(malbox::Event::task_created(1) != malbox::Event::task_created(2));
    assert(malbox::Event::task_created(1) != malbox::Event::task_failed(1));

    // Event to_c roundtrip
    auto c_event = evt.to_c();
    auto back = malbox::Event(c_event);
    assert(back == evt);

    std::printf("test_types: all assertions passed\n");
    return 0;
}
