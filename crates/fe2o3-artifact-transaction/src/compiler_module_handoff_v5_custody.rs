//! Pre-acquisition retained bounds for the default V5 lease/token pair.
use super::*;
use crate::ArtifactLockRetirementBarrierV1 as Barrier;
use fe2o3_compiler_ffi::INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5 as METADATA;

const PATH_GUARD_CAPACITY: usize = 16 * 1024;

#[path = "compiler_module_handoff_v5_quota.rs"]
mod quota;
pub(super) use quota::prepay_currentness;
pub use quota::{
    CompilerModuleHandoffCustodyQuotaV5, CompilerModuleHandoffOperationQuotaV5,
    compiler_module_handoff_custody_quota_for_limit_v5,
    compiler_module_handoff_try_recovery_quota_v5,
};

/// Recovers an inert V5 receipt without waiting for a cooperating writer's lock.
/// Uses the same recovery, decoding, and accounting as ordinary V5 recovery.
/// A contended output/path lock returns `Busy` before recovery mutates any slot.
/// The actual originating-process barrier protects temporary lock destruction
/// through all resource-scope exits; no currentness owner escapes this operation.
/// Filesystem I/O and short internal mutex acquisition are not deadline bounded.
pub fn try_recover_compiler_module_handoff_receipt_v5(
    output: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    barrier: &Barrier,
    budget: &mut Budget<'_>,
) -> Result<CompilerModuleHandoffReceiptV5> {
    entry(budget, 0, |resources| {
        if !barrier.guards_artifact_locks() {
            return Err(Resource::Accounting.into());
        }
        Ok(currentness::try_recover::<Schema>(
            output,
            producer,
            attempt,
            CompilerModuleHandoffSlotV5::Production,
            MAX_COMPILER_MODULE_HANDOFF_BYTES_V5,
            None,
            resources,
        )?)
    })
}

/// Nonblocking recovery with an inert payload-size ceiling, on the same engine.
/// The durable record must fit before payload opening, allocation or reading.
/// The ceiling grants no authority and does not replace any currentness check.
/// Fund `compiler_module_handoff_try_recovery_quota_v5` on the original ledger;
/// no owner escapes and all temporary locks drop under the borrowed barrier.
pub fn try_recover_compiler_module_handoff_receipt_with_limit_v5(
    output: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    maximum_handoff_bytes: usize,
    barrier: &Barrier,
    budget: &mut Budget<'_>,
) -> Result<CompilerModuleHandoffReceiptV5> {
    quota::validate_length(maximum_handoff_bytes)?;
    let dynamic = dynamic_bound(output, producer)?;
    entry(budget, 0, |resources| {
        recover_limited(
            output,
            producer,
            attempt,
            maximum_handoff_bytes,
            dynamic,
            barrier,
            resources,
        )
    })
}

pub(super) fn recover_limited(
    output: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    maximum_handoff_bytes: usize,
    dynamic: usize,
    barrier: &Barrier,
    resources: &mut Resources<'_, '_>,
) -> Result<CompilerModuleHandoffReceiptV5> {
    if !barrier.guards_artifact_locks() {
        return Err(Resource::Accounting.into());
    }
    quota::filesystem(dynamic, quota::Operation::Recovery)?.prepay(resources)?;
    Ok(currentness::try_recover::<Schema>(
        output,
        producer,
        attempt,
        CompilerModuleHandoffSlotV5::Production,
        maximum_handoff_bytes,
        Some(dynamic),
        resources,
    )?)
}

/// Inert allocation quote for one exact receipt and input allocation shape.
/// This proves no publication, currentness, compiler identity, or retirement authority.
///
/// Includes both owners' complete currentness backing (conservatively twice),
/// producer String capacities, all retained path capacities, the receipt's exact
/// payload length, canonical V5 decoded-metadata allowance, and consumed headers.
/// The quote itself and an enclosing late payload's other fields are additional.
/// This is a retained bound, not a constructor scratch/work or allocator-RSS bound.
/// Constructor overlap must still fit the unchanged 256 MiB verification cap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerModuleHandoffCurrentnessCustodyQuoteV5 {
    receipt: CompilerModuleHandoffReceiptV5,
    output_length: usize,
    source_capacity: usize,
    crate_capacity: usize,
    dynamic: usize,
    lease: CompilerModuleHandoffStorageV5,
    token: CompilerModuleHandoffStorageV5,
    retained: usize,
}

impl CompilerModuleHandoffCurrentnessCustodyQuoteV5 {
    /// Complete future lease plus default-token charge, before either is acquired.
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
    pub const fn lease_storage(self) -> CompilerModuleHandoffStorageV5 {
        self.lease
    }
    pub const fn token_storage(self) -> CompilerModuleHandoffStorageV5 {
        self.token
    }
    pub const fn receipt(self) -> CompilerModuleHandoffReceiptV5 {
        self.receipt
    }
}

/// Quotes exact receipt-length retention without reserving the maximum wire size.
///
/// The constructor must see the same output byte length and producer capacities.
/// Actual retained path/producer capacities are checked against this declaration
/// BEFORE taking an artifact lock. Path-guard backing is capped at 16 KiB; larger
/// configured paths refuse before lock acquisition. Quotes are allocation limits,
/// not bindings to a path's contents, producer authority, or filesystem identity.
pub fn quote_compiler_module_handoff_currentness_custody_v5(
    output: &Path,
    producer: &ProducerIdentity,
    receipt: CompilerModuleHandoffReceiptV5,
) -> Result<CompilerModuleHandoffCurrentnessCustodyQuoteV5> {
    if receipt.length == 0
        || receipt.length > MAX_COMPILER_MODULE_HANDOFF_BYTES_V5
        || usize::try_from(receipt.handoff_identity.byte_len()).ok() != Some(receipt.length)
    {
        return Err(Error::HandoffIdentityMismatch);
    }
    let output_length = output.as_os_str().len();
    let source_capacity = producer.stable_source.capacity();
    let crate_capacity = producer.crate_name.capacity();
    let dynamic = dynamic_bound(output, producer)?;
    let lease = CompilerModuleHandoffStorageV5(quota::lease_storage(dynamic)?);
    let token = CompilerModuleHandoffStorageV5(quota::token_storage(dynamic, receipt.length)?);
    let retained = lease.0.checked_add(token.0).ok_or(Resource::Arithmetic)?;
    Ok(CompilerModuleHandoffCurrentnessCustodyQuoteV5 {
        receipt,
        output_length,
        source_capacity,
        crate_capacity,
        dynamic,
        lease,
        token,
        retained,
    })
}

pub(super) fn dynamic_bound(output: &Path, producer: &ProducerIdentity) -> Result<usize> {
    dynamic_bound_for_lengths(
        output.as_os_str().len(),
        producer.stable_source.capacity(),
        producer.crate_name.capacity(),
    )
}

fn dynamic_bound_for_lengths(
    output_length: usize,
    source_capacity: usize,
    crate_capacity: usize,
) -> Result<usize> {
    let parent_name = Schema::PARENT_PREFIX
        .len()
        .checked_add(64)
        .ok_or(Resource::Arithmetic)?;
    let slot_name = Schema::SLOT_PREFIX
        .len()
        .checked_add(64)
        .ok_or(Resource::Arithmetic)?;
    let parent_path = output_length
        .checked_add(1)
        .and_then(|n| n.checked_add(parent_name))
        .ok_or(Resource::Arithmetic)?;
    let slot_path = parent_path
        .checked_add(1)
        .and_then(|n| n.checked_add(slot_name))
        .ok_or(Resource::Arithmetic)?;
    let mut dynamic = source_capacity
        .checked_add(crate_capacity)
        .and_then(|n| n.checked_add(PATH_GUARD_CAPACITY))
        .ok_or(Resource::Arithmetic)?;
    // Allow ordinary PathBuf join growth, but never assume an allocator's capacity policy.
    // The complete actual capacities are checked before the first artifact lock.
    for length in [
        output_length,
        parent_name,
        slot_name,
        parent_path,
        slot_path,
    ] {
        dynamic = length
            .checked_mul(2)
            .and_then(|n| dynamic.checked_add(n))
            .ok_or(Resource::Arithmetic)?;
    }
    Ok(dynamic)
}

/// Acquires a real V5 lease within the quote's complete retained bound.
///
/// Borrow the actual originating-process retirement barrier across this entire
/// call, including all resource-scope exits. Temporary-lock rollback is therefore
/// free of spawn-progress waits. All retained variable-sized metadata is allocated
/// and checked before taking a lock; only fixed-size pins/Arc headers follow it.
/// Success returns UNRESERVED lease storage. A late custodian must already have
/// prepaid the complete quote and install this owner before any fallible exit.
pub fn acquire_compiler_module_handoff_currentness_lease_with_quote_v5(
    output: &Path,
    producer: &ProducerIdentity,
    quote: &CompilerModuleHandoffCurrentnessCustodyQuoteV5,
    barrier: &Barrier,
    budget: &mut Budget<'_>,
) -> Result<(
    CompilerModuleHandoffCurrentnessLeaseV5,
    CompilerModuleHandoffStorageV5,
)> {
    entry(budget, 0, |resources| {
        acquire_quoted(output, producer, quote, barrier, resources)
    })
}

pub(super) fn acquire_quoted(
    output: &Path,
    producer: &ProducerIdentity,
    quote: &CompilerModuleHandoffCurrentnessCustodyQuoteV5,
    barrier: &Barrier,
    resources: &mut Resources<'_, '_>,
) -> Result<(
    CompilerModuleHandoffCurrentnessLeaseV5,
    CompilerModuleHandoffStorageV5,
)> {
    if !barrier.guards_artifact_locks()
        || quote_compiler_module_handoff_currentness_custody_v5(output, producer, quote.receipt)?
            != *quote
    {
        return Err(Resource::Accounting.into());
    }
    quota::filesystem(quote.dynamic, quota::Operation::Lease)?.prepay(resources)?;
    resources.reserve(quote.lease.0)?;
    resources.work(quote.dynamic.checked_mul(4).ok_or(Resource::Arithmetic)?)?;
    let binding = mint_quoted(output, producer, quote, resources)?;
    Ok((
        CompilerModuleHandoffCurrentnessLeaseV5 {
            binding,
            storage: quote.lease,
        },
        quote.lease,
    ))
}

fn dynamic_storage(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    parent: &PinnedDirectory,
    slot: &PinnedDirectory,
) -> std::result::Result<usize, Resource> {
    let guard = output
        .path_guard
        .as_ref()
        .map_or(0, |guard| guard.display_path.capacity());
    if guard > PATH_GUARD_CAPACITY {
        return Err(Resource::Accounting);
    }
    currentness::location_storage(output, producer, parent, slot)
}

fn mint_quoted(
    output_dir: &Path,
    producer: &ProducerIdentity,
    quote: &CompilerModuleHandoffCurrentnessCustodyQuoteV5,
    resources: &mut Resources<'_, '_>,
) -> std::result::Result<Arc<currentness::Current<Schema>>, HandoffEngineError> {
    resources.require::<Schema>()?;
    let receipt = quote.receipt;
    let fields = <Schema as currentness::Schema>::receipt_fields(receipt);
    let output = PinnedOutput::open_existing(output_dir)?;
    let producer = producer.clone();
    let producer_identity = producer_identity_for::<Schema>(&producer);
    let slot_identity = slot_identity_for::<Schema>(producer_identity, fields.attempt, fields.slot);
    let parent = open_private_directory(
        &output.fd,
        &output.display_path,
        format!("{}{}", Schema::PARENT_PREFIX, hex(&producer_identity)),
    )?
    .ok_or(CompilerModuleHandoffErrorV1::NotPublished)?;
    let slot_directory = open_private_directory(
        &parent.fd,
        &parent.path,
        format!("{}{}", Schema::SLOT_PREFIX, hex(&slot_identity)),
    )?
    .ok_or(CompilerModuleHandoffErrorV1::NotPublished)?;
    if dynamic_storage(&output, &producer, &parent, &slot_directory)? > quote.dynamic {
        return Err(Resource::Accounting.into());
    }
    let _lock = output.try_lock()?.ok_or(HandoffEngineError::Busy)?;
    output.verify_path_identity()?;
    authorize_for_custody(&output, &producer, fields.attempt, Schema::ATTEMPT_CUSTODY)?;
    // The directories were opened before locking. Revalidate those exact pins under lock.
    parent.verify()?;
    cleanup_stale_slots::<Schema>(&parent, producer_identity, fields.attempt)?;
    currentness::finish_mint::<Schema>(
        currentness::CurrentLocation {
            output,
            producer,
            producer_identity,
            parent,
            slot_directory,
            slot_identity,
        },
        receipt,
        resources,
    )
}

impl CompilerModuleHandoffCurrentnessLeaseV5 {
    /// Acquires the default token with exact receipt-length backing under the same quote.
    ///
    /// Requires a lease from the quoted constructor. Allocate/check the entire
    /// Vec capacity before taking the token lock; filling and decoding do not grow
    /// that backing. Refusal/unwind drops temporary locks under the borrowed actual
    /// barrier and leaves the caller's existing lease in place. The successful
    /// token must be installed in prepaid late custody before any fallible work.
    pub fn acquire_current_token_with_quote(
        &self,
        quote: &CompilerModuleHandoffCurrentnessCustodyQuoteV5,
        barrier: &Barrier,
        budget: &mut Budget<'_>,
    ) -> Result<(
        CompilerModuleHandoffConsumptionTokenV5,
        CompilerModuleHandoffStorageV5,
    )> {
        entry(budget, self.storage.0, |resources| {
            self.acquire_quoted_in(quote, barrier, resources)
        })
    }

    pub(super) fn acquire_quoted_in(
        &self,
        quote: &CompilerModuleHandoffCurrentnessCustodyQuoteV5,
        barrier: &Barrier,
        resources: &mut Resources<'_, '_>,
    ) -> Result<(
        CompilerModuleHandoffConsumptionTokenV5,
        CompilerModuleHandoffStorageV5,
    )> {
        if !barrier.guards_artifact_locks()
            || self.receipt() != quote.receipt
            || self.storage != quote.lease
        {
            return Err(Resource::Accounting.into());
        }
        quota::filesystem(quote.dynamic, quota::Operation::Token)?.prepay(resources)?;
        let headers = quote
            .token
            .0
            .checked_sub(quote.receipt.length)
            .and_then(|n| n.checked_sub(METADATA))
            .ok_or(Resource::Accounting)?;
        resources.reserve(headers)?;
        let mut bytes = resources.exact_buffer(quote.receipt.length)?;
        resources.work(quote.receipt.length)?;
        bytes.resize(quote.receipt.length, 0);
        let lock = self
            .binding
            .output
            .try_lock()
            .map_err(HandoffEngineError::from)?
            .ok_or(Error::Busy)?;
        let content = currentness::load_preallocated(&self.binding, bytes, resources)?;
        Ok((
            CompilerModuleHandoffConsumptionTokenV5 {
                binding: Arc::clone(&self.binding),
                backing: backing_snapshot(&content),
                content,
                storage: quote.token,
                _lock: lock,
            },
            quote.token,
        ))
    }
}

#[cfg(test)]
#[path = "compiler_module_handoff_v5_custody_tests.rs"]
mod tests;
