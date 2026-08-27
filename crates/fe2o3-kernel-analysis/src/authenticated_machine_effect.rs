//! Policy-authenticated Linux execution for physical machine-effect analysis.
//!
//! This is intentionally separate from the descriptive pathname helper. A
//! production worker is copied into a sealed memfd, executed only through that
//! retained image, and accepted only when both its bytes and its observed
//! file-backed runtime closure match a caller-pinned policy. Fresh OS challenges
//! bind the ready/done/ack process handshake, identity probe, and every analysis
//! response. Each receipt binds every file-backed mapping instance and
//! kernel-provided executable mapping observed at ready by address range,
//! permissions, file offset, device/inode, path, digest, and object length; the
//! exact admitted mapping set is checked again before acknowledgement.
//! Persistent anonymous executable mappings are rejected, while ordinary
//! anonymous non-executable allocator mappings may move. This two-snapshot
//! check cannot observe a mapping created and removed entirely between
//! snapshots. Receipts remain inert: they cannot publish, load, or launch code.
//!
//! The authenticated result is still only a list of reachable static
//! instruction sites and their bounded effect kinds for one exact finalized
//! HSACO. It does not establish concrete addresses, runtime execution counts,
//! out-of-bounds absence, race freedom, source/compiler refinement, Verus
//! correctness, or safe dispatch.

use crate::{
    MAX_PHYSICAL_MACHINE_EFFECT_EVIDENCE_BYTES_V1, PhysicalMachineAnalyzerIdentityV1,
    PhysicalMachineEffectEntryRequestV1, PhysicalMachineEffectEvidenceErrorV1,
    PhysicalMachineEffectEvidenceV1, PhysicalMachineEffectRequestErrorV1,
    PhysicalMachineEffectRequestV1, PhysicalMachineExecutionChallengeV1,
    PhysicalMachineToolchainIdentityV1,
};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt, path::Path, time::Duration};

const EXECUTABLE_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-WORKER-EXECUTABLE/V1\0";
const RUNTIME_CLOSURE_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-RUNTIME-CLOSURE/V1\0";
const RUNTIME_MAPPING_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-RUNTIME-MAPPINGS/V1\0";
const RECEIPT_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-AUTHENTICATED-RECEIPT/V1\0";
const RECEIPT_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-AUTHENTICATED-RECEIPT-RECORD/V1\0";
const IDENTITY_CHALLENGE_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-IDENTITY-CHALLENGE/V1\0";
const IDENTITY_RESPONSE_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-IDENTITY-RESPONSE/V1\0";
const WORKER_READY_DOMAIN: &[u8] = b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-WORKER-READY/V1\0";
const WORKER_DONE_DOMAIN: &[u8] = b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-WORKER-DONE/V1\0";
const WORKER_ACK_DOMAIN: &[u8] = b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-WORKER-ACK/V1\0";
const CONTAINMENT_RESPONSE: &[u8] = b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-CONTAINMENT-OK/V1\0";
const SCHEMA_VERSION: u16 = 1;

pub const MAX_PHYSICAL_MACHINE_EFFECT_WORKER_BYTES_V1: u64 = 512 * 1024 * 1024;
pub const MAX_PHYSICAL_MACHINE_EFFECT_RUNTIME_FILES_V1: usize = 256;
pub const MAX_PHYSICAL_MACHINE_EFFECT_RUNTIME_MAPPINGS_V1: usize = 4096;
pub const MAX_PHYSICAL_MACHINE_EFFECT_RUNTIME_BYTES_V1: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_PHYSICAL_MACHINE_EFFECT_STDERR_BYTES_V1: usize = 64 * 1024;
pub const DEFAULT_PHYSICAL_MACHINE_EFFECT_TIMEOUT_V1: Duration = Duration::from_secs(120);
pub const MAX_PHYSICAL_MACHINE_EFFECT_TIMEOUT_V1: Duration = Duration::from_secs(10 * 60);

macro_rules! measured_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            sha256: [u8; 32],
            byte_len: u64,
        }

        impl $name {
            pub const fn from_parts(sha256: [u8; 32], byte_len: u64) -> Self {
                Self { sha256, byte_len }
            }

            pub const fn sha256(self) -> [u8; 32] {
                self.sha256
            }

            pub const fn byte_len(self) -> u64 {
                self.byte_len
            }
        }
    };
}

measured_identity!(PhysicalMachineWorkerExecutableIdentityV1);
measured_identity!(PhysicalMachineRuntimeClosureIdentityV1);
measured_identity!(PhysicalMachineRuntimeMappingIdentityV1);
measured_identity!(AuthenticatedPhysicalMachineEffectReceiptIdentityV1);

impl PhysicalMachineWorkerExecutableIdentityV1 {
    pub fn calculate(bytes: &[u8]) -> Self {
        Self {
            sha256: domain_hash(EXECUTABLE_IDENTITY_DOMAIN, bytes),
            byte_len: bytes.len() as u64,
        }
    }
}

/// Deployment-pinned executable and ASLR-stable file-object closure.
///
/// Per-execution mapping ranges, permissions, offsets, and object identities
/// are bound separately in the authenticated receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalMachineEffectWorkerPolicyV1 {
    executable: PhysicalMachineWorkerExecutableIdentityV1,
    runtime_closure: PhysicalMachineRuntimeClosureIdentityV1,
}

impl PhysicalMachineEffectWorkerPolicyV1 {
    pub fn new(
        executable: PhysicalMachineWorkerExecutableIdentityV1,
        runtime_closure: PhysicalMachineRuntimeClosureIdentityV1,
    ) -> Result<Self, AuthenticatedPhysicalMachineEffectErrorV1> {
        if executable.byte_len == 0
            || executable.byte_len > MAX_PHYSICAL_MACHINE_EFFECT_WORKER_BYTES_V1
            || executable.sha256 == [0; 32]
            || runtime_closure.byte_len == 0
            || runtime_closure.sha256 == [0; 32]
        {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::InvalidPolicy,
            ));
        }
        Ok(Self {
            executable,
            runtime_closure,
        })
    }

    pub const fn executable(self) -> PhysicalMachineWorkerExecutableIdentityV1 {
        self.executable
    }

    pub const fn runtime_closure(self) -> PhysicalMachineRuntimeClosureIdentityV1 {
        self.runtime_closure
    }
}

/// Inert candidate measurement for review and deployment policy generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalMachineEffectWorkerCandidateV1 {
    policy: PhysicalMachineEffectWorkerPolicyV1,
    analyzer_identity: PhysicalMachineAnalyzerIdentityV1,
    toolchain_identity: PhysicalMachineToolchainIdentityV1,
}

impl PhysicalMachineEffectWorkerCandidateV1 {
    pub const fn policy(&self) -> PhysicalMachineEffectWorkerPolicyV1 {
        self.policy
    }

    pub const fn analyzer_identity(&self) -> PhysicalMachineAnalyzerIdentityV1 {
        self.analyzer_identity
    }

    pub const fn toolchain_identity(&self) -> PhysicalMachineToolchainIdentityV1 {
        self.toolchain_identity
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedPhysicalMachineEffectLimitsV1 {
    timeout: Duration,
    stdout_bytes: usize,
    stderr_bytes: usize,
}

impl AuthenticatedPhysicalMachineEffectLimitsV1 {
    pub fn new(
        timeout: Duration,
        stdout_bytes: usize,
        stderr_bytes: usize,
    ) -> Result<Self, AuthenticatedPhysicalMachineEffectErrorV1> {
        if timeout.is_zero()
            || timeout > MAX_PHYSICAL_MACHINE_EFFECT_TIMEOUT_V1
            || stdout_bytes == 0
            || stdout_bytes > MAX_PHYSICAL_MACHINE_EFFECT_EVIDENCE_BYTES_V1
            || stderr_bytes > MAX_PHYSICAL_MACHINE_EFFECT_STDERR_BYTES_V1
        {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::InvalidLimits,
            ));
        }
        Ok(Self {
            timeout,
            stdout_bytes,
            stderr_bytes,
        })
    }

    pub const fn timeout(self) -> Duration {
        self.timeout
    }
}

impl Default for AuthenticatedPhysicalMachineEffectLimitsV1 {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_PHYSICAL_MACHINE_EFFECT_TIMEOUT_V1,
            stdout_bytes: MAX_PHYSICAL_MACHINE_EFFECT_EVIDENCE_BYTES_V1,
            stderr_bytes: 16 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticatedPhysicalMachineEffectTerminationV1 {
    Exit(i32),
    Signal(i32),
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AuthenticatedPhysicalMachineEffectErrorKindV1 {
    UnsupportedPlatform,
    InvalidPolicy,
    InvalidLimits,
    OpenWorker,
    WorkerNotRegular,
    WorkerNotExecutable,
    WorkerNotNativeElf,
    WorkerChangedDuringCapture,
    WorkerIdentityMismatch {
        expected: PhysicalMachineWorkerExecutableIdentityV1,
        actual: PhysicalMachineWorkerExecutableIdentityV1,
    },
    PreparePinnedImage,
    Spawn,
    ProcessObservation,
    RuntimeClosureChanged,
    ConfigureResourceLimits,
    ContainmentUnavailable,
    RuntimeClosureMismatch {
        expected: PhysicalMachineRuntimeClosureIdentityV1,
        actual: PhysicalMachineRuntimeClosureIdentityV1,
    },
    ConfigurePipe,
    ControlHandshake,
    WriteRequest,
    RequestWriteIncomplete,
    ReadStdout,
    ReadStderr,
    StdoutLimitExceeded,
    StderrLimitExceeded,
    Timeout,
    Wait,
    ProcessTreeNotQuiescent,
    ExitFailure(AuthenticatedPhysicalMachineEffectTerminationV1),
    UnexpectedStderr,
    IdentityProbe,
    IdentityMismatch,
    Request(PhysicalMachineEffectRequestErrorV1),
    Evidence(PhysicalMachineEffectEvidenceErrorV1),
    PersistReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedPhysicalMachineEffectErrorV1 {
    kind: Box<AuthenticatedPhysicalMachineEffectErrorKindV1>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    detail: Option<String>,
}

impl AuthenticatedPhysicalMachineEffectErrorV1 {
    fn plain(kind: AuthenticatedPhysicalMachineEffectErrorKindV1) -> Self {
        Self {
            kind: Box::new(kind),
            stdout: Vec::new(),
            stderr: Vec::new(),
            detail: None,
        }
    }

    fn detail(kind: AuthenticatedPhysicalMachineEffectErrorKindV1, detail: impl ToString) -> Self {
        Self {
            kind: Box::new(kind),
            stdout: Vec::new(),
            stderr: Vec::new(),
            detail: Some(detail.to_string()),
        }
    }

    pub const fn kind(&self) -> &AuthenticatedPhysicalMachineEffectErrorKindV1 {
        &self.kind
    }

    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
}

impl fmt::Display for AuthenticatedPhysicalMachineEffectErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "authenticated physical machine-effect execution failed: {:?}",
            self.kind
        )?;
        if let Some(detail) = &self.detail {
            write!(formatter, ": {detail}")?;
        }
        Ok(())
    }
}

impl Error for AuthenticatedPhysicalMachineEffectErrorV1 {}

/// A policy-authenticated static-site analysis receipt. Deliberately not `Clone`.
///
/// Authentication binds the worker image, observed object and executable mapping closures,
/// challenge, and exact evidence bytes. Every file-backed mapping and kernel-provided
/// executable mapping is retained exactly. Anonymous non-executable allocator mappings may move
/// during analysis; persistent anonymous executable mappings are rejected. This does not upgrade
/// the static effect list into a memory-safety, race-freedom, refinement, or runtime-count proof.
pub struct AuthenticatedPhysicalMachineEffectExecutionV1 {
    policy: PhysicalMachineEffectWorkerPolicyV1,
    execution_challenge: PhysicalMachineExecutionChallengeV1,
    process_id: u32,
    process_start_ticks: u64,
    runtime_mapping_identity: PhysicalMachineRuntimeMappingIdentityV1,
    evidence: PhysicalMachineEffectEvidenceV1,
    canonical_receipt: Vec<u8>,
}

impl fmt::Debug for AuthenticatedPhysicalMachineEffectExecutionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedPhysicalMachineEffectExecutionV1")
            .field("policy", &self.policy)
            .field("process_id", &self.process_id)
            .field("process_start_ticks", &self.process_start_ticks)
            .field("evidence_identity", &self.evidence.identity())
            .finish_non_exhaustive()
    }
}

impl AuthenticatedPhysicalMachineEffectExecutionV1 {
    pub const fn policy(&self) -> PhysicalMachineEffectWorkerPolicyV1 {
        self.policy
    }

    pub const fn execution_challenge(&self) -> PhysicalMachineExecutionChallengeV1 {
        self.execution_challenge
    }

    pub const fn process_id(&self) -> u32 {
        self.process_id
    }

    pub const fn process_start_ticks(&self) -> u64 {
        self.process_start_ticks
    }

    pub const fn runtime_mapping_identity(&self) -> PhysicalMachineRuntimeMappingIdentityV1 {
        self.runtime_mapping_identity
    }

    pub const fn evidence(&self) -> &PhysicalMachineEffectEvidenceV1 {
        &self.evidence
    }

    pub fn canonical_receipt_bytes(&self) -> &[u8] {
        &self.canonical_receipt
    }

    pub fn identity(&self) -> AuthenticatedPhysicalMachineEffectReceiptIdentityV1 {
        AuthenticatedPhysicalMachineEffectReceiptIdentityV1 {
            sha256: domain_hash(RECEIPT_IDENTITY_DOMAIN, &self.canonical_receipt),
            byte_len: self.canonical_receipt.len() as u64,
        }
    }

    pub fn persist_create_new(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), AuthenticatedPhysicalMachineEffectErrorV1> {
        persist_receipt(path.as_ref(), &self.canonical_receipt)
    }

    pub const fn authenticates_analyzer_execution(&self) -> bool {
        true
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn encode_identity_challenge(challenge: PhysicalMachineExecutionChallengeV1) -> Vec<u8> {
    let mut output = Vec::with_capacity(IDENTITY_CHALLENGE_DOMAIN.len() + 38);
    output.extend_from_slice(IDENTITY_CHALLENGE_DOMAIN);
    push_u32(&mut output, 0);
    push_u16(&mut output, SCHEMA_VERSION);
    output.extend_from_slice(&challenge.as_bytes());
    let length = output.len() as u32;
    let offset = IDENTITY_CHALLENGE_DOMAIN.len();
    output[offset..offset + 4].copy_from_slice(&length.to_le_bytes());
    output
}

fn decode_identity_response(
    bytes: &[u8],
    challenge: PhysicalMachineExecutionChallengeV1,
) -> Result<
    (
        PhysicalMachineAnalyzerIdentityV1,
        PhysicalMachineToolchainIdentityV1,
    ),
    AuthenticatedPhysicalMachineEffectErrorV1,
> {
    let expected = IDENTITY_RESPONSE_DOMAIN.len() + 4 + 2 + 32 + 32 + 32;
    if bytes.len() != expected || !bytes.starts_with(IDENTITY_RESPONSE_DOMAIN) {
        return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
            AuthenticatedPhysicalMachineEffectErrorKindV1::IdentityProbe,
        ));
    }
    let mut position = IDENTITY_RESPONSE_DOMAIN.len();
    let length = u32::from_le_bytes(bytes[position..position + 4].try_into().unwrap()) as usize;
    position += 4;
    let version = u16::from_le_bytes(bytes[position..position + 2].try_into().unwrap());
    position += 2;
    let observed_challenge: [u8; 32] = bytes[position..position + 32].try_into().unwrap();
    position += 32;
    let analyzer: [u8; 32] = bytes[position..position + 32].try_into().unwrap();
    position += 32;
    let toolchain: [u8; 32] = bytes[position..position + 32].try_into().unwrap();
    if length != bytes.len()
        || version != SCHEMA_VERSION
        || observed_challenge != challenge.as_bytes()
        || analyzer == [0; 32]
        || toolchain == [0; 32]
    {
        return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
            AuthenticatedPhysicalMachineEffectErrorKindV1::IdentityMismatch,
        ));
    }
    Ok((
        PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes(analyzer),
        PhysicalMachineToolchainIdentityV1::from_sha256_bytes(toolchain),
    ))
}

fn encode_receipt(
    policy: PhysicalMachineEffectWorkerPolicyV1,
    challenge: PhysicalMachineExecutionChallengeV1,
    process_id: u32,
    process_start_ticks: u64,
    runtime_mapping: PhysicalMachineRuntimeMappingIdentityV1,
    evidence: &PhysicalMachineEffectEvidenceV1,
) -> Vec<u8> {
    let mut output = Vec::with_capacity(RECEIPT_DOMAIN.len() + 256);
    output.extend_from_slice(RECEIPT_DOMAIN);
    push_u32(&mut output, 0);
    push_u16(&mut output, SCHEMA_VERSION);
    output.extend_from_slice(&policy.executable.sha256);
    push_u64(&mut output, policy.executable.byte_len);
    output.extend_from_slice(&policy.runtime_closure.sha256);
    push_u64(&mut output, policy.runtime_closure.byte_len);
    output.extend_from_slice(&challenge.as_bytes());
    push_u32(&mut output, process_id);
    push_u64(&mut output, process_start_ticks);
    output.extend_from_slice(&runtime_mapping.sha256);
    push_u64(&mut output, runtime_mapping.byte_len);
    let request = evidence.request_identity();
    output.extend_from_slice(&request.sha256());
    push_u64(&mut output, request.byte_len());
    let evidence_identity = evidence.identity();
    output.extend_from_slice(&evidence_identity.sha256());
    push_u64(&mut output, evidence_identity.byte_len());
    let length = output.len() as u32;
    let offset = RECEIPT_DOMAIN.len();
    output[offset..offset + 4].copy_from_slice(&length.to_le_bytes());
    output
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    digest.finalize().into()
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    output
}

fn control_frame(domain: &[u8], challenge: PhysicalMachineExecutionChallengeV1) -> Vec<u8> {
    let mut output = Vec::with_capacity(domain.len() + 32);
    output.extend_from_slice(domain);
    output.extend_from_slice(&challenge.as_bytes());
    output
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use rustix::{
        fs::{MemfdFlags, Mode, OFlags, SealFlags},
        io::Errno,
        process::{
            Pid, PidfdFlags, Resource, Rlimit, Signal, WaitId, WaitIdOptions, getpgid, getrlimit,
            getsid, kill_process_group, pidfd_open, pidfd_send_signal, setrlimit, setsid, waitid,
        },
        thread::set_no_new_privs,
    };
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs::{File, Metadata, OpenOptions},
        io::{self, Read, Seek, SeekFrom, Write},
        os::{
            fd::{AsFd, AsRawFd, OwnedFd},
            unix::{fs::MetadataExt, process::CommandExt},
        },
        path::PathBuf,
        process::{Child, ChildStderr, Command, ExitStatus, Stdio},
        thread,
        time::Instant,
    };

    const IO_CHUNK_BYTES: usize = 64 * 1024;
    const POLL_INTERVAL: Duration = Duration::from_millis(2);
    const DESCENDANT_SCAN_INTERVAL: Duration = Duration::from_millis(25);
    const DRAIN_GRACE: Duration = Duration::from_millis(200);
    const CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
    const MAX_PROC_MAPS_BYTES: usize = 1024 * 1024;
    const MAX_RUNTIME_MAPPING_CANONICAL_BYTES: usize = 4 * 1024 * 1024;
    const MAX_PROC_STAT_BYTES: usize = 4096;
    const MAX_PROC_STATUS_BYTES: usize = 16 * 1024;
    const WORKER_ADDRESS_SPACE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
    const WORKER_DATA_BYTES: u64 = 2 * 1024 * 1024 * 1024;
    const WORKER_FILE_BYTES: u64 = 16 * 1024 * 1024;
    const REQUIRED_SEALS: SealFlags = SealFlags::WRITE
        .union(SealFlags::GROW)
        .union(SealFlags::SHRINK)
        .union(SealFlags::SEAL);
    const ENVIRONMENT: &[(&str, &str)] = &[("LANG", "C"), ("LC_ALL", "C"), ("TZ", "UTC")];

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Snapshot {
        device: u64,
        inode: u64,
        mode: u32,
        size: u64,
        modified_seconds: i64,
        modified_nanoseconds: i64,
        changed_seconds: i64,
        changed_nanoseconds: i64,
    }

    impl Snapshot {
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

    #[derive(Debug)]
    struct Capture {
        bytes: Vec<u8>,
        overflow: bool,
    }

    impl Capture {
        fn new() -> Self {
            Self {
                bytes: Vec::new(),
                overflow: false,
            }
        }
    }

    struct WorkerControlCapture {
        done: bool,
        stderr: Capture,
    }

    type RuntimeFileIdentity = ((u32, u32, u64), String, [u8; 32], u64);

    struct SupervisionContext<'a> {
        stderr_pipe: ChildStderr,
        request: &'a [u8],
        challenge: PhysicalMachineExecutionChallengeV1,
        limits: AuthenticatedPhysicalMachineEffectLimitsV1,
        deadline: Instant,
        observation: &'a mut ProcessObservation,
    }

    #[derive(Debug)]
    struct ChildProcessGuard {
        child: Child,
        pidfd: OwnedFd,
        descendant_pidfds: BTreeMap<u32, OwnedFd>,
        cleanup_deadline: Option<Instant>,
        reaped: bool,
    }

    impl ChildProcessGuard {
        fn new(child: Child) -> Result<Self, AuthenticatedPhysicalMachineEffectErrorV1> {
            Self::new_with_pidfd_open(child, |pid| Ok(pidfd_open(pid, PidfdFlags::empty())?))
        }

        fn new_with_pidfd_open(
            mut child: Child,
            open_pidfd: impl FnOnce(Pid) -> io::Result<OwnedFd>,
        ) -> Result<Self, AuthenticatedPhysicalMachineEffectErrorV1> {
            let pid = child_pid(&child);
            let pidfd = match open_pidfd(pid) {
                Ok(pidfd) => pidfd,
                Err(error) => {
                    terminate_without_pidfd(pid);
                    let _ = reap_child_until(&mut child, Instant::now() + CLEANUP_TIMEOUT);
                    return Err(containment_io(error));
                }
            };
            if let Err(error) = validate_pidfd_identity(&pidfd, pid) {
                terminate_without_pidfd(pid);
                let _ = reap_child_until(&mut child, Instant::now() + CLEANUP_TIMEOUT);
                return Err(containment_io(error));
            }
            if let Err(error) = validate_session_leader(pid) {
                terminate_owned_process(&mut child, &pidfd, &BTreeMap::new());
                let _ = reap_child_until(&mut child, Instant::now() + CLEANUP_TIMEOUT);
                return Err(containment_io(error));
            }
            Ok(Self {
                child,
                pidfd,
                descendant_pidfds: BTreeMap::new(),
                cleanup_deadline: None,
                reaped: false,
            })
        }

        fn refresh_descendants(&mut self) -> io::Result<()> {
            let root = child_pid(&self.child);
            validate_session_leader(root)?;
            for raw in descendants(self.child.id()) {
                if self.descendant_pidfds.contains_key(&raw) {
                    continue;
                }
                let pid = checked_pid(raw)?;
                let pidfd = retain_descendant_pidfd(root, pid)?;
                self.descendant_pidfds.insert(raw, pidfd);
            }
            Ok(())
        }

        fn terminate_and_wait(&mut self) -> io::Result<ExitStatus> {
            let refresh_result = self.refresh_descendants();
            terminate_owned_process(&mut self.child, &self.pidfd, &self.descendant_pidfds);
            let deadline = *self
                .cleanup_deadline
                .get_or_insert_with(|| Instant::now() + CLEANUP_TIMEOUT);
            wait_for_exit_until(deadline, || {
                match waitid(
                    WaitId::PidFd(self.pidfd.as_fd()),
                    WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
                ) {
                    Ok(status) => Ok(status.is_some()),
                    Err(error) => Err(io::Error::from(error)),
                }
            })?;
            let status = self.child.wait()?;
            self.reaped = true;
            refresh_result?;
            Ok(status)
        }

        fn exited_without_reaping(&self) -> io::Result<bool> {
            match waitid(
                WaitId::PidFd(self.pidfd.as_fd()),
                WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
            ) {
                Ok(status) => Ok(status.is_some()),
                Err(Errno::INTR) => Ok(false),
                Err(error) => Err(io::Error::from(error)),
            }
        }
    }

    fn wait_for_exit_until(
        deadline: Instant,
        mut observe_exit: impl FnMut() -> io::Result<bool>,
    ) -> io::Result<()> {
        loop {
            match observe_exit() {
                Ok(true) => return Ok(()),
                Ok(false) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "worker did not become reapable before cleanup deadline",
                ));
            }
            thread::sleep(POLL_INTERVAL.min(deadline.saturating_duration_since(now)));
        }
    }

    fn reap_child_until(child: &mut Child, deadline: Instant) -> io::Result<()> {
        wait_for_exit_until(deadline, || child.try_wait().map(|status| status.is_some()))
    }

    impl Drop for ChildProcessGuard {
        fn drop(&mut self) {
            if !self.reaped {
                let _ = self.terminate_and_wait();
            }
        }
    }

    struct ProcessCapture {
        status: ExitStatus,
        request_written: usize,
        stdout: Capture,
        stderr: Capture,
    }

    struct ProcessObservation {
        process_id: u32,
        start_ticks: u64,
        executable: RetainedExecutable,
        runtime_closure: PhysicalMachineRuntimeClosureIdentityV1,
        runtime_mappings: PhysicalMachineRuntimeMappingIdentityV1,
        runtime_files: Vec<RuntimeFile>,
    }

    struct RetainedExecutable {
        file: File,
        key: (u32, u32, u64),
        link: String,
        snapshot: Snapshot,
        identity: PhysicalMachineWorkerExecutableIdentityV1,
    }

    struct RuntimeFile {
        file: File,
        key: (u32, u32, u64),
        path: String,
        snapshot: Snapshot,
        digest: [u8; 32],
        length: u64,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct RuntimeMapping {
        start: u64,
        end: u64,
        permissions: [u8; 4],
        file_offset: u64,
        key: (u32, u32, u64),
        path: String,
        digest: [u8; 32],
        length: u64,
    }

    struct RawExecution {
        capture: ProcessCapture,
        observation: ProcessObservation,
    }

    pub struct AuthenticatedPhysicalMachineEffectWorkerV1 {
        image: File,
        descriptor_path: PathBuf,
        snapshot: Snapshot,
        policy: PhysicalMachineEffectWorkerPolicyV1,
        analyzer_identity: PhysicalMachineAnalyzerIdentityV1,
        toolchain_identity: PhysicalMachineToolchainIdentityV1,
    }

    impl fmt::Debug for AuthenticatedPhysicalMachineEffectWorkerV1 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("AuthenticatedPhysicalMachineEffectWorkerV1")
                .field("policy", &self.policy)
                .finish_non_exhaustive()
        }
    }

    impl AuthenticatedPhysicalMachineEffectWorkerV1 {
        pub fn open(
            path: impl AsRef<Path>,
            policy: PhysicalMachineEffectWorkerPolicyV1,
            limits: AuthenticatedPhysicalMachineEffectLimitsV1,
        ) -> Result<Self, AuthenticatedPhysicalMachineEffectErrorV1> {
            let (image, descriptor_path, snapshot, executable) =
                capture_and_seal(path.as_ref(), Some(policy.executable))?;
            let mut result = Self {
                image,
                descriptor_path,
                snapshot,
                policy,
                analyzer_identity: PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([0; 32]),
                toolchain_identity: PhysicalMachineToolchainIdentityV1::from_sha256_bytes([0; 32]),
            };
            debug_assert_eq!(executable, policy.executable);
            let (analyzer, toolchain, closure) =
                result.probe_identities(limits, Some(policy.runtime_closure))?;
            if closure != policy.runtime_closure {
                return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureMismatch {
                        expected: policy.runtime_closure,
                        actual: closure,
                    },
                ));
            }
            result.analyzer_identity = analyzer;
            result.toolchain_identity = toolchain;
            Ok(result)
        }

        pub const fn policy(&self) -> PhysicalMachineEffectWorkerPolicyV1 {
            self.policy
        }

        pub const fn analyzer_identity(&self) -> PhysicalMachineAnalyzerIdentityV1 {
            self.analyzer_identity
        }

        pub const fn toolchain_identity(&self) -> PhysicalMachineToolchainIdentityV1 {
            self.toolchain_identity
        }

        #[doc(hidden)]
        pub fn retained_executable_descriptor_path_for_test(&self) -> &Path {
            &self.descriptor_path
        }

        #[doc(hidden)]
        pub fn verify_deployed_no_fork_profile_for_test(
            &self,
            limits: AuthenticatedPhysicalMachineEffectLimitsV1,
        ) -> Result<(), AuthenticatedPhysicalMachineEffectErrorV1> {
            let challenge = fresh_challenge()?;
            let execution = self.run(
                "--machine-effects-containment-probe-v1",
                &[0],
                challenge,
                limits,
                Some(self.policy.runtime_closure),
            )?;
            validate_success(&execution.capture)?;
            if execution.capture.stdout.bytes != CONTAINMENT_RESPONSE {
                return Err(process_error(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ControlHandshake,
                    &execution.capture,
                ));
            }
            Ok(())
        }

        pub fn analyze(
            &self,
            payload: Vec<u8>,
            entries: Vec<PhysicalMachineEffectEntryRequestV1>,
            limits: AuthenticatedPhysicalMachineEffectLimitsV1,
        ) -> Result<
            AuthenticatedPhysicalMachineEffectExecutionV1,
            AuthenticatedPhysicalMachineEffectErrorV1,
        > {
            let challenge = fresh_challenge()?;
            let request = PhysicalMachineEffectRequestV1::new(
                challenge,
                self.analyzer_identity,
                self.toolchain_identity,
                payload,
                entries,
            )
            .map_err(|error| {
                AuthenticatedPhysicalMachineEffectErrorV1::plain(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::Request(error),
                )
            })?;
            let execution = self.run(
                "--machine-effects-gfx942-v1",
                request.canonical_bytes(),
                challenge,
                limits,
                Some(self.policy.runtime_closure),
            )?;
            validate_success(&execution.capture)?;
            if execution.observation.runtime_closure != self.policy.runtime_closure {
                return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureMismatch {
                        expected: self.policy.runtime_closure,
                        actual: execution.observation.runtime_closure,
                    },
                ));
            }
            let evidence = PhysicalMachineEffectEvidenceV1::decode_canonical_for(
                &request,
                &execution.capture.stdout.bytes,
            )
            .map_err(|error| {
                process_error(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::Evidence(error),
                    &execution.capture,
                )
            })?;
            let canonical_receipt = encode_receipt(
                self.policy,
                challenge,
                execution.observation.process_id,
                execution.observation.start_ticks,
                execution.observation.runtime_mappings,
                &evidence,
            );
            Ok(AuthenticatedPhysicalMachineEffectExecutionV1 {
                policy: self.policy,
                execution_challenge: challenge,
                process_id: execution.observation.process_id,
                process_start_ticks: execution.observation.start_ticks,
                runtime_mapping_identity: execution.observation.runtime_mappings,
                evidence,
                canonical_receipt,
            })
        }

        fn probe_identities(
            &self,
            limits: AuthenticatedPhysicalMachineEffectLimitsV1,
            expected_runtime: Option<PhysicalMachineRuntimeClosureIdentityV1>,
        ) -> Result<
            (
                PhysicalMachineAnalyzerIdentityV1,
                PhysicalMachineToolchainIdentityV1,
                PhysicalMachineRuntimeClosureIdentityV1,
            ),
            AuthenticatedPhysicalMachineEffectErrorV1,
        > {
            let challenge = fresh_challenge()?;
            let request = encode_identity_challenge(challenge);
            let execution = self.run(
                "--machine-effects-gfx942-identities-v1",
                &request,
                challenge,
                AuthenticatedPhysicalMachineEffectLimitsV1 {
                    stdout_bytes: 4096,
                    ..limits
                },
                expected_runtime,
            )?;
            validate_success(&execution.capture)?;
            let (analyzer, toolchain) =
                decode_identity_response(&execution.capture.stdout.bytes, challenge)
                    .map_err(|error| process_error((*error.kind).clone(), &execution.capture))?;
            Ok((analyzer, toolchain, execution.observation.runtime_closure))
        }

        fn run(
            &self,
            argument: &str,
            request: &[u8],
            challenge: PhysicalMachineExecutionChallengeV1,
            limits: AuthenticatedPhysicalMachineEffectLimitsV1,
            expected_runtime: Option<PhysicalMachineRuntimeClosureIdentityV1>,
        ) -> Result<RawExecution, AuthenticatedPhysicalMachineEffectErrorV1> {
            validate_no_fork_profile()?;
            validate_image(&self.image, &self.descriptor_path, self.snapshot)?;
            let deadline = Instant::now() + limits.timeout;
            let mut command = Command::new(&self.descriptor_path);
            command
                .arg0("fe2o3-llvm-link-worker")
                .arg(argument)
                .arg(format!(
                    "--fe2o3-control-challenge={}",
                    encode_hex(&challenge.as_bytes())
                ))
                .arg(format!("--fe2o3-request-bytes={}", request.len()))
                .env_clear()
                .envs(ENVIRONMENT.iter().copied())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            configure_worker_pre_exec(&mut command);
            let child = crate::process_execution::spawn(&mut command).map_err(|error| {
                AuthenticatedPhysicalMachineEffectErrorV1::detail(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::Spawn,
                    error,
                )
            })?;
            let mut child = ChildProcessGuard::new(child)?;
            let stderr = child.child.stderr.take().expect("worker stderr is piped");
            let stderr = await_worker_ready(stderr, challenge, deadline)?;
            validate_worker_security_profile(child.child.id())?;
            let mut observation =
                observe_process(child.child.id(), self.policy.executable, deadline)?;
            if observation.executable.identity != self.policy.executable {
                return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::WorkerIdentityMismatch {
                        expected: self.policy.executable,
                        actual: observation.executable.identity,
                    },
                ));
            }
            validate_retained_executable(
                child.child.id(),
                observation.start_ticks,
                &mut observation.executable,
                deadline,
            )?;
            if let Some(expected) = expected_runtime
                && observation.runtime_closure != expected
            {
                return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureMismatch {
                        expected,
                        actual: observation.runtime_closure,
                    },
                ));
            }
            validate_runtime_files(&mut observation.runtime_files, deadline)?;
            let capture = supervise(
                &mut child,
                SupervisionContext {
                    stderr_pipe: stderr,
                    request,
                    challenge,
                    limits,
                    deadline,
                    observation: &mut observation,
                },
            );
            let runtime_result = validate_runtime_files(&mut observation.runtime_files, deadline);
            let image_result = validate_image(&self.image, &self.descriptor_path, self.snapshot);
            let capture = match capture {
                Ok(capture) => capture,
                Err(error) => {
                    let _ = runtime_result;
                    let _ = image_result;
                    return Err(error);
                }
            };
            runtime_result?;
            image_result?;
            if capture.request_written != request.len() {
                return Err(process_error(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::RequestWriteIncomplete,
                    &capture,
                ));
            }
            Ok(RawExecution {
                capture,
                observation,
            })
        }
    }

    pub fn inspect_physical_machine_effect_worker_candidate_v1(
        path: impl AsRef<Path>,
        limits: AuthenticatedPhysicalMachineEffectLimitsV1,
    ) -> Result<PhysicalMachineEffectWorkerCandidateV1, AuthenticatedPhysicalMachineEffectErrorV1>
    {
        let (image, descriptor_path, snapshot, executable) = capture_and_seal(path.as_ref(), None)?;
        let provisional = PhysicalMachineEffectWorkerPolicyV1 {
            executable,
            runtime_closure: PhysicalMachineRuntimeClosureIdentityV1::from_parts([1; 32], 1),
        };
        let worker = AuthenticatedPhysicalMachineEffectWorkerV1 {
            image,
            descriptor_path,
            snapshot,
            policy: provisional,
            analyzer_identity: PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([0; 32]),
            toolchain_identity: PhysicalMachineToolchainIdentityV1::from_sha256_bytes([0; 32]),
        };
        let (analyzer_identity, toolchain_identity, runtime_closure) =
            worker.probe_identities(limits, None)?;
        Ok(PhysicalMachineEffectWorkerCandidateV1 {
            policy: PhysicalMachineEffectWorkerPolicyV1::new(executable, runtime_closure)?,
            analyzer_identity,
            toolchain_identity,
        })
    }

    fn capture_and_seal(
        path: &Path,
        expected: Option<PhysicalMachineWorkerExecutableIdentityV1>,
    ) -> Result<
        (
            File,
            PathBuf,
            Snapshot,
            PhysicalMachineWorkerExecutableIdentityV1,
        ),
        AuthenticatedPhysicalMachineEffectErrorV1,
    > {
        let source_fd = rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::OpenWorker,
                error,
            )
        })?;
        let mut source = File::from(source_fd);
        let initial_metadata = source.metadata().map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::OpenWorker,
                error,
            )
        })?;
        if !initial_metadata.is_file() {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::WorkerNotRegular,
            ));
        }
        let initial = Snapshot::from_metadata(&initial_metadata);
        if initial.mode & 0o111 == 0 {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::WorkerNotExecutable,
            ));
        }
        if initial.size == 0 || initial.size > MAX_PHYSICAL_MACHINE_EFFECT_WORKER_BYTES_V1 {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::InvalidPolicy,
            ));
        }

        let image_fd = rustix::fs::memfd_create(
            "fe2o3-machine-effect-worker",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )
        .map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
                error,
            )
        })?;
        let mut image_writer = File::from(image_fd);
        let mut bytes = 0_u64;
        let mut magic = Vec::new();
        let mut hasher = Sha256::new();
        hasher.update(EXECUTABLE_IDENTITY_DOMAIN);
        let mut buffer = [0_u8; IO_CHUNK_BYTES];
        while bytes < initial.size {
            let needed = usize::try_from((initial.size - bytes).min(IO_CHUNK_BYTES as u64))
                .expect("bounded chunk fits usize");
            let read = read_retry(&mut source, &mut buffer[..needed]).map_err(|error| {
                AuthenticatedPhysicalMachineEffectErrorV1::detail(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::OpenWorker,
                    error,
                )
            })?;
            if read == 0 {
                return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::WorkerChangedDuringCapture,
                ));
            }
            if magic.len() < 4 {
                let take = (4 - magic.len()).min(read);
                magic.extend_from_slice(&buffer[..take]);
            }
            image_writer.write_all(&buffer[..read]).map_err(|error| {
                AuthenticatedPhysicalMachineEffectErrorV1::detail(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
                    error,
                )
            })?;
            hasher.update(&buffer[..read]);
            bytes += read as u64;
        }
        if read_retry(&mut source, &mut buffer[..1]).map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::OpenWorker,
                error,
            )
        })? != 0
            || Snapshot::from_metadata(&source.metadata().map_err(|error| {
                AuthenticatedPhysicalMachineEffectErrorV1::detail(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::OpenWorker,
                    error,
                )
            })?) != initial
        {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::WorkerChangedDuringCapture,
            ));
        }
        if magic != b"\x7fELF" {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::WorkerNotNativeElf,
            ));
        }
        let executable = PhysicalMachineWorkerExecutableIdentityV1 {
            sha256: hasher.finalize().into(),
            byte_len: bytes,
        };
        image_writer.sync_all().map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
                error,
            )
        })?;
        rustix::fs::fchmod(&image_writer, Mode::from_bits_truncate(0o500)).map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
                error,
            )
        })?;
        if let Some(expected) = expected
            && executable != expected
        {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::WorkerIdentityMismatch {
                    expected,
                    actual: executable,
                },
            ));
        }
        rustix::fs::fcntl_add_seals(
            &image_writer,
            SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK,
        )
        .and_then(|()| rustix::fs::fcntl_add_seals(&image_writer, SealFlags::SEAL))
        .map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
                error,
            )
        })?;
        if rustix::fs::fcntl_get_seals(&image_writer).map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
                error,
            )
        })? != REQUIRED_SEALS
        {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
            ));
        }
        let writer_path = PathBuf::from(format!("/proc/self/fd/{}", image_writer.as_raw_fd()));
        let mut image = File::open(&writer_path).map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
                error,
            )
        })?;
        drop(image_writer);
        image.seek(SeekFrom::Start(0)).map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
                error,
            )
        })?;
        let snapshot = Snapshot::from_metadata(&image.metadata().map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
                error,
            )
        })?);
        let descriptor_path = PathBuf::from(format!("/proc/self/fd/{}", image.as_raw_fd()));
        validate_image(&image, &descriptor_path, snapshot)?;
        Ok((image, descriptor_path, snapshot, executable))
    }

    fn validate_image(
        image: &File,
        descriptor_path: &Path,
        expected: Snapshot,
    ) -> Result<(), AuthenticatedPhysicalMachineEffectErrorV1> {
        let seals = rustix::fs::fcntl_get_seals(image).map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
                error,
            )
        })?;
        let status_flags = rustix::fs::fcntl_getfl(image).map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
                error,
            )
        })?;
        let descriptor = Snapshot::from_metadata(&image.metadata().map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
                error,
            )
        })?);
        let procfs =
            Snapshot::from_metadata(&std::fs::metadata(descriptor_path).map_err(|error| {
                AuthenticatedPhysicalMachineEffectErrorV1::detail(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
                    error,
                )
            })?);
        let proc_target = std::fs::read_link(descriptor_path).map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
                error,
            )
        })?;
        let proc_target = proc_target.to_string_lossy();
        if seals != REQUIRED_SEALS
            || status_flags & OFlags::ACCMODE != OFlags::RDONLY
            || descriptor != expected
            || procfs != expected
            || descriptor.mode & 0o777 != 0o500
            || !(proc_target.starts_with("/memfd:fe2o3-machine-effect-worker")
                || proc_target.starts_with("memfd:fe2o3-machine-effect-worker"))
        {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::PreparePinnedImage,
            ));
        }
        Ok(())
    }

    fn await_worker_ready(
        mut stderr: ChildStderr,
        challenge: PhysicalMachineExecutionChallengeV1,
        deadline: Instant,
    ) -> Result<ChildStderr, AuthenticatedPhysicalMachineEffectErrorV1> {
        let expected = control_frame(WORKER_READY_DOMAIN, challenge);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        thread::spawn(move || {
            let mut bytes = vec![0_u8; expected.len()];
            let result = stderr.read_exact(&mut bytes).map(|()| (stderr, bytes));
            let _ = sender.send(result);
        });
        let remaining = deadline.saturating_duration_since(Instant::now());
        let received = receiver.recv_timeout(remaining);
        match received {
            Ok(Ok((stderr, bytes))) if bytes == control_frame(WORKER_READY_DOMAIN, challenge) => {
                Ok(stderr)
            }
            Ok(Err(error)) => Err(AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::ControlHandshake,
                error,
            )),
            Ok(Ok(_)) | Err(_) => Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                if Instant::now() >= deadline {
                    AuthenticatedPhysicalMachineEffectErrorKindV1::Timeout
                } else {
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ControlHandshake
                },
            )),
        }
    }

    fn observe_process(
        pid: u32,
        expected: PhysicalMachineWorkerExecutableIdentityV1,
        deadline: Instant,
    ) -> Result<ProcessObservation, AuthenticatedPhysicalMachineEffectErrorV1> {
        ensure_before_deadline(deadline)?;
        let start_ticks = process_start_ticks(pid)?;
        let executable = retain_process_executable(pid, expected, deadline)?;
        let (runtime_closure, runtime_mappings, runtime_files) =
            runtime_closure(pid, &executable, deadline)?;
        Ok(ProcessObservation {
            process_id: pid,
            start_ticks,
            executable,
            runtime_closure,
            runtime_mappings,
            runtime_files,
        })
    }

    fn retain_process_executable(
        pid: u32,
        expected: PhysicalMachineWorkerExecutableIdentityV1,
        deadline: Instant,
    ) -> Result<RetainedExecutable, AuthenticatedPhysicalMachineEffectErrorV1> {
        ensure_before_deadline(deadline)?;
        let proc_path = format!("/proc/{pid}/exe");
        let link = std::fs::read_link(&proc_path)
            .map_err(observation_io)?
            .into_os_string()
            .into_string()
            .map_err(|_| {
                AuthenticatedPhysicalMachineEffectErrorV1::plain(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
                )
            })?;
        if link.len() > u16::MAX as usize {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
            ));
        }
        let mut file = File::open(&proc_path).map_err(observation_io)?;
        let metadata = file.metadata().map_err(observation_io)?;
        if !metadata.is_file() {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
            ));
        }
        let snapshot = Snapshot::from_metadata(&metadata);
        let key = (
            rustix::fs::major(metadata.dev()),
            rustix::fs::minor(metadata.dev()),
            metadata.ino(),
        );
        let identity = hash_file_identity(
            &mut file,
            EXECUTABLE_IDENTITY_DOMAIN,
            snapshot.size,
            deadline,
        )?;
        if Snapshot::from_metadata(&file.metadata().map_err(observation_io)?) != snapshot {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
            ));
        }
        if identity != expected {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::WorkerIdentityMismatch {
                    expected,
                    actual: identity,
                },
            ));
        }
        Ok(RetainedExecutable {
            file,
            key,
            link,
            snapshot,
            identity,
        })
    }

    fn validate_retained_executable(
        pid: u32,
        start_ticks: u64,
        retained: &mut RetainedExecutable,
        deadline: Instant,
    ) -> Result<(), AuthenticatedPhysicalMachineEffectErrorV1> {
        ensure_before_deadline(deadline)?;
        if process_start_ticks(pid)? != start_ticks {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureChanged,
                "retained executable process identity changed",
            ));
        }
        let retained_snapshot =
            Snapshot::from_metadata(&retained.file.metadata().map_err(observation_io)?);
        let retained_identity = hash_file_identity(
            &mut retained.file,
            EXECUTABLE_IDENTITY_DOMAIN,
            retained.snapshot.size,
            deadline,
        )?;
        if retained_snapshot != retained.snapshot
            || retained_identity != retained.identity
            || writable_alias(&retained.file)
        {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureChanged,
                "retained executable changed or became writable",
            ));
        }
        let current = retain_process_executable(pid, retained.identity, deadline)?;
        if current.key != retained.key
            || current.link != retained.link
            || current.snapshot != retained.snapshot
        {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureChanged,
                "worker executable mapping changed",
            ));
        }
        Ok(())
    }

    #[allow(unsafe_code)]
    fn configure_worker_pre_exec(command: &mut Command) {
        // Command runs this after fork and before exec. The closure performs
        // only direct rustix syscall wrappers and stack-only arithmetic.
        unsafe {
            command.pre_exec(|| {
                setsid().map_err(io::Error::from)?;
                set_no_new_privs(true).map_err(io::Error::from)?;
                for (resource, bound) in [
                    (Resource::As, WORKER_ADDRESS_SPACE_BYTES),
                    (Resource::Data, WORKER_DATA_BYTES),
                    (Resource::Fsize, WORKER_FILE_BYTES),
                    (Resource::Core, 0),
                    (Resource::Nproc, 0),
                ] {
                    let existing = getrlimit(resource);
                    let maximum = existing.maximum.map_or(bound, |value| value.min(bound));
                    let current = existing.current.map_or(maximum, |value| value.min(maximum));
                    setrlimit(
                        resource,
                        Rlimit {
                            current: Some(current),
                            maximum: Some(maximum),
                        },
                    )
                    .map_err(io::Error::from)?;
                }
                Ok(())
            });
        }
    }

    fn validate_no_fork_profile() -> Result<(), AuthenticatedPhysicalMachineEffectErrorV1> {
        validate_security_profile_for_pid("self", false, false)
    }

    fn validate_worker_security_profile(
        pid: u32,
    ) -> Result<(), AuthenticatedPhysicalMachineEffectErrorV1> {
        validate_security_profile_for_pid(&pid.to_string(), true, true)
    }

    fn validate_security_profile_for_pid(
        pid: &str,
        require_no_new_privs: bool,
        require_single_thread: bool,
    ) -> Result<(), AuthenticatedPhysicalMachineEffectErrorV1> {
        let status = read_bounded_utf8(format!("/proc/{pid}/status"), MAX_PROC_STATUS_BYTES)
            .map_err(containment_io)?;
        let uid_map =
            read_bounded_utf8(format!("/proc/{pid}/uid_map"), 4096).map_err(containment_io)?;
        let namespace =
            std::fs::read_link(format!("/proc/{pid}/ns/user")).map_err(containment_io)?;
        let parent_namespace = std::fs::read_link("/proc/self/ns/user").map_err(containment_io)?;
        if namespace != parent_namespace
            || !security_profile_record_is_valid(
                &status,
                &uid_map,
                require_no_new_privs,
                require_single_thread,
            )
        {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::ContainmentUnavailable,
            ));
        }
        Ok(())
    }

    fn security_profile_record_is_valid(
        status: &str,
        uid_map: &str,
        require_no_new_privs: bool,
        require_single_thread: bool,
    ) -> bool {
        let Some(uids) = status
            .lines()
            .find_map(|line| line.strip_prefix("Uid:"))
            .map(|value| {
                value
                    .split_ascii_whitespace()
                    .map(str::parse::<u32>)
                    .collect::<Result<Vec<_>, _>>()
            })
            .and_then(Result::ok)
        else {
            return false;
        };
        if uids.len() != 4 || uids[0] == 0 || !uids.iter().all(|uid| *uid == uids[0]) {
            return false;
        }
        for field in ["CapInh:", "CapPrm:", "CapEff:", "CapAmb:"] {
            let Some(value) = status
                .lines()
                .find_map(|line| line.strip_prefix(field))
                .map(str::trim)
            else {
                return false;
            };
            if u64::from_str_radix(value, 16).ok() != Some(0) {
                return false;
            }
        }
        if require_no_new_privs
            && status
                .lines()
                .find_map(|line| line.strip_prefix("NoNewPrivs:"))
                .map(str::trim)
                != Some("1")
        {
            return false;
        }
        if require_single_thread
            && status
                .lines()
                .find_map(|line| line.strip_prefix("Threads:"))
                .map(str::trim)
                != Some("1")
        {
            return false;
        }
        let mapping = uid_map
            .split_ascii_whitespace()
            .map(str::parse::<u64>)
            .collect::<Result<Vec<_>, _>>();
        mapping.ok().as_deref() == Some(&[0, 0, u32::MAX as u64])
    }

    fn containment_io(error: io::Error) -> AuthenticatedPhysicalMachineEffectErrorV1 {
        AuthenticatedPhysicalMachineEffectErrorV1::detail(
            AuthenticatedPhysicalMachineEffectErrorKindV1::ContainmentUnavailable,
            error,
        )
    }

    fn process_start_ticks(pid: u32) -> Result<u64, AuthenticatedPhysicalMachineEffectErrorV1> {
        let stat = read_bounded_utf8(format!("/proc/{pid}/stat"), MAX_PROC_STAT_BYTES).map_err(
            |error| {
                AuthenticatedPhysicalMachineEffectErrorV1::detail(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
                    error,
                )
            },
        )?;
        let end = stat.rfind(')').ok_or_else(|| {
            AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
            )
        })?;
        stat[end + 1..]
            .split_ascii_whitespace()
            .nth(19)
            .ok_or_else(|| {
                AuthenticatedPhysicalMachineEffectErrorV1::plain(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
                )
            })?
            .parse()
            .map_err(|error| {
                AuthenticatedPhysicalMachineEffectErrorV1::detail(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
                    error,
                )
            })
    }

    fn runtime_closure(
        pid: u32,
        executable: &RetainedExecutable,
        deadline: Instant,
    ) -> Result<
        (
            PhysicalMachineRuntimeClosureIdentityV1,
            PhysicalMachineRuntimeMappingIdentityV1,
            Vec<RuntimeFile>,
        ),
        AuthenticatedPhysicalMachineEffectErrorV1,
    > {
        ensure_before_deadline(deadline)?;
        let maps = read_bounded_utf8(format!("/proc/{pid}/maps"), MAX_PROC_MAPS_BYTES).map_err(
            |error| {
                AuthenticatedPhysicalMachineEffectErrorV1::detail(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
                    error,
                )
            },
        )?;
        let mut files = BTreeMap::<(u32, u32, u64), RuntimeFile>::new();
        let mut mappings = Vec::new();
        let mut total = 0_u64;
        for line in maps.lines() {
            ensure_before_deadline(deadline)?;
            if line.is_empty() || mappings.len() >= MAX_PHYSICAL_MACHINE_EFFECT_RUNTIME_MAPPINGS_V1
            {
                return Err(observation_error());
            }
            let mut fields = line.split_ascii_whitespace();
            let range = fields.next().ok_or_else(observation_error)?;
            let permissions = fields.next().ok_or_else(observation_error)?;
            let offset = fields.next().ok_or_else(observation_error)?;
            let device = fields.next().ok_or_else(observation_error)?;
            let inode = fields.next().ok_or_else(observation_error)?;
            let mut path = String::new();
            for component in fields {
                let separator = usize::from(!path.is_empty());
                if path
                    .len()
                    .checked_add(separator + component.len())
                    .is_none_or(|length| length > u16::MAX as usize)
                {
                    return Err(observation_error());
                }
                if separator != 0 {
                    path.push(' ');
                }
                path.push_str(component);
            }
            let (start, end) = range.split_once('-').ok_or_else(observation_error)?;
            let start = u64::from_str_radix(start, 16).map_err(|_| observation_error())?;
            let end = u64::from_str_radix(end, 16).map_err(|_| observation_error())?;
            if start >= end {
                return Err(observation_error());
            }
            let permissions: [u8; 4] = permissions
                .as_bytes()
                .try_into()
                .map_err(|_| observation_error())?;
            if !matches!(permissions[0], b'r' | b'-')
                || !matches!(permissions[1], b'w' | b'-')
                || !matches!(permissions[2], b'x' | b'-')
                || !matches!(permissions[3], b'p' | b's')
            {
                return Err(observation_error());
            }
            let file_offset = u64::from_str_radix(offset, 16).map_err(|_| observation_error())?;
            let (major, minor) = device.split_once(':').ok_or_else(observation_error)?;
            let major = u32::from_str_radix(major, 16).map_err(|_| observation_error())?;
            let minor = u32::from_str_radix(minor, 16).map_err(|_| observation_error())?;
            let inode = inode.parse::<u64>().map_err(|_| observation_error())?;
            let key = (major, minor, inode);
            let (digest, length) = if inode == 0 {
                if key != (0, 0, 0) {
                    return Err(observation_error());
                }
                if permissions[2] != b'x' {
                    continue;
                }
                if !matches!(path.as_str(), "[vdso]" | "[vsyscall]") {
                    return Err(AuthenticatedPhysicalMachineEffectErrorV1::detail(
                        AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureChanged,
                        "anonymous executable mapping is outside the admitted kernel profile",
                    ));
                }
                ([0; 32], 0)
            } else {
                if !path.starts_with('/') && !path.starts_with("/memfd:") {
                    return Err(observation_error());
                }
                if !files.contains_key(&key) {
                    let mut file = if key == executable.key {
                        executable.file.try_clone().map_err(observation_io)?
                    } else {
                        open_mapped_file(pid, range, &path, key)?
                    };
                    let metadata = file.metadata().map_err(observation_io)?;
                    let snapshot = Snapshot::from_metadata(&metadata);
                    if writable_alias(&file)
                        || files.len() >= MAX_PHYSICAL_MACHINE_EFFECT_RUNTIME_FILES_V1
                        || metadata.len() > MAX_PHYSICAL_MACHINE_EFFECT_WORKER_BYTES_V1
                        || total > MAX_PHYSICAL_MACHINE_EFFECT_RUNTIME_BYTES_V1 - metadata.len()
                    {
                        return Err(observation_error());
                    }
                    let (digest, _, length) =
                        hash_runtime_file(&mut file, metadata.len(), deadline)?;
                    if Snapshot::from_metadata(&file.metadata().map_err(observation_io)?)
                        != snapshot
                    {
                        return Err(observation_error());
                    }
                    total += metadata.len();
                    files.insert(
                        key,
                        RuntimeFile {
                            file,
                            key,
                            path: path.clone(),
                            snapshot,
                            digest,
                            length,
                        },
                    );
                }
                let file = files.get(&key).ok_or_else(observation_error)?;
                if file.path != path {
                    return Err(observation_error());
                }
                (file.digest, file.length)
            };
            mappings.push(RuntimeMapping {
                start,
                end,
                permissions,
                file_offset,
                key,
                path,
                digest,
                length,
            });
        }
        if files.is_empty() {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
            ));
        }
        let mut records = files
            .values()
            .map(|file| (file.path.as_str(), file.digest, file.length))
            .collect::<Vec<_>>();
        records.sort();
        let mut canonical = Vec::new();
        push_u32(&mut canonical, records.len() as u32);
        for (path, digest, length) in records {
            if path.len() > u16::MAX as usize {
                return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
                ));
            }
            push_u16(&mut canonical, path.len() as u16);
            canonical.extend_from_slice(path.as_bytes());
            canonical.extend_from_slice(&digest);
            push_u64(&mut canonical, length);
        }
        let mapping_identity = runtime_mapping_identity(mappings)?;
        Ok((
            PhysicalMachineRuntimeClosureIdentityV1 {
                sha256: domain_hash(RUNTIME_CLOSURE_IDENTITY_DOMAIN, &canonical),
                byte_len: canonical.len() as u64,
            },
            mapping_identity,
            files.into_values().collect(),
        ))
    }

    fn runtime_mapping_identity(
        mut mappings: Vec<RuntimeMapping>,
    ) -> Result<PhysicalMachineRuntimeMappingIdentityV1, AuthenticatedPhysicalMachineEffectErrorV1>
    {
        mappings.sort_by(|left, right| {
            (
                left.start,
                left.end,
                left.permissions,
                left.file_offset,
                left.key,
                left.path.as_str(),
                left.digest,
                left.length,
            )
                .cmp(&(
                    right.start,
                    right.end,
                    right.permissions,
                    right.file_offset,
                    right.key,
                    right.path.as_str(),
                    right.digest,
                    right.length,
                ))
        });
        let canonical_len = mappings.iter().try_fold(4_usize, |total, mapping| {
            total.checked_add(86)?.checked_add(mapping.path.len())
        });
        let Some(canonical_len) =
            canonical_len.filter(|length| *length <= MAX_RUNTIME_MAPPING_CANONICAL_BYTES)
        else {
            return Err(observation_error());
        };
        let mut canonical = Vec::with_capacity(canonical_len);
        push_u32(&mut canonical, mappings.len() as u32);
        for mapping in mappings {
            push_u64(&mut canonical, mapping.start);
            push_u64(&mut canonical, mapping.end);
            canonical.extend_from_slice(&mapping.permissions);
            push_u64(&mut canonical, mapping.file_offset);
            push_u32(&mut canonical, mapping.key.0);
            push_u32(&mut canonical, mapping.key.1);
            push_u64(&mut canonical, mapping.key.2);
            push_u16(&mut canonical, mapping.path.len() as u16);
            canonical.extend_from_slice(mapping.path.as_bytes());
            canonical.extend_from_slice(&mapping.digest);
            push_u64(&mut canonical, mapping.length);
        }
        debug_assert_eq!(canonical.len(), canonical_len);
        Ok(PhysicalMachineRuntimeMappingIdentityV1 {
            sha256: domain_hash(RUNTIME_MAPPING_IDENTITY_DOMAIN, &canonical),
            byte_len: canonical.len() as u64,
        })
    }

    fn runtime_file_set(files: &[RuntimeFile]) -> BTreeSet<RuntimeFileIdentity> {
        files
            .iter()
            .map(|file| (file.key, file.path.clone(), file.digest, file.length))
            .collect()
    }

    fn validate_post_execution_closure(
        pid: u32,
        observation: &mut ProcessObservation,
        deadline: Instant,
    ) -> Result<(), AuthenticatedPhysicalMachineEffectErrorV1> {
        ensure_before_deadline(deadline)?;
        if process_start_ticks(pid)? != observation.start_ticks {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureChanged,
                "worker process identity changed",
            ));
        }
        validate_retained_executable(
            pid,
            observation.start_ticks,
            &mut observation.executable,
            deadline,
        )?;
        let (identity, mappings, mut files) =
            runtime_closure(pid, &observation.executable, deadline)?;
        if identity != observation.runtime_closure
            || mappings != observation.runtime_mappings
            || runtime_file_set(&files) != runtime_file_set(&observation.runtime_files)
        {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureChanged,
                "worker runtime mapping closure changed",
            ));
        }
        validate_runtime_files(&mut files, deadline)?;
        observation.runtime_files.append(&mut files);
        Ok(())
    }

    fn open_mapped_file(
        pid: u32,
        range: &str,
        path: &str,
        expected: (u32, u32, u64),
    ) -> Result<File, AuthenticatedPhysicalMachineEffectErrorV1> {
        let map_path = format!("/proc/{pid}/map_files/{range}");
        let file = File::open(&map_path)
            .or_else(|map_error| {
                if path.ends_with(" (deleted)") {
                    return Err(map_error);
                }
                File::open(format!("/proc/{pid}/root{path}"))
            })
            .map_err(|error| {
                AuthenticatedPhysicalMachineEffectErrorV1::detail(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
                    format!("{map_path}: {error}"),
                )
            })?;
        let metadata = file.metadata().map_err(observation_io)?;
        if rustix::fs::major(metadata.dev()) != expected.0
            || rustix::fs::minor(metadata.dev()) != expected.1
            || metadata.ino() != expected.2
            || !metadata.is_file()
        {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
            ));
        }
        Ok(file)
    }

    fn writable_alias(file: &File) -> bool {
        OpenOptions::new()
            .write(true)
            .open(format!("/proc/self/fd/{}", file.as_raw_fd()))
            .is_ok()
    }

    fn validate_runtime_files(
        files: &mut [RuntimeFile],
        deadline: Instant,
    ) -> Result<(), AuthenticatedPhysicalMachineEffectErrorV1> {
        for runtime in files {
            ensure_before_deadline(deadline)?;
            let before = Snapshot::from_metadata(&runtime.file.metadata().map_err(observation_io)?);
            if before != runtime.snapshot || writable_alias(&runtime.file) {
                return Err(AuthenticatedPhysicalMachineEffectErrorV1::detail(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureChanged,
                    "retained runtime file metadata changed or became writable",
                ));
            }
            let (digest, _, length) =
                hash_runtime_file(&mut runtime.file, runtime.snapshot.size, deadline)?;
            let after = Snapshot::from_metadata(&runtime.file.metadata().map_err(observation_io)?);
            if after != runtime.snapshot || digest != runtime.digest || length != runtime.length {
                return Err(AuthenticatedPhysicalMachineEffectErrorV1::detail(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureChanged,
                    "retained runtime file content changed",
                ));
            }
        }
        Ok(())
    }

    fn hash_runtime_file(
        file: &mut File,
        expected: u64,
        deadline: Instant,
    ) -> Result<([u8; 32], [u8; 32], u64), AuthenticatedPhysicalMachineEffectErrorV1> {
        file.seek(SeekFrom::Start(0)).map_err(observation_io)?;
        let mut plain = Sha256::new();
        let mut executable = Sha256::new();
        executable.update(EXECUTABLE_IDENTITY_DOMAIN);
        let mut total = 0_u64;
        let mut buffer = [0_u8; IO_CHUNK_BYTES];
        loop {
            ensure_before_deadline(deadline)?;
            let read = read_retry(file, &mut buffer).map_err(observation_io)?;
            if read == 0 {
                break;
            }
            total = total.checked_add(read as u64).ok_or_else(|| {
                AuthenticatedPhysicalMachineEffectErrorV1::plain(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
                )
            })?;
            if total > expected {
                return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
                ));
            }
            plain.update(&buffer[..read]);
            executable.update(&buffer[..read]);
        }
        if total != expected {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
            ));
        }
        Ok((plain.finalize().into(), executable.finalize().into(), total))
    }

    fn hash_file_identity(
        file: &mut File,
        domain: &[u8],
        expected: u64,
        deadline: Instant,
    ) -> Result<PhysicalMachineWorkerExecutableIdentityV1, AuthenticatedPhysicalMachineEffectErrorV1>
    {
        file.seek(SeekFrom::Start(0)).map_err(observation_io)?;
        let mut hasher = Sha256::new();
        hasher.update(domain);
        let mut length = 0_u64;
        let mut buffer = [0_u8; IO_CHUNK_BYTES];
        loop {
            ensure_before_deadline(deadline)?;
            let read = read_retry(file, &mut buffer).map_err(observation_io)?;
            if read == 0 {
                break;
            }
            length = length.checked_add(read as u64).ok_or_else(|| {
                AuthenticatedPhysicalMachineEffectErrorV1::plain(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
                )
            })?;
            if length > expected {
                return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
                ));
            }
            hasher.update(&buffer[..read]);
        }
        if length != expected {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
            ));
        }
        Ok(PhysicalMachineWorkerExecutableIdentityV1 {
            sha256: hasher.finalize().into(),
            byte_len: length,
        })
    }

    fn observation_io(error: io::Error) -> AuthenticatedPhysicalMachineEffectErrorV1 {
        AuthenticatedPhysicalMachineEffectErrorV1::detail(
            AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
            error,
        )
    }

    fn observation_error() -> AuthenticatedPhysicalMachineEffectErrorV1 {
        AuthenticatedPhysicalMachineEffectErrorV1::plain(
            AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
        )
    }

    fn ensure_before_deadline(
        deadline: Instant,
    ) -> Result<(), AuthenticatedPhysicalMachineEffectErrorV1> {
        if Instant::now() >= deadline {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::Timeout,
            ));
        }
        Ok(())
    }

    fn supervise(
        child: &mut ChildProcessGuard,
        context: SupervisionContext<'_>,
    ) -> Result<ProcessCapture, AuthenticatedPhysicalMachineEffectErrorV1> {
        let SupervisionContext {
            stderr_pipe,
            request,
            challenge,
            limits,
            deadline,
            observation,
        } = context;
        let stdin = child.child.stdin.take().expect("worker stdin is piped");
        let stdout_pipe = child.child.stdout.take().expect("worker stdout is piped");
        let (write_sender, write_receiver) = std::sync::mpsc::sync_channel(1);
        let request = request.to_vec();
        thread::spawn(move || {
            let mut stdin = stdin;
            let mut written = 0;
            let result = loop {
                match stdin.write(&request[written..]) {
                    Ok(0) => break Ok((stdin, written)),
                    Ok(count) => {
                        written += count;
                        if written == request.len() {
                            break Ok((stdin, written));
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) => break Err(error),
                }
            };
            let _ = write_sender.send(result);
        });
        let stdout_receiver = capture_pipe(stdout_pipe, limits.stdout_bytes);
        let control_receiver = capture_worker_done(stderr_pipe, challenge, limits.stderr_bytes);

        let mut next_descendant_scan = Instant::now();
        let mut timed_out = false;
        let mut request_state = None;
        let mut control_state = None;
        let mut completed_request_written = None;
        let mut acknowledged = false;
        let status = loop {
            if Instant::now() >= next_descendant_scan {
                child.refresh_descendants().map_err(containment_io)?;
                next_descendant_scan = Instant::now() + DESCENDANT_SCAN_INTERVAL;
            }
            if request_state.is_none()
                && let Ok(result) = write_receiver.try_recv()
            {
                request_state = Some(result);
            }
            if control_state.is_none()
                && let Ok(result) = control_receiver.try_recv()
            {
                control_state = Some(result);
            }
            let can_acknowledge = matches!(request_state.as_ref(), Some(Ok(_)))
                && matches!(control_state.as_ref(), Some(Ok(control)) if control.done)
                && !acknowledged;
            if can_acknowledge {
                let (mut stdin, request_written) = request_state
                    .take()
                    .expect("checked request state")
                    .map_err(|error| {
                        AuthenticatedPhysicalMachineEffectErrorV1::detail(
                            AuthenticatedPhysicalMachineEffectErrorKindV1::WriteRequest,
                            error,
                        )
                    })?;
                completed_request_written = Some(request_written);
                validate_post_execution_closure(child.child.id(), observation, deadline)?;
                stdin
                    .write_all(&control_frame(WORKER_ACK_DOMAIN, challenge))
                    .map_err(|error| {
                        AuthenticatedPhysicalMachineEffectErrorV1::detail(
                            AuthenticatedPhysicalMachineEffectErrorKindV1::ControlHandshake,
                            error,
                        )
                    })?;
                drop(stdin);
                acknowledged = true;
            }
            match child.exited_without_reaping() {
                Ok(true) => {
                    break child.terminate_and_wait().map_err(|error| {
                        AuthenticatedPhysicalMachineEffectErrorV1::detail(
                            AuthenticatedPhysicalMachineEffectErrorKindV1::Wait,
                            error,
                        )
                    })?;
                }
                Ok(false) if Instant::now() >= deadline => {
                    timed_out = true;
                    break child.terminate_and_wait().map_err(|error| {
                        AuthenticatedPhysicalMachineEffectErrorV1::detail(
                            AuthenticatedPhysicalMachineEffectErrorKindV1::Wait,
                            error,
                        )
                    })?;
                }
                Ok(false) => thread::sleep(POLL_INTERVAL),
                Err(error) => {
                    return Err(AuthenticatedPhysicalMachineEffectErrorV1::detail(
                        AuthenticatedPhysicalMachineEffectErrorKindV1::Wait,
                        error,
                    ));
                }
            }
        };
        let request_written = match completed_request_written {
            Some(written) => written,
            None => {
                let request_result = match request_state {
                    Some(result) => result,
                    None => write_receiver.recv_timeout(DRAIN_GRACE).map_err(|_| {
                        AuthenticatedPhysicalMachineEffectErrorV1::plain(
                            AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessTreeNotQuiescent,
                        )
                    })?,
                };
                request_result
                    .map(|(_, written)| written)
                    .map_err(|error| {
                        AuthenticatedPhysicalMachineEffectErrorV1::detail(
                            AuthenticatedPhysicalMachineEffectErrorKindV1::WriteRequest,
                            error,
                        )
                    })?
            }
        };
        let control = match control_state {
            Some(result) => result,
            None => control_receiver.recv_timeout(DRAIN_GRACE).map_err(|_| {
                AuthenticatedPhysicalMachineEffectErrorV1::plain(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessTreeNotQuiescent,
                )
            })?,
        }
        .map_err(|error| {
            AuthenticatedPhysicalMachineEffectErrorV1::detail(
                AuthenticatedPhysicalMachineEffectErrorKindV1::ReadStderr,
                error,
            )
        })?;
        let stdout = receive_capture(
            stdout_receiver,
            AuthenticatedPhysicalMachineEffectErrorKindV1::ReadStdout,
        )?;
        let capture = ProcessCapture {
            status,
            request_written,
            stdout,
            stderr: control.stderr,
        };
        if capture.stdout.overflow || capture.stderr.overflow {
            let kind = if capture.stdout.overflow {
                AuthenticatedPhysicalMachineEffectErrorKindV1::StdoutLimitExceeded
            } else {
                AuthenticatedPhysicalMachineEffectErrorKindV1::StderrLimitExceeded
            };
            return Err(process_error(kind, &capture));
        }
        if timed_out {
            return Err(process_error(
                AuthenticatedPhysicalMachineEffectErrorKindV1::Timeout,
                &capture,
            ));
        }
        if !control.done || !acknowledged {
            return Err(process_error(
                AuthenticatedPhysicalMachineEffectErrorKindV1::ControlHandshake,
                &capture,
            ));
        }
        Ok(capture)
    }

    fn capture_worker_done(
        mut stderr: ChildStderr,
        challenge: PhysicalMachineExecutionChallengeV1,
        limit: usize,
    ) -> std::sync::mpsc::Receiver<io::Result<WorkerControlCapture>> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        thread::spawn(move || {
            let expected = control_frame(WORKER_DONE_DOMAIN, challenge);
            let mut capture = Capture::new();
            let mut frame = vec![0_u8; expected.len()];
            let mut frame_bytes = 0;
            let frame_result = loop {
                match stderr.read(&mut frame[frame_bytes..]) {
                    Ok(0) => break Ok(false),
                    Ok(read) => {
                        frame_bytes += read;
                        if frame_bytes == frame.len() {
                            break Ok(frame == expected);
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) => break Err(error),
                }
            };
            let result = match frame_result {
                Ok(true) => Ok(WorkerControlCapture {
                    done: true,
                    stderr: capture,
                }),
                Ok(false) => {
                    capture
                        .bytes
                        .extend_from_slice(&frame[..frame_bytes.min(limit)]);
                    capture.overflow = frame_bytes > limit;
                    let mut buffer = [0_u8; 8192];
                    loop {
                        match stderr.read(&mut buffer) {
                            Ok(0) => break,
                            Ok(read) => {
                                let remaining = limit.saturating_sub(capture.bytes.len());
                                capture
                                    .bytes
                                    .extend_from_slice(&buffer[..read.min(remaining)]);
                                capture.overflow |= read > remaining;
                            }
                            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                            Err(error) => {
                                let _ = sender.send(Err(error));
                                return;
                            }
                        }
                    }
                    Ok(WorkerControlCapture {
                        done: false,
                        stderr: capture,
                    })
                }
                Err(error) => Err(error),
            };
            let _ = sender.send(result);
        });
        receiver
    }

    fn validate_success(
        capture: &ProcessCapture,
    ) -> Result<(), AuthenticatedPhysicalMachineEffectErrorV1> {
        if !capture.status.success() {
            return Err(process_error(
                AuthenticatedPhysicalMachineEffectErrorKindV1::ExitFailure(termination(
                    capture.status,
                )),
                capture,
            ));
        }
        if capture.request_written == 0 {
            return Err(process_error(
                AuthenticatedPhysicalMachineEffectErrorKindV1::RequestWriteIncomplete,
                capture,
            ));
        }
        if !capture.stderr.bytes.is_empty() {
            return Err(process_error(
                AuthenticatedPhysicalMachineEffectErrorKindV1::UnexpectedStderr,
                capture,
            ));
        }
        Ok(())
    }

    fn process_error(
        kind: AuthenticatedPhysicalMachineEffectErrorKindV1,
        capture: &ProcessCapture,
    ) -> AuthenticatedPhysicalMachineEffectErrorV1 {
        AuthenticatedPhysicalMachineEffectErrorV1 {
            kind: Box::new(kind),
            stdout: capture.stdout.bytes.clone(),
            stderr: capture.stderr.bytes.clone(),
            detail: None,
        }
    }

    fn capture_pipe(
        mut reader: impl Read + Send + 'static,
        limit: usize,
    ) -> std::sync::mpsc::Receiver<io::Result<Capture>> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        thread::spawn(move || {
            let mut capture = Capture::new();
            let mut buffer = [0_u8; 8192];
            let result = loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break Ok(capture),
                    Ok(read) => {
                        let remaining = limit.saturating_sub(capture.bytes.len());
                        capture
                            .bytes
                            .extend_from_slice(&buffer[..read.min(remaining)]);
                        capture.overflow |= read > remaining;
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) => break Err(error),
                }
            };
            let _ = sender.send(result);
        });
        receiver
    }

    fn receive_capture(
        receiver: std::sync::mpsc::Receiver<io::Result<Capture>>,
        kind: AuthenticatedPhysicalMachineEffectErrorKindV1,
    ) -> Result<Capture, AuthenticatedPhysicalMachineEffectErrorV1> {
        receiver
            .recv_timeout(DRAIN_GRACE)
            .map_err(|_| {
                AuthenticatedPhysicalMachineEffectErrorV1::plain(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessTreeNotQuiescent,
                )
            })?
            .map_err(|error| AuthenticatedPhysicalMachineEffectErrorV1::detail(kind, error))
    }

    fn checked_pid(raw: u32) -> io::Result<Pid> {
        let raw = i32::try_from(raw)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "PID exceeds i32"))?;
        Pid::from_raw(raw).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "PID is zero"))
    }

    fn child_pid(child: &Child) -> Pid {
        Pid::from_child(child)
    }

    fn validate_session_leader(pid: Pid) -> io::Result<()> {
        if getpgid(Some(pid))? != pid || getsid(Some(pid))? != pid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "worker is not its session and process-group leader",
            ));
        }
        Ok(())
    }

    fn validate_pidfd_identity(pidfd: &OwnedFd, pid: Pid) -> io::Result<()> {
        let record = std::fs::read_to_string(format!("/proc/self/fdinfo/{}", pidfd.as_raw_fd()))?;
        let actual = record
            .lines()
            .find_map(|line| line.strip_prefix("Pid:"))
            .and_then(|value| value.trim().parse::<i32>().ok())
            .and_then(Pid::from_raw)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "pidfd has no live PID"))?;
        if actual != pid {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "pidfd does not identify the spawned worker",
            ));
        }
        Ok(())
    }

    fn retain_descendant_pidfd(root: Pid, pid: Pid) -> io::Result<OwnedFd> {
        let pidfd = pidfd_open(pid, PidfdFlags::empty())?;
        validate_pidfd_identity(&pidfd, pid)?;
        if getsid(Some(pid))? != root {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "observed descendant is not in the worker session",
            ));
        }
        Ok(pidfd)
    }

    fn terminate_without_pidfd(pid: Pid) {
        // `spawn` has not reaped the child. A verified, isolated session is the
        // only safe fallback when the kernel cannot provide the required pidfd.
        if validate_session_leader(pid).is_ok() {
            let _ = kill_process_group(pid, Signal::KILL);
        }
    }

    fn terminate_owned_process(
        child: &mut Child,
        pidfd: &OwnedFd,
        descendant_pidfds: &BTreeMap<u32, OwnedFd>,
    ) {
        // Every exact PID signal is descriptor-bound. Group signaling is used
        // only while the leader is our unreaped child and still owns both the
        // fresh session and its initial process group.
        let _ = pidfd_send_signal(pidfd, Signal::KILL);
        let pid = child_pid(child);
        if validate_session_leader(pid).is_ok() {
            let _ = kill_process_group(pid, Signal::KILL);
        }
        for pidfd in descendant_pidfds.values() {
            let _ = pidfd_send_signal(pidfd, Signal::KILL);
        }
    }

    fn descendants(root: u32) -> BTreeSet<u32> {
        const MAX_DESCENDANTS: usize = 4096;
        let mut found = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(parent) = pending.pop() {
            if found.len() >= MAX_DESCENDANTS {
                break;
            }
            let path = format!("/proc/{parent}/task/{parent}/children");
            let Ok(children) = std::fs::read_to_string(path) else {
                continue;
            };
            for child in children.split_ascii_whitespace() {
                if let Ok(child) = child.parse::<u32>()
                    && child != root
                    && found.insert(child)
                {
                    pending.push(child);
                }
            }
        }
        found
    }

    fn termination(status: ExitStatus) -> AuthenticatedPhysicalMachineEffectTerminationV1 {
        if let Some(code) = status.code() {
            return AuthenticatedPhysicalMachineEffectTerminationV1::Exit(code);
        }
        use std::os::unix::process::ExitStatusExt;
        status.signal().map_or(
            AuthenticatedPhysicalMachineEffectTerminationV1::Unknown,
            AuthenticatedPhysicalMachineEffectTerminationV1::Signal,
        )
    }

    fn fresh_challenge()
    -> Result<PhysicalMachineExecutionChallengeV1, AuthenticatedPhysicalMachineEffectErrorV1> {
        let mut bytes = [0_u8; 32];
        File::open("/dev/urandom")
            .and_then(|mut source| source.read_exact(&mut bytes))
            .map_err(|error| {
                AuthenticatedPhysicalMachineEffectErrorV1::detail(
                    AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
                    error,
                )
            })?;
        if bytes == [0; 32] {
            return Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
                AuthenticatedPhysicalMachineEffectErrorKindV1::ProcessObservation,
            ));
        }
        Ok(PhysicalMachineExecutionChallengeV1::from_sha256_bytes(
            bytes,
        ))
    }

    fn read_retry(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            match reader.read(buffer) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                result => return result,
            }
        }
    }

    fn read_bounded_utf8(path: impl AsRef<Path>, limit: usize) -> io::Result<String> {
        let mut file = File::open(path)?;
        let mut bytes = Vec::with_capacity(limit.min(8192));
        let mut buffer = [0_u8; 8192];
        loop {
            let read = read_retry(&mut file, &mut buffer)?;
            if read == 0 {
                break;
            }
            if read > limit.saturating_sub(bytes.len()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "bounded proc record exceeds byte limit",
                ));
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
        String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    pub(super) fn persist_receipt(
        path: &Path,
        bytes: &[u8],
    ) -> Result<(), AuthenticatedPhysicalMachineEffectErrorV1> {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(persist_error)?;
        file.write_all(bytes).map_err(persist_error)?;
        file.sync_all().map_err(persist_error)?;
        if let Some(parent) = path.parent() {
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(persist_error)?;
        }
        Ok(())
    }

    fn persist_error(error: io::Error) -> AuthenticatedPhysicalMachineEffectErrorV1 {
        AuthenticatedPhysicalMachineEffectErrorV1::detail(
            AuthenticatedPhysicalMachineEffectErrorKindV1::PersistReceipt,
            error,
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        fn spawned_session_child() -> (Child, u32) {
            let mut command = Command::new("/bin/sleep");
            command.arg("30");
            configure_worker_pre_exec(&mut command);
            let child = crate::process_execution::spawn(&mut command).unwrap();
            let pid = child.id();
            (child, pid)
        }

        #[test]
        fn cleanup_wait_is_monotonic_bounded_and_handles_interrupts() {
            let started = Instant::now();
            let mut polls = 0_u32;
            let error = wait_for_exit_until(started + Duration::from_millis(20), || {
                polls += 1;
                Ok(false)
            })
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::TimedOut);
            assert!(started.elapsed() < Duration::from_secs(1));
            assert!((2..100).contains(&polls), "unexpected poll count {polls}");

            let mut interrupted = 0_u32;
            wait_for_exit_until(Instant::now() + Duration::from_secs(1), || {
                interrupted += 1;
                if interrupted < 4 {
                    Err(io::Error::from(io::ErrorKind::Interrupted))
                } else {
                    Ok(true)
                }
            })
            .unwrap();
            assert_eq!(interrupted, 4);

            let mut child_errors = 0_u32;
            let error = wait_for_exit_until(Instant::now() + Duration::from_secs(1), || {
                child_errors += 1;
                Err(io::Error::from(Errno::CHILD))
            })
            .unwrap_err();
            assert_eq!(error.raw_os_error(), Some(Errno::CHILD.raw_os_error()));
            assert_eq!(child_errors, 1);
        }

        #[test]
        fn pidfd_open_failure_preserves_error_and_reaps_session() {
            let (child, pid) = spawned_session_child();
            let error = ChildProcessGuard::new_with_pidfd_open(child, |_| {
                Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "injected pidfd_open failure",
                ))
            })
            .unwrap_err();
            assert_eq!(
                error.kind(),
                &AuthenticatedPhysicalMachineEffectErrorKindV1::ContainmentUnavailable
            );
            assert!(
                error
                    .detail
                    .as_deref()
                    .is_some_and(|detail| detail.contains("injected pidfd_open failure"))
            );
            assert!(!Path::new(&format!("/proc/{pid}")).exists());
        }

        #[test]
        fn pidfd_identity_mismatch_fails_closed_and_reaps_session() {
            let (child, pid) = spawned_session_child();
            let error = ChildProcessGuard::new_with_pidfd_open(child, |_| {
                Ok(pidfd_open(rustix::process::getpid(), PidfdFlags::empty())?)
            })
            .unwrap_err();
            assert_eq!(
                error.kind(),
                &AuthenticatedPhysicalMachineEffectErrorKindV1::ContainmentUnavailable
            );
            assert!(!Path::new(&format!("/proc/{pid}")).exists());
        }

        #[test]
        fn unrelated_process_cannot_be_retained_as_a_descendant() {
            let (child, _) = spawned_session_child();
            let mut worker = ChildProcessGuard::new(child).unwrap();
            let mut command = Command::new("/bin/sleep");
            command.arg("30");
            let mut unrelated = crate::process_execution::spawn(&mut command).unwrap();
            let unrelated_pid = child_pid(&unrelated);
            let error = retain_descendant_pidfd(child_pid(&worker.child), unrelated_pid)
                .expect_err("parent-session process was accepted as a worker descendant");
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            assert!(unrelated.try_wait().unwrap().is_none());
            worker.terminate_and_wait().unwrap();
            assert!(unrelated.try_wait().unwrap().is_none());
            unrelated.kill().unwrap();
            unrelated.wait().unwrap();
        }

        #[test]
        fn retained_runtime_file_mutation_is_detected_after_observation() {
            let path = std::env::temp_dir().join(format!(
                "fe2o3-runtime-closure-rehash-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_file(&path);
            let mut writer = OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .open(&path)
                .unwrap();
            writer.write_all(b"observed-runtime-image").unwrap();
            writer.sync_all().unwrap();
            let mut file = File::open(&path).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
            if writable_alias(&file) {
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
                std::fs::remove_file(path).unwrap();
                return;
            }
            let snapshot = Snapshot::from_metadata(&file.metadata().unwrap());
            let deadline = Instant::now() + Duration::from_secs(30);
            let (digest, _, length) =
                hash_runtime_file(&mut file, snapshot.size, deadline).unwrap();
            let mut retained = [RuntimeFile {
                file,
                key: (1, 2, 3),
                path: path.to_string_lossy().into_owned(),
                snapshot,
                digest,
                length,
            }];
            validate_runtime_files(&mut retained, deadline).unwrap();

            writer.seek(SeekFrom::Start(0)).unwrap();
            writer.write_all(b"mutated!").unwrap();
            writer.sync_all().unwrap();
            let error = validate_runtime_files(&mut retained, deadline).unwrap_err();
            assert_eq!(
                error.kind(),
                &AuthenticatedPhysicalMachineEffectErrorKindV1::RuntimeClosureChanged
            );
            drop(writer);
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            std::fs::remove_file(path).unwrap();
        }

        #[test]
        fn mapping_identity_binds_every_instance_field() {
            let base = RuntimeMapping {
                start: 0x1000,
                end: 0x2000,
                permissions: *b"r-xp",
                file_offset: 0x3000,
                key: (8, 1, 42),
                path: "/runtime.so".to_string(),
                digest: [0x5a; 32],
                length: 8192,
            };
            let identity = runtime_mapping_identity(vec![base.clone()]).unwrap();
            let mut variants = Vec::new();
            let mut changed = base.clone();
            changed.start += 0x1000;
            variants.push(changed);
            let mut changed = base.clone();
            changed.end += 0x1000;
            variants.push(changed);
            let mut changed = base.clone();
            changed.permissions = *b"rw-p";
            variants.push(changed);
            let mut changed = base.clone();
            changed.file_offset += 0x1000;
            variants.push(changed);
            let mut changed = base.clone();
            changed.key.0 += 1;
            variants.push(changed);
            let mut changed = base.clone();
            changed.key.1 += 1;
            variants.push(changed);
            let mut changed = base.clone();
            changed.key.2 += 1;
            variants.push(changed);
            let mut changed = base.clone();
            changed.path.push_str(".changed");
            variants.push(changed);
            let mut changed = base.clone();
            changed.digest[0] ^= 1;
            variants.push(changed);
            let mut changed = base.clone();
            changed.length += 1;
            variants.push(changed);
            for changed in variants {
                assert_ne!(runtime_mapping_identity(vec![changed]).unwrap(), identity);
            }
            assert_ne!(runtime_mapping_identity(Vec::new()).unwrap(), identity);
            let mut added = base.clone();
            added.start = 0x4000;
            added.end = 0x5000;
            assert_ne!(
                runtime_mapping_identity(vec![base, added]).unwrap(),
                identity
            );
        }

        #[test]
        fn security_profile_rejects_root_credentials_capabilities_userns_and_threads() {
            let valid = "Uid:\t1002\t1002\t1002\t1002\n\
                         CapInh:\t0000000000000000\n\
                         CapPrm:\t0000000000000000\n\
                         CapEff:\t0000000000000000\n\
                         CapAmb:\t0000000000000000\n\
                         NoNewPrivs:\t1\n\
                         Threads:\t1\n";
            let initial_map = "0 0 4294967295\n";
            assert!(security_profile_record_is_valid(
                valid,
                initial_map,
                true,
                true
            ));
            assert!(!security_profile_record_is_valid(
                &valid.replace("Uid:\t1002\t1002\t1002\t1002", "Uid:\t0\t0\t0\t0"),
                initial_map,
                true,
                true
            ));
            assert!(!security_profile_record_is_valid(
                &valid.replace(
                    "Uid:\t1002\t1002\t1002\t1002",
                    "Uid:\t1002\t1002\t1003\t1002"
                ),
                initial_map,
                true,
                true
            ));
            assert!(!security_profile_record_is_valid(
                &valid.replace("CapEff:\t0000000000000000", "CapEff:\t1"),
                initial_map,
                true,
                true
            ));
            assert!(!security_profile_record_is_valid(
                &valid.replace("NoNewPrivs:\t1", "NoNewPrivs:\t0"),
                initial_map,
                true,
                true
            ));
            assert!(!security_profile_record_is_valid(
                &valid.replace("Threads:\t1", "Threads:\t2"),
                initial_map,
                true,
                true
            ));
            assert!(!security_profile_record_is_valid(
                valid,
                "0 1002 1\n",
                true,
                true
            ));
        }
    }
}

#[cfg(target_os = "linux")]
pub use platform::{
    AuthenticatedPhysicalMachineEffectWorkerV1, inspect_physical_machine_effect_worker_candidate_v1,
};

#[cfg(target_os = "linux")]
fn persist_receipt(
    path: &Path,
    bytes: &[u8],
) -> Result<(), AuthenticatedPhysicalMachineEffectErrorV1> {
    platform::persist_receipt(path, bytes)
}

#[cfg(not(target_os = "linux"))]
#[derive(Debug)]
pub struct AuthenticatedPhysicalMachineEffectWorkerV1;

#[cfg(not(target_os = "linux"))]
impl AuthenticatedPhysicalMachineEffectWorkerV1 {
    pub fn open(
        _path: impl AsRef<Path>,
        _policy: PhysicalMachineEffectWorkerPolicyV1,
        _limits: AuthenticatedPhysicalMachineEffectLimitsV1,
    ) -> Result<Self, AuthenticatedPhysicalMachineEffectErrorV1> {
        Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
            AuthenticatedPhysicalMachineEffectErrorKindV1::UnsupportedPlatform,
        ))
    }
}

#[cfg(not(target_os = "linux"))]
pub fn inspect_physical_machine_effect_worker_candidate_v1(
    _path: impl AsRef<Path>,
    _limits: AuthenticatedPhysicalMachineEffectLimitsV1,
) -> Result<PhysicalMachineEffectWorkerCandidateV1, AuthenticatedPhysicalMachineEffectErrorV1> {
    Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
        AuthenticatedPhysicalMachineEffectErrorKindV1::UnsupportedPlatform,
    ))
}

#[cfg(not(target_os = "linux"))]
fn persist_receipt(
    _path: &Path,
    _bytes: &[u8],
) -> Result<(), AuthenticatedPhysicalMachineEffectErrorV1> {
    Err(AuthenticatedPhysicalMachineEffectErrorV1::plain(
        AuthenticatedPhysicalMachineEffectErrorKindV1::UnsupportedPlatform,
    ))
}
