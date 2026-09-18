//! Explicit-file, read-only adapter for the pure source-edit proposal API.

use std::fs::{self, Metadata, OpenOptions};
use std::io::Read;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(target_os = "linux")]
use std::os::unix::fs::OpenOptionsExt;

use fe2o3_source_isa_observation::multilevel_authoring_v1::{
    AuthoringRegionSelectorV1, AuthoringSnapshotV1,
};
use fe2o3_source_isa_observation::source_edit_v1::{
    MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1, SourceEditProposalV1, SourceEditRangeV1,
    SourceEditRequestV1, prepare_source_edit_v1, validate_source_edit_path_v1,
};

pub(super) fn validate_arguments(source: &str, expected_sha256: &str) -> Result<(), String> {
    validate_source_edit_path_v1(source).map_err(|error| error.to_string())?;
    if expected_sha256.len() != 64
        || !expected_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("source baseline must be a canonical lowercase SHA-256 digest".into());
    }
    Ok(())
}

pub(super) fn prepare(
    snapshot: &AuthoringSnapshotV1,
    selector: &AuthoringRegionSelectorV1,
    helper: &str,
    source: &str,
    expected_sha256: &str,
) -> Result<SourceEditProposalV1, String> {
    validate_arguments(source, expected_sha256)?;
    let region = snapshot
        .select_region(selector)
        .map_err(|error| error.to_string())?;
    let mut selected = None;
    for operation in &region.operations {
        let span = match operation.source_spans.as_slice() {
            [span] => span,
            [] => return Err("selected operation has no source span".into()),
            _ => {
                return Err(
                    "selection must have one identical source span for every operation".into(),
                );
            }
        };
        if selected.is_some_and(|previous| previous != span) {
            return Err("selection must have one identical source span for every operation".into());
        }
        selected = Some(span);
    }
    let span = selected.ok_or("selected operation has no source span")?;
    // Only the caller's explicit path is opened. The source-map display label
    // below remains inert data, even if it contains an absolute/remapped path.
    let original = read_source(source)?;
    let source_bytes = original.len() as u32;
    let request = SourceEditRequestV1 {
        selector: selector.clone(),
        relative_path: source.to_owned(),
        expected_source_sha256: expected_sha256.to_owned(),
        expected_source_bytes: source_bytes,
        source_file_identity: span.file_identity.clone(),
        source_display_path: span.display_path.clone(),
        insertion: SourceEditRangeV1 {
            start: source_bytes,
            end: source_bytes,
        },
        helper_name: helper.to_owned(),
    };
    prepare_source_edit_v1(snapshot, &request, &original).map_err(|error| error.to_string())
}

fn read_source(source: &str) -> Result<Vec<u8>, String> {
    validate_source_edit_path_v1(source).map_err(|error| error.to_string())?;
    let path = Path::new(source);
    // Reject static symlink ancestors; this is a read-only path and not a
    // filesystem mutation capability or a proof of source association.
    for ancestor in path
        .ancestors()
        .skip(1)
        .filter(|path| !path.as_os_str().is_empty())
    {
        let metadata = fs::symlink_metadata(ancestor)
            .map_err(|error| format!("cannot inspect source parent: {error}"))?;
        if !metadata.file_type().is_dir() {
            return Err("source parent must be an ordinary directory".into());
        }
    }
    let before =
        fs::symlink_metadata(path).map_err(|error| format!("cannot inspect source: {error}"))?;
    if !before.file_type().is_file() {
        return Err("source must be an ordinary regular file".into());
    }
    if before.len() > MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 as u64 {
        return Err("source exceeds the 1 MiB preview limit".into());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    // Avoid following a swapped final symlink or blocking on a swapped FIFO.
    options.custom_flags(0x20000 | 0x800); // O_NOFOLLOW | O_NONBLOCK
    let mut file = options
        .open(path)
        .map_err(|error| format!("cannot open source: {error}"))?;
    let opened = file
        .metadata()
        .map_err(|error| format!("cannot inspect opened source: {error}"))?;
    if !opened.is_file() || !same_snapshot(&before, &opened) {
        return Err("source changed while opening".into());
    }
    let mut original = Vec::new();
    (&mut file)
        .take(MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 as u64 + 1)
        .read_to_end(&mut original)
        .map_err(|error| format!("cannot read source: {error}"))?;
    if original.len() > MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 {
        return Err("source exceeds the 1 MiB preview limit".into());
    }
    let after = file
        .metadata()
        .map_err(|error| format!("cannot inspect retained source: {error}"))?;
    if !same_snapshot(&opened, &after) || after.len() != original.len() as u64 {
        return Err("source changed while reading".into());
    }
    Ok(original)
}

fn same_snapshot(left: &Metadata, right: &Metadata) -> bool {
    if left.len() != right.len() || left.modified().ok() != right.modified().ok() {
        return false;
    }
    #[cfg(unix)]
    if left.dev() != right.dev()
        || left.ino() != right.ino()
        || left.ctime() != right.ctime()
        || left.ctime_nsec() != right.ctime_nsec()
    {
        return false;
    }
    true
}
