extern crate malbox_plugin_sdk as malbox;

use base64::Engine;
use malbox::prelude::*;
use serde_json::json;

#[malbox::host_plugin]
struct ReportShowcase;

#[malbox::handlers]
impl ReportShowcase {
    #[malbox::on_start]
    fn init(&self) -> Result<()> {
        info!("Report showcase plugin started");
        Ok(())
    }

    #[malbox::on_task]
    fn analyze(&self, ctx: &Context) -> Result<()> {
        ctx.progress(0.1, "generating showcase report")?;

        // Push sibling artifacts that Image and Download blocks reference.
        ctx.results().push(PluginResult::bytes(
            "screenshot",
            vec![0x89, 0x50, 0x4E, 0x47],
        ))?;
        ctx.results().push(PluginResult::bytes(
            "network_capture",
            vec![0xD4, 0xC3, 0xB2, 0xA1],
        ))?;

        ctx.progress(0.3, "building report")?;

        let indicators = build_indicators();
        let ttps = build_ttps();

        let report = ReportBuilder::new("host-report-showcase", "0.1.0")
            .display_name("Report Showcase")
            .summary(
                "Comprehensive analysis detected a multi-stage dropper with process injection, \
                 C2 communication, and credential harvesting capabilities. \
                 7 IOCs extracted, 5 ATT&CK techniques mapped.",
            )
            .verdict(Classification::Malicious, Some(92), Some(Confidence::High))
            .labels([
                "trojan",
                "dropper",
                "stealer",
                "process-injection",
                "c2-beacon",
            ])
            // -- semantic indicators --
            .indicator(indicators[0].clone())
            .indicator(indicators[1].clone())
            .indicator(indicators[2].clone())
            .indicator(indicators[3].clone())
            .indicator(indicators[4].clone())
            .indicator(indicators[5].clone())
            .indicator(indicators[6].clone())
            // -- semantic TTPs --
            .ttp(ttps[0].clone())
            .ttp(ttps[1].clone())
            .ttp(ttps[2].clone())
            .ttp(ttps[3].clone())
            .ttp(ttps[4].clone())
            // -- artifacts --
            .artifact(
                ArtifactRef::new("screenshot", "screenshot")
                    .description("Desktop screenshot captured at T+12s during detonation"),
            )
            .artifact(
                ArtifactRef::new("network_capture", "pcap")
                    .description("Full packet capture from sandbox network tap"),
            )
            // -- presentation sections --
            .section("overview", "Overview", build_overview_section)
            .section("callouts", "Alerts & Notifications", build_callouts_section)
            .section("file-metadata", "File Metadata", build_kv_section)
            .section("strings", "Extracted Strings", build_table_section)
            .section("code-samples", "Code Samples", build_code_section)
            .section("raw-config", "Extracted Configuration", build_json_section)
            .section("pe-header", "PE Header Hex Dump", build_hex_section)
            .section("artifacts", "Artifacts", build_artifact_section)
            .section("process-tree", "Process Tree", build_tree_section)
            .section("timeline", "Behavioral Timeline", build_timeline_section)
            .section(
                "network-graph",
                "Network Communication Graph",
                build_graph_section,
            )
            .raw(json!({
                "engine_version": "3.2.1",
                "scan_duration_ms": 14832,
                "rules_loaded": 2847,
                "sandbox_profile": "win10-office-network"
            }))
            .build();

        ctx.progress(0.9, "submitting report")?;
        ctx.results().push(report.into_plugin_result()?)?;

        info!(task_id = ctx.task().id(), "showcase report submitted");
        Ok(())
    }

    #[malbox::on_stop]
    fn shutdown(&self) -> Result<()> {
        info!("Report showcase plugin stopped");
        Ok(())
    }
}

fn build_indicators() -> Vec<Indicator> {
    vec![
        Indicator::new(
            "sha256",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
        .context("submitted sample")
        .first_seen("2026-05-08T09:14:22Z"),
        Indicator::new("md5", "d41d8cd98f00b204e9800998ecf8427e").context("submitted sample"),
        Indicator::new("domain", "update-service.evil.example")
            .context("C2 beacon resolved via DNS TXT query")
            .first_seen("2026-05-08T09:14:35Z"),
        Indicator::new("ipv4", "198.51.100.47")
            .context("C2 server, HTTPS on port 8443")
            .first_seen("2026-05-08T09:14:36Z"),
        Indicator::new("url", "https://198.51.100.47:8443/gate.php")
            .context("exfiltration endpoint receiving stolen credentials"),
        Indicator::new("mutex", "Global\\{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}")
            .context("single-instance mutex created at startup"),
        Indicator::new(
            "registry",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run\\SvcHost",
        )
        .context("persistence via Run key, value points to dropped binary"),
    ]
}

fn build_ttps() -> Vec<Ttp> {
    vec![
        Ttp::new("T1055.012", "Process Hollowing")
            .evidence("Hollowed svchost.exe (PID 4812) - unmapped original image and wrote payload at base 0x00400000"),
        Ttp::new("T1547.001", "Registry Run Keys / Startup Folder")
            .evidence("Created HKCU\\...\\Run\\SvcHost pointing to %APPDATA%\\svchost.exe"),
        Ttp::new("T1071.001", "Web Protocols")
            .evidence("HTTPS POST to 198.51.100.47:8443/gate.php with base64-encoded credential dump"),
        Ttp::new("T1003.001", "LSASS Memory")
            .evidence("Called MiniDumpWriteDump on lsass.exe (PID 680) to dump credentials"),
        Ttp::new("T1140", "Deobfuscate/Decode Files or Information")
            .evidence("XOR-decoded second stage payload from resource section using key 0x5A"),
    ]
}

fn build_overview_section(s: SectionBuilder) -> SectionBuilder {
    s.heading(2, "Executive Summary")
        .markdown(
            "This sample is a **multi-stage dropper** that delivers a credential-stealing payload \
             via process hollowing. On execution, it decodes an embedded second stage from its PE \
             resource section using a single-byte XOR key (`0x5A`), hollows a legitimate `svchost.exe` \
             process, and injects the decoded payload.\n\n\
             The injected payload:\n\
             - Harvests credentials from LSASS memory via `MiniDumpWriteDump`\n\
             - Establishes persistence through a registry Run key\n\
             - Beacons to a C2 server at `198.51.100.47:8443` over HTTPS\n\
             - Exfiltrates stolen credentials in base64-encoded POST bodies\n\n\
             Detection confidence is **high** - behavioral signatures match known Lumma Stealer variants.",
        )
        .divider()
        .heading(3, "Risk Assessment")
        .markdown(
            "| Factor | Rating |\n\
             |--------|--------|\n\
             | Data exfiltration | Critical |\n\
             | Persistence | High |\n\
             | Evasion sophistication | Medium |\n\
             | Lateral movement | Low |",
        )
}

fn build_callouts_section(s: SectionBuilder) -> SectionBuilder {
    s.callout(
        CalloutLevel::Error,
        "Active credential theft detected - LSASS memory was dumped. Rotate all credentials \
         on the affected host immediately.",
    )
    .callout(
        CalloutLevel::Warn,
        "The C2 domain update-service.evil.example resolves to a fast-flux network. \
         Block the IP range 198.51.100.0/24 at the perimeter firewall.",
    )
    .callout(
        CalloutLevel::Info,
        "This sample shares code overlap with Lumma Stealer v4.2 (build 2026-04). \
         See VirusTotal collection VT-2026-04-lumma for related samples.",
    )
    .callout(
        CalloutLevel::Success,
        "Sandbox detonation completed successfully. All behavioral artifacts were captured \
         within the 60-second execution window.",
    )
}

fn build_kv_section(s: SectionBuilder) -> SectionBuilder {
    s.kv([
        KvPair {
            key: "File name".into(),
            value: "invoice_2026_05.exe".into(),
            mono: false,
        },
        KvPair {
            key: "File size".into(),
            value: "284,672 bytes (278 KB)".into(),
            mono: false,
        },
        KvPair {
            key: "File type".into(),
            value: "PE32 executable (GUI) Intel 80386, for MS Windows".into(),
            mono: false,
        },
        KvPair {
            key: "SHA-256".into(),
            value: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
            mono: true,
        },
        KvPair {
            key: "MD5".into(),
            value: "d41d8cd98f00b204e9800998ecf8427e".into(),
            mono: true,
        },
        KvPair {
            key: "SHA-1".into(),
            value: "da39a3ee5e6b4b0d3255bfef95601890afd80709".into(),
            mono: true,
        },
        KvPair {
            key: "Compile time".into(),
            value: "2026-04-30T14:22:08Z".into(),
            mono: false,
        },
        KvPair {
            key: "Packer".into(),
            value: "UPX 3.96 (modified)".into(),
            mono: false,
        },
        KvPair {
            key: "Imphash".into(),
            value: "f34d5f2d4577ed6d9ceec516c1f5a744".into(),
            mono: true,
        },
        KvPair {
            key: "Entropy".into(),
            value: "7.84 (packed)".into(),
            mono: false,
        },
        KvPair {
            key: "Signed".into(),
            value: "No".into(),
            mono: false,
        },
    ])
}

fn build_table_section(s: SectionBuilder) -> SectionBuilder {
    s.heading(3, "Suspicious Strings")
        .markdown("Strings extracted after unpacking the UPX layer. Filtered to items with analytical value.")
        .block(Block::Table {
            columns: vec![
                Column { key: "offset".into(), label: "Offset".into(), r#type: "string".into() },
                Column { key: "string".into(), label: "String".into(), r#type: "string".into() },
                Column { key: "encoding".into(), label: "Encoding".into(), r#type: "string".into() },
                Column { key: "category".into(), label: "Category".into(), r#type: "string".into() },
            ],
            rows: vec![
                json!({"offset": "0x00012A40", "string": "MiniDumpWriteDump", "encoding": "ASCII", "category": "API"}),
                json!({"offset": "0x00012A80", "string": "NtUnmapViewOfSection", "encoding": "ASCII", "category": "API"}),
                json!({"offset": "0x00012AC0", "string": "VirtualAllocEx", "encoding": "ASCII", "category": "API"}),
                json!({"offset": "0x00012B00", "string": "WriteProcessMemory", "encoding": "ASCII", "category": "API"}),
                json!({"offset": "0x00013100", "string": "update-service.evil.example", "encoding": "ASCII", "category": "Network"}),
                json!({"offset": "0x00013140", "string": "/gate.php", "encoding": "ASCII", "category": "Network"}),
                json!({"offset": "0x00013180", "string": "Mozilla/5.0 (Windows NT 10.0; Win64; x64)", "encoding": "ASCII", "category": "Network"}),
                json!({"offset": "0x00013400", "string": "Software\\Microsoft\\Windows\\CurrentVersion\\Run", "encoding": "UTF-16LE", "category": "Persistence"}),
                json!({"offset": "0x00013500", "string": "Global\\{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}", "encoding": "ASCII", "category": "Mutex"}),
                json!({"offset": "0x00013600", "string": "%APPDATA%\\svchost.exe", "encoding": "UTF-16LE", "category": "Filesystem"}),
                json!({"offset": "0x00013700", "string": "lsass.exe", "encoding": "ASCII", "category": "Process"}),
                json!({"offset": "0x00013740", "string": "SELECT * FROM Win32_Processor", "encoding": "ASCII", "category": "Recon"}),
            ],
            sortable: true,
            searchable: true,
        })
}

fn build_code_section(s: SectionBuilder) -> SectionBuilder {
    s.heading(3, "Decoded XOR Routine")
        .markdown(
            "Reconstructed decryption stub extracted from the unpacked binary at `0x00401200`:",
        )
        .code(
            "c",
            r#"void decode_payload(unsigned char *buf, size_t len, unsigned char key) {
    for (size_t i = 0; i < len; i++) {
        buf[i] ^= key;
        key = (key + buf[i]) & 0xFF;  // rolling key
    }
}"#,
        )
        .divider()
        .heading(3, "YARA Signature")
        .markdown("Custom rule that triggered on the decoded second stage:")
        .code(
            "yara",
            r#"rule LummaStealer_v4_Dropper {
    meta:
        author = "Malbox Research"
        date = "2026-05-08"
        description = "Detects Lumma Stealer v4.x dropper stage"
        severity = "critical"

    strings:
        $xor_loop = { 30 ?? 0F B6 ?? 02 ?? 25 FF 00 00 00 }
        $mutex    = "Global\\{A1B2C3D4" ascii
        $gate     = "/gate.php" ascii
        $ua       = "Mozilla/5.0 (Windows NT 10.0" ascii

    condition:
        uint16(0) == 0x5A4D and
        filesize < 500KB and
        $xor_loop and
        2 of ($mutex, $gate, $ua)
}"#,
        )
}

fn build_json_section(s: SectionBuilder) -> SectionBuilder {
    s.markdown(
        "Configuration block extracted from the decoded second stage at offset `0x0001A000`:",
    )
    .json(json!({
        "version": "4.2.1",
        "build_id": "lm-2026-04-30-7f3a",
        "c2": {
            "primary": {
                "host": "198.51.100.47",
                "port": 8443,
                "path": "/gate.php",
                "protocol": "https",
                "timeout_ms": 30000
            },
            "fallback": [
                {"host": "203.0.113.12", "port": 443, "path": "/api/v2/check"},
                {"host": "update-cdn.evil.example", "port": 8443, "path": "/gate.php"}
            ],
            "beacon_interval_s": 300,
            "jitter_pct": 20
        },
        "steal": {
            "browsers": ["chrome", "firefox", "edge", "brave", "opera"],
            "crypto_wallets": ["metamask", "phantom", "exodus"],
            "credentials": true,
            "cookies": true,
            "autofill": true,
            "credit_cards": true
        },
        "evasion": {
            "check_debugger": true,
            "check_vm": true,
            "check_sandbox": true,
            "sleep_on_detect_ms": 0,
            "vm_vendors_exit": ["vmware", "virtualbox", "qemu", "xen"],
            "min_ram_gb": 4,
            "min_disk_gb": 60,
            "min_cpu_cores": 2
        },
        "persistence": {
            "method": "registry_run",
            "key": "SvcHost",
            "copy_to": "%APPDATA%\\svchost.exe"
        },
        "exfil": {
            "method": "https_post",
            "encoding": "base64",
            "chunk_size_kb": 512,
            "encrypt": false
        }
    }))
}

fn build_hex_section(s: SectionBuilder) -> SectionBuilder {
    let pe_header: Vec<u8> = vec![
        // DOS header
        0x4D, 0x5A, 0x90, 0x00, 0x03, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0x00,
        0x00, 0xB8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xF0, 0x00, 0x00, 0x00, // PE signature
        0x50, 0x45, 0x00, 0x00, // COFF header
        0x4C, 0x01, 0x05, 0x00, 0x3E, 0xB1, 0x72, 0x68, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xE0, 0x00, 0x02, 0x01, // Optional header (partial)
        0x0B, 0x01, 0x0E, 0x1C, 0x00, 0x40, 0x03, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x6A, 0x12, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x50, 0x03, 0x00, 0x00, 0x00,
        0x40, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let encoded = base64::engine::general_purpose::STANDARD.encode(&pe_header);

    s.markdown("First 128 bytes of the PE file showing DOS header and PE signature:")
        .hex(encoded, 0x0000)
}

fn build_artifact_section(s: SectionBuilder) -> SectionBuilder {
    s.heading(3, "Desktop Screenshot")
        .markdown("Captured at T+12s during detonation - shows the sample mimicking an invoice PDF viewer:")
        .image("screenshot", Some("Desktop state at T+12s during execution".into()))
        .divider()
        .heading(3, "Network Capture")
        .markdown("Full PCAP from the sandbox network tap covering the 60-second detonation window:")
        .download("network_capture", "Download network capture (PCAP)")
}

fn build_tree_section(s: SectionBuilder) -> SectionBuilder {
    s.markdown("Process tree captured during the 60-second detonation window:")
        .tree([
            TreeNode {
                label: "explorer.exe (PID 1024)".into(),
                children: vec![TreeNode {
                    label: "invoice_2026_05.exe (PID 3456)".into(),
                    children: vec![
                        TreeNode {
                            label: "cmd.exe (PID 3460)".into(),
                            children: vec![TreeNode {
                                label: "svchost.exe [HOLLOWED] (PID 4812)".into(),
                                children: vec![
                                    TreeNode {
                                        label: "credential dump -> lsass.exe (PID 680)".into(),
                                        children: vec![],
                                        meta: json!({
                                            "technique": "T1003.001",
                                            "api": "MiniDumpWriteDump",
                                            "timestamp": "T+8.4s"
                                        }),
                                    },
                                    TreeNode {
                                        label: "C2 beacon -> 198.51.100.47:8443".into(),
                                        children: vec![],
                                        meta: json!({
                                            "technique": "T1071.001",
                                            "protocol": "HTTPS",
                                            "interval": "300s",
                                            "timestamp": "T+10.1s"
                                        }),
                                    },
                                ],
                                meta: json!({
                                    "technique": "T1055.012",
                                    "original_image": "C:\\Windows\\System32\\svchost.exe",
                                    "injected_base": "0x00400000",
                                    "timestamp": "T+4.2s"
                                }),
                            }],
                            meta: json!({"commandline": "cmd.exe /c start svchost.exe", "timestamp": "T+3.8s"}),
                        },
                        TreeNode {
                            label: "reg.exe (PID 3472)".into(),
                            children: vec![],
                            meta: json!({
                                "commandline": "reg add HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run /v SvcHost /d %APPDATA%\\svchost.exe",
                                "technique": "T1547.001",
                                "timestamp": "T+5.1s"
                            }),
                        },
                    ],
                    meta: json!({
                        "path": "C:\\Users\\user\\Downloads\\invoice_2026_05.exe",
                        "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                        "timestamp": "T+0.0s"
                    }),
                }],
                meta: json!({"session": 1, "user": "user"}),
            },
            TreeNode {
                label: "services.exe (PID 512)".into(),
                children: vec![TreeNode {
                    label: "lsass.exe (PID 680)".into(),
                    children: vec![],
                    meta: json!({"note": "target of credential dumping", "integrity": "SYSTEM"}),
                }],
                meta: json!({"session": 0, "user": "SYSTEM"}),
            },
        ])
}

fn build_timeline_section(s: SectionBuilder) -> SectionBuilder {
    s.markdown("Chronological sequence of significant events during the sandbox detonation:")
        .timeline([
            TimelineEvent {
                ts: "T+0.0s".into(),
                label: "Sample executed by user double-click".into(),
                severity: None,
                meta: json!({"pid": 3456, "path": "C:\\Users\\user\\Downloads\\invoice_2026_05.exe"}),
            },
            TimelineEvent {
                ts: "T+0.3s".into(),
                label: "UPX unpacking layer decoded in memory".into(),
                severity: None,
                meta: json!({"technique": "T1140"}),
            },
            TimelineEvent {
                ts: "T+1.2s".into(),
                label: "Mutex created: Global\\{A1B2C3D4-...}".into(),
                severity: Some("low".into()),
                meta: json!({"api": "CreateMutexW"}),
            },
            TimelineEvent {
                ts: "T+2.1s".into(),
                label: "Second stage XOR-decoded from PE resource (key=0x5A)".into(),
                severity: Some("medium".into()),
                meta: json!({"technique": "T1140", "payload_size": 163840}),
            },
            TimelineEvent {
                ts: "T+3.0s".into(),
                label: "Dropped copy to %APPDATA%\\svchost.exe".into(),
                severity: Some("medium".into()),
                meta: json!({"api": "CopyFileW", "destination": "%APPDATA%\\svchost.exe"}),
            },
            TimelineEvent {
                ts: "T+3.8s".into(),
                label: "Spawned cmd.exe as child process".into(),
                severity: None,
                meta: json!({"pid": 3460}),
            },
            TimelineEvent {
                ts: "T+4.2s".into(),
                label: "Process hollowing: svchost.exe unmapped and payload injected".into(),
                severity: Some("high".into()),
                meta: json!({
                    "technique": "T1055.012",
                    "target_pid": 4812,
                    "apis": ["NtUnmapViewOfSection", "VirtualAllocEx", "WriteProcessMemory", "SetThreadContext"]
                }),
            },
            TimelineEvent {
                ts: "T+5.1s".into(),
                label: "Registry Run key set for persistence".into(),
                severity: Some("high".into()),
                meta: json!({"technique": "T1547.001", "key": "HKCU\\...\\Run\\SvcHost"}),
            },
            TimelineEvent {
                ts: "T+6.0s".into(),
                label: "DNS TXT query for update-service.evil.example".into(),
                severity: Some("medium".into()),
                meta: json!({"resolved_ip": "198.51.100.47", "record_type": "A"}),
            },
            TimelineEvent {
                ts: "T+8.4s".into(),
                label: "LSASS memory dumped via MiniDumpWriteDump".into(),
                severity: Some("high".into()),
                meta: json!({"technique": "T1003.001", "target_pid": 680, "dump_size": 34603008}),
            },
            TimelineEvent {
                ts: "T+10.1s".into(),
                label: "First C2 beacon: HTTPS POST to 198.51.100.47:8443/gate.php".into(),
                severity: Some("high".into()),
                meta: json!({"technique": "T1071.001", "bytes_sent": 524288, "response_code": 200}),
            },
            TimelineEvent {
                ts: "T+12.0s".into(),
                label: "Desktop screenshot captured".into(),
                severity: None,
                meta: json!({"artifact": "screenshot"}),
            },
            TimelineEvent {
                ts: "T+45.0s".into(),
                label: "Second C2 beacon with additional exfiltrated data".into(),
                severity: Some("high".into()),
                meta: json!({"bytes_sent": 262144, "response_code": 200}),
            },
            TimelineEvent {
                ts: "T+60.0s".into(),
                label: "Sandbox timeout reached - execution terminated".into(),
                severity: None,
                meta: serde_json::Value::Null,
            },
        ])
}

fn build_graph_section(s: SectionBuilder) -> SectionBuilder {
    s.markdown(
        "Network communication graph showing relationships between the sample, \
         C2 infrastructure, and exfiltration targets:",
    )
    .graph(
        [
            GraphNode {
                id: "sample".into(),
                label: "invoice_2026_05.exe".into(),
                meta: json!({"type": "malware", "pid": 3456}),
            },
            GraphNode {
                id: "svchost".into(),
                label: "svchost.exe (hollowed)".into(),
                meta: json!({"type": "injected_process", "pid": 4812}),
            },
            GraphNode {
                id: "dns".into(),
                label: "DNS Resolver".into(),
                meta: json!({"type": "infrastructure", "ip": "10.0.0.2"}),
            },
            GraphNode {
                id: "c2_domain".into(),
                label: "update-service.evil.example".into(),
                meta: json!({"type": "c2_domain"}),
            },
            GraphNode {
                id: "c2_primary".into(),
                label: "198.51.100.47:8443".into(),
                meta: json!({"type": "c2_server", "port": 8443, "geo": "RU"}),
            },
            GraphNode {
                id: "c2_fallback1".into(),
                label: "203.0.113.12:443".into(),
                meta: json!({"type": "c2_server", "port": 443, "geo": "UA"}),
            },
            GraphNode {
                id: "c2_fallback2".into(),
                label: "update-cdn.evil.example".into(),
                meta: json!({"type": "c2_domain", "fallback": true}),
            },
            GraphNode {
                id: "lsass".into(),
                label: "lsass.exe".into(),
                meta: json!({"type": "target_process", "pid": 680}),
            },
            GraphNode {
                id: "registry".into(),
                label: "HKCU\\...\\Run\\SvcHost".into(),
                meta: json!({"type": "persistence"}),
            },
        ],
        [
            GraphEdge {
                from: "sample".into(),
                to: "svchost".into(),
                label: Some("process hollowing (T1055.012)".into()),
            },
            GraphEdge {
                from: "svchost".into(),
                to: "dns".into(),
                label: Some("TXT query".into()),
            },
            GraphEdge {
                from: "dns".into(),
                to: "c2_domain".into(),
                label: Some("resolves".into()),
            },
            GraphEdge {
                from: "c2_domain".into(),
                to: "c2_primary".into(),
                label: Some("A record".into()),
            },
            GraphEdge {
                from: "svchost".into(),
                to: "c2_primary".into(),
                label: Some("HTTPS beacon + exfil".into()),
            },
            GraphEdge {
                from: "c2_primary".into(),
                to: "c2_fallback1".into(),
                label: Some("fallback".into()),
            },
            GraphEdge {
                from: "c2_primary".into(),
                to: "c2_fallback2".into(),
                label: Some("fallback".into()),
            },
            GraphEdge {
                from: "svchost".into(),
                to: "lsass".into(),
                label: Some("credential dump (T1003.001)".into()),
            },
            GraphEdge {
                from: "sample".into(),
                to: "registry".into(),
                label: Some("persistence (T1547.001)".into()),
            },
        ],
    )
}
