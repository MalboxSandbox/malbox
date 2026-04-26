#include <cstdio>
#include <cstring>
#include <memory>
#include <string>
#include <unordered_map>

#include <malbox/plugin.hpp>
#include "malbox_runtime_config.hpp"

class HelloPlugin final : public malbox::Plugin {
public:
    void on_start(const std::unordered_map<std::string, std::string>& config) override {
        std::printf("[guest-cpp-hello] on_start called with %zu config entries\n",
                    config.size());
    }

    void on_task(const malbox::Task& task, const malbox::Context& ctx) override {
        int32_t task_id = task.id();
        std::printf("[guest-cpp-hello] on_task called for task %d\n", task_id);

        ctx.emit_progress(0.5, "processing");

        // Build a JSON result: {"message": "hello from C++", "task_id": <id>}
        std::string json = R"({"message": "hello from C++", "task_id": )"
                         + std::to_string(task_id) + "}";

        auto data = std::span<const uint8_t>{
            reinterpret_cast<const uint8_t*>(json.data()),
            json.size()
        };

        ctx.push_result(malbox::PluginResult::json("greeting", data));
    }

    void on_stop() override {
        std::printf("[guest-cpp-hello] on_stop called\n");
    }
};

int main() {
    malbox::PluginMeta meta{};
    meta.name        = "guest-cpp-hello";
    meta.version     = "0.1.0";
    meta.description = "Example guest plugin written in C++";
    meta.authors     = "Malbox Team";
    meta.plugin_type = malbox::PluginType::Guest;
    meta.state       = malbox::PluginState::Ephemeral;
    meta.execution   = malbox::ExecutionContext::Exclusive;

    malbox::run_guest_plugin(
        std::make_unique<HelloPlugin>(),
        meta,
        malbox::generated::runtime_config);
    return 0;
}
