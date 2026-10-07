//! Same-ledger observation of native application input objects, not registration authority.
use super::{
    ExpectedClientProcessIdentityV1, MAX_APPLICATION_BYTES, MAX_PROC_RECORD_BYTES,
    ProofEndpointFacts, RetainedFile, Role, WorkerV3ApplicationIdentityV1,
    WorkerV3ApplicationObservationErrorV1 as MechanicalError, inspect_executable,
    inspect_proof_endpoint, invalid, io_error, parse_source_flags, require_procfs, require_seals,
    validate_credentials,
};
use crate::{LiveClientPidfdErrorV2, LiveClientPidfdIdentityV2 as Client};
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_runtime_protocol::{
    InertConditionalWorkerReadinessWireV5, MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5,
    NativeApplicationRegistrationBindingV1 as Binding,
    WorkerV3ApplicationOccurrenceV1 as Occurrence,
};
use rustix::fs::{FileType, Mode, OFlags};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    fs::File,
    marker::PhantomData,
    mem::size_of,
    os::fd::{AsFd, BorrowedFd},
};

const ENTRY: usize = 8;
const FIXED: usize = 512 * 1024;
const CHUNK: usize = 64 * 1024;

#[derive(Debug)]
pub enum NativeApplicationObservationErrorV3 {
    Resource(Resource),
    Process(LiveClientPidfdErrorV2),
    Mechanical(MechanicalError),
    Binding,
    Readiness,
}
type Error = NativeApplicationObservationErrorV3;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<LiveClientPidfdErrorV2> for Error {
    fn from(e: LiveClientPidfdErrorV2) -> Self {
        Self::Process(e)
    }
}
impl From<MechanicalError> for Error {
    fn from(e: MechanicalError) -> Self {
        Self::Mechanical(e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native application observation: {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeApplicationObservationStorageV3(usize);
impl NativeApplicationObservationStorageV3 {
    /// Reserve this growth in addition to the three consumed input owners.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Retains the original native process owners and exactly observed V5 bytes.
///
/// This is point-in-time process/input observation. Registration must separately
/// authenticate the source of the original Cargo and application pidfds and keep
/// the application at its pre-ACK barrier. No publication is recovered and no
/// currentness, protected peer, proof or GPU authority is produced. In particular,
/// equal image bytes do not detect an earlier same-image re-exec.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::RetainedNativeApplicationObservationV3;
/// fn duplicate<T: Clone>() {}
/// duplicate::<RetainedNativeApplicationObservationV3<'_>>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::RetainedNativeApplicationObservationV3;
/// fn fd<T: std::os::fd::AsFd>() {}
/// fd::<RetainedNativeApplicationObservationV3<'_>>();
/// ```
pub struct RetainedNativeApplicationObservationV3<'work> {
    application: Client,
    parent: Client,
    binding: Binding,
    application_proc: RetainedFile,
    parent_proc: RetainedFile,
    executable: RetainedFile,
    envelope: RetainedFile,
    directory: RetainedFile,
    proof: ProofEndpointFacts,
    readiness: Box<[u8]>,
    retained: usize,
    ledger: Ledger,
    account: Option<Account>,
    lifetime: PhantomData<&'work Work>,
}
type Observation<'work> = RetainedNativeApplicationObservationV3<'work>;

impl<'work> Observation<'work> {
    /// Consumes prepaid owners; on error their reservations remain caller-owned.
    /// All bounded I/O, hashing and temporary allocations use the original budget.
    /// The ACK and application-side proof aliases are dropped before returning.
    pub fn observe_pre_ack(
        application: Client,
        parent: Client,
        binding: Binding,
        root_proof_peer: BorrowedFd<'_>,
        budget: &mut Budget<'work>,
    ) -> Result<(Self, NativeApplicationObservationStorageV3)> {
        budget.charge_work(ENTRY)?;
        let floor = input_floor(&application, &parent, &binding)?;
        let (work, scratch) = allowance(&binding)?;
        budget.with_prepaid_scope(floor, 0, work, scratch, |b| {
            if !matches_process(
                application.expected_client(),
                binding.compiler_handoff().launch_manifest().client(),
            ) || !matches_process(
                parent.expected_client(),
                binding.compiler_handoff().submitter(),
            ) || application.expected_client().uid != parent.expected_client().uid
                || application.expected_client().gid != parent.expected_client().gid
            {
                return Err(Error::Binding);
            }
            application.validate_parent(&parent, b)?;
            let application_proc = open_process(&application)?;
            let parent_proc = open_process(&parent)?;
            validate_process(&application, &application_proc, b)?;
            validate_process(&parent, &parent_proc, b)?;
            let executable = inspect_executable(&application_proc, application.expected_client())?;
            if executable.snapshot.length != binding.inputs().occurrence().application().byte_len()
            {
                return Err(Error::Binding);
            }
            let image = read_file(&executable, MAX_APPLICATION_BYTES)?;
            let image_identity = WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(&image)
                .map_err(MechanicalError::from)?;
            drop(image);
            let [envelope_slot, directory_slot, ack_slot, proof_slot] =
                binding.inputs().descriptors().as_array();
            let envelope = inspect_remote(
                &application,
                &application_proc,
                envelope_slot,
                Role::Envelope,
            )?;
            let directory = inspect_remote(
                &application,
                &application_proc,
                directory_slot,
                Role::Directory,
            )?;
            let ack = inspect_remote(
                &application,
                &application_proc,
                ack_slot,
                Role::Acknowledgment,
            )?;
            let ack_snapshot = ack.snapshot;
            drop(ack);
            let proof = inspect_remote_proof(
                &application,
                &application_proc,
                proof_slot,
                parent.expected_client(),
            )?;
            proof.require_counterpart(&inspect_proof_endpoint(
                root_proof_peer,
                parent.expected_client(),
            )?)?;
            let observed = Occurrence::new(
                image_identity,
                binding.inputs().occurrence().spawn_identity(),
                &[
                    envelope.snapshot.input(1)?,
                    directory.snapshot.input(2)?,
                    ack_snapshot.input(3)?,
                    proof.snapshot.input(4)?,
                ],
            )
            .map_err(MechanicalError::from)?;
            if &observed != binding.inputs().occurrence()
                || envelope.snapshot.length != binding.inputs().readiness_byte_len()
            {
                return Err(Error::Binding);
            }
            let readiness = read_file(&envelope, MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5 as u64)?;
            if <[u8; 32]>::from(Sha256::digest(&readiness)) != binding.inputs().readiness_sha256() {
                return Err(Error::Binding);
            }
            InertConditionalWorkerReadinessWireV5::decode(
                &readiness,
                MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5,
            )
            .map_err(|_| Error::Readiness)?;
            let growth = size_of::<Self>()
                .checked_sub(size_of::<(Client, Client, Binding)>())
                .and_then(|n| n.checked_add(readiness.len()))
                .ok_or(Resource::Arithmetic)?;
            let retained = floor.checked_add(growth).ok_or(Resource::Arithmetic)?;
            let value = Self {
                application,
                parent,
                binding,
                application_proc,
                parent_proc,
                executable,
                envelope,
                directory,
                proof,
                readiness: readiness.into_boxed_slice(),
                retained,
                ledger: b.work_ledger_identity_v1(),
                account: b.storage_account_identity_v1(),
                lifetime: PhantomData,
            };
            value.check(root_proof_peer, b)?;
            let ack = inspect_remote(
                &value.application,
                &value.application_proc,
                ack_slot,
                Role::Acknowledgment,
            )?;
            if ack.snapshot != ack_snapshot {
                return Err(Error::Binding);
            }
            drop(ack);
            value.application.validate_parent(&value.parent, b)?;
            Ok((value, NativeApplicationObservationStorageV3(growth)))
        })
    }

    /// Revalidates exact retained process/object continuity, not application readiness.
    pub fn revalidate(
        &self,
        root_proof_peer: BorrowedFd<'_>,
        budget: &mut Budget<'work>,
    ) -> Result<()> {
        budget.charge_work(ENTRY)?;
        if budget.work_ledger_identity_v1() != self.ledger
            || budget.storage_account_identity_v1() != self.account
        {
            return Err(Resource::Accounting.into());
        }
        let (work, scratch) = allowance(&self.binding)?;
        budget.with_prepaid_scope(self.retained, 0, work, scratch, |b| {
            self.check(root_proof_peer, b)
        })
    }

    fn check(&self, root_proof_peer: BorrowedFd<'_>, b: &mut Budget<'_>) -> Result<()> {
        self.application.validate_parent(&self.parent, b)?;
        validate_process(&self.application, &self.application_proc, b)?;
        validate_process(&self.parent, &self.parent_proc, b)?;
        self.executable.revalidate()?;
        require_seals(&self.executable.file)?;
        if inspect_executable(&self.application_proc, self.application.expected_client())?.snapshot
            != self.executable.snapshot
        {
            return Err(Error::Binding);
        }
        self.envelope.revalidate()?;
        self.directory.revalidate()?;
        let [envelope, directory, _, proof] = self.binding.inputs().descriptors().as_array();
        for (slot, role, expected) in [
            (envelope, Role::Envelope, self.envelope.snapshot),
            (directory, Role::Directory, self.directory.snapshot),
        ] {
            if inspect_remote(&self.application, &self.application_proc, slot, role)?.snapshot
                != expected
            {
                return Err(Error::Binding);
            }
        }
        if inspect_remote_proof(
            &self.application,
            &self.application_proc,
            proof,
            self.parent.expected_client(),
        )? != self.proof
        {
            return Err(Error::Binding);
        }
        self.proof.require_counterpart(&inspect_proof_endpoint(
            root_proof_peer,
            self.parent.expected_client(),
        )?)?;
        if read_file(
            &self.envelope,
            MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5 as u64,
        )? != *self.readiness
        {
            return Err(Error::Binding);
        }
        self.application.validate_parent(&self.parent, b)?;
        Ok(())
    }

    pub const fn binding(&self) -> &Binding {
        &self.binding
    }
    pub(crate) const fn application(&self) -> &Client {
        &self.application
    }
    pub(crate) const fn parent(&self) -> &Client {
        &self.parent
    }

    // Only an authenticated registration owner may consume these original
    // owners after revalidation. No bytes or reopened PID reconstruct custody.
    pub(crate) fn into_registration_owners(self) -> (Client, Client, Binding) {
        (self.application, self.parent, self.binding)
    }
    pub fn exact_readiness_bytes(&self) -> &[u8] {
        &self.readiness
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn authenticates_protected_currentness(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn input_floor(application: &Client, parent: &Client, binding: &Binding) -> Result<usize> {
    application
        .retained_storage()
        .checked_add(parent.retained_storage())
        .and_then(|n| n.checked_add(binding.retained_storage()))
        .ok_or(Resource::Arithmetic.into())
}

fn allowance(binding: &Binding) -> Result<(usize, usize)> {
    allowance_for_lengths(
        binding.inputs().occurrence().application().byte_len(),
        binding.inputs().readiness_byte_len(),
    )
}

fn allowance_for_lengths(image: u64, readiness: u64) -> Result<(usize, usize)> {
    if image == 0
        || image > MAX_APPLICATION_BYTES
        || readiness == 0
        || readiness > MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5 as u64
    {
        return Err(Error::Binding);
    }
    let bytes = usize::try_from(image)
        .ok()
        .and_then(|n| n.checked_add(usize::try_from(readiness).ok()?))
        .ok_or(Resource::Arithmetic)?;
    let work = bytes
        .checked_mul(16)
        .and_then(|n| n.checked_add(FIXED))
        .ok_or(Resource::Arithmetic)?;
    let scratch = bytes
        .checked_mul(4)
        .and_then(|n| n.checked_add(FIXED))
        .ok_or(Resource::Arithmetic)?;
    Ok((work, scratch))
}

fn matches_process(
    actual: ExpectedClientProcessIdentityV1,
    expected: fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1,
) -> bool {
    (actual.pid, actual.uid, actual.gid) == (expected.pid(), expected.uid(), expected.gid())
}

fn open_process(process: &Client) -> Result<RetainedFile> {
    let file = File::from(
        rustix::fs::open(
            format!("/proc/{}", process.expected_client().pid),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| io_error("open native application proc directory", e))?,
    );
    require_procfs(&file, "native application proc directory").map_err(MechanicalError::from)?;
    Ok(RetainedFile::new(file)?)
}

// One fixed-size read, with no retry on interruption or short/truncated records.
fn read_proc(directory: &RetainedFile, name: &str) -> Result<Vec<u8>> {
    directory.revalidate()?;
    let fd = rustix::fs::openat(
        &directory.file,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|e| io_error("open native application proc record", e))?;
    let file = File::from(fd);
    require_procfs(&file, "native application proc record").map_err(MechanicalError::from)?;
    let mut bytes = vec![0; MAX_PROC_RECORD_BYTES as usize + 1];
    let count = rustix::io::read(&file, &mut bytes)
        .map_err(|e| io_error("read native application proc record", e))?;
    bytes.truncate(count);
    if bytes.is_empty() || bytes.len() > MAX_PROC_RECORD_BYTES as usize || !bytes.ends_with(b"\n") {
        return Err(invalid("native application proc record is truncated or over bound").into());
    }
    directory.revalidate()?;
    Ok(bytes)
}

fn validate_process(
    process: &Client,
    directory: &RetainedFile,
    budget: &mut Budget<'_>,
) -> Result<()> {
    process.validate_liveness(budget)?;
    let expected = process.expected_client();
    let stat = read_proc(directory, "stat")?;
    if super::parse_process_start_time_ticks(&stat, expected.pid).map_err(MechanicalError::from)?
        != process.process_identity().1
    {
        return Err(Error::Binding);
    }
    validate_credentials(&read_proc(directory, "status")?, expected)?;
    process.validate_liveness(budget)?;
    Ok(())
}

fn read_file(file: &RetainedFile, maximum: u64) -> Result<Vec<u8>> {
    file.revalidate()?;
    let length = file.snapshot.length;
    if length == 0 || length > maximum {
        return Err(Error::Binding);
    }
    let length = usize::try_from(length).map_err(|_| Resource::Arithmetic)?;
    let mut bytes = vec![0; length];
    for (index, chunk) in bytes.chunks_mut(CHUNK).enumerate() {
        let offset = index.checked_mul(CHUNK).ok_or(Resource::Arithmetic)?;
        let read = rustix::io::pread(&file.file, &mut *chunk, offset as u64)
            .map_err(|e| io_error("read exact native application object", e))?;
        if read != chunk.len() {
            return Err(Error::Binding);
        }
    }
    file.revalidate()?;
    Ok(bytes)
}

fn duplicate_remote(
    application: &Client,
    directory: &RetainedFile,
    slot: i32,
) -> Result<(RetainedFile, OFlags)> {
    let fd = rustix::process::pidfd_getfd(
        application.pidfd(),
        slot,
        rustix::process::PidfdGetfdFlags::empty(),
    )
    .map_err(|e| io_error("duplicate native application descriptor", e))?;
    let retained = RetainedFile::new(File::from(fd))?;
    let flags = rustix::fs::fcntl_getfl(&retained.file)
        .map_err(|e| io_error("inspect native application descriptor flags", e))?;
    let source = parse_source_flags(&read_proc(directory, &format!("fdinfo/{slot}"))?)?;
    if source & libc::O_CLOEXEC as u32 == 0
        || source & !(libc::O_CLOEXEC as u32) != flags.bits()
        || flags.intersects(OFlags::PATH | OFlags::ASYNC | OFlags::DIRECT | OFlags::APPEND)
    {
        return Err(Error::Binding);
    }
    Ok((retained, flags))
}

fn inspect_remote(
    application: &Client,
    directory: &RetainedFile,
    slot: i32,
    role: Role,
) -> Result<RetainedFile> {
    let (file, flags) = duplicate_remote(application, directory, slot)?;
    let s = file.snapshot;
    let (kind, access) = match role {
        Role::Envelope => (FileType::RegularFile, OFlags::RDONLY),
        Role::Directory => (FileType::Directory, OFlags::RDONLY),
        Role::Acknowledgment => (FileType::Fifo, OFlags::WRONLY),
    };
    if FileType::from_raw_mode(s.mode) != kind
        || flags & OFlags::ACCMODE != access
        || s.uid != application.expected_client().uid
        || s.links == 0
        || s.mode & 0o077 != 0
        || (matches!(role, Role::Acknowledgment) && !flags.contains(OFlags::NONBLOCK))
        || (matches!(role, Role::Envelope)
            && (s.links != 1
                || s.length == 0
                || s.length > MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5 as u64))
    {
        return Err(Error::Binding);
    }
    file.revalidate()?;
    Ok(file)
}

fn inspect_remote_proof(
    application: &Client,
    directory: &RetainedFile,
    slot: i32,
    creator: ExpectedClientProcessIdentityV1,
) -> Result<ProofEndpointFacts> {
    let (file, _) = duplicate_remote(application, directory, slot)?;
    let facts = inspect_proof_endpoint(file.file.as_fd(), creator)?;
    if facts.snapshot != file.snapshot {
        return Err(Error::Binding);
    }
    file.revalidate()?;
    Ok(facts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn memory_file(bytes: &[u8]) -> RetainedFile {
        let fd =
            rustix::fs::memfd_create("native-observation-test", rustix::fs::MemfdFlags::CLOEXEC)
                .unwrap();
        let mut file = File::from(fd);
        file.write_all(bytes).unwrap();
        RetainedFile::new(file).unwrap()
    }

    #[test]
    fn bounded_read_preserves_exact_bytes_across_chunk_boundary() {
        let bytes: Vec<u8> = (0..CHUNK + 7).map(|i| (i % 251) as u8).collect();
        let file = memory_file(&bytes);
        assert_eq!(read_file(&file, bytes.len() as u64).unwrap(), bytes);
        assert!(read_file(&file, bytes.len() as u64 - 1).is_err());
        assert!(read_file(&memory_file(&[]), 1).is_err());
    }

    #[test]
    fn changed_retained_file_is_refused_before_reading() {
        let file = memory_file(b"original");
        rustix::fs::ftruncate(&file.file, 1).unwrap();
        assert!(read_file(&file, 64).is_err());
    }

    #[test]
    fn length_quotes_are_finite_and_refuse_each_out_of_profile_axis() {
        let max = MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5 as u64;
        for (image, readiness) in [(1, 1), (MAX_APPLICATION_BYTES, max)] {
            let (work, scratch) = allowance_for_lengths(image, readiness).unwrap();
            assert!(work >= scratch);
            assert!(scratch >= (image + readiness) as usize * 4);
        }
        for (image, readiness) in [
            (0, 1),
            (1, 0),
            (MAX_APPLICATION_BYTES + 1, 1),
            (1, max + 1),
            (u64::MAX, u64::MAX),
        ] {
            assert!(allowance_for_lengths(image, readiness).is_err());
        }
    }

    #[test]
    fn process_matching_binds_every_coordinate() {
        let actual = ExpectedClientProcessIdentityV1::new(7, 8, 9).unwrap();
        for (pid, uid, gid, matches) in [
            (7, 8, 9, true),
            (6, 8, 9, false),
            (7, 6, 9, false),
            (7, 8, 6, false),
        ] {
            let expected =
                fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1::new(
                    pid, uid, gid,
                )
                .unwrap();
            assert_eq!(matches_process(actual, expected), matches);
        }
    }
}
