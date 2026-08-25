use crate::build_config::BuildConfigIdentity;
use crate::project::{PinnedDirectory, is_synthetic_dot_entry};
use cap_primitives::fs::{read_base_dir, remove_open_dir_all};
use rustix::fs::{AtFlags, FileType, FlockOperation};
use rustix::fs::{Mode, OFlags, fchmod, flock, fstat, fsync, openat, renameat, statat, unlinkat};
use sha2::{Digest, Sha256};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

const ARTIFACT_COMPONENT: &str = "fe2o3";
const LOCK_NAME: &str = ".fe2o3-generation.lock-v1";
const INTENT_NAME: &str = ".fe2o3-create-intent-v1";
const OWNER_NAME: &str = ".fe2o3-owned-v1";
const INTENT_MAGIC: &[u8] = b"fe2o3-create-intent-v1\0";
const OWNER_MAGIC: &[u8] = b"fe2o3-owned-v1\0";
const MARKER_NAME: &str = ".codegen-generation-v1";
const MARKER_MAGIC: &[u8; 28] = b"fe2o3-codegen-generation-v1\0";
const MARKER_BYTES: usize = MARKER_MAGIC.len() + 32 + 16 + 32 + 8;
const GENERATION_SEMANTIC_IDENTITY_DOMAIN_V5: &[u8] = b"fe2o3-cargo-codegen-semantics-v5";
const MAX_SNAPSHOT_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_SNAPSHOT_ENTRIES: u64 = 4_096;
const MAX_SNAPSHOT_DEPTH: usize = 64;
const MAX_SNAPSHOT_WORK: u64 = MAX_SNAPSHOT_BYTES + 16 * 1024 * 1024;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(crate) struct PreparedGeneration {
    _lock: GenerationLock,
    artifact_dir: Option<PinnedDirectory>,
    semantic: [u8; 32],
    token: [u8; 16],
    pending: bool,
}

impl PreparedGeneration {
    pub(crate) fn prepare(
        target_dir: &PinnedDirectory,
        semantic: [u8; 32],
    ) -> Result<Self, String> {
        let lock = GenerationLock::acquire(target_dir)?;
        if let Some(existing) =
            target_dir.open_child(ARTIFACT_COMPONENT, "fe2o3 artifact directory")?
        {
            recover_or_validate_artifact_guard(target_dir, &existing)?;
            make_artifact_directory_private(&existing)?;
            if let Some(token) = load_marker(&existing, semantic)? {
                return Ok(Self {
                    _lock: lock,
                    artifact_dir: Some(existing),
                    semantic,
                    token,
                    pending: false,
                });
            }
            remove_generated(existing)?;
        }

        clear_or_reject_orphaned_intent(target_dir)?;
        let guard_token = random_token(b"artifact-guard")?;
        write_fixed_record_exclusive(target_dir, INTENT_NAME, INTENT_MAGIC, guard_token)?;
        let artifact_dir =
            match target_dir.open_or_create_child(ARTIFACT_COMPONENT, "fe2o3 artifact directory") {
                Ok(directory) => directory,
                Err(error) => {
                    let _ = unlinkat(target_dir.file(), INTENT_NAME, AtFlags::empty());
                    return Err(error);
                }
            };
        if let Err(error) = make_artifact_directory_private(&artifact_dir) {
            let _ = remove_open_dir_all(artifact_dir.into_file());
            let _ = unlinkat(target_dir.file(), INTENT_NAME, AtFlags::empty());
            return Err(error);
        }
        if let Err(error) =
            write_fixed_record_exclusive(&artifact_dir, OWNER_NAME, OWNER_MAGIC, guard_token)
        {
            let _ = remove_open_dir_all(artifact_dir.into_file());
            let _ = unlinkat(target_dir.file(), INTENT_NAME, AtFlags::empty());
            return Err(error);
        }
        unlinkat(target_dir.file(), INTENT_NAME, AtFlags::empty())
            .map_err(|error| format!("failed to clear fe2o3 creation intent: {error}"))?;
        sync_directory(target_dir)?;
        Ok(Self {
            _lock: lock,
            artifact_dir: Some(artifact_dir),
            semantic,
            token: random_token(b"codegen-generation")?,
            pending: true,
        })
    }

    pub(crate) fn artifact_dir(&self) -> &PinnedDirectory {
        self.artifact_dir
            .as_ref()
            .expect("prepared generation retains its artifact directory")
    }

    pub(crate) const fn token(&self) -> [u8; 16] {
        self.token
    }

    pub(crate) fn commit(&mut self) -> Result<(), String> {
        write_marker(self.artifact_dir(), self.semantic, self.token)?;
        self.pending = false;
        Ok(())
    }

    pub(crate) fn reject_if_substituted(&self) -> Result<(), String> {
        self.artifact_dir()
            .validate_path("fe2o3 artifact directory")
    }

    pub(crate) fn discard_simulation_generation(&mut self) -> Result<(), String> {
        if !self.pending {
            return Err(
                "refusing to retain or delete a committed generation as ephemeral simulation state"
                    .to_owned(),
            );
        }
        self.discard_pending()
    }

    fn discard_pending(&mut self) -> Result<(), String> {
        if !self.pending {
            return Ok(());
        }
        let artifact_dir = self
            .artifact_dir
            .take()
            .expect("pending generation retains its artifact directory");
        remove_generated(artifact_dir)?;
        self.pending = false;
        Ok(())
    }
}

fn make_artifact_directory_private(directory: &PinnedDirectory) -> Result<(), String> {
    let normalized = directory.try_clone_for_transfer()?;
    fchmod(&normalized, Mode::RUSR | Mode::WUSR | Mode::XUSR)
        .map_err(|error| format!("failed to make fe2o3 artifact directory private: {error}"))?;
    let stat = fstat(&normalized)
        .map_err(|error| format!("failed to inspect private fe2o3 artifact directory: {error}"))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory || stat.st_mode & 0o777 != 0o700
    {
        return Err("fe2o3 artifact directory is not private 0700".to_string());
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct GenerationLock {
    _file: File,
}

impl GenerationLock {
    pub(crate) fn acquire(target_dir: &PinnedDirectory) -> Result<Self, String> {
        let descriptor = openat(
            target_dir.file(),
            LOCK_NAME,
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|error| format!("failed to open fe2o3 generation lock: {error}"))?;
        validate_private_record(target_dir, LOCK_NAME, &descriptor, Some(0))?;
        flock(&descriptor, FlockOperation::LockExclusive)
            .map_err(|error| format!("failed to acquire fe2o3 generation lock: {error}"))?;
        validate_private_record(target_dir, LOCK_NAME, &descriptor, Some(0))?;
        Ok(Self {
            _file: File::from(descriptor),
        })
    }
}

pub(crate) fn validate_owned_artifact(directory: &PinnedDirectory) -> Result<(), String> {
    read_fixed_record(directory, OWNER_NAME, OWNER_MAGIC)?.ok_or_else(|| {
        format!(
            "refusing to remove unowned fe2o3 artifact directory {}",
            directory.display_path().display()
        )
    })?;
    Ok(())
}

fn recover_or_validate_artifact_guard(
    target_dir: &PinnedDirectory,
    artifact_dir: &PinnedDirectory,
) -> Result<(), String> {
    if let Some(owner) = read_fixed_record(artifact_dir, OWNER_NAME, OWNER_MAGIC)? {
        if let Some(intent) = read_fixed_record(target_dir, INTENT_NAME, INTENT_MAGIC)? {
            if owner != intent {
                return Err(
                    "fe2o3 creation intent does not match the artifact deletion guard".to_string(),
                );
            }
            unlinkat(target_dir.file(), INTENT_NAME, AtFlags::empty())
                .map_err(|error| format!("failed to clear completed creation intent: {error}"))?;
            sync_directory(target_dir)?;
        }
        return Ok(());
    }

    let Some(intent) = read_fixed_record(target_dir, INTENT_NAME, INTENT_MAGIC)? else {
        return Err(format!(
            "refusing to replace unowned fe2o3 artifact directory {}",
            artifact_dir.display_path().display()
        ));
    };
    if !directory_is_empty(artifact_dir, "interrupted artifact directory")? {
        return Err(format!(
            "interrupted fe2o3 creation directory is not empty: {}",
            artifact_dir.display_path().display()
        ));
    }
    write_fixed_record_exclusive(artifact_dir, OWNER_NAME, OWNER_MAGIC, intent)?;
    unlinkat(target_dir.file(), INTENT_NAME, AtFlags::empty())
        .map_err(|error| format!("failed to clear recovered creation intent: {error}"))?;
    sync_directory(target_dir)
}

fn clear_or_reject_orphaned_intent(target_dir: &PinnedDirectory) -> Result<(), String> {
    match read_fixed_record(target_dir, INTENT_NAME, INTENT_MAGIC)? {
        Some(_) => {
            unlinkat(target_dir.file(), INTENT_NAME, AtFlags::empty())
                .map_err(|error| format!("failed to clear orphaned creation intent: {error}"))?;
            sync_directory(target_dir)
        }
        None => Ok(()),
    }
}

fn directory_is_empty(directory: &PinnedDirectory, kind: &str) -> Result<bool, String> {
    let mut entries = read_base_dir(directory.file())
        .map_err(|error| format!("failed to inspect {kind}: {error}"))?;
    match entries.next() {
        None => Ok(true),
        Some(Ok(_)) => Ok(false),
        Some(Err(error)) => Err(format!("failed to inspect {kind}: {error}")),
    }
}

fn write_fixed_record_exclusive(
    directory: &PinnedDirectory,
    name: &str,
    magic: &[u8],
    token: [u8; 16],
) -> Result<(), String> {
    let descriptor = openat(
        directory.file(),
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|error| format!("failed to create fe2o3 deletion-guard record {name}: {error}"))?;
    let mut file = File::from(descriptor);
    file.write_all(magic)
        .and_then(|()| file.write_all(&token))
        .and_then(|()| file.sync_all())
        .map_err(|error| {
            format!("failed to persist fe2o3 deletion-guard record {name}: {error}")
        })?;
    sync_directory(directory)
}

fn read_fixed_record(
    directory: &PinnedDirectory,
    name: &str,
    magic: &[u8],
) -> Result<Option<[u8; 16]>, String> {
    let descriptor = match openat(
        directory.file(),
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(descriptor) => descriptor,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(error) => {
            return Err(format!(
                "refusing invalid fe2o3 deletion-guard record {name}: {error}"
            ));
        }
    };
    validate_private_record(directory, name, &descriptor, Some(magic.len() + 16))?;
    let mut bytes = vec![0_u8; magic.len() + 16];
    File::from(descriptor)
        .read_exact(&mut bytes)
        .map_err(|error| format!("failed to read fe2o3 deletion-guard record {name}: {error}"))?;
    if &bytes[..magic.len()] != magic {
        return Err(format!(
            "refusing malformed fe2o3 deletion-guard record {name}"
        ));
    }
    let token: [u8; 16] = bytes[magic.len()..]
        .try_into()
        .expect("fixed deletion-guard token length");
    if token == [0; 16] {
        return Err(format!(
            "refusing zero-token fe2o3 deletion-guard record {name}"
        ));
    }
    Ok(Some(token))
}

fn validate_private_record(
    directory: &PinnedDirectory,
    name: &str,
    descriptor: &impl std::os::fd::AsFd,
    expected_size: Option<usize>,
) -> Result<(), String> {
    let opened = fstat(descriptor)
        .map_err(|error| format!("failed to inspect fe2o3 record {name}: {error}"))?;
    let linked = statat(directory.file(), name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| format!("failed to inspect linked fe2o3 record {name}: {error}"))?;
    let valid_type = FileType::from_raw_mode(opened.st_mode) == FileType::RegularFile;
    let valid_identity = opened.st_dev == linked.st_dev && opened.st_ino == linked.st_ino;
    let valid_size = expected_size.is_none_or(|size| opened.st_size == size as i64);
    let private_mode = opened.st_mode & 0o077 == 0;
    if !valid_type || opened.st_nlink != 1 || !valid_identity || !valid_size || !private_mode {
        return Err(format!("refusing unsafe fe2o3 record {name}"));
    }
    Ok(())
}

impl Drop for PreparedGeneration {
    fn drop(&mut self) {
        let _ = self.discard_pending();
    }
}

pub(crate) fn semantic_identity(
    target: &str,
    compiler_closure_sha256: &[u8; 32],
    build_config: Option<BuildConfigIdentity>,
    cargo_configuration: &[u8],
) -> Result<[u8; 32], String> {
    let mut hash = Sha256::new();
    update_hash(&mut hash, GENERATION_SEMANTIC_IDENTITY_DOMAIN_V5);
    update_hash(&mut hash, target.as_bytes());
    update_hash(&mut hash, compiler_closure_sha256);
    match build_config {
        Some(identity) => {
            update_hash(&mut hash, b"build-config");
            update_hash(&mut hash, identity.as_bytes());
        }
        None => update_hash(&mut hash, b"no-build-config"),
    }
    update_hash(&mut hash, cargo_configuration);

    let mut environment = std::env::vars_os()
        .filter(|(key, _)| os_bytes(key).starts_with(b"FE2O3_"))
        .filter(|(key, _)| {
            !matches!(
                key.to_str(),
                Some(
                    "FE2O3_BUILD_SESSION_V1" | "FE2O3_HSACO_DIR" | "FE2O3_BINDING_WRAPPER_MODE_V1"
                )
            )
        })
        .collect::<Vec<_>>();
    environment.sort_by(|(left, _), (right, _)| os_bytes(left).cmp(os_bytes(right)));
    for (key, value) in environment {
        update_hash(&mut hash, os_bytes(&key));
        update_hash(&mut hash, os_bytes(&value));
    }
    let mut cargo_environment = std::env::vars_os()
        .filter(|(key, _)| {
            let key = os_bytes(key);
            key.starts_with(b"CARGO_") || key.starts_with(b"RUST")
        })
        .collect::<Vec<_>>();
    cargo_environment.sort_by(|(left, _), (right, _)| os_bytes(left).cmp(os_bytes(right)));
    for (key, value) in cargo_environment {
        update_hash(&mut hash, os_bytes(&key));
        update_hash(&mut hash, os_bytes(&value));
    }
    Ok(hash.finalize().into())
}

pub(crate) fn managed_rustc_args(backend: &Path, generation: [u8; 16]) -> Result<OsString, String> {
    let mut flags = Vec::<Vec<u8>>::new();
    flags.push(b"-Zmir-enable-passes=-JumpThreading".to_vec());
    flags.push(b"--cfg".to_vec());
    flags.push(format!("fe2o3_codegen_generation=\"{}\"", hex(&generation)).into_bytes());
    let mut backend_flag = b"-Zcodegen-backend=".to_vec();
    backend_flag.extend_from_slice(os_bytes(backend.as_os_str()));
    flags.push(backend_flag);
    if flags.iter().any(|flag| flag.contains(&0x1f)) {
        return Err("managed rustc arguments contain the separator byte".to_string());
    }

    let mut encoded = Vec::new();
    for (index, flag) in flags.iter().enumerate() {
        if index != 0 {
            encoded.push(0x1f);
        }
        encoded.extend_from_slice(flag);
    }
    os_string(encoded)
}

fn load_marker(
    directory: &PinnedDirectory,
    expected_semantic: [u8; 32],
) -> Result<Option<[u8; 16]>, String> {
    let descriptor = match openat(
        directory.file(),
        MARKER_NAME,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(descriptor) => descriptor,
        Err(rustix::io::Errno::NOENT) => {
            snapshot(directory)?;
            return Ok(None);
        }
        Err(error) => {
            return Err(format!("failed to open codegen generation marker: {error}"));
        }
    };
    let mut file = File::from(descriptor);
    validate_marker(directory, &file)?;
    let mut bytes = [0_u8; MARKER_BYTES];
    file.read_exact(&mut bytes)
        .map_err(|error| format!("failed to read codegen generation marker: {error}"))?;
    if &bytes[..MARKER_MAGIC.len()] != MARKER_MAGIC {
        return Err("refusing malformed codegen generation marker".to_string());
    }
    let semantic_start = MARKER_MAGIC.len();
    let semantic_end = semantic_start + 32;
    let semantic: [u8; 32] = bytes[semantic_start..semantic_end]
        .try_into()
        .expect("fixed generation semantic length");
    let token_end = semantic_end + 16;
    let token = bytes[semantic_end..token_end]
        .try_into()
        .expect("fixed generation token length");
    if token == [0; 16] {
        return Err("refusing zero-token codegen generation marker".to_string());
    }
    let snapshot_end = token_end + 32;
    let expected_snapshot: [u8; 32] = bytes[token_end..snapshot_end]
        .try_into()
        .expect("fixed snapshot identity length");
    let expected_entries = u64::from_le_bytes(
        bytes[snapshot_end..]
            .try_into()
            .expect("fixed snapshot count length"),
    );
    if expected_entries > MAX_SNAPSHOT_ENTRIES {
        return Err("refusing out-of-range codegen generation marker entry count".to_string());
    }
    let (snapshot, entries) = snapshot(directory)?;
    validate_marker(directory, &file)?;
    if semantic != expected_semantic || snapshot != expected_snapshot || entries != expected_entries
    {
        return Ok(None);
    }
    Ok(Some(token))
}

fn validate_marker(directory: &PinnedDirectory, marker: &File) -> Result<(), String> {
    let opened = fstat(marker)
        .map_err(|error| format!("failed to inspect codegen generation marker: {error}"))?;
    let linked = statat(directory.file(), MARKER_NAME, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| format!("failed to inspect linked codegen generation marker: {error}"))?;
    if FileType::from_raw_mode(opened.st_mode) != FileType::RegularFile
        || opened.st_nlink != 1
        || opened.st_size != MARKER_BYTES as i64
        || opened.st_dev != linked.st_dev
        || opened.st_ino != linked.st_ino
        || opened.st_mode & 0o077 != 0
    {
        return Err("refusing unsafe codegen generation marker".to_string());
    }
    Ok(())
}

fn write_marker(
    directory: &PinnedDirectory,
    semantic: [u8; 32],
    token: [u8; 16],
) -> Result<(), String> {
    let (snapshot, entries) = snapshot(directory)?;
    let temp = format!(
        "{MARKER_NAME}.tmp-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    );
    let result = (|| {
        let descriptor = openat(
            directory.file(),
            &temp,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|error| format!("failed to create codegen generation marker: {error}"))?;
        let mut file = File::from(descriptor);
        file.write_all(MARKER_MAGIC)
            .and_then(|()| file.write_all(&semantic))
            .and_then(|()| file.write_all(&token))
            .and_then(|()| file.write_all(&snapshot))
            .and_then(|()| file.write_all(&entries.to_le_bytes()))
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("failed to write codegen generation marker: {error}"))?;
        renameat(directory.file(), &temp, directory.file(), MARKER_NAME)
            .map_err(|error| format!("failed to publish codegen generation marker: {error}"))?;
        sync_directory(directory)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = unlinkat(directory.file(), &temp, AtFlags::empty());
    }
    result
}

fn snapshot(directory: &PinnedDirectory) -> Result<([u8; 32], u64), String> {
    snapshot_with_limits(directory, SnapshotLimits::PRODUCTION)
}

#[derive(Clone, Copy)]
struct SnapshotLimits {
    directory_entries: usize,
    entries: u64,
    bytes: u64,
    work: u64,
    depth: usize,
}

impl SnapshotLimits {
    const PRODUCTION: Self = Self {
        directory_entries: MAX_SNAPSHOT_ENTRIES as usize,
        entries: MAX_SNAPSHOT_ENTRIES,
        bytes: MAX_SNAPSHOT_BYTES,
        work: MAX_SNAPSHOT_WORK,
        depth: MAX_SNAPSHOT_DEPTH,
    };
}

fn snapshot_with_limits(
    directory: &PinnedDirectory,
    limits: SnapshotLimits,
) -> Result<([u8; 32], u64), String> {
    let mut hash = Sha256::new();
    update_hash(&mut hash, b"fe2o3-generated-artifact-snapshot-v1");
    let mut state = SnapshotState {
        entries: 0,
        bytes: 0,
        work: 0,
    };
    snapshot_directory(directory.file(), &mut hash, &mut state, limits)?;
    Ok((hash.finalize().into(), state.entries))
}

struct SnapshotState {
    entries: u64,
    bytes: u64,
    work: u64,
}

impl SnapshotState {
    fn admit_entry(&mut self, name_bytes: usize, limits: SnapshotLimits) -> Result<(), String> {
        let entries = self
            .entries
            .checked_add(1)
            .filter(|entries| *entries <= limits.entries)
            .ok_or_else(|| "generated artifact snapshot has too many entries".to_string())?;
        let name_work = u64::try_from(name_bytes)
            .ok()
            .and_then(|bytes| bytes.checked_add(1))
            .ok_or_else(|| "generated artifact snapshot work counter overflowed".to_string())?;
        let work = self
            .work
            .checked_add(name_work)
            .filter(|work| *work <= limits.work)
            .ok_or_else(|| "generated artifact snapshot exceeds its work bound".to_string())?;
        self.entries = entries;
        self.work = work;
        Ok(())
    }

    fn admit_file(&mut self, size: u64, limits: SnapshotLimits) -> Result<(), String> {
        let bytes = self
            .bytes
            .checked_add(size)
            .filter(|bytes| *bytes <= limits.bytes)
            .ok_or_else(|| "generated artifact snapshot is too large".to_string())?;
        let work = self
            .work
            .checked_add(size)
            .filter(|work| *work <= limits.work)
            .ok_or_else(|| "generated artifact snapshot exceeds its work bound".to_string())?;
        self.bytes = bytes;
        self.work = work;
        Ok(())
    }
}

struct SnapshotFrame {
    directory: File,
    names: std::vec::IntoIter<OsString>,
    depth: usize,
}

fn snapshot_directory(
    directory: &File,
    hash: &mut Sha256,
    state: &mut SnapshotState,
    limits: SnapshotLimits,
) -> Result<(), String> {
    let root = openat(
        directory,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| format!("failed to pin generated artifact root: {error}"))?;
    let names = snapshot_names(&root, true, state, limits)?;
    let mut stack = Vec::new();
    stack
        .try_reserve(1)
        .map_err(|_| "failed to reserve generated artifact traversal stack".to_string())?;
    stack.push(SnapshotFrame {
        directory: root,
        names: names.into_iter(),
        depth: 0,
    });

    while let Some(frame) = stack.last_mut() {
        let Some(name) = frame.names.next() else {
            stack.pop();
            continue;
        };
        update_hash(hash, os_bytes(&name));
        let descriptor = openat(
            &frame.directory,
            Path::new(&name),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| format!("failed to pin generated artifact {name:?}: {error}"))?;
        let stat = fstat(&descriptor)
            .map_err(|error| format!("failed to inspect generated artifact {name:?}: {error}"))?;
        match FileType::from_raw_mode(stat.st_mode) {
            FileType::RegularFile => {
                hash.update(b"file");
                let size = u64::try_from(stat.st_size)
                    .map_err(|_| format!("generated artifact has a negative size: {name:?}"))?;
                state.admit_file(size, limits)?;
                hash.update(size.to_le_bytes());
                let mut file = File::from(descriptor);
                let mut remaining = size;
                let mut chunk = [0_u8; 64 * 1024];
                while remaining != 0 {
                    let limit = usize::try_from(remaining.min(chunk.len() as u64))
                        .expect("bounded snapshot read size");
                    let read = file.read(&mut chunk[..limit]).map_err(|error| {
                        format!("failed to read generated artifact {name:?}: {error}")
                    })?;
                    if read == 0 {
                        return Err(format!(
                            "generated artifact shortened while hashing: {name:?}"
                        ));
                    }
                    hash.update(&chunk[..read]);
                    remaining -= read as u64;
                }
            }
            FileType::Directory => {
                hash.update(b"directory");
                let depth = frame
                    .depth
                    .checked_add(1)
                    .filter(|depth| *depth <= limits.depth)
                    .ok_or_else(|| {
                        "generated artifact snapshot exceeds its depth bound".to_string()
                    })?;
                let directory = File::from(descriptor);
                let names = snapshot_names(&directory, false, state, limits)?;
                stack.try_reserve(1).map_err(|_| {
                    "failed to reserve generated artifact traversal stack".to_string()
                })?;
                stack.push(SnapshotFrame {
                    directory,
                    names: names.into_iter(),
                    depth,
                });
            }
            _ => {
                return Err(format!(
                    "generated artifact is not a regular file or directory: {name:?}"
                ));
            }
        }
    }
    Ok(())
}

fn snapshot_names(
    directory: &File,
    root: bool,
    state: &mut SnapshotState,
    limits: SnapshotLimits,
) -> Result<Vec<OsString>, String> {
    let scan = openat(
        directory,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| format!("failed to pin generated artifact directory scan: {error}"))?;
    let mut entries = rustix::fs::Dir::read_from(&scan)
        .map_err(|error| format!("failed to enumerate generated artifacts: {error}"))?;
    let mut visible_entries = 0_usize;
    let mut names = Vec::new();
    for entry in &mut entries {
        let entry =
            entry.map_err(|error| format!("failed to enumerate a generated artifact: {error}"))?;
        let bytes = entry.file_name().to_bytes();
        if is_synthetic_dot_entry(bytes) {
            continue;
        }
        visible_entries = visible_entries
            .checked_add(1)
            .filter(|entries| *entries <= limits.directory_entries)
            .ok_or_else(|| "generated artifact directory exceeds its scan bound".to_string())?;
        if root && bytes == MARKER_NAME.as_bytes() {
            continue;
        }
        state.admit_entry(bytes.len(), limits)?;
        names
            .try_reserve(1)
            .map_err(|_| "failed to reserve generated artifact directory scan".to_string())?;
        names.push(copy_file_name(bytes)?);
    }
    names.sort_by(|left, right| os_bytes(left).cmp(os_bytes(right)));
    Ok(names)
}

#[cfg(unix)]
fn copy_file_name(bytes: &[u8]) -> Result<OsString, String> {
    use std::os::unix::ffi::OsStrExt;

    let mut name = OsString::new();
    name.try_reserve(bytes.len())
        .map_err(|_| "failed to reserve generated artifact name".to_string())?;
    name.push(OsStr::from_bytes(bytes));
    Ok(name)
}

#[cfg(not(unix))]
fn copy_file_name(bytes: &[u8]) -> Result<OsString, String> {
    let value = std::str::from_utf8(bytes)
        .map_err(|_| "generated artifact name is not valid UTF-8".to_string())?;
    let mut name = OsString::new();
    name.try_reserve(value.len())
        .map_err(|_| "failed to reserve generated artifact name".to_string())?;
    name.push(value);
    Ok(name)
}

fn sync_directory(directory: &PinnedDirectory) -> Result<(), String> {
    let descriptor = openat(
        directory.file(),
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| format!("failed to open codegen generation directory for sync: {error}"))?;
    fsync(&descriptor)
        .map_err(|error| format!("failed to sync codegen generation directory: {error}"))
}

fn remove_generated(directory: PinnedDirectory) -> Result<(), String> {
    validate_owned_artifact(&directory)?;
    let path = directory.display_path().to_path_buf();
    remove_open_dir_all(directory.into_file()).map_err(|error| {
        format!(
            "failed to remove opened stale fe2o3 artifact directory {}: {error}",
            path.display()
        )
    })
}

fn random_token(label: &[u8]) -> Result<[u8; 16], String> {
    if crate::non_production_reproduction::enabled() {
        return Ok(crate::non_production_reproduction::deterministic_16(label));
    }
    for _ in 0..8 {
        let mut token = [0_u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut source| source.read_exact(&mut token))
            .map_err(|error| format!("failed to obtain a codegen generation nonce: {error}"))?;
        if token != [0; 16] {
            return Ok(token);
        }
    }
    Err("failed to obtain a nonzero codegen generation nonce".to_string())
}

fn update_hash(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(unix)]
fn os_bytes(value: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes()
}

#[cfg(not(unix))]
fn os_bytes(value: &OsStr) -> &[u8] {
    value
        .to_str()
        .expect("non-Unix Cargo environment values must be UTF-8")
        .as_bytes()
}

#[cfg(unix)]
fn os_string(value: Vec<u8>) -> Result<OsString, String> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(value))
}

#[cfg(not(unix))]
fn os_string(value: Vec<u8>) -> Result<OsString, String> {
    String::from_utf8(value)
        .map(OsString::from)
        .map_err(|_| "encoded rustflags are not valid UTF-8 on this platform".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler_toolchain::compiler_closure_v2_from_pins;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
            let path = std::env::temp_dir().join(format!(
                "cargo-fe2o3-generation-snapshot-{}-{}",
                std::process::id(),
                NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn pinned(&self) -> PinnedDirectory {
            PinnedDirectory::open_existing(self.0.clone(), "snapshot test directory").unwrap()
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn committed_generation() -> (TestDirectory, [u8; 32]) {
        let directory = TestDirectory::new();
        let semantic = [0x5a; 32];
        let mut generation = PreparedGeneration::prepare(&directory.pinned(), semantic).unwrap();
        fs::write(directory.0.join("fe2o3/payload"), b"keep").unwrap();
        generation.commit().unwrap();
        drop(generation);
        (directory, semantic)
    }

    fn rejected_prepare(directory: &TestDirectory, semantic: [u8; 32]) -> String {
        match PreparedGeneration::prepare(&directory.pinned(), semantic) {
            Ok(_) => panic!("hostile committed generation unexpectedly allowed a rebuild"),
            Err(error) => error,
        }
    }

    fn assert_committed_artifacts_preserved(directory: &TestDirectory) {
        assert_eq!(
            fs::read(directory.0.join("fe2o3/payload")).unwrap(),
            b"keep"
        );
        assert!(directory.0.join("fe2o3/.fe2o3-owned-v1").is_file());
        assert!(directory.0.join("fe2o3/.codegen-generation-v1").exists());
    }

    #[test]
    fn semantic_v5_preserves_the_v1_marker_wire() {
        assert_eq!(
            GENERATION_SEMANTIC_IDENTITY_DOMAIN_V5,
            b"fe2o3-cargo-codegen-semantics-v5"
        );
        assert_eq!(MARKER_MAGIC, b"fe2o3-codegen-generation-v1\0");
        assert_eq!(MARKER_BYTES, 116);
        assert_eq!(hex(&[0x00, 0x7f, 0xff]), "007fff");
    }

    #[test]
    fn simulation_generation_is_explicitly_ephemeral_and_never_deletes_committed_state() {
        let directory = TestDirectory::new();
        let semantic = [0x6b; 32];
        let mut ephemeral = PreparedGeneration::prepare(&directory.pinned(), semantic).unwrap();
        fs::write(directory.0.join("fe2o3/transient"), b"kir").unwrap();
        ephemeral.discard_simulation_generation().unwrap();
        assert!(!directory.0.join("fe2o3").exists());
        assert!(ephemeral.discard_simulation_generation().is_err());

        let (committed, semantic) = committed_generation();
        let mut retained = PreparedGeneration::prepare(&committed.pinned(), semantic).unwrap();
        assert!(retained.discard_simulation_generation().is_err());
        assert_committed_artifacts_preserved(&committed);
    }

    #[test]
    fn generation_semantics_bind_both_v2_wrapper_images() {
        let baseline = compiler_closure_v2_from_pins(
            &[1; 32], &[2; 32], &[3; 32], &[4; 32], &[5; 32], &[6; 32],
        )
        .unwrap();
        let changed_trampoline = compiler_closure_v2_from_pins(
            &[1; 32], &[7; 32], &[3; 32], &[4; 32], &[5; 32], &[6; 32],
        )
        .unwrap();
        let changed_wrapper = compiler_closure_v2_from_pins(
            &[1; 32], &[2; 32], &[7; 32], &[4; 32], &[5; 32], &[6; 32],
        )
        .unwrap();

        let semantic = |closure: fe2o3_build_authority::CompilerClosureV2| {
            semantic_identity(
                "gfx942",
                &closure.identity_sha256(),
                None,
                b"release-configuration",
            )
            .unwrap()
        };
        let baseline = semantic(baseline);

        assert_ne!(baseline, semantic(changed_trampoline));
        assert_ne!(baseline, semantic(changed_wrapper));
    }

    #[test]
    fn snapshot_accepts_exact_real_entry_limit_and_rejects_limit_plus_one() {
        let directory = TestDirectory::new();
        for index in 0..MAX_SNAPSHOT_ENTRIES as usize {
            fs::write(directory.0.join(format!("entry-{index:04}")), []).unwrap();
        }
        let (_, entries) = snapshot(&directory.pinned()).unwrap();
        assert_eq!(entries, MAX_SNAPSHOT_ENTRIES);

        fs::write(directory.0.join("entry-over-limit"), []).unwrap();
        let error = snapshot(&directory.pinned()).unwrap_err();
        assert!(
            error.contains("directory exceeds its scan bound"),
            "{error}"
        );
    }

    #[test]
    fn snapshot_rejects_sparse_payload_above_byte_limit_before_reading() {
        let directory = TestDirectory::new();
        File::create(directory.0.join("huge"))
            .unwrap()
            .set_len(MAX_SNAPSHOT_BYTES + 1)
            .unwrap();

        let error = snapshot(&directory.pinned()).unwrap_err();
        assert!(error.contains("snapshot is too large"), "{error}");
    }

    #[test]
    fn snapshot_enforces_independent_work_limit() {
        let directory = TestDirectory::new();
        fs::write(directory.0.join("payload"), b"12345678").unwrap();
        let limits = SnapshotLimits {
            directory_entries: 1,
            entries: 1,
            bytes: 8,
            work: 15,
            depth: 0,
        };

        let error = snapshot_with_limits(&directory.pinned(), limits).unwrap_err();
        assert!(error.contains("work bound"), "{error}");
    }

    #[test]
    fn snapshot_depth_plus_one_fails_on_a_small_thread_stack() {
        let directory = TestDirectory::new();
        let mut child = directory.0.clone();
        for index in 0..=MAX_SNAPSHOT_DEPTH {
            child.push(format!("depth-{index:02}"));
            fs::create_dir(&child).unwrap();
        }
        let path = directory.0.clone();

        let error = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(move || {
                let pinned =
                    PinnedDirectory::open_existing(path, "small-stack snapshot directory").unwrap();
                snapshot(&pinned).unwrap_err()
            })
            .unwrap()
            .join()
            .unwrap();
        assert!(error.contains("depth bound"), "{error}");
    }

    #[test]
    fn snapshot_rejects_symlinks_special_files_and_io_failures() {
        let directory = TestDirectory::new();
        fs::write(directory.0.join("target"), b"keep").unwrap();
        let hostile = directory.0.join("hostile");
        symlink("target", &hostile).unwrap();
        assert!(snapshot(&directory.pinned()).is_err());

        fs::remove_file(&hostile).unwrap();
        let listener = UnixListener::bind(&hostile).unwrap();
        assert!(snapshot(&directory.pinned()).is_err());
        drop(listener);
        fs::remove_file(&hostile).unwrap();

        fs::write(&hostile, b"unreadable").unwrap();
        fs::set_permissions(&hostile, fs::Permissions::from_mode(0o000)).unwrap();
        let result = snapshot(&directory.pinned());
        fs::set_permissions(&hostile, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn snapshot_counter_overflow_is_fallible() {
        let mut state = SnapshotState {
            entries: u64::MAX,
            bytes: 0,
            work: 0,
        };
        assert!(state.admit_entry(1, SnapshotLimits::PRODUCTION).is_err());

        let mut state = SnapshotState {
            entries: 0,
            bytes: u64::MAX,
            work: 0,
        };
        assert!(state.admit_file(1, SnapshotLimits::PRODUCTION).is_err());

        let mut state = SnapshotState {
            entries: 0,
            bytes: 0,
            work: u64::MAX,
        };
        assert!(
            state
                .admit_entry(usize::MAX, SnapshotLimits::PRODUCTION)
                .is_err()
        );
    }

    #[test]
    fn valid_snapshot_mismatch_is_the_only_destructive_rebuild_path() {
        let (directory, semantic) = committed_generation();
        fs::write(directory.0.join("fe2o3/payload"), b"changed").unwrap();

        let generation = PreparedGeneration::prepare(&directory.pinned(), semantic).unwrap();
        assert!(generation.pending);
        assert!(!directory.0.join("fe2o3/payload").exists());
        assert!(directory.0.join("fe2o3/.fe2o3-owned-v1").is_file());
    }

    #[test]
    fn over_limit_marker_scan_error_preserves_committed_artifacts() {
        let (directory, semantic) = committed_generation();
        let artifact = directory.0.join("fe2o3");
        let existing_entries = 3;
        for index in 0..=MAX_SNAPSHOT_ENTRIES as usize - existing_entries {
            fs::write(artifact.join(format!("filler-{index:04}")), []).unwrap();
        }

        let error = rejected_prepare(&directory, semantic);
        assert!(error.contains("scan bound"), "{error}");
        assert_committed_artifacts_preserved(&directory);
    }

    #[test]
    fn depth_and_byte_marker_scan_errors_preserve_committed_artifacts() {
        let (directory, semantic) = committed_generation();
        let mut child = directory.0.join("fe2o3");
        for index in 0..=MAX_SNAPSHOT_DEPTH {
            child.push(format!("depth-{index:02}"));
            fs::create_dir(&child).unwrap();
        }
        let error = rejected_prepare(&directory, semantic);
        assert!(error.contains("depth bound"), "{error}");
        assert_committed_artifacts_preserved(&directory);

        fs::remove_dir_all(directory.0.join("fe2o3/depth-00")).unwrap();
        File::create(directory.0.join("fe2o3/huge"))
            .unwrap()
            .set_len(MAX_SNAPSHOT_BYTES + 1)
            .unwrap();
        let error = rejected_prepare(&directory, semantic);
        assert!(error.contains("snapshot is too large"), "{error}");
        assert_committed_artifacts_preserved(&directory);
    }

    #[test]
    fn hostile_entry_and_io_marker_scan_errors_preserve_committed_artifacts() {
        let (directory, semantic) = committed_generation();
        let hostile = directory.0.join("fe2o3/hostile");
        symlink("payload", &hostile).unwrap();
        assert!(rejected_prepare(&directory, semantic).contains("generated artifact"));
        assert_committed_artifacts_preserved(&directory);

        fs::remove_file(&hostile).unwrap();
        let listener = UnixListener::bind(&hostile).unwrap();
        assert!(rejected_prepare(&directory, semantic).contains("generated artifact"));
        assert_committed_artifacts_preserved(&directory);
        drop(listener);
        fs::remove_file(&hostile).unwrap();

        fs::write(&hostile, b"unreadable").unwrap();
        fs::set_permissions(&hostile, fs::Permissions::from_mode(0o000)).unwrap();
        let error = rejected_prepare(&directory, semantic);
        fs::set_permissions(&hostile, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(error.contains("generated artifact"), "{error}");
        assert_committed_artifacts_preserved(&directory);
    }

    #[test]
    fn malformed_or_unsafe_marker_preserves_committed_artifacts() {
        let (directory, semantic) = committed_generation();
        let marker = directory.0.join("fe2o3/.codegen-generation-v1");
        fs::write(&marker, b"truncated").unwrap();

        let error = rejected_prepare(&directory, semantic);
        assert!(
            error.contains("unsafe codegen generation marker"),
            "{error}"
        );
        assert_committed_artifacts_preserved(&directory);

        fs::remove_file(&marker).unwrap();
        symlink("payload", &marker).unwrap();
        let error = rejected_prepare(&directory, semantic);
        assert!(
            error.contains("failed to open codegen generation marker"),
            "{error}"
        );
        assert_committed_artifacts_preserved(&directory);
    }
}
