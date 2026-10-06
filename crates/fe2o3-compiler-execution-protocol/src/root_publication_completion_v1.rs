//! Inert structured publication result. Authenticated original-root custody is separate.
use crate::{
    COMPILER_EXECUTION_ROOT_COMPLETION_BYTES_V1 as TERMINAL_BYTES,
    COMPILER_EXECUTION_ROOT_COMPLETION_STORAGE_V1 as TERMINAL_SCRATCH,
    COMPILER_EXECUTION_ROOT_COMPLETION_WORK_V1 as TERMINAL_WORK,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V3 as MANIFEST_BYTES,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V3 as MANIFEST_SCRATCH,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V3 as MANIFEST_WORK,
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V3 as READY_BYTES,
    COMPILER_EXECUTION_SERVICE_READY_STORAGE_V3 as READY_SCRATCH,
    COMPILER_EXECUTION_SERVICE_READY_WORK_V3 as READY_WORK,
    CompilerExecutionRootCompletionErrorV1 as TerminalError,
    CompilerExecutionRootCompletionRecordV1 as Terminal,
    CompilerExecutionRootIntakeRecordV4 as Intake,
    CompilerExecutionRootTerminationV1 as Termination,
    CompilerExecutionServiceLaunchManifestErrorV3 as ManifestError,
    CompilerExecutionServiceLaunchManifestV3 as Manifest,
    CompilerExecutionServiceReadyErrorV3 as ReadyError, CompilerExecutionServiceReadyV3 as Ready,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
};
use fe2o3_artifact_transaction::{
    CompilerExecutionSubjectErrorV3 as SubjectError,
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V3 as SUBJECT_BYTES,
    INERT_COMPILER_EXECUTION_SUBJECT_STORAGE_V3 as SUBJECT_SCRATCH,
    INERT_COMPILER_EXECUTION_SUBJECT_WORK_V3 as SUBJECT_WORK,
    InertCompilerExecutionSubjectStorageV3 as SubjectStorage,
    InertCompilerExecutionSubjectV3 as Subject,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of};

const HEADER: usize = 16;
const SUBJECT_START: usize = HEADER + TERMINAL_BYTES;
const MANIFEST_START: usize = SUBJECT_START + SUBJECT_BYTES;
const READY_START: usize = MANIFEST_START + MANIFEST_BYTES;
const DIGEST_START: usize = READY_START + READY_BYTES;
pub const COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_BYTES_V1: usize = DIGEST_START + 32;
const N: usize = COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_BYTES_V1;
const MAGIC: &[u8; 8] = b"F2O3CRP1";
const DOMAIN: &[u8] = b"FE2O3/COMPILER-ROOT-PUBLICATION-COMPLETION/V1\0";
const INHERITED: usize = size_of::<(Terminal, Storage)>()
    + size_of::<(Subject, SubjectStorage)>()
    + size_of::<(Manifest, Storage)>()
    + size_of::<(Ready, Storage)>();
const RETAINED: usize = size_of::<(Record, Storage)>();
const LOCAL_WORK: usize = 8 + 64 * N;
const FRAME: usize = 4 * RETAINED + 4 * N + 2 * size_of::<Sha256>() + 4096;
pub const COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_NEW_WORK_V1: usize = LOCAL_WORK;
pub const COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_DECODE_WORK_V1: usize =
    LOCAL_WORK + TERMINAL_WORK + SUBJECT_WORK + MANIFEST_WORK + READY_WORK;
pub const COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_MATCH_WORK_V1: usize = 8 + TERMINAL_WORK;
pub const COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_STORAGE_V1: usize =
    FRAME + INHERITED + TERMINAL_SCRATCH + SUBJECT_SCRATCH + MANIFEST_SCRATCH + READY_SCRATCH;

/// Public inert framing, not a receipt or custody constructor. Even an
/// authenticated root result must be joined to the retained original profile,
/// account, input transcript and output owner. A parent reconstructs Subject
/// from its actual locked V5 publication and requires exact equality before
/// admitting the independently authenticated signed carriage.
///
/// No constructor grants execution, publication, proof, load or launch authority.
#[derive(Debug, Eq, PartialEq)]
pub struct CompilerExecutionRootPublicationCompletionV1 {
    terminal: Terminal,
    subject: Subject,
    manifest: Manifest,
    ready: Ready,
    bytes: [u8; N],
}
use CompilerExecutionRootPublicationCompletionV1 as Record;

impl Record {
    /// Same immutable framing on a larger original owned account; the nested
    /// Subject codec uses its fixed local window without replacing that account.
    pub const COMPOSED_DECODE_WORK: usize =
        COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_DECODE_WORK_V1
            + Subject::COMPOSED_DECODE_WORK
            - SUBJECT_WORK;
    pub const COMPOSED_DECODE_STORAGE: usize =
        COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_STORAGE_V1
            + Subject::COMPOSED_DECODE_SCRATCH
            - SUBJECT_SCRATCH;

    /// Consume four original prepaid inert records. Returns only the additional
    /// charge above their full reservations, which remain charged on refusal.
    pub fn new(
        terminal: Terminal,
        subject: Subject,
        manifest: Manifest,
        ready: Ready,
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        resources::fixed(b, INHERITED, LOCAL_WORK, FRAME, || {
            let value = Self::from_owned(terminal, subject, manifest, ready)?;
            Ok((value, Storage(RETAINED - INHERITED)))
        })
    }

    /// Decode on the same account; full result charge is returned unreserved.
    /// Nested canonical codecs and every retained child remain separately paid.
    pub fn decode(bytes: &[u8], b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Self::decode_using(bytes, b, DecodeAccount::Legacy)
    }

    /// Original-owned-account variant. Does not authenticate the endpoint,
    /// original occurrence, runtime, publication, or nominated proof policy.
    pub fn decode_in_original_account_v1(
        bytes: &[u8],
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::decode_using(bytes, b, DecodeAccount::Original)
    }

    fn decode_using(
        bytes: &[u8],
        b: &mut Budget<'_>,
        account: DecodeAccount,
    ) -> Result<(Self, Storage)> {
        resources::nested_fixed(
            b,
            resources::fixed_input_floor(bytes, N),
            LOCAL_WORK,
            FRAME,
            |b| {
                if bytes.len() != N
                    || &bytes[..8] != MAGIC
                    || bytes[8..12] != 1u32.to_le_bytes()
                    || bytes[12..HEADER] != (N as u32).to_le_bytes()
                {
                    return Err(Error::Framing("root publication completion header"));
                }
                let (terminal, charge) = Terminal::decode(&bytes[HEADER..SUBJECT_START], b)?;
                b.reserve_storage(charge.additional_storage())?;
                let subject_bytes = &bytes[SUBJECT_START..MANIFEST_START];
                let (subject, charge) = match account {
                    DecodeAccount::Legacy => Subject::decode(subject_bytes, b),
                    DecodeAccount::Original => {
                        Subject::decode_in_original_account_v3(subject_bytes, b)
                    }
                }?;
                b.reserve_storage(charge.retained_storage())?;
                let (manifest, charge) = Manifest::decode(&bytes[MANIFEST_START..READY_START], b)?;
                b.reserve_storage(charge.additional_storage())?;
                let (ready, charge) = Ready::decode(&bytes[READY_START..DIGEST_START], b)?;
                b.reserve_storage(charge.additional_storage())?;
                let value = Self::from_owned(terminal, subject, manifest, ready)?;
                if value.bytes != bytes {
                    return Err(Error::Framing("root publication completion digest"));
                }
                Ok((value, Storage(RETAINED)))
            },
        )
    }

    fn from_owned(
        terminal: Terminal,
        subject: Subject,
        manifest: Manifest,
        ready: Ready,
    ) -> Result<Self> {
        if terminal.termination() != Termination::Exited(0)
            || subject.rustc_invocation_sha256() != terminal.invocation_identity()
            || manifest.policy_identity().as_bytes() != terminal.policy_identity()
            || ready.policy_identity() != manifest.policy_identity()
            || ready.launch_manifest_identity() != manifest.identity()
        {
            return Err(Error::Framing("root publication completion associations"));
        }
        let mut bytes = [0; N];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
        bytes[12..HEADER].copy_from_slice(&(N as u32).to_le_bytes());
        bytes[HEADER..SUBJECT_START].copy_from_slice(terminal.canonical_bytes());
        bytes[SUBJECT_START..MANIFEST_START].copy_from_slice(subject.canonical_bytes());
        bytes[MANIFEST_START..READY_START].copy_from_slice(manifest.canonical_bytes());
        bytes[READY_START..DIGEST_START].copy_from_slice(ready.canonical_bytes());
        let digest = digest(&bytes);
        bytes[DIGEST_START..].copy_from_slice(&digest);
        Ok(Self {
            terminal,
            subject,
            manifest,
            ready,
            bytes,
        })
    }

    pub fn matches_intake(&self, last: &Intake, b: &mut Budget<'_>) -> Result<bool> {
        let floor = RETAINED
            .checked_add(last.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, 8, 0, |b| {
            Ok(self.terminal.matches_intake(last, b)?)
        })
    }
    pub const fn terminal(&self) -> &Terminal {
        &self.terminal
    }
    pub const fn subject(&self) -> &Subject {
        &self.subject
    }
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub const fn readiness(&self) -> &Ready {
        &self.ready
    }
    pub const fn canonical_bytes(&self) -> &[u8; N] {
        &self.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        RETAINED
    }
}

fn digest(bytes: &[u8; N]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(DOMAIN);
    h.update(&bytes[..DIGEST_START]);
    h.finalize().into()
}

enum DecodeAccount {
    Legacy,
    Original,
}

#[derive(Debug)]
pub enum CompilerExecutionRootPublicationCompletionErrorV1 {
    Framing(&'static str),
    Resource(Resource),
    Terminal(TerminalError),
    Subject(SubjectError),
    Manifest(ManifestError),
    Ready(ReadyError),
}
use CompilerExecutionRootPublicationCompletionErrorV1 as Error;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
macro_rules! nested_error {
    ($kind:ident, $error:ident) => {
        impl From<$error> for Error {
            fn from(e: $error) -> Self {
                match e {
                    $error::Resource(e) => Self::Resource(e),
                    e => Self::$kind(e),
                }
            }
        }
    };
}
nested_error!(Terminal, TerminalError);
nested_error!(Subject, SubjectError);
nested_error!(Manifest, ManifestError);
nested_error!(Ready, ReadyError);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framing(reason) => f.write_str(reason),
            Self::Resource(e) => e.fmt(f),
            Self::Terminal(e) => e.fmt(f),
            Self::Subject(e) => e.fmt(f),
            Self::Manifest(e) => e.fmt(f),
            Self::Ready(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {}

#[cfg(test)]
#[path = "root_publication_completion_v1_tests.rs"]
mod tests;
