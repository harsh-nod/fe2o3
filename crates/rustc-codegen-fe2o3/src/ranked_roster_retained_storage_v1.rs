//! Actual ranked-verification owner composition; no reconstructed proof owners.
//! Includes logical BTree key payload, NOT physical BTree node allocation.

use super::{
    AuthenticatedFunctionalVerificationV1, AuthenticatedRankedVerificationRootV1,
    AuthenticatedRankedVerificationRosterV1, AuthenticatedRankedVerificationV5,
};
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1};
use fe2o3_verifier::InertFunctionalRefinementReceiptSignatureV2;

fn fixed<T: Copy>(_: &T) {}
fn fixed_slice<T: Copy>(_: &[T]) {}
fn owned_shape<T>(_: &T) {}

fn charge_extent(
    counter: &mut LogicalStorageCounterV1,
    count: usize,
    width: usize,
) -> Result<(), LogicalStorageErrorV1> {
    let bytes = count
        .checked_mul(width)
        .ok_or(LogicalStorageErrorV1::Arithmetic)?;
    counter.charge(bytes, 1)
}

// These transport rows are inline Copy bytes, not imported proofs or runtime
// owners. Count actual spare Vec slots; no value scan or nested heap is needed.
fn charge_effect_receipts(
    counter: &mut LogicalStorageCounterV1,
    receipts: &Vec<InertFunctionalRefinementReceiptSignatureV2>,
) -> Result<(), LogicalStorageErrorV1> {
    fixed_slice(receipts);
    counter.vector(receipts)
}

impl AuthenticatedRankedVerificationRosterV1 {
    /// Charge all retained heap payloads of this exact roster.
    ///
    /// The caller must charge one complete roster header/root beforehand (or
    /// include it in its enclosing owner). The roots Box charges all inline
    /// root/verification/Option/functional headers once. Each actual root costs
    /// an additional zero-byte item BEFORE inspecting its nested payloads.
    /// Canonical ordering is a separate Box at actual length. The retained
    /// original projection account contributes one actual Box<OwnedBudget>;
    /// its numeric reservation counter is not counted as duplicate owned memory.
    ///
    /// Each original root contributes String capacity, export Box bytes,
    /// middle-end canonical bytes, induction certificates and optional reachable
    /// Vec capacity, and effect-receipt Vec capacity. A functional Some also
    /// contributes its reconciled semantic contract, parallel hierarchy,
    /// original aggregate execution/receipt and retained staging policy.
    /// None has no nested heap; no Some branch is omitted or reconstructed.
    ///
    /// Policy storage explicitly uses LOGICAL BTree key payload at actual len;
    /// it excludes physical node capacity/metadata. Other extents use actual
    /// Vec/String capacity or Box length. This is not allocator/peak/RSS,
    /// whole-action storage or proof of the 128 MiB qualification target.
    ///
    /// One bounded counter is shared by every child. Collections are charged
    /// before variable traversal and each iterated root before its children.
    /// First arithmetic/byte/item refusal stops immediately. Prefix charges
    /// remain; discard the incomplete observation. No extra child headers,
    /// temporary candidate owners, clones, hashes, validation, constructors or
    /// authority are produced. This does not alter any admission limits.
    pub(crate) fn charge_retained_heap_storage_v1(
        &self,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> {
        let Self {
            roots,
            canonical_roster_identity,
            canonical_kernel_order,
            phase,
        } = self;
        fixed(canonical_roster_identity);
        owned_shape::<Box<[AuthenticatedRankedVerificationRootV1]>>(roots);
        owned_shape::<Box<[usize]>>(canonical_kernel_order);

        counter.array::<AuthenticatedRankedVerificationRootV1>(roots.len())?;
        for root in roots {
            counter.charge(0, 1)?;
            root.charge_retained_heap_storage_v1(counter)?;
        }
        counter.array::<usize>(canonical_kernel_order.len())?;
        phase.charge_retained_heap_storage_v1(counter)
    }
}

impl AuthenticatedRankedVerificationRootV1 {
    fn charge_retained_heap_storage_v1(
        &self,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> {
        let Self {
            logical_name,
            export_symbol,
            semantic_root,
            semantic_root_identity,
            kernel_binding,
            source_rank,
            verification,
        } = self;
        owned_shape::<String>(logical_name);
        owned_shape::<Box<[u8]>>(export_symbol);
        fixed(semantic_root);
        fixed(semantic_root_identity);
        fixed(kernel_binding);
        fixed(source_rank);
        counter.string(logical_name)?;
        counter.array::<u8>(export_symbol.len())?;
        verification.charge_retained_heap_storage_v1(counter)
    }
}

impl AuthenticatedRankedVerificationV5 {
    fn charge_retained_heap_storage_v1(
        &self,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> {
        let Self {
            middle_end_evidence,
            functional,
            semantic_u32_induction,
            effect_receipts,
        } = self;
        owned_shape::<Option<AuthenticatedFunctionalVerificationV1>>(functional);
        middle_end_evidence.charge_retained_heap_storage_v1(counter)?;
        semantic_u32_induction
            .visit_retained_heap_storage_v1(|count, width| charge_extent(counter, count, width))?;
        charge_effect_receipts(counter, effect_receipts)?;
        if let Some(functional) = functional {
            functional.charge_retained_heap_storage_v1(counter)?;
        }
        Ok(())
    }
}

impl AuthenticatedFunctionalVerificationV1 {
    fn charge_retained_heap_storage_v1(
        &self,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> {
        let Self {
            semantics,
            parallel_contract,
            parallel_report,
            aggregate,
        } = self;
        fixed(parallel_report);
        semantics
            .visit_retained_heap_storage_v1(|count, width| charge_extent(counter, count, width))?;
        parallel_contract
            .visit_retained_heap_storage_v1(|count, width| charge_extent(counter, count, width))?;
        aggregate.charge_retained_heap_storage_v1(counter)
    }
}

#[cfg(test)]
#[path = "ranked_roster_retained_storage_v1_tests.rs"]
mod tests;
