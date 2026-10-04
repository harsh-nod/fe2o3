//! Exact native sidecar content, never an execution attestation or launch gate.
use super::super::receipt_transport as shared;
use super::*;
use crate::compiler_execution_subject::native_v2::RETAINED as SUBJECT_STORAGE;
use crate::{CompilerExecutionSubjectErrorV2, InertCompilerExecutionSubjectV2 as Subject};
#[cfg(test)]
use shared::envelope::{SUBJECT_END, SUBJECT_START};
use shared::{
    envelope::{self, BODY_START, OVERHEAD},
    native,
};

#[cfg(test)]
#[path = "compiler_execution_receipt_transport_v2_tests.rs"]
mod tests;

pub(super) const ENTRY: &str = "compiler-execution-receipt-v2";
/// Maximum opaque receipt body; the enclosing transport has a separate bound.
pub const MAX_COMPILER_EXECUTION_RECEIPT_TRANSPORT_BYTES_V2: usize = 64 * 1024;
pub const COMPILER_EXECUTION_RECEIPT_TRANSPORT_MAGIC_V2: [u8; 8] = *b"F2O3CRT2";
pub const COMPILER_EXECUTION_RECEIPT_TRANSPORT_VERSION_V2: u16 = 2;
const DOMAIN: &[u8] = b"fe2o3.compiler-execution-receipt-transport.identity.v2\0";
pub const MAX_COMPILER_EXECUTION_RECEIPT_ENVELOPE_BYTES_V2: usize =
    MAX_COMPILER_EXECUTION_RECEIPT_TRANSPORT_BYTES_V2 + OVERHEAD;
const WIRE: envelope::Schema = envelope::Schema {
    magic: COMPILER_EXECUTION_RECEIPT_TRANSPORT_MAGIC_V2,
    version: COMPILER_EXECUTION_RECEIPT_TRANSPORT_VERSION_V2,
    domain: DOMAIN,
    maximum: MAX_COMPILER_EXECUTION_RECEIPT_ENVELOPE_BYTES_V2,
};
const _: () = assert!(crate::INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V2 == envelope::SUBJECT_BYTES);
const FRAME: usize = 4 * size_of::<RecoveredCompilerExecutionReceiptTransportV2>()
    + 4 * size_of::<Error>()
    + 4 * size_of::<shared::Failure>()
    + 2 * size_of::<Sha256>()
    + 4096;
const _: () = assert!(OVERHEAD == 754);
// Bound logical fixed scratch independently of heap-backed wire capacities.
const _: () = {
    type Output = (
        RecoveredCompilerExecutionReceiptTransportV2,
        CompilerExecutionReceiptTransportStorageV2,
    );
    type Scoped<T> = std::result::Result<Result<T>, CompilerModuleHandoffErrorV4>;
    assert!(
        size_of::<Output>()
            <= size_of::<RecoveredCompilerExecutionReceiptTransportV2>()
                + size_of::<CompilerExecutionReceiptTransportStorageV2>()
    );
    let scalars = 2 * size_of::<shared::Coordinates<Schema>>()
        + 2 * size_of::<HandoffRecord<Schema>>()
        + 8 * size_of::<Vec<u8>>()
        + 2 * size_of::<currentness::PinnedFile>()
        + 4 * size_of::<CompilerExecutionReceiptTransportStorageV2>()
        + size_of::<Result<([u8; 32], usize)>>()
        + size_of::<envelope::Schema>()
        + 64 * size_of::<usize>();
    assert!(scalars + resources::fixed_scope_overhead::<Scoped<Output>>() <= 4096);
    assert!(
        scalars
            + resources::fixed_scope_overhead::<Scoped<CompilerExecutionReceiptTransportReceiptV2>>(
            )
            <= 4096
    );
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionReceiptTransportIdentityV2([u8; 32]);
impl CompilerExecutionReceiptTransportIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert association of exact sidecar bytes with their complete native subject.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionReceiptTransportReceiptV2 {
    subject: crate::InertCompilerExecutionSubjectIdentityV2,
    identity: CompilerExecutionReceiptTransportIdentityV2,
    length: usize,
}
impl CompilerExecutionReceiptTransportReceiptV2 {
    pub const fn subject(&self) -> crate::InertCompilerExecutionSubjectIdentityV2 {
        self.subject
    }
    pub const fn identity(&self) -> CompilerExecutionReceiptTransportIdentityV2 {
        self.identity
    }
    /// Length of the opaque receipt body, not the enclosing wire.
    pub const fn length(&self) -> usize {
        self.length
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
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

/// Additional admitted retained storage, returned unreserved to the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionReceiptTransportStorageV2(usize);
impl CompilerExecutionReceiptTransportStorageV2 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// One move-only wire owner. Higher layers must decode and authenticate the body.
/// ```compile_fail
/// use fe2o3_artifact_transaction::RecoveredCompilerExecutionReceiptTransportV2;
/// fn duplicate(v: RecoveredCompilerExecutionReceiptTransportV2) { let _ = v.clone(); }
/// ```
pub struct RecoveredCompilerExecutionReceiptTransportV2 {
    receipt: CompilerExecutionReceiptTransportReceiptV2,
    wire: Vec<u8>,
}
impl RecoveredCompilerExecutionReceiptTransportV2 {
    pub const fn receipt(&self) -> CompilerExecutionReceiptTransportReceiptV2 {
        self.receipt
    }
    pub fn exact_bytes(&self) -> &[u8] {
        &self.wire[BODY_START..self.wire.len() - 32]
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
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
impl fmt::Debug for RecoveredCompilerExecutionReceiptTransportV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecoveredCompilerExecutionReceiptTransportV2")
            .field("receipt", &self.receipt)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum CompilerExecutionReceiptTransportErrorV2 {
    Handoff(CompilerModuleHandoffErrorV4),
    Subject(CompilerExecutionSubjectErrorV2),
    Resource(Resource),
    InvalidReceiptSize { actual: usize, maximum: usize },
    InvalidTransportSize { actual: usize, maximum: usize },
    InvalidTransport(&'static str),
    NotPublished,
    ConflictingPublication,
    SubjectBindingMismatch,
}
type Error = CompilerExecutionReceiptTransportErrorV2;
type Result<T> = std::result::Result<T, Error>;
impl From<CompilerModuleHandoffErrorV4> for Error {
    fn from(e: CompilerModuleHandoffErrorV4) -> Self {
        match e {
            CompilerModuleHandoffErrorV4::Resource(e) => Self::Resource(e),
            e => Self::Handoff(e),
        }
    }
}
impl From<HandoffEngineError> for Error {
    fn from(e: HandoffEngineError) -> Self {
        CompilerModuleHandoffErrorV4::from(e).into()
    }
}
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<CompilerExecutionSubjectErrorV2> for Error {
    fn from(e: CompilerExecutionSubjectErrorV2) -> Self {
        match e {
            CompilerExecutionSubjectErrorV2::Resource(e) => Self::Resource(e),
            e => Self::Subject(e),
        }
    }
}
impl From<shared::Failure> for Error {
    fn from(e: shared::Failure) -> Self {
        match e {
            shared::Failure::Handoff(e) => e.into(),
            shared::Failure::Subject(e) => e.into(),
            shared::Failure::InvalidSize { actual, maximum } => {
                Self::InvalidTransportSize { actual, maximum }
            }
            shared::Failure::NotPublished => Self::NotPublished,
            shared::Failure::Conflict => Self::ConflictingPublication,
            shared::Failure::Mismatch => Self::SubjectBindingMismatch,
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handoff(e) => e.fmt(f),
            Self::Subject(e) => e.fmt(f),
            Self::Resource(e) => e.fmt(f),
            other => write!(f, "native compiler-execution receipt transport: {other:?}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Handoff(e) => Some(e),
            Self::Subject(e) => Some(e),
            Self::Resource(e) => Some(e),
            _ => None,
        }
    }
}

impl shared::Subject for Subject {
    type Error = CompilerExecutionSubjectErrorV2;
    type Schema = Schema;
    type Postcheck = shared::PreparedPostcheck<Schema>;
    const ENTRY: &'static str = ENTRY;
    const MAX_BYTES: usize = MAX_COMPILER_EXECUTION_RECEIPT_ENVELOPE_BYTES_V2;
    fn coordinates(&self) -> shared::Coordinates<Schema> {
        shared::Coordinates {
            attempt: self.attempt(),
            slot: self.slot(),
            outer: currentness::Binding {
                sha256: *self.outer_handoff().sha256(),
                byte_len: self.outer_handoff().byte_len(),
            },
            transaction: *self.transaction_identity().as_bytes(),
        }
    }
    fn validate_payload(
        &self,
        record: &HandoffRecord<Schema>,
        bytes: Vec<u8>,
        resources: &mut Resources<'_, '_>,
    ) -> shared::Result<()> {
        native::validate_payload(self, record, bytes, resources)
    }

    fn prepare_postcheck(
        &self,
        _output: &PinnedOutput,
        producer: &ProducerIdentity,
        slot: &PinnedDirectory,
        resources: &mut Resources<'_, '_>,
    ) -> shared::Result<Self::Postcheck> {
        shared::PreparedPostcheck::new(slot, producer, &self.coordinates(), resources)
    }
    fn postcheck(
        &self,
        output: &PinnedOutput,
        producer: &ProducerIdentity,
        slot: &PinnedDirectory,
        prepared: Self::Postcheck,
    ) -> shared::Result<()> {
        prepared.finish(output, producer, self.attempt(), slot)
    }
}

impl native::NativeSubject for Subject {
    const WIRE: envelope::Schema = WIRE;
    fn canonical_bytes(&self) -> &[u8; envelope::SUBJECT_BYTES] {
        self.canonical_bytes()
    }
    fn reconstruct(
        &self,
        payload: &Handoff,
        budget: &mut Budget<'_>,
    ) -> std::result::Result<(Self, usize), CompilerExecutionSubjectErrorV2> {
        let (subject, storage) = Self::from_replay_evidence(
            self.attempt(),
            self.slot(),
            self.transaction_identity(),
            payload,
            budget,
        )?;
        Ok((subject, storage.retained_storage()))
    }
    fn from_receipt(
        receipt: CompilerModuleHandoffReceiptV4,
        payload: &Handoff,
        budget: &mut Budget<'_>,
    ) -> std::result::Result<(Self, usize), CompilerExecutionSubjectErrorV2> {
        let (subject, storage) = Self::from_publication(receipt, payload, budget)?;
        Ok((subject, storage.retained_storage()))
    }
}

fn entry<T>(
    budget: &mut Budget<'_>,
    floor: usize,
    f: impl FnOnce(&mut Resources<'_, '_>) -> Result<T>,
) -> Result<T> {
    super::entry(budget, floor, |r| {
        r.reserve(FRAME)?;
        Ok(f(r))
    })?
}

/// Publishes opaque bytes only while the exact V4 handoff is ready. Keep the
/// subject and the complete borrowed body's owner prepaid on the same ledger.
pub fn publish_compiler_execution_receipt_transport_v2(
    output: &Path,
    producer: &ProducerIdentity,
    subject: &Subject,
    body: &[u8],
    budget: &mut Budget<'_>,
) -> Result<CompilerExecutionReceiptTransportReceiptV2> {
    publish(output, producer, subject, body, budget, &mut NoFaults)
}
fn publish(
    output: &Path,
    producer: &ProducerIdentity,
    subject: &Subject,
    body: &[u8],
    budget: &mut Budget<'_>,
    hooks: &mut impl HandoffHooks,
) -> Result<CompilerExecutionReceiptTransportReceiptV2> {
    let input = if body.len() <= MAX_COMPILER_EXECUTION_RECEIPT_TRANSPORT_BYTES_V2 {
        body.len()
    } else {
        0
    };
    entry(budget, SUBJECT_STORAGE + input, |r| {
        if body.is_empty() || body.len() > MAX_COMPILER_EXECUTION_RECEIPT_TRANSPORT_BYTES_V2 {
            return Err(Error::InvalidReceiptSize {
                actual: body.len(),
                maximum: MAX_COMPILER_EXECUTION_RECEIPT_TRANSPORT_BYTES_V2,
            });
        }
        let (digest, length) =
            native::publish::<Subject, Error>(output, producer, subject, body, r, hooks)?;
        Ok(receipt(subject, digest, length))
    })
}

/// Recovery accepts exact ready or consumed state, not a new consumption lease.
/// Reserve the returned additional storage before retaining/using its owner.
pub fn recover_compiler_execution_receipt_transport_v2(
    output: &Path,
    producer: &ProducerIdentity,
    subject: &Subject,
    budget: &mut Budget<'_>,
) -> Result<(
    RecoveredCompilerExecutionReceiptTransportV2,
    CompilerExecutionReceiptTransportStorageV2,
)> {
    entry(budget, SUBJECT_STORAGE, |r| {
        let wire = shared::recover(output, producer, subject, r)?;
        recovered(wire, subject, r)
    })
}

/// Read under the raw token's retained lock before mapping into a verifier owner.
/// No second payload decode or arbitrary user-provided AsRef callback is invoked.
/// ```compile_fail
/// use fe2o3_artifact_transaction::{CompilerModuleHandoffCurrentnessLeaseV4,
///     CompilerModuleHandoffConsumptionTokenV4, InertCompilerExecutionSubjectV2,
///     recover_compiler_execution_receipt_transport_with_currentness_v2};
/// use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV4;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn mapped(lease: &CompilerModuleHandoffCurrentnessLeaseV4,
///     token: &CompilerModuleHandoffConsumptionTokenV4<Box<InertSemanticCompilerModuleHandoffV4>>,
///     subject: &InertCompilerExecutionSubjectV2,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = recover_compiler_execution_receipt_transport_with_currentness_v2(
///         lease, token, subject, budget);
/// }
/// ```
pub fn recover_compiler_execution_receipt_transport_with_currentness_v2(
    lease: &CompilerModuleHandoffCurrentnessLeaseV4,
    token: &CompilerModuleHandoffConsumptionTokenV4,
    subject: &Subject,
    budget: &mut Budget<'_>,
) -> Result<(
    RecoveredCompilerExecutionReceiptTransportV2,
    CompilerExecutionReceiptTransportStorageV2,
)> {
    let floor = token
        .storage
        .0
        .checked_add(lease.storage().0)
        .and_then(|n| n.checked_add(SUBJECT_STORAGE))
        .ok_or(Resource::Arithmetic)?;
    entry(budget, floor, |r| {
        lease.validate_current_token(token)?;
        let recovered = native::recover_locked::<Subject, Error>(
            &token.binding,
            &token.content,
            subject,
            HEADERS,
            r,
        )?;
        Ok(finish_recovered(recovered, subject))
    })
}

fn recovered(
    wire: Vec<u8>,
    subject: &Subject,
    r: &mut Resources<'_, '_>,
) -> Result<(
    RecoveredCompilerExecutionReceiptTransportV2,
    CompilerExecutionReceiptTransportStorageV2,
)> {
    let recovered = native::recovered::<Subject, Error>(wire, subject, HEADERS, r)?;
    Ok(finish_recovered(recovered, subject))
}

const HEADERS: usize = size_of::<RecoveredCompilerExecutionReceiptTransportV2>()
    + size_of::<CompilerExecutionReceiptTransportStorageV2>();
const _: () = assert!(size_of::<native::Recovered>() <= HEADERS);

fn finish_recovered(
    recovered: native::Recovered,
    subject: &Subject,
) -> (
    RecoveredCompilerExecutionReceiptTransportV2,
    CompilerExecutionReceiptTransportStorageV2,
) {
    (
        RecoveredCompilerExecutionReceiptTransportV2 {
            receipt: receipt(subject, recovered.digest, recovered.length),
            wire: recovered.wire,
        },
        CompilerExecutionReceiptTransportStorageV2(recovered.storage),
    )
}

fn receipt(
    subject: &Subject,
    digest: [u8; 32],
    length: usize,
) -> CompilerExecutionReceiptTransportReceiptV2 {
    CompilerExecutionReceiptTransportReceiptV2 {
        subject: subject.identity(),
        identity: CompilerExecutionReceiptTransportIdentityV2(digest),
        length,
    }
}

impl envelope::Error for Error {
    fn invalid(reason: &'static str) -> Self {
        Self::InvalidTransport(reason)
    }
    fn mismatch() -> Self {
        Self::SubjectBindingMismatch
    }
}

#[cfg(test)]
fn identity(prefix: &[u8]) -> [u8; 32] {
    WIRE.identity(prefix)
}
#[cfg(test)]
fn encode(subject: &Subject, body: &[u8], r: &mut Resources<'_, '_>) -> Result<Vec<u8>> {
    WIRE.encode(subject.canonical_bytes(), body, r)
}
#[cfg(test)]
fn inspect(
    wire: &[u8],
    expected: &Subject,
    r: &mut Resources<'_, '_>,
) -> Result<CompilerExecutionReceiptTransportReceiptV2> {
    let (digest, length) = WIRE.inspect::<Error>(wire, expected.canonical_bytes(), r)?;
    Ok(receipt(expected, digest, length))
}
