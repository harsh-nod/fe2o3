//! Source-owned Policy10 plus actual V18 LICM relocation, before final native
//! completion. This owner cannot be used as a Policy10 mixed output handoff.

use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1 as InventoryError, CanonicalKirInventoryV18 as Inventory,
    CanonicalKirMemorySsaErrorV1 as MemoryError, CanonicalKirMemorySsaV18 as Memory,
};
use fe2o3_kernel_opt::{OwnedLicmContinuationV18 as Tail, OwnedLicmErrorV1 as MotionError};
use std::mem::{align_of, size_of};

#[path = "production_source_mixed_licm_coordinates_v28.rs"]
mod coordinates;
pub use coordinates::ProductionMixedLicmDefinitionProjectionV28;

#[path = "production_source_mixed_licm_native_v28.rs"]
mod native;
pub use native::{
    ProductionConditionalMixedLicmOutputHandoffV28, ProductionMixedLicmCompletionErrorV28,
    ProductionMixedLicmRuntimeOccurrenceV28,
};

/// Refusal while retaining or replaying a source-bound Policy10-to-LICM tail.
#[derive(Debug)]
pub enum ProductionMixedLicmRelocationErrorV28 {
    /// Original source association or its retained resource custody was refused.
    Source(ProductionSourceOwnedViewErrorV18),
    /// Actual LICM preparation or independent motion replay was refused.
    Motion(MotionError),
    /// An exact endpoint inventory could not be derived.
    Inventory(InventoryError),
    /// MemorySSA construction or endpoint memory correspondence was refused.
    Memory(MemoryError),
    /// Retained source, coordinate or endpoint identities do not agree.
    Binding(&'static str),
}
type Error = ProductionMixedLicmRelocationErrorV28;
type Result<T> = std::result::Result<T, Error>;
impl From<ProductionSourceOwnedViewErrorV18> for Error {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}
impl From<ArgumentResourceV1> for Error {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Source(error.into())
    }
}
impl From<MotionError> for Error {
    fn from(error: MotionError) -> Self {
        Self::Motion(error)
    }
}
impl From<InventoryError> for Error {
    fn from(error: InventoryError) -> Self {
        Self::Inventory(error)
    }
}
impl From<MemoryError> for Error {
    fn from(error: MemoryError) -> Self {
        Self::Memory(error)
    }
}
impl From<coordinates::Error> for Error {
    fn from(error: coordinates::Error) -> Self {
        match error {
            coordinates::Error::Resource(error) => error.into(),
            coordinates::Error::Memory(error) => Self::Memory(error),
            coordinates::Error::Mismatch(message) => Self::Binding(message),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "source-owned mixed LICM relocation: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Motion(error) => Some(error),
            Self::Inventory(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Binding(_) => None,
        }
    }
}

/// Retains the genuine source-bound Policy10 prefix and actual freshly admitted
/// LICM output together. Relocation and MemorySSA are checked on exact owners.
/// Final native/source completion, target emission and Worker admission remain
/// separate gates; no Policy10 witness is reinterpreted as a motion witness.
#[must_use = "retain the source prefix and discard this owner's exact credit explicitly"]
pub struct ProductionMixedLicmRelocationV28<'prefix, 'view, 'source> {
    prefix: &'prefix ProductionConditionalMixedPureCseOutputHandoffV26<'view, 'source>,
    tail: Tail,
    projection: coordinates::Projection,
    retained: usize,
    required: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

fn headers() -> Result<(usize, usize)> {
    let owner = size_of::<ProductionMixedLicmRelocationV28<'_, '_, '_>>()
        .checked_sub(size_of::<Tail>())
        .and_then(|bytes| bytes.checked_sub(size_of::<coordinates::Projection>()))
        .and_then(|bytes| {
            bytes.checked_add(align_of::<ProductionMixedLicmRelocationV28<'_, '_, '_>>())
        })
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    let scratch = argument_sum_v1(&[
        2 * size_of::<Inventory<'_>>(),
        2 * size_of::<Memory<'_, '_>>(),
        size_of::<fe2o3_kernel_analysis::CheckedCanonicalKirLicmV18<'_>>(),
        2 * size_of::<Result<(Tail, coordinates::Projection, usize)>>(),
        size_of::<[usize; 24]>(),
        size_of::<[&(); 12]>(),
    ])?;
    Ok((owner, scratch))
}

impl<'view, 'source> ProductionConditionalMixedPureCseOutputHandoffV26<'view, 'source> {
    /// Runs actual LICM on this output and retains the source prefix borrow.
    /// Caller-selected graphs, origin rows and layout-limit widening are absent.
    pub fn prepare_mixed_licm_v28<'prefix>(
        &'prefix self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ProductionMixedLicmRelocationV28<'prefix, 'view, 'source>> {
        self.owned.check(budget)?;
        let source = self.owned.source;
        let floor = budget.storage();
        let (tail, projection, retained) =
            scoped_source_attempt_v29(source.cleanup, budget, floor, |budget| -> Result<_> {
                let entry = budget.storage();
                let (owner_header, scratch_header) = headers()?;
                budget.reserve_storage(argument_sum_v1(&[owner_header, scratch_header])?)?;
                let input = self.output(budget)?.owner();
                let layouts = source.limits(budget)?.storage_layout_limits();
                let tail = fe2o3_kernel_opt::prepare_owned_licm_v18(input, layouts, budget)?;
                budget.reserve_storage(tail.retained_storage())?;
                let (pair, ps) = tail.replay_against(input, budget)?;
                budget.reserve_storage(ps.retained_storage())?;
                let (before, bs) = Inventory::derive_v18(input, budget)?;
                budget.reserve_storage(bs.retained_storage())?;
                let (after, os) = Inventory::derive_v18(tail.output(), budget)?;
                budget.reserve_storage(os.retained_storage())?;
                let (input_memory, ims) = Memory::derive_v18(&before, Default::default(), budget)?;
                budget.reserve_storage(ims.retained_storage())?;
                let (output_memory, oms) = Memory::derive_v18(&after, Default::default(), budget)?;
                budget.reserve_storage(oms.retained_storage())?;
                let map_floor = budget.storage();
                let projection = coordinates::Projection::build(&pair, &before, &after, budget)?;
                let map_storage = budget
                    .storage()
                    .checked_sub(map_floor)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                coordinates::replay_memory(
                    &pair,
                    &before,
                    &after,
                    &input_memory,
                    &output_memory,
                    budget,
                )?;
                drop(output_memory);
                drop(input_memory);
                drop(after);
                drop(before);
                drop(pair);
                budget.release_storage(argument_sum_v1(&[
                    scratch_header,
                    ps.retained_storage(),
                    bs.retained_storage(),
                    os.retained_storage(),
                    ims.retained_storage(),
                    oms.retained_storage(),
                ])?)?;
                self.owned.check(budget)?;
                let retained =
                    argument_sum_v1(&[owner_header, tail.retained_storage(), map_storage])?;
                if entry.checked_add(retained) != Some(budget.storage()) {
                    source.cleanup.deny_refund();
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                Ok((tail, projection, retained))
            })?;
        Ok(ProductionMixedLicmRelocationV28 {
            prefix: self,
            tail,
            projection,
            retained,
            required: budget.storage(),
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
        })
    }
}

impl<'prefix, 'view, 'source> ProductionMixedLicmRelocationV28<'prefix, 'view, 'source> {
    fn custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let source = self.prefix.owned.source;
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.required
        {
            source.cleanup.deny_refund();
            return source.retain_query(Err(ArgumentResourceV1::Accounting.into()));
        }
        self.prefix.owned.custody(budget)
    }

    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let custody = self.custody(budget);
        self.prefix.owned.check(budget).and(custody)
    }

    /// Borrows the genuine retained Policy10 handoff under its original custody.
    pub fn prefix(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&ProductionConditionalMixedPureCseOutputHandoffV26<'view, 'source>>
    {
        self.check(budget)?;
        Ok(self.prefix)
    }

    /// Borrows the actual LICM continuation, not a reconstructed output graph.
    pub fn tail(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<&Tail> {
        self.check(budget)?;
        Ok(&self.tail)
    }

    /// Returns the complete checked prefix-to-final definition coordinate map.
    pub fn definition_projection(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[ProductionMixedLicmDefinitionProjectionV28]> {
        self.check(budget)?;
        Ok(self.projection.rows())
    }

    /// Rejoins this tail to the exact original semantic SSA owner of its prefix.
    pub fn check_original_source(
        &self,
        source: &ProductionSemanticSsaOwnerV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        self.prefix.check_original_source(source, budget)
    }

    /// Returns this tail's retained credit, excluding the borrowed prefix.
    pub fn retained_storage(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<usize> {
        self.check(budget)?;
        Ok(self.retained)
    }

    /// Rebuilds both actual inventories and memory graphs and independently
    /// replays the retained map. A digest match is not a replay substitute.
    pub fn replay(&self, budget: &mut ArgumentBudgetV1<'_>) -> Result<()> {
        self.replay_inner(budget, false)
    }

    fn replay_inner(&self, budget: &mut ArgumentBudgetV1<'_>, test_refusals: bool) -> Result<()> {
        self.check(budget)?;
        let source = self.prefix.owned.source;
        let floor = budget.storage();
        scoped_source_attempt_v29(source.cleanup, budget, floor, |budget| {
            let entry = budget.storage();
            let scratch_header = headers()?.1;
            budget.reserve_storage(scratch_header)?;
            let input = self.prefix.output(budget)?.owner();
            let (pair, ps) = self.tail.replay_against(input, budget)?;
            budget.reserve_storage(ps.retained_storage())?;
            let (before, bs) = Inventory::derive_v18(input, budget)?;
            budget.reserve_storage(bs.retained_storage())?;
            let (after, os) = Inventory::derive_v18(self.tail.output(), budget)?;
            budget.reserve_storage(os.retained_storage())?;
            let (input_memory, ims) = Memory::derive_v18(&before, Default::default(), budget)?;
            budget.reserve_storage(ims.retained_storage())?;
            let (output_memory, oms) = Memory::derive_v18(&after, Default::default(), budget)?;
            budget.reserve_storage(oms.retained_storage())?;
            self.projection.replay(&pair, &before, &after, budget)?;
            coordinates::replay_memory(
                &pair,
                &before,
                &after,
                &input_memory,
                &output_memory,
                budget,
            )?;
            #[cfg(test)]
            if test_refusals {
                coordinates::test_refusals(
                    &pair,
                    &before,
                    &after,
                    &input_memory,
                    &output_memory,
                    budget,
                )?;
            }
            #[cfg(not(test))]
            let _ = test_refusals;
            drop(output_memory);
            drop(input_memory);
            drop(after);
            drop(before);
            drop(pair);
            budget.release_storage(argument_sum_v1(&[
                scratch_header,
                ps.retained_storage(),
                bs.retained_storage(),
                os.retained_storage(),
                ims.retained_storage(),
                oms.retained_storage(),
            ])?)?;
            if budget.storage() != entry {
                source.cleanup.deny_refund();
                return Err(ArgumentResourceV1::Accounting.into());
            }
            self.check(budget)?;
            Ok(())
        })
    }

    #[cfg(test)]
    pub(super) fn check_projection_refusals_v28(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<()> {
        self.replay_inner(budget, true)
    }

    /// Drops only this tail and its map, preserving the borrowed prefix credit.
    /// Selected query failure does not prevent intact cleanup-only settlement.
    pub fn discard(self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let result = self.check(budget);
        let custody = self.custody(budget);
        let Self {
            prefix,
            tail,
            projection,
            retained,
            ..
        } = self;
        drop(projection);
        drop(tail);
        let settled = custody.and_then(|()| {
            prefix
                .owned
                .source
                .retain_query(budget.release_storage(retained).map_err(Into::into))
        });
        result?;
        settled
    }

    /// Always false: relocation alone does not run final native/source checks.
    pub const fn final_native_completion_is_complete(&self) -> bool {
        false
    }

    /// Always false: checked relocation is not artifact or launch admission.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}
