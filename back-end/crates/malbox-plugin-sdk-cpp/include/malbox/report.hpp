#pragma once

// Malbox plugin report envelope — C++ mirror of `malbox-plugin-sdk`'s Rust
// `Report` type (see `crates/malbox-plugin-sdk/src/types/report/mod.rs`).
//
// The JSON produced by `to_json(Report)` is byte-compatible with what the
// Rust SDK emits for an equivalent Report. The scheduler tags the resulting
// row with `role = 'report'` when `result_name == "report"` and the API
// deserializes it back into the same Rust struct. C++ plugins can construct
// reports fluently via `ReportBuilder`.
//
// The writer is self-contained — no nlohmann/json dependency. We only escape
// the JSON string characters required by RFC 8259.

#include <cstdint>
#include <cstdio>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>
#include <variant>
#include <vector>

#include "malbox_plugin.h"
#include <malbox/error.hpp>
#include <malbox/result.hpp>

namespace malbox::report {

// Keep in sync with `SCHEMA_VERSION` in
// `crates/malbox-plugin-sdk/src/types/report/mod.rs`.
constexpr uint32_t SCHEMA_VERSION = 1;
constexpr const char* REPORT_RESULT_NAME = "report";

// --------------------------------------------------------------------------
// Leaf types
// --------------------------------------------------------------------------

enum class Classification { Clean, Suspicious, Malicious, Unknown };
enum class Confidence { Low, Medium, High };
enum class CalloutLevel { Info, Success, Warn, Error };

inline const char* to_snake(Classification c) {
    switch (c) {
        case Classification::Clean:      return "clean";
        case Classification::Suspicious: return "suspicious";
        case Classification::Malicious:  return "malicious";
        case Classification::Unknown:    return "unknown";
    }
    return "unknown";
}
inline const char* to_snake(Confidence c) {
    switch (c) {
        case Confidence::Low:    return "low";
        case Confidence::Medium: return "medium";
        case Confidence::High:   return "high";
    }
    return "low";
}
inline const char* to_snake(CalloutLevel l) {
    switch (l) {
        case CalloutLevel::Info:    return "info";
        case CalloutLevel::Success: return "success";
        case CalloutLevel::Warn:    return "warn";
        case CalloutLevel::Error:   return "error";
    }
    return "info";
}

struct PluginInfo {
    std::string id;
    std::string version;
    std::optional<std::string> display_name;
};

struct Verdict {
    Classification classification = Classification::Unknown;
    std::optional<uint8_t> score;
    std::optional<Confidence> confidence;
    std::vector<std::string> labels;
};

struct Indicator {
    std::string kind;
    std::string value;
    std::optional<std::string> context;
    std::optional<std::string> first_seen;

    Indicator(std::string k, std::string v) : kind(std::move(k)), value(std::move(v)) {}
    Indicator& with_context(std::string c) { context = std::move(c); return *this; }
    Indicator& with_first_seen(std::string ts) { first_seen = std::move(ts); return *this; }
};

struct Ttp {
    std::string id;
    std::string name;
    std::optional<std::string> evidence;

    Ttp(std::string i, std::string n) : id(std::move(i)), name(std::move(n)) {}
    Ttp& with_evidence(std::string e) { evidence = std::move(e); return *this; }
};

struct ArtifactRef {
    std::string result_name;
    std::string kind;
    std::optional<std::string> description;

    ArtifactRef(std::string rn, std::string k) : result_name(std::move(rn)), kind(std::move(k)) {}
    ArtifactRef& with_description(std::string d) { description = std::move(d); return *this; }
};

struct KvPair {
    std::string key;
    std::string value;
    bool mono = false;
};

struct Column {
    std::string key;
    std::string label;
    std::string type = "string";
};

struct TreeNode;
struct TreeNode {
    std::string label;
    std::vector<TreeNode> children;
    /// Already-encoded JSON (or empty). Empty → omitted.
    std::string meta_json;
};

struct TimelineEvent {
    std::string ts;
    std::string label;
    std::optional<std::string> severity;
    std::string meta_json;
};

struct GraphNode {
    std::string id;
    std::string label;
    std::string meta_json;
};

struct GraphEdge {
    std::string from;
    std::string to;
    std::optional<std::string> label;
};

// --------------------------------------------------------------------------
// Block — std::variant over every Rust Block variant
// --------------------------------------------------------------------------

struct BlockMarkdown { std::string text; };
struct BlockCallout  { CalloutLevel level = CalloutLevel::Info; std::string text; };
struct BlockHeading  { uint8_t level = 2; std::string text; };
struct BlockDivider  {};
struct BlockKv       { std::vector<KvPair> pairs; };
struct BlockTable    {
    std::vector<Column> columns;
    /// Each row is an already-encoded JSON object (so the caller fully
    /// controls cell types: strings, numbers, booleans).
    std::vector<std::string> rows_json;
    bool sortable = true;
    bool searchable = false;
};
struct BlockCode     { std::string language; std::string text; };
struct BlockJson     { std::string data_json; bool collapsed = true; };
struct BlockHex      { std::string bytes_b64; uint64_t offset = 0; };
struct BlockImage    { std::string artifact; std::optional<std::string> caption; };
struct BlockDownload { std::string artifact; std::string label; };
struct BlockIocs     { std::vector<Indicator> items; };
struct BlockTtps     { std::vector<Ttp> items; };
struct BlockTree     { std::vector<TreeNode> nodes; };
struct BlockTimeline { std::vector<TimelineEvent> events; };
struct BlockGraph    { std::vector<GraphNode> nodes; std::vector<GraphEdge> edges; };

using Block = std::variant<
    BlockMarkdown, BlockCallout, BlockHeading, BlockDivider,
    BlockKv, BlockTable, BlockCode, BlockJson, BlockHex,
    BlockImage, BlockDownload, BlockIocs, BlockTtps,
    BlockTree, BlockTimeline, BlockGraph
>;

struct Section {
    std::string id;
    std::string title;
    std::vector<Block> blocks;
};

struct Report {
    uint32_t schema_version = SCHEMA_VERSION;
    PluginInfo plugin;
    std::optional<Verdict> verdict;
    std::vector<Indicator> indicators;
    std::vector<Ttp> ttps;
    std::vector<ArtifactRef> artifacts;
    std::optional<std::string> summary;
    std::vector<Section> sections;
    /// Already-encoded JSON (or empty). Empty → omitted.
    std::string raw_json;
};

// --------------------------------------------------------------------------
// JSON writer — self-contained, no external dependency
// --------------------------------------------------------------------------

namespace detail {

inline void escape_into(std::string& out, std::string_view s) {
    out.push_back('"');
    for (char c : s) {
        switch (c) {
            case '"':  out += "\\\""; break;
            case '\\': out += "\\\\"; break;
            case '\b': out += "\\b";  break;
            case '\f': out += "\\f";  break;
            case '\n': out += "\\n";  break;
            case '\r': out += "\\r";  break;
            case '\t': out += "\\t";  break;
            default:
                if (static_cast<unsigned char>(c) < 0x20) {
                    char buf[8];
                    std::snprintf(buf, sizeof(buf), "\\u%04x",
                                  static_cast<unsigned char>(c));
                    out += buf;
                } else {
                    out.push_back(c);
                }
        }
    }
    out.push_back('"');
}

struct Writer {
    std::string out;

    void raw(std::string_view s) { out.append(s); }
    void str(std::string_view s) { escape_into(out, s); }
    void number(uint64_t n) { out += std::to_string(n); }
    void number(int64_t n) { out += std::to_string(n); }
    void boolean(bool b) { raw(b ? "true" : "false"); }
    void null() { raw("null"); }
    void colon() { out.push_back(':'); }
    void comma() { out.push_back(','); }
    void begin_obj() { out.push_back('{'); }
    void end_obj() { out.push_back('}'); }
    void begin_arr() { out.push_back('['); }
    void end_arr() { out.push_back(']'); }
};

class Sep {
public:
    explicit Sep(Writer& w) : w_(w) {}
    void next() {
        if (!first_) w_.comma();
        first_ = false;
    }
    void key(std::string_view k) { next(); w_.str(k); w_.colon(); }
private:
    Writer& w_;
    bool first_ = true;
};

inline void write_indicator(Writer& w, const Indicator& i) {
    w.begin_obj();
    Sep s(w);
    s.key("kind");  w.str(i.kind);
    s.key("value"); w.str(i.value);
    if (i.context)    { s.key("context");    w.str(*i.context); }
    if (i.first_seen) { s.key("first_seen"); w.str(*i.first_seen); }
    w.end_obj();
}

inline void write_ttp(Writer& w, const Ttp& t) {
    w.begin_obj();
    Sep s(w);
    s.key("id");   w.str(t.id);
    s.key("name"); w.str(t.name);
    if (t.evidence) { s.key("evidence"); w.str(*t.evidence); }
    w.end_obj();
}

inline void write_artifact_ref(Writer& w, const ArtifactRef& a) {
    w.begin_obj();
    Sep s(w);
    s.key("result_name"); w.str(a.result_name);
    s.key("kind");        w.str(a.kind);
    if (a.description) { s.key("description"); w.str(*a.description); }
    w.end_obj();
}

inline void write_kv_pair(Writer& w, const KvPair& p) {
    w.begin_obj();
    Sep s(w);
    s.key("key");   w.str(p.key);
    s.key("value"); w.str(p.value);
    if (p.mono) { s.key("mono"); w.boolean(true); }
    w.end_obj();
}

inline void write_column(Writer& w, const Column& c) {
    w.begin_obj();
    Sep s(w);
    s.key("key");   w.str(c.key);
    s.key("label"); w.str(c.label);
    s.key("type");  w.str(c.type);
    w.end_obj();
}

inline void write_tree_node(Writer& w, const TreeNode& n) {
    w.begin_obj();
    Sep s(w);
    s.key("label"); w.str(n.label);
    if (!n.children.empty()) {
        s.key("children"); w.begin_arr();
        Sep sc(w);
        for (const auto& child : n.children) { sc.next(); write_tree_node(w, child); }
        w.end_arr();
    }
    if (!n.meta_json.empty()) { s.key("meta"); w.raw(n.meta_json); }
    w.end_obj();
}

inline void write_timeline_event(Writer& w, const TimelineEvent& e) {
    w.begin_obj();
    Sep s(w);
    s.key("ts");    w.str(e.ts);
    s.key("label"); w.str(e.label);
    if (e.severity) { s.key("severity"); w.str(*e.severity); }
    if (!e.meta_json.empty()) { s.key("meta"); w.raw(e.meta_json); }
    w.end_obj();
}

inline void write_graph_node(Writer& w, const GraphNode& n) {
    w.begin_obj();
    Sep s(w);
    s.key("id");    w.str(n.id);
    s.key("label"); w.str(n.label);
    if (!n.meta_json.empty()) { s.key("meta"); w.raw(n.meta_json); }
    w.end_obj();
}

inline void write_graph_edge(Writer& w, const GraphEdge& e) {
    w.begin_obj();
    Sep s(w);
    s.key("from"); w.str(e.from);
    s.key("to");   w.str(e.to);
    if (e.label) { s.key("label"); w.str(*e.label); }
    w.end_obj();
}

template <typename T, typename F>
inline void write_arr(Writer& w, const std::vector<T>& xs, F&& emit_each) {
    w.begin_arr();
    Sep s(w);
    for (const auto& x : xs) { s.next(); emit_each(w, x); }
    w.end_arr();
}

inline void write_block(Writer& w, const Block& block) {
    w.begin_obj();
    Sep s(w);
    std::visit([&](auto const& b) {
        using T = std::decay_t<decltype(b)>;
        if constexpr (std::is_same_v<T, BlockMarkdown>) {
            s.key("type"); w.str("markdown");
            s.key("text"); w.str(b.text);
        } else if constexpr (std::is_same_v<T, BlockCallout>) {
            s.key("type");  w.str("callout");
            s.key("level"); w.str(to_snake(b.level));
            s.key("text");  w.str(b.text);
        } else if constexpr (std::is_same_v<T, BlockHeading>) {
            s.key("type");  w.str("heading");
            s.key("level"); w.number(static_cast<uint64_t>(b.level));
            s.key("text");  w.str(b.text);
        } else if constexpr (std::is_same_v<T, BlockDivider>) {
            s.key("type"); w.str("divider");
        } else if constexpr (std::is_same_v<T, BlockKv>) {
            s.key("type"); w.str("kv");
            s.key("pairs"); write_arr(w, b.pairs, write_kv_pair);
        } else if constexpr (std::is_same_v<T, BlockTable>) {
            s.key("type");    w.str("table");
            s.key("columns"); write_arr(w, b.columns, write_column);
            s.key("rows");    w.begin_arr();
            Sep sr(w);
            for (const auto& row_json : b.rows_json) { sr.next(); w.raw(row_json); }
            w.end_arr();
            if (b.sortable)   { s.key("sortable");   w.boolean(true); }
            if (b.searchable) { s.key("searchable"); w.boolean(true); }
        } else if constexpr (std::is_same_v<T, BlockCode>) {
            s.key("type");     w.str("code");
            s.key("language"); w.str(b.language);
            s.key("text");     w.str(b.text);
        } else if constexpr (std::is_same_v<T, BlockJson>) {
            s.key("type"); w.str("json");
            s.key("data"); w.raw(b.data_json.empty() ? "null" : b.data_json);
            if (b.collapsed) { s.key("collapsed"); w.boolean(true); }
        } else if constexpr (std::is_same_v<T, BlockHex>) {
            s.key("type");      w.str("hex");
            s.key("bytes_b64"); w.str(b.bytes_b64);
            if (b.offset != 0) { s.key("offset"); w.number(b.offset); }
        } else if constexpr (std::is_same_v<T, BlockImage>) {
            s.key("type");     w.str("image");
            s.key("artifact"); w.str(b.artifact);
            if (b.caption) { s.key("caption"); w.str(*b.caption); }
        } else if constexpr (std::is_same_v<T, BlockDownload>) {
            s.key("type");     w.str("download");
            s.key("artifact"); w.str(b.artifact);
            s.key("label");    w.str(b.label);
        } else if constexpr (std::is_same_v<T, BlockIocs>) {
            s.key("type");  w.str("iocs");
            s.key("items"); write_arr(w, b.items, write_indicator);
        } else if constexpr (std::is_same_v<T, BlockTtps>) {
            s.key("type");  w.str("ttps");
            s.key("items"); write_arr(w, b.items, write_ttp);
        } else if constexpr (std::is_same_v<T, BlockTree>) {
            s.key("type");  w.str("tree");
            s.key("nodes"); write_arr(w, b.nodes, write_tree_node);
        } else if constexpr (std::is_same_v<T, BlockTimeline>) {
            s.key("type");   w.str("timeline");
            s.key("events"); write_arr(w, b.events, write_timeline_event);
        } else if constexpr (std::is_same_v<T, BlockGraph>) {
            s.key("type");  w.str("graph");
            s.key("nodes"); write_arr(w, b.nodes, write_graph_node);
            s.key("edges"); write_arr(w, b.edges, write_graph_edge);
        }
    }, block);
    w.end_obj();
}

inline void write_section(Writer& w, const Section& sec) {
    w.begin_obj();
    Sep s(w);
    s.key("id");    w.str(sec.id);
    s.key("title"); w.str(sec.title);
    if (!sec.blocks.empty()) {
        s.key("blocks"); write_arr(w, sec.blocks, write_block);
    }
    w.end_obj();
}

inline void write_plugin_info(Writer& w, const PluginInfo& p) {
    w.begin_obj();
    Sep s(w);
    s.key("id");      w.str(p.id);
    s.key("version"); w.str(p.version);
    if (p.display_name) { s.key("display_name"); w.str(*p.display_name); }
    w.end_obj();
}

inline void write_verdict(Writer& w, const Verdict& v) {
    w.begin_obj();
    Sep s(w);
    s.key("classification"); w.str(to_snake(v.classification));
    if (v.score)      { s.key("score");      w.number(static_cast<uint64_t>(*v.score)); }
    if (v.confidence) { s.key("confidence"); w.str(to_snake(*v.confidence)); }
    if (!v.labels.empty()) {
        s.key("labels"); w.begin_arr();
        Sep sl(w);
        for (const auto& l : v.labels) { sl.next(); w.str(l); }
        w.end_arr();
    }
    w.end_obj();
}

} // namespace detail

inline std::string to_json(const Report& r) {
    detail::Writer w;
    w.begin_obj();
    detail::Sep s(w);
    s.key("schema_version"); w.number(static_cast<uint64_t>(r.schema_version));
    s.key("plugin"); detail::write_plugin_info(w, r.plugin);
    if (r.verdict) { s.key("verdict"); detail::write_verdict(w, *r.verdict); }
    if (!r.indicators.empty()) { s.key("indicators"); detail::write_arr(w, r.indicators, detail::write_indicator); }
    if (!r.ttps.empty())       { s.key("ttps");       detail::write_arr(w, r.ttps, detail::write_ttp); }
    if (!r.artifacts.empty())  { s.key("artifacts");  detail::write_arr(w, r.artifacts, detail::write_artifact_ref); }
    if (r.summary) { s.key("summary"); w.str(*r.summary); }
    if (!r.sections.empty())   { s.key("sections");   detail::write_arr(w, r.sections, detail::write_section); }
    if (!r.raw_json.empty())   { s.key("raw");        w.raw(r.raw_json); }
    w.end_obj();
    return std::move(w.out);
}

/// Serialize this report into a `PluginResult::json("report", ...)` ready to
/// be pushed via `malbox::Context::push_result`.
inline PluginResult into_plugin_result(const Report& r) {
    std::string s = to_json(r);
    std::span<const uint8_t> sp{
        reinterpret_cast<const uint8_t*>(s.data()),
        s.size()
    };
    return PluginResult::json(REPORT_RESULT_NAME, sp);
}

// --------------------------------------------------------------------------
// Fluent builders
// --------------------------------------------------------------------------

class SectionBuilder {
public:
    SectionBuilder(std::string id, std::string title) {
        section_.id = std::move(id);
        section_.title = std::move(title);
    }

    SectionBuilder& block(Block b)                  { section_.blocks.push_back(std::move(b)); return *this; }
    SectionBuilder& markdown(std::string t)         { return block(BlockMarkdown{std::move(t)}); }
    SectionBuilder& callout(CalloutLevel lvl, std::string t) { return block(BlockCallout{lvl, std::move(t)}); }
    SectionBuilder& heading(uint8_t level, std::string t)    { return block(BlockHeading{level, std::move(t)}); }
    SectionBuilder& divider()                       { return block(BlockDivider{}); }
    SectionBuilder& kv(std::vector<KvPair> pairs)   { return block(BlockKv{std::move(pairs)}); }
    SectionBuilder& table(std::vector<Column> cols, std::vector<std::string> rows_json, bool sortable=true, bool searchable=false) {
        return block(BlockTable{std::move(cols), std::move(rows_json), sortable, searchable});
    }
    SectionBuilder& code(std::string lang, std::string text) { return block(BlockCode{std::move(lang), std::move(text)}); }
    SectionBuilder& json(std::string data_json, bool collapsed=true) {
        return block(BlockJson{std::move(data_json), collapsed});
    }
    SectionBuilder& hex(std::string bytes_b64, uint64_t offset=0) { return block(BlockHex{std::move(bytes_b64), offset}); }
    SectionBuilder& image(std::string artifact, std::optional<std::string> caption={}) {
        return block(BlockImage{std::move(artifact), std::move(caption)});
    }
    SectionBuilder& download(std::string artifact, std::string label) {
        return block(BlockDownload{std::move(artifact), std::move(label)});
    }
    SectionBuilder& iocs(std::vector<Indicator> items) { return block(BlockIocs{std::move(items)}); }
    SectionBuilder& ttps(std::vector<Ttp> items)       { return block(BlockTtps{std::move(items)}); }
    SectionBuilder& tree(std::vector<TreeNode> nodes)  { return block(BlockTree{std::move(nodes)}); }
    SectionBuilder& timeline(std::vector<TimelineEvent> events) { return block(BlockTimeline{std::move(events)}); }
    SectionBuilder& graph(std::vector<GraphNode> nodes, std::vector<GraphEdge> edges) {
        return block(BlockGraph{std::move(nodes), std::move(edges)});
    }

    Section build() { return std::move(section_); }

private:
    Section section_;
};

class ReportBuilder {
public:
    ReportBuilder(std::string plugin_id, std::string plugin_version) {
        report_.plugin.id = std::move(plugin_id);
        report_.plugin.version = std::move(plugin_version);
    }

    ReportBuilder& display_name(std::string name) { report_.plugin.display_name = std::move(name); return *this; }
    ReportBuilder& summary(std::string s)         { report_.summary = std::move(s); return *this; }

    ReportBuilder& verdict(Classification c, std::optional<uint8_t> score={}, std::optional<Confidence> conf={}) {
        if (!report_.verdict) report_.verdict.emplace();
        report_.verdict->classification = c;
        report_.verdict->score = score;
        report_.verdict->confidence = conf;
        return *this;
    }
    ReportBuilder& labels(std::vector<std::string> ls) {
        if (!report_.verdict) report_.verdict.emplace();
        for (auto& l : ls) report_.verdict->labels.push_back(std::move(l));
        return *this;
    }

    ReportBuilder& indicator(Indicator i) { report_.indicators.push_back(std::move(i)); return *this; }
    ReportBuilder& ttp(Ttp t)             { report_.ttps.push_back(std::move(t)); return *this; }
    ReportBuilder& artifact(ArtifactRef a){ report_.artifacts.push_back(std::move(a)); return *this; }

    ReportBuilder& section(SectionBuilder sb) {
        report_.sections.push_back(sb.build());
        return *this;
    }
    template <typename F>
    ReportBuilder& section(std::string id, std::string title, F&& fn) {
        SectionBuilder sb(std::move(id), std::move(title));
        std::forward<F>(fn)(sb);
        report_.sections.push_back(sb.build());
        return *this;
    }

    /// Attach the plugin's native JSON as the `raw` escape hatch.
    /// `pre_encoded_json` must be a valid JSON document.
    ReportBuilder& raw(std::string pre_encoded_json) { report_.raw_json = std::move(pre_encoded_json); return *this; }

    Report build() { return std::move(report_); }

private:
    Report report_;
};

} // namespace malbox::report
