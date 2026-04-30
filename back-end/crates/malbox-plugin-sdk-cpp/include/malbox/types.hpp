#pragma once

#include <cstdint>
#include <string>
#include <vector>
#include "malbox_plugin.h"

namespace malbox {

/// C++ mirror of MalboxPluginState.
enum class PluginState : uint8_t {
    Persistent = 0, // MALBOX_PLUGIN_STATE_PERSISTENT
    Ephemeral  = 1, // MALBOX_PLUGIN_STATE_EPHEMERAL
    Scoped     = 2, // MALBOX_PLUGIN_STATE_SCOPED
};

/// C++ mirror of MalboxExecutionContext.
enum class ExecutionContext : uint8_t {
    Exclusive    = 0, // MALBOX_EXECUTION_CONTEXT_EXCLUSIVE
    Sequential   = 1, // MALBOX_EXECUTION_CONTEXT_SEQUENTIAL
    Parallel     = 2, // MALBOX_EXECUTION_CONTEXT_PARALLEL
    Unrestricted = 3, // MALBOX_EXECUTION_CONTEXT_UNRESTRICTED
};

static_assert(sizeof(PluginState)      == sizeof(MalboxPluginState),
              "PluginState size mismatch with C enum");
static_assert(sizeof(ExecutionContext) == sizeof(MalboxExecutionContext),
              "ExecutionContext size mismatch with C enum");

// Sanity-check that the numeric values match the C enum values.
static_assert(static_cast<uint8_t>(PluginState::Persistent)    == 0u);
static_assert(static_cast<uint8_t>(PluginState::Ephemeral)     == 1u);
static_assert(static_cast<uint8_t>(PluginState::Scoped)        == 2u);
static_assert(static_cast<uint8_t>(ExecutionContext::Exclusive)    == 0u);
static_assert(static_cast<uint8_t>(ExecutionContext::Sequential)   == 1u);
static_assert(static_cast<uint8_t>(ExecutionContext::Parallel)     == 2u);
static_assert(static_cast<uint8_t>(ExecutionContext::Unrestricted) == 3u);

/// Result of execute_sample: did the plugin launch the sample itself?
enum class LaunchResult : int32_t {
    UseDefault = 0,
    Launched   = 1,
};

/// Metadata describing a plugin. Mirrors MalboxPluginMeta.
///
/// All std::string fields are owned. The struct is converted to the
/// C MalboxPluginMeta (which uses const char*) at runtime entry.
struct PluginMeta {
    std::string     name;
    std::string     version;
    std::string     description;
    std::string     authors;
    PluginState     state         = PluginState::Ephemeral;
    ExecutionContext execution    = ExecutionContext::Exclusive;
};

/// Health status returned by a plugin's health_check callback.
struct HealthStatus {
    bool        ready;
    std::string reason;

    /// Construct a healthy status.
    [[nodiscard]] static HealthStatus ok() {
        return HealthStatus{true, {}};
    }

    /// Construct an unhealthy status with the given reason.
    [[nodiscard]] static HealthStatus not_ready(const char* reason) {
        return HealthStatus{false, reason ? reason : ""};
    }
};

} // namespace malbox
