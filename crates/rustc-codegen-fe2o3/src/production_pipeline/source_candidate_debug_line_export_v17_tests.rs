//! Fixed test-only publication from the live source/V17 callback.
//! These files are inert observations. Nothing decodes them into compiler owners.
use super::*;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

pub(crate) const LLVM_BYTES: usize = 64 * 1024;
pub(crate) const EXPECTED_BYTES: usize = 16 * 1024;
const EXTRA_LOGICAL_BYTES: usize = 256 * 1024;
pub(crate) const SOURCE_BYTES: usize = 128 * 1024;

pub(super) fn prepay() -> Result<usize, String> {
    prepay_fixed_envelope()?
        .checked_add(EXTRA_LOGICAL_BYTES)
        .filter(|n| *n <= LOGICAL_ENVELOPE)
        .ok_or_else(|| "line export exceeds original cumulative128MiB envelope".into())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}
fn paths(edited: bool) -> (&'static str, &'static str) {
    if edited {
        ("edited.ll", "edited.expected.json")
    } else {
        ("default.ll", "default.expected.json")
    }
}
fn directory(path: &Path) -> Result<(), String> {
    if !path.is_absolute()
        || path.as_os_str().len() > 4096
        || !std::fs::symlink_metadata(path)
            .map_err(|e| e.to_string())?
            .is_dir()
        || path.canonicalize().map_err(|e| e.to_string())? != path
    {
        return Err("line export requires exact real bounded output directory".into());
    }
    Ok(())
}
fn identity(m: &std::fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
fn compare(file: &mut File, expected: &[u8]) -> Result<(), String> {
    let before = file.metadata().map_err(|e| e.to_string())?;
    if !before.is_file() || before.len() != expected.len() as u64 {
        return Err("line export output shape changed".into());
    }
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut buffer = [0u8; 4096];
    let mut offset = 0usize;
    for _ in 0..64 {
        if offset == expected.len() {
            break;
        }
        let count = (expected.len() - offset).min(buffer.len());
        match file.read(&mut buffer[..count]) {
            Ok(0) => return Err("line export readback truncated".into()),
            Ok(n) => {
                if buffer[..n] != expected[offset..offset + n] {
                    return Err("line export full readback differs".into());
                }
                offset += n;
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    if offset != expected.len() {
        return Err("line export read-call cap".into());
    }
    // One separately bounded EOF call; EINTR here is a refusal, not a retry.
    if file.read(&mut buffer[..1]).map_err(|e| e.to_string())? != 0
        || identity(&file.metadata().map_err(|e| e.to_string())?) != identity(&before)
    {
        return Err("line export EOF/identity changed".into());
    }
    Ok(())
}
fn publish_file(directory: &Path, name: &str, bytes: &[u8], cap: usize) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > cap {
        return Err("line export byte cap".into());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(directory.join(name))
        .map_err(|e| e.to_string())?;
    let mut offset = 0usize;
    for _ in 0..64 {
        if offset == bytes.len() {
            break;
        }
        match file.write(&bytes[offset..(offset + 4096).min(bytes.len())]) {
            Ok(0) => return Err("line export write made no progress".into()),
            Ok(n) => offset += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    if offset != bytes.len() {
        return Err("line export write-call cap".into());
    }
    file.sync_all().map_err(|e| e.to_string())?;
    compare(&mut file, bytes)
}

pub(super) fn publish(
    directory_path: &Path,
    edited: bool,
    owner: &OrderedProgramObservationOwnerV32,
    llvm: &str,
    catalog: &DebugSourceCatalogV1,
    selected: SimulationDebugSiteV1,
    source_sha256: [u8; 32],
    source_bytes: usize,
) -> Result<Value, String> {
    prepay()?;
    directory(directory_path)?;
    if llvm.is_empty()
        || llvm.len() > LLVM_BYTES
        || source_bytes == 0
        || source_bytes > SOURCE_BYTES
    {
        return Err("line export LLVM/source byte profile".into());
    }
    let executable = owner.materialized().executable();
    let function = executable
        .module()
        .functions
        .get(selected.function_ordinal)
        .ok_or("line export selected function absent")?;
    let body = function.body.as_ref().ok_or("line export body absent")?;
    let (block_ordinal, block) = body
        .blocks
        .iter()
        .enumerate()
        .find(|(_, block)| block.id == selected.block)
        .ok_or("line export block absent")?;
    let operation = block
        .operations
        .get(selected.operation as usize)
        .ok_or("line export operation absent")?;
    let OperationKind::Gfx942OrderedProgram(region) = &operation.kind else {
        return Err("line export operation is not actual ordered region".into());
    };
    let mut mapped = catalog.sites().iter().filter(|s| s.site == selected);
    let mapped_site = mapped.next().ok_or("line export span absent")?;
    if mapped.next().is_some() {
        return Err("line export ambiguous site".into());
    }
    let [span] = mapped_site.spans.as_slice() else {
        return Err("line export ambiguous span".into());
    };
    let mut matching_files = catalog.files().iter().filter(|f| f.identity == span.file);
    let file = matching_files.next().ok_or("line export file absent")?;
    if matching_files.next().is_some() || file.byte_len != source_bytes as u64 {
        return Err("line export exact source file differs".into());
    }
    let source = region.source();
    let expected = json!({
        "schema":"private-ordered-region-line-export-v17",
        "scenario":if edited {"edited"} else {"default"},
        "llvm":{"bytes":llvm.len(),"sha256":digest(llvm.as_bytes())},
        "canonical":{"bytes":executable.canonical_bytes().len(),
            "sha256":hex(executable.identity().digest())},
        "source_bytes":{"bytes":source_bytes,"sha256":hex(&source_sha256)},
        "source_identity":{"frontend_unit":hex(&source.frontend_unit),
            "function":hex(&source.function),"contract":hex(&source.contract),
            "statement":hex(&source.statement)},
        "kir_site":{"function_ordinal":selected.function_ordinal,
            "block_ordinal":block_ordinal,"block_id":selected.block.0,
            "operation_ordinal":selected.operation},
        "span":{"file_identity":hex(&span.file),"display_path":file.display_path,
            "file_bytes":file.byte_len,"byte_start":span.byte_start,"byte_end":span.byte_end,
            "line":span.line,"column":span.column},
        "target":"gfx942:xnack-","line_scope":"whole-ordered-region",
        "source_custody_checked_in_callback":true,
        "source_postflight_required":true,
        "native_emitted":false,"hardware_observed":false,
        "grants_artifact_or_launch_authority":false,
    });
    let expected_bytes = serde_json::to_vec_pretty(&expected).map_err(|e| e.to_string())?;
    if expected_bytes.len() > EXPECTED_BYTES {
        return Err("line export metadata byte cap".into());
    }
    let (llvm_file, expected_file) = paths(edited);
    // Original owner and catalog are still borrowed throughout both exclusive
    // writes and full readbacks. The enclosing fresh-source helper rechecks its
    // retained source after this callback; failure never promotes these files.
    publish_file(directory_path, llvm_file, llvm.as_bytes(), LLVM_BYTES)?;
    publish_file(
        directory_path,
        expected_file,
        &expected_bytes,
        EXPECTED_BYTES,
    )?;
    Ok(json!({"llvm_file":llvm_file,"llvm":expected["llvm"],
        "expected_file":expected_file,
        "expected":{"bytes":expected_bytes.len(),"sha256":digest(&expected_bytes)},
        "prepaid_combined_logical_payload":prepay()?,
        "combined_logical_payload_limit":LOGICAL_ENVELOPE,
        "publication_is_source_or_native_authority":false}))
}

fn read_file(directory: &Path, name: &str, cap: usize) -> Result<Vec<u8>, String> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(directory.join(name))
        .map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > cap as u64 {
        return Err("line export read profile".into());
    }
    let mut bytes = vec![0; metadata.len() as usize];
    let mut offset = 0;
    for _ in 0..64 {
        if offset == bytes.len() {
            break;
        }
        let end = (offset + 4096).min(bytes.len());
        match file.read(&mut bytes[offset..end]) {
            Ok(0) => return Err("line export read truncated".into()),
            Ok(n) => offset += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    if offset != bytes.len()
        || identity(&file.metadata().map_err(|e| e.to_string())?) != identity(&metadata)
    {
        return Err("line export read cap/identity".into());
    }
    let mut eof = [0; 1];
    if file.read(&mut eof).map_err(|e| e.to_string())? != 0
        || identity(&file.metadata().map_err(|e| e.to_string())?) != identity(&metadata)
    {
        return Err("line export appended".into());
    }
    Ok(bytes)
}

pub(crate) fn verify_export(
    directory_path: &Path,
    edited: bool,
    report: &Value,
    source_sha256: &str,
    canonical_digest: [u8; 32],
) -> Result<Value, String> {
    prepay()?;
    directory(directory_path)?;
    let (llvm_file, expected_file) = paths(edited);
    if report["llvm_file"] != llvm_file
        || report["expected_file"] != expected_file
        || report["publication_is_source_or_native_authority"] != false
    {
        return Err("line export report shape differs".into());
    }
    let llvm = read_file(directory_path, llvm_file, LLVM_BYTES)?;
    let bytes = read_file(directory_path, expected_file, EXPECTED_BYTES)?;
    validate_payload_joins(
        edited,
        report,
        source_sha256,
        canonical_digest,
        &llvm,
        &bytes,
    )
}

fn validate_payload_joins(
    edited: bool,
    report: &Value,
    source_sha256: &str,
    canonical_digest: [u8; 32],
    llvm: &[u8],
    bytes: &[u8],
) -> Result<Value, String> {
    if llvm.is_empty()
        || llvm.len() > LLVM_BYTES
        || bytes.is_empty()
        || bytes.len() > EXPECTED_BYTES
    {
        return Err("line export bounded payload join".into());
    }
    let (llvm_file, expected_file) = paths(edited);
    if report["llvm_file"] != llvm_file
        || report["expected_file"] != expected_file
        || report["publication_is_source_or_native_authority"] != false
    {
        return Err("line export report shape differs".into());
    }
    if report["llvm"]["bytes"] != llvm.len()
        || report["llvm"]["sha256"] != digest(&llvm)
        || report["expected"]["bytes"] != bytes.len()
        || report["expected"]["sha256"] != digest(&bytes)
    {
        return Err("line export payload differs from actual callback".into());
    }
    let expected: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if expected["schema"] != "private-ordered-region-line-export-v17"
        || expected["scenario"] != if edited { "edited" } else { "default" }
        || expected["llvm"] != report["llvm"]
        || expected["source_bytes"]["sha256"] != source_sha256
        || expected["canonical"]["sha256"] != hex(&canonical_digest)
        || expected["source_custody_checked_in_callback"] != true
        || expected["source_postflight_required"] != true
        || expected["native_emitted"] != false
        || expected["hardware_observed"] != false
        || expected["grants_artifact_or_launch_authority"] != false
    {
        return Err("line export source/canonical/authority join differs".into());
    }
    Ok(expected)
}

// Preserve the original <=64KiB child JSON line, not a reserialized report.
pub(crate) fn publish_pair(directory_path: &Path, original_line: &[u8]) -> Result<(), String> {
    prepay()?;
    directory(directory_path)?;
    publish_file(
        directory_path,
        "line-export-pair-observation.json",
        original_line,
        LLVM_BYTES,
    )
}

#[test]
fn line_export_cap_and_names_are_closed() {
    assert!(prepay().unwrap() <= LOGICAL_ENVELOPE);
    assert_eq!(LLVM_BYTES, 65_536);
    assert_eq!(EXPECTED_BYTES, 16_384);
    assert_eq!(paths(false), ("default.ll", "default.expected.json"));
    assert_eq!(paths(true), ("edited.ll", "edited.expected.json"));
    assert_eq!(SOURCE_BYTES, 131_072);
}

#[cfg(test)]
fn synthetic_payloads() -> (Vec<u8>, Vec<u8>, Value) {
    // Only byte-join controls: no synthetic compiler/source owner is constructed.
    let llvm = b"synthetic inert LLVM text\n".to_vec();
    let expected = json!({
        "schema":"private-ordered-region-line-export-v17","scenario":"edited",
        "llvm":{"bytes":llvm.len(),"sha256":digest(&llvm)},
        "source_bytes":{"bytes":3,"sha256":hex(&[1;32])},
        "canonical":{"bytes":4,"sha256":hex(&[2;32])},
        "source_custody_checked_in_callback":true,"source_postflight_required":true,
        "native_emitted":false,"hardware_observed":false,
        "grants_artifact_or_launch_authority":false,
    });
    let bytes = serde_json::to_vec(&expected).unwrap();
    let report = json!({"llvm_file":"edited.ll","llvm":expected["llvm"],
        "expected_file":"edited.expected.json",
        "expected":{"bytes":bytes.len(),"sha256":digest(&bytes)},
        "publication_is_source_or_native_authority":false});
    (llvm, bytes, report)
}
#[test]
fn line_export_byte_join_positive_is_not_a_source_owner() {
    let (llvm, bytes, report) = synthetic_payloads();
    assert!(validate_payload_joins(true, &report, &hex(&[1; 32]), [2; 32], &llvm, &bytes).is_ok());
}
#[test]
fn line_export_full_llvm_and_metadata_corruption_refuse() {
    let (mut llvm, mut bytes, report) = synthetic_payloads();
    llvm[0] ^= 1;
    assert_eq!(
        validate_payload_joins(true, &report, &hex(&[1; 32]), [2; 32], &llvm, &bytes).unwrap_err(),
        "line export payload differs from actual callback"
    );
    llvm[0] ^= 1;
    bytes[0] ^= 1;
    assert_eq!(
        validate_payload_joins(true, &report, &hex(&[1; 32]), [2; 32], &llvm, &bytes).unwrap_err(),
        "line export payload differs from actual callback"
    );
}
#[test]
fn line_export_stale_source_and_canonical_joins_refuse() {
    let (llvm, bytes, report) = synthetic_payloads();
    for (source, canonical) in [(hex(&[3; 32]), [2; 32]), (hex(&[1; 32]), [3; 32])] {
        assert_eq!(
            validate_payload_joins(true, &report, &source, canonical, &llvm, &bytes).unwrap_err(),
            "line export source/canonical/authority join differs"
        );
    }
}
#[test]
fn line_export_rebound_authority_and_scenario_refuse() {
    let (llvm, bytes, mut report) = synthetic_payloads();
    for field in [
        "native_emitted",
        "hardware_observed",
        "grants_artifact_or_launch_authority",
    ] {
        let mut expected: Value = serde_json::from_slice(&bytes).unwrap();
        expected[field] = json!(true);
        let changed = serde_json::to_vec(&expected).unwrap();
        report["expected"] = json!({"bytes":changed.len(),"sha256":digest(&changed)});
        assert_eq!(
            validate_payload_joins(true, &report, &hex(&[1; 32]), [2; 32], &llvm, &changed)
                .unwrap_err(),
            "line export source/canonical/authority join differs"
        );
    }
    assert_eq!(
        validate_payload_joins(false, &report, &hex(&[1; 32]), [2; 32], &llvm, &bytes).unwrap_err(),
        "line export report shape differs"
    );
}
#[test]
fn line_export_empty_and_oversized_payloads_refuse_before_parsing() {
    let (llvm, bytes, report) = synthetic_payloads();
    for (llvm, bytes) in [
        (Vec::new(), bytes.clone()),
        (vec![0; LLVM_BYTES + 1], bytes),
        (llvm.clone(), Vec::new()),
        (llvm, vec![0; EXPECTED_BYTES + 1]),
    ] {
        assert_eq!(
            validate_payload_joins(true, &report, &hex(&[1; 32]), [2; 32], &llvm, &bytes)
                .unwrap_err(),
            "line export bounded payload join"
        );
    }
}
