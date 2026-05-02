#pragma once

#include <cstdint>
#include <filesystem>
#include <memory>
#include <string>
#include <unordered_map>
#include <vector>
#include "../../generated/malbox_plugin.h"
#include <malbox/error.hpp>
#include <malbox/types.hpp>
#include <malbox/events.hpp>
#include <malbox/guest_plugin.hpp>
#include <malbox/handlers.hpp>
#include <malbox/result.hpp>
#include <malbox/context.hpp>

namespace malbox {

/// Auto-collection settings for a single directory.
struct AutoCollectConfig {
    bool                         enabled = false;
    std::vector<std::string>     include;
    std::vector<std::string>     exclude;
    std::uint64_t                max_file_size = 0;
};

/// Runtime configuration baked into the plugin at build time.
///
/// All paths must be absolute, non-null, and valid UTF-8.
struct RuntimeConfig {
    std::uint16_t port = 0;
    std::string   sample_dir;
    std::string   artifact_dir;
    std::string   stash_dir;
    std::string   log_dir;
    std::string   external_log_dir;
    std::size_t   stash_threshold_bytes = 0;
    std::uint64_t stash_ttl_secs = 0;
    std::string   log_filter;
    std::uint64_t analysis_timeout = 300;
    AutoCollectConfig auto_collect_artifacts;
    AutoCollectConfig auto_collect_external_logs;
};

namespace detail {

// -------------------------------------------------------------------------
// Host plugin trampolines
// -------------------------------------------------------------------------

/// Trampoline for HostPlugin::on_task.
///
/// Vtable signature: (void*, const MalboxContext*, MalboxResultBuilder*) -> i32
///
/// The plugin pushes results via ctx.results().push() which calls into the
/// Rust runtime directly. The result builder is unused by the C++ layer but
/// kept in the vtable for ABI compatibility.
inline int32_t trampoline_on_task(
    void*                       plugin_ptr,
    const MalboxContext*        ctx_ptr,
    MalboxResultBuilder*        /*builder*/)
{
    auto* plugin = static_cast<HostPlugin*>(plugin_ptr);
    try {
        auto ctx = Context::from_borrowed(ctx_ptr);
        plugin->on_task(ctx);
        return 0;
    } catch (const malbox::Error& e) {
        return e.code();
    } catch (const std::exception&) {
        return -1;
    } catch (...) {
        return -1;
    }
}

/// Trampoline for HostPlugin::on_start.
///
/// Vtable signature: (void*, const char*const*, const char*const*, uintptr_t) -> i32
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

/// Trampoline for HostPlugin::on_stop.
///
/// Vtable signature: (void*) -> i32
inline int32_t trampoline_on_stop(void* plugin_ptr) {
    auto* plugin = static_cast<HostPlugin*>(plugin_ptr);
    try {
        plugin->on_stop();
        return 0;
    } catch (...) {
        return -1;
    }
}

/// Trampoline for PluginBase::health_check (used by both host and guest).
///
/// Vtable signature: (void*, MalboxHealthStatus*) -> i32
template <typename T>
inline int32_t trampoline_health_check(
    void*                plugin_ptr,
    MalboxHealthStatus*  out_status)
{
    static thread_local std::string tl_reason;
    auto* plugin = static_cast<T*>(plugin_ptr);
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

/// Trampoline for HostPlugin::on_event.
///
/// Vtable signature: (void*, MalboxEvent) -> i32
/// Note: on_event does not receive a Context.
inline int32_t trampoline_on_event(
    void*                plugin_ptr,
    MalboxEvent          c_event)
{
    auto* plugin = static_cast<HostPlugin*>(plugin_ptr);
    try {
        Event event{c_event};
        plugin->on_event(event);
        return 0;
    } catch (...) {
        return -1;
    }
}

// -------------------------------------------------------------------------
// Vtable builder helper
// -------------------------------------------------------------------------

/// Build a MalboxPluginVtable from a raw HostPlugin pointer.
inline MalboxPluginVtable build_vtable(HostPlugin* plugin_ptr) {
    MalboxPluginVtable vtable{};
    vtable.abi_version        = MALBOX_ABI_VERSION;
    vtable.plugin_ptr         = plugin_ptr;
    vtable.on_task            = &trampoline_on_task;
    vtable.on_start           = &trampoline_on_start;
    vtable.on_stop            = &trampoline_on_stop;
    vtable.health_check       = &trampoline_health_check<HostPlugin>;
    vtable.on_event           = &trampoline_on_event;
    return vtable;
}

/// Convert a C++ PluginMeta to the C MalboxPluginMeta.
///
/// The returned struct borrows from the source PluginMeta's std::string
/// members, so the PluginMeta must outlive the returned C struct.
inline MalboxPluginMeta build_c_meta(const PluginMeta& meta) {
    MalboxPluginMeta c{};
    c.name        = meta.name.c_str();
    c.version     = meta.version.c_str();
    c.description = meta.description.empty() ? nullptr : meta.description.c_str();
    c.authors     = meta.authors.c_str();
    c.state       = static_cast<MalboxPluginState>(meta.state);
    c.execution   = static_cast<MalboxExecutionContext>(meta.execution);
    return c;
}

// -------------------------------------------------------------------------
// Guest plugin trampolines
// -------------------------------------------------------------------------

/// Trampoline for GuestPlugin::on_start.
///
/// Vtable signature: (void*, const MalboxContext*) -> i32
inline int32_t trampoline_guest_on_start(
    void* plugin_ptr,
    const MalboxContext* ctx_ptr)
{
    auto* plugin = static_cast<GuestPlugin*>(plugin_ptr);
    try {
        auto ctx = Context::from_borrowed(ctx_ptr);
        plugin->on_start(ctx);
        return 0;
    } catch (...) { return -1; }
}

/// Trampoline for GuestPlugin::on_stop.
///
/// Vtable signature: (void*, const MalboxContext*) -> i32
inline int32_t trampoline_guest_on_stop(
    void* plugin_ptr,
    const MalboxContext* ctx_ptr)
{
    auto* plugin = static_cast<GuestPlugin*>(plugin_ptr);
    try {
        auto ctx = Context::from_borrowed(ctx_ptr);
        plugin->on_stop(ctx);
        return 0;
    } catch (...) { return -1; }
}

/// Trampoline for GuestPlugin::execute_sample.
///
/// Vtable signature: (void*, const char*) -> i32
/// Returns 0 for UseDefault, 1 for Launched, negative for error.
inline int32_t trampoline_guest_execute_sample(
    void* plugin_ptr,
    const char* sample_path_utf8)
{
    auto* plugin = static_cast<GuestPlugin*>(plugin_ptr);
    try {
        const auto* begin = reinterpret_cast<const char8_t*>(sample_path_utf8);
        const auto len = std::char_traits<char>::length(sample_path_utf8);
        std::filesystem::path p{begin, begin + len};
        return static_cast<int32_t>(plugin->execute_sample(p));
    } catch (...) { return -1; }
}

/// Build a MalboxGuestPluginVtable from a raw GuestPlugin pointer.
inline MalboxGuestPluginVtable build_guest_vtable(GuestPlugin* plugin_ptr) {
    MalboxGuestPluginVtable vtable{};
    vtable.abi_version    = MALBOX_ABI_VERSION;
    vtable.plugin_ptr     = plugin_ptr;
    vtable.on_start       = &trampoline_guest_on_start;
    vtable.on_stop        = &trampoline_guest_on_stop;
    vtable.execute_sample = &trampoline_guest_execute_sample;
    vtable.health_check   = &trampoline_health_check<GuestPlugin>;
    return vtable;
}

} // namespace detail

// -------------------------------------------------------------------------
// Public entry points
// -------------------------------------------------------------------------

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

    // Convert std::vector<std::string> include/exclude to raw C arrays.
    // Keep the CString-like storage alive for the duration of the FFI call.
    auto make_c_ptrs = [](const std::vector<std::string>& strings)
        -> std::pair<std::vector<const char*>, std::size_t>
    {
        std::vector<const char*> ptrs;
        ptrs.reserve(strings.size());
        for (const auto& s : strings) {
            ptrs.push_back(s.c_str());
        }
        return {ptrs, strings.size()};
    };

    auto [art_inc_ptrs, art_inc_count] = make_c_ptrs(config.auto_collect_artifacts.include);
    auto [art_exc_ptrs, art_exc_count] = make_c_ptrs(config.auto_collect_artifacts.exclude);
    auto [log_inc_ptrs, log_inc_count] = make_c_ptrs(config.auto_collect_external_logs.include);
    auto [log_exc_ptrs, log_exc_count] = make_c_ptrs(config.auto_collect_external_logs.exclude);

    MalboxAutoCollectConfig c_art{};
    c_art.enabled       = config.auto_collect_artifacts.enabled;
    c_art.include       = art_inc_ptrs.empty() ? nullptr : art_inc_ptrs.data();
    c_art.include_count = art_inc_count;
    c_art.exclude       = art_exc_ptrs.empty() ? nullptr : art_exc_ptrs.data();
    c_art.exclude_count = art_exc_count;
    c_art.max_file_size = config.auto_collect_artifacts.max_file_size;

    MalboxAutoCollectConfig c_log{};
    c_log.enabled       = config.auto_collect_external_logs.enabled;
    c_log.include       = log_inc_ptrs.empty() ? nullptr : log_inc_ptrs.data();
    c_log.include_count = log_inc_count;
    c_log.exclude       = log_exc_ptrs.empty() ? nullptr : log_exc_ptrs.data();
    c_log.exclude_count = log_exc_count;
    c_log.max_file_size = config.auto_collect_external_logs.max_file_size;

    MalboxGuestRuntimeConfig c_config{};
    c_config.port                       = config.port;
    c_config.sample_dir                 = config.sample_dir.c_str();
    c_config.artifact_dir               = config.artifact_dir.c_str();
    c_config.stash_dir                  = config.stash_dir.c_str();
    c_config.log_dir                    = config.log_dir.c_str();
    c_config.external_log_dir           = config.external_log_dir.c_str();
    c_config.stash_threshold_bytes      = config.stash_threshold_bytes;
    c_config.stash_ttl_secs             = config.stash_ttl_secs;
    c_config.log_filter                 = config.log_filter.c_str();
    c_config.analysis_timeout           = config.analysis_timeout;
    c_config.auto_collect_artifacts     = c_art;
    c_config.auto_collect_external_logs = c_log;

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
