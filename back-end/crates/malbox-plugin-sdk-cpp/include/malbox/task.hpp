#pragma once

#include <cstdint>
#include <optional>
#include <string>
#include <unordered_map>
#include <vector>
#include "malbox_plugin.h"
#include <malbox/error.hpp>

namespace malbox {

/// Non-owning wrapper around a const MalboxTask*.
///
/// The pointer is owned by the Malbox runtime and remains valid for the
/// duration of the on_task callback. Do not store a Task beyond that scope.
class Task {
public:
    explicit Task(const MalboxTask* ptr) noexcept : ptr_(ptr) {}

    // Non-copyable, non-movable — the underlying pointer is runtime-owned.
    Task(const Task&)            = delete;
    Task& operator=(const Task&) = delete;
    Task(Task&&)                 = delete;
    Task& operator=(Task&&)      = delete;

    /// Return the numeric task identifier.
    [[nodiscard]] int32_t id() const {
        int32_t result = malbox_task_get_id(ptr_);
        if (result < 0) {
            const char* msg = nullptr;
            malbox_last_error(&msg);
            throw malbox::Error{result, msg ? msg : "malbox_task_get_id failed"};
        }
        return result;
    }

    /// Return the path to the sample file as a std::string.
    [[nodiscard]] std::string sample_path() const {
        const char* p = malbox_task_get_sample_path(ptr_);
        if (!p) {
            const char* msg = nullptr;
            malbox_last_error(&msg);
            throw malbox::Error{-1, msg ? msg : "malbox_task_get_sample_path failed"};
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
    const MalboxTask* ptr_;
};

} // namespace malbox
