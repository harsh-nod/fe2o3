//! Consuming authenticated original-N roster to independently checked erased E.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionRankedSemanticProjectionRootV1 as LoweringRoot,
    ProductionUnitLocalErasedSourceOwnerV1 as Erased,
};

type E = ProductionRankedVerificationErrorV1;

fn resource(error: Resource) -> E {
    E::Custody(error.into())
}

fn sum(left: usize, right: usize) -> Result<usize, E> {
    left.checked_add(right)
        .ok_or_else(|| resource(Resource::Arithmetic))
}

fn row_bytes<T>(count: usize) -> Result<usize, E> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| resource(Resource::Arithmetic))
}

fn rows<T>(count: usize, budget: &mut Budget<'_>) -> Result<(Vec<T>, usize), E> {
    budget.charge_work(2).map_err(resource)?;
    let requested = sum(row_bytes::<T>(count)?, std::mem::size_of::<Vec<T>>())?;
    budget.reserve_storage(requested).map_err(resource)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| resource(Resource::Allocation))?;
    let actual = sum(
        row_bytes::<T>(values.capacity())?,
        std::mem::size_of::<Vec<T>>(),
    )?;
    budget
        .reserve_storage(
            actual
                .checked_sub(requested)
                .ok_or_else(|| resource(Resource::Accounting))?,
        )
        .map_err(resource)?;
    Ok((values, actual))
}

// Only the new erased endpoint opts into explicit map conversion accounting.
// The legacy caller of the pure split retains its existing charges/domain.
fn prepare_source_map_conversion_v1(
    accesses: &Vec<ProductionRankedAccessSourceV1>,
    effects: &Vec<ProductionRankedExecutableEffectSourceV1>,
    budget: &mut Budget<'_>,
) -> Result<(usize, usize), E> {
    budget
        .charge_work(sum(12, sum(accesses.len(), effects.len())?)?)
        .map_err(resource)?;
    let headers = sum(
        std::mem::size_of::<Vec<ProductionRankedAccessSourceV1>>(),
        std::mem::size_of::<Vec<ProductionRankedExecutableEffectSourceV1>>(),
    )?;
    let temporary = sum(
        headers,
        sum(
            row_bytes::<ProductionRankedAccessSourceV1>(accesses.capacity())?,
            row_bytes::<ProductionRankedExecutableEffectSourceV1>(effects.capacity())?,
        )?,
    )?;
    let boxed = sum(
        row_bytes::<ProductionRankedAccessSourceV1>(accesses.len())?,
        row_bytes::<ProductionRankedExecutableEffectSourceV1>(effects.len())?,
    )?;
    // Shrinking either Vec can allocate its boxed destination before releasing
    // the original buffer. LoweringRoot's header is already container-reserved.
    budget
        .reserve_storage(sum(temporary, boxed)?)
        .map_err(resource)?;
    Ok((temporary, boxed))
}

/// Shared ownership split only. All authenticated joins precede this move.
impl ProductionRankedVerifiedRootCandidateV1 {
    pub(super) fn into_source_and_verification_v1(
        self,
    ) -> (LoweringRoot, AuthenticatedRankedVerificationRootV1) {
        let Self {
            logical_name,
            export_symbol,
            semantic_root,
            semantic_root_identity,
            kernel_binding,
            source_rank,
            lowering,
            ranked_ir,
            access_sources,
            executable_effect_sources,
            verification,
        } = self;
        (
            LoweringRoot::new(
                semantic_root,
                source_rank,
                lowering,
                ranked_ir,
                access_sources,
                executable_effect_sources,
            ),
            AuthenticatedRankedVerificationRootV1 {
                logical_name,
                export_symbol,
                semantic_root,
                semantic_root_identity,
                kernel_binding,
                source_rank,
                verification,
            },
        )
    }
}

/// New retained source-root/E rows and verification-container storage. Original
/// N/source stays caller-reserved. Existing authenticated proof and collector
/// payloads retain their inherited engine limits; this is not heap accounting.
#[derive(Clone, Copy)]
pub(crate) struct ErasedSourceStageStorageV1(usize);

impl ErasedSourceStageStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

impl ProductionRankedSemanticProjectionRosterReceiptV1 {
    /// Keeps N as source evidence and produces a distinct E. Neither the legacy
    /// materialized-receipt guard nor any raw-empty policy is altered.
    /// Caller reserves original N/source before entry and returned storage before
    /// allocating B. New scratch restores its incoming floor on every exit.
    pub(crate) fn into_silent_unit_erased_source_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            Erased,
            AuthenticatedRankedVerificationRosterV1,
            ErasedSourceStageStorageV1,
        ),
        E,
    > {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let slot = budget as *const Budget<'_> as usize;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            budget.charge_work(8).map_err(resource)?;
            let original_storage = self
                .materialized
                .unit_local_source_storage_floor_v1()
                .map_err(E::Custody)?;
            if floor < original_storage {
                return Err(resource(Resource::Accounting));
            }
            self.verify_equivalence()?;
            let Self {
                materialized,
                source_order_roots,
                canonical_kernel_order,
                canonical_roster_identity,
            } = self;
            let count = source_order_roots.len();
            let (mut lowering, lowering_container) = rows::<LoweringRoot>(count, budget)?;
            let (mut verification, verification_container) =
                rows::<AuthenticatedRankedVerificationRootV1>(count, budget)?;
            let mut prepaid_maps = 0usize;
            for root in source_order_roots.into_vec() {
                budget.charge_work(2).map_err(resource)?;
                let (temporary_maps, boxed_maps) = prepare_source_map_conversion_v1(
                    &root.access_sources,
                    &root.executable_effect_sources,
                    budget,
                )?;
                let (source, verified) = root.into_source_and_verification_v1();
                // Both original Vecs have now been consumed. Their exact boxed
                // payload stays reserved and transfers into the lowerer owner.
                budget.release_storage(temporary_maps).map_err(resource)?;
                prepaid_maps = sum(prepaid_maps, boxed_maps)?;
                lowering.push(source);
                verification.push(verified);
            }
            let input_storage = Erased::input_storage_floor_v1(&materialized, &lowering, budget)
                .map_err(E::Custody)?;
            let input_extra = input_storage
                .checked_sub(original_storage)
                .ok_or_else(|| resource(Resource::Accounting))?;
            budget
                .reserve_storage(
                    input_extra
                        .checked_sub(lowering_container)
                        .and_then(|remaining| remaining.checked_sub(prepaid_maps))
                        .ok_or_else(|| resource(Resource::Accounting))?,
                )
                .map_err(resource)?;
            let (source, additional) =
                Erased::try_produce_v1(materialized, lowering, budget).map_err(E::ErasedSource)?;
            budget
                .reserve_storage(additional.retained_storage())
                .map_err(resource)?;

            // Box conversion may shrink by allocating a second buffer. Both
            // container reservations overlap until the Vec is consumed.
            let final_verification = sum(
                row_bytes::<AuthenticatedRankedVerificationRootV1>(count)?,
                std::mem::size_of::<AuthenticatedRankedVerificationRosterV1>(),
            )?;
            budget
                .reserve_storage(final_verification)
                .map_err(resource)?;
            let roots = verification.into_boxed_slice();
            budget
                .release_storage(verification_container)
                .map_err(resource)?;
            if roots.len() != source.erased().module().kernels.len() {
                return Err(E::RosterMetadata(
                    "erased source changed the authenticated root roster",
                ));
            }
            let retained = sum(
                sum(input_extra, additional.retained_storage())?,
                final_verification,
            )?;
            Ok((
                source,
                AuthenticatedRankedVerificationRosterV1 {
                    roots,
                    canonical_roster_identity,
                    canonical_kernel_order,
                },
                ErasedSourceStageStorageV1(retained),
            ))
        }));
        if ledger != budget.work_ledger_identity_v1()
            || slot != budget as *const Budget<'_> as usize
        {
            drop(result);
            return Err(resource(Resource::Accounting));
        }
        let Some(extra) = budget.storage().checked_sub(floor) else {
            drop(result);
            return Err(resource(Resource::Accounting));
        };
        if let Err(error) = budget.release_storage(extra) {
            drop(result);
            return Err(resource(error));
        }
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

#[cfg(test)]
#[path = "production_ranked_unit_local_source_map_budget_v1_tests.rs"]
mod map_budget_tests;
