#pragma once

#include <cstdint>
#include <stdexcept>
#include <string>

extern "C" {
int32_t malbox_last_error(const char** out_message);
}

namespace malbox {

/// Well-known error categories returned by the C API.
enum class ErrorKind : int32_t {
    Unknown        = -1,
    InvalidContext = -2,
    ChannelClosed  = -3,
    Io             = -4,
    Transport      = -5,
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
