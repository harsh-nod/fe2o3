//! Authority-free remote observation of one admitted live rustc process.

use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::PathBuf;

use fe2o3_compiler_closure_capability::{
    RUSTC_INVOCATION_CHILD_FD_V1, RustcInvocationCapabilityV1,
};
use fe2o3_process_identity::{
    MAX_ARGUMENTS_V3, MAX_ENCODED_CONSISTENCY_BYTES_V3, MAX_ENVIRONMENT_ENTRIES_V3,
    MAX_EXECUTABLE_BYTES_V3, MAX_FIELD_BYTES_V3, ProtectedRustcProcessValidationErrorV1,
    validate_protected_rustc_process_observation_v1,
};
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, InvocationDigestV3, RustcInvocationDescriptorV3, ValidationError,
};
use sha2::{Digest, Sha256};

use crate::ProtectedServiceAdmissionV1;
use crate::linux::{ProtectedServiceAdmissionErrorV1, require_procfs};

const SUPERVISION_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/REMOTE-RUSTC-PROCESS-OBSERVATION/V1\0";
const PROC_CMDLINE: &str = "cmdline";
const PROC_CWD: &str = "cwd";
const PROC_ENVIRON: &str = "environ";
const PROC_EXE: &str = "exe";
const ARTIFACT_DIRECTORY_FD: i32 =
    fe2o3_artifact_transaction::BROKERED_ARTIFACT_DIRECTORY_CHILD_FD_V1;
const CODEGEN_BACKEND_FD: i32 = fe2o3_artifact_transaction::BROKERED_CODEGEN_BACKEND_CHILD_FD_V1;

/// One independently observed, authority-free snapshot of an admitted live rustc process.
///
/// The value retains the exact invocation capability, rustc executable, codegen backend,
/// artifact directory, procfs process directory, admitted peer, and pidfd. It is move-only and
/// cannot create an issuer occurrence, sign a receipt, publish an artifact, or authorize loading
/// or execution.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ValidatedRemoteRustcProcessObservationV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<ValidatedRemoteRustcProcessObservationV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_broker_authority_service::ValidatedRemoteRustcProcessObservationV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<ValidatedRemoteRustcProcessObservationV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ValidatedRemoteRustcProcessObservationV1;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<ValidatedRemoteRustcProcessObservationV1>();
/// ```
pub struct ValidatedRemoteRustcProcessObservationV1 {
    client_session: ProtectedServiceAdmissionV1,
    pid: u32,
    start_time_ticks: u64,
    proc_dir: RetainedDirectoryV1,
    invocation: RustcInvocationCapabilityV1,
    rustc_executable: RetainedMeasuredFileV1,
    codegen_backend: RetainedMeasuredFileV1,
    artifact_directory: RetainedDirectoryV1,
    argv: Vec<String>,
    canonical_working_directory: String,
    compile_environment: CompileEnvironmentV2,
    identity: [u8; 32],
}

impl fmt::Debug for ValidatedRemoteRustcProcessObservationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedRemoteRustcProcessObservationV1")
            .field("authority", &"none")
            .field("pid", &self.pid)
            .field("start_time_ticks", &self.start_time_ticks)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl ValidatedRemoteRustcProcessObservationV1 {
    /// Observes the exact process retained by one protected-service admission.
    ///
    /// Linux applies ptrace access checks to `pidfd_getfd` and protected procfs entries. A
    /// production service running under a distinct UID must be launched with the narrowly scoped
    /// permission required to inspect its admitted client; an access failure rejects observation.
    pub fn observe(
        admission: &ProtectedServiceAdmissionV1,
    ) -> Result<Self, CompilerExecutionSupervisionErrorV1> {
        admission.validate_session_continuity()?;
        let client_session = admission.retain_session()?;
        let (pid, start_time_ticks) = client_session.client_process_identity();
        let proc_dir = open_process_directory(pid)?;
        let (argv, canonical_working_directory, compile_environment) =
            observe_process_inputs(&proc_dir)?;
        let invocation = RustcInvocationCapabilityV1::from_file(File::from(
            duplicate_remote_descriptor(&client_session, RUSTC_INVOCATION_CHILD_FD_V1)?,
        ))
        .map_err(CompilerExecutionSupervisionErrorV1::InvocationCapability)?;
        let rustc_executable = RetainedMeasuredFileV1::admit(
            open_proc_component(&proc_dir, PROC_EXE, false)?,
            "rustc executable",
            MAX_EXECUTABLE_BYTES_V3,
            true,
        )?;
        let codegen_backend = RetainedMeasuredFileV1::admit(
            File::from(duplicate_remote_descriptor(
                &client_session,
                CODEGEN_BACKEND_FD,
            )?),
            "codegen backend",
            MAX_EXECUTABLE_BYTES_V3,
            false,
        )?;
        let artifact_directory = RetainedDirectoryV1::admit(
            File::from(duplicate_remote_descriptor(
                &client_session,
                ARTIFACT_DIRECTORY_FD,
            )?),
            "artifact directory",
            false,
        )?;

        validate_descriptor_observation(
            invocation.descriptor(),
            &argv,
            &canonical_working_directory,
            &compile_environment,
            rustc_executable.sha256,
            codegen_backend.sha256,
        )?;
        let identity = derive_observation_identity(
            pid,
            start_time_ticks,
            invocation.descriptor(),
            &rustc_executable,
            &codegen_backend,
            &artifact_directory,
        )?;
        let observed = Self {
            client_session,
            pid,
            start_time_ticks,
            proc_dir,
            invocation,
            rustc_executable,
            codegen_backend,
            artifact_directory,
            argv,
            canonical_working_directory,
            compile_environment,
            identity,
        };
        admission.validate_session_continuity()?;
        observed.revalidate()?;
        Ok(observed)
    }

    /// Repeats every process, descriptor, byte, and semantic comparison against the same
    /// admitted pid/start identity.
    pub fn revalidate(&self) -> Result<(), CompilerExecutionSupervisionErrorV1> {
        self.client_session.validate_session_continuity()?;
        if !self
            .client_session
            .matches_client_process(self.pid, self.start_time_ticks)
        {
            return Err(CompilerExecutionSupervisionErrorV1::ProcessIdentityChanged);
        }
        self.proc_dir.revalidate("client procfs directory", true)?;
        self.invocation
            .revalidate()
            .map_err(CompilerExecutionSupervisionErrorV1::InvocationCapability)?;
        self.rustc_executable.revalidate("rustc executable")?;
        self.codegen_backend.revalidate("codegen backend")?;
        self.artifact_directory
            .revalidate("artifact directory", false)?;

        let (argv, canonical_working_directory, compile_environment) =
            observe_process_inputs(&self.proc_dir)?;
        if argv != self.argv
            || canonical_working_directory != self.canonical_working_directory
            || compile_environment != self.compile_environment
        {
            return Err(CompilerExecutionSupervisionErrorV1::ProcessInputsChanged);
        }

        let current_invocation = RustcInvocationCapabilityV1::from_file(File::from(
            duplicate_remote_descriptor(&self.client_session, RUSTC_INVOCATION_CHILD_FD_V1)?,
        ))
        .map_err(CompilerExecutionSupervisionErrorV1::InvocationCapability)?;
        if current_invocation.descriptor() != self.invocation.descriptor() {
            return Err(CompilerExecutionSupervisionErrorV1::InvocationChanged);
        }
        let current_rustc = RetainedMeasuredFileV1::admit(
            open_proc_component(&self.proc_dir, PROC_EXE, false)?,
            "rustc executable",
            MAX_EXECUTABLE_BYTES_V3,
            true,
        )?;
        if !current_rustc.same_observation(&self.rustc_executable) {
            return Err(CompilerExecutionSupervisionErrorV1::RustcExecutableChanged);
        }
        let current_backend = RetainedMeasuredFileV1::admit(
            File::from(duplicate_remote_descriptor(
                &self.client_session,
                CODEGEN_BACKEND_FD,
            )?),
            "codegen backend",
            MAX_EXECUTABLE_BYTES_V3,
            false,
        )?;
        if !current_backend.same_observation(&self.codegen_backend) {
            return Err(CompilerExecutionSupervisionErrorV1::CodegenBackendChanged);
        }
        let current_artifact_directory = RetainedDirectoryV1::admit(
            File::from(duplicate_remote_descriptor(
                &self.client_session,
                ARTIFACT_DIRECTORY_FD,
            )?),
            "artifact directory",
            false,
        )?;
        if current_artifact_directory.snapshot != self.artifact_directory.snapshot {
            return Err(CompilerExecutionSupervisionErrorV1::ArtifactDirectoryChanged);
        }
        validate_descriptor_observation(
            self.invocation.descriptor(),
            &argv,
            &canonical_working_directory,
            &compile_environment,
            current_rustc.sha256,
            current_backend.sha256,
        )?;
        if derive_observation_identity(
            self.pid,
            self.start_time_ticks,
            self.invocation.descriptor(),
            &current_rustc,
            &current_backend,
            &current_artifact_directory,
        )? != self.identity
        {
            return Err(CompilerExecutionSupervisionErrorV1::IdentityChanged);
        }
        self.client_session.validate_session_continuity()?;
        Ok(())
    }

    /// Returns the exact canonical invocation descriptor observed in the client process.
    pub const fn descriptor(&self) -> &RustcInvocationDescriptorV3 {
        self.invocation.descriptor()
    }

    /// Returns the domain-separated identity of every retained observation axis.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    /// This observation is inert and does not authenticate a compiler occurrence by itself.
    pub const fn authenticates_protected_compiler_execution(&self) -> bool {
        false
    }

    /// Process observation alone grants no compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
}

fn validate_descriptor_observation(
    descriptor: &RustcInvocationDescriptorV3,
    argv: &[String],
    canonical_working_directory: &str,
    compile_environment: &CompileEnvironmentV2,
    rustc_sha256: [u8; 32],
    backend_sha256: [u8; 32],
) -> Result<(), CompilerExecutionSupervisionErrorV1> {
    validate_protected_rustc_process_observation_v1(
        descriptor,
        argv,
        canonical_working_directory,
        compile_environment,
        rustc_sha256,
        backend_sha256,
        fe2o3_artifact_transaction::BROKERED_CODEGEN_BACKEND_PATH_V1,
        fe2o3_artifact_transaction::BROKERED_ARTIFACT_DIRECTORY_PATH_V1,
    )
    .map_err(CompilerExecutionSupervisionErrorV1::ProcessValidation)
}

fn duplicate_remote_descriptor(
    client_session: &ProtectedServiceAdmissionV1,
    descriptor: i32,
) -> Result<OwnedFd, CompilerExecutionSupervisionErrorV1> {
    rustix::process::pidfd_getfd(
        client_session.client_pidfd(),
        descriptor,
        rustix::process::PidfdGetfdFlags::empty(),
    )
    .map_err(
        |error| CompilerExecutionSupervisionErrorV1::RemoteDescriptor {
            descriptor,
            source: io::Error::from(error),
        },
    )
}

fn open_process_directory(
    pid: u32,
) -> Result<RetainedDirectoryV1, CompilerExecutionSupervisionErrorV1> {
    let path = PathBuf::from(format!("/proc/{pid}"));
    let file = File::open(&path).map_err(|source| CompilerExecutionSupervisionErrorV1::Io {
        operation: "open client procfs directory",
        source,
    })?;
    RetainedDirectoryV1::admit(file, "client procfs directory", true)
}

fn observe_process_inputs(
    proc_dir: &RetainedDirectoryV1,
) -> Result<(Vec<String>, String, CompileEnvironmentV2), CompilerExecutionSupervisionErrorV1> {
    proc_dir.revalidate("client procfs directory", true)?;
    let argv = parse_nul_strings(
        &read_proc_component(proc_dir, PROC_CMDLINE, MAX_ENCODED_CONSISTENCY_BYTES_V3)?,
        "rustc argv",
        MAX_ARGUMENTS_V3,
    )?;
    let compile_environment = parse_environment(&read_proc_component(
        proc_dir,
        PROC_ENVIRON,
        MAX_ENCODED_CONSISTENCY_BYTES_V3,
    )?)?;
    let cwd = fs::read_link(proc_component_path(proc_dir, PROC_CWD)).map_err(|source| {
        CompilerExecutionSupervisionErrorV1::Io {
            operation: "read client working-directory link",
            source,
        }
    })?;
    let cwd = cwd.into_os_string().into_string().map_err(|_| {
        CompilerExecutionSupervisionErrorV1::InvalidObservation(
            "client working directory is not UTF-8",
        )
    })?;
    if !cwd.starts_with('/') || cwd.as_bytes().contains(&0) || cwd.ends_with(" (deleted)") {
        return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
            "client working directory is not a live canonical absolute path",
        ));
    }
    proc_dir.revalidate("client procfs directory", true)?;
    Ok((argv, cwd, compile_environment))
}

fn read_proc_component(
    proc_dir: &RetainedDirectoryV1,
    component: &'static str,
    maximum: usize,
) -> Result<Vec<u8>, CompilerExecutionSupervisionErrorV1> {
    let mut file = open_proc_component(proc_dir, component, true)?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take((maximum as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| CompilerExecutionSupervisionErrorV1::Io {
            operation: "read bounded client procfs record",
            source,
        })?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
            "client procfs record is empty or exceeds its bound",
        ));
    }
    Ok(bytes)
}

fn open_proc_component(
    proc_dir: &RetainedDirectoryV1,
    component: &'static str,
    no_follow: bool,
) -> Result<File, CompilerExecutionSupervisionErrorV1> {
    let mut flags = rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC;
    if no_follow {
        flags |= rustix::fs::OFlags::NOFOLLOW;
    }
    let file = rustix::fs::openat(&proc_dir.file, component, flags, rustix::fs::Mode::empty())
        .map(File::from)
        .map_err(|error| CompilerExecutionSupervisionErrorV1::Io {
            operation: "open client procfs component",
            source: io::Error::from(error),
        })?;
    if no_follow {
        require_procfs(&file, component)?;
    }
    Ok(file)
}

fn proc_component_path(proc_dir: &RetainedDirectoryV1, component: &str) -> PathBuf {
    PathBuf::from(format!(
        "/proc/self/fd/{}/{component}",
        proc_dir.file.as_raw_fd()
    ))
}

fn parse_nul_strings(
    bytes: &[u8],
    label: &'static str,
    maximum_count: usize,
) -> Result<Vec<String>, CompilerExecutionSupervisionErrorV1> {
    if bytes.last() != Some(&0) {
        return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
            "NUL-delimited procfs record has no terminal NUL",
        ));
    }
    let mut values = Vec::new();
    for field in bytes[..bytes.len() - 1].split(|byte| *byte == 0) {
        if field.is_empty() || field.len() > MAX_FIELD_BYTES_V3 || values.len() == maximum_count {
            return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
                "NUL-delimited procfs field is empty or exceeds its bound",
            ));
        }
        let value = std::str::from_utf8(field).map_err(|_| {
            CompilerExecutionSupervisionErrorV1::InvalidObservation(
                "NUL-delimited procfs field is not UTF-8",
            )
        })?;
        values.push(value.to_owned());
    }
    if values.is_empty() {
        return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
            label,
        ));
    }
    Ok(values)
}

fn parse_environment(
    bytes: &[u8],
) -> Result<CompileEnvironmentV2, CompilerExecutionSupervisionErrorV1> {
    let entries = parse_nul_strings(bytes, "rustc environment", MAX_ENVIRONMENT_ENTRIES_V3)?;
    let mut environment = Vec::with_capacity(entries.len());
    for entry in entries {
        let (key, value) = entry.split_once('=').ok_or(
            CompilerExecutionSupervisionErrorV1::InvalidObservation(
                "client environment entry has no key/value separator",
            ),
        )?;
        if key.is_empty() {
            return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
                "client environment entry has an empty key",
            ));
        }
        environment.push((
            OsString::from_vec(key.as_bytes().to_vec()),
            OsString::from_vec(value.as_bytes().to_vec()),
        ));
    }
    CompileEnvironmentV2::from_child_environment(environment)
        .map_err(CompilerExecutionSupervisionErrorV1::CompileEnvironment)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileSnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    owner: u32,
    group: u32,
    links: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl FileSnapshotV1 {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            owner: metadata.uid(),
            group: metadata.gid(),
            links: metadata.nlink(),
            length: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }

    fn update_identity(&self, digest: &mut Sha256) {
        digest.update(self.device.to_le_bytes());
        digest.update(self.inode.to_le_bytes());
        digest.update(self.mode.to_le_bytes());
        digest.update(self.owner.to_le_bytes());
        digest.update(self.group.to_le_bytes());
        digest.update(self.links.to_le_bytes());
        digest.update(self.length.to_le_bytes());
        digest.update(self.modified_seconds.to_le_bytes());
        digest.update(self.modified_nanoseconds.to_le_bytes());
        digest.update(self.changed_seconds.to_le_bytes());
        digest.update(self.changed_nanoseconds.to_le_bytes());
    }
}

struct RetainedDirectoryV1 {
    file: File,
    snapshot: FileSnapshotV1,
}

impl RetainedDirectoryV1 {
    fn admit(
        file: File,
        label: &'static str,
        procfs: bool,
    ) -> Result<Self, CompilerExecutionSupervisionErrorV1> {
        require_close_on_exec(&file, label)?;
        if procfs {
            require_procfs(&file, label)?;
        }
        let metadata =
            file.metadata()
                .map_err(|source| CompilerExecutionSupervisionErrorV1::Io {
                    operation: "inspect retained directory",
                    source,
                })?;
        if !metadata.is_dir() || metadata.nlink() == 0 {
            return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
                "retained directory is not a live directory",
            ));
        }
        Ok(Self {
            file,
            snapshot: FileSnapshotV1::from_metadata(&metadata),
        })
    }

    fn revalidate(
        &self,
        label: &'static str,
        procfs: bool,
    ) -> Result<(), CompilerExecutionSupervisionErrorV1> {
        require_close_on_exec(&self.file, label)?;
        if procfs {
            require_procfs(&self.file, label)?;
        }
        let metadata =
            self.file
                .metadata()
                .map_err(|source| CompilerExecutionSupervisionErrorV1::Io {
                    operation: "reinspect retained directory",
                    source,
                })?;
        if !metadata.is_dir()
            || metadata.nlink() == 0
            || FileSnapshotV1::from_metadata(&metadata) != self.snapshot
        {
            return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
                "retained directory identity or metadata changed",
            ));
        }
        Ok(())
    }
}

struct RetainedMeasuredFileV1 {
    file: File,
    snapshot: FileSnapshotV1,
    sha256: [u8; 32],
    maximum: u64,
    require_executable: bool,
}

impl RetainedMeasuredFileV1 {
    fn admit(
        file: File,
        label: &'static str,
        maximum: u64,
        require_executable: bool,
    ) -> Result<Self, CompilerExecutionSupervisionErrorV1> {
        require_close_on_exec(&file, label)?;
        let snapshot = measure_file_snapshot(&file, label, maximum, require_executable)?;
        let sha256 = hash_exact_file(&file, snapshot.length, label)?;
        if measure_file_snapshot(&file, label, maximum, require_executable)? != snapshot {
            return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
                "measured file changed while its bytes were read",
            ));
        }
        Ok(Self {
            file,
            snapshot,
            sha256,
            maximum,
            require_executable,
        })
    }

    fn revalidate(&self, label: &'static str) -> Result<(), CompilerExecutionSupervisionErrorV1> {
        require_close_on_exec(&self.file, label)?;
        if measure_file_snapshot(&self.file, label, self.maximum, self.require_executable)?
            != self.snapshot
            || hash_exact_file(&self.file, self.snapshot.length, label)? != self.sha256
        {
            return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
                "retained measured file changed",
            ));
        }
        Ok(())
    }

    fn same_observation(&self, other: &Self) -> bool {
        self.snapshot == other.snapshot && self.sha256 == other.sha256
    }
}

fn measure_file_snapshot(
    file: &File,
    _label: &'static str,
    maximum: u64,
    require_executable: bool,
) -> Result<FileSnapshotV1, CompilerExecutionSupervisionErrorV1> {
    let metadata = file
        .metadata()
        .map_err(|source| CompilerExecutionSupervisionErrorV1::Io {
            operation: "inspect measured file",
            source,
        })?;
    if !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > maximum
        || (require_executable && metadata.mode() & 0o111 == 0)
    {
        return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
            "measured file has an invalid type, size, or executable mode",
        ));
    }
    Ok(FileSnapshotV1::from_metadata(&metadata))
}

fn hash_exact_file(
    file: &File,
    length: u64,
    _label: &'static str,
) -> Result<[u8; 32], CompilerExecutionSupervisionErrorV1> {
    const CHUNK: usize = 64 * 1024;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; CHUNK];
    let mut offset = 0_u64;
    while offset < length {
        let remaining = usize::try_from((length - offset).min(CHUNK as u64))
            .expect("bounded chunk length fits usize");
        let count = file
            .read_at(&mut buffer[..remaining], offset)
            .map_err(|source| CompilerExecutionSupervisionErrorV1::Io {
                operation: "read measured file",
                source,
            })?;
        if count == 0 {
            return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
                "measured file ended before its admitted length",
            ));
        }
        digest.update(&buffer[..count]);
        offset += count as u64;
    }
    let mut trailing = [0_u8; 1];
    if file.read_at(&mut trailing, length).map_err(|source| {
        CompilerExecutionSupervisionErrorV1::Io {
            operation: "check measured file boundary",
            source,
        }
    })? != 0
    {
        return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
            "measured file grew beyond its admitted length",
        ));
    }
    Ok(digest.finalize().into())
}

fn require_close_on_exec(
    file: &File,
    _label: &'static str,
) -> Result<(), CompilerExecutionSupervisionErrorV1> {
    let flags =
        rustix::io::fcntl_getfd(file).map_err(|error| CompilerExecutionSupervisionErrorV1::Io {
            operation: "inspect retained descriptor flags",
            source: io::Error::from(error),
        })?;
    if !flags.contains(rustix::io::FdFlags::CLOEXEC) {
        return Err(CompilerExecutionSupervisionErrorV1::InvalidObservation(
            "retained descriptor lacks FD_CLOEXEC",
        ));
    }
    Ok(())
}

fn derive_observation_identity(
    pid: u32,
    start_time_ticks: u64,
    descriptor: &RustcInvocationDescriptorV3,
    rustc: &RetainedMeasuredFileV1,
    backend: &RetainedMeasuredFileV1,
    artifact_directory: &RetainedDirectoryV1,
) -> Result<[u8; 32], CompilerExecutionSupervisionErrorV1> {
    let invocation = InvocationDigestV3::calculate(descriptor)
        .map_err(CompilerExecutionSupervisionErrorV1::InvocationDigest)?;
    let mut digest = Sha256::new();
    digest.update(SUPERVISION_IDENTITY_DOMAIN_V1);
    digest.update(pid.to_le_bytes());
    digest.update(start_time_ticks.to_le_bytes());
    digest.update(invocation.as_bytes());
    digest.update(rustc.sha256);
    rustc.snapshot.update_identity(&mut digest);
    digest.update(backend.sha256);
    backend.snapshot.update_identity(&mut digest);
    artifact_directory.snapshot.update_identity(&mut digest);
    let identity: [u8; 32] = digest.finalize().into();
    if identity == [0; 32] {
        return Err(CompilerExecutionSupervisionErrorV1::IdentityChanged);
    }
    Ok(identity)
}

/// Failure while independently observing an admitted remote rustc process.
#[derive(Debug)]
pub enum CompilerExecutionSupervisionErrorV1 {
    ServiceAdmission(ProtectedServiceAdmissionErrorV1),
    RemoteDescriptor {
        descriptor: i32,
        source: io::Error,
    },
    Io {
        operation: &'static str,
        source: io::Error,
    },
    InvocationCapability(String),
    CompileEnvironment(ValidationError),
    ProcessValidation(ProtectedRustcProcessValidationErrorV1),
    InvocationDigest(fe2o3_rustc_invocation::DigestError),
    InvalidObservation(&'static str),
    ProcessIdentityChanged,
    ProcessInputsChanged,
    InvocationChanged,
    RustcExecutableChanged,
    CodegenBackendChanged,
    ArtifactDirectoryChanged,
    IdentityChanged,
}

impl fmt::Display for CompilerExecutionSupervisionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ServiceAdmission(error) => write!(formatter, "service admission failed: {error}"),
            Self::RemoteDescriptor { descriptor, source } => write!(
                formatter,
                "cannot duplicate client descriptor {descriptor} through its pidfd: {source}"
            ),
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
            Self::InvocationCapability(error) => {
                write!(
                    formatter,
                    "invalid remote rustc invocation capability: {error}"
                )
            }
            Self::CompileEnvironment(error) => {
                write!(formatter, "invalid remote rustc environment: {error}")
            }
            Self::ProcessValidation(error) => {
                write!(formatter, "remote rustc process does not match V3: {error}")
            }
            Self::InvocationDigest(error) => {
                write!(
                    formatter,
                    "cannot derive remote rustc invocation identity: {error}"
                )
            }
            Self::InvalidObservation(reason) => {
                write!(formatter, "invalid remote rustc observation: {reason}")
            }
            Self::ProcessIdentityChanged => {
                formatter.write_str("remote rustc pid/start identity changed")
            }
            Self::ProcessInputsChanged => {
                formatter.write_str("remote rustc argv, cwd, or environment changed")
            }
            Self::InvocationChanged => {
                formatter.write_str("remote rustc invocation capability changed")
            }
            Self::RustcExecutableChanged => formatter.write_str("remote rustc executable changed"),
            Self::CodegenBackendChanged => {
                formatter.write_str("remote rustc codegen backend changed")
            }
            Self::ArtifactDirectoryChanged => {
                formatter.write_str("remote rustc artifact directory changed")
            }
            Self::IdentityChanged => {
                formatter.write_str("remote rustc supervision identity changed")
            }
        }
    }
}

impl Error for CompilerExecutionSupervisionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ServiceAdmission(error) => Some(error),
            Self::RemoteDescriptor { source, .. } | Self::Io { source, .. } => Some(source),
            Self::CompileEnvironment(error) => Some(error),
            Self::ProcessValidation(error) => Some(error),
            Self::InvocationDigest(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ProtectedServiceAdmissionErrorV1> for CompilerExecutionSupervisionErrorV1 {
    fn from(error: ProtectedServiceAdmissionErrorV1) -> Self {
        Self::ServiceAdmission(error)
    }
}

#[cfg(test)]
mod tests {
    use std::mem;
    use std::os::fd::{AsRawFd, FromRawFd, RawFd};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, ExitStatus};
    use std::ptr;

    use fe2o3_build_authority::CompilerClosureV2;
    use fe2o3_rustc_invocation::{RustcInvocationDescriptorV2, RustcUnitV2};
    use rustix::net::{AddressFamily, SocketFlags, SocketType, socketpair};
    use tempfile::TempDir;

    use super::*;

    const CONTROL_FD: RawFd = 0;
    const HELD_PEER_FD: RawFd = 9;

    fn hex(digest: [u8; 32]) -> String {
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn seqpacket() -> (OwnedFd, OwnedFd) {
        socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap()
    }

    fn send_descriptor(socket: RawFd, descriptor: RawFd) -> io::Result<()> {
        let mut byte = 0x72_u8;
        let mut io_vector = libc::iovec {
            iov_base: ptr::from_mut(&mut byte).cast(),
            iov_len: 1,
        };
        let mut control = [0_usize; 8];
        // SAFETY: zero is the documented empty initialization for `msghdr`.
        let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
        header.msg_iov = &mut io_vector;
        header.msg_iovlen = 1;
        header.msg_control = control.as_mut_ptr().cast();
        header.msg_controllen = unsafe { libc::CMSG_SPACE(mem::size_of::<RawFd>() as _) } as usize;
        // SAFETY: the aligned control buffer has room for exactly one SCM_RIGHTS descriptor.
        unsafe {
            let message = libc::CMSG_FIRSTHDR(&header);
            (*message).cmsg_level = libc::SOL_SOCKET;
            (*message).cmsg_type = libc::SCM_RIGHTS;
            (*message).cmsg_len = libc::CMSG_LEN(mem::size_of::<RawFd>() as _) as usize;
            ptr::write_unaligned(libc::CMSG_DATA(message).cast::<RawFd>(), descriptor);
        }
        let sent = unsafe { libc::sendmsg(socket, &header, libc::MSG_NOSIGNAL) };
        if sent == 1 {
            Ok(())
        } else if sent < 0 {
            Err(io::Error::last_os_error())
        } else {
            Err(io::Error::from_raw_os_error(libc::EIO))
        }
    }

    fn receive_descriptor(socket: RawFd) -> io::Result<OwnedFd> {
        let mut byte = 0_u8;
        let mut io_vector = libc::iovec {
            iov_base: ptr::from_mut(&mut byte).cast(),
            iov_len: 1,
        };
        let mut control = [0_usize; 8];
        // SAFETY: zero is the documented empty initialization for `msghdr`.
        let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
        header.msg_iov = &mut io_vector;
        header.msg_iovlen = 1;
        header.msg_control = control.as_mut_ptr().cast();
        header.msg_controllen = unsafe { libc::CMSG_SPACE(mem::size_of::<RawFd>() as _) } as usize;
        // SAFETY: all pointers in the header name live, writable, correctly sized storage.
        let received = unsafe { libc::recvmsg(socket, &mut header, libc::MSG_CMSG_CLOEXEC) };
        if received < 0 {
            return Err(io::Error::last_os_error());
        }
        if received != 1
            || byte != 0x72
            || header.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0
        {
            return Err(io::Error::from_raw_os_error(libc::EBADMSG));
        }
        // SAFETY: recvmsg bounded the control data and exactly one descriptor is required.
        let descriptor = unsafe {
            let message = libc::CMSG_FIRSTHDR(&header);
            if message.is_null()
                || (*message).cmsg_level != libc::SOL_SOCKET
                || (*message).cmsg_type != libc::SCM_RIGHTS
                || (*message).cmsg_len != libc::CMSG_LEN(mem::size_of::<RawFd>() as _) as usize
                || !libc::CMSG_NXTHDR(&header, message).is_null()
            {
                return Err(io::Error::from_raw_os_error(libc::EBADMSG));
            }
            ptr::read_unaligned(libc::CMSG_DATA(message).cast::<RawFd>())
        };
        if descriptor < 0 {
            return Err(io::Error::from_raw_os_error(libc::EBADMSG));
        }
        // SAFETY: SCM_RIGHTS installed a fresh descriptor owned by this process.
        Ok(unsafe { OwnedFd::from_raw_fd(descriptor) })
    }

    fn pidfd_for(pid: u32) -> OwnedFd {
        let pid = libc::pid_t::try_from(pid).unwrap();
        // SAFETY: pidfd_open receives one positive PID and zero flags and returns a fresh fd.
        let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
        assert!(
            descriptor >= 0,
            "pidfd_open failed: {}",
            io::Error::last_os_error()
        );
        // SAFETY: successful pidfd_open returned a fresh owned descriptor.
        unsafe { OwnedFd::from_raw_fd(descriptor as RawFd) }
    }

    fn protected_root() -> (TempDir, OwnedFd) {
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let root = File::open(directory.path()).unwrap();
        (directory, root.into())
    }

    fn digest_file(file: &File) -> [u8; 32] {
        hash_exact_file(file, file.metadata().unwrap().len(), "test fixture").unwrap()
    }

    #[derive(Clone, Copy, Eq, PartialEq)]
    enum RemoteFixtureMutation {
        Exact,
        BackendBytesMismatch,
        MissingInvocationCapability,
    }

    struct RemoteRustcFixture {
        descriptor: RustcInvocationDescriptorV3,
        control: Option<OwnedFd>,
        child: Option<Child>,
        admission: Option<ProtectedServiceAdmissionV1>,
        _service_root: TempDir,
        _artifact_directory: TempDir,
    }

    impl RemoteRustcFixture {
        fn admission(&self) -> &ProtectedServiceAdmissionV1 {
            self.admission.as_ref().expect("live fixture admission")
        }

        fn shutdown(&mut self) -> ExitStatus {
            let control = self.control.take().expect("live fixture control endpoint");
            assert_eq!(rustix::io::write(&control, b"9\n").unwrap(), 2);
            // `dash` reads one byte at a time, while `SOCK_SEQPACKET` discards the unread suffix of
            // a record. Closing the endpoint supplies the EOF needed to complete the shell `read`.
            drop(control);
            self.child
                .take()
                .expect("live fixture child")
                .wait()
                .unwrap()
        }
    }

    impl Drop for RemoteRustcFixture {
        fn drop(&mut self) {
            if let Some(control) = self.control.take() {
                let _ = rustix::io::write(&control, b"9\n");
                drop(control);
            }
            if let Some(mut child) = self.child.take() {
                if child.try_wait().ok().flatten().is_none() {
                    let _ = child.kill();
                }
                let _ = child.wait();
            }
        }
    }

    fn spawn_remote_rustc(mutation: RemoteFixtureMutation) -> RemoteRustcFixture {
        let rustc_executable = fs::canonicalize("/bin/sh").unwrap();
        let rustc_executable_text = rustc_executable.to_str().unwrap().to_owned();
        let rustc_file = File::open(&rustc_executable).unwrap();
        let rustc_sha256 = digest_file(&rustc_file);
        let backend = tempfile::tempfile().unwrap();
        backend.set_len(4096).unwrap();
        backend.write_at(&[0x5a; 4096], 0).unwrap();
        let running_backend_sha256 = digest_file(&backend);
        let descriptor_backend_sha256 = if mutation == RemoteFixtureMutation::BackendBytesMismatch {
            Sha256::digest([0x5b; 4096]).into()
        } else {
            running_backend_sha256
        };
        let artifact_directory = tempfile::tempdir().unwrap();
        let artifact_file = File::open(artifact_directory.path()).unwrap();
        let working_directory = fs::canonicalize(".").unwrap();
        let working_directory = working_directory.to_str().unwrap().to_owned();
        let closure = CompilerClosureV2::new(
            [0x11; 32],
            [0x22; 32],
            [0x33; 32],
            rustc_sha256,
            [0x44; 32],
            descriptor_backend_sha256,
        )
        .unwrap();
        let environment = CompileEnvironmentV2::from_child_environment([
            (
                OsString::from("FE2O3_HSACO_DIR"),
                OsString::from(fe2o3_artifact_transaction::BROKERED_ARTIFACT_DIRECTORY_PATH_V1),
            ),
            (
                OsString::from("FE2O3_TARGET"),
                OsString::from("gfx942:xnack-"),
            ),
            (
                OsString::from(fe2o3_process_identity::EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1),
                OsString::from(hex(closure.identity_sha256())),
            ),
            (
                OsString::from(fe2o3_process_identity::CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2),
                OsString::from(hex(descriptor_backend_sha256)),
            ),
        ])
        .unwrap();
        let argv = vec![
            rustc_executable_text.clone(),
            "-c".into(),
            "IFS= read -r _ || :".into(),
            "fe2o3-remote-rustc-observation".into(),
            format!(
                "-Zcodegen-backend={}",
                fe2o3_artifact_transaction::BROKERED_CODEGEN_BACKEND_PATH_V1
            ),
        ];
        let descriptor = RustcInvocationDescriptorV3::new(
            RustcInvocationDescriptorV2::new(
                rustc_sha256,
                descriptor_backend_sha256,
                RustcUnitV2::new(&working_directory, argv.clone()).unwrap(),
                environment.clone(),
            )
            .unwrap(),
            closure,
        )
        .unwrap();
        let invocation = RustcInvocationCapabilityV1::create(descriptor.clone()).unwrap();
        let invocation_file = invocation.try_clone_for_transfer().unwrap();
        let (control, child_control) = seqpacket();

        let mut command = Command::new(&rustc_executable);
        command.args(&argv[1..]);
        command.current_dir(&working_directory);
        environment.configure_command(&mut command);
        let child_control_fd = child_control.as_raw_fd();
        let invocation_fd = invocation_file.as_raw_fd();
        let backend_fd = backend.as_raw_fd();
        let artifact_fd = artifact_file.as_raw_fd();
        // SAFETY: every source descriptor remains live through spawn. The child creates its
        // service socket before exec so SO_PEERCRED names the exact child PID, transfers one end,
        // and retains the other at HELD_PEER_FD while the shell waits on CONTROL_FD.
        unsafe {
            command.pre_exec(move || {
                for (source, target) in [
                    (child_control_fd, CONTROL_FD),
                    (artifact_fd, ARTIFACT_DIRECTORY_FD),
                    (backend_fd, CODEGEN_BACKEND_FD),
                ] {
                    if libc::dup2(source, target) != target
                        || libc::fcntl(target, libc::F_SETFD, 0) != 0
                    {
                        return Err(io::Error::last_os_error());
                    }
                }
                if mutation != RemoteFixtureMutation::MissingInvocationCapability
                    && (libc::dup2(invocation_fd, RUSTC_INVOCATION_CHILD_FD_V1)
                        != RUSTC_INVOCATION_CHILD_FD_V1
                        || libc::fcntl(RUSTC_INVOCATION_CHILD_FD_V1, libc::F_SETFD, 0) != 0)
                {
                    return Err(io::Error::last_os_error());
                }
                let mut peers = [-1_i32; 2];
                if libc::socketpair(
                    libc::AF_UNIX,
                    libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                    0,
                    peers.as_mut_ptr(),
                ) != 0
                {
                    return Err(io::Error::last_os_error());
                }
                if let Err(error) = send_descriptor(CONTROL_FD, peers[0]) {
                    libc::close(peers[0]);
                    libc::close(peers[1]);
                    return Err(error);
                }
                if libc::dup2(peers[1], HELD_PEER_FD) != HELD_PEER_FD
                    || libc::fcntl(HELD_PEER_FD, libc::F_SETFD, 0) != 0
                {
                    let error = io::Error::last_os_error();
                    libc::close(peers[0]);
                    libc::close(peers[1]);
                    return Err(error);
                }
                libc::close(peers[0]);
                libc::close(peers[1]);
                Ok(())
            });
        }
        let child = crate::test_process_execution::spawn(&mut command).unwrap();
        drop(child_control);
        let retained_peer = receive_descriptor(control.as_raw_fd()).unwrap();
        let expected = crate::ExpectedClientProcessIdentityV1::new(
            child.id(),
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .unwrap();
        let live_client =
            crate::LiveClientPidfdIdentityV1::admit(pidfd_for(child.id()), expected).unwrap();
        let (service_root, root) = protected_root();
        let admission = ProtectedServiceAdmissionV1::admit_non_authoritative_same_uid_session_test(
            root,
            retained_peer,
            live_client,
        )
        .unwrap();

        RemoteRustcFixture {
            descriptor,
            control: Some(control),
            child: Some(child),
            admission: Some(admission),
            _service_root: service_root,
            _artifact_directory: artifact_directory,
        }
    }

    #[test]
    fn observes_revalidates_and_rejects_exit_of_one_exact_remote_process() {
        let mut fixture = spawn_remote_rustc(RemoteFixtureMutation::Exact);
        let observation = ValidatedRemoteRustcProcessObservationV1::observe(fixture.admission())
            .and_then(|observation| {
                observation.revalidate()?;
                Ok(observation)
            });

        drop(fixture.admission.take());
        observation.as_ref().unwrap().revalidate().unwrap();
        assert!(fixture.shutdown().success());
        let observation = observation.unwrap();
        assert_eq!(observation.descriptor(), &fixture.descriptor);
        assert_ne!(observation.identity(), &[0; 32]);
        assert!(!observation.authenticates_protected_compiler_execution());
        assert!(!observation.grants_compiler_authority());
        assert!(observation.revalidate().is_err());
    }

    #[test]
    fn remote_backend_byte_substitution_fails_closed() {
        let mut fixture = spawn_remote_rustc(RemoteFixtureMutation::BackendBytesMismatch);
        assert!(matches!(
            ValidatedRemoteRustcProcessObservationV1::observe(fixture.admission()),
            Err(CompilerExecutionSupervisionErrorV1::ProcessValidation(
                ProtectedRustcProcessValidationErrorV1::RunningCodegenBackendMismatch
            ))
        ));
        assert!(fixture.shutdown().success());
    }

    #[test]
    fn missing_remote_invocation_capability_fails_closed() {
        let mut fixture = spawn_remote_rustc(RemoteFixtureMutation::MissingInvocationCapability);
        assert!(matches!(
            ValidatedRemoteRustcProcessObservationV1::observe(fixture.admission()),
            Err(CompilerExecutionSupervisionErrorV1::RemoteDescriptor {
                descriptor: RUSTC_INVOCATION_CHILD_FD_V1,
                ..
            })
        ));
        assert!(fixture.shutdown().success());
    }

    #[test]
    fn nul_records_are_bounded_utf8_and_terminal() {
        assert_eq!(
            parse_nul_strings(b"rustc\0--crate-name\0fixture\0", "argv", 3).unwrap(),
            ["rustc", "--crate-name", "fixture"]
        );
        for malformed in [&b"rustc"[..], &b"rustc\0\0"[..], &b"\xff\0"[..], &b"\0"[..]] {
            assert!(parse_nul_strings(malformed, "argv", 3).is_err());
        }
        assert!(parse_nul_strings(b"a\0b\0", "argv", 1).is_err());
    }

    #[test]
    fn environment_parser_preserves_values_and_rejects_malformed_entries() {
        let environment =
            parse_environment(b"FE2O3_TARGET=gfx942:xnack-\0FE2O3_HSACO_DIR=/output\0EMPTY=\0")
                .unwrap();
        assert_eq!(environment.entries().len(), 3);
        assert!(parse_environment(b"NO_SEPARATOR\0").is_err());
        assert!(parse_environment(b"=empty-key\0").is_err());
        assert!(parse_environment(b"DUP=1\0DUP=2\0").is_err());
    }

    #[test]
    fn file_snapshot_identity_binds_every_metadata_axis() {
        let original = FileSnapshotV1 {
            device: 1,
            inode: 2,
            mode: 3,
            owner: 4,
            group: 5,
            links: 6,
            length: 7,
            modified_seconds: 8,
            modified_nanoseconds: 9,
            changed_seconds: 10,
            changed_nanoseconds: 11,
        };
        let identity = |snapshot: FileSnapshotV1| {
            let mut digest = Sha256::new();
            snapshot.update_identity(&mut digest);
            <[u8; 32]>::from(digest.finalize())
        };
        let baseline = identity(original);
        let mut variants = Vec::new();
        macro_rules! changed {
            ($field:ident) => {{
                let mut value = original;
                value.$field = value.$field.wrapping_add(1);
                variants.push(value);
            }};
        }
        changed!(device);
        changed!(inode);
        changed!(mode);
        changed!(owner);
        changed!(group);
        changed!(links);
        changed!(length);
        changed!(modified_seconds);
        changed!(modified_nanoseconds);
        changed!(changed_seconds);
        changed!(changed_nanoseconds);
        assert!(
            variants
                .into_iter()
                .all(|value| identity(value) != baseline)
        );
    }
}
