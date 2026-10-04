//! Retained process-and-input observation, without publication recovery or an ACK writer lease.

use std::error::Error;
use std::fmt;
use std::fs::{File, Metadata};
use std::io::{self, Read};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::{FileExt, MetadataExt};

use fe2o3_runtime_protocol::{
    MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2, WorkerV3ApplicationHandoffExpectationV1,
    WorkerV3ApplicationHandoffProtocolErrorV1, WorkerV3ApplicationIdentityV1,
    WorkerV3ApplicationInputOccurrenceV1, WorkerV3ApplicationOccurrenceV1,
    WorkerV3ApplicationRegistrationBindingV1, WorkerV3LoadEnvelopeIdentityV1,
};
use rustix::fs::{FileType, Mode, OFlags, SealFlags};
use rustix::net::{AddressFamily, SocketAddrUnix, SocketType};

use super::{
    ExpectedClientProcessIdentityV1, LiveClientPidfdIdentityV1, ProtectedServiceAdmissionErrorV1,
    parse_process_start_time_ticks, require_procfs,
};

const MAX_APPLICATION_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_PROC_RECORD_BYTES: u64 = 16 * 1024;
const REQUIRED_SEALS: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);

/// Lookup coordinates in the original application's descriptor table, not evidence of ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationDescriptorNumbersV1 {
    envelope: i32,
    directory: i32,
    acknowledgment: i32,
}

impl WorkerV3ApplicationDescriptorNumbersV1 {
    /// Requires three distinct descriptors above standard input, output, and error.
    pub fn new(envelope: i32, directory: i32, acknowledgment: i32) -> Result<Self> {
        if envelope <= 2
            || directory <= 2
            || acknowledgment <= 2
            || envelope == directory
            || envelope == acknowledgment
            || directory == acknowledgment
        {
            return Err(invalid(
                "application descriptor coordinates alias or name stdio",
            ));
        }
        Ok(Self {
            envelope,
            directory,
            acknowledgment,
        })
    }
}

/// Failure to independently inspect or revalidate the retained application occurrence.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3ApplicationObservationErrorV1 {
    Process(ProtectedServiceAdmissionErrorV1),
    Protocol(WorkerV3ApplicationHandoffProtocolErrorV1),
    Io {
        operation: &'static str,
        source: io::Error,
    },
    InvalidObservation(&'static str),
}

impl fmt::Display for WorkerV3ApplicationObservationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process(error) => write!(formatter, "application process observation: {error}"),
            Self::Protocol(error) => write!(formatter, "application occurrence: {error}"),
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
            Self::InvalidObservation(message) => formatter.write_str(message),
        }
    }
}

impl Error for WorkerV3ApplicationObservationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Process(error) => Some(error),
            Self::Protocol(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::InvalidObservation(_) => None,
        }
    }
}

impl From<ProtectedServiceAdmissionErrorV1> for WorkerV3ApplicationObservationErrorV1 {
    fn from(error: ProtectedServiceAdmissionErrorV1) -> Self {
        Self::Process(error)
    }
}

impl From<WorkerV3ApplicationHandoffProtocolErrorV1> for WorkerV3ApplicationObservationErrorV1 {
    fn from(error: WorkerV3ApplicationHandoffProtocolErrorV1) -> Self {
        Self::Protocol(error)
    }
}

type Result<T> = std::result::Result<T, WorkerV3ApplicationObservationErrorV1>;

fn invalid(message: &'static str) -> WorkerV3ApplicationObservationErrorV1 {
    WorkerV3ApplicationObservationErrorV1::InvalidObservation(message)
}

fn io_error(
    operation: &'static str,
    source: impl Into<io::Error>,
) -> WorkerV3ApplicationObservationErrorV1 {
    WorkerV3ApplicationObservationErrorV1::Io {
        operation,
        source: source.into(),
    }
}

/// Move-only observation of one live application and its original handoff input objects.
///
/// The supervisor must separately authenticate the source of the original, unreaped child
/// pidfd, parent pidfd, expected occurrence, and envelope identity. This value independently
/// observes their OS association; it does not authenticate a registration channel or spawn nonce.
/// The ACK writer is inspected only before return and is never retained, so Cargo can see EOF.
/// The optional fourth input retains only socket facts, never an application-side socket alias.
/// This layer does not require a distinct service UID or establish protected-service isolation.
/// That policy belongs to the authenticated deployment/registration join.
/// Nor can PID/start time and equal image bytes distinguish a same-image re-exec before this
/// observation. Authenticating Cargo's original spawn/sandbox supervision remains necessary;
/// later executable-object substitutions are rejected by retained-object revalidation.
///
/// Envelope bytes are bounded and measured, not decoded. Directory membership, canonical
/// envelope validity, compiler origin, publication currentness, proof custody, and launch
/// authority are separate contracts. No publication lock or recovery is performed here. Later
/// joins must compare the exact observed bytes with both the pure compiler-closure check and the
/// original host admission under its own token. Process exit invalidates this observation; it
/// does not prove GPU settlement or authorize release of proof/native custody.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::RetainedWorkerV3ApplicationObservationV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<RetainedWorkerV3ApplicationObservationV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::RetainedWorkerV3ApplicationObservationV1;
/// fn require_as_fd<T: std::os::fd::AsFd>() {}
/// require_as_fd::<RetainedWorkerV3ApplicationObservationV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::RetainedWorkerV3ApplicationObservationV1;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<RetainedWorkerV3ApplicationObservationV1>();
/// ```
pub struct RetainedWorkerV3ApplicationObservationV1 {
    application: LiveClientPidfdIdentityV1,
    parent: LiveClientPidfdIdentityV1,
    application_proc: RetainedFile,
    parent_proc: RetainedFile,
    executable: RetainedFile,
    envelope: RetainedFile,
    directory: RetainedFile,
    slots: WorkerV3ApplicationDescriptorNumbersV1,
    occurrence: WorkerV3ApplicationOccurrenceV1,
    expectation: WorkerV3ApplicationHandoffExpectationV1,
    envelope_bytes: Box<[u8]>,
    proof: Option<(i32, ProofEndpointFacts)>,
}

impl fmt::Debug for RetainedWorkerV3ApplicationObservationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedWorkerV3ApplicationObservationV1")
            .field("authority", &"none")
            .field("application", &self.application)
            .field("occurrence", &self.occurrence)
            .finish_non_exhaustive()
    }
}

impl RetainedWorkerV3ApplicationObservationV1 {
    /// Observes claimed, CLOEXEC handoff descriptors before the application emits its ACK.
    ///
    /// Both original process identity tokens are consumed. Linux ptrace/procfs access is required and
    /// access failure rejects; there is no numeric-PID reopening or pathname fallback for remote
    /// descriptors. Checks are point-in-time, not an atomic process freeze. The registration
    /// protocol must keep the application at its pre-ACK barrier until this method returns.
    /// The caller must retain a separate containment owner: failure drops the supplied tokens,
    /// but never signals or reaps either process and never establishes safe GPU settlement.
    pub fn observe_pre_ack(
        application: LiveClientPidfdIdentityV1,
        parent: LiveClientPidfdIdentityV1,
        slots: WorkerV3ApplicationDescriptorNumbersV1,
        expected_occurrence: &WorkerV3ApplicationOccurrenceV1,
        expected_envelope: WorkerV3LoadEnvelopeIdentityV1,
    ) -> Result<Self> {
        Self::observe_inputs(
            application,
            parent,
            slots,
            expected_occurrence,
            expected_envelope,
            None,
        )
    }

    /// Observes the four-input registration profile and its exact Cargo-created proof counterpart.
    ///
    /// The caller must authenticate the binding and original process owners separately, retain
    /// the root counterpart, and hold the application at its pre-ACK barrier. Creator credentials
    /// are not packet sender authentication or proof custody. No application-side alias survives.
    pub fn observe_registered_pre_ack(
        application: LiveClientPidfdIdentityV1,
        parent: LiveClientPidfdIdentityV1,
        binding: &WorkerV3ApplicationRegistrationBindingV1,
        root_proof_peer: BorrowedFd<'_>,
    ) -> Result<Self> {
        let matches = |actual: ExpectedClientProcessIdentityV1, expected: fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1| {
            (actual.pid, actual.uid, actual.gid) == (expected.pid(), expected.uid(), expected.gid())
        };
        if !matches(
            application.expected_client,
            binding.compiler_handoff().launch_manifest().client(),
        ) || !matches(
            parent.expected_client,
            binding.compiler_handoff().submitter(),
        ) {
            return Err(invalid(
                "registration binding differs from original process owners",
            ));
        }
        let [envelope, directory, acknowledgment, proof] = binding.descriptors().as_array();
        Self::observe_inputs(
            application,
            parent,
            WorkerV3ApplicationDescriptorNumbersV1::new(envelope, directory, acknowledgment)?,
            binding.occurrence(),
            binding.expectation().envelope(),
            Some((proof, root_proof_peer)),
        )
    }

    fn observe_inputs(
        application: LiveClientPidfdIdentityV1,
        parent: LiveClientPidfdIdentityV1,
        slots: WorkerV3ApplicationDescriptorNumbersV1,
        expected_occurrence: &WorkerV3ApplicationOccurrenceV1,
        expected_envelope: WorkerV3LoadEnvelopeIdentityV1,
        proof_peer: Option<(i32, BorrowedFd<'_>)>,
    ) -> Result<Self> {
        application.validate_parent(&parent)?;
        if application.expected_client.uid != parent.expected_client.uid
            || application.expected_client.gid != parent.expected_client.gid
        {
            return Err(invalid("application and Cargo parent credentials differ"));
        }
        let application_proc = open_process(&application)?;
        let parent_proc = open_process(&parent)?;
        validate_process(&application, &application_proc)?;
        validate_process(&parent, &parent_proc)?;
        let executable = inspect_executable(&application_proc, application.expected_client)?;
        if executable.snapshot.length != expected_occurrence.application().byte_len() {
            return Err(invalid(
                "application executable length differs from supervisor binding",
            ));
        }
        let application_identity = WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(
            &read_exact(&executable, MAX_APPLICATION_BYTES)?,
        )?;
        executable.revalidate()?;
        require_seals(&executable.file)?;
        let envelope = inspect_remote(
            &application,
            &application_proc,
            slots.envelope,
            Role::Envelope,
        )?;
        let directory = inspect_remote(
            &application,
            &application_proc,
            slots.directory,
            Role::Directory,
        )?;
        if envelope.snapshot.length != expected_envelope.byte_len() {
            return Err(invalid(
                "application envelope length differs from supervisor binding",
            ));
        }
        let acknowledgment = inspect_remote(
            &application,
            &application_proc,
            slots.acknowledgment,
            Role::Acknowledgment,
        )?;
        let acknowledgment_snapshot = acknowledgment.snapshot;
        let mut inputs = vec![
            envelope.snapshot.input(1)?,
            directory.snapshot.input(2)?,
            acknowledgment.snapshot.input(3)?,
        ];
        // An extra writer would keep Cargo's ACK reader open even after the application closes it.
        drop(acknowledgment);
        let proof = proof_peer
            .map(|(number, peer)| {
                let facts = inspect_remote_proof(
                    &application,
                    &application_proc,
                    number,
                    parent.expected_client,
                )?;
                facts
                    .require_counterpart(&inspect_proof_endpoint(peer, parent.expected_client)?)?;
                Ok::<_, WorkerV3ApplicationObservationErrorV1>((number, facts))
            })
            .transpose()?;
        if let Some((_, facts)) = &proof {
            inputs.push(facts.snapshot.input(4)?);
        }
        let occurrence = WorkerV3ApplicationOccurrenceV1::new(
            application_identity,
            expected_occurrence.spawn_identity(),
            &inputs,
        )?;
        if &occurrence != expected_occurrence {
            return Err(invalid(
                "observed application occurrence differs from supervisor binding",
            ));
        }
        let envelope_bytes = read_exact(&envelope, MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2 as u64)?;
        let envelope_identity = WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(&envelope_bytes)?;
        if envelope_identity != expected_envelope {
            return Err(invalid(
                "observed envelope bytes differ from supervisor binding",
            ));
        }
        let expectation =
            WorkerV3ApplicationHandoffExpectationV1::new(envelope_identity, &occurrence);
        let observed = Self {
            application,
            parent,
            application_proc,
            parent_proc,
            executable,
            envelope,
            directory,
            slots,
            occurrence,
            expectation,
            envelope_bytes: envelope_bytes.into_boxed_slice(),
            proof,
        };
        observed.revalidate_retained_inputs()?;
        let acknowledgment = inspect_remote(
            &observed.application,
            &observed.application_proc,
            slots.acknowledgment,
            Role::Acknowledgment,
        )?;
        if acknowledgment.snapshot != acknowledgment_snapshot {
            return Err(invalid(
                "application ACK descriptor changed during observation",
            ));
        }
        drop(acknowledgment);
        if let (Some((_, facts)), Some((_, peer))) = (&observed.proof, proof_peer) {
            facts.require_counterpart(&inspect_proof_endpoint(
                peer,
                observed.parent.expected_client,
            )?)?;
        }
        observed.validate_processes()?;
        Ok(observed)
    }

    /// Revalidates process, executable, envelope, directory and optional proof source continuity.
    ///
    /// The historical ACK slot is deliberately not inspected: it normally closes at startup and
    /// may be reused. Original pidfds remain the liveness authority; no process is waited or reaped.
    pub fn revalidate_retained_inputs(&self) -> Result<()> {
        self.validate_processes()?;
        self.executable.revalidate()?;
        require_seals(&self.executable.file)?;
        let current_executable =
            inspect_executable(&self.application_proc, self.application.expected_client)?;
        if current_executable.snapshot != self.executable.snapshot {
            return Err(invalid("application executable object changed"));
        }
        self.envelope.revalidate()?;
        self.directory.revalidate()?;
        for (slot, role, expected) in [
            (self.slots.envelope, Role::Envelope, &self.envelope),
            (self.slots.directory, Role::Directory, &self.directory),
        ] {
            let current = inspect_remote(&self.application, &self.application_proc, slot, role)?;
            if current.snapshot != expected.snapshot {
                return Err(invalid("application handoff source slot was replaced"));
            }
        }
        if let Some((number, expected)) = &self.proof
            && inspect_remote_proof(
                &self.application,
                &self.application_proc,
                *number,
                self.parent.expected_client,
            )? != *expected
        {
            return Err(invalid("application proof source slot changed"));
        }
        if read_exact(&self.envelope, MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2 as u64)?
            != *self.envelope_bytes
        {
            return Err(invalid("retained application envelope bytes changed"));
        }
        self.validate_processes()
    }

    fn validate_processes(&self) -> Result<()> {
        self.application.validate_parent(&self.parent)?;
        validate_process(&self.application, &self.application_proc)?;
        validate_process(&self.parent, &self.parent_proc)?;
        self.application.validate_parent(&self.parent)?;
        Ok(())
    }

    /// Returns the independently reconstructed, descriptive occurrence, not launch authority.
    pub fn occurrence(&self) -> &WorkerV3ApplicationOccurrenceV1 {
        &self.occurrence
    }

    /// Returns the independently derived handoff expectation, not proof of an emitted ACK.
    pub const fn expectation(&self) -> WorkerV3ApplicationHandoffExpectationV1 {
        self.expectation
    }

    /// Returns copied exact envelope bytes; canonical or compiler-closure validity is not implied.
    pub fn exact_envelope_bytes(&self) -> &[u8] {
        &self.envelope_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Snapshot {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    length: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl Snapshot {
    fn new(metadata: Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            links: metadata.nlink(),
            length: metadata.len(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }

    fn input(self, slot: u16) -> Result<WorkerV3ApplicationInputOccurrenceV1> {
        Ok(
            WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(
                slot,
                self.device,
                self.inode,
                self.mode,
            )?,
        )
    }
}

struct RetainedFile {
    file: File,
    snapshot: Snapshot,
}

impl RetainedFile {
    fn new(file: File) -> Result<Self> {
        let snapshot = inspect_file(&file)?;
        Ok(Self { file, snapshot })
    }

    fn revalidate(&self) -> Result<()> {
        if inspect_file(&self.file)? != self.snapshot {
            return Err(invalid("retained application object metadata changed"));
        }
        Ok(())
    }
}

fn inspect_file(file: &File) -> Result<Snapshot> {
    if !rustix::io::fcntl_getfd(file)
        .map_err(|e| io_error("inspect retained descriptor flags", e))?
        .contains(rustix::io::FdFlags::CLOEXEC)
    {
        return Err(invalid("retained application descriptor is not CLOEXEC"));
    }
    Ok(Snapshot::new(
        file.metadata()
            .map_err(|e| io_error("inspect application object", e))?,
    ))
}

fn open_process(process: &LiveClientPidfdIdentityV1) -> Result<RetainedFile> {
    let file = File::from(
        rustix::fs::open(
            format!("/proc/{}", process.expected_client.pid),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| io_error("open application process directory", e))?,
    );
    require_procfs(&file, "application process directory")?;
    RetainedFile::new(file)
}

fn read_proc(directory: &RetainedFile, name: &str) -> Result<Vec<u8>> {
    directory.revalidate()?;
    let file = File::from(
        rustix::fs::openat(
            &directory.file,
            name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(|e| io_error("open application process record", e))?,
    );
    require_procfs(&file, "application process record")?;
    let mut bytes = Vec::new();
    file.take(MAX_PROC_RECORD_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| io_error("read bounded application process record", e))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_PROC_RECORD_BYTES {
        return Err(invalid(
            "application process record exceeds its nonempty bound",
        ));
    }
    directory.revalidate()?;
    Ok(bytes)
}

fn validate_process(process: &LiveClientPidfdIdentityV1, directory: &RetainedFile) -> Result<()> {
    process.validate_liveness()?;
    let stat = read_proc(directory, "stat")?;
    if parse_process_start_time_ticks(&stat, process.expected_client.pid)?
        != process.start_time_ticks
    {
        return Err(invalid(
            "retained application procfs entry changed process identity",
        ));
    }
    validate_credentials(&read_proc(directory, "status")?, process.expected_client)?;
    process.validate_liveness()?;
    Ok(())
}

fn validate_credentials(bytes: &[u8], expected: ExpectedClientProcessIdentityV1) -> Result<()> {
    let contents =
        std::str::from_utf8(bytes).map_err(|_| invalid("process status is not UTF-8"))?;
    for (name, id) in [("Uid:", expected.uid), ("Gid:", expected.gid)] {
        let mut records = contents.lines().filter_map(|line| line.strip_prefix(name));
        let fields: Vec<_> = records
            .next()
            .ok_or_else(|| invalid("process credentials missing"))?
            .split_ascii_whitespace()
            .collect();
        if records.next().is_some()
            || fields.len() != 4
            || fields
                .iter()
                .any(|field| field.parse::<u32>().ok() != Some(id))
        {
            return Err(invalid(
                "observed real/effective/saved/filesystem credentials differ",
            ));
        }
    }
    Ok(())
}

fn require_seals(file: &File) -> Result<()> {
    let seals = rustix::fs::fcntl_get_seals(file)
        .map_err(|e| io_error("inspect sealed application image", e))?;
    if seals != REQUIRED_SEALS && seals != REQUIRED_SEALS | SealFlags::FUTURE_WRITE {
        return Err(invalid(
            "application image lacks the exact immutable Cargo seal profile",
        ));
    }
    Ok(())
}

fn inspect_executable(
    directory: &RetainedFile,
    process: ExpectedClientProcessIdentityV1,
) -> Result<RetainedFile> {
    let file = File::from(
        rustix::fs::openat(
            &directory.file,
            "exe",
            OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| io_error("open original application executable", e))?,
    );
    let retained = RetainedFile::new(file)?;
    let s = retained.snapshot;
    if FileType::from_raw_mode(s.mode) != FileType::RegularFile
        || s.uid != process.uid
        || s.gid != process.gid
        || s.mode & 0o111 == 0
        || s.mode & 0o7022 != 0
        || s.length == 0
        || s.length > MAX_APPLICATION_BYTES
    {
        return Err(invalid(
            "application image type, credentials, mode, or length is invalid",
        ));
    }
    require_seals(&retained.file)?;
    Ok(retained)
}

#[derive(Clone, Copy)]
enum Role {
    Envelope,
    Directory,
    Acknowledgment,
}

fn inspect_remote(
    application: &LiveClientPidfdIdentityV1,
    process_dir: &RetainedFile,
    number: i32,
    role: Role,
) -> Result<RetainedFile> {
    let (retained, flags) = duplicate_remote(application, process_dir, number)?;
    let s = retained.snapshot;
    if matches!(role, Role::Acknowledgment) && !flags.contains(OFlags::NONBLOCK) {
        return Err(invalid("application ACK writer is not nonblocking"));
    }
    let (kind, access) = match role {
        Role::Envelope => (FileType::RegularFile, OFlags::RDONLY),
        Role::Directory => (FileType::Directory, OFlags::RDONLY),
        Role::Acknowledgment => (FileType::Fifo, OFlags::WRONLY),
    };
    if FileType::from_raw_mode(s.mode) != kind
        || flags & OFlags::ACCMODE != access
        || s.uid != application.expected_client.uid
        || s.links == 0
        || s.mode & 0o077 != 0
    {
        return Err(invalid(
            "application input type, access, credentials, links, or private mode is invalid",
        ));
    }
    if matches!(role, Role::Envelope)
        && (s.links != 1 || s.length == 0 || s.length > MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2 as u64)
    {
        return Err(invalid(
            "application envelope link count or bounded length is invalid",
        ));
    }
    retained.revalidate()?;
    Ok(retained)
}

fn duplicate_remote(
    application: &LiveClientPidfdIdentityV1,
    process_dir: &RetainedFile,
    number: i32,
) -> Result<(RetainedFile, OFlags)> {
    let file = File::from(
        rustix::process::pidfd_getfd(
            application.pidfd.as_fd(),
            number,
            rustix::process::PidfdGetfdFlags::empty(),
        )
        .map_err(|e| io_error("duplicate original application descriptor", e))?,
    );
    let retained = RetainedFile::new(file)?;
    let flags = rustix::fs::fcntl_getfl(&retained.file)
        .map_err(|e| io_error("inspect application descriptor access", e))?;
    let source_flags = parse_source_flags(&read_proc(process_dir, &format!("fdinfo/{number}"))?)?;
    if source_flags & libc::O_CLOEXEC as u32 == 0
        || source_flags & !(libc::O_CLOEXEC as u32) != flags.bits()
        || flags.intersects(OFlags::PATH | OFlags::ASYNC | OFlags::DIRECT | OFlags::APPEND)
    {
        return Err(invalid(
            "application source descriptor flags differ or lack CLOEXEC",
        ));
    }
    Ok((retained, flags))
}

#[derive(Debug, Eq, PartialEq)]
struct ProofEndpointFacts {
    snapshot: Snapshot,
    local: SocketAddrUnix,
    remote: SocketAddrUnix,
}

impl ProofEndpointFacts {
    fn require_counterpart(&self, peer: &Self) -> Result<()> {
        if (self.snapshot.device, self.snapshot.inode)
            == (peer.snapshot.device, peer.snapshot.inode)
            || self.local != peer.remote
            || self.remote != peer.local
        {
            return Err(invalid("application proof endpoint counterpart differs"));
        }
        Ok(())
    }
}

fn inspect_remote_proof(
    application: &LiveClientPidfdIdentityV1,
    process_dir: &RetainedFile,
    number: i32,
    creator: ExpectedClientProcessIdentityV1,
) -> Result<ProofEndpointFacts> {
    let (temporary, _) = duplicate_remote(application, process_dir, number)?;
    let facts = inspect_proof_endpoint(temporary.file.as_fd(), creator)?;
    if facts.snapshot != temporary.snapshot {
        return Err(invalid(
            "application proof endpoint changed during inspection",
        ));
    }
    temporary.revalidate()?;
    // Retaining this alias would hide application EOF from the independent root session.
    drop(temporary);
    Ok(facts)
}

fn inspect_proof_endpoint(
    fd: BorrowedFd<'_>,
    creator: ExpectedClientProcessIdentityV1,
) -> Result<ProofEndpointFacts> {
    let inspect = || -> std::result::Result<_, rustix::io::Errno> {
        Ok((
            rustix::fs::fstat(fd)?,
            rustix::io::fcntl_getfd(fd)?,
            rustix::fs::fcntl_getfl(fd)?,
            rustix::net::sockopt::socket_domain(fd)?,
            rustix::net::sockopt::socket_type(fd)?,
            rustix::net::sockopt::socket_acceptconn(fd)?,
            rustix::net::sockopt::socket_passcred(fd)?,
            rustix::net::sockopt::socket_peercred(fd)?,
            rustix::net::getsockname(fd)?,
            rustix::net::getpeername(fd)?,
        ))
    };
    let (stat, fd_flags, flags, domain, kind, listener, passcred, credentials, local, remote) =
        inspect().map_err(|e| io_error("inspect application proof endpoint", e))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Socket
        || fd_flags != rustix::io::FdFlags::CLOEXEC
        || flags & OFlags::ACCMODE != OFlags::RDWR
        || !flags.contains(OFlags::NONBLOCK)
        || flags.intersects(OFlags::PATH | OFlags::ASYNC | OFlags::DIRECT | OFlags::APPEND)
        || domain != AddressFamily::UNIX
        || kind != SocketType::SEQPACKET
        || listener
        || !passcred
        || credentials.pid.as_raw_nonzero().get() as u32 != creator.pid
        || credentials.uid.as_raw() != creator.uid
        || credentials.gid.as_raw() != creator.gid
    {
        return Err(invalid(
            "application proof endpoint shape, flags, or Cargo creator differs",
        ));
    }
    let local =
        SocketAddrUnix::try_from(local).map_err(|e| io_error("inspect proof local address", e))?;
    let remote =
        SocketAddrUnix::try_from(remote.ok_or_else(|| invalid("proof endpoint is disconnected"))?)
            .map_err(|e| io_error("inspect proof peer address", e))?;
    if local.abstract_name().is_none() || remote.abstract_name().is_none() || local == remote {
        return Err(invalid(
            "proof endpoint addresses are not distinct abstract names",
        ));
    }
    Ok(ProofEndpointFacts {
        snapshot: Snapshot {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            uid: stat.st_uid,
            gid: stat.st_gid,
            links: stat.st_nlink,
            length: stat.st_size as u64,
            modified: (stat.st_mtime, stat.st_mtime_nsec as i64),
            changed: (stat.st_ctime, stat.st_ctime_nsec as i64),
        },
        local,
        remote,
    })
}

fn parse_source_flags(bytes: &[u8]) -> Result<u32> {
    let contents = std::str::from_utf8(bytes).map_err(|_| invalid("source fdinfo is not UTF-8"))?;
    let mut records = contents
        .lines()
        .filter_map(|line| line.strip_prefix("flags:"));
    let value = records
        .next()
        .ok_or_else(|| invalid("source descriptor flags missing"))?
        .trim();
    if records.next().is_some()
        || value.is_empty()
        || !value.bytes().all(|b| matches!(b, b'0'..=b'7'))
    {
        return Err(invalid("source descriptor flags are not one octal field"));
    }
    u32::from_str_radix(value, 8).map_err(|_| invalid("source descriptor flags overflow"))
}

fn read_exact(retained: &RetainedFile, maximum: u64) -> Result<Vec<u8>> {
    retained.revalidate()?;
    let length = retained.snapshot.length;
    if length == 0 || length > maximum {
        return Err(invalid(
            "application input length exceeds its nonempty bound",
        ));
    }
    let length_usize =
        usize::try_from(length).map_err(|_| invalid("application input length overflow"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length_usize)
        .map_err(|_| invalid("application input allocation failed"))?;
    bytes.resize(length_usize, 0);
    retained
        .file
        .read_exact_at(&mut bytes, 0)
        .map_err(|e| io_error("read exact application input", e))?;
    let mut trailing = [0];
    if retained
        .file
        .read_at(&mut trailing, length)
        .map_err(|e| io_error("check application input boundary", e))?
        != 0
    {
        return Err(invalid("application input grew during observation"));
    }
    retained.revalidate()?;
    Ok(bytes)
}

#[cfg(test)]
pub(crate) mod tests;
