#pragma once

#include <cstdint>
#include <span>
#include <string>
#include <vector>
#include "../../generated/malbox_plugin.h"
#include <malbox/error.hpp>

namespace malbox {

/// Represents a single analysis result produced by a plugin.
///
/// Construct via the static factory methods json(), bytes(), or file().
class PluginResult {
public:
    /// Discriminant indicating how the result data is encoded.
    enum class Tag : uint8_t {
        Json  = 0,
        Bytes = 1,
        File  = 2,
    };

    // -----------------------------------------------------------------------
    // Factory methods
    // -----------------------------------------------------------------------

    /// Create a JSON-encoded result from a byte span.
    [[nodiscard]] static PluginResult json(std::string name, std::span<const uint8_t> data) {
        PluginResult r;
        r.tag_  = Tag::Json;
        r.name_ = std::move(name);
        r.data_.assign(data.begin(), data.end());
        return r;
    }

    /// Create a raw-bytes result from a byte span.
    [[nodiscard]] static PluginResult bytes(std::string name, std::span<const uint8_t> data) {
        PluginResult r;
        r.tag_  = Tag::Bytes;
        r.name_ = std::move(name);
        r.data_.assign(data.begin(), data.end());
        return r;
    }

    /// Create a file-reference result. The file at path will be read by the runtime.
    [[nodiscard]] static PluginResult file(std::string name, std::string path) {
        PluginResult r;
        r.tag_  = Tag::File;
        r.name_ = std::move(name);
        r.path_ = std::move(path);
        return r;
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    [[nodiscard]] const std::string&          name() const noexcept { return name_; }
    [[nodiscard]] Tag                         tag()  const noexcept { return tag_; }
    [[nodiscard]] const std::vector<uint8_t>& data() const noexcept { return data_; }
    [[nodiscard]] const std::string&          path() const noexcept { return path_; }

    // -----------------------------------------------------------------------
    // nlohmann/json convenience overload (opt-in)
    // -----------------------------------------------------------------------
#ifdef MALBOX_HAS_NLOHMANN_JSON
    /// Create a JSON result by serialising an nlohmann::json-compatible type T.
    template <typename T>
    [[nodiscard]] static PluginResult json(std::string name, const T& value) {
        std::string serialised = nlohmann::json(value).dump();
        std::span<const uint8_t> sp{
            reinterpret_cast<const uint8_t*>(serialised.data()),
            serialised.size()
        };
        return json(std::move(name), sp);
    }
#endif

private:
    PluginResult() = default;

    Tag                  tag_{Tag::Json};
    std::string          name_;
    std::vector<uint8_t> data_;
    std::string          path_;

};

} // namespace malbox
