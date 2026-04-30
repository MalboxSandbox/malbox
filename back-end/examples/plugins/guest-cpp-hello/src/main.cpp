#include <cstdio>
#include <cstring>
#include <memory>
#include <string>

#include <malbox/plugin.hpp>
#include "malbox_runtime_config.hpp"

class HelloPlugin final : public malbox::GuestPlugin {
public:
    void on_start(const malbox::Context& ctx) override {
        int32_t task_id = ctx.task().id();
        std::printf("[guest-cpp-hello] on_start called for task %d\n", task_id);

        ctx.progress(0.5, "processing");

        // Build a JSON result: {"message": "hello from C++", "task_id": <id>}
        std::string json = R"({"message": "hello from C++", "task_id": )"
                         + std::to_string(task_id) + "}";

        auto data = std::span<const uint8_t>{
            reinterpret_cast<const uint8_t*>(json.data()),
            json.size()
        };

        ctx.results().push(malbox::PluginResult::json("greeting", data));
    }

    void on_stop(const malbox::Context& /*ctx*/) override {
        std::printf("[guest-cpp-hello] on_stop called\n");
    }
};

int main() {
    malbox::PluginMeta meta{};
    meta.name        = "guest-cpp-hello";
    meta.version     = "0.1.0";
    meta.description = "Example guest plugin written in C++";
    meta.authors     = "Malbox Team";
    meta.state       = malbox::PluginState::Ephemeral;
    meta.execution   = malbox::ExecutionContext::Exclusive;

    malbox::run_guest_plugin(
        std::make_unique<HelloPlugin>(),
        meta,
        malbox::generated::runtime_config);
    return 0;
}
