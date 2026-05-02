#pragma once

#include <cstdint>
#include <cstring>
#include <optional>
#include <span>
#include <string>
#include <unordered_map>
#include <vector>
#include "../../generated/malbox_plugin.h"
#include <malbox/error.hpp>
#include <malbox/events.hpp>
#include <malbox/result.hpp>
#include <malbox/types.hpp>

extern "C" {
MalboxContext* malbox_context_clone(const MalboxContext* ctx);
void malbox_context_drop(MalboxContext* ctx);
}

namespace malbox {

// Forward declarations
class TaskInfo;
class ResultSink;

/// Ref-counted handle to the runtime context for the current task.
///
/// Copyable and thread-safe. Clone it to share with background threads -
/// the underlying Rust Arc keeps the context alive. The runtime closes the
/// result channel after on_stop; late pushes return an error.
class Context {
public:
    /// Wrap an already-cloned (owned) MalboxContext pointer.
    /// Takes ownership - will call malbox_context_drop on destruction.
    explicit Context(MalboxContext* owned_ptr) noexcept : ptr_(owned_ptr) {}

    /// Wrap a borrowed (non-owned) pointer by cloning it.
    static Context from_borrowed(const MalboxContext* borrowed) {
        return Context{malbox_context_clone(borrowed)};
    }

    Context(const Context& other) : ptr_(malbox_context_clone(other.ptr_)) {}
    Context& operator=(const Context& other) {
        if (this != &other) {
            if (ptr_) malbox_context_drop(ptr_);
            ptr_ = malbox_context_clone(other.ptr_);
        }
        return *this;
    }
    Context(Context&& other) noexcept : ptr_(other.ptr_) { other.ptr_ = nullptr; }
    Context& operator=(Context&& other) noexcept {
        if (this != &other) {
            if (ptr_) malbox_context_drop(ptr_);
            ptr_ = other.ptr_;
            other.ptr_ = nullptr;
        }
        return *this;
    }
    ~Context() { if (ptr_) malbox_context_drop(ptr_); }

    /// Access task metadata (id, sample path, config).
    [[nodiscard]] TaskInfo task() const;

    /// Access the result sink for pushing results.
    [[nodiscard]] ResultSink results() const;

    /// Report execution progress (0.0 to 1.0) with a status message.
    void progress(double pct, const char* message) const {
        detail::check_rc(malbox_context_emit_progress(ptr_, pct, message));
    }

    /// Log a warning that will be attached to the task report.
    void warn(const char* message) const {
        detail::check_rc(malbox_context_warn(ptr_, message));
    }

    /// Emit a flat event back to the daemon.
    void emit_event(const Event& event) const {
        detail::check_rc(malbox_context_emit_event(ptr_, event.to_c()));
    }

    /// Get the raw opaque pointer (for advanced use).
    [[nodiscard]] const MalboxContext* raw() const noexcept { return ptr_; }

private:
    MalboxContext* ptr_;
};

/// Non-owning view into task metadata stored on the Context.
///
/// Obtained via Context::task(). The TaskInfo borrows the Context's pointer
/// and must not outlive it.
class TaskInfo {
public:
    explicit TaskInfo(MalboxContext* ptr) noexcept : ptr_(ptr) {}

    /// Return the numeric task identifier.
    [[nodiscard]] int32_t id() const {
        int32_t result = malbox_task_get_id(ptr_);
        if (result < 0) {
            const char* msg = nullptr;
            malbox_last_error(&msg);
            throw malbox::Error{ErrorKind::Unknown, msg ? msg : "malbox_task_get_id failed"};
        }
        return result;
    }

    /// Return the path to the sample file as a std::string.
    [[nodiscard]] std::string sample_path() const {
        const char* p = malbox_task_get_sample_path(ptr_);
        if (!p) {
            const char* msg = nullptr;
            malbox_last_error(&msg);
            throw malbox::Error{ErrorKind::Unknown, msg ? msg : "malbox_task_get_sample_path failed"};
        }
        return std::string{p};
    }

    /// Read the sample file into memory and return the bytes.
    [[nodiscard]] std::vector<uint8_t> sample_bytes() const {
        const uint8_t* data  = nullptr;
        uintptr_t      len   = 0;
        int32_t        rc    = malbox_task_get_sample_bytes(ptr_, &data, &len);
        detail::check_rc(rc);
        return std::vector<uint8_t>{data, data + len};
    }

    /// Return all configuration key/value pairs as an unordered_map.
    [[nodiscard]] std::unordered_map<std::string, std::string> config() const {
        uintptr_t count = malbox_task_get_config_count(ptr_);
        std::unordered_map<std::string, std::string> result;
        result.reserve(count);
        for (uintptr_t i = 0; i < count; ++i) {
            const char* key   = nullptr;
            const char* value = nullptr;
            int32_t rc = malbox_task_get_config_entry(ptr_, i, &key, &value);
            detail::check_rc(rc);
            result.emplace(key ? key : "", value ? value : "");
        }
        return result;
    }

    /// Look up a single configuration value by key.
    /// Returns std::nullopt if the key is not present.
    [[nodiscard]] std::optional<std::string> config_value(const char* key) const {
        const char* val = malbox_task_get_config_value(ptr_, key);
        if (!val) {
            return std::nullopt;
        }
        return std::string{val};
    }

private:
    MalboxContext* ptr_;  // non-owning, borrows from Context
};

/// Non-owning handle for pushing results via the Context.
///
/// Obtained via Context::results(). The ResultSink borrows the Context's
/// pointer and must not outlive it.
class ResultSink {
public:
    explicit ResultSink(MalboxContext* ptr) noexcept : ptr_(ptr) {}

    /// Push a single PluginResult to the daemon.
    void push(const PluginResult& result) const {
        flush(std::span<const PluginResult>(&result, 1));
    }

    /// Push a JSON-encoded result from raw bytes.
    void push_json(const char* name, std::span<const uint8_t> data) const {
        const char* names[] = {name};
        const uint8_t* datas[] = {data.data()};
        uintptr_t lens[] = {data.size()};
        int32_t formats[] = {1};
        detail::check_rc(malbox_context_flush_results(ptr_, names, datas, lens, formats, 1));
    }

    /// Push a raw-bytes result.
    void push_bytes(const char* name, std::span<const uint8_t> data) const {
        const char* names[] = {name};
        const uint8_t* datas[] = {data.data()};
        uintptr_t lens[] = {data.size()};
        int32_t formats[] = {2};
        detail::check_rc(malbox_context_flush_results(ptr_, names, datas, lens, formats, 1));
    }

    /// Push a file-reference result. The file at path will be read by the runtime.
    void push_file(const char* name, const char* path) const {
        const char* names[] = {name};
        const uint8_t* datas[] = {reinterpret_cast<const uint8_t*>(path)};
        uintptr_t lens[] = {std::strlen(path) + 1};
        int32_t formats[] = {3};
        detail::check_rc(malbox_context_flush_results(ptr_, names, datas, lens, formats, 1));
    }

    /// Flush a batch of PluginResult objects to the daemon.
    void flush(std::span<const PluginResult> results) const {
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
                    formats.push_back(1);
                    break;
                case PluginResult::Tag::Bytes:
                    names.push_back(r.name().c_str());
                    datas.push_back(r.data().data());
                    data_lens.push_back(r.data().size());
                    formats.push_back(2);
                    break;
                case PluginResult::Tag::File:
                    names.push_back(r.name().c_str());
                    datas.push_back(reinterpret_cast<const uint8_t*>(r.path().c_str()));
                    data_lens.push_back(r.path().size() + 1);
                    formats.push_back(3);
                    break;
            }
        }

        if (names.empty()) return;

        detail::check_rc(malbox_context_flush_results(
            ptr_, names.data(), datas.data(), data_lens.data(),
            formats.data(), names.size()));
    }

#ifdef MALBOX_HAS_NLOHMANN_JSON
    /// Push a JSON result by serialising an nlohmann::json-compatible type T.
    template <typename T>
    void push_json(const char* name, const T& value) const {
        std::string s = nlohmann::json(value).dump();
        push_json(name, std::span<const uint8_t>{
            reinterpret_cast<const uint8_t*>(s.data()), s.size()});
    }
#endif

private:
    MalboxContext* ptr_;  // non-owning
};

// Inline definitions after both classes are complete
inline TaskInfo Context::task() const { return TaskInfo{ptr_}; }
inline ResultSink Context::results() const { return ResultSink{ptr_}; }

} // namespace malbox
