use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::warn;

use super::error::ScanError;
use super::manifest::{PluginTypeConfig, parse_manifest};
use super::types::{PluginEntry, PluginId, PluginStatus};

pub struct Scanner {
    plugin_dir: PathBuf,
}

impl Scanner {
    pub fn new(plugin_dir: PathBuf) -> Self {
        Self { plugin_dir }
    }

    /// Scan the entire plugin directory. Invalid manifests are logged and skipped.
    pub fn scan_all(&self) -> Result<Vec<PluginEntry>, ScanError> {
        if !self.plugin_dir.exists() {
            return Err(ScanError::DirectoryNotFound(self.plugin_dir.clone()));
        }

        let mut entries = Vec::new();

        for dir_entry in std::fs::read_dir(&self.plugin_dir)? {
            let dir_entry = dir_entry?;
            let path = dir_entry.path();

            if !path.is_dir() {
                continue;
            }

            match self.scan_one(&path) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    warn!(
                        path = %path.display(),
                        error = %e,
                        "Skipping plugin directory"
                    );
                }
            }
        }

        Ok(entries)
    }

    /// Scan a single plugin subdirectory.
    pub fn scan_one(&self, dir: &Path) -> Result<PluginEntry, ScanError> {
        let manifest_path = dir.join("plugin.toml");

        let manifest = parse_manifest(&manifest_path).map_err(|source| ScanError::Manifest {
            path: manifest_path.clone(),
            source,
        })?;

        let id = PluginId::new(&manifest.plugin.name);

        // Resolve binary path: explicit `binary` field or directory name
        let binary_name = manifest
            .plugin
            .binary
            .clone()
            .or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| manifest.plugin.name.clone());

        let binary_path = resolve_binary_path(dir, &binary_name);

        let is_guest = manifest.plugin.plugin_type == PluginTypeConfig::Guest;

        // Resolve the runtime section (filling defaults) and validate.
        let raw_runtime = manifest.runtime.clone().unwrap_or_default();
        let resolved_runtime =
            malbox_plugin_manifest::ResolvedRuntimeConfig::from_raw(&raw_runtime);

        let (status, runtime_config) = match resolved_runtime.validate() {
            Ok(()) => match check_binary(&binary_path, is_guest) {
                Ok(()) => (PluginStatus::Registered, Some(resolved_runtime)),
                Err(reason) => (PluginStatus::Invalid(reason), Some(resolved_runtime)),
            },
            Err(e) => (
                PluginStatus::Invalid(format!("invalid runtime config: {e}")),
                None,
            ),
        };

        Ok(PluginEntry {
            id,
            manifest,
            binary_path,
            plugin_dir: dir.to_path_buf(),
            registered_at: SystemTime::now(),
            status,
            runtime_config,
        })
    }
}

/// Resolve the binary path, trying the bare name first, then with `.exe` extension.
fn resolve_binary_path(dir: &Path, binary_name: &str) -> PathBuf {
    let bare = dir.join(binary_name);
    if bare.exists() {
        return bare;
    }

    let with_exe = dir.join(format!("{binary_name}.exe"));
    if with_exe.exists() {
        return with_exe;
    }

    // Return the bare path so the error message references the expected name.
    bare
}

/// Check that a binary exists and is executable.
///
/// Guest plugins are Windows executables deployed into the VM, so only
/// existence is verified — Unix permission bits are irrelevant.
fn check_binary(path: &Path, is_guest: bool) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("binary not found: {}", path.display()));
    }

    let metadata =
        std::fs::metadata(path).map_err(|e| format!("cannot read binary metadata: {}", e))?;

    if !metadata.is_file() {
        return Err(format!("binary path is not a file: {}", path.display()));
    }

    if !is_guest {
        let permissions = metadata.permissions();
        if permissions.mode() & 0o111 == 0 {
            return Err(format!("binary is not executable: {}", path.display()));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn create_plugin_dir(parent: &Path, name: &str, manifest_toml: &str) -> PathBuf {
        let dir = parent.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("plugin.toml"), manifest_toml).unwrap();

        let binary_path = dir.join(name);
        std::fs::write(&binary_path, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&binary_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        dir
    }

    fn valid_manifest(name: &str) -> String {
        format!(
            r#"
[plugin]
name = "{name}"
version = "1.0.0"
type = "host"
state = "ephemeral"
execution = "parallel"
"#
        )
    }

    #[test]
    fn scan_all_discovers_plugins() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "plugin-a", &valid_manifest("plugin-a"));
        create_plugin_dir(tmp.path(), "plugin-b", &valid_manifest("plugin-b"));

        let scanner = Scanner::new(tmp.path().to_path_buf());
        let entries = scanner.scan_all().unwrap();

        assert_eq!(entries.len(), 2);
        let ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains(&"plugin-a"));
        assert!(ids.contains(&"plugin-b"));
    }

    #[test]
    fn scan_all_empty_directory() {
        let tmp = TempDir::new().unwrap();
        let scanner = Scanner::new(tmp.path().to_path_buf());
        let entries = scanner.scan_all().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn scan_all_skips_files() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "real-plugin", &valid_manifest("real-plugin"));
        std::fs::write(tmp.path().join("not-a-plugin.txt"), "hello").unwrap();

        let scanner = Scanner::new(tmp.path().to_path_buf());
        let entries = scanner.scan_all().unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn scan_one_valid_plugin() {
        let tmp = TempDir::new().unwrap();
        let dir = create_plugin_dir(tmp.path(), "pe-parser", &valid_manifest("pe-parser"));

        let scanner = Scanner::new(tmp.path().to_path_buf());
        let entry = scanner.scan_one(&dir).unwrap();

        assert_eq!(entry.id.as_str(), "pe-parser");
        assert!(matches!(entry.status, PluginStatus::Registered));
        assert!(entry.binary_path.exists());
    }

    #[test]
    fn scan_one_missing_manifest() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("no-manifest");
        std::fs::create_dir_all(&dir).unwrap();

        let scanner = Scanner::new(tmp.path().to_path_buf());
        let result = scanner.scan_one(&dir);
        assert!(result.is_err());
    }

    #[test]
    fn scan_one_missing_binary_returns_invalid() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("no-binary");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("plugin.toml"), valid_manifest("no-binary")).unwrap();

        let scanner = Scanner::new(tmp.path().to_path_buf());
        let entry = scanner.scan_one(&dir).unwrap();
        assert!(matches!(entry.status, PluginStatus::Invalid(_)));
    }

    #[test]
    fn scan_one_non_executable_binary_returns_invalid() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("no-exec");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("plugin.toml"), valid_manifest("no-exec")).unwrap();

        let binary_path = dir.join("no-exec");
        std::fs::write(&binary_path, "not executable").unwrap();

        let scanner = Scanner::new(tmp.path().to_path_buf());
        let entry = scanner.scan_one(&dir).unwrap();
        assert!(matches!(entry.status, PluginStatus::Invalid(_)));
    }

    #[test]
    fn scan_one_explicit_binary_name() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("custom-bin");
        std::fs::create_dir_all(&dir).unwrap();

        let manifest = r#"
[plugin]
name = "custom-bin"
version = "1.0.0"
type = "host"
state = "ephemeral"
execution = "parallel"
binary = "my-custom-binary"
"#;
        std::fs::write(dir.join("plugin.toml"), manifest).unwrap();

        let binary_path = dir.join("my-custom-binary");
        std::fs::write(&binary_path, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&binary_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let scanner = Scanner::new(tmp.path().to_path_buf());
        let entry = scanner.scan_one(&dir).unwrap();
        assert!(matches!(entry.status, PluginStatus::Registered));
        assert!(entry.binary_path.ends_with("my-custom-binary"));
    }

    fn guest_manifest(name: &str) -> String {
        format!(
            r#"
[plugin]
name = "{name}"
version = "1.0.0"
type = "guest"
state = "ephemeral"
execution = "exclusive"
"#
        )
    }

    #[test]
    fn scan_one_guest_plugin_with_exe_binary() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("guest-yara-scanner");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("plugin.toml"),
            guest_manifest("guest-yara-scanner"),
        )
        .unwrap();

        // Only an .exe binary exists (no bare name)
        let binary_path = dir.join("guest-yara-scanner.exe");
        std::fs::write(&binary_path, b"MZ\x00").unwrap();

        let scanner = Scanner::new(tmp.path().to_path_buf());
        let entry = scanner.scan_one(&dir).unwrap();
        assert!(matches!(entry.status, PluginStatus::Registered));
        assert!(entry.binary_path.ends_with("guest-yara-scanner.exe"));
    }

    #[test]
    fn scan_one_guest_plugin_no_exec_permission_still_valid() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("guest-plugin");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("plugin.toml"), guest_manifest("guest-plugin")).unwrap();

        let binary_path = dir.join("guest-plugin.exe");
        std::fs::write(&binary_path, b"MZ\x00").unwrap();
        // Explicitly non-executable
        std::fs::set_permissions(&binary_path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let scanner = Scanner::new(tmp.path().to_path_buf());
        let entry = scanner.scan_one(&dir).unwrap();
        assert!(matches!(entry.status, PluginStatus::Registered));
    }

    #[test]
    fn scan_all_nonexistent_directory() {
        let scanner = Scanner::new(PathBuf::from("/nonexistent/path"));
        let result = scanner.scan_all();
        assert!(result.is_err());
    }

    #[test]
    fn scan_all_invalid_manifest_still_skipped() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("bad-manifest");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("plugin.toml"), "this is not valid toml [[[").unwrap();

        let scanner = Scanner::new(tmp.path().to_path_buf());
        let entries = scanner.scan_all().unwrap();
        assert!(entries.is_empty());
    }
}

#[cfg(test)]
mod runtime_validation_tests {
    use super::*;
    use tempfile::TempDir;

    fn write_plugin_dir(name: &str, manifest: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join(name);
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.toml"), manifest).unwrap();
        // Bare "binary" file so resolve_binary_path succeeds.
        std::fs::write(plugin_dir.join(name), b"").unwrap();
        dir
    }

    #[test]
    fn scanner_marks_plugin_invalid_when_runtime_port_out_of_range() {
        let dir = write_plugin_dir(
            "bad-port-plugin",
            r#"
[plugin]
name = "bad-port-plugin"
version = "0.1.0"
type = "guest"
state = "ephemeral"
execution = "exclusive"

[runtime]
port = 80
"#,
        );
        let scanner = Scanner::new(dir.path().to_path_buf());
        let entries = scanner.scan_all().unwrap();
        assert_eq!(entries.len(), 1);
        match &entries[0].status {
            PluginStatus::Invalid(reason) => assert!(reason.contains("port")),
            s => panic!("expected Invalid, got {:?}", s),
        }
    }

    #[test]
    fn scanner_exposes_resolved_runtime_config() {
        let dir = write_plugin_dir(
            "good-plugin",
            r#"
[plugin]
name = "good-plugin"
version = "0.1.0"
type = "guest"
state = "ephemeral"
execution = "exclusive"

[runtime]
port = 50100
work_dir = "/opt/malbox"
"#,
        );
        let scanner = Scanner::new(dir.path().to_path_buf());
        let entries = scanner.scan_all().unwrap();
        let runtime = entries[0].runtime_config.as_ref().expect("runtime_config");
        assert_eq!(runtime.port, 50100);
        assert_eq!(runtime.work_dir, std::path::PathBuf::from("/opt/malbox"));
        assert_eq!(runtime.log_filter, "info");
    }
}
