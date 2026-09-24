//! Exact native sidecar content, never an execution attestation or launch gate.
use super::super::receipt_transport as shared;
use super::*;
use crate::compiler_execution_subject::native_v2::RETAINED as SUBJECT_STORAGE;
use crate::{CompilerExecutionSubjectErrorV2, InertCompilerExecutionSubjectV2 as Subject};

#[cfg(test)]
#[path = "compiler_execution_receipt_transport_v2_tests.rs"]
mod tests;

pub(super) const ENTRY: &str = "compiler-execution-receipt-v2";
/// Maximum opaque receipt body; the enclosing transport has a separate bound.
pub const MAX_COMPILER_EXECUTION_RECEIPT_TRANSPORT_BYTES_V2: usize = 64 * 1024;
pub const COMPILER_EXECUTION_RECEIPT_TRANSPORT_MAGIC_V2: [u8; 8] = *b"F2O3CRT2";
pub const COMPILER_EXECUTION_RECEIPT_TRANSPORT_VERSION_V2: u16 = 2;
const DOMAIN: &[u8] = b"fe2o3.compiler-execution-receipt-transport.identity.v2\0";
const SUBJECT_START: usize = 24;
const SUBJECT_END: usize = SUBJECT_START + crate::INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V2;
const BODY_START: usize = SUBJECT_END + 8;
const OVERHEAD: usize = BODY_START + 32;
pub const MAX_COMPILER_EXECUTION_RECEIPT_ENVELOPE_BYTES_V2: usize =
    MAX_COMPILER_EXECUTION_RECEIPT_TRANSPORT_BYTES_V2 + OVERHEAD;
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
        let handoff = Schema::decode_payload(record.binding, bytes, resources)?;
        let (actual, storage) = Subject::from_replay_evidence(
            self.attempt(),
            self.slot(),
            self.transaction_identity(),
            &handoff,
            resources.budget()?,
        )
        .map_err(shared::Failure::Subject)?;
        resources.reserve(storage.retained_storage())?;
        resources.work(2 * crate::INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V2 + 64)?;
        if actual.canonical_bytes() != self.canonical_bytes() {
            return Err(shared::Failure::Mismatch);
        }
        Ok(())
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
        let wire = encode(subject, body, r)?;
        let receipt = inspect(&wire, subject, r)?;
        shared::publish(output, producer, subject, &wire, r, hooks)?;
        Ok(receipt)
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
        currentness::metadata(&token.binding, r)?;
        let (actual, storage) =
            Subject::from_publication(lease.receipt(), &token.content, r.budget()?)?;
        r.reserve(storage.retained_storage())?;
        r.work(2 * crate::INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V2 + 64)?;
        if actual.canonical_bytes() != subject.canonical_bytes() {
            return Err(Error::SubjectBindingMismatch);
        }
        let wire = shared::read::<Subject>(&token.binding.slot_directory, r)?
            .ok_or(Error::NotPublished)?;
        let result = recovered(wire, subject, r)?;
        currentness::metadata(&token.binding, r)?;
        Ok(result)
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
    let receipt = inspect(&wire, subject, r)?;
    let headers = size_of::<RecoveredCompilerExecutionReceiptTransportV2>()
        + size_of::<CompilerExecutionReceiptTransportStorageV2>();
    r.reserve(headers)?;
    let storage = CompilerExecutionReceiptTransportStorageV2(
        wire.capacity()
            .checked_add(headers)
            .ok_or(Resource::Arithmetic)?,
    );
    Ok((
        RecoveredCompilerExecutionReceiptTransportV2 { receipt, wire },
        storage,
    ))
}

fn identity(prefix: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((prefix.len() as u64).to_le_bytes());
    hash.update(prefix);
    hash.finalize().into()
}
fn encode(subject: &Subject, body: &[u8], r: &mut Resources<'_, '_>) -> Result<Vec<u8>> {
    let n = OVERHEAD + body.len();
    r.work(4 * n + 4096)?;
    let mut wire = r.buffer(n)?;
    wire.resize(n, 0);
    wire[..8].copy_from_slice(&COMPILER_EXECUTION_RECEIPT_TRANSPORT_MAGIC_V2);
    wire[8..10].copy_from_slice(&COMPILER_EXECUTION_RECEIPT_TRANSPORT_VERSION_V2.to_le_bytes());
    wire[12..20].copy_from_slice(&(n as u64).to_le_bytes());
    wire[SUBJECT_START..SUBJECT_END].copy_from_slice(subject.canonical_bytes());
    wire[SUBJECT_END..BODY_START].copy_from_slice(&(body.len() as u64).to_le_bytes());
    wire[BODY_START..n - 32].copy_from_slice(body);
    let digest = identity(&wire[..n - 32]);
    wire[n - 32..].copy_from_slice(&digest);
    Ok(wire)
}
fn inspect(
    wire: &[u8],
    expected: &Subject,
    r: &mut Resources<'_, '_>,
) -> Result<CompilerExecutionReceiptTransportReceiptV2> {
    let n = wire.len();
    if !(OVERHEAD + 1..=MAX_COMPILER_EXECUTION_RECEIPT_ENVELOPE_BYTES_V2).contains(&n) {
        return Err(Error::InvalidTransport("length"));
    }
    r.work(4 * n + 4096)?;
    let u64_at = |offset| -> Result<u64> {
        Ok(u64::from_le_bytes(
            wire[offset..offset + 8]
                .try_into()
                .map_err(|_| Error::InvalidTransport("truncated"))?,
        ))
    };
    if wire[..8] != COMPILER_EXECUTION_RECEIPT_TRANSPORT_MAGIC_V2
        || wire[8..10] != COMPILER_EXECUTION_RECEIPT_TRANSPORT_VERSION_V2.to_le_bytes()
        || wire[10..12] != [0; 2]
        || wire[20..24] != [0; 4]
        || u64_at(12)? != n as u64
        || u64_at(SUBJECT_END)? != (n - OVERHEAD) as u64
    {
        return Err(Error::InvalidTransport("header"));
    }
    if &wire[SUBJECT_START..SUBJECT_END] != expected.canonical_bytes() {
        return Err(Error::SubjectBindingMismatch);
    }
    let digest = identity(&wire[..n - 32]);
    if wire[n - 32..] != digest {
        return Err(Error::InvalidTransport("identity"));
    }
    Ok(CompilerExecutionReceiptTransportReceiptV2 {
        subject: expected.identity(),
        identity: CompilerExecutionReceiptTransportIdentityV2(digest),
        length: n - OVERHEAD,
    })
}
