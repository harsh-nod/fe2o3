//! Inert consistency observations for a parent-prepared command and its child process.
//!
//! The narrow S09 pilot uses this crate on both sides of `execve`. The parent records the
//! descriptor-pinned executable and cwd objects, exact raw argv, complete environment, and one
//! bounded protected-source measurement. The backend process independently observes those same
//! values and compares its digest with the parent's sealed expectation.
//!
//! Agreement is not authenticated execution history. In particular, backend code runs after the
//! kernel, ELF interpreter, and loader have acted, so it cannot authenticate pre-backend loader
//! behavior. Establishing that authority requires a protected supervisor outside this mechanism.

#[cfg(not(target_os = "linux"))]
compile_error!("fe2o3-process-identity requires Linux descriptor and procfs semantics");

use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{File, Metadata};
use std::io::{Read, Seek, SeekFrom};
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path};
use std::process::Command;

use sha2::{Digest, Sha256};

mod protected_rustc;
mod sealed_memfd;

pub use protected_rustc::{
    CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2, EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1,
    ProtectedRustcProcessValidationErrorV1, validate_protected_rustc_process_observation_v1,
};
pub use sealed_memfd::{
    EXACT_IMMUTABLE_MEMFD_SEALS_V1, ImmutableMemfdBusyPolicyV1, ImmutableMemfdSealErrorV1,
    ImmutableMemfdSealStageV1, seal_immutable_memfd_v1,
};

pub const S09_PROCESS_CONSISTENCY_EXPECTATION_FD_V3: RawFd = 194;
// Measurement streams the executable through a fixed-size buffer. Keep a
// finite admission bound while accommodating large rustc and debug binaries.
pub const MAX_EXECUTABLE_BYTES_V3: u64 = 1024 * 1024 * 1024;
pub const MAX_PROTECTED_SOURCE_BYTES_V3: u64 = 16 * 1024 * 1024;
pub const MAX_ARGUMENTS_V3: usize = 65_536;
pub const MAX_ENVIRONMENT_ENTRIES_V3: usize = 4_096;
pub const MAX_FIELD_BYTES_V3: usize = 1024 * 1024;
pub const MAX_ENCODED_CONSISTENCY_BYTES_V3: usize = 8 * 1024 * 1024;

const CONSISTENCY_DOMAIN_V3: &[u8] = b"FE2O3/PARENT-PREPARED-CHILD-OBSERVED/V3\0";
const SOURCE_TREE_DOMAIN_V3: &[u8] = b"FE2O3/PROTECTED-SOURCE-TREE-OBSERVATION/V3\0";
const HASH_CHUNK_BYTES: usize = 64 * 1024;
const MAX_SOURCE_PATH_BYTES: usize = 4096;
const MAX_SOURCE_PATH_COMPONENTS: usize = 64;

#[derive(Debug)]
pub struct ProcessIdentityError(String);

impl ProcessIdentityError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ProcessIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ProcessIdentityError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinuxObjectIdentityV3 {
    device: u64,
    inode: u64,
    mode: u32,
}

impl LinuxObjectIdentityV3 {
    pub const fn from_linux_stat(device: u64, inode: u64, mode: u32) -> Self {
        Self {
            device,
            inode,
            mode,
        }
    }

    fn from_metadata(metadata: &Metadata) -> Self {
        Self::from_linux_stat(metadata.dev(), metadata.ino(), metadata.mode())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileSnapshot {
    object: LinuxObjectIdentityV3,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl FileSnapshot {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            object: LinuxObjectIdentityV3::from_metadata(metadata),
            size: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

/// Inputs for an inert parent-prepared/child-observed consistency digest.
pub struct ParentPreparedProcessConsistencyV3<'a> {
    pub executable_object: LinuxObjectIdentityV3,
    pub executable_sha256: [u8; 32],
    /// Exact configured argv, including the raw argv0 at index zero.
    pub argv: &'a [OsString],
    pub current_dir_object: LinuxObjectIdentityV3,
    pub protected_source_tree_sha256: [u8; 32],
    /// Complete environment, sorted strictly by raw variable-name bytes.
    pub environment: &'a [(OsString, OsString)],
}

/// A cwd object retained through child setup. It grants no authority over directory contents.
pub struct PinnedWorkingDirectoryV3 {
    file: File,
    object: LinuxObjectIdentityV3,
}

impl PinnedWorkingDirectoryV3 {
    pub fn open(path: &Path) -> Result<Self, ProcessIdentityError> {
        let file = File::from(
            rustix::fs::open(
                path,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(|error| {
                ProcessIdentityError::new(format!(
                    "cannot pin working directory {}: {error}",
                    path.display()
                ))
            })?,
        );
        let metadata = file.metadata().map_err(|error| {
            ProcessIdentityError::new(format!(
                "cannot inspect working directory {}: {error}",
                path.display()
            ))
        })?;
        if !metadata.is_dir() {
            return Err(ProcessIdentityError::new(format!(
                "working directory {} is not a directory",
                path.display()
            )));
        }
        Ok(Self {
            object: LinuxObjectIdentityV3::from_metadata(&metadata),
            file,
        })
    }

    pub const fn object_identity(&self) -> LinuxObjectIdentityV3 {
        self.object
    }

    /// Installs a final descriptor-based `fchdir` immediately before exec.
    pub fn configure_child_fchdir(&self, command: &mut Command) {
        let descriptor = self.file.as_raw_fd();
        let expected = self.object;
        // SAFETY: `self` remains borrowed through spawn; the callback performs async-signal-safe
        // descriptor operations and does not allocate.
        unsafe {
            command.pre_exec(move || {
                let stat = rustix::fs::fstat(BorrowedFd::borrow_raw(descriptor))
                    .map_err(std::io::Error::from)?;
                let observed =
                    LinuxObjectIdentityV3::from_linux_stat(stat.st_dev, stat.st_ino, stat.st_mode);
                if observed != expected {
                    return Err(std::io::Error::from_raw_os_error(
                        rustix::io::Errno::STALE.raw_os_error(),
                    ));
                }
                if libc::fchdir(descriptor) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    /// Measures one canonical relative source beneath this exact directory object.
    pub fn measure_protected_source_tree(
        &self,
        relative_source: &Path,
    ) -> Result<ProtectedSourceTreeMeasurementV3, ProcessIdentityError> {
        let components = canonical_source_components(relative_source)?;
        let mut directory = self.file.try_clone().map_err(|error| {
            ProcessIdentityError::new(format!("cannot clone pinned working directory: {error}"))
        })?;
        for component in &components[..components.len() - 1] {
            directory = File::from(
                rustix::fs::openat(
                    &directory,
                    *component,
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .map_err(|error| {
                    ProcessIdentityError::new(format!(
                        "cannot traverse protected source {}: {error}",
                        relative_source.display()
                    ))
                })?,
            );
        }
        let mut source = File::from(
            rustix::fs::openat(
                &directory,
                *components.last().expect("source has one component"),
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::NONBLOCK
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(|error| {
                ProcessIdentityError::new(format!(
                    "cannot open protected source {}: {error}",
                    relative_source.display()
                ))
            })?,
        );
        let (source_sha256, source_bytes, _) = measure_open_regular_file(
            &mut source,
            MAX_PROTECTED_SOURCE_BYTES_V3,
            "protected source",
        )?;
        let identity_sha256 = protected_source_tree_identity_v3(
            self.object,
            relative_source,
            source_sha256,
            source_bytes,
        )?;
        Ok(ProtectedSourceTreeMeasurementV3 {
            identity_sha256,
            source_sha256,
            source_bytes,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedSourceTreeMeasurementV3 {
    identity_sha256: [u8; 32],
    source_sha256: [u8; 32],
    source_bytes: u64,
}

impl ProtectedSourceTreeMeasurementV3 {
    pub const fn identity_sha256(&self) -> [u8; 32] {
        self.identity_sha256
    }

    pub const fn source_sha256(&self) -> [u8; 32] {
        self.source_sha256
    }

    pub const fn source_bytes(&self) -> u64 {
        self.source_bytes
    }
}

pub fn protected_source_tree_identity_v3(
    current_dir_object: LinuxObjectIdentityV3,
    relative_source: &Path,
    source_sha256: [u8; 32],
    source_bytes: u64,
) -> Result<[u8; 32], ProcessIdentityError> {
    canonical_source_components(relative_source)?;
    if source_sha256 == [0; 32] || source_bytes == 0 || source_bytes > MAX_PROTECTED_SOURCE_BYTES_V3
    {
        return Err(ProcessIdentityError::new(
            "protected source has an invalid bounded identity",
        ));
    }
    let mut encoder = DigestEncoder::new(SOURCE_TREE_DOMAIN_V3)?;
    encoder.object(current_dir_object)?;
    encoder.field(relative_source.as_os_str())?;
    encoder.fixed(&source_bytes.to_le_bytes())?;
    encoder.fixed(&source_sha256)?;
    Ok(encoder.finish())
}

pub fn parent_prepared_process_consistency_digest_v3(
    inputs: &ParentPreparedProcessConsistencyV3<'_>,
) -> Result<[u8; 32], ProcessIdentityError> {
    if inputs.executable_sha256 == [0; 32] || inputs.protected_source_tree_sha256 == [0; 32] {
        return Err(ProcessIdentityError::new(
            "process consistency digests must not be zero",
        ));
    }
    if inputs.argv.is_empty() || inputs.argv.len() > MAX_ARGUMENTS_V3 {
        return Err(ProcessIdentityError::new(
            "process consistency argv has an invalid count",
        ));
    }
    if inputs.environment.len() > MAX_ENVIRONMENT_ENTRIES_V3 {
        return Err(ProcessIdentityError::new(
            "process consistency environment has too many entries",
        ));
    }
    for pair in inputs.environment.windows(2) {
        if os_bytes(&pair[0].0) >= os_bytes(&pair[1].0) {
            return Err(ProcessIdentityError::new(
                "process consistency environment names must be strictly sorted",
            ));
        }
    }

    let mut encoder = DigestEncoder::new(CONSISTENCY_DOMAIN_V3)?;
    encoder.object(inputs.executable_object)?;
    encoder.fixed(&inputs.executable_sha256)?;
    encoder.fixed(&(inputs.argv.len() as u64).to_le_bytes())?;
    for argument in inputs.argv {
        encoder.field(argument)?;
    }
    encoder.object(inputs.current_dir_object)?;
    encoder.fixed(&inputs.protected_source_tree_sha256)?;
    encoder.fixed(&(inputs.environment.len() as u64).to_le_bytes())?;
    for (name, value) in inputs.environment {
        if name.is_empty() || os_bytes(name).contains(&b'=') {
            return Err(ProcessIdentityError::new(
                "process consistency environment contains an invalid name",
            ));
        }
        encoder.field(name)?;
        encoder.field(value)?;
    }
    Ok(encoder.finish())
}

/// Observes the current Linux process after exec. The result is inert consistency data.
pub fn child_observed_process_consistency_digest_v3(
    protected_source_tree_sha256: [u8; 32],
) -> Result<[u8; 32], ProcessIdentityError> {
    let mut executable = File::open("/proc/self/exe").map_err(|error| {
        ProcessIdentityError::new(format!("cannot open /proc/self/exe: {error}"))
    })?;
    let (executable_sha256, _, executable_object) = measure_open_regular_file(
        &mut executable,
        MAX_EXECUTABLE_BYTES_V3,
        "running executable",
    )?;
    if executable_object.mode & 0o111 == 0 {
        return Err(ProcessIdentityError::new(
            "running executable has no execute permission bits",
        ));
    }
    let argv = std::env::args_os().collect::<Vec<_>>();
    let current_dir_object = current_directory_object_identity_v3()?;
    let mut environment = std::env::vars_os().collect::<Vec<_>>();
    environment.sort_unstable_by(|left, right| os_bytes(&left.0).cmp(os_bytes(&right.0)));
    parent_prepared_process_consistency_digest_v3(&ParentPreparedProcessConsistencyV3 {
        executable_object,
        executable_sha256,
        argv: &argv,
        current_dir_object,
        protected_source_tree_sha256,
        environment: &environment,
    })
}

pub fn current_directory_object_identity_v3() -> Result<LinuxObjectIdentityV3, ProcessIdentityError>
{
    let directory = File::from(
        rustix::fs::open(
            ".",
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|error| {
            ProcessIdentityError::new(format!("cannot open current directory: {error}"))
        })?,
    );
    let metadata = directory.metadata().map_err(|error| {
        ProcessIdentityError::new(format!("cannot inspect current directory: {error}"))
    })?;
    if !metadata.is_dir() {
        return Err(ProcessIdentityError::new(
            "current directory observation is not a directory",
        ));
    }
    Ok(LinuxObjectIdentityV3::from_metadata(&metadata))
}

pub fn measure_executable_sha256_v3(path: &Path) -> Result<[u8; 32], ProcessIdentityError> {
    let mut file = File::open(path).map_err(|error| {
        ProcessIdentityError::new(format!(
            "cannot open executable {}: {error}",
            path.display()
        ))
    })?;
    let (sha256, _, object) =
        measure_open_regular_file(&mut file, MAX_EXECUTABLE_BYTES_V3, "executable")?;
    if object.mode & 0o111 == 0 {
        return Err(ProcessIdentityError::new(format!(
            "executable {} has no execute permission bits",
            path.display()
        )));
    }
    Ok(sha256)
}

/// Consumes one immutable parent expectation and compares it with a child observation.
pub fn compare_child_observation_with_parent_preparation_v3(
    descriptor: RawFd,
    protected_source_tree_sha256: [u8; 32],
) -> Result<[u8; 32], ProcessIdentityError> {
    let expected = consume_sealed_digest_v3(descriptor)?;
    let observed = child_observed_process_consistency_digest_v3(protected_source_tree_sha256)?;
    if observed != expected {
        return Err(ProcessIdentityError::new(
            "child-observed process does not match the sealed parent preparation",
        ));
    }
    Ok(observed)
}

fn canonical_source_components(path: &Path) -> Result<Vec<&OsStr>, ProcessIdentityError> {
    let bytes = os_bytes(path.as_os_str());
    if path.is_absolute()
        || bytes.is_empty()
        || bytes.len() > MAX_SOURCE_PATH_BYTES
        || bytes.contains(&0)
    {
        return Err(ProcessIdentityError::new(
            "protected source path must be bounded, relative, and nonempty",
        ));
    }
    let components = path
        .components()
        .map(|component| match component {
            Component::Normal(value) if !value.is_empty() => Ok(value),
            _ => Err(ProcessIdentityError::new(
                "protected source path must contain only canonical components",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if components.is_empty() || components.len() > MAX_SOURCE_PATH_COMPONENTS {
        return Err(ProcessIdentityError::new(
            "protected source path has an invalid component count",
        ));
    }
    Ok(components)
}

fn measure_open_regular_file(
    file: &mut File,
    maximum: u64,
    label: &str,
) -> Result<([u8; 32], u64, LinuxObjectIdentityV3), ProcessIdentityError> {
    let initial_metadata = file
        .metadata()
        .map_err(|error| ProcessIdentityError::new(format!("cannot inspect {label}: {error}")))?;
    if !initial_metadata.is_file() {
        return Err(ProcessIdentityError::new(format!(
            "{label} is not a regular file"
        )));
    }
    let initial = FileSnapshot::from_metadata(&initial_metadata);
    if initial.size == 0 || initial.size > maximum {
        return Err(ProcessIdentityError::new(format!(
            "{label} has an invalid bounded size"
        )));
    }
    file.seek(SeekFrom::Start(0)).map_err(|error| {
        ProcessIdentityError::new(format!("cannot rewind {label} before hashing: {error}"))
    })?;
    let mut remaining = initial.size;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; HASH_CHUNK_BYTES];
    while remaining != 0 {
        let wanted = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("bounded hash chunk fits usize");
        let read = file
            .read(&mut buffer[..wanted])
            .map_err(|error| ProcessIdentityError::new(format!("cannot hash {label}: {error}")))?;
        if read == 0 {
            return Err(ProcessIdentityError::new(format!(
                "{label} was truncated while hashing"
            )));
        }
        digest.update(&buffer[..read]);
        remaining -= read as u64;
    }
    if file
        .read(&mut buffer[..1])
        .map_err(|error| ProcessIdentityError::new(format!("cannot bound {label}: {error}")))?
        != 0
    {
        return Err(ProcessIdentityError::new(format!(
            "{label} grew while hashing"
        )));
    }
    let final_snapshot = FileSnapshot::from_metadata(&file.metadata().map_err(|error| {
        ProcessIdentityError::new(format!("cannot reinspect {label}: {error}"))
    })?);
    if final_snapshot != initial {
        return Err(ProcessIdentityError::new(format!(
            "{label} changed while hashing"
        )));
    }
    Ok((digest.finalize().into(), initial.size, initial.object))
}

fn consume_sealed_digest_v3(descriptor: RawFd) -> Result<[u8; 32], ProcessIdentityError> {
    if descriptor < 3 {
        return Err(ProcessIdentityError::new(
            "process-consistency descriptor overlaps a standard stream",
        ));
    }
    // SAFETY: fcntl validates the process-local descriptor before ownership is assumed.
    let borrowed = unsafe { BorrowedFd::borrow_raw(descriptor) };
    rustix::io::fcntl_getfd(borrowed).map_err(|error| {
        ProcessIdentityError::new(format!(
            "process-consistency descriptor is unavailable: {error}"
        ))
    })?;
    rustix::io::fcntl_setfd(borrowed, rustix::io::FdFlags::CLOEXEC).map_err(|error| {
        ProcessIdentityError::new(format!(
            "cannot make process-consistency descriptor close-on-exec: {error}"
        ))
    })?;
    // SAFETY: the validated fixed descriptor is consumed exactly once by this function.
    let mut file = unsafe { File::from_raw_fd(descriptor) };
    let metadata = file.metadata().map_err(|error| {
        ProcessIdentityError::new(format!(
            "cannot inspect process-consistency descriptor: {error}"
        ))
    })?;
    if !metadata.is_file() || metadata.len() != 32 {
        return Err(ProcessIdentityError::new(
            "process-consistency descriptor is not an exact 32-byte regular file",
        ));
    }
    let required = rustix::fs::SealFlags::WRITE
        | rustix::fs::SealFlags::GROW
        | rustix::fs::SealFlags::SHRINK
        | rustix::fs::SealFlags::SEAL;
    if rustix::fs::fcntl_get_seals(&file)
        .map_err(|error| ProcessIdentityError::new(format!("cannot inspect seals: {error}")))?
        != required
    {
        return Err(ProcessIdentityError::new(
            "process-consistency descriptor does not have exact immutable seals",
        ));
    }
    file.seek(SeekFrom::Start(0)).map_err(|error| {
        ProcessIdentityError::new(format!(
            "cannot rewind process-consistency descriptor: {error}"
        ))
    })?;
    let mut digest = [0_u8; 32];
    file.read_exact(&mut digest).map_err(|error| {
        ProcessIdentityError::new(format!(
            "process-consistency descriptor is truncated: {error}"
        ))
    })?;
    let mut trailing = [0_u8; 1];
    if file.read(&mut trailing).map_err(|error| {
        ProcessIdentityError::new(format!(
            "cannot bound process-consistency descriptor: {error}"
        ))
    })? != 0
        || digest == [0; 32]
    {
        return Err(ProcessIdentityError::new(
            "process-consistency descriptor is noncanonical",
        ));
    }
    drop(file);
    Ok(digest)
}

struct DigestEncoder {
    digest: Sha256,
    encoded_bytes: usize,
}

impl DigestEncoder {
    fn new(domain: &[u8]) -> Result<Self, ProcessIdentityError> {
        let mut encoder = Self {
            digest: Sha256::new(),
            encoded_bytes: 0,
        };
        encoder.fixed(domain)?;
        Ok(encoder)
    }

    fn reserve(&mut self, bytes: usize) -> Result<(), ProcessIdentityError> {
        self.encoded_bytes = self
            .encoded_bytes
            .checked_add(bytes)
            .filter(|total| *total <= MAX_ENCODED_CONSISTENCY_BYTES_V3)
            .ok_or_else(|| {
                ProcessIdentityError::new("process consistency encoding exceeds aggregate limit")
            })?;
        Ok(())
    }

    fn fixed(&mut self, bytes: &[u8]) -> Result<(), ProcessIdentityError> {
        self.reserve(bytes.len())?;
        self.digest.update(bytes);
        Ok(())
    }

    fn field(&mut self, value: &OsStr) -> Result<(), ProcessIdentityError> {
        let bytes = os_bytes(value);
        if bytes.len() > MAX_FIELD_BYTES_V3 {
            return Err(ProcessIdentityError::new(
                "process consistency field exceeds its byte limit",
            ));
        }
        self.reserve(8 + bytes.len())?;
        self.digest.update((bytes.len() as u64).to_le_bytes());
        self.digest.update(bytes);
        Ok(())
    }

    fn object(&mut self, object: LinuxObjectIdentityV3) -> Result<(), ProcessIdentityError> {
        self.fixed(&object.device.to_le_bytes())?;
        self.fixed(&object.inode.to_le_bytes())?;
        self.fixed(&object.mode.to_le_bytes())
    }

    fn finish(self) -> [u8; 32] {
        self.digest.finalize().into()
    }
}

fn os_bytes(value: &OsStr) -> &[u8] {
    value.as_bytes()
}

#[cfg(test)]
mod tests {
    use super::{
        LinuxObjectIdentityV3, MAX_FIELD_BYTES_V3, ParentPreparedProcessConsistencyV3,
        parent_prepared_process_consistency_digest_v3,
    };
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    fn inputs<'a>(
        argv: &'a [OsString],
        environment: &'a [(OsString, OsString)],
    ) -> ParentPreparedProcessConsistencyV3<'a> {
        ParentPreparedProcessConsistencyV3 {
            executable_object: LinuxObjectIdentityV3::from_linux_stat(1, 2, 0o100755),
            executable_sha256: [1; 32],
            argv,
            current_dir_object: LinuxObjectIdentityV3::from_linux_stat(3, 4, 0o40700),
            protected_source_tree_sha256: [2; 32],
            environment,
        }
    }

    #[test]
    fn every_parent_prepared_field_changes_the_consistency_digest() {
        let argv = vec![OsString::from("raw-argv0"), OsString::from("--crate-name")];
        let environment = vec![(OsString::from("A"), OsString::from("one"))];
        let baseline =
            parent_prepared_process_consistency_digest_v3(&inputs(&argv, &environment)).unwrap();

        let mut changed = inputs(&argv, &environment);
        changed.executable_object = LinuxObjectIdentityV3::from_linux_stat(1, 9, 0o100755);
        assert_ne!(
            baseline,
            parent_prepared_process_consistency_digest_v3(&changed).unwrap()
        );
        let mut changed = inputs(&argv, &environment);
        changed.executable_sha256 = [3; 32];
        assert_ne!(
            baseline,
            parent_prepared_process_consistency_digest_v3(&changed).unwrap()
        );
        let changed_argv = vec![
            OsString::from("other-argv0"),
            OsString::from("--crate-name"),
        ];
        assert_ne!(
            baseline,
            parent_prepared_process_consistency_digest_v3(&inputs(&changed_argv, &environment))
                .unwrap()
        );
        let mut changed = inputs(&argv, &environment);
        changed.current_dir_object = LinuxObjectIdentityV3::from_linux_stat(3, 8, 0o40700);
        assert_ne!(
            baseline,
            parent_prepared_process_consistency_digest_v3(&changed).unwrap()
        );
        let mut changed = inputs(&argv, &environment);
        changed.protected_source_tree_sha256 = [4; 32];
        assert_ne!(
            baseline,
            parent_prepared_process_consistency_digest_v3(&changed).unwrap()
        );
        let changed_environment = vec![(OsString::from("A"), OsString::from("two"))];
        assert_ne!(
            baseline,
            parent_prepared_process_consistency_digest_v3(&inputs(&argv, &changed_environment))
                .unwrap()
        );
    }

    #[test]
    fn aggregate_encoding_limit_is_enforced() {
        let argv = (0..9)
            .map(|_| OsString::from_vec(vec![b'x'; MAX_FIELD_BYTES_V3]))
            .collect::<Vec<_>>();
        let error = parent_prepared_process_consistency_digest_v3(&inputs(&argv, &[])).unwrap_err();
        assert!(error.to_string().contains("aggregate limit"));
    }

    #[test]
    fn environment_must_be_complete_and_strictly_sorted() {
        let argv = [OsString::from("argv0")];
        let unsorted = [
            (OsString::from("B"), OsString::from("two")),
            (OsString::from("A"), OsString::from("one")),
        ];
        assert!(parent_prepared_process_consistency_digest_v3(&inputs(&argv, &unsorted)).is_err());
    }
}
