//! Bind the stamped original file to the live rustc semantic provenance domain.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticSourceOriginV1;
use rustc_span::{FileName, SourceFile, Span};
use std::io::Read as _;

const MAX_ACTIVE_SOURCE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy)]
pub(super) struct ActiveSource {
    whole_file: SemanticSourceOriginV1,
}

impl ActiveSource {
    pub(super) fn check_origin(&self, origin: &SemanticSourceOriginV1) -> Result<(), String> {
        if origin.file() != self.whole_file.file() {
            return Err("candidate span is not the exact active source file".into());
        }
        let (start, end) = origin.byte_range();
        let (file_start, file_end) = self.whole_file.byte_range();
        if start < file_start
            || start >= end
            || end > file_end
            || origin.start_coordinate() < self.whole_file.start_coordinate()
            || origin.end_coordinate() > self.whole_file.end_coordinate()
        {
            return Err("candidate span is not a nonempty range of the active source file".into());
        }
        Ok(())
    }
}

fn unique_file<'a, T>(
    active: &Path,
    candidates: impl IntoIterator<Item = (Option<&'a Path>, T)>,
) -> Result<T, String> {
    let mut selected = None;
    for (path, file) in candidates {
        let Some(path) = path.and_then(|path| path.canonicalize().ok()) else {
            continue;
        };
        if path != active {
            continue;
        }
        if selected.replace(file).is_some() {
            return Err("ambiguous active source file in actual SourceMap".into());
        }
    }
    selected.ok_or_else(|| "active source file missing from actual SourceMap".into())
}

fn read_original(file: &SourceFile, stamp: &Stamp) -> Result<(), String> {
    let length = file.unnormalized_source_len as usize;
    if length == 0 || length > MAX_ACTIVE_SOURCE_BYTES {
        return Err("active source byte bound exceeded".into());
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let input = options.open(&stamp.path).map_err(|e| e.to_string())?;
    let metadata = input.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() != length as u64 {
        return Err("active source is not the exact original regular file length".into());
    }
    let mut original = String::new();
    input
        .take(length as u64 + 1)
        .read_to_string(&mut original)
        .map_err(|e| e.to_string())?;
    if original.len() != length
        || digest(original.as_bytes()) != stamp.sha256
        || !file.src_hash.matches(&original)
    {
        return Err("active source bytes disagree with request or rustc input".into());
    }
    Ok(())
}

pub(super) fn resolve(tcx: TyCtxt<'_>, stamp: &Stamp) -> Result<ActiveSource, String> {
    let active = stamp.path.canonicalize().map_err(|e| e.to_string())?;
    if active != stamp.path {
        return Err("active source stamp path is not canonical".into());
    }
    // Release the SourceMap files guard before the adapter performs its own lookup.
    let file = {
        let files = tcx.sess.source_map().files();
        unique_file(
            &active,
            files.iter().map(|file| {
                let path = match &file.name {
                    FileName::Real(name) => name.local_path(),
                    _ => None,
                };
                (path, file)
            }),
        )?
        .clone()
    };
    read_original(&file, stamp)?;
    let span = Span::with_root_ctxt(file.start_pos, file.end_position());
    let provenance = crate::rustc_semantic_adapter_v1::canonical_source_provenance_v1(tcx, span, 0)
        .map_err(|e| format!("active source provenance: {e}"))?
        .provenance();
    let whole_file = provenance
        .call_site()
        .ok_or("active file has no canonical origin")?;
    let length = u64::from((file.end_position() - file.start_pos).0);
    if length == 0
        || provenance.expansion() != Some(whole_file)
        || whole_file.byte_range() != (0, length)
    {
        return Err("active file canonical provenance range mismatch".into());
    }
    Ok(ActiveSource { whole_file })
}

#[test]
fn active_source_selection_requires_one_exact_local_file() {
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-policy7-source-selection");
    let active = scratch.path().join("active.rs");
    let other = scratch.path().join("other.rs");
    std::fs::write(&active, "active").unwrap();
    std::fs::write(&other, "other").unwrap();
    let active = active.canonicalize().unwrap();
    assert_eq!(
        unique_file(
            &active,
            [(Some(other.as_path()), 1), (Some(active.as_path()), 2)]
        )
        .unwrap(),
        2
    );
    assert!(unique_file(&active, [(Some(other.as_path()), 1), (None, 2)]).is_err());
    assert!(
        unique_file(
            &active,
            [(Some(active.as_path()), 1), (Some(active.as_path()), 2)]
        )
        .is_err()
    );
    let missing = scratch.path().join("missing.rs");
    assert!(unique_file(&active, [(Some(missing.as_path()), 1)]).is_err());
}

#[test]
fn active_source_original_bytes_refuse_stale_normalized_and_nonregular_inputs() {
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-policy7-source-original");
    let original = "\u{feff}let value = 1;\r\n";
    let file = SourceFile::new(
        FileName::Custom("fixture.rs".into()),
        original.into(),
        rustc_span::SourceFileHashAlgorithm::Sha256,
        None,
    )
    .unwrap();
    let path = scratch.path().join("fixture.rs");
    std::fs::write(&path, original).unwrap();
    let stamp = Stamp {
        path,
        sha256: digest(original.as_bytes()),
    };
    read_original(&file, &stamp).unwrap();
    assert!(
        read_original(
            &file,
            &Stamp {
                sha256: [0; 32],
                ..stamp.clone()
            }
        )
        .is_err()
    );
    let stale = original.replace('1', "2");
    std::fs::write(&stamp.path, &stale).unwrap();
    assert!(
        read_original(
            &file,
            &Stamp {
                sha256: digest(stale.as_bytes()),
                ..stamp.clone()
            }
        )
        .is_err()
    );
    std::fs::write(&stamp.path, "let value = 1;\n").unwrap();
    assert!(read_original(&file, &stamp).is_err());
    assert!(
        read_original(
            &file,
            &Stamp {
                path: scratch.path().to_owned(),
                ..stamp
            }
        )
        .is_err()
    );
}

#[test]
fn active_source_origin_requires_semantic_identity_and_nonempty_bounded_range() {
    use fe2o3_mir_model::semantic_mir_v1::SemanticSourceFileIdentityV1;
    let semantic = SemanticSourceFileIdentityV1::from_sha256([7; 32]);
    let active = ActiveSource {
        whole_file: SemanticSourceOriginV1::new(semantic, 0, 100, 1, 0, 10, 0).unwrap(),
    };
    let origin =
        |file, start, end| SemanticSourceOriginV1::new(file, start, end, 2, 0, 2, 1).unwrap();
    active.check_origin(&origin(semantic, 10, 20)).unwrap();
    for (start, end) in [(10, 10), (0, 101), (100, 101)] {
        assert!(active.check_origin(&origin(semantic, start, end)).is_err());
    }
    let content_hash = SemanticSourceFileIdentityV1::from_sha256(digest(b"active file bytes"));
    assert_ne!(semantic, content_hash);
    assert!(active.check_origin(&origin(content_hash, 10, 20)).is_err());
    let wrong_lines = SemanticSourceOriginV1::new(semantic, 10, 20, 11, 0, 11, 1).unwrap();
    assert!(active.check_origin(&wrong_lines).is_err());
}
