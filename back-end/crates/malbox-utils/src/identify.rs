//! File identification: maps libmagic output to Malbox's AssemblyLine-style
//! artifact type taxonomy.

use std::sync::Once;

use magic::cookie::{DatabasePaths, Flags, Load};
use tracing::warn;

/// Result of identifying a file: artifact type plus raw libmagic output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identification {
    /// Malbox artifact type, e.g. `executable/windows/pe32`.
    pub artifact_type: String,
    /// MIME type reported by libmagic.
    pub mime: String,
    /// Human-readable libmagic description.
    pub description: String,
}

thread_local! {
    static DESCRIPTION_COOKIE: Option<magic::Cookie<Load>> = load_cookie(Flags::default());
    static MIME_COOKIE: Option<magic::Cookie<Load>> = load_cookie(Flags::MIME_TYPE);
}

static WARN_ONCE: Once = Once::new();

fn warn_once(msg: &str) {
    WARN_ONCE.call_once(|| warn!("{msg}"));
}

fn load_cookie(flags: Flags) -> Option<magic::Cookie<Load>> {
    let cookie = match magic::Cookie::open(flags) {
        Ok(c) => c,
        Err(e) => {
            warn_once(&format!("failed to open libmagic cookie: {e}"));
            return None;
        }
    };
    match cookie.load(&DatabasePaths::default()) {
        Ok(c) => Some(c),
        Err(e) => {
            warn_once(&format!("failed to load libmagic database: {e}"));
            None
        }
    }
}

fn unknown() -> Identification {
    Identification {
        artifact_type: "unknown".to_string(),
        mime: "application/octet-stream".to_string(),
        description: "unknown".to_string(),
    }
}

/// Identify `data`. Blocking (libmagic); call inside `spawn_blocking`.
pub fn identify(data: &[u8], file_name: Option<&str>) -> Identification {
    if data.is_empty() {
        return Identification {
            artifact_type: "empty".to_string(),
            mime: "application/x-empty".to_string(),
            description: "empty".to_string(),
        };
    }
    let description = DESCRIPTION_COOKIE.with(|c| c.as_ref().map(|c| c.buffer(data)));
    let mime = MIME_COOKIE.with(|c| c.as_ref().map(|c| c.buffer(data)));
    let (description, mime) = match (description, mime) {
        (Some(Ok(d)), Some(Ok(m))) => (d, m),
        (Some(Err(e)), _) | (_, Some(Err(e))) => {
            warn_once(&format!("libmagic failed to analyse buffer: {e}"));
            return unknown();
        }
        _ => return unknown(),
    };
    let ext = file_name.and_then(extension);
    let artifact_type = classify(
        &mime.to_lowercase(),
        &description.to_lowercase(),
        ext.as_deref(),
    );
    let artifact_type = if artifact_type == IMAGE_MARKER {
        let m = mime.to_lowercase();
        format!("image/{}", m.strip_prefix("image/").unwrap_or_default())
    } else {
        artifact_type.to_string()
    };
    Identification {
        artifact_type,
        mime,
        description,
    }
}

fn extension(name: &str) -> Option<String> {
    std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
}

/// Returned by `classify` for rule 42; `identify` appends the MIME subtype.
const IMAGE_MARKER: &str = "image/";

/// Mapping rules over lowercased MIME (`m`), description (`d`) and extension
/// (`e`). First match wins. Empty input is handled by `identify` before this.
#[expect(
    clippy::too_many_lines,
    reason = "flat, ordered rule table; splitting would obscure first-match-wins precedence"
)]
fn classify(m: &str, d: &str, e: Option<&str>) -> &'static str {
    let e = e.unwrap_or("");
    if d.contains("mono/.net assembly") {
        return "executable/windows/dotnet";
    }
    if d.starts_with("pe32+") && d.contains("(dll)") {
        return "executable/windows/dll64";
    }
    if d.starts_with("pe32 ") && d.contains("(dll)") {
        return "executable/windows/dll32";
    }
    if d.starts_with("pe32+") {
        return "executable/windows/pe64";
    }
    if d.starts_with("pe32") || d.starts_with("ms-dos executable") {
        return "executable/windows/pe32";
    }
    if d.starts_with("elf 64-bit") {
        return "executable/linux/elf64";
    }
    if d.starts_with("elf 32-bit") {
        return "executable/linux/elf32";
    }
    if d.starts_with("mach-o") {
        return "executable/macos/macho";
    }
    if m == "application/x-msi" || (d.contains("composite document file") && e == "msi") {
        return "installer/windows/msi";
    }
    if m == "application/java-archive" || e == "jar" {
        return "java/jar";
    }
    if e == "apk" && m == "application/zip" {
        return "android/apk";
    }
    match m {
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        | "application/msword" => return "document/office/word",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        | "application/vnd.ms-excel" => return "document/office/excel",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        | "application/vnd.ms-powerpoint" => return "document/office/powerpoint",
        "text/rtf" | "application/rtf" => return "document/office/rtf",
        _ => {}
    }
    if e == "one" {
        return "document/office/onenote";
    }
    if m == "application/x-ole-storage" || d.contains("composite document file") {
        return "document/office/ole";
    }
    if m == "application/pdf" {
        return "document/pdf";
    }
    if m == "application/x-ms-shortcut" || d.starts_with("ms windows shortcut") {
        return "shortcut/windows";
    }
    match m {
        "application/zip" => return "archive/zip",
        "application/x-7z-compressed" => return "archive/7-zip",
        "application/vnd.rar" => return "archive/rar",
        "application/gzip" => return "archive/gzip",
        "application/x-bzip2" => return "archive/bzip2",
        "application/x-xz" => return "archive/xz",
        "application/x-tar" => return "archive/tar",
        "application/vnd.ms-cab-compressed" => return "archive/cab",
        "application/x-iso9660-image" => return "archive/iso",
        "application/vnd.tcpdump.pcap" => return "network/pcap",
        _ => {}
    }
    if m.starts_with("application/x-rar") {
        return "archive/rar";
    }
    if d.starts_with("pcap capture file") {
        return "network/pcap";
    }
    if d.starts_with("pcapng capture file") {
        return "network/pcapng";
    }
    match e {
        "ps1" | "psm1" => return "code/powershell",
        "js" | "jse" => return "code/javascript",
        "vbs" | "vbe" => return "code/vbs",
        "bat" | "cmd" => return "code/batch",
        "hta" => return "code/hta",
        _ => {}
    }
    if m == "text/x-python" || e == "py" {
        return "code/python";
    }
    if m == "text/x-shellscript" || e == "sh" {
        return "code/shell";
    }
    match m {
        "text/html" => return "code/html",
        "application/json" => return "text/json",
        "text/xml" | "application/xml" => return "text/xml",
        _ => {}
    }
    if m.starts_with("image/") {
        return IMAGE_MARKER;
    }
    if m.starts_with("text/") {
        return "text/plain";
    }
    "unknown"
}

/// Canonical file extension (with leading dot) for an artifact type. The
/// Office types depend on whether the MIME type is OOXML.
pub fn canonical_extension(artifact_type: &str, mime: &str) -> Option<&'static str> {
    let ooxml = mime
        .to_lowercase()
        .starts_with("application/vnd.openxmlformats-officedocument.");
    Some(match artifact_type {
        "executable/windows/pe32" | "executable/windows/pe64" | "executable/windows/dotnet" => {
            ".exe"
        }
        "executable/windows/dll32" | "executable/windows/dll64" => ".dll",
        "installer/windows/msi" => ".msi",
        "code/powershell" => ".ps1",
        "code/javascript" => ".js",
        "code/vbs" => ".vbs",
        "code/batch" => ".bat",
        "code/hta" => ".hta",
        "code/python" => ".py",
        "code/shell" => ".sh",
        "document/office/word" if ooxml => ".docx",
        "document/office/word" => ".doc",
        "document/office/excel" if ooxml => ".xlsx",
        "document/office/excel" => ".xls",
        "document/office/powerpoint" if ooxml => ".pptx",
        "document/office/powerpoint" => ".ppt",
        "document/office/rtf" => ".rtf",
        "document/office/onenote" => ".one",
        "document/pdf" => ".pdf",
        "shortcut/windows" => ".lnk",
        "java/jar" => ".jar",
        "android/apk" => ".apk",
        "archive/zip" => ".zip",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[expect(clippy::too_many_lines, reason = "test case table")]
    fn classify_rules() {
        // (row, mime, description, ext, empty, expected)
        type Case = (
            u32,
            &'static str,
            &'static str,
            Option<&'static str>,
            bool,
            &'static str,
        );
        let cases: &[Case] = &[
            (1, "", "", None, true, "empty"),
            (
                2,
                "application/x-dosexec",
                "pe32 executable (gui) intel 80386 mono/.net assembly",
                None,
                false,
                "executable/windows/dotnet",
            ),
            (
                3,
                "application/x-dosexec",
                "pe32+ executable (dll) (console) x86-64",
                None,
                false,
                "executable/windows/dll64",
            ),
            (
                4,
                "application/x-dosexec",
                "pe32 executable (dll) (gui) intel 80386",
                None,
                false,
                "executable/windows/dll32",
            ),
            (
                5,
                "application/x-dosexec",
                "pe32+ executable (gui) x86-64",
                None,
                false,
                "executable/windows/pe64",
            ),
            (
                6,
                "application/x-dosexec",
                "ms-dos executable",
                None,
                false,
                "executable/windows/pe32",
            ),
            (
                7,
                "application/x-executable",
                "elf 64-bit lsb executable, x86-64",
                None,
                false,
                "executable/linux/elf64",
            ),
            (
                8,
                "application/x-executable",
                "elf 32-bit lsb executable, intel 80386",
                None,
                false,
                "executable/linux/elf32",
            ),
            (
                9,
                "application/x-mach-binary",
                "mach-o 64-bit x86_64 executable",
                None,
                false,
                "executable/macos/macho",
            ),
            (
                10,
                "application/x-ole-storage",
                "composite document file v2 document",
                Some("msi"),
                false,
                "installer/windows/msi",
            ),
            (
                11,
                "application/zip",
                "zip archive data",
                Some("jar"),
                false,
                "java/jar",
            ),
            (
                12,
                "application/zip",
                "zip archive data",
                Some("apk"),
                false,
                "android/apk",
            ),
            (
                13,
                "application/msword",
                "composite document file v2 document",
                None,
                false,
                "document/office/word",
            ),
            (
                14,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                "microsoft excel 2007+",
                None,
                false,
                "document/office/excel",
            ),
            (
                15,
                "application/vnd.ms-powerpoint",
                "composite document file v2 document",
                None,
                false,
                "document/office/powerpoint",
            ),
            (
                16,
                "text/rtf",
                "rich text format data",
                None,
                false,
                "document/office/rtf",
            ),
            (
                17,
                "application/octet-stream",
                "data",
                Some("one"),
                false,
                "document/office/onenote",
            ),
            (
                18,
                "application/x-ole-storage",
                "composite document file v2 document",
                None,
                false,
                "document/office/ole",
            ),
            (
                19,
                "application/pdf",
                "pdf document, version 1.7",
                None,
                false,
                "document/pdf",
            ),
            (
                20,
                "application/octet-stream",
                "ms windows shortcut, item id list present",
                None,
                false,
                "shortcut/windows",
            ),
            (
                21,
                "application/zip",
                "zip archive data",
                None,
                false,
                "archive/zip",
            ),
            (
                22,
                "application/x-7z-compressed",
                "7-zip archive data",
                None,
                false,
                "archive/7-zip",
            ),
            (
                23,
                "application/x-rar-compressed",
                "rar archive data",
                None,
                false,
                "archive/rar",
            ),
            (
                24,
                "application/gzip",
                "gzip compressed data",
                None,
                false,
                "archive/gzip",
            ),
            (
                25,
                "application/x-bzip2",
                "bzip2 compressed data",
                None,
                false,
                "archive/bzip2",
            ),
            (
                26,
                "application/x-xz",
                "xz compressed data",
                None,
                false,
                "archive/xz",
            ),
            (
                27,
                "application/x-tar",
                "posix tar archive",
                None,
                false,
                "archive/tar",
            ),
            (
                28,
                "application/vnd.ms-cab-compressed",
                "microsoft cabinet archive data",
                None,
                false,
                "archive/cab",
            ),
            (
                29,
                "application/x-iso9660-image",
                "iso 9660 cd-rom filesystem data",
                None,
                false,
                "archive/iso",
            ),
            (
                30,
                "application/octet-stream",
                "pcap capture file, microsecond ts",
                None,
                false,
                "network/pcap",
            ),
            (
                31,
                "application/octet-stream",
                "pcapng capture file - version 1.0",
                None,
                false,
                "network/pcapng",
            ),
            (
                32,
                "text/plain",
                "ascii text",
                Some("ps1"),
                false,
                "code/powershell",
            ),
            (
                33,
                "text/plain",
                "ascii text",
                Some("js"),
                false,
                "code/javascript",
            ),
            (
                34,
                "text/plain",
                "ascii text",
                Some("vbs"),
                false,
                "code/vbs",
            ),
            (
                35,
                "text/plain",
                "ascii text",
                Some("bat"),
                false,
                "code/batch",
            ),
            (
                36,
                "text/html",
                "html document",
                Some("hta"),
                false,
                "code/hta",
            ),
            (
                37,
                "text/x-python",
                "python script",
                None,
                false,
                "code/python",
            ),
            (
                38,
                "text/x-shellscript",
                "posix shell script",
                None,
                false,
                "code/shell",
            ),
            (39, "text/html", "html document", None, false, "code/html"),
            (
                40,
                "application/json",
                "json text data",
                None,
                false,
                "text/json",
            ),
            (41, "text/xml", "xml 1.0 document", None, false, "text/xml"),
            (42, "image/png", "png image data", None, false, IMAGE_MARKER),
            (43, "text/plain", "ascii text", None, false, "text/plain"),
            (
                44,
                "application/octet-stream",
                "data",
                None,
                false,
                "unknown",
            ),
        ];
        assert_eq!(cases.len(), 44);
        for (row, m, d, e, empty, want) in cases {
            let got = if *empty { "empty" } else { classify(m, d, *e) };
            assert_eq!(got, *want, "row {row}");
        }
    }

    #[test]
    fn ooxml_beats_zip() {
        assert_eq!(
            classify(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                "microsoft word 2007+",
                Some("docx"),
            ),
            "document/office/word"
        );
    }

    #[test]
    fn canonical_extensions() {
        let ooxml = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
        let cases: &[(&str, &str, Option<&str>)] = &[
            ("executable/windows/pe32", "", Some(".exe")),
            ("executable/windows/pe64", "", Some(".exe")),
            ("executable/windows/dotnet", "", Some(".exe")),
            ("executable/windows/dll32", "", Some(".dll")),
            ("executable/windows/dll64", "", Some(".dll")),
            ("installer/windows/msi", "", Some(".msi")),
            ("code/powershell", "", Some(".ps1")),
            ("code/javascript", "", Some(".js")),
            ("code/vbs", "", Some(".vbs")),
            ("code/batch", "", Some(".bat")),
            ("code/hta", "", Some(".hta")),
            ("code/python", "", Some(".py")),
            ("code/shell", "", Some(".sh")),
            ("document/office/word", ooxml, Some(".docx")),
            ("document/office/word", "application/msword", Some(".doc")),
            (
                "document/office/excel",
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                Some(".xlsx"),
            ),
            (
                "document/office/excel",
                "application/vnd.ms-excel",
                Some(".xls"),
            ),
            (
                "document/office/powerpoint",
                "application/vnd.openxmlformats-officedocument.presentationml.presentation",
                Some(".pptx"),
            ),
            (
                "document/office/powerpoint",
                "application/vnd.ms-powerpoint",
                Some(".ppt"),
            ),
            ("document/office/rtf", "", Some(".rtf")),
            ("document/office/onenote", "", Some(".one")),
            ("document/pdf", "", Some(".pdf")),
            ("shortcut/windows", "", Some(".lnk")),
            ("java/jar", "", Some(".jar")),
            ("android/apk", "", Some(".apk")),
            ("archive/zip", "", Some(".zip")),
            ("archive/7-zip", "", None),
            ("unknown", "", None),
        ];
        for (t, m, want) in cases {
            assert_eq!(canonical_extension(t, m), *want, "{t}");
        }
    }

    #[test]
    fn empty_input() {
        assert_eq!(identify(&[], Some("a.txt")).artifact_type, "empty");
    }
}
