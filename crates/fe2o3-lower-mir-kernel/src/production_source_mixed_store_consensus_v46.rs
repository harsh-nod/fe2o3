//! A separately retained actual post-LICM graph, never a relabeled LICM output.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirCrossBlockForwardingStorageV1 as PairStorage,
    CheckedCanonicalKirCrossBlockForwardingV18 as Pair,
};
use fe2o3_kernel_opt::OwnedCrossBlockForwardingV18 as Forwarding;

/// Source-bound all-path Store consensus. Final native checks and the concrete
/// memory refinement stage remain independent obligations on this exact owner.
#[must_use = "discard before the borrowed LICM and original source owners"]
pub struct ProductionMixedStoreConsensusV46<
    'motion,
    'prefix,
    'view,
    'source,
    P: ProductionMixedPrefixOwnerV29<'view, 'source>,
> {
    pub(super) relocation: &'motion ProductionMixedLicmRelocationV28<'prefix, 'view, 'source, P>,
    pub(super) tail: Forwarding,
    retained: usize,
    required: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

/// The fixed production tail is available only after the genuine Policy11 owner.
pub type ProductionMixedFixedpointStoreConsensusV46<'motion, 'prefix, 'view, 'source> =
    ProductionMixedStoreConsensusV46<
        'motion,
        'prefix,
        'view,
        'source,
        ProductionConditionalMixedFixedpointOutputHandoffV29<'view, 'source>,
    >;

fn owner_header<'view, 'source: 'view, P: ProductionMixedPrefixOwnerV29<'view, 'source>>()
-> Result<usize> {
    size_of::<ProductionMixedStoreConsensusV46<'_, '_, 'view, 'source, P>>()
        .checked_sub(size_of::<Forwarding>())
        .and_then(|n| {
            n.checked_add(align_of::<
                ProductionMixedStoreConsensusV46<'_, '_, 'view, 'source, P>,
            >())
        })
        .ok_or_else(|| ArgumentResourceV1::Arithmetic.into())
}

impl<'prefix, 'view, 'source> ProductionMixedFixedpointLicmRelocationV29<'prefix, 'view, 'source> {
    /// Runs the fixed consensus transaction against the actual LICM output.
    /// There is no user pass list, identity-only shortcut or failed-pass fallback.
    pub fn prepare_store_consensus_v46<'motion>(
        &'motion self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ProductionMixedFixedpointStoreConsensusV46<'motion, 'prefix, 'view, 'source>> {
        self.check(budget)?;
        let source = self.prefix.source_owned_v29();
        let floor = budget.storage();
        let (tail, retained) =
            scoped_source_attempt_v29(source.cleanup, budget, floor, |budget| -> Result<_> {
                let entry = budget.storage();
                let header = owner_header::<
                    ProductionConditionalMixedFixedpointOutputHandoffV29<'view, 'source>,
                >()?;
                let scratch = argument_sum_v1(&[
                    size_of::<Pair<'_>>(),
                    size_of::<PairStorage>(),
                    2 * size_of::<Result<(Forwarding, usize)>>(),
                    size_of::<fe2o3_kernel_analysis::CanonicalKirCrossBlockForwardingLimitsV1>(),
                    size_of::<fe2o3_kernel_ir::StorageLayoutLimitsV1>(),
                ])?;
                budget.reserve_storage(argument_sum_v1(&[header, scratch])?)?;
                self.replay(budget)?;
                let layouts = source.limits(budget)?.storage_layout_limits();
                let tail = fe2o3_kernel_opt::prepare_owned_cross_block_forwarding_v18(
                    self.tail.output(),
                    Default::default(),
                    layouts,
                    budget,
                )?;
                budget.reserve_storage(tail.retained_storage())?;
                let (pair, storage) = tail.replay_against(self.tail.output(), budget)?;
                budget.reserve_storage(storage.retained_storage())?;
                drop(pair);
                budget.release_storage(argument_sum_v1(&[scratch, storage.retained_storage()])?)?;
                self.check(budget)?;
                let retained = argument_sum_v1(&[header, tail.retained_storage()])?;
                if entry.checked_add(retained) != Some(budget.storage()) {
                    source.cleanup.deny_refund();
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                Ok((tail, retained))
            })?;
        Ok(ProductionMixedStoreConsensusV46 {
            relocation: self,
            tail,
            retained,
            required: budget.storage(),
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
        })
    }
}

impl<'motion, 'prefix, 'view, 'source, P: ProductionMixedPrefixOwnerV29<'view, 'source>>
    ProductionMixedStoreConsensusV46<'motion, 'prefix, 'view, 'source, P>
{
    pub(super) fn custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let source = self.relocation.prefix.source_owned_v29();
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.required
        {
            source.cleanup.deny_refund();
            return source.retain_query(Err(ArgumentResourceV1::Accounting.into()));
        }
        self.relocation.custody(budget)
    }
    pub(super) fn check(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let custody = self.custody(budget);
        self.relocation.check(budget).and(custody)
    }
    /// The independently retained motion stage, whose output is this tail's input.
    pub fn relocation(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&'motion ProductionMixedLicmRelocationV28<'prefix, 'view, 'source, P>>
    {
        self.check(budget)?;
        Ok(self.relocation)
    }
    /// Actual final graph; it may not be substituted for the LICM proof subject.
    pub fn output(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18> {
        self.check(budget)?;
        Ok(self.tail.output())
    }
    /// Reconstructs the complete all-path check from actual operations/CFG.
    /// The caller must reserve the returned pair credit for its entire lifetime.
    pub fn replay<'a>(
        &'a self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(Pair<'a>, PairStorage)> {
        self.check(budget)?;
        let source = self.relocation.prefix.source_owned_v29();
        let floor = budget.storage();
        scoped_source_attempt_v29(source.cleanup, budget, floor, |budget| -> Result<_> {
            let result = self
                .tail
                .replay_against(self.relocation.tail.output(), budget)?;
            self.check(budget)?;
            Ok(result)
        })
    }
    /// Exact owned credit, excluding the borrowed source/prefix/LICM owners.
    pub fn retained_storage(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<usize> {
        self.check(budget)?;
        Ok(self.retained)
    }
    /// Drops all actual output bytes before refund; selected errors do not skip
    /// custody-only cleanup of otherwise intact storage.
    pub fn discard(self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let selected = self.check(budget);
        let custody = self.custody(budget);
        let Self {
            relocation,
            tail,
            retained,
            ..
        } = self;
        drop(tail);
        let settled = custody.and_then(|()| {
            relocation
                .prefix
                .source_owned_v29()
                .retain_query(budget.release_storage(retained).map_err(Into::into))
        });
        selected?;
        settled
    }
    /// This transaction alone is not native, proof, artifact or launch admission.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}
