#pragma once

/// @file log.hpp
/// @brief Structured logging for Malbox plugins.

#include <cstdint>
#include <string>

extern "C" {
    void malbox_log(int32_t level, const char* target, const char* message);
    void malbox_log_trace(const char* target, const char* message);
    void malbox_log_debug(const char* target, const char* message);
    void malbox_log_info(const char* target, const char* message);
    void malbox_log_warn(const char* target, const char* message);
    void malbox_log_error(const char* target, const char* message);
}

namespace malbox::log {

inline void trace(const std::string& target, const std::string& message) {
    malbox_log_trace(target.c_str(), message.c_str());
}

inline void debug(const std::string& target, const std::string& message) {
    malbox_log_debug(target.c_str(), message.c_str());
}

inline void info(const std::string& target, const std::string& message) {
    malbox_log_info(target.c_str(), message.c_str());
}

inline void warn(const std::string& target, const std::string& message) {
    malbox_log_warn(target.c_str(), message.c_str());
}

inline void error(const std::string& target, const std::string& message) {
    malbox_log_error(target.c_str(), message.c_str());
}

} // namespace malbox::log

#define MALBOX_LOG_TRACE(msg) malbox::log::trace(std::string(__FILE__) + "::" + __func__, (msg))
#define MALBOX_LOG_DEBUG(msg) malbox::log::debug(std::string(__FILE__) + "::" + __func__, (msg))
#define MALBOX_LOG_INFO(msg)  malbox::log::info(std::string(__FILE__) + "::" + __func__, (msg))
#define MALBOX_LOG_WARN(msg)  malbox::log::warn(std::string(__FILE__) + "::" + __func__, (msg))
#define MALBOX_LOG_ERROR(msg) malbox::log::error(std::string(__FILE__) + "::" + __func__, (msg))
