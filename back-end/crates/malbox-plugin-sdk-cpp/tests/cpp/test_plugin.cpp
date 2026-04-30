#include <malbox/plugin.hpp>
#include <cassert>
#include <cstdio>
#include <atomic>

static std::atomic<int> start_count{0};
static std::atomic<int> task_count{0};
static std::atomic<int> stop_count{0};
static std::atomic<int> health_count{0};

class TestPlugin : public malbox::HostPlugin {
public:
    void on_start(const std::unordered_map<std::string, std::string>& config) override {
        start_count++;
        assert(config.size() == 1);
        assert(config.at("key") == "value");
    }

    void on_task(
        const malbox::Task& task,
        const malbox::Context& ctx
    ) override {
        task_count++;
        assert(task.id() == 42);
        ctx.emit_progress(0.5, "halfway");
        std::string json = R"({"status": "ok"})";
        auto data = std::span<const uint8_t>(
            reinterpret_cast<const uint8_t*>(json.data()), json.size());
        ctx.push_result(malbox::HostPluginResult::json("result", data));
    }

    void on_stop() override {
        stop_count++;
    }

    malbox::HealthStatus health_check() override {
        health_count++;
        return malbox::HealthStatus::ok();
    }
};

int main() {
    auto plugin = std::make_unique<TestPlugin>();
    // Use test_run_plugin to exercise the full lifecycle.
    // Sequence: on_start -> on_task -> health_check -> on_event -> on_stop
    malbox::test_run_plugin(std::move(plugin), 42, "/tmp/sample.bin",
        {{"key", "value"}});

    assert(start_count == 1);
    assert(task_count == 1);
    assert(stop_count == 1);
    assert(health_count == 1);

    std::printf("test_plugin: all assertions passed\n");
    return 0;
}
