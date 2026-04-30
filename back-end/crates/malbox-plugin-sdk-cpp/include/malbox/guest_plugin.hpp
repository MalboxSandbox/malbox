#pragma once

#include <cstdint>
#include <filesystem>

#include "malbox_plugin.h"
#include <malbox/context.hpp>
#include <malbox/types.hpp>

namespace malbox {

/// Abstract base for all Malbox C++ plugins (host and guest).
///
/// Provides health_check with a default healthy-by-default implementation.
class PluginBase {
public:
    virtual ~PluginBase() = default;

    /// Return the current health status of the plugin.
    ///
    /// The runtime may call this periodically and on demand. The default
    /// implementation always reports healthy.
    virtual HealthStatus health_check() {
        return HealthStatus::ok();
    }
};

/// Abstract base for a guest plugin that runs inside a sandbox VM.
///
/// Guest plugins receive task context via on_start and can optionally
/// handle sample launching themselves via execute_sample.
class GuestPlugin : public PluginBase {
public:
    /// Called when a new task is assigned to this guest plugin.
    virtual void on_start(const Context& ctx) = 0;

    /// Called when the current task ends.
    virtual void on_stop(const Context& ctx) = 0;

    /// Launch the sample. Return LaunchResult::UseDefault to use the SDK's
    /// built-in launcher, or LaunchResult::Launched if the plugin handled it.
    virtual LaunchResult execute_sample(const std::filesystem::path& /*sample_path*/) {
        return LaunchResult::UseDefault;
    }
};

} // namespace malbox
