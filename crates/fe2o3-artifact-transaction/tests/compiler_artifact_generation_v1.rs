use fe2o3_artifact_transaction::{
    CompilerArtifactGenerationErrorV1, CompilerArtifactGenerationFaultPointV1,
    CompilerArtifactGenerationFaultTimingV1, CompilerArtifactGenerationManifestV1,
    CompilerArtifactGenerationObjectBoundaryV1, CompilerArtifactGenerationObjectV1,
    CompilerArtifactGenerationObservationV1, CompilerArtifactGenerationOptionsV1,
    CompilerArtifactGenerationPublishOutcomeV1, CompilerArtifactGenerationQuotaV1,
    CompilerArtifactGenerationRecordBoundaryV1, CompilerArtifactGenerationRecordOperationV1,
    CompilerArtifactGenerationRequestV1, CompilerArtifactGenerationScopeV1,
    CompilerArtifactGenerationStoreV1, CompilerArtifactRoleV1,
    MAX_COMPILER_ARTIFACT_GENERATION_BYTES_V1,
    MAX_COMPILER_ARTIFACT_GENERATION_SCOPE_RECORD_BYTES_V1, MAX_COMPILER_HSACO_BYTES_V1,
    MAX_COMPILER_LINEAGE_BYTES_V1, MAX_COMPILER_NEUTRAL_KIR_BYTES_V1,
    MAX_COMPILER_SEMANTIC_MIR_BYTES_V1, MAX_COMPILER_TARGET_KIR_BYTES_V1,
};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

const BLOB_PREFIX: &str = ".fe2o3-compiler-generation-v1-blob-";
const MANIFEST_PREFIX: &str = ".fe2o3-compiler-generation-v1-manifest-";
const SCOPE_PREFIX: &str = ".fe2o3-compiler-generation-v1-scope-";
const RECORD_SUFFIX: &str = ".record";
const REDO_SUFFIX: &str = ".redo";
const LOCK_FILE: &str = ".fe2o3-artifacts.lock";
const MANIFEST_ENTRY_BYTES: usize = 1 + 8 + 32;
const SCOPE_RECORD_MAGIC: &[u8] = b"FE2O3-COMPILER-ARTIFACT-SCOPE-V1\0";
const SCOPE_RECORD_CHECKSUM_DOMAIN: &[u8] =
    b"fe2o3.compiler-artifact-generation.scope-record-checksum.v1\0";

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        loop {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "fe2o3-compiler-generation-v1-test-{}-{id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
                    return Self { path };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("failed to create test directory: {error}"),
            }
        }
    }

    fn store(&self) -> CompilerArtifactGenerationStoreV1 {
        CompilerArtifactGenerationStoreV1::open(&self.path, scope()).unwrap()
    }

    fn store_with_quota(
        &self,
        maximum_bytes: u64,
        maximum_entries: usize,
    ) -> CompilerArtifactGenerationStoreV1 {
        CompilerArtifactGenerationStoreV1::open_with_quota(
            &self.path,
            scope(),
            CompilerArtifactGenerationQuotaV1::new(maximum_bytes, maximum_entries).unwrap(),
        )
        .unwrap()
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(0o700));
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn scope() -> CompilerArtifactGenerationScopeV1 {
    CompilerArtifactGenerationScopeV1::from_bytes([0x51; 32])
}

struct FixtureBytes {
    mir: &'static [u8],
    neutral: &'static [u8],
    target: &'static [u8],
    lineage: &'static [u8],
    code: &'static [u8],
}

fn request(seed: u8, hsaco: bool) -> CompilerArtifactGenerationRequestV1<'static> {
    let bytes = match seed {
        1 => FixtureBytes {
            mir: b"semantic-mir-generation-a",
            neutral: b"neutral-kir-generation-a",
            target: b"target-kir-generation-a",
            lineage: b"lineage-generation-a",
            code: b"hsaco-generation-a",
        },
        2 => FixtureBytes {
            mir: b"semantic-mir-generation-b",
            neutral: b"neutral-kir-generation-b",
            target: b"target-kir-generation-b",
            lineage: b"lineage-generation-b",
            code: b"hsaco-generation-b",
        },
        3 => FixtureBytes {
            mir: b"semantic-mir-generation-c",
            neutral: b"neutral-kir-generation-c",
            target: b"target-kir-generation-c",
            lineage: b"lineage-generation-c",
            code: b"hsaco-generation-c",
        },
        _ => panic!("unknown fixture seed"),
    };
    CompilerArtifactGenerationRequestV1::new(
        [seed; 32],
        [seed.wrapping_add(10); 32],
        [seed.wrapping_add(20); 32],
        bytes.mir,
        bytes.neutral,
        bytes.target,
        bytes.lineage,
        hsaco.then_some(bytes.code),
    )
}

fn shared_payload_request() -> CompilerArtifactGenerationRequestV1<'static> {
    const SHARED: &[u8] = b"one-content-address-shared-by-four-roles";
    CompilerArtifactGenerationRequestV1::new(
        [0x41; 32], [0x42; 32], [0x43; 32], SHARED, SHARED, SHARED, SHARED, None,
    )
}

fn committed(
    outcome: CompilerArtifactGenerationPublishOutcomeV1,
) -> fe2o3_artifact_transaction::CompilerArtifactGenerationLeaseV1 {
    match outcome {
        CompilerArtifactGenerationPublishOutcomeV1::Committed(lease) => lease,
        other => panic!("expected committed generation, got {other:?}"),
    }
}

fn entries_with_prefix(root: &Path, prefix: &str) -> Vec<PathBuf> {
    let mut entries = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn managed_entries(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with(BLOB_PREFIX)
                        || name.starts_with(MANIFEST_PREFIX)
                        || name.starts_with(SCOPE_PREFIX)
                })
        })
        .collect()
}

fn deleted_generation_descriptors(root: &Path) -> Vec<PathBuf> {
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|entry| fs::read_link(entry.path()).ok())
        .filter(|target| {
            target.starts_with(root) && target.to_string_lossy().ends_with(" (deleted)")
        })
        .collect()
}

fn canonical_record(root: &Path) -> PathBuf {
    entries_with_prefix(root, SCOPE_PREFIX)
        .into_iter()
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(RECORD_SUFFIX))
        })
        .expect("canonical scope record")
}

fn canonical_record_for_scope(root: &Path, scope: CompilerArtifactGenerationScopeV1) -> PathBuf {
    let scope_offset = SCOPE_RECORD_MAGIC.len() + 2;
    entries_with_prefix(root, SCOPE_PREFIX)
        .into_iter()
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(RECORD_SUFFIX))
        })
        .find(|path| {
            fs::read(path).ok().is_some_and(|bytes| {
                bytes.get(scope_offset..scope_offset + 32) == Some(&scope.as_bytes())
            })
        })
        .expect("canonical scope record")
}

fn redo_record(root: &Path) -> PathBuf {
    entries_with_prefix(root, SCOPE_PREFIX)
        .into_iter()
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(REDO_SUFFIX))
        })
        .expect("scope redo record")
}

fn blob_for(root: &Path, bytes: &[u8]) -> PathBuf {
    let digest = Sha256::digest(bytes);
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    root.join(format!("{BLOB_PREFIX}{hex}.bin"))
}

fn replace_named_lock(root: &Path) -> PathBuf {
    let named = root.join(LOCK_FILE);
    let displaced = root.join("displaced-active-generation-lock");
    fs::rename(&named, &displaced).unwrap();
    fs::write(&named, b"replacement lock inode").unwrap();
    fs::set_permissions(&named, fs::Permissions::from_mode(0o600)).unwrap();
    displaced
}

fn assert_generation(
    lease: &fe2o3_artifact_transaction::CompilerArtifactGenerationLeaseV1,
    seed: u8,
    hsaco: bool,
) {
    let expected = request(seed, hsaco);
    for role in [
        CompilerArtifactRoleV1::SemanticMir,
        CompilerArtifactRoleV1::NeutralKir,
        CompilerArtifactRoleV1::TargetKir,
        CompilerArtifactRoleV1::Lineage,
        CompilerArtifactRoleV1::Hsaco,
    ] {
        assert_eq!(lease.artifact(role), expected.artifact(role), "{role:?}");
    }
    assert_eq!(lease.manifest().compiler_identity(), [seed; 32]);
    assert!(!lease.grants_verification_authority());
    assert!(!lease.grants_load_authority());
    assert!(!lease.grants_launch_authority());
}

#[test]
fn canonical_manifest_is_deterministic_domain_separated_and_round_trips() {
    let generation = request(1, true);
    let first = CompilerArtifactGenerationManifestV1::for_request(scope(), &generation).unwrap();
    let second = CompilerArtifactGenerationManifestV1::for_request(scope(), &generation).unwrap();
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    assert_eq!(first.identity(), second.identity());
    assert_eq!(
        CompilerArtifactGenerationManifestV1::decode_canonical(first.canonical_bytes()).unwrap(),
        first
    );
    assert_ne!(
        first.identity().as_bytes(),
        Sha256::digest(first.canonical_bytes()).as_slice()
    );

    let changed =
        CompilerArtifactGenerationManifestV1::for_request(scope(), &request(2, true)).unwrap();
    let no_hsaco =
        CompilerArtifactGenerationManifestV1::for_request(scope(), &request(1, false)).unwrap();
    assert_ne!(first.identity(), changed.identity());
    assert_ne!(first.identity(), no_hsaco.identity());
    assert_eq!(first.entries().len(), 5);
    assert_eq!(no_hsaco.entries().len(), 4);
}

#[test]
fn manifest_decoder_rejects_missing_duplicate_reordered_unknown_and_trailing_roles() {
    let manifest =
        CompilerArtifactGenerationManifestV1::for_request(scope(), &request(1, true)).unwrap();
    let canonical = manifest.canonical_bytes();
    let count_offset = canonical.len() - 5 * MANIFEST_ENTRY_BYTES - 1;
    assert_eq!(canonical[count_offset], 5);
    let first = count_offset + 1;
    let second = first + MANIFEST_ENTRY_BYTES;

    let mut missing = canonical.to_vec();
    missing[count_offset] = 4;
    assert!(CompilerArtifactGenerationManifestV1::decode_canonical(&missing).is_err());

    let mut duplicate = canonical.to_vec();
    duplicate[second] = duplicate[first];
    assert!(CompilerArtifactGenerationManifestV1::decode_canonical(&duplicate).is_err());

    let mut reordered = canonical.to_vec();
    for offset in 0..MANIFEST_ENTRY_BYTES {
        reordered.swap(first + offset, second + offset);
    }
    assert!(CompilerArtifactGenerationManifestV1::decode_canonical(&reordered).is_err());

    let mut unknown = canonical.to_vec();
    unknown[first] = 0xff;
    assert!(CompilerArtifactGenerationManifestV1::decode_canonical(&unknown).is_err());

    let mut trailing = canonical.to_vec();
    trailing.push(0);
    assert!(CompilerArtifactGenerationManifestV1::decode_canonical(&trailing).is_err());
}

#[test]
fn role_and_aggregate_bounds_are_checked_before_publication() {
    assert_eq!(
        MAX_COMPILER_ARTIFACT_GENERATION_BYTES_V1,
        MAX_COMPILER_SEMANTIC_MIR_BYTES_V1
            + MAX_COMPILER_NEUTRAL_KIR_BYTES_V1
            + MAX_COMPILER_TARGET_KIR_BYTES_V1
            + MAX_COMPILER_LINEAGE_BYTES_V1
            + MAX_COMPILER_HSACO_BYTES_V1
    );

    let directory = TestDirectory::new();
    let store = directory.store();
    let empty = CompilerArtifactGenerationRequestV1::new(
        [1; 32], [2; 32], [3; 32], b"", b"neutral", b"target", b"lineage", None,
    );
    assert!(matches!(
        store.publish_generation_v1(&empty),
        CompilerArtifactGenerationPublishOutcomeV1::NotCommitted(
            CompilerArtifactGenerationErrorV1::Bounds {
                role: Some(CompilerArtifactRoleV1::SemanticMir),
                ..
            }
        )
    ));
    assert!(managed_entries(&directory.path).is_empty());

    let oversized_lineage = vec![0; MAX_COMPILER_LINEAGE_BYTES_V1 + 1];
    let oversized = CompilerArtifactGenerationRequestV1::new(
        [1; 32],
        [2; 32],
        [3; 32],
        b"mir",
        b"neutral",
        b"target",
        &oversized_lineage,
        None,
    );
    assert!(matches!(
        store.publish_generation_v1(&oversized),
        CompilerArtifactGenerationPublishOutcomeV1::NotCommitted(
            CompilerArtifactGenerationErrorV1::Bounds {
                role: Some(CompilerArtifactRoleV1::Lineage),
                ..
            }
        )
    ));
    assert!(managed_entries(&directory.path).is_empty());

    let exact_lineage = vec![0; MAX_COMPILER_LINEAGE_BYTES_V1];
    let exact = CompilerArtifactGenerationRequestV1::new(
        [1; 32],
        [2; 32],
        [3; 32],
        b"mir",
        b"neutral",
        b"target",
        &exact_lineage,
        None,
    );
    CompilerArtifactGenerationManifestV1::for_request(scope(), &exact).unwrap();

    let manifest =
        CompilerArtifactGenerationManifestV1::for_request(scope(), &request(1, true)).unwrap();
    let canonical = manifest.canonical_bytes();
    let count_offset = canonical.len() - 5 * MANIFEST_ENTRY_BYTES - 1;
    for (index, (role, maximum)) in [
        (
            CompilerArtifactRoleV1::SemanticMir,
            MAX_COMPILER_SEMANTIC_MIR_BYTES_V1,
        ),
        (
            CompilerArtifactRoleV1::NeutralKir,
            MAX_COMPILER_NEUTRAL_KIR_BYTES_V1,
        ),
        (
            CompilerArtifactRoleV1::TargetKir,
            MAX_COMPILER_TARGET_KIR_BYTES_V1,
        ),
        (
            CompilerArtifactRoleV1::Lineage,
            MAX_COMPILER_LINEAGE_BYTES_V1,
        ),
        (CompilerArtifactRoleV1::Hsaco, MAX_COMPILER_HSACO_BYTES_V1),
    ]
    .into_iter()
    .enumerate()
    {
        let length_offset = count_offset + 1 + index * MANIFEST_ENTRY_BYTES + 1;
        let mut oversized = canonical.to_vec();
        oversized[length_offset..length_offset + 8]
            .copy_from_slice(&((maximum as u64) + 1).to_le_bytes());
        assert!(matches!(
            CompilerArtifactGenerationManifestV1::decode_canonical(&oversized),
            Err(CompilerArtifactGenerationErrorV1::Bounds {
                role: Some(actual),
                ..
            }) if actual == role
        ));

        let mut empty = canonical.to_vec();
        empty[length_offset..length_offset + 8].copy_from_slice(&0u64.to_le_bytes());
        assert!(matches!(
            CompilerArtifactGenerationManifestV1::decode_canonical(&empty),
            Err(CompilerArtifactGenerationErrorV1::Bounds {
                role: Some(actual),
                ..
            }) if actual == role
        ));
    }
}

#[test]
fn four_and_five_role_generations_publish_with_one_scope_commit() {
    for hsaco in [false, true] {
        let directory = TestDirectory::new();
        let store = directory.store();
        let first = committed(store.publish_generation_v1(&request(1, hsaco)));
        assert_generation(&first, 1, hsaco);
        assert_eq!(first.manifest().entries().len(), if hsaco { 5 } else { 4 });

        let second = committed(store.publish_generation_v1(&request(1, hsaco)));
        assert_eq!(first.manifest().identity(), second.manifest().identity());
        let opened = store.open_generation_v1().unwrap().unwrap();
        assert_generation(&opened, 1, hsaco);

        assert_eq!(
            entries_with_prefix(&directory.path, MANIFEST_PREFIX).len(),
            1
        );
        assert_eq!(
            entries_with_prefix(&directory.path, BLOB_PREFIX)
                .into_iter()
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.ends_with(".bin"))
                })
                .count(),
            if hsaco { 5 } else { 4 }
        );
        let record = canonical_record(&directory.path);
        assert_eq!(
            fs::metadata(record).unwrap().permissions().mode() & 0o777,
            0o600
        );
        for path in entries_with_prefix(&directory.path, MANIFEST_PREFIX)
            .into_iter()
            .chain(entries_with_prefix(&directory.path, BLOB_PREFIX))
            .filter(|path| path.extension().is_some_and(|extension| extension == "bin"))
        {
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o400
            );
        }
    }
}

#[test]
fn prior_move_only_lease_remains_exact_after_scope_replacement() {
    let directory = TestDirectory::new();
    let store = directory.store();
    let first = committed(store.publish_generation_v1(&request(1, true)));
    let first_identity = first.manifest().identity();
    let second = committed(store.publish_generation_v1(&request(2, true)));
    assert_ne!(first_identity, second.manifest().identity());
    assert_generation(&first, 1, true);
    assert_generation(&second, 2, true);
    assert_generation(&store.open_generation_v1().unwrap().unwrap(), 2, true);
}

#[test]
fn complete_reader_rejects_blob_manifest_and_scope_record_mutation() {
    let blob_directory = TestDirectory::new();
    let blob_store = blob_directory.store();
    committed(blob_store.publish_generation_v1(&request(1, true)));
    let blob = blob_for(&blob_directory.path, b"target-kir-generation-a");
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&blob, b"target-kir-generation-z").unwrap();
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o400)).unwrap();
    assert!(matches!(
        blob_store.open_generation_v1(),
        Err(CompilerArtifactGenerationErrorV1::UnsafeEntry { .. })
    ));

    let manifest_directory = TestDirectory::new();
    let manifest_store = manifest_directory.store();
    committed(manifest_store.publish_generation_v1(&request(1, true)));
    let manifest = entries_with_prefix(&manifest_directory.path, MANIFEST_PREFIX)
        .into_iter()
        .find(|path| path.extension().is_some_and(|extension| extension == "bin"))
        .unwrap();
    let mut bytes = fs::read(&manifest).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&manifest, bytes).unwrap();
    fs::set_permissions(&manifest, fs::Permissions::from_mode(0o400)).unwrap();
    assert!(matches!(
        manifest_store.open_generation_v1(),
        Err(CompilerArtifactGenerationErrorV1::UnsafeEntry { .. })
    ));

    let record_directory = TestDirectory::new();
    let record_store = record_directory.store();
    committed(record_store.publish_generation_v1(&request(1, true)));
    let record = canonical_record(&record_directory.path);
    let mut bytes = fs::read(&record).unwrap();
    bytes[0] ^= 1;
    fs::write(record, bytes).unwrap();
    assert!(matches!(
        record_store.open_generation_v1(),
        Err(CompilerArtifactGenerationErrorV1::Codec { .. })
    ));

    let length_directory = TestDirectory::new();
    let length_store = length_directory.store();
    committed(length_store.publish_generation_v1(&request(1, true)));
    let blob = blob_for(&length_directory.path, b"semantic-mir-generation-a");
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&blob, b"truncated").unwrap();
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o400)).unwrap();
    assert!(matches!(
        length_store.open_generation_v1(),
        Err(CompilerArtifactGenerationErrorV1::UnsafeEntry { .. })
    ));
}

#[test]
fn root_symlinks_wrong_modes_content_symlinks_and_hardlinks_fail_closed() {
    let wrong_mode = TestDirectory::new();
    fs::set_permissions(&wrong_mode.path, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(CompilerArtifactGenerationStoreV1::open(&wrong_mode.path, scope()).is_err());

    let special_mode = TestDirectory::new();
    fs::set_permissions(&special_mode.path, fs::Permissions::from_mode(0o1700)).unwrap();
    assert!(CompilerArtifactGenerationStoreV1::open(&special_mode.path, scope()).is_err());

    let parent = TestDirectory::new();
    let real = parent.path.join("real");
    let alias = parent.path.join("alias");
    fs::create_dir(&real).unwrap();
    fs::set_permissions(&real, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&real, &alias).unwrap();
    assert!(CompilerArtifactGenerationStoreV1::open(&alias, scope()).is_err());

    let hardlink_directory = TestDirectory::new();
    let hardlink_store = hardlink_directory.store();
    committed(hardlink_store.publish_generation_v1(&request(1, true)));
    let blob = blob_for(&hardlink_directory.path, b"neutral-kir-generation-a");
    let attack_link = hardlink_directory.path.join("unexpected-hardlink");
    fs::hard_link(&blob, &attack_link).unwrap();
    assert!(hardlink_store.open_generation_v1().is_err());
    fs::remove_file(attack_link).unwrap();
    assert_generation(
        &hardlink_store.open_generation_v1().unwrap().unwrap(),
        1,
        true,
    );
    let record = canonical_record(&hardlink_directory.path);
    let record_link = hardlink_directory.path.join("unexpected-record-hardlink");
    fs::hard_link(&record, &record_link).unwrap();
    assert!(hardlink_store.open_generation_v1().is_err());
    fs::remove_file(record_link).unwrap();
    fs::set_permissions(&record, fs::Permissions::from_mode(0o4600)).unwrap();
    assert!(hardlink_store.open_generation_v1().is_err());
    fs::set_permissions(&record, fs::Permissions::from_mode(0o600)).unwrap();

    fs::set_permissions(&hardlink_directory.path, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(hardlink_store.open_generation_v1().is_err());
    fs::set_permissions(&hardlink_directory.path, fs::Permissions::from_mode(0o700)).unwrap();

    let symlink_directory = TestDirectory::new();
    let symlink_store = symlink_directory.store();
    committed(symlink_store.publish_generation_v1(&request(1, true)));
    let blob = blob_for(&symlink_directory.path, b"lineage-generation-a");
    let displaced = symlink_directory.path.join("displaced-object");
    fs::rename(&blob, &displaced).unwrap();
    symlink(&displaced, &blob).unwrap();
    assert!(symlink_store.open_generation_v1().is_err());
}

#[test]
fn redo_only_and_post_commit_ambiguity_recover_to_the_exact_generation() {
    for (boundary, timing) in [
        (
            CompilerArtifactGenerationRecordBoundaryV1::RenameTempToRedo,
            CompilerArtifactGenerationFaultTimingV1::After,
        ),
        (
            CompilerArtifactGenerationRecordBoundaryV1::SyncRedoName,
            CompilerArtifactGenerationFaultTimingV1::Before,
        ),
        (
            CompilerArtifactGenerationRecordBoundaryV1::SyncRedoName,
            CompilerArtifactGenerationFaultTimingV1::After,
        ),
        (
            CompilerArtifactGenerationRecordBoundaryV1::RenameRedoToCanonical,
            CompilerArtifactGenerationFaultTimingV1::Before,
        ),
        (
            CompilerArtifactGenerationRecordBoundaryV1::RenameRedoToCanonical,
            CompilerArtifactGenerationFaultTimingV1::After,
        ),
        (
            CompilerArtifactGenerationRecordBoundaryV1::SyncCanonicalName,
            CompilerArtifactGenerationFaultTimingV1::Before,
        ),
        (
            CompilerArtifactGenerationRecordBoundaryV1::SyncCanonicalName,
            CompilerArtifactGenerationFaultTimingV1::After,
        ),
    ] {
        let directory = TestDirectory::new();
        let store = directory.store();
        let expected =
            CompilerArtifactGenerationManifestV1::for_request(scope(), &request(1, true))
                .unwrap()
                .identity();
        let outcome = store.publish_generation_v1_with_options(
            &request(1, true),
            CompilerArtifactGenerationOptionsV1::inject_fault(
                CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                    operation: CompilerArtifactGenerationRecordOperationV1::Commit,
                    boundary,
                    timing,
                },
            ),
        );
        match outcome {
            CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate {
                expected_manifest,
                ..
            } => assert_eq!(expected_manifest, expected),
            other => panic!("wrong outcome at {boundary:?}/{timing:?}: {other:?}"),
        }
        let recovered = store.recover_generation_v1().unwrap().unwrap();
        assert_eq!(recovered.manifest().identity(), expected);
        assert_generation(&recovered, 1, true);
    }
}

#[test]
fn visible_canonical_error_is_never_upgraded_inside_the_originating_publish() {
    for first_fault in [
        CompilerArtifactGenerationFaultPointV1::ScopeRecord {
            operation: CompilerArtifactGenerationRecordOperationV1::Commit,
            boundary: CompilerArtifactGenerationRecordBoundaryV1::RenameRedoToCanonical,
            timing: CompilerArtifactGenerationFaultTimingV1::After,
        },
        CompilerArtifactGenerationFaultPointV1::ScopeRecord {
            operation: CompilerArtifactGenerationRecordOperationV1::Commit,
            boundary: CompilerArtifactGenerationRecordBoundaryV1::SyncCanonicalName,
            timing: CompilerArtifactGenerationFaultTimingV1::Before,
        },
    ] {
        for barrier_timing in [
            CompilerArtifactGenerationFaultTimingV1::Before,
            CompilerArtifactGenerationFaultTimingV1::After,
        ] {
            let directory = TestDirectory::new();
            let store = directory.store();
            let expected =
                CompilerArtifactGenerationManifestV1::for_request(scope(), &request(1, true))
                    .unwrap()
                    .identity();
            let outcome = store.publish_generation_v1_with_options(
                &request(1, true),
                CompilerArtifactGenerationOptionsV1::inject_fault(first_fault),
            );
            assert!(
                matches!(
                    outcome,
                    CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate {
                        expected_manifest,
                        ..
                    } if expected_manifest == expected
                ),
                "{first_fault:?}/{barrier_timing:?}: {outcome:?}"
            );
            assert!(canonical_record(&directory.path).exists());
            assert!(
                entries_with_prefix(&directory.path, SCOPE_PREFIX)
                    .iter()
                    .all(|entry| !entry.to_string_lossy().ends_with(REDO_SUFFIX))
            );
            let recovery_fault = CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                operation: CompilerArtifactGenerationRecordOperationV1::Recover,
                boundary: CompilerArtifactGenerationRecordBoundaryV1::SyncCanonicalName,
                timing: barrier_timing,
            };
            assert!(
                store
                    .recover_generation_v1_with_options(
                        CompilerArtifactGenerationOptionsV1::inject_fault(recovery_fault),
                    )
                    .is_err()
            );
            assert_generation(&store.recover_generation_v1().unwrap().unwrap(), 1, true);
        }
    }
}

#[test]
fn faults_before_redo_visibility_are_not_committed_and_retry_cleanly() {
    for (boundary, timing) in [
        (
            CompilerArtifactGenerationRecordBoundaryV1::CreateTemp,
            CompilerArtifactGenerationFaultTimingV1::Before,
        ),
        (
            CompilerArtifactGenerationRecordBoundaryV1::CreateTemp,
            CompilerArtifactGenerationFaultTimingV1::After,
        ),
        (
            CompilerArtifactGenerationRecordBoundaryV1::WriteTemp,
            CompilerArtifactGenerationFaultTimingV1::Before,
        ),
        (
            CompilerArtifactGenerationRecordBoundaryV1::WriteTemp,
            CompilerArtifactGenerationFaultTimingV1::After,
        ),
        (
            CompilerArtifactGenerationRecordBoundaryV1::SyncTemp,
            CompilerArtifactGenerationFaultTimingV1::Before,
        ),
        (
            CompilerArtifactGenerationRecordBoundaryV1::SyncTemp,
            CompilerArtifactGenerationFaultTimingV1::After,
        ),
        (
            CompilerArtifactGenerationRecordBoundaryV1::RenameTempToRedo,
            CompilerArtifactGenerationFaultTimingV1::Before,
        ),
    ] {
        let directory = TestDirectory::new();
        let store = directory.store();
        let outcome = store.publish_generation_v1_with_options(
            &request(1, true),
            CompilerArtifactGenerationOptionsV1::inject_fault(
                CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                    operation: CompilerArtifactGenerationRecordOperationV1::Commit,
                    boundary,
                    timing,
                },
            ),
        );
        assert!(
            matches!(
                outcome,
                CompilerArtifactGenerationPublishOutcomeV1::NotCommitted(_)
            ),
            "{boundary:?}/{timing:?}"
        );
        assert!(store.open_generation_v1().unwrap().is_none());
        assert_generation(
            &committed(store.publish_generation_v1(&request(1, true))),
            1,
            true,
        );
    }
}

#[test]
fn every_object_publication_fault_leaves_no_mixed_generation_and_retries() {
    let boundaries = [
        CompilerArtifactGenerationObjectBoundaryV1::CreateTemp,
        CompilerArtifactGenerationObjectBoundaryV1::WriteTemp,
        CompilerArtifactGenerationObjectBoundaryV1::SyncTemp,
        CompilerArtifactGenerationObjectBoundaryV1::RenameTempToStaged,
        CompilerArtifactGenerationObjectBoundaryV1::SyncStagedName,
        CompilerArtifactGenerationObjectBoundaryV1::SetFinalMode,
        CompilerArtifactGenerationObjectBoundaryV1::SyncFinalMode,
        CompilerArtifactGenerationObjectBoundaryV1::RenameStagedToFinal,
        CompilerArtifactGenerationObjectBoundaryV1::SyncFinalName,
    ];
    for object in [
        CompilerArtifactGenerationObjectV1::Artifact(CompilerArtifactRoleV1::SemanticMir),
        CompilerArtifactGenerationObjectV1::Manifest,
    ] {
        for boundary in boundaries {
            for timing in [
                CompilerArtifactGenerationFaultTimingV1::Before,
                CompilerArtifactGenerationFaultTimingV1::After,
            ] {
                let directory = TestDirectory::new();
                let store = directory.store();
                let outcome = store.publish_generation_v1_with_options(
                    &request(1, true),
                    CompilerArtifactGenerationOptionsV1::inject_fault(
                        CompilerArtifactGenerationFaultPointV1::Object {
                            object,
                            boundary,
                            timing,
                        },
                    ),
                );
                assert!(
                    matches!(
                        outcome,
                        CompilerArtifactGenerationPublishOutcomeV1::NotCommitted(_)
                    ),
                    "{object:?}/{boundary:?}/{timing:?}"
                );
                assert!(store.open_generation_v1().unwrap().is_none());
                assert_generation(
                    &committed(store.publish_generation_v1(&request(1, true))),
                    1,
                    true,
                );
            }
        }
    }
}

#[test]
fn identical_redo_recovers_but_stale_predecessor_fails_closed() {
    let identical_directory = TestDirectory::new();
    let identical_store = identical_directory.store();
    committed(identical_store.publish_generation_v1(&request(1, true)));
    let record = canonical_record(&identical_directory.path);
    let redo = PathBuf::from(format!("{}{REDO_SUFFIX}", record.display()));
    fs::copy(&record, &redo).unwrap();
    fs::set_permissions(&redo, fs::Permissions::from_mode(0o600)).unwrap();
    assert_generation(
        &identical_store.recover_generation_v1().unwrap().unwrap(),
        1,
        true,
    );
    assert!(!redo.exists());

    let stale_directory = TestDirectory::new();
    let stale_store = stale_directory.store();
    committed(stale_store.publish_generation_v1(&request(1, true)));
    let old_record = fs::read(canonical_record(&stale_directory.path)).unwrap();
    committed(stale_store.publish_generation_v1(&request(2, true)));
    let current_path = canonical_record(&stale_directory.path);
    let current_record = fs::read(&current_path).unwrap();
    let stale_redo = PathBuf::from(format!("{}{REDO_SUFFIX}", current_path.display()));
    fs::write(&stale_redo, old_record).unwrap();
    fs::set_permissions(&stale_redo, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(matches!(
        stale_store.recover_generation_v1(),
        Err(CompilerArtifactGenerationErrorV1::ConflictingRedo { .. })
    ));
    assert_eq!(fs::read(current_path).unwrap(), current_record);
    assert!(stale_redo.exists());
}

#[test]
fn incomplete_redo_never_becomes_the_canonical_generation() {
    let directory = TestDirectory::new();
    let store = directory.store();
    committed(store.publish_generation_v1(&request(1, true)));
    let record = canonical_record(&directory.path);
    let redo = PathBuf::from(format!("{}{REDO_SUFFIX}", record.display()));
    fs::copy(&record, &redo).unwrap();
    fs::set_permissions(&redo, fs::Permissions::from_mode(0o600)).unwrap();
    fs::remove_file(&record).unwrap();
    fs::remove_file(blob_for(&directory.path, b"target-kir-generation-a")).unwrap();

    assert!(store.recover_generation_v1().is_err());
    assert!(redo.exists());
    assert!(!record.exists());
    assert!(store.open_generation_v1().is_err());
}

#[test]
fn concurrent_publishers_and_readers_observe_only_complete_generations() {
    let directory = TestDirectory::new();
    let store = Arc::new(directory.store());
    committed(store.publish_generation_v1(&request(1, true)));
    let barrier = Arc::new(Barrier::new(6));

    let writer_store = Arc::clone(&store);
    let writer_barrier = Arc::clone(&barrier);
    let writer = thread::spawn(move || {
        writer_barrier.wait();
        for index in 0..20 {
            let seed = if index % 2 == 0 { 2 } else { 3 };
            committed(writer_store.publish_generation_v1(&request(seed, true)));
        }
    });

    let mut readers = Vec::new();
    for _ in 0..5 {
        let reader_store = Arc::clone(&store);
        let reader_barrier = Arc::clone(&barrier);
        readers.push(thread::spawn(move || {
            reader_barrier.wait();
            for _ in 0..40 {
                let lease = reader_store.open_generation_v1().unwrap().unwrap();
                let seed = lease.manifest().compiler_identity()[0];
                assert!((1..=3).contains(&seed));
                assert_generation(&lease, seed, true);
            }
        }));
    }

    writer.join().unwrap();
    for reader in readers {
        reader.join().unwrap();
    }
    let final_lease = store.open_generation_v1().unwrap().unwrap();
    let final_seed = final_lease.manifest().compiler_identity()[0];
    assert!(matches!(final_seed, 2 | 3));
    assert_generation(&final_lease, final_seed, true);
}

#[test]
fn sixty_four_simultaneous_publishers_serialize_to_complete_generations() {
    const PUBLISHERS: usize = 64;

    let directory = TestDirectory::new();
    let store = Arc::new(directory.store());
    let barrier = Arc::new(Barrier::new(PUBLISHERS + 1));
    let mut publishers = Vec::new();
    for index in 0..PUBLISHERS {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        publishers.push(thread::spawn(move || {
            barrier.wait();
            let seed = u8::try_from(index % 3 + 1).unwrap();
            committed(store.publish_generation_v1(&request(seed, true)))
        }));
    }
    barrier.wait();
    for publisher in publishers {
        let lease = publisher.join().unwrap();
        let seed = lease.manifest().compiler_identity()[0];
        assert_generation(&lease, seed, true);
    }
    let final_lease = store.open_generation_v1().unwrap().unwrap();
    let final_seed = final_lease.manifest().compiler_identity()[0];
    assert_generation(&final_lease, final_seed, true);
}

#[test]
fn two_scopes_share_the_root_without_cross_publishing() {
    let directory = TestDirectory::new();
    let first = CompilerArtifactGenerationStoreV1::open(
        &directory.path,
        CompilerArtifactGenerationScopeV1::from_bytes([1; 32]),
    )
    .unwrap();
    let second = CompilerArtifactGenerationStoreV1::open(
        &directory.path,
        CompilerArtifactGenerationScopeV1::from_bytes([2; 32]),
    )
    .unwrap();
    committed(first.publish_generation_v1(&request(1, false)));
    committed(second.publish_generation_v1(&request(2, true)));
    assert_generation(&first.open_generation_v1().unwrap().unwrap(), 1, false);
    assert_generation(&second.open_generation_v1().unwrap().unwrap(), 2, true);
    assert_eq!(
        entries_with_prefix(&directory.path, SCOPE_PREFIX)
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(RECORD_SUFFIX))
            })
            .count(),
        2
    );
}

#[test]
fn reclamation_revalidates_a_shared_manifest_for_every_referencing_scope() {
    let directory = TestDirectory::new();
    let first_scope = scope();
    let second_scope = CompilerArtifactGenerationScopeV1::from_bytes([0x52; 32]);
    let first = CompilerArtifactGenerationStoreV1::open(&directory.path, first_scope).unwrap();
    let second = CompilerArtifactGenerationStoreV1::open(&directory.path, second_scope).unwrap();
    committed(first.publish_generation_v1(&request(1, true)));
    committed(second.publish_generation_v1(&request(2, true)));

    let first_record = fs::read(canonical_record_for_scope(&directory.path, first_scope)).unwrap();
    let second_record_path = canonical_record_for_scope(&directory.path, second_scope);
    let mut forged = first_record;
    let scope_offset = SCOPE_RECORD_MAGIC.len() + 2;
    forged[scope_offset..scope_offset + 32].copy_from_slice(&second_scope.as_bytes());
    let checksum_offset = forged.len() - 32;
    let mut checksum = Sha256::new();
    checksum.update(SCOPE_RECORD_CHECKSUM_DOMAIN);
    checksum.update(&forged[..checksum_offset]);
    forged[checksum_offset..].copy_from_slice(&checksum.finalize());
    fs::write(&second_record_path, forged).unwrap();
    fs::set_permissions(&second_record_path, fs::Permissions::from_mode(0o600)).unwrap();

    assert!(
        first.reclaim_store_v1().is_err(),
        "a foreign scope record reused an already reachable manifest"
    );
}

#[test]
fn open_fault_never_returns_a_partial_lease() {
    let directory = TestDirectory::new();
    let store = directory.store();
    let outcome = store.publish_generation_v1_with_options(
        &request(1, true),
        CompilerArtifactGenerationOptionsV1::inject_fault(
            CompilerArtifactGenerationFaultPointV1::Open {
                object: CompilerArtifactGenerationObjectV1::Artifact(
                    CompilerArtifactRoleV1::TargetKir,
                ),
            },
        ),
    );
    assert!(matches!(
        outcome,
        CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
    ));
    assert_generation(&store.recover_generation_v1().unwrap().unwrap(), 1, true);
}

#[test]
fn redo_name_is_private_single_link_and_consumed_by_recovery() {
    let directory = TestDirectory::new();
    let store = directory.store();
    let outcome = store.publish_generation_v1_with_options(
        &request(1, true),
        CompilerArtifactGenerationOptionsV1::inject_fault(
            CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                operation: CompilerArtifactGenerationRecordOperationV1::Commit,
                boundary: CompilerArtifactGenerationRecordBoundaryV1::RenameTempToRedo,
                timing: CompilerArtifactGenerationFaultTimingV1::After,
            },
        ),
    );
    assert!(matches!(
        outcome,
        CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
    ));
    let redo = redo_record(&directory.path);
    let metadata = fs::metadata(&redo).unwrap();
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    assert_generation(&store.recover_generation_v1().unwrap().unwrap(), 1, true);
    assert!(!redo.exists());
}

#[test]
fn canonical_only_recovery_recommits_without_entry_quota_headroom() {
    for boundary in [
        CompilerArtifactGenerationRecordBoundaryV1::RenameCanonicalToRedo,
        CompilerArtifactGenerationRecordBoundaryV1::SyncRedoName,
        CompilerArtifactGenerationRecordBoundaryV1::RenameRedoToCanonical,
        CompilerArtifactGenerationRecordBoundaryV1::SyncCanonicalName,
    ] {
        for timing in [
            CompilerArtifactGenerationFaultTimingV1::Before,
            CompilerArtifactGenerationFaultTimingV1::After,
        ] {
            let directory = TestDirectory::new();
            let store = directory.store_with_quota(
                u64::try_from(MAX_COMPILER_ARTIFACT_GENERATION_BYTES_V1).unwrap(),
                3,
            );
            let request = shared_payload_request();
            let expected = committed(store.publish_generation_v1(&request))
                .manifest()
                .identity();
            assert_eq!(managed_entries(&directory.path).len(), 3);

            let recovered = store.recover_generation_v1_with_options(
                CompilerArtifactGenerationOptionsV1::inject_fault(
                    CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                        operation: CompilerArtifactGenerationRecordOperationV1::Recover,
                        boundary,
                        timing,
                    },
                ),
            );
            assert!(recovered.is_err(), "{boundary:?}/{timing:?}");
            assert_eq!(managed_entries(&directory.path).len(), 3);

            let recovered = store.recover_generation_v1().unwrap().unwrap();
            assert_eq!(recovered.manifest().identity(), expected);
            assert_eq!(managed_entries(&directory.path).len(), 3);
            assert!(
                entries_with_prefix(&directory.path, SCOPE_PREFIX)
                    .iter()
                    .all(|entry| !entry.to_string_lossy().ends_with(REDO_SUFFIX))
            );
        }
    }
}

#[test]
fn redo_promotion_recovery_survives_every_exposed_commit_boundary() {
    for (boundary, timing) in [
        (
            CompilerArtifactGenerationRecordBoundaryV1::RenameRedoToCanonical,
            CompilerArtifactGenerationFaultTimingV1::Before,
        ),
        (
            CompilerArtifactGenerationRecordBoundaryV1::RenameRedoToCanonical,
            CompilerArtifactGenerationFaultTimingV1::After,
        ),
        (
            CompilerArtifactGenerationRecordBoundaryV1::SyncCanonicalName,
            CompilerArtifactGenerationFaultTimingV1::Before,
        ),
        (
            CompilerArtifactGenerationRecordBoundaryV1::SyncCanonicalName,
            CompilerArtifactGenerationFaultTimingV1::After,
        ),
    ] {
        let directory = TestDirectory::new();
        let store = directory.store();
        let interrupted = store.publish_generation_v1_with_options(
            &request(1, true),
            CompilerArtifactGenerationOptionsV1::inject_fault(
                CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                    operation: CompilerArtifactGenerationRecordOperationV1::Commit,
                    boundary: CompilerArtifactGenerationRecordBoundaryV1::RenameTempToRedo,
                    timing: CompilerArtifactGenerationFaultTimingV1::After,
                },
            ),
        );
        assert!(matches!(
            interrupted,
            CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
        ));
        assert!(
            store
                .recover_generation_v1_with_options(
                    CompilerArtifactGenerationOptionsV1::inject_fault(
                        CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                            operation: CompilerArtifactGenerationRecordOperationV1::Recover,
                            boundary,
                            timing,
                        },
                    ),
                )
                .is_err(),
            "{boundary:?}/{timing:?}"
        );
        assert_generation(&store.recover_generation_v1().unwrap().unwrap(), 1, true);
    }
}

#[test]
fn existing_canonical_and_identical_redo_faults_never_report_not_committed() {
    let canonical_directory = TestDirectory::new();
    let canonical_store = canonical_directory.store();
    committed(canonical_store.publish_generation_v1(&request(1, true)));
    let outcome = canonical_store.publish_generation_v1_with_options(
        &request(1, true),
        CompilerArtifactGenerationOptionsV1::inject_fault(
            CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                operation: CompilerArtifactGenerationRecordOperationV1::Recover,
                boundary: CompilerArtifactGenerationRecordBoundaryV1::SyncCanonicalName,
                timing: CompilerArtifactGenerationFaultTimingV1::Before,
            },
        ),
    );
    assert!(matches!(
        outcome,
        CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
    ));

    let redo_directory = TestDirectory::new();
    let redo_store = redo_directory.store();
    committed(redo_store.publish_generation_v1(&request(1, true)));
    let canonical = canonical_record(&redo_directory.path);
    let redo = PathBuf::from(format!("{}{REDO_SUFFIX}", canonical.display()));
    fs::copy(&canonical, &redo).unwrap();
    fs::set_permissions(&redo, fs::Permissions::from_mode(0o600)).unwrap();
    let outcome = redo_store.publish_generation_v1_with_options(
        &request(1, true),
        CompilerArtifactGenerationOptionsV1::inject_fault(
            CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                operation: CompilerArtifactGenerationRecordOperationV1::Recover,
                boundary: CompilerArtifactGenerationRecordBoundaryV1::RenameRedoToCanonical,
                timing: CompilerArtifactGenerationFaultTimingV1::Before,
            },
        ),
    );
    assert!(matches!(
        outcome,
        CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
    ));
}

#[test]
fn preexisting_recoverable_requested_redo_is_indeterminate_on_recovery_fault() {
    let directory = TestDirectory::new();
    let store = directory.store();
    committed(store.publish_generation_v1(&request(1, true)));
    let pending = store.publish_generation_v1_with_options(
        &request(2, true),
        CompilerArtifactGenerationOptionsV1::inject_fault(
            CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                operation: CompilerArtifactGenerationRecordOperationV1::Commit,
                boundary: CompilerArtifactGenerationRecordBoundaryV1::RenameTempToRedo,
                timing: CompilerArtifactGenerationFaultTimingV1::After,
            },
        ),
    );
    assert!(matches!(
        pending,
        CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
    ));
    let retried = store.publish_generation_v1_with_options(
        &request(2, true),
        CompilerArtifactGenerationOptionsV1::inject_fault(
            CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                operation: CompilerArtifactGenerationRecordOperationV1::Recover,
                boundary: CompilerArtifactGenerationRecordBoundaryV1::RenameRedoToCanonical,
                timing: CompilerArtifactGenerationFaultTimingV1::Before,
            },
        ),
    );
    assert!(matches!(
        retried,
        CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
    ));
    assert_generation(&store.recover_generation_v1().unwrap().unwrap(), 2, true);
}

#[test]
fn reclamation_resolves_redo_before_pruning_its_predecessor_closure() {
    let directory = TestDirectory::new();
    let store = directory.store();
    committed(store.publish_generation_v1(&request(1, true)));
    let pending = store.publish_generation_v1_with_options(
        &request(2, true),
        CompilerArtifactGenerationOptionsV1::inject_fault(
            CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                operation: CompilerArtifactGenerationRecordOperationV1::Commit,
                boundary: CompilerArtifactGenerationRecordBoundaryV1::RenameTempToRedo,
                timing: CompilerArtifactGenerationFaultTimingV1::After,
            },
        ),
    );
    assert!(matches!(
        pending,
        CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
    ));
    let redo = redo_record(&directory.path);
    let before = managed_entries(&directory.path).len();
    let report = store.reclaim_store_v1().unwrap();
    assert!(report.removed_entries() > 0);
    assert!(report.retained_entries() < before);
    assert!(canonical_record(&directory.path).exists());
    assert!(!redo.exists());
    assert!(!blob_for(&directory.path, b"semantic-mir-generation-a").exists());
    assert!(blob_for(&directory.path, b"semantic-mir-generation-b").exists());
    assert_generation(&store.recover_generation_v1().unwrap().unwrap(), 2, true);
}

#[test]
fn many_retained_leases_do_not_retain_deleted_open_blocks_or_bypass_quota() {
    let directory = TestDirectory::new();
    let store = directory.store_with_quota(64 * 1024, 32);
    let mut leases = Vec::new();
    leases.push(committed(store.publish_generation_v1(&request(1, true))));
    for index in 0..120 {
        let seed = 2 + u8::try_from(index % 2).unwrap();
        leases.push(committed(store.publish_generation_v1(&request(seed, true))));
        assert_eq!(managed_entries(&directory.path).len(), 7);
        assert!(deleted_generation_descriptors(&directory.path).is_empty());
    }
    assert_generation(leases.first().unwrap(), 1, true);
    assert_generation(leases.last().unwrap(), 3, true);
    assert!(!blob_for(&directory.path, b"semantic-mir-generation-a").exists());
    let report = store.reclaim_store_v1().unwrap();
    assert_eq!(report.removed_entries(), 0);
    assert_eq!(report.retained_entries(), 7);
    assert!(report.retained_bytes() <= 64 * 1024);
    for entry in managed_entries(&directory.path) {
        assert_eq!(fs::metadata(entry).unwrap().nlink(), 1);
    }
    assert!(deleted_generation_descriptors(&directory.path).is_empty());
}

#[test]
fn configured_byte_and_entry_quotas_fail_before_commit_and_preserve_active() {
    let entry_directory = TestDirectory::new();
    let entry_store = entry_directory.store_with_quota(128 * 1024, 5);
    assert!(matches!(
        entry_store.publish_generation_v1(&request(1, false)),
        CompilerArtifactGenerationPublishOutcomeV1::NotCommitted(
            CompilerArtifactGenerationErrorV1::ManagedEntryLimitExceeded { .. }
        )
    ));
    assert!(managed_entries(&entry_directory.path).is_empty());

    let byte_directory = TestDirectory::new();
    let byte_store = byte_directory.store_with_quota(16 * 1024, 64);
    assert!(matches!(
        byte_store.publish_generation_v1(&request(1, true)),
        CompilerArtifactGenerationPublishOutcomeV1::NotCommitted(
            CompilerArtifactGenerationErrorV1::StorageQuotaExceeded { .. }
        )
    ));
    assert!(managed_entries(&byte_directory.path).is_empty());

    let replacement_directory = TestDirectory::new();
    let replacement_store = replacement_directory.store_with_quota(40 * 1024, 64);
    committed(replacement_store.publish_generation_v1(&request(1, true)));
    assert!(matches!(
        replacement_store.publish_generation_v1(&request(2, true)),
        CompilerArtifactGenerationPublishOutcomeV1::NotCommitted(
            CompilerArtifactGenerationErrorV1::StorageQuotaExceeded { .. }
        )
    ));
    assert_generation(
        &replacement_store.open_generation_v1().unwrap().unwrap(),
        1,
        true,
    );
    drop(replacement_store);
    assert!(matches!(
        CompilerArtifactGenerationStoreV1::open_with_quota(
            &replacement_directory.path,
            scope(),
            CompilerArtifactGenerationQuotaV1::new(16 * 1024, 64).unwrap(),
        ),
        Err(CompilerArtifactGenerationErrorV1::StorageQuotaExceeded { .. })
    ));
    assert!(matches!(
        CompilerArtifactGenerationStoreV1::open_with_quota(
            &replacement_directory.path,
            scope(),
            CompilerArtifactGenerationQuotaV1::new(128 * 1024, 6).unwrap(),
        ),
        Err(CompilerArtifactGenerationErrorV1::ManagedEntryLimitExceeded { .. })
    ));
    assert_generation(
        &replacement_directory
            .store()
            .open_generation_v1()
            .unwrap()
            .unwrap(),
        1,
        true,
    );
}

#[test]
fn quota_projection_deduplicates_shared_incoming_content_addresses_at_exact_boundaries() {
    let exact_directory = TestDirectory::new();
    let filesystem = rustix::fs::fstatvfs(
        OpenOptions::new()
            .read(true)
            .open(&exact_directory.path)
            .unwrap(),
    )
    .unwrap();
    let allocation_unit = filesystem.f_frsize.max(filesystem.f_bsize).max(512);
    let charge = |length: usize| {
        let length = u64::try_from(length).unwrap();
        length.div_ceil(allocation_unit) * allocation_unit
    };
    let request = shared_payload_request();
    let manifest = CompilerArtifactGenerationManifestV1::for_request(scope(), &request).unwrap();
    let exact_bytes = charge(MAX_COMPILER_ARTIFACT_GENERATION_SCOPE_RECORD_BYTES_V1)
        + charge(manifest.canonical_bytes().len())
        + charge(
            request
                .artifact(CompilerArtifactRoleV1::SemanticMir)
                .unwrap()
                .len(),
        );

    let exact_store = exact_directory.store_with_quota(exact_bytes, 3);
    let lease = committed(exact_store.publish_generation_v1(&request));
    assert_eq!(managed_entries(&exact_directory.path).len(), 3);
    for role in [
        CompilerArtifactRoleV1::SemanticMir,
        CompilerArtifactRoleV1::NeutralKir,
        CompilerArtifactRoleV1::TargetKir,
        CompilerArtifactRoleV1::Lineage,
    ] {
        assert_eq!(
            lease.artifact(role),
            request.artifact(CompilerArtifactRoleV1::SemanticMir)
        );
    }
    drop(lease);
    drop(exact_store);
    let reopened = exact_directory.store_with_quota(exact_bytes, 3);
    let reopened_lease = reopened.open_generation_v1().unwrap().unwrap();
    assert_eq!(managed_entries(&exact_directory.path).len(), 3);
    for role in [
        CompilerArtifactRoleV1::SemanticMir,
        CompilerArtifactRoleV1::NeutralKir,
        CompilerArtifactRoleV1::TargetKir,
        CompilerArtifactRoleV1::Lineage,
    ] {
        assert_eq!(
            reopened_lease.artifact(role),
            request.artifact(CompilerArtifactRoleV1::SemanticMir)
        );
    }

    let byte_directory = TestDirectory::new();
    let byte_store = byte_directory.store_with_quota(exact_bytes - 1, 3);
    assert!(matches!(
        byte_store.publish_generation_v1(&request),
        CompilerArtifactGenerationPublishOutcomeV1::NotCommitted(
            CompilerArtifactGenerationErrorV1::StorageQuotaExceeded { .. }
        )
    ));
    assert!(managed_entries(&byte_directory.path).is_empty());

    let entry_directory = TestDirectory::new();
    let entry_store = entry_directory.store_with_quota(exact_bytes, 2);
    assert!(matches!(
        entry_store.publish_generation_v1(&request),
        CompilerArtifactGenerationPublishOutcomeV1::NotCommitted(
            CompilerArtifactGenerationErrorV1::ManagedEntryLimitExceeded { .. }
        )
    ));
    assert!(managed_entries(&entry_directory.path).is_empty());
}

#[test]
fn actual_allocated_bytes_are_rejected_before_scope_record_promotion() {
    let directory = TestDirectory::new();
    let store = Arc::new(directory.store_with_quota(256 * 1024, 64));
    let point = CompilerArtifactGenerationFaultPointV1::ScopeRecord {
        operation: CompilerArtifactGenerationRecordOperationV1::Commit,
        boundary: CompilerArtifactGenerationRecordBoundaryV1::SyncRedoName,
        timing: CompilerArtifactGenerationFaultTimingV1::After,
    };
    let observation = Arc::new(CompilerArtifactGenerationObservationV1::new(point));
    let publishing_store = Arc::clone(&store);
    let publishing_observation = Arc::clone(&observation);
    let publisher = thread::spawn(move || {
        publishing_store.publish_generation_v1_with_options(
            &request(1, true),
            CompilerArtifactGenerationOptionsV1::observe(publishing_observation),
        )
    });
    observation.wait_until_reached();

    let redo = redo_record(&directory.path);
    let redo_file = OpenOptions::new().write(true).open(&redo).unwrap();
    let allocation = unsafe {
        libc::fallocate(
            redo_file.as_raw_fd(),
            libc::FALLOC_FL_KEEP_SIZE,
            0,
            1024 * 1024,
        )
    };
    assert_eq!(allocation, 0, "{}", std::io::Error::last_os_error());
    assert!(redo_file.metadata().unwrap().blocks() * 512 > 256 * 1024);
    drop(redo_file);

    observation.release();
    let outcome = publisher.join().unwrap();
    assert!(matches!(
        outcome,
        CompilerArtifactGenerationPublishOutcomeV1::NotCommitted(
            CompilerArtifactGenerationErrorV1::StorageQuotaExceeded { .. }
        )
    ));
    assert!(canonical_record_optional(&directory.path).is_none());
    assert!(!redo.exists());
    assert!(store.recover_generation_v1().unwrap().is_none());
    assert!(managed_entries(&directory.path).is_empty());
}

#[test]
fn restart_discards_an_over_quota_redo_and_preserves_the_committed_generation() {
    let directory = TestDirectory::new();
    let quota = CompilerArtifactGenerationQuotaV1::new(256 * 1024, 64).unwrap();
    let store = CompilerArtifactGenerationStoreV1::open_with_quota(&directory.path, scope(), quota)
        .unwrap();
    committed(store.publish_generation_v1(&request(1, true)));
    let outcome = store.publish_generation_v1_with_options(
        &request(2, true),
        CompilerArtifactGenerationOptionsV1::inject_fault(
            CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                operation: CompilerArtifactGenerationRecordOperationV1::Commit,
                boundary: CompilerArtifactGenerationRecordBoundaryV1::SyncRedoName,
                timing: CompilerArtifactGenerationFaultTimingV1::After,
            },
        ),
    );
    assert!(matches!(
        outcome,
        CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
    ));
    let redo = redo_record(&directory.path);
    let redo_file = OpenOptions::new().write(true).open(&redo).unwrap();
    let allocation = unsafe {
        libc::fallocate(
            redo_file.as_raw_fd(),
            libc::FALLOC_FL_KEEP_SIZE,
            0,
            1024 * 1024,
        )
    };
    assert_eq!(allocation, 0, "{}", std::io::Error::last_os_error());
    assert!(redo_file.metadata().unwrap().blocks() * 512 > quota.maximum_bytes());
    drop(redo_file);
    drop(store);

    let reopened =
        CompilerArtifactGenerationStoreV1::open_with_quota(&directory.path, scope(), quota)
            .unwrap();
    assert!(!redo.exists());
    assert_generation(&reopened.open_generation_v1().unwrap().unwrap(), 1, true);
}

#[test]
fn foreign_scope_over_quota_redo_cannot_block_root_admission() {
    let directory = TestDirectory::new();
    let first_scope = scope();
    let second_scope = CompilerArtifactGenerationScopeV1::from_bytes([0x52; 32]);
    let first = CompilerArtifactGenerationStoreV1::open(&directory.path, first_scope).unwrap();
    let second = CompilerArtifactGenerationStoreV1::open(&directory.path, second_scope).unwrap();
    committed(first.publish_generation_v1(&request(1, true)));
    committed(second.publish_generation_v1(&request(2, true)));
    let pending = second.publish_generation_v1_with_options(
        &request(3, true),
        CompilerArtifactGenerationOptionsV1::inject_fault(
            CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                operation: CompilerArtifactGenerationRecordOperationV1::Commit,
                boundary: CompilerArtifactGenerationRecordBoundaryV1::SyncRedoName,
                timing: CompilerArtifactGenerationFaultTimingV1::After,
            },
        ),
    );
    assert!(matches!(
        pending,
        CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
    ));
    let redo = redo_record(&directory.path);
    let redo_file = OpenOptions::new().write(true).open(&redo).unwrap();
    let allocation = unsafe {
        libc::fallocate(
            redo_file.as_raw_fd(),
            libc::FALLOC_FL_KEEP_SIZE,
            0,
            1024 * 1024,
        )
    };
    assert_eq!(allocation, 0, "{}", std::io::Error::last_os_error());
    drop(redo_file);
    drop(second);
    drop(first);

    let quota = CompilerArtifactGenerationQuotaV1::new(256 * 1024, 64).unwrap();
    let reopened_first =
        CompilerArtifactGenerationStoreV1::open_with_quota(&directory.path, first_scope, quota)
            .unwrap();
    assert!(!redo.exists());
    assert_generation(
        &reopened_first.open_generation_v1().unwrap().unwrap(),
        1,
        true,
    );
    let reopened_second =
        CompilerArtifactGenerationStoreV1::open_with_quota(&directory.path, second_scope, quota)
            .unwrap();
    assert_generation(
        &reopened_second.open_generation_v1().unwrap().unwrap(),
        2,
        true,
    );
}

#[test]
fn stale_object_and_record_temporaries_are_reclaimed_under_lock() {
    for fault in [
        CompilerArtifactGenerationFaultPointV1::Object {
            object: CompilerArtifactGenerationObjectV1::Artifact(
                CompilerArtifactRoleV1::SemanticMir,
            ),
            boundary: CompilerArtifactGenerationObjectBoundaryV1::CreateTemp,
            timing: CompilerArtifactGenerationFaultTimingV1::After,
        },
        CompilerArtifactGenerationFaultPointV1::Object {
            object: CompilerArtifactGenerationObjectV1::Artifact(
                CompilerArtifactRoleV1::SemanticMir,
            ),
            boundary: CompilerArtifactGenerationObjectBoundaryV1::RenameTempToStaged,
            timing: CompilerArtifactGenerationFaultTimingV1::After,
        },
        CompilerArtifactGenerationFaultPointV1::ScopeRecord {
            operation: CompilerArtifactGenerationRecordOperationV1::Commit,
            boundary: CompilerArtifactGenerationRecordBoundaryV1::CreateTemp,
            timing: CompilerArtifactGenerationFaultTimingV1::After,
        },
    ] {
        let directory = TestDirectory::new();
        let store = directory.store();
        let outcome = store.publish_generation_v1_with_options(
            &request(1, true),
            CompilerArtifactGenerationOptionsV1::inject_fault(fault),
        );
        assert!(matches!(
            outcome,
            CompilerArtifactGenerationPublishOutcomeV1::NotCommitted(_)
        ));
        assert!(!managed_entries(&directory.path).is_empty());
        assert!(store.reclaim_store_v1().unwrap().removed_entries() > 0);
        assert!(managed_entries(&directory.path).is_empty());
    }
}

#[test]
fn persistent_lock_inode_replacement_is_rejected() {
    let directory = TestDirectory::new();
    let store = directory.store();
    let lock = directory.path.join(LOCK_FILE);
    fs::remove_file(&lock).unwrap();
    fs::write(&lock, b"replacement").unwrap();
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o600)).unwrap();
    let result = store.open_generation_v1();
    assert!(
        matches!(
            result,
            Err(CompilerArtifactGenerationErrorV1::UnsafeRoot { .. })
        ),
        "{result:?}"
    );
}

#[test]
fn same_process_fresh_store_cannot_bypass_an_active_replaced_named_lock() {
    let directory = TestDirectory::new();
    let store = Arc::new(directory.store());
    let observation = Arc::new(CompilerArtifactGenerationObservationV1::new(
        CompilerArtifactGenerationFaultPointV1::DirectoryScan,
    ));
    let publisher_store = Arc::clone(&store);
    let publisher_observation = Arc::clone(&observation);
    let publisher = thread::spawn(move || {
        publisher_store.publish_generation_v1_with_options(
            &request(1, true),
            CompilerArtifactGenerationOptionsV1::observe(publisher_observation),
        )
    });
    observation.wait_until_reached();
    let _displaced = replace_named_lock(&directory.path);

    let contender_root = directory.path.clone();
    let contender =
        thread::spawn(move || CompilerArtifactGenerationStoreV1::open(&contender_root, scope()));
    thread::sleep(Duration::from_millis(200));
    assert!(
        !contender.is_finished(),
        "fresh store bypassed the stable root guard through a replacement lock inode"
    );
    assert!(managed_entries(&directory.path).is_empty());

    observation.release();
    let outcome = publisher.join().unwrap();
    assert!(matches!(
        outcome,
        CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
    ));
    let replacement_store = contender.join().unwrap().unwrap();
    assert!(managed_entries(&directory.path).is_empty());
    committed(replacement_store.publish_generation_v1(&request(1, true)));
    assert!(store.open_generation_v1().is_err());
}

#[test]
fn cross_process_fresh_store_cannot_bypass_an_active_replaced_named_lock() {
    let directory = TestDirectory::new();
    let store = Arc::new(directory.store());
    let observation = Arc::new(CompilerArtifactGenerationObservationV1::new(
        CompilerArtifactGenerationFaultPointV1::DirectoryScan,
    ));
    let publisher_store = Arc::clone(&store);
    let publisher_observation = Arc::clone(&observation);
    let publisher = thread::spawn(move || {
        publisher_store.publish_generation_v1_with_options(
            &request(1, true),
            CompilerArtifactGenerationOptionsV1::observe(publisher_observation),
        )
    });
    observation.wait_until_reached();
    let _displaced = replace_named_lock(&directory.path);

    let ready = directory.path.join("replacement-contender-ready");
    let mut child = subprocess_with_ready("publish", &directory.path, Some(&ready));
    wait_for_path(&ready);
    thread::sleep(Duration::from_millis(200));
    assert!(
        child.try_wait().unwrap().is_none(),
        "cross-process store bypassed the stable root guard"
    );
    assert!(managed_entries(&directory.path).is_empty());

    observation.release();
    assert!(matches!(
        publisher.join().unwrap(),
        CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
    ));
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "child failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_generation(
        &CompilerArtifactGenerationStoreV1::open(&directory.path, scope())
            .unwrap()
            .open_generation_v1()
            .unwrap()
            .unwrap(),
        1,
        true,
    );
    assert!(matches!(
        store.open_generation_v1(),
        Err(CompilerArtifactGenerationErrorV1::UnsafeRoot { .. })
    ));
}

#[test]
fn root_path_substitution_during_commit_never_publishes_into_replacement() {
    let directory = TestDirectory::new();
    let displaced = directory.path.with_extension("displaced");
    let point = CompilerArtifactGenerationFaultPointV1::ScopeRecord {
        operation: CompilerArtifactGenerationRecordOperationV1::Commit,
        boundary: CompilerArtifactGenerationRecordBoundaryV1::RenameRedoToCanonical,
        timing: CompilerArtifactGenerationFaultTimingV1::Before,
    };
    let observation = Arc::new(CompilerArtifactGenerationObservationV1::new(point));
    let store = Arc::new(directory.store());
    let publishing_store = Arc::clone(&store);
    let publishing_observation = Arc::clone(&observation);
    let publisher = thread::spawn(move || {
        publishing_store.publish_generation_v1_with_options(
            &request(1, true),
            CompilerArtifactGenerationOptionsV1::observe(publishing_observation),
        )
    });
    observation.wait_until_reached();
    fs::rename(&directory.path, &displaced).unwrap();
    fs::create_dir(&directory.path).unwrap();
    fs::set_permissions(&directory.path, fs::Permissions::from_mode(0o700)).unwrap();
    observation.release();
    let outcome = publisher.join().unwrap();
    assert!(matches!(
        outcome,
        CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
    ));
    assert!(managed_entries(&directory.path).is_empty());
    drop(store);
    fs::remove_dir_all(displaced).unwrap();
}

#[test]
fn fresh_store_on_replacement_root_waits_for_displaced_root_transaction() {
    let directory = TestDirectory::new();
    let displaced = directory.path.with_extension("displaced-contended");
    let point = CompilerArtifactGenerationFaultPointV1::ScopeRecord {
        operation: CompilerArtifactGenerationRecordOperationV1::Commit,
        boundary: CompilerArtifactGenerationRecordBoundaryV1::RenameRedoToCanonical,
        timing: CompilerArtifactGenerationFaultTimingV1::Before,
    };
    let observation = Arc::new(CompilerArtifactGenerationObservationV1::new(point));
    let store = Arc::new(directory.store());
    let publishing_store = Arc::clone(&store);
    let publishing_observation = Arc::clone(&observation);
    let publisher = thread::spawn(move || {
        publishing_store.publish_generation_v1_with_options(
            &request(1, true),
            CompilerArtifactGenerationOptionsV1::observe(publishing_observation),
        )
    });
    observation.wait_until_reached();

    fs::rename(&directory.path, &displaced).unwrap();
    fs::create_dir(&directory.path).unwrap();
    fs::set_permissions(&directory.path, fs::Permissions::from_mode(0o700)).unwrap();
    let replacement_path = directory.path.clone();
    let contender =
        thread::spawn(move || CompilerArtifactGenerationStoreV1::open(&replacement_path, scope()));
    thread::sleep(Duration::from_millis(200));
    assert!(
        !contender.is_finished(),
        "replacement-root writer bypassed the path-stable guard"
    );
    assert!(managed_entries(&directory.path).is_empty());

    observation.release();
    assert!(matches!(
        publisher.join().unwrap(),
        CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
    ));
    let replacement_store = contender.join().unwrap().unwrap();
    committed(replacement_store.publish_generation_v1(&request(2, true)));
    assert_generation(
        &replacement_store.open_generation_v1().unwrap().unwrap(),
        2,
        true,
    );
    assert!(store.open_generation_v1().is_err());
    drop(replacement_store);
    drop(store);
    fs::remove_dir_all(displaced).unwrap();
}

#[test]
fn cross_process_store_on_replacement_root_waits_for_displaced_root_transaction() {
    let directory = TestDirectory::new();
    let displaced = directory.path.with_extension("displaced-cross-process");
    let ready = directory.path.with_extension("replacement-contender-ready");
    let point = CompilerArtifactGenerationFaultPointV1::ScopeRecord {
        operation: CompilerArtifactGenerationRecordOperationV1::Commit,
        boundary: CompilerArtifactGenerationRecordBoundaryV1::RenameRedoToCanonical,
        timing: CompilerArtifactGenerationFaultTimingV1::Before,
    };
    let observation = Arc::new(CompilerArtifactGenerationObservationV1::new(point));
    let store = Arc::new(directory.store());
    let publishing_store = Arc::clone(&store);
    let publishing_observation = Arc::clone(&observation);
    let publisher = thread::spawn(move || {
        publishing_store.publish_generation_v1_with_options(
            &request(1, true),
            CompilerArtifactGenerationOptionsV1::observe(publishing_observation),
        )
    });
    observation.wait_until_reached();

    fs::rename(&directory.path, &displaced).unwrap();
    fs::create_dir(&directory.path).unwrap();
    fs::set_permissions(&directory.path, fs::Permissions::from_mode(0o700)).unwrap();
    let mut child = subprocess_with_ready("publish", &directory.path, Some(&ready));
    wait_for_path(&ready);
    thread::sleep(Duration::from_millis(200));
    assert!(
        child.try_wait().unwrap().is_none(),
        "cross-process replacement-root writer bypassed the path-stable guard"
    );
    assert!(managed_entries(&directory.path).is_empty());

    observation.release();
    assert!(matches!(
        publisher.join().unwrap(),
        CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
    ));
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "child failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_generation(
        &CompilerArtifactGenerationStoreV1::open(&directory.path, scope())
            .unwrap()
            .open_generation_v1()
            .unwrap()
            .unwrap(),
        1,
        true,
    );
    assert!(store.open_generation_v1().is_err());
    drop(store);
    fs::remove_file(ready).unwrap();
    fs::remove_dir_all(displaced).unwrap();
}

#[test]
fn cross_process_store_waits_when_the_output_parent_is_replaced() {
    let directory = TestDirectory::new();
    let parent = directory.path.join("active-parent");
    let root = parent.join("store");
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let displaced_parent = directory.path.join("displaced-parent");
    let ready = directory.path.join("ancestor-replacement-contender-ready");
    let point = CompilerArtifactGenerationFaultPointV1::ScopeRecord {
        operation: CompilerArtifactGenerationRecordOperationV1::Commit,
        boundary: CompilerArtifactGenerationRecordBoundaryV1::RenameRedoToCanonical,
        timing: CompilerArtifactGenerationFaultTimingV1::Before,
    };
    let observation = Arc::new(CompilerArtifactGenerationObservationV1::new(point));
    let store = Arc::new(CompilerArtifactGenerationStoreV1::open(&root, scope()).unwrap());
    let publishing_store = Arc::clone(&store);
    let publishing_observation = Arc::clone(&observation);
    let publisher = thread::spawn(move || {
        publishing_store.publish_generation_v1_with_options(
            &request(1, true),
            CompilerArtifactGenerationOptionsV1::observe(publishing_observation),
        )
    });
    observation.wait_until_reached();

    fs::rename(&parent, &displaced_parent).unwrap();
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let mut child = subprocess_with_ready("publish", &root, Some(&ready));
    wait_for_path(&ready);
    thread::sleep(Duration::from_millis(200));
    assert!(
        child.try_wait().unwrap().is_none(),
        "cross-process writer bypassed the stable absolute-path guard after ancestor replacement"
    );
    assert!(managed_entries(&root).is_empty());

    observation.release();
    assert!(matches!(
        publisher.join().unwrap(),
        CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
    ));
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "child failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_generation(
        &CompilerArtifactGenerationStoreV1::open(&root, scope())
            .unwrap()
            .open_generation_v1()
            .unwrap()
            .unwrap(),
        1,
        true,
    );
    assert!(store.open_generation_v1().is_err());
    drop(store);
    fs::remove_file(ready).unwrap();
}

#[test]
fn redo_symlink_hardlink_and_mode_attacks_fail_closed() {
    for attack in 0..4 {
        let directory = TestDirectory::new();
        let store = directory.store();
        let outcome = store.publish_generation_v1_with_options(
            &request(1, true),
            CompilerArtifactGenerationOptionsV1::inject_fault(
                CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                    operation: CompilerArtifactGenerationRecordOperationV1::Commit,
                    boundary: CompilerArtifactGenerationRecordBoundaryV1::RenameTempToRedo,
                    timing: CompilerArtifactGenerationFaultTimingV1::After,
                },
            ),
        );
        assert!(matches!(
            outcome,
            CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
        ));
        let redo = redo_record(&directory.path);
        match attack {
            0 => {
                let displaced = directory.path.join("displaced-redo");
                fs::rename(&redo, &displaced).unwrap();
                symlink(&displaced, &redo).unwrap();
            }
            1 => fs::hard_link(&redo, directory.path.join("redo-hardlink")).unwrap(),
            2 => fs::set_permissions(&redo, fs::Permissions::from_mode(0o644)).unwrap(),
            3 => fs::set_permissions(&redo, fs::Permissions::from_mode(0o4600)).unwrap(),
            _ => unreachable!(),
        }
        assert!(store.recover_generation_v1().is_err(), "attack {attack}");
        assert!(canonical_record_optional(&directory.path).is_none());
    }
}

#[test]
fn corrupted_large_sparse_object_is_rejected_before_payload_allocation() {
    let directory = TestDirectory::new();
    let store = directory.store();
    committed(store.publish_generation_v1(&request(1, true)));
    let blob = blob_for(&directory.path, b"semantic-mir-generation-a");
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o600)).unwrap();
    OpenOptions::new()
        .write(true)
        .open(&blob)
        .unwrap()
        .set_len(MAX_COMPILER_SEMANTIC_MIR_BYTES_V1 as u64)
        .unwrap();
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o400)).unwrap();
    assert!(matches!(
        store.open_generation_v1(),
        Err(CompilerArtifactGenerationErrorV1::UnsafeEntry { .. })
    ));
}

fn canonical_record_optional(root: &Path) -> Option<PathBuf> {
    entries_with_prefix(root, SCOPE_PREFIX)
        .into_iter()
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(RECORD_SUFFIX))
        })
}

fn subprocess(action: &str, root: &Path) -> std::process::Child {
    subprocess_with_ready(action, root, None)
}

fn subprocess_with_ready(action: &str, root: &Path, ready: Option<&Path>) -> std::process::Child {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg("compiler_artifact_generation_subprocess_helper")
        .arg("--nocapture")
        .env("FE2O3_GENERATION_SUBPROCESS", action)
        .env("FE2O3_GENERATION_ROOT", root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(ready) = ready {
        command.env("FE2O3_GENERATION_READY", ready);
    }
    command.spawn().unwrap()
}

fn wait_for_path(path: &Path) {
    for _ in 0..500 {
        if path.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {}", path.display());
}

#[test]
fn dropping_duplicate_store_never_releases_an_active_ofd_lock() {
    for duplicate_scope in [
        scope(),
        CompilerArtifactGenerationScopeV1::from_bytes([0x52; 32]),
    ] {
        let directory = TestDirectory::new();
        let store_a = Arc::new(directory.store());
        let store_b =
            CompilerArtifactGenerationStoreV1::open(&directory.path, duplicate_scope).unwrap();
        let observation = Arc::new(CompilerArtifactGenerationObservationV1::new(
            CompilerArtifactGenerationFaultPointV1::DirectoryScan,
        ));
        let publishing_store = Arc::clone(&store_a);
        let publishing_observation = Arc::clone(&observation);
        let publisher = thread::spawn(move || {
            publishing_store.publish_generation_v1_with_options(
                &request(1, true),
                CompilerArtifactGenerationOptionsV1::observe(publishing_observation),
            )
        });
        observation.wait_until_reached();

        let ready = directory.path.join("ofd-contender-ready");
        let mut child = subprocess_with_ready("publish", &directory.path, Some(&ready));
        wait_for_path(&ready);
        thread::sleep(Duration::from_millis(100));
        assert!(child.try_wait().unwrap().is_none());

        drop(store_b);
        thread::sleep(Duration::from_millis(200));
        assert!(
            child.try_wait().unwrap().is_none(),
            "dropping a duplicate store released Store A's active lock for {duplicate_scope:?}"
        );

        observation.release();
        committed(publisher.join().unwrap());
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "child failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        fs::remove_file(ready).unwrap();
    }
}

#[test]
fn subprocess_contention_and_restart_recovery_use_one_persistent_lock() {
    let contention_directory = TestDirectory::new();
    let contention_store = contention_directory.store();
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(contention_directory.path.join(LOCK_FILE))
        .unwrap();
    rustix::fs::fcntl_lock(&lock_file, rustix::fs::FlockOperation::LockExclusive).unwrap();
    let mut child = subprocess("publish", &contention_directory.path);
    thread::sleep(Duration::from_millis(150));
    assert!(child.try_wait().unwrap().is_none());
    rustix::fs::fcntl_lock(&lock_file, rustix::fs::FlockOperation::Unlock).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "child failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_generation(
        &contention_store.open_generation_v1().unwrap().unwrap(),
        1,
        true,
    );

    let restart_directory = TestDirectory::new();
    let output = subprocess("publish-pending", &restart_directory.path)
        .wait_with_output()
        .unwrap();
    assert!(output.status.success());
    let recovering_store = restart_directory.store();
    assert_generation(
        &recovering_store.recover_generation_v1().unwrap().unwrap(),
        1,
        true,
    );
}

#[test]
fn abrupt_death_after_canonical_rename_requires_restart_durability_recovery() {
    for action in [
        "crash-after-canonical-rename",
        "crash-before-canonical-sync",
    ] {
        for recovery_timing in [
            CompilerArtifactGenerationFaultTimingV1::Before,
            CompilerArtifactGenerationFaultTimingV1::After,
        ] {
            let directory = TestDirectory::new();
            let store = directory.store();
            let output = subprocess(action, &directory.path)
                .wait_with_output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(86),
                "{action}: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(canonical_record(&directory.path).exists());
            assert!(
                store
                    .recover_generation_v1_with_options(
                        CompilerArtifactGenerationOptionsV1::inject_fault(
                            CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                                operation: CompilerArtifactGenerationRecordOperationV1::Recover,
                                boundary:
                                    CompilerArtifactGenerationRecordBoundaryV1::SyncCanonicalName,
                                timing: recovery_timing,
                            },
                        ),
                    )
                    .is_err(),
                "{action}/{recovery_timing:?}"
            );
            assert_generation(&store.recover_generation_v1().unwrap().unwrap(), 1, true);
        }
    }
}

fn crash_publish_at_canonical_boundary(
    store: &CompilerArtifactGenerationStoreV1,
    point: CompilerArtifactGenerationFaultPointV1,
) -> ! {
    let observation = Arc::new(CompilerArtifactGenerationObservationV1::new(point));
    let crash_observation = Arc::clone(&observation);
    thread::spawn(move || {
        crash_observation.wait_until_reached();
        // SAFETY: this subprocess intentionally models abrupt process death without unwinding or
        // flushing userspace state. The parent checks and recovers only durable filesystem state.
        unsafe { libc::_exit(86) }
    });
    let _ = store.publish_generation_v1_with_options(
        &request(1, true),
        CompilerArtifactGenerationOptionsV1::observe(observation),
    );
    panic!("crash boundary unexpectedly returned")
}

#[test]
fn compiler_artifact_generation_subprocess_helper() {
    let Ok(action) = std::env::var("FE2O3_GENERATION_SUBPROCESS") else {
        return;
    };
    let root = PathBuf::from(std::env::var_os("FE2O3_GENERATION_ROOT").unwrap());
    if let Some(ready) = std::env::var_os("FE2O3_GENERATION_READY") {
        fs::write(ready, b"ready").unwrap();
    }
    let store = CompilerArtifactGenerationStoreV1::open(&root, scope()).unwrap();
    match action.as_str() {
        "publish" => {
            committed(store.publish_generation_v1(&request(1, true)));
        }
        "publish-pending" => {
            assert!(matches!(
                store.publish_generation_v1_with_options(
                    &request(1, true),
                    CompilerArtifactGenerationOptionsV1::inject_fault(
                        CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                            operation: CompilerArtifactGenerationRecordOperationV1::Commit,
                            boundary: CompilerArtifactGenerationRecordBoundaryV1::RenameTempToRedo,
                            timing: CompilerArtifactGenerationFaultTimingV1::After,
                        },
                    ),
                ),
                CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate { .. }
            ));
        }
        "crash-after-canonical-rename" => crash_publish_at_canonical_boundary(
            &store,
            CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                operation: CompilerArtifactGenerationRecordOperationV1::Commit,
                boundary: CompilerArtifactGenerationRecordBoundaryV1::RenameRedoToCanonical,
                timing: CompilerArtifactGenerationFaultTimingV1::After,
            },
        ),
        "crash-before-canonical-sync" => crash_publish_at_canonical_boundary(
            &store,
            CompilerArtifactGenerationFaultPointV1::ScopeRecord {
                operation: CompilerArtifactGenerationRecordOperationV1::Commit,
                boundary: CompilerArtifactGenerationRecordBoundaryV1::SyncCanonicalName,
                timing: CompilerArtifactGenerationFaultTimingV1::Before,
            },
        ),
        other => panic!("unknown subprocess action {other}"),
    }
}
