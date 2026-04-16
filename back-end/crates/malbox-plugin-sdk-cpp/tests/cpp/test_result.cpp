#include <malbox/plugin.hpp>
#include <cassert>
#include <cstdio>
#include <vector>

int main() {
    // Bytes result
    std::vector<uint8_t> data = {0x01, 0x02};
    auto bytes_result = malbox::PluginResult::bytes("raw", std::span(data));
    assert(bytes_result.name() == "raw");
    assert(bytes_result.tag() == malbox::PluginResult::Tag::Bytes);

    // Json result (raw bytes)
    std::string json = R"({"key": "value"})";
    auto json_span = std::span<const uint8_t>(
        reinterpret_cast<const uint8_t*>(json.data()), json.size());
    auto json_result = malbox::PluginResult::json("data", json_span);
    assert(json_result.name() == "data");
    assert(json_result.tag() == malbox::PluginResult::Tag::Json);

    // File result
    auto file_result = malbox::PluginResult::file("report", "/tmp/report.txt");
    assert(file_result.name() == "report");
    assert(file_result.tag() == malbox::PluginResult::Tag::File);

    std::printf("test_result: all assertions passed\n");
    return 0;
}
