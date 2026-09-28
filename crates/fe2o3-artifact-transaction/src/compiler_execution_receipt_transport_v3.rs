//! Conditional V5 sidecar content through the existing journal/currentness engine.
//! No proof callback or mapped owner enters this fixed refundable transport scope.
use super::super::receipt_transport as shared;
use super::*;
use crate::{
    CompilerExecutionSubjectErrorV3, InertCompilerExecutionSubjectStorageV3,
    InertCompilerExecutionSubjectV3 as Subject,
};
// Match the existing subject's returned, additional inline owner/receipt charge
// without changing its API or claiming that its inert data authenticates origin.
const SUBJECT_STORAGE: usize =
    size_of::<Subject>() + size_of::<InertCompilerExecutionSubjectStorageV3>();
type SharedFailure = shared::Failure<CompilerExecutionSubjectErrorV3>;
#[cfg(test)]
use shared::envelope::{SUBJECT_END, SUBJECT_START};
use shared::{
    envelope::{self, BODY_START, OVERHEAD},
    native,
};

#[cfg(test)]
#[path = "compiler_execution_receipt_transport_v3_tests.rs"]
mod tests;

pub(super) const ENTRY: &str = "compiler-execution-receipt-v3";
/// Maximum opaque receipt body; the enclosing transport has a separate bound.
pub const MAX_COMPILER_EXECUTION_RECEIPT_TRANSPORT_BYTES_V3: usize = 64 * 1024;
pub const COMPILER_EXECUTION_RECEIPT_TRANSPORT_MAGIC_V3: [u8; 8] = *b"F2O3CRT3";
pub const COMPILER_EXECUTION_RECEIPT_TRANSPORT_VERSION_V3: u16 = 3;
const DOMAIN: &[u8] = b"fe2o3.compiler-execution-receipt-transport.identity.v3\0";
pub const MAX_COMPILER_EXECUTION_RECEIPT_ENVELOPE_BYTES_V3: usize =
    MAX_COMPILER_EXECUTION_RECEIPT_TRANSPORT_BYTES_V3 + OVERHEAD;
const WIRE: envelope::Schema = envelope::Schema {
    magic: COMPILER_EXECUTION_RECEIPT_TRANSPORT_MAGIC_V3,
    version: COMPILER_EXECUTION_RECEIPT_TRANSPORT_VERSION_V3,
    domain: DOMAIN,
    maximum: MAX_COMPILER_EXECUTION_RECEIPT_ENVELOPE_BYTES_V3,
};
const _: () = assert!(crate::INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V3 == envelope::SUBJECT_BYTES);
const FRAME: usize = 4 * size_of::<RecoveredCompilerExecutionReceiptTransportV3>()
    + 4 * size_of::<Error>()
    + 4 * size_of::<SharedFailure>()
    + 2 * size_of::<Sha256>()
    + 4096;
const _: () = assert!(OVERHEAD == 754);
// Bound logical fixed scratch independently of heap-backed wire capacities.
const _: () = {
    type Output = (
        RecoveredCompilerExecutionReceiptTransportV3,
        CompilerExecutionReceiptTransportStorageV3,
    );
    type Scoped<T> = std::result::Result<Result<T>, CompilerModuleHandoffErrorV5>;
    assert!(
        size_of::<Output>()
            <= size_of::<RecoveredCompilerExecutionReceiptTransportV3>()
                + size_of::<CompilerExecutionReceiptTransportStorageV3>()
    );
    let scalars = 2 * size_of::<shared::Coordinates<Schema>>()
        + 2 * size_of::<HandoffRecord<Schema>>()
        + 8 * size_of::<Vec<u8>>()
        + 2 * size_of::<currentness::PinnedFile>()
        + 4 * size_of::<CompilerExecutionReceiptTransportStorageV3>()
        + size_of::<Result<([u8; 32], usize)>>()
        + size_of::<envelope::Schema>()
        + 64 * size_of::<usize>();
    assert!(scalars + resources::fixed_scope_overhead::<Scoped<Output>>() <= 4096);
    assert!(
        scalars
            + resources::fixed_scope_overhead::<Scoped<CompilerExecutionReceiptTransportReceiptV3>>(
            )
            <= 4096
    );
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionReceiptTransportIdentityV3([u8; 32]);
impl CompilerExecutionReceiptTransportIdentityV3 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert association of exact sidecar bytes with their complete native subject.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionReceiptTransportReceiptV3 {
    subject: crate::InertCompilerExecutionSubjectIdentityV3,
    identity: CompilerExecutionReceiptTransportIdentityV3,
    length: usize,
}
impl CompilerExecutionReceiptTransportReceiptV3 {
    pub const fn subject(&self) -> crate::InertCompilerExecutionSubjectIdentityV3 {
        self.subject
    }
    pub const fn identity(&self) -> CompilerExecutionReceiptTransportIdentityV3 {
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
pub struct CompilerExecutionReceiptTransportStorageV3(usize);
impl CompilerExecutionReceiptTransportStorageV3 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// One move-only wire owner. Higher layers must decode and authenticate the body.
/// ```compile_fail
/// use fe2o3_artifact_transaction::RecoveredCompilerExecutionReceiptTransportV3;
/// fn duplicate(v: RecoveredCompilerExecutionReceiptTransportV3) { let _ = v.clone(); }
/// ```
pub struct RecoveredCompilerExecutionReceiptTransportV3 {
    receipt: CompilerExecutionReceiptTransportReceiptV3,
    wire: Vec<u8>,
}
impl RecoveredCompilerExecutionReceiptTransportV3 {
    pub const fn receipt(&self) -> CompilerExecutionReceiptTransportReceiptV3 {
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
impl fmt::Debug for RecoveredCompilerExecutionReceiptTransportV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecoveredCompilerExecutionReceiptTransportV3")
            .field("receipt", &self.receipt)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum CompilerExecutionReceiptTransportErrorV3 {
    Handoff(CompilerModuleHandoffErrorV5),
    Subject(CompilerExecutionSubjectErrorV3),
    Resource(Resource),
    InvalidReceiptSize { actual: usize, maximum: usize },
    InvalidTransportSize { actual: usize, maximum: usize },
    InvalidTransport(&'static str),
    NotPublished,
    ConflictingPublication,
    SubjectBindingMismatch,
}
type Error = CompilerExecutionReceiptTransportErrorV3;
type Result<T> = std::result::Result<T, Error>;
impl From<CompilerModuleHandoffErrorV5> for Error {
    fn from(e: CompilerModuleHandoffErrorV5) -> Self {
        match e {
            CompilerModuleHandoffErrorV5::Resource(e) => Self::Resource(e),
            e => Self::Handoff(e),
        }
    }
}
impl From<HandoffEngineError> for Error {
    fn from(e: HandoffEngineError) -> Self {
        CompilerModuleHandoffErrorV5::from(e).into()
    }
}
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<CompilerExecutionSubjectErrorV3> for Error {
    fn from(e: CompilerExecutionSubjectErrorV3) -> Self {
        match e {
            CompilerExecutionSubjectErrorV3::Resource(e) => Self::Resource(e),
            e => Self::Subject(e),
        }
    }
}
impl From<SharedFailure> for Error {
    fn from(e: SharedFailure) -> Self {
        match e {
            SharedFailure::Handoff(e) => e.into(),
            SharedFailure::Subject(e) => e.into(),
            SharedFailure::InvalidSize { actual, maximum } => {
                Self::InvalidTransportSize { actual, maximum }
            }
            SharedFailure::NotPublished => Self::NotPublished,
            SharedFailure::Conflict => Self::ConflictingPublication,
            SharedFailure::Mismatch => Self::SubjectBindingMismatch,
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handoff(e) => e.fmt(f),
            Self::Subject(e) => e.fmt(f),
            Self::Resource(e) => e.fmt(f),
            other => write!(
                f,
                "conditional compiler-execution receipt transport: {other:?}"
            ),
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
    type Error = CompilerExecutionSubjectErrorV3;
    type Schema = Schema;
    type Postcheck = shared::PreparedPostcheck<Schema>;
    const ENTRY: &'static str = ENTRY;
    const MAX_BYTES: usize = MAX_COMPILER_EXECUTION_RECEIPT_ENVELOPE_BYTES_V3;
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
    ) -> shared::Result<(), CompilerExecutionSubjectErrorV3> {
        native::validate_payload(self, record, bytes, resources)
    }

    fn prepare_postcheck(
        &self,
        _output: &PinnedOutput,
        producer: &ProducerIdentity,
        slot: &PinnedDirectory,
        resources: &mut Resources<'_, '_>,
    ) -> shared::Result<Self::Postcheck, CompilerExecutionSubjectErrorV3> {
        shared::PreparedPostcheck::new(slot, producer, &self.coordinates(), resources)
    }
    fn postcheck(
        &self,
        output: &PinnedOutput,
        producer: &ProducerIdentity,
        slot: &PinnedDirectory,
        prepared: Self::Postcheck,
    ) -> shared::Result<(), CompilerExecutionSubjectErrorV3> {
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
    ) -> std::result::Result<(Self, usize), CompilerExecutionSubjectErrorV3> {
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
        receipt: CompilerModuleHandoffReceiptV5,
        payload: &Handoff,
        budget: &mut Budget<'_>,
    ) -> std::result::Result<(Self, usize), CompilerExecutionSubjectErrorV3> {
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

/// Publishes opaque bytes only while the exact V5 handoff is ready. Keep the
/// subject and the complete borrowed body's owner prepaid on the same ledger.
/// Identical bytes are idempotent; changed bytes conflict. The body is NOT an
/// authenticated execution receipt here, and does not grant any authority.
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::{InertCompilerExecutionSubjectV2,
///     ProducerIdentity, publish_compiler_execution_receipt_transport_v3};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn ordinary(p: &std::path::Path, producer: &ProducerIdentity,
///     subject: &InertCompilerExecutionSubjectV2,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = publish_compiler_execution_receipt_transport_v3(p, producer, subject, b"body", budget);
/// }
/// ```
pub fn publish_compiler_execution_receipt_transport_v3(
    output: &Path,
    producer: &ProducerIdentity,
    subject: &Subject,
    body: &[u8],
    budget: &mut Budget<'_>,
) -> Result<CompilerExecutionReceiptTransportReceiptV3> {
    publish(output, producer, subject, body, budget, &mut NoFaults)
}
fn publish(
    output: &Path,
    producer: &ProducerIdentity,
    subject: &Subject,
    body: &[u8],
    budget: &mut Budget<'_>,
    hooks: &mut impl HandoffHooks,
) -> Result<CompilerExecutionReceiptTransportReceiptV3> {
    let input = if body.len() <= MAX_COMPILER_EXECUTION_RECEIPT_TRANSPORT_BYTES_V3 {
        body.len()
    } else {
        0
    };
    entry(budget, SUBJECT_STORAGE + input, |r| {
        if body.is_empty() || body.len() > MAX_COMPILER_EXECUTION_RECEIPT_TRANSPORT_BYTES_V3 {
            return Err(Error::InvalidReceiptSize {
                actual: body.len(),
                maximum: MAX_COMPILER_EXECUTION_RECEIPT_TRANSPORT_BYTES_V3,
            });
        }
        let (digest, length) =
            native::publish::<Subject, Error>(output, producer, subject, body, r, hooks)?;
        Ok(receipt(subject, digest, length))
    })
}

/// Recovery accepts exact ready or consumed state, not a new consumption lease.
/// Reserve the returned additional storage before retaining/using its owner.
/// A consumed journal has no payload to reconstruct: recovery compares its exact
/// coordinates and the stored full subject with the expected subject supplied by
/// the caller. It does not restore proof, currentness, compiler or launch authority.
pub fn recover_compiler_execution_receipt_transport_v3(
    output: &Path,
    producer: &ProducerIdentity,
    subject: &Subject,
    budget: &mut Budget<'_>,
) -> Result<(
    RecoveredCompilerExecutionReceiptTransportV3,
    CompilerExecutionReceiptTransportStorageV3,
)> {
    entry(budget, SUBJECT_STORAGE, |r| {
        let wire = shared::recover(output, producer, subject, r)?;
        recovered(wire, subject, r)
    })
}

/// Read under the raw token's retained lock before mapping into a verifier owner.
/// No second payload decode or arbitrary user-provided AsRef callback is invoked.
/// Run this before terminal proof-owner mapping, never around that mapping. The
/// returned owner is inert and its capacity/header charge is returned unreserved.
/// ```compile_fail
/// use fe2o3_artifact_transaction::{CompilerModuleHandoffCurrentnessLeaseV4,
///     CompilerModuleHandoffConsumptionTokenV4, InertCompilerExecutionSubjectV3,
///     recover_compiler_execution_receipt_transport_with_currentness_v3};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn ordinary(lease: &CompilerModuleHandoffCurrentnessLeaseV4,
///     token: &CompilerModuleHandoffConsumptionTokenV4, subject: &InertCompilerExecutionSubjectV3,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = recover_compiler_execution_receipt_transport_with_currentness_v3(
///         lease, token, subject, budget);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_artifact_transaction::{CompilerModuleHandoffCurrentnessLeaseV5,
///     CompilerModuleHandoffConsumptionTokenV5, InertCompilerExecutionSubjectV3,
///     recover_compiler_execution_receipt_transport_with_currentness_v3};
/// use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV5;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn mapped(lease: &CompilerModuleHandoffCurrentnessLeaseV5,
///     token: &CompilerModuleHandoffConsumptionTokenV5<Box<InertSemanticCompilerModuleHandoffV5>>,
///     subject: &InertCompilerExecutionSubjectV3,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = recover_compiler_execution_receipt_transport_with_currentness_v3(
///         lease, token, subject, budget);
/// }
/// ```
pub fn recover_compiler_execution_receipt_transport_with_currentness_v3(
    lease: &CompilerModuleHandoffCurrentnessLeaseV5,
    token: &CompilerModuleHandoffConsumptionTokenV5,
    subject: &Subject,
    budget: &mut Budget<'_>,
) -> Result<(
    RecoveredCompilerExecutionReceiptTransportV3,
    CompilerExecutionReceiptTransportStorageV3,
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
    RecoveredCompilerExecutionReceiptTransportV3,
    CompilerExecutionReceiptTransportStorageV3,
)> {
    let recovered = native::recovered::<Subject, Error>(wire, subject, HEADERS, r)?;
    Ok(finish_recovered(recovered, subject))
}

const HEADERS: usize = size_of::<RecoveredCompilerExecutionReceiptTransportV3>()
    + size_of::<CompilerExecutionReceiptTransportStorageV3>();
const _: () = assert!(size_of::<native::Recovered>() <= HEADERS);

fn finish_recovered(
    recovered: native::Recovered,
    subject: &Subject,
) -> (
    RecoveredCompilerExecutionReceiptTransportV3,
    CompilerExecutionReceiptTransportStorageV3,
) {
    (
        RecoveredCompilerExecutionReceiptTransportV3 {
            receipt: receipt(subject, recovered.digest, recovered.length),
            wire: recovered.wire,
        },
        CompilerExecutionReceiptTransportStorageV3(recovered.storage),
    )
}

fn receipt(
    subject: &Subject,
    digest: [u8; 32],
    length: usize,
) -> CompilerExecutionReceiptTransportReceiptV3 {
    CompilerExecutionReceiptTransportReceiptV3 {
        subject: subject.identity(),
        identity: CompilerExecutionReceiptTransportIdentityV3(digest),
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
) -> Result<CompilerExecutionReceiptTransportReceiptV3> {
    let (digest, length) = WIRE.inspect::<Error>(wire, expected.canonical_bytes(), r)?;
    Ok(receipt(expected, digest, length))
}
