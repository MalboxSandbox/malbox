#pragma once

#include <string>
#include <unordered_map>
#include <malbox/events.hpp>
#include <malbox/result.hpp>
#include <malbox/task.hpp>
#include <malbox/context.hpp>
#include <malbox/types.hpp>

namespace malbox {

/// Abstract base class that every Malbox C++ plugin must derive from.
///
/// All methods have default (no-op) implementations. A useful plugin should
/// at minimum override on_task. Results are pushed via Context::push_result()
/// during on_task rather than returned.
///
/// ## Thread Safety
/// The runtime guarantees that only one callback is active at a time (unless
/// ExecutionContext::Parallel is chosen). Plugin authors should still protect
/// any shared mutable state with appropriate synchronisation primitives when
/// using the parallel context.
class HostPlugin {
public:
    virtual ~HostPlugin() = default;

    // -----------------------------------------------------------------------
    // Task processing
    // -----------------------------------------------------------------------

    /// Process a single analysis task.
    ///
    /// Push results via ctx.push_result() during execution. The runtime
    /// sends a final marker automatically once this method returns.
    virtual void on_task(const Task& task, const Context& ctx) {
        (void)task;
        (void)ctx;
    }

    // -----------------------------------------------------------------------
    // Optional lifecycle hooks
    // -----------------------------------------------------------------------

    /// Called once before the plugin begins accepting tasks.
    ///
    /// @param config  Key/value configuration passed at plugin startup.
    virtual void on_start(const std::unordered_map<std::string, std::string>& config) {
        (void)config;
    }

    /// Called once after the plugin finishes accepting tasks.
    virtual void on_stop() {}

    /// Return the current health status of the plugin.
    ///
    /// The runtime may call this periodically and on demand. The default
    /// implementation always reports healthy.
    virtual HealthStatus health_check() {
        return HealthStatus::ok();
    }

    // -----------------------------------------------------------------------
    // Event handling
    // -----------------------------------------------------------------------

    /// Handle a system event.
    virtual void on_event(const Event& event, const Context& ctx) {
        (void)event;
        (void)ctx;
    }

};

} // namespace malbox
