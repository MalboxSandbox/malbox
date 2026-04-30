#pragma once

#include <cstdint>
#include <filesystem>

#include "malbox_plugin.h"
#include <malbox/context.hpp>
#include <malbox/task.hpp>
#include <malbox/types.hpp>

namespace malbox {

class GuestPlugin {
public:
    virtual ~GuestPlugin() = default;
    virtual void on_start(const Task& task, const Context& ctx) = 0;
    virtual void on_stop(const Context& ctx) = 0;

    /// Launch the sample. Return 0 to use the SDK's built-in launcher (default),
    /// 1 if the plugin handled it successfully, or -1 on failure.
    virtual int32_t execute_sample(const std::filesystem::path& /*sample_path*/) {
        return 0;
    }

    virtual HealthStatus health_check() {
        return HealthStatus::ok();
    }
};

} // namespace malbox
