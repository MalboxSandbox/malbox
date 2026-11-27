extern crate malbox_plugin_sdk as malbox;

use malbox::prelude::*;
use std::fs::File;
use std::io::Read;
use zip::ZipArchive;
use zip::result::ZipError;

/// Entries larger than this (uncompressed) are listed but not extracted.
const MAX_ENTRY_BYTES: u64 = 100 * 1024 * 1024;
/// Extraction stops once this many bytes have been extracted in total.
const MAX_TOTAL_BYTES: usize = 512 * 1024 * 1024;
/// Password tried after the configured and shared ones.
const DEFAULT_PASSWORD: &str = "infected";

/// Extracts zip archives and pushes every entry as an analysed child artifact.
#[malbox::host_plugin]
struct ArchiveExtractor;

#[malbox::handlers]
impl ArchiveExtractor {
    #[malbox::on_task]
    fn process(&self, ctx: &Context) -> Result<()> {
        let passwords = candidate_passwords(&ctx.task());
        let mut archive =
            ZipArchive::new(File::open(ctx.task().sample_path())?).map_err(zip_err)?;

        let mut rows = Vec::new();
        let mut extracted = 0usize;
        let mut any_encrypted = false;
        let mut limit_reached = false;
        let mut total = 0usize;
        let mut total_bytes = 0usize;

        for index in 0..archive.len() {
            let (name, size, is_dir, encrypted) = match archive.by_index_raw(index) {
                Ok(entry) => (
                    entry.name().to_owned(),
                    entry.size(),
                    entry.is_dir(),
                    entry.encrypted(),
                ),
                Err(e) => {
                    total += 1;
                    let status = format!("error: {e}");
                    rows.push(serde_json::json!({ "name": format!("#{index}"), "size": 0, "status": status }));
                    continue;
                }
            };
            if is_dir {
                continue;
            }
            total += 1;
            if !limit_reached && total_bytes >= MAX_TOTAL_BYTES {
                info!(
                    max_total_bytes = MAX_TOTAL_BYTES,
                    "extraction byte budget spent; stopping extraction"
                );
                limit_reached = true;
            }
            let status = if limit_reached {
                "limit reached".to_owned()
            } else if size > MAX_ENTRY_BYTES {
                "skipped (too large)".to_owned()
            } else {
                match read_entry(&mut archive, index, encrypted, &passwords) {
                    Err(e) => format!("error: {e}"),
                    Ok(EntryRead::Encrypted) => {
                        any_encrypted = true;
                        "encrypted".to_owned()
                    }
                    Ok(EntryRead::TooLarge) => "skipped (too large)".to_owned(),
                    Ok(EntryRead::Data(buf)) => {
                        let len = buf.len();
                        match ctx.results().push_artifact(
                            &name,
                            ArtifactSource::Bytes(buf),
                            ArtifactOpts::new(ArtifactRelation::Extracted),
                        ) {
                            Ok(()) => {
                                extracted += 1;
                                total_bytes = total_bytes.saturating_add(len);
                                "extracted".to_owned()
                            }
                            Err(SdkError::ArtifactLimit { limit }) => {
                                info!(limit, "artifact limit reached; stopping extraction");
                                limit_reached = true;
                                "limit reached".to_owned()
                            }
                            Err(e) => return Err(e),
                        }
                    }
                }
            };
            rows.push(serde_json::json!({ "name": name, "size": size, "status": status }));
        }

        let mut report = ReportBuilder::new("host-archive-extractor", env!("CARGO_PKG_VERSION"))
            .display_name("Archive Extractor")
            .summary(format!("extracted {extracted} of {total} entries"))
            .verdict(Classification::Unknown, None, None);
        if any_encrypted {
            report = report.labels(["encrypted"]);
        }
        let report = report
            .section("entries", "Entries", |s| {
                s.table(
                    [
                        Column {
                            key: "name".into(),
                            label: "Name".into(),
                            r#type: "string".into(),
                        },
                        Column {
                            key: "size".into(),
                            label: "Size".into(),
                            r#type: "number".into(),
                        },
                        Column {
                            key: "status".into(),
                            label: "Status".into(),
                            r#type: "string".into(),
                        },
                    ],
                    rows,
                )
            })
            .build();
        ctx.results().push(report.into_plugin_result()?)?;
        Ok(())
    }
}

/// `params.passwords`, then `shared.passwords`, then the default; deduplicated.
fn candidate_passwords(task: &TaskInfo) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let sources = [
        task.params().get("passwords").cloned(),
        task.shared().get("passwords").cloned(),
    ];
    let listed = sources
        .into_iter()
        .flatten()
        .filter_map(|v| match v {
            serde_json::Value::Array(items) => Some(items),
            _ => None,
        })
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned));
    for p in listed.chain(std::iter::once(DEFAULT_PASSWORD.to_owned())) {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// Outcome of reading one entry.
enum EntryRead {
    Data(Vec<u8>),
    /// Encrypted and no candidate password opens it.
    Encrypted,
    /// Decompressed past [`MAX_ENTRY_BYTES`] despite its declared size.
    TooLarge,
}

/// Reads one entry, never buffering more than [`MAX_ENTRY_BYTES`].
fn read_entry(
    archive: &mut ZipArchive<File>,
    index: usize,
    encrypted: bool,
    passwords: &[String],
) -> Result<EntryRead> {
    if !encrypted {
        let mut entry = archive.by_index(index).map_err(zip_err)?;
        return Ok(read_bounded(&mut entry)?.map_or(EntryRead::TooLarge, EntryRead::Data));
    }
    for password in passwords {
        let mut entry = match archive.by_index_decrypt(index, password.as_bytes()) {
            Ok(entry) => entry,
            Err(ZipError::InvalidPassword) => continue,
            Err(e) => return Err(zip_err(e)),
        };
        // ZipCrypto's header check passes for ~1/256 wrong passwords; the
        // CRC check while reading catches those.
        if let Ok(read) = read_bounded(&mut entry) {
            return Ok(read.map_or(EntryRead::TooLarge, EntryRead::Data));
        }
    }
    Ok(EntryRead::Encrypted)
}

/// Reads `entry` to the end; `None` once it exceeds [`MAX_ENTRY_BYTES`], so a
/// lying declared size cannot make us buffer more than that.
fn read_bounded(entry: &mut impl Read) -> std::io::Result<Option<Vec<u8>>> {
    let mut buf = Vec::new();
    entry.take(MAX_ENTRY_BYTES + 1).read_to_end(&mut buf)?;
    Ok(u64::try_from(buf.len())
        .is_ok_and(|n| n <= MAX_ENTRY_BYTES)
        .then_some(buf))
}

fn zip_err(e: ZipError) -> SdkError {
    SdkError::Io(std::io::Error::other(e))
}
