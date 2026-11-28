//! Launch packages: how a detonated sample is started inside the guest.

use malbox_database::repositories::machinery::MachinePlatform;
use malbox_utils::identify::canonical_extension;

/// Program and arguments that start a sample inside the guest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LaunchCommand {
    /// Executable to run in the guest.
    pub(crate) command: String,
    /// Arguments passed to `command`.
    pub(crate) args: Vec<String>,
}

/// Every package name. Windows accepts the first eight, Linux
/// `elf, shell, python, open`.
pub(crate) const PACKAGES: &[&str] = &[
    "exe",
    "dll",
    "msi",
    "powershell",
    "wscript",
    "batch",
    "hta",
    "open",
    "elf",
    "shell",
    "python",
];

const WINDOWS_PACKAGES: &[&str] = &[
    "exe",
    "dll",
    "msi",
    "powershell",
    "wscript",
    "batch",
    "hta",
    "open",
];
const LINUX_PACKAGES: &[&str] = &["elf", "shell", "python", "open"];

/// Longest guest file name (before the extension is appended).
const MAX_NAME_CHARS: usize = 100;

fn cmd(command: &str, args: &[&str]) -> LaunchCommand {
    LaunchCommand {
        command: command.to_string(),
        args: args.iter().map(ToString::to_string).collect(),
    }
}

fn default_package(artifact_type: &str, platform: &MachinePlatform) -> &'static str {
    match platform {
        MachinePlatform::Windows => match artifact_type {
            "executable/windows/pe32" | "executable/windows/pe64" | "executable/windows/dotnet" => {
                "exe"
            }
            "executable/windows/dll32" | "executable/windows/dll64" => "dll",
            "installer/windows/msi" => "msi",
            "code/powershell" => "powershell",
            "code/javascript" | "code/vbs" => "wscript",
            "code/batch" => "batch",
            "code/hta" => "hta",
            _ => "open",
        },
        MachinePlatform::Linux => match artifact_type {
            t if t.starts_with("executable/linux/") => "elf",
            "code/shell" => "shell",
            "code/python" => "python",
            _ => "open",
        },
    }
}

/// The command that starts `target` (a file name in the guest sample dir).
///
/// # Errors
///
/// Returns a message when `package` is not a package supported on `platform`.
pub(crate) fn launch_command(
    artifact_type: &str,
    platform: &MachinePlatform,
    target: &str,
    package: Option<&str>,
    dll_function: Option<&str>,
) -> Result<LaunchCommand, String> {
    let allowed = match platform {
        MachinePlatform::Windows => WINDOWS_PACKAGES,
        MachinePlatform::Linux => LINUX_PACKAGES,
    };
    let package = match package {
        Some(p) if allowed.contains(&p) => p,
        Some(p) => return Err(format!("unknown package {p} for {}", platform.as_str())),
        None => default_package(artifact_type, platform),
    };
    let t = target;
    Ok(match (platform, package) {
        (MachinePlatform::Windows, "exe" | "batch") => cmd("cmd.exe", &["/c", t]),
        (MachinePlatform::Windows, "dll") => {
            let function = dll_function.unwrap_or("DllMain");
            cmd("rundll32.exe", &[&format!("{t},{function}")])
        }
        (MachinePlatform::Windows, "msi") => cmd("msiexec.exe", &["/i", t, "/quiet"]),
        (MachinePlatform::Windows, "powershell") => cmd(
            "powershell.exe",
            &["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", t],
        ),
        (MachinePlatform::Windows, "wscript") => cmd("wscript.exe", &[t]),
        (MachinePlatform::Windows, "hta") => cmd("mshta.exe", &[t]),
        (MachinePlatform::Windows, _) => cmd("cmd.exe", &["/c", "start", "", t]),
        (MachinePlatform::Linux, "elf") => cmd("sh", &["-c", &format!("chmod +x ./{t} && ./{t}")]),
        (MachinePlatform::Linux, "shell") => cmd("sh", &[t]),
        (MachinePlatform::Linux, "python") => cmd("python3", &[t]),
        (MachinePlatform::Linux, _) => cmd("xdg-open", &[t]),
    })
}

/// Replace characters outside `[A-Za-z0-9._-]` with `_` and truncate to 100
/// characters. An empty name stays empty.
pub fn sanitize_file_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(MAX_NAME_CHARS)
        .collect()
}

/// File name the sample gets inside the guest: the sanitised artifact name
/// (or the first 16 hex chars of its SHA-256), plus the canonical extension
/// of its type when missing.
pub(crate) fn guest_file_name(
    name: Option<&str>,
    sha256: &str,
    artifact_type: &str,
    mime: &str,
) -> String {
    let mut out = name.map(sanitize_file_name).unwrap_or_default();
    if out.is_empty() {
        out = sha256.chars().take(16).collect();
    }
    if let Some(ext) = canonical_extension(artifact_type, mime)
        && !out.to_ascii_lowercase().ends_with(ext)
    {
        out.push_str(ext);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(t: &str, package: Option<&str>) -> LaunchCommand {
        launch_command(t, &MachinePlatform::Windows, "s.bin", package, None).unwrap()
    }

    fn linux(t: &str, package: Option<&str>) -> LaunchCommand {
        launch_command(t, &MachinePlatform::Linux, "s.bin", package, None).unwrap()
    }

    #[test]
    fn windows_exe_uses_cmd() {
        let c = launch_command(
            "executable/windows/pe32",
            &MachinePlatform::Windows,
            "sample.exe",
            None,
            None,
        )
        .unwrap();
        assert_eq!(c, cmd("cmd.exe", &["/c", "sample.exe"]));
    }

    #[test]
    fn linux_elf_uses_sh_with_chmod() {
        let c = launch_command(
            "executable/linux/elf64",
            &MachinePlatform::Linux,
            "sample.elf",
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            c,
            cmd("sh", &["-c", "chmod +x ./sample.elf && ./sample.elf"])
        );
    }

    #[test]
    fn package_exe() {
        assert_eq!(
            win("text/plain", Some("exe")),
            cmd("cmd.exe", &["/c", "s.bin"])
        );
    }

    #[test]
    fn package_dll() {
        assert_eq!(
            win("executable/windows/dll32", None),
            cmd("rundll32.exe", &["s.bin,DllMain"])
        );
        let c = launch_command(
            "executable/windows/dll64",
            &MachinePlatform::Windows,
            "s.dll",
            None,
            Some("Run"),
        )
        .unwrap();
        assert_eq!(c, cmd("rundll32.exe", &["s.dll,Run"]));
    }

    #[test]
    fn package_msi() {
        assert_eq!(
            win("installer/windows/msi", None),
            cmd("msiexec.exe", &["/i", "s.bin", "/quiet"])
        );
    }

    #[test]
    fn package_powershell() {
        assert_eq!(
            win("code/powershell", None),
            cmd(
                "powershell.exe",
                &["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "s.bin"]
            )
        );
    }

    #[test]
    fn package_wscript() {
        assert_eq!(win("code/javascript", None), cmd("wscript.exe", &["s.bin"]));
        assert_eq!(win("code/vbs", None), cmd("wscript.exe", &["s.bin"]));
    }

    #[test]
    fn package_batch() {
        assert_eq!(win("code/batch", None), cmd("cmd.exe", &["/c", "s.bin"]));
    }

    #[test]
    fn package_hta() {
        assert_eq!(win("code/hta", None), cmd("mshta.exe", &["s.bin"]));
    }

    #[test]
    fn package_open() {
        assert_eq!(
            win("document/pdf", None),
            cmd("cmd.exe", &["/c", "start", "", "s.bin"])
        );
        assert_eq!(linux("document/pdf", None), cmd("xdg-open", &["s.bin"]));
    }

    #[test]
    fn package_elf() {
        assert_eq!(
            linux("text/plain", Some("elf")),
            cmd("sh", &["-c", "chmod +x ./s.bin && ./s.bin"])
        );
    }

    #[test]
    fn package_shell() {
        assert_eq!(linux("code/shell", None), cmd("sh", &["s.bin"]));
    }

    #[test]
    fn package_python() {
        assert_eq!(linux("code/python", None), cmd("python3", &["s.bin"]));
    }

    #[test]
    fn package_invalid_for_platform() {
        let e = launch_command(
            "code/shell",
            &MachinePlatform::Linux,
            "s",
            Some("msi"),
            None,
        )
        .unwrap_err();
        assert_eq!(e, "unknown package msi for linux");
        assert!(launch_command("x", &MachinePlatform::Windows, "s", Some("elf"), None).is_err());
    }

    #[test]
    fn guest_name_sanitises_and_appends_extension() {
        assert_eq!(
            guest_file_name(Some("my file (1)"), "abc", "executable/windows/pe32", ""),
            "my_file__1_.exe"
        );
    }

    #[test]
    fn guest_name_keeps_existing_extension_case_insensitive() {
        assert_eq!(
            guest_file_name(Some("SETUP.EXE"), "abc", "executable/windows/pe32", ""),
            "SETUP.EXE"
        );
    }

    #[test]
    fn guest_name_falls_back_to_sha() {
        let sha = "0123456789abcdef0123456789abcdef";
        assert_eq!(
            guest_file_name(None, sha, "text/plain", ""),
            "0123456789abcdef"
        );
        assert_eq!(
            guest_file_name(Some(""), sha, "code/shell", ""),
            "0123456789abcdef.sh"
        );
    }

    #[test]
    fn guest_name_truncates() {
        let long = "a".repeat(150);
        let out = guest_file_name(Some(&long), "abc", "text/plain", "");
        assert_eq!(out.len(), 100);
    }

    #[test]
    fn guest_name_office_extension_depends_on_mime() {
        assert_eq!(
            guest_file_name(
                Some("doc"),
                "abc",
                "document/office/word",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            ),
            "doc.docx"
        );
        assert_eq!(
            guest_file_name(
                Some("doc"),
                "abc",
                "document/office/word",
                "application/msword"
            ),
            "doc.doc"
        );
    }

    #[test]
    fn sanitize_keeps_empty() {
        assert_eq!(sanitize_file_name(""), "");
        assert_eq!(sanitize_file_name("a/b\\c"), "a_b_c");
    }
}
