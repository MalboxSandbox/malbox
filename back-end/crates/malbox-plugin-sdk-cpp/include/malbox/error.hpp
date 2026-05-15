#pragma once

#include <cstdint>
#include <stdexcept>
#include <string>

extern "C" {
int32_t malbox_last_error(const char** out_message);
}

namespace malbox {

/// Error category returned by the C API.
///
/// The Rust FFI layer currently returns -1 for all errors; the specific
/// category is conveyed through the message string retrieved via
/// malbox_last_error(). This enum is kept as a single value for forward
/// compatibility - differentiated codes may be added in the future.
enum class ErrorKind : int32_t {
    Unknown = -1,
};

/// Exception type thrown when a Malbox C API call fails (returns a negative rc).
class Error : public std::runtime_error {
public:
    Error(ErrorKind kind, const std::string& message)
        : std::runtime_error(message), kind_(kind) {}

    /// The structured error category.
    [[nodiscard]] ErrorKind kind() const noexcept { return kind_; }

    /// The raw error code returned by the C API (always < 0 on failure).
    [[nodiscard]] int32_t code() const noexcept { return static_cast<int32_t>(kind_); }

private:
    ErrorKind kind_;
};

namespace detail {

/// Check a return code from a Malbox C API function.
/// Throws malbox::Error (with message retrieved from malbox_last_error) if rc < 0.
inline void check_rc(int32_t rc) {
    if (rc < 0) {
        const char* msg = nullptr;
        malbox_last_error(&msg);
        auto kind = static_cast<ErrorKind>(rc);
        throw malbox::Error{kind, msg ? msg : "unknown malbox error"};
    }
}

} // namespace detail
} // namespace malbox
