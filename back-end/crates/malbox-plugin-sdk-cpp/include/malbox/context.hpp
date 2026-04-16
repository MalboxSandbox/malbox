#pragma once

#include <chrono>
#include <cstdint>
#include <vector>
#include "malbox_plugin.h"
#include <malbox/error.hpp>
#include <malbox/events.hpp>
#include <malbox/result.hpp>
#include <malbox/types.hpp>

namespace malbox {

/// Non-owning wrapper around a const MalboxContext*.
///
/// The pointer is owned by the Malbox runtime. Do not store a Context beyond
/// the callback that receives it.
class Context {
public:
    explicit Context(const MalboxContext* ptr) noexcept : ptr_(ptr) {}

    // Non-copyable, non-movable — the underlying pointer is runtime-owned.
    Context(const Context&)            = delete;
    Context& operator=(const Context&) = delete;
    Context(Context&&)                 = delete;
    Context& operator=(Context&&)      = delete;

    /// Report execution progress (0.0 to 1.0) with a status message.
    void emit_progress(double progress, const char* message) const {
        detail::check_rc(malbox_context_emit_progress(ptr_, progress, message));
    }

    /// Log a warning that will be attached to the task report.
    void warn(const char* message) const {
        detail::check_rc(malbox_context_warn(ptr_, message));
    }

    /// Emit a flat event back to the daemon.
    void emit_event(const Event& event) const {
        detail::check_rc(malbox_context_emit_event(ptr_, event.to_c()));
    }

    /// Push a single result to the daemon during on_task.
    ///
    /// This is the primary way to return results from a plugin. Call it
    /// one or more times during on_task to stream results incrementally.
    void push_result(const PluginResult& result) const {
        flush_results({result});
    }

    /// Flush a batch of results to the daemon immediately during on_task.
    ///
    /// All result types (JSON, Bytes, File) are supported. File-type results
    /// are read into memory by the Rust runtime before being sent.
    void flush_results(const std::vector<PluginResult>& results) const {
        if (results.empty()) return;

        std::vector<const char*> names;
        std::vector<const uint8_t*> datas;
        std::vector<uintptr_t> data_lens;
        std::vector<int32_t> formats;

        names.reserve(results.size());
        datas.reserve(results.size());
        data_lens.reserve(results.size());
        formats.reserve(results.size());

        for (const auto& r : results) {
            switch (r.tag()) {
                case PluginResult::Tag::Json:
                    names.push_back(r.name().c_str());
                    datas.push_back(r.data().data());
                    data_lens.push_back(r.data().size());
                    formats.push_back(1); // JSON
                    break;
                case PluginResult::Tag::Bytes:
                    names.push_back(r.name().c_str());
                    datas.push_back(r.data().data());
                    data_lens.push_back(r.data().size());
                    formats.push_back(2); // Bytes
                    break;
                case PluginResult::Tag::File:
                    // Format 3: File — data is the null-terminated path string.
                    names.push_back(r.name().c_str());
                    datas.push_back(reinterpret_cast<const uint8_t*>(r.path().c_str()));
                    data_lens.push_back(r.path().size() + 1); // include null terminator
                    formats.push_back(3); // File
                    break;
            }
        }

        if (names.empty()) return;

        detail::check_rc(malbox_context_flush_results(
            ptr_, names.data(), datas.data(), data_lens.data(),
            formats.data(), names.size()));
    }

    /// Block until sample execution begins. Returns execution metadata.
    ExecutionInfo wait_for_execution() const {
        uint32_t pid = 0;
        const char* cmd = nullptr;
        const char* const* args = nullptr;
        size_t args_count = 0;
        detail::check_rc(malbox_context_wait_for_execution(
            ptr_, &pid, &cmd, &args, &args_count));
        ExecutionInfo info;
        info.pid = pid;
        info.command = cmd ? std::string(cmd) : std::string();
        info.args.reserve(args_count);
        for (size_t i = 0; i < args_count; ++i) {
            info.args.emplace_back(args[i] ? args[i] : "");
        }
        return info;
    }

    /// Block with custom timeout (milliseconds).
    ExecutionInfo wait_for_execution(std::chrono::milliseconds timeout) const {
        uint32_t pid = 0;
        const char* cmd = nullptr;
        const char* const* args = nullptr;
        size_t args_count = 0;
        detail::check_rc(malbox_context_wait_for_execution_timeout(
            ptr_, static_cast<uint64_t>(timeout.count()),
            &pid, &cmd, &args, &args_count));
        ExecutionInfo info;
        info.pid = pid;
        info.command = cmd ? std::string(cmd) : std::string();
        info.args.reserve(args_count);
        for (size_t i = 0; i < args_count; ++i) {
            info.args.emplace_back(args[i] ? args[i] : "");
        }
        return info;
    }

private:
    const MalboxContext* ptr_;
};

} // namespace malbox
