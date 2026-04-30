#pragma once

#include <string>
#include <unordered_map>
#include <malbox/events.hpp>
#include <malbox/context.hpp>
#include <malbox/guest_plugin.hpp>
#include <malbox/types.hpp>

namespace malbox {

/// Abstract base class for a host-side Malbox C++ plugin.
///
/// All methods have default (no-op) implementations. A useful plugin should
/// at minimum override on_task. Results are pushed via ctx.results().push()
/// during on_task rather than returned.
///
/// ## Thread Safety
/// The runtime guarantees that only one callback is active at a time (unless
/// ExecutionContext::Parallel is chosen). Plugin authors should still protect
/// any shared mutable state with appropriate synchronisation primitives when
/// using the parallel context.
class HostPlugin : public PluginBase {
public:
    // -----------------------------------------------------------------------
    // Task processing
    // -----------------------------------------------------------------------

    /// Process a single analysis task.
    ///
    /// Push results via ctx.results().push() during execution. The runtime
    /// sends a final marker automatically once this method returns.
    virtual void on_task(const Context& ctx) {
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

    // -----------------------------------------------------------------------
    // Event handling
    // -----------------------------------------------------------------------

    /// Handle a system event.
    ///
    /// Note: on_event does not receive a Context. Events are system-wide
    /// notifications not tied to a specific task.
    virtual void on_event(const Event& event) {
        (void)event;
    }
};

} // namespace malbox
