//! Canonical identities for a prepared command and the process it creates.
//!
//! The S09 pilot uses this neutral crate on both sides of execve: the parent hashes the
//! fully prepared command, and the compiler process independently hashes what actually ran.

use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{File, Metadata};
use std::io::{Read, Seek, SeekFrom};
use std::os::fd::{BorrowedFd, FromRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use sha2::{Digest, Sha256};

pub const S09_PREPARED_COMMAND_EXPECTATION_FD_V2: RawFd = 194;
pub const MAX_EXECUTABLE_BYTES_V2: u64 = 512 * 1024 * 1024;
pub const MAX_ARGUMENTS_V2: usize = 65_536;
pub const MAX_ENVIRONMENT_ENTRIES_V2: usize = 4_096;
pub const MAX_FIELD_BYTES_V2: usize = 1024 * 1024;

const DOMAIN_V2: &[u8] = b"FE2O3/PREPARED-PROCESS-COMMAND/V2\0";
const HASH_CHUNK_BYTES: usize = 64 * 1024;

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
struct FileSnapshot {
    device: u64,
    inode: u64,
    mode: u32,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl FileSnapshot {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            size: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

/// Inputs shared by the preparing parent and the process after execve.
pub struct PreparedCommandIdentityV2<'a> {
    pub executable_path: &'a Path,
    pub executable_sha256: [u8; 32],
    /// Ordered arguments after argv0. argv0 is normalized to the executable path.
    pub arguments_after_argv0: &'a [OsString],
    pub current_dir: &'a Path,
    /// The complete environment, sorted strictly by raw variable-name bytes.
    pub environment: &'a [(OsString, OsString)],
}

pub fn prepared_command_digest_v2(
    inputs: &PreparedCommandIdentityV2<'_>,
) -> Result<[u8; 32], ProcessIdentityError> {
    validate_absolute_path(inputs.executable_path, "executable path")?;
    validate_absolute_path(inputs.current_dir, "current directory")?;
    if inputs.executable_sha256 == [0; 32] {
        return Err(ProcessIdentityError::new(
            "executable digest must not be zero",
        ));
    }
    if inputs.arguments_after_argv0.len() > MAX_ARGUMENTS_V2 {
        return Err(ProcessIdentityError::new(
            "prepared command has too many arguments",
        ));
    }
    if inputs.environment.len() > MAX_ENVIRONMENT_ENTRIES_V2 {
        return Err(ProcessIdentityError::new(
            "prepared command has too many environment entries",
        ));
    }
    for pair in inputs.environment.windows(2) {
        if os_bytes(&pair[0].0) >= os_bytes(&pair[1].0) {
            return Err(ProcessIdentityError::new(
                "prepared environment names must be strictly sorted",
            ));
        }
    }

    let mut digest = Sha256::new();
    digest.update(DOMAIN_V2);
    hash_field(&mut digest, inputs.executable_path.as_os_str())?;
    digest.update(inputs.executable_sha256);
    hash_field(&mut digest, inputs.current_dir.as_os_str())?;
    digest.update((inputs.arguments_after_argv0.len() as u64 + 1).to_le_bytes());
    // Raw argv0 is ignored on both sides and replaced by the measured path.
    hash_field(&mut digest, inputs.executable_path.as_os_str())?;
    for argument in inputs.arguments_after_argv0 {
        hash_field(&mut digest, argument)?;
    }
    digest.update((inputs.environment.len() as u64).to_le_bytes());
    for (name, value) in inputs.environment {
        if name.is_empty() || os_bytes(name).contains(&b'=') {
            return Err(ProcessIdentityError::new(
                "prepared environment contains an invalid name",
            ));
        }
        hash_field(&mut digest, name)?;
        hash_field(&mut digest, value)?;
    }
    Ok(digest.finalize().into())
}

/// Recomputes the canonical command identity from the process that is currently running.
pub fn actual_process_command_digest_v2() -> Result<[u8; 32], ProcessIdentityError> {
    let executable_path = std::fs::read_link("/proc/self/exe").map_err(|error| {
        ProcessIdentityError::new(format!("cannot resolve /proc/self/exe: {error}"))
    })?;
    validate_absolute_path(&executable_path, "running executable path")?;
    let executable_sha256 = measure_executable_sha256_v2(Path::new("/proc/self/exe"))?;
    let arguments_after_argv0 = std::env::args_os().skip(1).collect::<Vec<_>>();
    let current_dir = std::env::current_dir().map_err(|error| {
        ProcessIdentityError::new(format!("cannot read running process cwd: {error}"))
    })?;
    let mut environment = std::env::vars_os().collect::<Vec<_>>();
    environment.sort_unstable_by(|left, right| os_bytes(&left.0).cmp(os_bytes(&right.0)));
    prepared_command_digest_v2(&PreparedCommandIdentityV2 {
        executable_path: &executable_path,
        executable_sha256,
        arguments_after_argv0: &arguments_after_argv0,
        current_dir: &current_dir,
        environment: &environment,
    })
}

/// Measures a bounded regular executable through one opened object.
pub fn measure_executable_sha256_v2(path: &Path) -> Result<[u8; 32], ProcessIdentityError> {
    let mut file = File::open(path).map_err(|error| {
        ProcessIdentityError::new(format!(
            "cannot open executable {}: {error}",
            path.display()
        ))
    })?;
    let initial_metadata = file.metadata().map_err(|error| {
        ProcessIdentityError::new(format!(
            "cannot inspect executable {}: {error}",
            path.display()
        ))
    })?;
    if !initial_metadata.is_file() || initial_metadata.mode() & 0o111 == 0 {
        return Err(ProcessIdentityError::new(format!(
            "executable {} is not an executable regular file",
            path.display()
        )));
    }
    if initial_metadata.len() == 0 || initial_metadata.len() > MAX_EXECUTABLE_BYTES_V2 {
        return Err(ProcessIdentityError::new(format!(
            "executable {} has an invalid bounded size",
            path.display()
        )));
    }
    let initial = FileSnapshot::from_metadata(&initial_metadata);
    let mut remaining = initial.size;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; HASH_CHUNK_BYTES];
    while remaining != 0 {
        let wanted = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("bounded executable hash chunk fits usize");
        let read = file.read(&mut buffer[..wanted]).map_err(|error| {
            ProcessIdentityError::new(format!(
                "cannot hash executable {}: {error}",
                path.display()
            ))
        })?;
        if read == 0 {
            return Err(ProcessIdentityError::new(format!(
                "executable {} was truncated while hashing",
                path.display()
            )));
        }
        digest.update(&buffer[..read]);
        remaining -= read as u64;
    }
    if file.read(&mut buffer[..1]).map_err(|error| {
        ProcessIdentityError::new(format!(
            "cannot bound executable {}: {error}",
            path.display()
        ))
    })? != 0
    {
        return Err(ProcessIdentityError::new(format!(
            "executable {} grew while hashing",
            path.display()
        )));
    }
    let final_snapshot = FileSnapshot::from_metadata(&file.metadata().map_err(|error| {
        ProcessIdentityError::new(format!(
            "cannot reinspect executable {}: {error}",
            path.display()
        ))
    })?);
    if final_snapshot != initial {
        return Err(ProcessIdentityError::new(format!(
            "executable {} changed while hashing",
            path.display()
        )));
    }
    Ok(digest.finalize().into())
}

/// Consumes one immutable expectation and compares it with the actual process.
pub fn verify_actual_process_against_sealed_expectation_v2(
    descriptor: RawFd,
) -> Result<[u8; 32], ProcessIdentityError> {
    let expected = consume_sealed_digest_v2(descriptor)?;
    let actual = actual_process_command_digest_v2()?;
    if actual != expected {
        return Err(ProcessIdentityError::new(
            "actual process command does not match the sealed prepared-command expectation",
        ));
    }
    Ok(actual)
}

fn consume_sealed_digest_v2(descriptor: RawFd) -> Result<[u8; 32], ProcessIdentityError> {
    if descriptor < 3 {
        return Err(ProcessIdentityError::new(
            "prepared-command descriptor overlaps a standard stream",
        ));
    }
    // SAFETY: fcntl validates the process-local descriptor before ownership is assumed.
    let borrowed = unsafe { BorrowedFd::borrow_raw(descriptor) };
    rustix::io::fcntl_getfd(borrowed).map_err(|error| {
        ProcessIdentityError::new(format!(
            "prepared-command descriptor is unavailable: {error}"
        ))
    })?;
    rustix::io::fcntl_setfd(borrowed, rustix::io::FdFlags::CLOEXEC).map_err(|error| {
        ProcessIdentityError::new(format!(
            "cannot make prepared-command descriptor close-on-exec: {error}"
        ))
    })?;
    // SAFETY: the validated fixed descriptor is consumed exactly once by this function.
    let mut file = unsafe { File::from_raw_fd(descriptor) };
    let metadata = file.metadata().map_err(|error| {
        ProcessIdentityError::new(format!(
            "cannot inspect prepared-command descriptor: {error}"
        ))
    })?;
    if !metadata.is_file() || metadata.len() != 32 {
        return Err(ProcessIdentityError::new(
            "prepared-command descriptor is not an exact 32-byte regular file",
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
            "prepared-command descriptor does not have exact immutable seals",
        ));
    }
    file.seek(SeekFrom::Start(0)).map_err(|error| {
        ProcessIdentityError::new(format!(
            "cannot rewind prepared-command descriptor: {error}"
        ))
    })?;
    let mut digest = [0_u8; 32];
    file.read_exact(&mut digest).map_err(|error| {
        ProcessIdentityError::new(format!("prepared-command descriptor is truncated: {error}"))
    })?;
    let mut trailing = [0_u8; 1];
    if file.read(&mut trailing).map_err(|error| {
        ProcessIdentityError::new(format!("cannot bound prepared-command descriptor: {error}"))
    })? != 0
        || digest == [0; 32]
    {
        return Err(ProcessIdentityError::new(
            "prepared-command descriptor is noncanonical",
        ));
    }
    // This closes the fixed descriptor before actual-process measurement starts.
    drop(file);
    Ok(digest)
}

fn validate_absolute_path(path: &Path, label: &str) -> Result<(), ProcessIdentityError> {
    let bytes = os_bytes(path.as_os_str());
    if !path.is_absolute() || bytes.is_empty() || bytes.len() > MAX_FIELD_BYTES_V2 {
        return Err(ProcessIdentityError::new(format!(
            "{label} must be a bounded absolute path"
        )));
    }
    Ok(())
}

fn hash_field(digest: &mut Sha256, value: &OsStr) -> Result<(), ProcessIdentityError> {
    let bytes = os_bytes(value);
    if bytes.len() > MAX_FIELD_BYTES_V2 {
        return Err(ProcessIdentityError::new(
            "prepared command field exceeds its byte limit",
        ));
    }
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    Ok(())
}

fn os_bytes(value: &OsStr) -> &[u8] {
    value.as_bytes()
}

#[cfg(test)]
mod tests {
    use super::{PreparedCommandIdentityV2, prepared_command_digest_v2};
    use std::ffi::OsString;
    use std::path::Path;

    fn digest(
        executable: &str,
        executable_sha256: [u8; 32],
        arguments: &[OsString],
        cwd: &str,
        environment: &[(OsString, OsString)],
    ) -> [u8; 32] {
        prepared_command_digest_v2(&PreparedCommandIdentityV2 {
            executable_path: Path::new(executable),
            executable_sha256,
            arguments_after_argv0: arguments,
            current_dir: Path::new(cwd),
            environment,
        })
        .unwrap()
    }

    #[test]
    fn every_prepared_process_field_changes_the_digest() {
        let arguments = vec![OsString::from("--crate-name"), OsString::from("alpha")];
        let environment = vec![
            (OsString::from("A"), OsString::from("one")),
            (OsString::from("B"), OsString::from("two")),
        ];
        let baseline = digest("/tool/rustc", [1; 32], &arguments, "/work", &environment);
        assert_ne!(
            baseline,
            digest("/tool/other", [1; 32], &arguments, "/work", &environment)
        );
        assert_ne!(
            baseline,
            digest("/tool/rustc", [2; 32], &arguments, "/work", &environment)
        );
        assert_ne!(
            baseline,
            digest(
                "/tool/rustc",
                [1; 32],
                &[OsString::from("--version")],
                "/work",
                &environment,
            )
        );
        assert_ne!(
            baseline,
            digest("/tool/rustc", [1; 32], &arguments, "/other", &environment)
        );
        let changed_environment = vec![
            (OsString::from("A"), OsString::from("one")),
            (OsString::from("B"), OsString::from("changed")),
        ];
        assert_ne!(
            baseline,
            digest(
                "/tool/rustc",
                [1; 32],
                &arguments,
                "/work",
                &changed_environment,
            )
        );
    }

    #[test]
    fn environment_must_be_complete_and_strictly_sorted() {
        let unsorted = vec![
            (OsString::from("B"), OsString::from("two")),
            (OsString::from("A"), OsString::from("one")),
        ];
        let result = prepared_command_digest_v2(&PreparedCommandIdentityV2 {
            executable_path: Path::new("/tool/rustc"),
            executable_sha256: [1; 32],
            arguments_after_argv0: &[],
            current_dir: Path::new("/work"),
            environment: &unsorted,
        });
        assert!(result.is_err());
    }
}
