//! Metered native transport through the existing transaction/currentness engine.
//!
//! Payload bytes, decoded metadata, hash scratch and transaction headers use the
//! caller's shared ledger. Filesystem registry/directory metadata retains the
//! existing separate protocol bounds; this is not a total filesystem/RSS meter.
use super::*;
use fe2o3_compiler_ffi::{
    InertSemanticCompilerModuleHandoffIdentityV4 as Identity,
    InertSemanticCompilerModuleHandoffV4 as Handoff,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::mem::size_of;

#[path = "compiler_module_handoff_v4_schema.rs"]
mod schema;
pub use schema::CompilerModuleHandoffErrorV4;
use schema::{Schema, payload_storage};
#[path = "compiler_execution_receipt_transport_v2.rs"]
pub(crate) mod receipt_transport_v2;
type Error = CompilerModuleHandoffErrorV4;
type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
#[path = "compiler_module_handoff_v4_tests.rs"]
pub(crate) mod tests;

pub const MAX_COMPILER_MODULE_HANDOFF_BYTES_V4: usize =
    fe2o3_compiler_ffi::MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V4;
/// Same logical replay cap as native semantic admission, not an enlarged limit.
pub const MAX_COMPILER_MODULE_HANDOFF_STORAGE_V4: usize =
    fe2o3_compiler_ffi::MAX_INERT_REFINED_FORWARDING_STORAGE_V1;

const CURRENT_STORAGE: usize = size_of::<currentness::Current<Schema>>() + 2 * size_of::<usize>();
const FRAME_STORAGE: usize = 4 * schema::RECORD_BYTES
    + size_of::<Sha256>()
    + size_of::<PublishedHandoff<Schema>>()
    + size_of::<Error>()
    + 256;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CompilerModuleHandoffSlotV4 {
    Production = 0,
}

/// Inert, version-domain-separated occurrence identity, never compiler authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerModuleHandoffTransactionIdentityV4([u8; 32]);
impl CompilerModuleHandoffTransactionIdentityV4 {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerModuleHandoffReceiptV4 {
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV4,
    handoff_identity: Identity,
    transaction_identity: CompilerModuleHandoffTransactionIdentityV4,
    length: usize,
}
impl CompilerModuleHandoffReceiptV4 {
    pub const fn attempt(self) -> BuildAttempt {
        self.attempt
    }
    pub const fn slot(self) -> CompilerModuleHandoffSlotV4 {
        self.slot
    }
    pub const fn handoff_identity(self) -> Identity {
        self.handoff_identity
    }
    pub const fn transaction_identity(self) -> CompilerModuleHandoffTransactionIdentityV4 {
        self.transaction_identity
    }
    pub const fn length(self) -> usize {
        self.length
    }
    pub const fn grants_compiler_authority(self) -> bool {
        false
    }
    pub const fn grants_publication_authority(self) -> bool {
        false
    }
}

/// Logical retained payload/metadata/header storage, not an authority receipt.
/// Shared currentness headers are conservatively charged in every retaining
/// owner. Token charges also prepay the consumed representation before commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerModuleHandoffStorageV4(usize);
impl CompilerModuleHandoffStorageV4 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// A move-only exact currentness lease; its returned storage is unreserved.
/// It retains filesystem custody, not compiler, link, load or launch authority.
/// ```compile_fail
/// use fe2o3_artifact_transaction::CompilerModuleHandoffCurrentnessLeaseV4;
/// fn duplicate(v: CompilerModuleHandoffCurrentnessLeaseV4) { let _ = v.clone(); }
/// ```
pub struct CompilerModuleHandoffCurrentnessLeaseV4 {
    binding: Arc<currentness::Current<Schema>>,
}
impl fmt::Debug for CompilerModuleHandoffCurrentnessLeaseV4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CompilerModuleHandoffCurrentnessLeaseV4")
            .field(&self.receipt())
            .finish()
    }
}
impl CompilerModuleHandoffCurrentnessLeaseV4 {
    pub fn receipt(&self) -> CompilerModuleHandoffReceiptV4 {
        self.binding.receipt
    }
    pub const fn storage(&self) -> CompilerModuleHandoffStorageV4 {
        CompilerModuleHandoffStorageV4(CURRENT_STORAGE + size_of::<Self>())
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Decodes exactly once under the retained lock. Reserve the returned
    /// additional storage before retaining or using the token.
    pub fn acquire_current_token(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(
        CompilerModuleHandoffConsumptionTokenV4,
        CompilerModuleHandoffStorageV4,
    )> {
        entry(budget, self.storage().0, |resources| {
            resources.reserve(token_headers::<Handoff>())?;
            let lock = self
                .binding
                .output
                .try_lock()
                .map_err(HandoffEngineError::from)?
                .ok_or(Error::Busy)?;
            let content = currentness::load(&self.binding, resources)?;
            let storage = CompilerModuleHandoffStorageV4(
                token_headers::<Handoff>()
                    .checked_add(payload_storage(&content)?)
                    .ok_or(Resource::Arithmetic)?,
            );
            Ok((
                CompilerModuleHandoffConsumptionTokenV4 {
                    binding: Arc::clone(&self.binding),
                    backing: backing_snapshot(&content),
                    content,
                    storage,
                    _lock: lock,
                },
                storage,
            ))
        })
    }

    /// Revalidates without retaining a decoded owner or resetting shared work.
    pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
        entry(budget, self.storage().0, |resources| {
            let _lock = self
                .binding
                .output
                .try_lock()
                .map_err(HandoffEngineError::from)?
                .ok_or(Error::Busy)?;
            let _handoff = currentness::load(&self.binding, resources)?;
            currentness::metadata(&self.binding, resources)?;
            Ok(())
        })
    }

    pub fn validate_current_token<T>(
        &self,
        token: &CompilerModuleHandoffConsumptionTokenV4<T>,
    ) -> Result<()> {
        if Arc::ptr_eq(&self.binding, &token.binding) {
            Ok(())
        } else {
            Err(Error::MismatchedCurrentnessToken)
        }
    }
}

#[derive(Debug)]
pub struct CompilerModuleHandoffPublicationV4 {
    lease: CompilerModuleHandoffCurrentnessLeaseV4,
}
impl CompilerModuleHandoffPublicationV4 {
    pub fn receipt(&self) -> CompilerModuleHandoffReceiptV4 {
        self.lease.receipt()
    }
    pub fn into_current_lease(self) -> CompilerModuleHandoffCurrentnessLeaseV4 {
        self.lease
    }
    pub fn into_parts(
        self,
    ) -> (
        CompilerModuleHandoffReceiptV4,
        CompilerModuleHandoffCurrentnessLeaseV4,
    ) {
        (self.lease.receipt(), self.lease)
    }
}

/// A single-use locked token. `T` may retain independently admitted evidence;
/// the transaction does not authenticate that evidence or confer authority.
/// ```compile_fail
/// use fe2o3_artifact_transaction::CompilerModuleHandoffConsumptionTokenV4;
/// fn duplicate(v: CompilerModuleHandoffConsumptionTokenV4) { let _ = v.clone(); }
/// ```
pub struct CompilerModuleHandoffConsumptionTokenV4<T = Handoff> {
    binding: Arc<currentness::Current<Schema>>,
    backing: (usize, usize),
    content: T,
    storage: CompilerModuleHandoffStorageV4,
    _lock: crate::OutputLock,
}
impl<T> fmt::Debug for CompilerModuleHandoffConsumptionTokenV4<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CompilerModuleHandoffConsumptionTokenV4")
            .field(&self.binding.receipt)
            .finish()
    }
}
impl<T> CompilerModuleHandoffConsumptionTokenV4<T> {
    pub const fn content(&self) -> &T {
        &self.content
    }
    pub const fn storage(&self) -> CompilerModuleHandoffStorageV4 {
        self.storage
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
    pub fn revalidate_locked_currentness(&self, budget: &mut Budget<'_>) -> Result<()> {
        entry(budget, self.storage.0, |resources| {
            currentness::metadata(&self.binding, resources)?;
            Ok(())
        })
    }
}
impl<T: AsRef<Handoff>> CompilerModuleHandoffConsumptionTokenV4<T> {
    pub fn handoff(&self) -> &Handoff {
        self.content.as_ref()
    }
}

#[derive(Debug)]
pub enum CompilerModuleHandoffAdmissionErrorV4<E> {
    Transaction(Error),
    Admission(E),
}

impl CompilerModuleHandoffConsumptionTokenV4 {
    /// Moves the exact decoded handoff into a higher-layer checker while keeping
    /// the lock. The returned owner must expose the same immutable transport.
    /// This callback is not a verifier or authority gate: production must use
    /// its crate-owned checker and concrete private-construction evidence type.
    ///
    /// The callback must restore its inherited ledger and return only its
    /// additional retained storage, as native semantic recovery does. On success
    /// the original token reservation stays paid; reserve the returned *additional*
    /// storage before using the mapped token. On refusal/unwind the input token
    /// is dropped, no tombstone is written, and its old reservation stays with
    /// the caller to release. Work and denial history are never refunded.
    pub fn try_map_handoff<T: AsRef<Handoff>, E>(
        self,
        budget: &mut Budget<'_>,
        admit: impl FnOnce(Handoff, &mut Budget<'_>) -> std::result::Result<(T, usize), E>,
    ) -> std::result::Result<
        (
            CompilerModuleHandoffConsumptionTokenV4<T>,
            CompilerModuleHandoffStorageV4,
        ),
        CompilerModuleHandoffAdmissionErrorV4<E>,
    > {
        let floor = self.storage.0;
        entry(budget, floor, |resources| {
            let identity = self.content.identity();
            let headers = token_headers::<T>();
            resources.reserve(headers)?;
            let checkpoint = resources.storage();
            let ledger = resources.budget()?.work_ledger_identity_v1();
            let Self {
                binding,
                backing,
                content,
                storage,
                _lock,
            } = self;
            let admitted = admit(content, resources.budget()?);
            if resources.storage() != checkpoint
                || resources.budget()?.work_ledger_identity_v1() != ledger
            {
                return Err(Resource::Accounting.into());
            }
            let (content, additional) = match admitted {
                Ok(value) => value,
                Err(error) => return Ok(Err(error)),
            };
            resources.reserve(additional)?;
            let handoff = content.as_ref();
            if handoff.identity() != identity || backing_snapshot(handoff) != backing {
                return Err(Error::HandoffIdentityMismatch);
            }
            let additional = additional
                .checked_add(headers)
                .ok_or(Resource::Arithmetic)?;
            let total = storage
                .0
                .checked_add(additional)
                .ok_or(Resource::Arithmetic)?;
            currentness::metadata(&binding, resources)?;
            Ok(Ok((
                CompilerModuleHandoffConsumptionTokenV4 {
                    binding,
                    backing,
                    content,
                    storage: CompilerModuleHandoffStorageV4(total),
                    _lock,
                },
                CompilerModuleHandoffStorageV4(additional),
            )))
        })
        .map_err(CompilerModuleHandoffAdmissionErrorV4::Transaction)?
        .map_err(CompilerModuleHandoffAdmissionErrorV4::Admission)
    }
}

/// Consumed content retains its already accepted token reservation. No larger
/// caller reservation is required after the durable one-shot transition.
pub struct ConsumedCompilerModuleHandoffV4<T = Handoff> {
    receipt: CompilerModuleHandoffReceiptV4,
    content: T,
    storage: CompilerModuleHandoffStorageV4,
}
impl<T> fmt::Debug for ConsumedCompilerModuleHandoffV4<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ConsumedCompilerModuleHandoffV4")
            .field(&self.receipt)
            .finish()
    }
}
impl<T> ConsumedCompilerModuleHandoffV4<T> {
    pub const fn receipt(&self) -> CompilerModuleHandoffReceiptV4 {
        self.receipt
    }
    pub const fn content(&self) -> &T {
        &self.content
    }
    pub const fn storage(&self) -> CompilerModuleHandoffStorageV4 {
        self.storage
    }
    /// Transfers the owner without copying payloads; its reservation stays paid.
    pub fn into_content(self) -> T {
        self.content
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}
impl<T: AsRef<Handoff>> ConsumedCompilerModuleHandoffV4<T> {
    pub fn handoff(&self) -> &Handoff {
        self.content.as_ref()
    }
    pub fn bytes(&self) -> &[u8] {
        self.handoff().canonical_bytes()
    }
}

fn token_headers<T>() -> usize {
    CURRENT_STORAGE
        + size_of::<CompilerModuleHandoffConsumptionTokenV4<T>>()
        + size_of::<ConsumedCompilerModuleHandoffV4<T>>()
}

fn backing_snapshot(handoff: &Handoff) -> (usize, usize) {
    (
        handoff.canonical_bytes().as_ptr() as usize,
        handoff.backing_capacity(),
    )
}

fn entry<T>(
    budget: &mut Budget<'_>,
    floor: usize,
    f: impl FnOnce(&mut Resources<'_, '_>) -> Result<T>,
) -> Result<T> {
    let limit = budget.storage_limit();
    Resources::Metered(budget)
        .scoped(|resources| {
            resources.require::<Schema>()?;
            resources.work(8)?;
            if limit > MAX_COMPILER_MODULE_HANDOFF_STORAGE_V4 || resources.storage() < floor {
                return Err(Resource::Accounting.into());
            }
            resources.reserve(FRAME_STORAGE)?;
            Ok(f(resources))
        })
        .map_err(Error::from)?
}

/// Publishes inert bytes only. The caller must keep the whole producer backing
/// capacity and V4 decoded metadata prepaid, including earlier decoder work.
pub fn publish_compiler_module_handoff_v4(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    handoff: &Handoff,
    budget: &mut Budget<'_>,
) -> Result<CompilerModuleHandoffReceiptV4> {
    entry(budget, payload_storage(handoff)?, |resources| {
        // V4 has private-construction identity and immutable, owned decoded bytes.
        // The transaction hash below binds them again to the exact occurrence.
        if usize::try_from(handoff.identity().byte_len()).ok()
            != Some(handoff.canonical_bytes().len())
        {
            return Err(Error::HandoffIdentityMismatch);
        }
        let fields = publish_in_slot_engine::<Schema>(
            output_dir,
            producer,
            attempt,
            CompilerModuleHandoffSlotV4::Production,
            handoff.identity().into(),
            handoff.canonical_bytes(),
            &mut NoFaults,
            resources,
        )?;
        Ok(<Schema as currentness::Schema>::receipt(fields, handoff))
    })
}

pub fn recover_compiler_module_handoff_receipt_v4(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    budget: &mut Budget<'_>,
) -> Result<CompilerModuleHandoffReceiptV4> {
    entry(budget, 0, |resources| {
        Ok(currentness::recover::<Schema>(
            output_dir,
            producer,
            attempt,
            CompilerModuleHandoffSlotV4::Production,
            resources,
        )?)
    })
}

/// The returned lease/header storage is admitted but unreserved; reserve it
/// before keeping or using the lease. Stream hashing never allocates a payload.
pub fn acquire_compiler_module_handoff_currentness_lease_v4(
    output_dir: &Path,
    producer: &ProducerIdentity,
    receipt: CompilerModuleHandoffReceiptV4,
    budget: &mut Budget<'_>,
) -> Result<(
    CompilerModuleHandoffCurrentnessLeaseV4,
    CompilerModuleHandoffStorageV4,
)> {
    entry(budget, 0, |resources| {
        let storage = CompilerModuleHandoffStorageV4(
            CURRENT_STORAGE + size_of::<CompilerModuleHandoffCurrentnessLeaseV4>(),
        );
        resources.reserve(storage.0)?;
        let binding = currentness::mint::<Schema>(output_dir, producer, receipt, resources)?;
        Ok((CompilerModuleHandoffCurrentnessLeaseV4 { binding }, storage))
    })
}

pub fn publish_compiler_module_handoff_with_currentness_v4(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    handoff: &Handoff,
    budget: &mut Budget<'_>,
) -> Result<(
    CompilerModuleHandoffPublicationV4,
    CompilerModuleHandoffStorageV4,
)> {
    let receipt =
        publish_compiler_module_handoff_v4(output_dir, producer, attempt, handoff, budget)?;
    let (lease, storage) = acquire_compiler_module_handoff_currentness_lease_v4(
        output_dir, producer, receipt, budget,
    )?;
    Ok((CompilerModuleHandoffPublicationV4 { lease }, storage))
}

pub fn consume_compiler_module_handoff_with_currentness_v4<T: AsRef<Handoff>>(
    lease: &CompilerModuleHandoffCurrentnessLeaseV4,
    token: CompilerModuleHandoffConsumptionTokenV4<T>,
    budget: &mut Budget<'_>,
) -> Result<ConsumedCompilerModuleHandoffV4<T>> {
    consume(lease, token, budget, &mut NoFaults)
}

fn consume<T: AsRef<Handoff>>(
    lease: &CompilerModuleHandoffCurrentnessLeaseV4,
    token: CompilerModuleHandoffConsumptionTokenV4<T>,
    budget: &mut Budget<'_>,
    hooks: &mut impl HandoffHooks,
) -> Result<ConsumedCompilerModuleHandoffV4<T>> {
    entry(budget, token.storage.0, |resources| {
        lease.validate_current_token(&token)?;
        let handoff = token.content.as_ref();
        if handoff.identity() != token.binding.receipt.handoff_identity
            || backing_snapshot(handoff) != token.backing
        {
            return Err(Error::HandoffIdentityMismatch);
        }
        let CompilerModuleHandoffConsumptionTokenV4 {
            binding,
            backing: _,
            content,
            storage,
            _lock,
        } = token;
        currentness::consume(&binding, resources, hooks)?;
        Ok(ConsumedCompilerModuleHandoffV4 {
            receipt: binding.receipt,
            content,
            storage,
        })
    })
}

impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
