#pragma once

#include <cstdint>
#include "../../generated/malbox_plugin.h"

namespace malbox {

/// Discriminant for flat events, mirroring MalboxEventTag.
enum class EventTag : int32_t {
    // Task events
    TaskCreated          = MALBOX_EVENT_TAG_TASK_CREATED,
    TaskStarting         = MALBOX_EVENT_TAG_TASK_STARTING,
    TaskCompleted        = MALBOX_EVENT_TAG_TASK_COMPLETED,
    TaskFailed           = MALBOX_EVENT_TAG_TASK_FAILED,
    // Plugin events
    PluginStarted        = MALBOX_EVENT_TAG_PLUGIN_STARTED,
    PluginStopped        = MALBOX_EVENT_TAG_PLUGIN_STOPPED,
    PluginResultProduced = MALBOX_EVENT_TAG_PLUGIN_RESULT_PRODUCED,
    // Sample events
    SampleStarted        = MALBOX_EVENT_TAG_SAMPLE_STARTED,
    SampleStopped        = MALBOX_EVENT_TAG_SAMPLE_STOPPED,
    SampleResultProduced = MALBOX_EVENT_TAG_SAMPLE_RESULT_PRODUCED,
    // Daemon events
    DaemonShutdown       = MALBOX_EVENT_TAG_DAEMON_SHUTDOWN,
    ConfigReloaded       = MALBOX_EVENT_TAG_CONFIG_RELOADED,
};

/// A flat system-wide event.
///
/// Each event carries a tag and at most one integer identifier:
/// - Task events: id is the task_id.
/// - Plugin events: id is the plugin_id.
/// - Sample events: id is the sample_id.
/// - Daemon events: id is unused (0).
///
/// Mirrors the Rust `Event` enum and the C `MalboxEvent` struct.
class Event {
public:
    /// Construct from tag and id.
    constexpr Event(EventTag tag, int32_t id = 0) noexcept
        : tag_(tag), id_(id) {}

    /// Construct from C MalboxEvent.
    constexpr Event(MalboxEvent c) noexcept
        : tag_(static_cast<EventTag>(c.tag)), id_(c.id) {}

    // -- Accessors --

    [[nodiscard]] constexpr EventTag tag() const noexcept { return tag_; }
    [[nodiscard]] constexpr int32_t  id()  const noexcept { return id_; }

    /// Get task_id (valid for Task* events).
    [[nodiscard]] constexpr int32_t task_id()   const noexcept { return id_; }
    /// Get plugin_id (valid for Plugin* events).
    [[nodiscard]] constexpr int32_t plugin_id() const noexcept { return id_; }
    /// Get sample_id (valid for Sample* events).
    [[nodiscard]] constexpr int32_t sample_id() const noexcept { return id_; }

    // -- Factory methods --

    [[nodiscard]] static constexpr Event task_created(int32_t task_id) noexcept {
        return {EventTag::TaskCreated, task_id};
    }
    [[nodiscard]] static constexpr Event task_starting(int32_t task_id) noexcept {
        return {EventTag::TaskStarting, task_id};
    }
    [[nodiscard]] static constexpr Event task_completed(int32_t task_id) noexcept {
        return {EventTag::TaskCompleted, task_id};
    }
    [[nodiscard]] static constexpr Event task_failed(int32_t task_id) noexcept {
        return {EventTag::TaskFailed, task_id};
    }
    [[nodiscard]] static constexpr Event plugin_started(int32_t plugin_id) noexcept {
        return {EventTag::PluginStarted, plugin_id};
    }
    [[nodiscard]] static constexpr Event plugin_stopped(int32_t plugin_id) noexcept {
        return {EventTag::PluginStopped, plugin_id};
    }
    [[nodiscard]] static constexpr Event plugin_result_produced(int32_t plugin_id) noexcept {
        return {EventTag::PluginResultProduced, plugin_id};
    }
    [[nodiscard]] static constexpr Event sample_started(int32_t sample_id) noexcept {
        return {EventTag::SampleStarted, sample_id};
    }
    [[nodiscard]] static constexpr Event sample_stopped(int32_t sample_id) noexcept {
        return {EventTag::SampleStopped, sample_id};
    }
    [[nodiscard]] static constexpr Event sample_result_produced(int32_t sample_id) noexcept {
        return {EventTag::SampleResultProduced, sample_id};
    }
    [[nodiscard]] static constexpr Event daemon_shutdown() noexcept {
        return {EventTag::DaemonShutdown, 0};
    }
    [[nodiscard]] static constexpr Event config_reloaded() noexcept {
        return {EventTag::ConfigReloaded, 0};
    }

    // -- Conversion to C type --

    [[nodiscard]] constexpr MalboxEvent to_c() const noexcept {
        return MalboxEvent{static_cast<MalboxEventTag>(tag_), id_};
    }

    // -- Comparison --

    [[nodiscard]] constexpr bool operator==(const Event& other) const noexcept {
        return tag_ == other.tag_ && id_ == other.id_;
    }
    [[nodiscard]] constexpr bool operator!=(const Event& other) const noexcept {
        return !(*this == other);
    }

private:
    EventTag tag_;
    int32_t  id_;
};

static_assert(sizeof(EventTag) == sizeof(MalboxEventTag), "EventTag size mismatch");

} // namespace malbox
