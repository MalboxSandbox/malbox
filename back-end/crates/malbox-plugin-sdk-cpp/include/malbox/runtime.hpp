#pragma once

#include <cstdint>
#include <memory>
#include <string>
#include <unordered_map>
#include <vector>
#include "malbox_plugin.h"
#include <malbox/error.hpp>
#include <malbox/types.hpp>
#include <malbox/events.hpp>
#include <malbox/handlers.hpp>
#include <malbox/result.hpp>
#include <malbox/task.hpp>
#include <malbox/context.hpp>

namespace malbox {
namespace detail {

// ─────────────────────────────────────────────────────────────────────────────
// Trampoline functions
// ─────────────────────────────────────────────────────────────────────────────

/// Trampoline for Plugin::on_task.
///
/// The plugin pushes results via ctx.push_result() / ctx.flush_results()
/// which call into the Rust runtime directly. The result builder is unused
/// by the C++ layer but kept in the vtable for ABI compatibility.
inline int32_t trampoline_on_task(
    void*                       plugin_ptr,
    const MalboxTask*           task_ptr,
    const MalboxContext*        ctx_ptr,
    MalboxResultBuilder*        /*builder*/)
{
    auto* plugin = static_cast<Plugin*>(plugin_ptr);
    try {
        Task    task{task_ptr};
        Context ctx{ctx_ptr};
        plugin->on_task(task, ctx);
        return 0;
    } catch (const malbox::Error& e) {
        return e.code();
    } catch (const std::exception& e) {
        (void)e;
        return -1;
    } catch (...) {
        return -1;
    }
}

/// Trampoline for Plugin::on_start.
inline int32_t trampoline_on_start(
    void*              plugin_ptr,
    const char* const* keys,
    const char* const* values,
    uintptr_t          count)
{
    auto* plugin = static_cast<Plugin*>(plugin_ptr);
    try {
        std::unordered_map<std::string, std::string> cfg;
        cfg.reserve(count);
        for (uintptr_t i = 0; i < count; ++i) {
            cfg.emplace(keys[i] ? keys[i] : "", values[i] ? values[i] : "");
        }
        plugin->on_start(cfg);
        return 0;
    } catch (...) {
        return -1;
    }
}

/// Trampoline for Plugin::on_stop.
inline int32_t trampoline_on_stop(void* plugin_ptr) {
    auto* plugin = static_cast<Plugin*>(plugin_ptr);
    try {
        plugin->on_stop();
        return 0;
    } catch (...) {
        return -1;
    }
}

/// Trampoline for Plugin::health_check.
inline int32_t trampoline_health_check(
    void*                plugin_ptr,
    MalboxHealthStatus*  out_status)
{
    static thread_local std::string tl_reason;
    auto* plugin = static_cast<Plugin*>(plugin_ptr);
    try {
        HealthStatus hs = plugin->health_check();
        tl_reason        = hs.reason;
        out_status->ready  = hs.ready;
        out_status->reason = tl_reason.c_str();
        return 0;
    } catch (...) {
        out_status->ready  = false;
        tl_reason          = "health_check threw an exception";
        out_status->reason = tl_reason.c_str();
        return -1;
    }
}

/// Trampoline for Plugin::on_event — single unified event handler.
inline int32_t trampoline_on_event(
    void*                plugin_ptr,
    MalboxEvent          c_event,
    const MalboxContext* ctx_ptr)
{
    auto* plugin = static_cast<Plugin*>(plugin_ptr);
    try {
        Event   event{c_event};
        Context ctx{ctx_ptr};
        plugin->on_event(event, ctx);
        return 0;
    } catch (...) {
        return -1;
    }
}

/// Trampoline for Plugin::on_execute_command.
inline int32_t trampoline_on_execute_command(
    void* plugin_ptr,
    const MalboxExecRequest* request,
    MalboxExecResult* result)
{
    try {
        auto* plugin = static_cast<Plugin*>(plugin_ptr);
        return plugin->on_execute_command(request, result);
    } catch (const std::exception& e) {
        (void)e;
        return -1;
    } catch (...) {
        return -1;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Vtable builder helper
// ─────────────────────────────────────────────────────────────────────────────

/// Build a MalboxPluginVtable from a raw Plugin pointer.
inline MalboxPluginVtable build_vtable(Plugin* plugin_ptr) {
    MalboxPluginVtable vtable{};
    vtable.abi_version        = MALBOX_ABI_VERSION;
    vtable.plugin_ptr         = plugin_ptr;
    vtable.on_task            = &trampoline_on_task;
    vtable.on_start           = &trampoline_on_start;
    vtable.on_stop            = &trampoline_on_stop;
    vtable.health_check       = &trampoline_health_check;
    vtable.on_event           = &trampoline_on_event;
    vtable.on_execute_command = &trampoline_on_execute_command;
    return vtable;
}

/// Convert a C++ PluginMeta to the C MalboxPluginMeta.
inline MalboxPluginMeta build_c_meta(const PluginMeta& meta) {
    MalboxPluginMeta c{};
    c.name        = meta.name;
    c.version     = meta.version;
    c.description = meta.description;
    c.authors     = meta.authors;
    c.plugin_type = static_cast<MalboxPluginType>(meta.plugin_type);
    c.state       = static_cast<MalboxPluginState>(meta.state);
    c.execution   = static_cast<MalboxExecutionContext>(meta.execution);
    return c;
}

} // namespace detail

// ─────────────────────────────────────────────────────────────────────────────
// Public entry points
// ─────────────────────────────────────────────────────────────────────────────

/// Start the IPC-based host-plugin runtime.
///
/// This function blocks until the runtime shuts down. Call it from main().
/// Throws malbox::Error on failure.
inline void run_host_plugin(std::unique_ptr<Plugin> plugin, const PluginMeta& meta) {
    MalboxPluginVtable vtable = detail::build_vtable(plugin.get());
    MalboxPluginMeta   c_meta = detail::build_c_meta(meta);
    detail::check_rc(malbox_run_host_plugin(vtable, c_meta));
}

/// Start the gRPC-based guest-plugin runtime.
///
/// This function blocks until the runtime shuts down. Call it from main().
/// Throws malbox::Error on failure.
inline void run_guest_plugin(std::unique_ptr<Plugin> plugin, const PluginMeta& meta) {
    MalboxPluginVtable vtable = detail::build_vtable(plugin.get());
    MalboxPluginMeta   c_meta = detail::build_c_meta(meta);
    detail::check_rc(malbox_run_guest_plugin(vtable, c_meta));
}

/// Run a plugin through a synthetic test lifecycle without starting any transport.
///
/// Useful in plugin unit tests.
/// Throws malbox::Error on failure.
inline void test_run_plugin(
    std::unique_ptr<Plugin>  plugin,
    int32_t                  task_id,
    const char*              sample_path,
    const std::unordered_map<std::string, std::string>& config = {})
{
    std::vector<const char*> keys, values;
    keys.reserve(config.size());
    values.reserve(config.size());
    for (const auto& [k, v] : config) {
        keys.push_back(k.c_str());
        values.push_back(v.c_str());
    }

    MalboxTestConfig cfg{};
    cfg.task_id      = task_id;
    cfg.sample_path  = sample_path;
    cfg.config_keys   = keys.empty()   ? nullptr : keys.data();
    cfg.config_values = values.empty() ? nullptr : values.data();
    cfg.config_count  = static_cast<uintptr_t>(config.size());

    MalboxPluginVtable vtable = detail::build_vtable(plugin.get());
    detail::check_rc(malbox_test_run_plugin(vtable, cfg));
}

} // namespace malbox
