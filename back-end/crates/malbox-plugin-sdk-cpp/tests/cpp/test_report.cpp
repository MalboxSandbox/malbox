// Parity test: the JSON emitted by the C++ ReportBuilder for a fixed input
// must match the golden string embedded below (and the same golden is
// asserted against from the Rust side — see the
// `parity_golden_matches_cpp_output` test in
// `crates/malbox-plugin-sdk/src/types/report/mod.rs`).
//
// Any divergence between the two SDKs will break exactly one of these two
// tests, making drift immediately visible.

#include <malbox/report.hpp>
#include <cassert>
#include <cstdio>
#include <string>

using namespace malbox::report;

// Keep in exact sync with the Rust golden in
// `crates/malbox-plugin-sdk/src/types/report/mod.rs::parity_golden`.
static constexpr const char* GOLDEN_JSON =
    "{"
    "\"schema_version\":1,"
    "\"plugin\":{\"id\":\"yara\",\"version\":\"1.0.0\",\"display_name\":\"YARA Scanner\"},"
    "\"verdict\":{\"classification\":\"malicious\",\"score\":87,\"confidence\":\"high\",\"labels\":[\"trojan\"]},"
    "\"indicators\":[{\"kind\":\"sha256\",\"value\":\"abc\",\"context\":\"sample\"}],"
    "\"ttps\":[{\"id\":\"T1055\",\"name\":\"Process Injection\"}],"
    "\"artifacts\":[{\"result_name\":\"details.json\",\"kind\":\"other\"}],"
    "\"summary\":\"Matched 1 rule\","
    "\"sections\":[{\"id\":\"overview\",\"title\":\"Overview\",\"blocks\":["
        "{\"type\":\"heading\",\"level\":2,\"text\":\"Rules\"},"
        "{\"type\":\"markdown\",\"text\":\"1 match\"},"
        "{\"type\":\"divider\"}"
    "]}]"
    "}";

int main() {
    Report r = ReportBuilder("yara", "1.0.0")
        .display_name("YARA Scanner")
        .summary("Matched 1 rule")
        .verdict(Classification::Malicious, 87, Confidence::High)
        .labels({"trojan"})
        .indicator(Indicator("sha256", "abc").with_context("sample"))
        .ttp(Ttp("T1055", "Process Injection"))
        .artifact(ArtifactRef("details.json", "other"))
        .section("overview", "Overview", [](SectionBuilder& s) {
            s.heading(2, "Rules").markdown("1 match").divider();
        })
        .build();

    std::string json = to_json(r);

    if (json != GOLDEN_JSON) {
        std::fprintf(stderr, "C++ report JSON does not match golden.\n");
        std::fprintf(stderr, "expected: %s\n", GOLDEN_JSON);
        std::fprintf(stderr, "actual:   %s\n", json.c_str());
        return 1;
    }

    // Also verify the helper that packages the JSON into a PluginResult.
    malbox::PluginResult pr = into_plugin_result(r);
    assert(pr.tag() == malbox::PluginResult::Tag::Json);
    assert(pr.name() == "report");
    // Data size matches the serialized JSON length.
    assert(pr.data().size() == json.size());

    std::printf("test_report: parity with Rust golden OK\n");
    return 0;
}
