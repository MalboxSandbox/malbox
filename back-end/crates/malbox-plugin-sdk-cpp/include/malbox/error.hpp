#pragma once

#include <cstdint>
#include <stdexcept>
#include <string>

extern "C" {
int32_t malbox_last_error(const char** out_message);
}

namespace malbox {

/// Exception type thrown when a Malbox C API call fails (returns a negative rc).
class Error : public std::runtime_error {
public:
    Error(int32_t code, const std::string& message)
        : std::runtime_error(message), code_(code) {}

    /// The raw error code returned by the C API (always < 0 on failure).
    [[nodiscard]] int32_t code() const noexcept { return code_; }

private:
    int32_t code_;
};

namespace detail {

/// Check a return code from a Malbox C API function.
/// Throws malbox::Error (with message retrieved from malbox_last_error) if rc < 0.
inline void check_rc(int32_t rc) {
    if (rc < 0) {
        const char* msg = nullptr;
        malbox_last_error(&msg);
        throw malbox::Error{rc, msg ? msg : "unknown malbox error"};
    }
}

} // namespace detail
} // namespace malbox
