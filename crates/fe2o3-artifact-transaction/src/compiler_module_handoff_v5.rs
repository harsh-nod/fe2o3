//! Conditional compiler-transport staging over the existing journal engine.
//!
//! Publication records byte custody, never artifact, compiler or launch authority.
//! Conditional admission is terminal on failure; no ordinary refund envelope may
//! enclose recovery. Payload/decoder accounting uses the caller's original ledger.
use super::*;
use fe2o3_compiler_ffi::{
    InertSemanticCompilerModuleHandoffIdentityV5 as Identity,
    InertSemanticCompilerModuleHandoffV5 as Handoff,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::mem::size_of;

#[path = "compiler_module_handoff_v5_schema.rs"]
mod schema;
pub use schema::CompilerModuleHandoffErrorV5;
use schema::{Schema, payload_storage};
#[path = "compiler_module_handoff_v5_composition.rs"]
mod composition;
pub use composition::{
    COMPILER_MODULE_HANDOFF_CUSTODY_COMPOSITION_SCRATCH_V5,
    COMPILER_MODULE_HANDOFF_CUSTODY_COMPOSITION_WORK_V5, CompilerModuleHandoffCustodyResourcesV5,
    CompilerModuleHandoffCustodyScopeV5,
    try_recover_compiler_module_handoff_receipt_in_root_budget_v5,
};
#[path = "compiler_module_handoff_v5_custody.rs"]
mod custody;
pub use custody::{
    CompilerModuleHandoffCurrentnessCustodyQuoteV5, CompilerModuleHandoffCustodyQuotaV5,
    CompilerModuleHandoffOperationQuotaV5,
    acquire_compiler_module_handoff_currentness_lease_with_quote_v5,
    compiler_module_handoff_custody_quota_for_limit_v5,
    compiler_module_handoff_try_recovery_quota_v5,
    quote_compiler_module_handoff_currentness_custody_v5,
    try_recover_compiler_module_handoff_receipt_v5,
    try_recover_compiler_module_handoff_receipt_with_limit_v5,
};
#[path = "compiler_module_handoff_v5_admission.rs"]
mod admission;
#[path = "compiler_execution_receipt_transport_v3.rs"]
pub(crate) mod receipt_transport_v3;
use admission::consume;
pub use admission::{CompilerModuleHandoffAdmissionCauseV5, CompilerModuleHandoffAdmissionErrorV5};
type Error = CompilerModuleHandoffErrorV5;
type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
#[path = "compiler_module_handoff_v5_replay_tests.rs"]
mod replay_tests;
#[cfg(test)]
#[path = "compiler_module_handoff_v5_tests.rs"]
pub(crate) mod tests;

pub const MAX_COMPILER_MODULE_HANDOFF_BYTES_V5: usize =
    fe2o3_compiler_ffi::MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V5;
/// Same logical replay cap as native semantic admission, not an enlarged limit.
pub const MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5: usize =
    fe2o3_compiler_ffi::MAX_INERT_REFINED_FORWARDING_STORAGE_V1;

const CURRENT_STORAGE: usize = size_of::<currentness::Current<Schema>>() + 2 * size_of::<usize>();
const FRAME_STORAGE: usize = 4 * schema::RECORD_BYTES
    + size_of::<Sha256>()
    + size_of::<PublishedHandoff<Schema>>()
    + size_of::<Error>()
    + 256;
const REPLAY_SCRATCH_STORAGE: usize = currentness::replay_scratch_storage::<Schema, Error>();
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CompilerModuleHandoffSlotV5 {
    Production = 0,
}

/// Inert, version-domain-separated occurrence identity, never compiler authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerModuleHandoffTransactionIdentityV5([u8; 32]);
impl CompilerModuleHandoffTransactionIdentityV5 {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert occurrence/content coordinates, not proof of publication or currentness.
/// A checked replay can rederive this value without observing the filesystem.
/// Only the separate lease/token APIs establish current filesystem custody;
/// neither this receipt nor those APIs authenticate compiler execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerModuleHandoffReceiptV5 {
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV5,
    handoff_identity: Identity,
    transaction_identity: CompilerModuleHandoffTransactionIdentityV5,
    length: usize,
}
impl CompilerModuleHandoffReceiptV5 {
    pub const fn attempt(self) -> BuildAttempt {
        self.attempt
    }
    pub const fn slot(self) -> CompilerModuleHandoffSlotV5 {
        self.slot
    }
    pub const fn handoff_identity(self) -> Identity {
        self.handoff_identity
    }
    pub const fn transaction_identity(self) -> CompilerModuleHandoffTransactionIdentityV5 {
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
pub struct CompilerModuleHandoffStorageV5(usize);
impl CompilerModuleHandoffStorageV5 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// A move-only exact currentness lease; its returned storage is unreserved.
/// It retains filesystem custody, not compiler, link, load or launch authority.
/// ```compile_fail
/// use fe2o3_artifact_transaction::CompilerModuleHandoffCurrentnessLeaseV5;
/// fn duplicate(v: CompilerModuleHandoffCurrentnessLeaseV5) { let _ = v.clone(); }
/// ```
pub struct CompilerModuleHandoffCurrentnessLeaseV5 {
    binding: Arc<currentness::Current<Schema>>,
    storage: CompilerModuleHandoffStorageV5,
}
impl fmt::Debug for CompilerModuleHandoffCurrentnessLeaseV5 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CompilerModuleHandoffCurrentnessLeaseV5")
            .field(&self.receipt())
            .finish()
    }
}
impl CompilerModuleHandoffCurrentnessLeaseV5 {
    pub fn receipt(&self) -> CompilerModuleHandoffReceiptV5 {
        self.binding.receipt
    }
    pub const fn storage(&self) -> CompilerModuleHandoffStorageV5 {
        self.storage
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
        CompilerModuleHandoffConsumptionTokenV5,
        CompilerModuleHandoffStorageV5,
    )> {
        entry(budget, self.storage().0, |resources| {
            let headers = token_headers::<Handoff>()
                .checked_add(
                    self.storage
                        .0
                        .checked_sub(CURRENT_STORAGE + size_of::<Self>())
                        .ok_or(Resource::Accounting)?,
                )
                .ok_or(Resource::Arithmetic)?;
            resources.reserve(headers)?;
            let lock = self
                .binding
                .output
                .try_lock()
                .map_err(HandoffEngineError::from)?
                .ok_or(Error::Busy)?;
            let content = currentness::load(&self.binding, resources)?;
            let storage = CompilerModuleHandoffStorageV5(
                headers
                    .checked_add(payload_storage(&content)?)
                    .ok_or(Resource::Arithmetic)?,
            );
            Ok((
                CompilerModuleHandoffConsumptionTokenV5 {
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
            currentness::stream(&self.binding, resources)?;
            Ok(())
        })
    }

    pub fn validate_current_token<T>(
        &self,
        token: &CompilerModuleHandoffConsumptionTokenV5<T>,
    ) -> Result<()> {
        if Arc::ptr_eq(&self.binding, &token.binding) {
            Ok(())
        } else {
            Err(Error::MismatchedCurrentnessToken)
        }
    }
}

#[derive(Debug)]
pub struct CompilerModuleHandoffPublicationV5 {
    lease: CompilerModuleHandoffCurrentnessLeaseV5,
}
impl CompilerModuleHandoffPublicationV5 {
    pub fn receipt(&self) -> CompilerModuleHandoffReceiptV5 {
        self.lease.receipt()
    }
    pub fn into_current_lease(self) -> CompilerModuleHandoffCurrentnessLeaseV5 {
        self.lease
    }
    pub fn into_parts(
        self,
    ) -> (
        CompilerModuleHandoffReceiptV5,
        CompilerModuleHandoffCurrentnessLeaseV5,
    ) {
        (self.lease.receipt(), self.lease)
    }
}

/// A single-use locked token. `T` may retain independently admitted evidence;
/// the transaction does not authenticate that evidence or confer authority.
/// ```compile_fail
/// use fe2o3_artifact_transaction::CompilerModuleHandoffConsumptionTokenV5;
/// fn duplicate(v: CompilerModuleHandoffConsumptionTokenV5) { let _ = v.clone(); }
/// ```
pub struct CompilerModuleHandoffConsumptionTokenV5<T = Handoff> {
    binding: Arc<currentness::Current<Schema>>,
    backing: (usize, usize, usize),
    content: T,
    storage: CompilerModuleHandoffStorageV5,
    _lock: crate::OutputLock,
}
impl<T> fmt::Debug for CompilerModuleHandoffConsumptionTokenV5<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CompilerModuleHandoffConsumptionTokenV5")
            .field(&self.binding.receipt)
            .finish()
    }
}
impl<T> CompilerModuleHandoffConsumptionTokenV5<T> {
    /// Exact inert receipt retained by this locked occurrence. Reading it does
    /// not consume the token or authenticate compiler execution.
    pub fn receipt(&self) -> CompilerModuleHandoffReceiptV5 {
        self.binding.receipt
    }

    pub const fn content(&self) -> &T {
        &self.content
    }
    pub const fn storage(&self) -> CompilerModuleHandoffStorageV5 {
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
            self.revalidate_locked_in(resources)
        })
    }

    /// Same locked metadata revalidation on an original owned account. Counts
    /// this complete actual token (including any mapped owner) again under an
    /// additional <=256 MiB window. This does not authenticate mapped evidence.
    pub fn revalidate_locked_currentness_in_original_account_v5(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        let floor = budget.storage();
        let overlap = self
            .storage
            .0
            .checked_add(Budget::STORAGE_WINDOW_SCRATCH_V1)
            .ok_or(Resource::Arithmetic)?;
        budget.with_additional_storage_window_v1(MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5, |b| {
            b.with_prepaid_scope(floor.max(self.storage.0), 0, 0, overlap, |b| {
                composition::entry_composed(b, self.storage.0, |r| self.revalidate_locked_in(r))
            })
        })
    }

    fn revalidate_locked_in(&self, resources: &mut Resources<'_, '_>) -> Result<()> {
        custody::prepay_currentness(&self.binding, resources)?;
        currentness::metadata(&self.binding, resources)?;
        Ok(())
    }
}
impl<T: AsRef<Handoff>> CompilerModuleHandoffConsumptionTokenV5<T> {
    pub fn handoff(&self) -> &Handoff {
        self.content.as_ref()
    }
}

/// Consumed content retains its already accepted token reservation. No larger
/// caller reservation is required after the durable one-shot transition.
pub struct ConsumedCompilerModuleHandoffV5<T = Handoff> {
    receipt: CompilerModuleHandoffReceiptV5,
    content: T,
    storage: CompilerModuleHandoffStorageV5,
}
impl<T> fmt::Debug for ConsumedCompilerModuleHandoffV5<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ConsumedCompilerModuleHandoffV5")
            .field(&self.receipt)
            .finish()
    }
}
impl<T> ConsumedCompilerModuleHandoffV5<T> {
    pub const fn receipt(&self) -> CompilerModuleHandoffReceiptV5 {
        self.receipt
    }
    pub const fn content(&self) -> &T {
        &self.content
    }
    pub const fn storage(&self) -> CompilerModuleHandoffStorageV5 {
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
impl<T: AsRef<Handoff>> ConsumedCompilerModuleHandoffV5<T> {
    pub fn handoff(&self) -> &Handoff {
        self.content.as_ref()
    }
    pub fn bytes(&self) -> &[u8] {
        self.handoff().canonical_bytes()
    }
}

fn token_headers<T>() -> usize {
    CURRENT_STORAGE
        + size_of::<CompilerModuleHandoffConsumptionTokenV5<T>>()
        + size_of::<ConsumedCompilerModuleHandoffV5<T>>()
}

fn backing_snapshot(handoff: &Handoff) -> (usize, usize, usize) {
    (
        handoff.canonical_bytes().as_ptr() as usize,
        handoff.canonical_bytes().len(),
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
            if limit > MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5 || resources.storage() < floor {
                return Err(Resource::Accounting.into());
            }
            resources.reserve(FRAME_STORAGE)?;
            Ok(f(resources))
        })
        .map_err(Error::from)?
}

/// Publishes inert bytes only. The caller must keep the whole producer backing
/// capacity and V5 decoded metadata prepaid, including earlier decoder work.
pub fn publish_compiler_module_handoff_v5(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    handoff: &Handoff,
    budget: &mut Budget<'_>,
) -> Result<CompilerModuleHandoffReceiptV5> {
    entry(budget, payload_storage(handoff)?, |resources| {
        // V5 has private-construction identity and immutable, owned decoded bytes.
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
            CompilerModuleHandoffSlotV5::Production,
            handoff.identity().into(),
            handoff.canonical_bytes(),
            &mut NoFaults,
            resources,
        )?;
        Ok(<Schema as currentness::Schema>::receipt(fields, handoff))
    })
}

pub fn recover_compiler_module_handoff_receipt_v5(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    budget: &mut Budget<'_>,
) -> Result<CompilerModuleHandoffReceiptV5> {
    entry(budget, 0, |resources| {
        Ok(currentness::recover::<Schema>(
            output_dir,
            producer,
            attempt,
            CompilerModuleHandoffSlotV5::Production,
            resources,
        )?)
    })
}

/// Rederive inert occurrence coordinates from the complete strictly decoded V5
/// handoff. This observes no filesystem and creates no currentness/consumption
/// token. Historical bytes need not have been published or remain current.
///
/// Keep the handoff's backing capacity and decode metadata prepaid on the same
/// account. Producer text and the complete transport are charged before hashing;
/// scratch is released on return. The returned fixed receipt is unreserved.
/// ```compile_fail
/// use fe2o3_artifact_transaction::{CompilerModuleHandoffReceiptV5 as R,
///     ConsumedCompilerModuleHandoffV5 as C};
/// fn promote(receipt: R) -> C { receipt.into() }
/// ```
pub fn rederive_compiler_module_handoff_receipt_for_replay_v5(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV5,
    expected_transaction: CompilerModuleHandoffTransactionIdentityV5,
    handoff: &Handoff,
    budget: &mut Budget<'_>,
) -> Result<CompilerModuleHandoffReceiptV5> {
    entry(budget, payload_storage(handoff)?, |resources| {
        resources.reserve(REPLAY_SCRATCH_STORAGE)?;
        currentness::rederive_receipt::<Schema>(
            producer,
            attempt,
            slot,
            expected_transaction.as_bytes(),
            handoff,
            handoff.canonical_bytes(),
            resources,
        )
        .map_err(Error::from)
    })
}

/// The returned lease/header storage is admitted but unreserved; reserve it
/// before keeping or using the lease. Stream hashing never allocates a payload.
pub fn acquire_compiler_module_handoff_currentness_lease_v5(
    output_dir: &Path,
    producer: &ProducerIdentity,
    receipt: CompilerModuleHandoffReceiptV5,
    budget: &mut Budget<'_>,
) -> Result<(
    CompilerModuleHandoffCurrentnessLeaseV5,
    CompilerModuleHandoffStorageV5,
)> {
    entry(budget, 0, |resources| {
        let storage = CompilerModuleHandoffStorageV5(
            CURRENT_STORAGE + size_of::<CompilerModuleHandoffCurrentnessLeaseV5>(),
        );
        resources.reserve(storage.0)?;
        let binding = currentness::mint::<Schema>(output_dir, producer, receipt, resources)?;
        Ok((
            CompilerModuleHandoffCurrentnessLeaseV5 { binding, storage },
            storage,
        ))
    })
}

pub fn publish_compiler_module_handoff_with_currentness_v5(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    handoff: &Handoff,
    budget: &mut Budget<'_>,
) -> Result<(
    CompilerModuleHandoffPublicationV5,
    CompilerModuleHandoffStorageV5,
)> {
    let receipt =
        publish_compiler_module_handoff_v5(output_dir, producer, attempt, handoff, budget)?;
    let (lease, storage) = acquire_compiler_module_handoff_currentness_lease_v5(
        output_dir, producer, receipt, budget,
    )?;
    Ok((CompilerModuleHandoffPublicationV5 { lease }, storage))
}

pub fn consume_compiler_module_handoff_with_currentness_v5<T: AsRef<Handoff>>(
    lease: &CompilerModuleHandoffCurrentnessLeaseV5,
    token: CompilerModuleHandoffConsumptionTokenV5<T>,
    budget: &mut Budget<'_>,
) -> Result<ConsumedCompilerModuleHandoffV5<T>> {
    consume(lease, token, budget, &mut NoFaults)
}

/// Same one-shot durable transition on an original owned resource account.
/// Counts the complete actual lease and token again inside a <=256 MiB local
/// window. Additional work is Budget::STORAGE_WINDOW_WORK_V1; additional scratch
/// is both retained charges plus Budget::STORAGE_WINDOW_SCRATCH_V1. Errors retain
/// terminal reservations; no caller-selected floor, new account or cap increase.
pub fn consume_compiler_module_handoff_in_original_account_v5<T: AsRef<Handoff>>(
    lease: &CompilerModuleHandoffCurrentnessLeaseV5,
    token: CompilerModuleHandoffConsumptionTokenV5<T>,
    budget: &mut Budget<'_>,
) -> Result<ConsumedCompilerModuleHandoffV5<T>> {
    admission::consume_original(lease, token, budget, &mut NoFaults)
}

impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
