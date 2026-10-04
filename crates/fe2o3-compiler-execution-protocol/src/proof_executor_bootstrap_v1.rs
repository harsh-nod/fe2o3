//! Inert fixed startup/terminal framing, not proof RPC or service approval.
use crate::attestation_resources as resources;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

/// Exact wire length; no optional fields, trailers or variable-sized payload.
pub const PROOF_EXECUTOR_BOOTSTRAP_BYTES_V1: usize = 88;
const MAGIC: &[u8; 16] = b"F2O3PROOFBOOTV1\0";
const RETAINED: usize = size_of::<(
    ProofExecutorBootstrapRecordV1,
    ProofExecutorBootstrapStorageV1,
)>();
/// Fixed logical work per construction, decode or association comparison.
/// This includes entry work; it is not an instruction or wall-time bound.
pub const PROOF_EXECUTOR_BOOTSTRAP_WORK_V1: usize =
    resources::ENTRY_WORK + 32 * PROOF_EXECUTOR_BOOTSTRAP_BYTES_V1;
/// Additional logical scratch, excluding separately prepaid borrowed inputs.
/// This is not an allocator, generated-stack or RSS bound.
pub const PROOF_EXECUTOR_BOOTSTRAP_STORAGE_V1: usize =
    4 * RETAINED + 4 * PROOF_EXECUTOR_BOOTSTRAP_BYTES_V1 + 4096;

/// Closed transport labels. A label does not establish a lifecycle transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ProofExecutorBootstrapKindV1 {
    Initial = 1,
    Ready = 2,
    Finish = 3,
    Finished = 4,
}

/// Additional storage returned UNRESERVED. Reserve this amount on the original
/// budget before retaining or using the record; release it when the owner retires.
#[derive(Debug, Eq, PartialEq)]
pub struct ProofExecutorBootstrapStorageV1(usize);

impl ProofExecutorBootstrapStorageV1 {
    pub const fn additional_storage(&self) -> usize {
        self.0
    }
}

/// Move-only, immutable, authority-free startup/terminal transport content.
///
/// Layout: versioned magic at 0..16, LE u32 kind at 16..20, LE u32 PID at
/// 20..24, opaque session at 24..56, opaque runtime identity at 56..88.
/// PID is 1..=i32::MAX; each opaque identity must contain a nonzero byte.
/// There is no checksum or authentication claim: changed valid fields describe
/// another canonical record and must be checked against independently held state.
///
/// Construction, decode and matching DO NOT approve a service identity, runtime,
/// image, profile, channel, parent, pidfd, namespace, lease, proof or receipt.
/// Actual credentialed sender association, fresh ownership, independently
/// accepted approval/runtime inputs and protocol sequencing remain obligations
/// of the caller. No proof execution is activated by any record kind.
///
/// Keep borrowed owners prepaid on the original caller budget. Fixed operations
/// preserve its entry storage on success, error and unwind without refunding
/// work. Constructor/match value arguments fit inside the fixed scratch quota.
/// Decode needs an 88-byte input floor only for exact-length input; other lengths
/// are rejected after preflight without reading input contents. Accessors borrow
/// an already prepaid owner and do not perform a new protocol operation.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::ProofExecutorBootstrapRecordV1;
/// fn duplicate(record: ProofExecutorBootstrapRecordV1) { let _ = record.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::ProofExecutorBootstrapRecordV1;
/// fn requires_copy<T: Copy>() {}
/// requires_copy::<ProofExecutorBootstrapRecordV1>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::ProofExecutorBootstrapRecordV1;
/// use std::os::fd::AsFd;
/// fn handle(record: &ProofExecutorBootstrapRecordV1) { let _ = record.as_fd(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{ProofExecutorBootstrapRecordV1,
///     CompilerExecutionAttestationReceiptV2};
/// fn promote(record: ProofExecutorBootstrapRecordV1) -> CompilerExecutionAttestationReceiptV2 {
///     record.into()
/// }
/// ```
#[derive(Eq, PartialEq)]
pub struct ProofExecutorBootstrapRecordV1 {
    kind: ProofExecutorBootstrapKindV1,
    pid: u32,
    session: [u8; 32],
    runtime: [u8; 32],
    bytes: [u8; PROOF_EXECUTOR_BOOTSTRAP_BYTES_V1],
}

impl ProofExecutorBootstrapRecordV1 {
    pub fn new(
        kind: ProofExecutorBootstrapKindV1,
        pid: u32,
        session: [u8; 32],
        runtime: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, ProofExecutorBootstrapStorageV1)> {
        metered(budget, 0, || {
            Ok((
                Self::encode(kind, pid, session, runtime)?,
                ProofExecutorBootstrapStorageV1(RETAINED),
            ))
        })
    }

    pub fn decode(
        bytes: &[u8],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, ProofExecutorBootstrapStorageV1)> {
        metered(
            budget,
            resources::fixed_input_floor(bytes, PROOF_EXECUTOR_BOOTSTRAP_BYTES_V1),
            || {
                use ProofExecutorBootstrapFramingErrorV1 as Framing;
                if bytes.len() != PROOF_EXECUTOR_BOOTSTRAP_BYTES_V1 {
                    return Err(Framing::Length.into());
                }
                if &bytes[..16] != MAGIC {
                    return Err(Framing::Magic.into());
                }
                let kind = match u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) {
                    1 => ProofExecutorBootstrapKindV1::Initial,
                    2 => ProofExecutorBootstrapKindV1::Ready,
                    3 => ProofExecutorBootstrapKindV1::Finish,
                    4 => ProofExecutorBootstrapKindV1::Finished,
                    _ => return Err(Framing::Kind.into()),
                };
                let pid = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
                let mut session = [0; 32];
                session.copy_from_slice(&bytes[24..56]);
                let mut runtime = [0; 32];
                runtime.copy_from_slice(&bytes[56..88]);
                let record = Self::encode(kind, pid, session, runtime)?;
                if record.bytes.as_slice() != bytes {
                    return Err(Framing::Canonical.into());
                }
                Ok((record, ProofExecutorBootstrapStorageV1(RETAINED)))
            },
        )
    }

    /// Equality against independently supplied values, not authentication or a
    /// protocol state transition. Both matches and mismatches use the full quota.
    pub fn matches_association(
        &self,
        kind: ProofExecutorBootstrapKindV1,
        pid: u32,
        session: [u8; 32],
        runtime: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<bool> {
        metered(budget, RETAINED, || {
            Ok(self.kind == kind
                && self.pid == pid
                && self.session == session
                && self.runtime == runtime)
        })
    }

    pub const fn kind(&self) -> ProofExecutorBootstrapKindV1 {
        self.kind
    }
    pub const fn pid(&self) -> u32 {
        self.pid
    }
    pub const fn session(&self) -> &[u8; 32] {
        &self.session
    }
    pub const fn runtime_identity(&self) -> &[u8; 32] {
        &self.runtime
    }
    pub const fn canonical_bytes(&self) -> &[u8; PROOF_EXECUTOR_BOOTSTRAP_BYTES_V1] {
        &self.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        RETAINED
    }

    fn encode(
        kind: ProofExecutorBootstrapKindV1,
        pid: u32,
        session: [u8; 32],
        runtime: [u8; 32],
    ) -> Result<Self> {
        use ProofExecutorBootstrapFramingErrorV1 as Framing;
        if pid == 0 || pid > i32::MAX as u32 {
            return Err(Framing::Pid.into());
        }
        if session == [0; 32] {
            return Err(Framing::Session.into());
        }
        if runtime == [0; 32] {
            return Err(Framing::RuntimeIdentity.into());
        }
        let mut bytes = [0; PROOF_EXECUTOR_BOOTSTRAP_BYTES_V1];
        bytes[..16].copy_from_slice(MAGIC);
        bytes[16..20].copy_from_slice(&(kind as u32).to_le_bytes());
        bytes[20..24].copy_from_slice(&pid.to_le_bytes());
        bytes[24..56].copy_from_slice(&session);
        bytes[56..88].copy_from_slice(&runtime);
        Ok(Self {
            kind,
            pid,
            session,
            runtime,
            bytes,
        })
    }
}

impl fmt::Debug for ProofExecutorBootstrapRecordV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProofExecutorBootstrapRecordV1")
            .field("authority", &"none")
            .field("kind", &self.kind)
            .field("pid", &self.pid)
            .finish_non_exhaustive()
    }
}

/// Closed, bounded framing failures, with no copied input bytes or strings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofExecutorBootstrapFramingErrorV1 {
    Length,
    Magic,
    Kind,
    Pid,
    Session,
    RuntimeIdentity,
    Canonical,
}

impl fmt::Display for ProofExecutorBootstrapFramingErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Length => "bootstrap record length",
            Self::Magic => "bootstrap record magic/version",
            Self::Kind => "bootstrap record kind",
            Self::Pid => "bootstrap record PID",
            Self::Session => "bootstrap record session",
            Self::RuntimeIdentity => "bootstrap record runtime identity",
            Self::Canonical => "bootstrap record canonical bytes",
        })
    }
}
impl Error for ProofExecutorBootstrapFramingErrorV1 {}

#[derive(Debug)]
pub enum ProofExecutorBootstrapErrorV1 {
    Framing(ProofExecutorBootstrapFramingErrorV1),
    Resource(Resource),
}
type Result<T> = std::result::Result<T, ProofExecutorBootstrapErrorV1>;

impl From<ProofExecutorBootstrapFramingErrorV1> for ProofExecutorBootstrapErrorV1 {
    fn from(error: ProofExecutorBootstrapFramingErrorV1) -> Self {
        Self::Framing(error)
    }
}
impl From<Resource> for ProofExecutorBootstrapErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for ProofExecutorBootstrapErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framing(error) => error.fmt(f),
            Self::Resource(error) => error.fmt(f),
        }
    }
}
impl Error for ProofExecutorBootstrapErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Framing(error) => error,
            Self::Resource(error) => error,
        })
    }
}

const _: () = {
    assert!(8 * size_of::<ProofExecutorBootstrapErrorV1>() + 64 * size_of::<usize>() <= 4096);
};

fn metered<T>(
    budget: &mut Budget<'_>,
    floor: usize,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    resources::fixed(
        budget,
        floor,
        PROOF_EXECUTOR_BOOTSTRAP_WORK_V1,
        PROOF_EXECUTOR_BOOTSTRAP_STORAGE_V1,
        operation,
    )
}

#[cfg(test)]
#[path = "proof_executor_bootstrap_v1_tests.rs"]
mod tests;
