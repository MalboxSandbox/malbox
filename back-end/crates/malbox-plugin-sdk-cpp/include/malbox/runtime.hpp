#pragma once

#include <cstdint>
#include <filesystem>
#include <memory>
#include <string>
#include <unordered_map>
#include <vector>
#include "malbox_plugin.h"
#include <malbox/error.hpp>
#include <malbox/types.hpp>
#include <malbox/events.hpp>
#include <malbox/guest_plugin.hpp>
#include <malbox/handlers.hpp>
#include <malbox/result.hpp>
#include <malbox/task.hpp>
#include <malbox/context.hpp>

namespace malbox {

/// Auto-collection settings for a single directory.
struct AutoCollectConfig {
    bool                enabled;
    const char* const*  include;
    std::size_t         include_count;
    const char* const*  exclude;
    std::size_t         exclude_count;
    std::uint64_t       max_file_size;
};

/// Runtime configuration baked into the plugin at build time.
///
/// All paths must be absolute, non-null, and valid UTF-8.
struct RuntimeConfig {
    std::uint16_t port;
    const char*   sample_dir;
    const char*   artifact_dir;
    const char*   stash_dir;
    const char*   log_dir;
    const char*   external_log_dir;
    std::size_t   stash_threshold_bytes;
    std::uint64_t stash_ttl_secs;
    const char*   log_filter;
    std::uint64_t analysis_timeout = 300;
    AutoCollectConfig auto_collect_artifacts;
    AutoCollectConfig auto_collect_external_logs;
};

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
    auto* plugin = static_cast<HostPlugin*>(plugin_ptr);
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
    auto* plugin = static_cast<HostPlugin*>(plugin_ptr);
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
    auto* plugin = static_cast<HostPlugin*>(plugin_ptr);
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
    auto* plugin = static_cast<HostPlugin*>(plugin_ptr);
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
    auto* plugin = static_cast<HostPlugin*>(plugin_ptr);
    try {
        Event   event{c_event};
        Context ctx{ctx_ptr};
        plugin->on_event(event, ctx);
        return 0;
    } catch (...) {
        return -1;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Vtable builder helper
// ─────────────────────────────────────────────────────────────────────────────

/// Build a MalboxPluginVtable from a raw Plugin pointer.
inline MalboxPluginVtable build_vtable(HostPlugin* plugin_ptr) {
    MalboxPluginVtable vtable{};
    vtable.abi_version        = MALBOX_ABI_VERSION;
    vtable.plugin_ptr         = plugin_ptr;
    vtable.on_task            = &trampoline_on_task;
    vtable.on_start           = &trampoline_on_start;
    vtable.on_stop            = &trampoline_on_stop;
    vtable.health_check       = &trampoline_health_check;
    vtable.on_event           = &trampoline_on_event;
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

// ─────────────────────────────────────────────────────────────────────────────
// Guest plugin trampolines
// ─────────────────────────────────────────────────────────────────────────────

inline int32_t trampoline_guest_on_start(
    void* plugin_ptr,
    const MalboxTask* task_ptr,
    const MalboxContext* ctx_ptr)
{
    auto* plugin = static_cast<GuestPlugin*>(plugin_ptr);
    try {
        Task task{task_ptr};
        Context ctx{ctx_ptr};
        plugin->on_start(task, ctx);
        return 0;
    } catch (...) { return -1; }
}

inline int32_t trampoline_guest_on_stop(
    void* plugin_ptr,
    const MalboxContext* ctx_ptr)
{
    auto* plugin = static_cast<GuestPlugin*>(plugin_ptr);
    try {
        Context ctx{ctx_ptr};
        plugin->on_stop(ctx);
        return 0;
    } catch (...) { return -1; }
}

inline int32_t trampoline_guest_execute_sample(
    void* plugin_ptr,
    const char* sample_path_utf8)
{
    auto* plugin = static_cast<GuestPlugin*>(plugin_ptr);
    try {
        const auto* begin = reinterpret_cast<const char8_t*>(sample_path_utf8);
        const auto len = std::char_traits<char>::length(sample_path_utf8);
        std::filesystem::path p{begin, begin + len};
        return plugin->execute_sample(p);
    } catch (...) { return -1; }
}

inline int32_t trampoline_guest_health_check(
    void*                plugin_ptr,
    MalboxHealthStatus*  out_status)
{
    static thread_local std::string tl_reason;
    auto* plugin = static_cast<GuestPlugin*>(plugin_ptr);
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

inline MalboxGuestPluginVtable build_guest_vtable(GuestPlugin* plugin_ptr) {
    MalboxGuestPluginVtable vtable{};
    vtable.abi_version    = MALBOX_ABI_VERSION;
    vtable.plugin_ptr     = plugin_ptr;
    vtable.on_start       = &trampoline_guest_on_start;
    vtable.on_stop        = &trampoline_guest_on_stop;
    vtable.execute_sample = &trampoline_guest_execute_sample;
    vtable.health_check   = &trampoline_guest_health_check;
    return vtable;
}

} // namespace detail

// ─────────────────────────────────────────────────────────────────────────────
// Public entry points
// ─────────────────────────────────────────────────────────────────────────────

/// Start the IPC-based host-plugin runtime.
///
/// This function blocks until the runtime shuts down. Call it from main().
/// Throws malbox::Error on failure.
inline void run_host_plugin(std::unique_ptr<HostPlugin> plugin, const PluginMeta& meta) {
    MalboxPluginVtable vtable = detail::build_vtable(plugin.get());
    MalboxPluginMeta   c_meta = detail::build_c_meta(meta);
    detail::check_rc(malbox_run_host_plugin(vtable, c_meta));
}

/// Start the gRPC-based guest-plugin runtime.
///
/// This function blocks until the runtime shuts down. Call it from main().
/// Throws malbox::Error on failure.
inline void run_guest_plugin(
    std::unique_ptr<GuestPlugin> plugin,
    const PluginMeta&            meta,
    const RuntimeConfig&         config)
{
    MalboxGuestPluginVtable vtable = detail::build_guest_vtable(plugin.get());
    MalboxPluginMeta        c_meta = detail::build_c_meta(meta);

    auto to_c_ac = [](const AutoCollectConfig& ac) -> MalboxAutoCollectConfig {
        MalboxAutoCollectConfig c{};
        c.enabled       = ac.enabled;
        c.include       = ac.include;
        c.include_count = ac.include_count;
        c.exclude       = ac.exclude;
        c.exclude_count = ac.exclude_count;
        c.max_file_size = ac.max_file_size;
        return c;
    };

    MalboxGuestRuntimeConfig c_config{};
    c_config.port                       = config.port;
    c_config.sample_dir                 = config.sample_dir;
    c_config.artifact_dir               = config.artifact_dir;
    c_config.stash_dir                  = config.stash_dir;
    c_config.log_dir                    = config.log_dir;
    c_config.external_log_dir           = config.external_log_dir;
    c_config.stash_threshold_bytes      = config.stash_threshold_bytes;
    c_config.stash_ttl_secs             = config.stash_ttl_secs;
    c_config.log_filter                 = config.log_filter;
    c_config.analysis_timeout           = config.analysis_timeout;
    c_config.auto_collect_artifacts     = to_c_ac(config.auto_collect_artifacts);
    c_config.auto_collect_external_logs = to_c_ac(config.auto_collect_external_logs);

    detail::check_rc(malbox_run_guest_plugin(vtable, c_meta, c_config));
}

/// Run a plugin through a synthetic test lifecycle without starting any transport.
///
/// Useful in plugin unit tests.
/// Throws malbox::Error on failure.
inline void test_run_plugin(
    std::unique_ptr<HostPlugin>  plugin,
    int32_t                      task_id,
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
