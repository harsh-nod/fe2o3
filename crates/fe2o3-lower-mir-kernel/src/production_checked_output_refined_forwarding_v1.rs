//! Same-owner refinement R followed by checked private forwarding R-to-F.
use super::*;
use fe2o3_kernel_analysis::CanonicalKirCrossBlockForwardingLimitsV1 as ForwardingLimits;
use fe2o3_kernel_opt::{
    OwnedCrossBlockForwardingV1 as ForwardingTail,
    prepare_owned_cross_block_forwarding_v1 as prepare_forwarding,
};
use promotion_sites::ProductionCrossBlockForwardingOriginV1 as FinalOrigin;

#[path = "production_checked_output_loop_unroll_v1.rs"]
mod loop_unroll;
pub use loop_unroll::{
    ProductionLoopUnrollErrorV1, ProductionLoopUnrollOriginV1, ProductionLoopUnrollStorageV1,
    ProductionOwnedLoopUnrollContinuationV1, ProductionOwnedUnitLocalLoopUnrollContinuationV1,
};

/// Refusal of the actual sequential refinement/forwarding continuation.
#[derive(Debug)]
pub enum ProductionRefinedCrossBlockForwardingErrorV1 {
    /// Cumulative allocation/work/storage arithmetic or active ledger failed.
    Resource(AssertOriginResourceV1),
    /// The genuine retained source/refinement prefix failed replay.
    Prefix(Box<ProductionInductionRefinementErrorV1>),
    /// Canonical forwarding on the actual refined graph failed.
    Continuation(fe2o3_kernel_opt::OwnedCrossBlockForwardingErrorV1),
    /// Exact source transport or final private/native/formal census failed.
    Admission(Box<ProductionPrivateCellPromotionContinuationErrorV1>),
    /// Stored exact forwarding settings disagree with its actual tail.
    LimitsMismatch,
    /// Fresh complete intermediate or final source lineage differs.
    OriginsMismatch,
    /// Private partial construction unwound without transferring an owner.
    Panicked,
}
type CError = ProductionRefinedCrossBlockForwardingErrorV1;
type CResult<T> = Result<T, CError>;
impl From<AssertOriginResourceV1> for CError {
    fn from(value: AssertOriginResourceV1) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for CError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source-owned refined private forwarding: {self:?}")
    }
}
impl Error for CError {}
fn prefix_error(error: IError) -> CError {
    match error {
        IError::Resource(error) => CError::Resource(error),
        IError::Panicked => CError::Panicked,
        IError::Admission(error) => CError::Admission(error),
        other => CError::Prefix(Box::new(other)),
    }
}
fn binding_check(binding: &PromotionBinding, budget: &AssertOriginBudgetV1<'_>) -> CResult<()> {
    binding.check(budget).map_err(|error| match error {
        PError::Resource(error) => CError::Resource(error),
        PError::Panicked => CError::Panicked,
        other => CError::Admission(Box::new(other)),
    })
}

/// New owning addition only, excluding genuine refinement/source and siblings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionRefinedCrossBlockForwardingStorageV1(usize);
impl ProductionRefinedCrossBlockForwardingStorageV1 {
    /// Reserve before controlled use while the returned owner remains live.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
struct FinalData {
    tail: ForwardingTail,
    origins: Vec<FinalOrigin>,
    kernels: Box<[FormalMemoryObligations]>,
    limits: ForwardingLimits,
    added: usize,
}

/// Genuine Direct L-to-R prefix followed by the actual R-to-F owning relation.
/// The two origin tables stay distinct and join at the exact intermediate owner.
/// Synthetic false never acquires a source statement or artifact authority.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOwnedRefinedCrossBlockForwardingContinuationV1 as Owner;
/// fn duplicate(value: &Owner) -> Owner { value.clone() }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_lower_mir_kernel::ProductionOwnedRefinedCrossBlockForwardingContinuationV1 as Owner;
/// fn detach(value: Owner) { let _ = value.prefix; }
/// ```
pub struct ProductionOwnedRefinedCrossBlockForwardingContinuationV1 {
    prefix: ProductionOwnedInductionRefinementContinuationV1,
    data: FinalData,
}
/// Genuine UnitLocal original source, roots, erasure and both rewrite owners.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1 as New,
///     ProductionOwnedUnitLocalInductionRefinementContinuationV1 as Old};
/// fn relabel(value: New) -> Old { value }
/// ```
pub struct ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1 {
    prefix: ProductionOwnedUnitLocalInductionRefinementContinuationV1,
    data: FinalData,
}
#[derive(Clone, Copy)]
enum RefinedPrefix<'a> {
    Direct(&'a ProductionOwnedInductionRefinementContinuationV1),
    Erased(&'a ProductionOwnedUnitLocalInductionRefinementContinuationV1),
}
impl<'a> RefinedPrefix<'a> {
    fn licm(self) -> LicmPrefix<'a> {
        match self {
            Self::Direct(value) => LicmPrefix::Direct(value.prefix()),
            Self::Erased(value) => LicmPrefix::Erased(value.prefix()),
        }
    }
    fn tail(self) -> &'a RefinementTail {
        match self {
            Self::Direct(value) => value.continuation(),
            Self::Erased(value) => value.continuation(),
        }
    }
    fn output(self) -> &'a StoreOwner {
        self.tail().output()
    }
    fn origins(self) -> &'a [SourceOrigin] {
        match self {
            Self::Direct(value) => value.origins(),
            Self::Erased(value) => value.origins(),
        }
    }
    fn limits(self) -> RefinementLimits {
        match self {
            Self::Direct(value) => value.limits(),
            Self::Erased(value) => value.limits(),
        }
    }
    fn floor(self) -> CResult<usize> {
        match self {
            Self::Direct(value) => value.retained_input_storage_floor_v1(),
            Self::Erased(value) => value.retained_input_storage_floor_v1(),
        }
        .map_err(prefix_error)
    }
    fn replay(self, budget: &mut AssertOriginBudgetV1<'_>) -> CResult<()> {
        match self {
            Self::Direct(value) => value.verify_equivalence(budget),
            Self::Erased(value) => value.verify_equivalence(budget),
        }
        .map_err(prefix_error)
    }
}
fn scoped<'w, T>(
    required: usize,
    budget: &mut AssertOriginBudgetV1<'w>,
    run: impl FnOnce(&mut AssertOriginBudgetV1<'w>, &PromotionBinding) -> CResult<T>,
) -> CResult<T> {
    match super::scoped(required, budget, |budget, binding| Ok(run(budget, binding))) {
        Ok(result) => result,
        Err(error) => Err(prefix_error(error)),
    }
}
fn header<P, W>() -> CResult<usize> {
    size_of::<W>()
        .checked_sub(size_of::<P>())
        .and_then(|bytes| bytes.checked_sub(size_of::<ForwardingTail>()))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn added(data: &FinalData, header: usize) -> CResult<usize> {
    header
        .checked_add(data.tail.retained_storage())
        .and_then(|bytes| {
            bytes.checked_add(
                data.origins
                    .capacity()
                    .checked_mul(size_of::<FinalOrigin>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(std::mem::size_of_val(data.kernels.as_ref())))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn required(prefix: RefinedPrefix<'_>, data: &FinalData, header: usize) -> CResult<usize> {
    if data.added != added(data, header)? {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    prefix
        .floor()?
        .checked_add(data.added)
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn final_rows(count: usize, budget: &mut AssertOriginBudgetV1<'_>) -> CResult<Vec<FinalOrigin>> {
    budget.charge_work(4)?;
    let requested = count
        .checked_mul(size_of::<FinalOrigin>())
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| AssertOriginResourceV1::Allocation)?;
    let actual = rows
        .capacity()
        .checked_mul(size_of::<FinalOrigin>())
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    // Requested capacity was prepaid. Excess is known only after allocation;
    // charge it before initialization/use, dropping backing on any refusal.
    budget.reserve_storage(
        actual
            .checked_sub(requested)
            .ok_or(AssertOriginResourceV1::Accounting)?,
    )?;
    Ok(rows)
}
fn check_actual(
    prefix: RefinedPrefix<'_>,
    tail: &ForwardingTail,
    limits: ForwardingLimits,
    origins: &mut Vec<FinalOrigin>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> CResult<Box<[FormalMemoryObligations]>> {
    with_actual_refined_forwarding_sites(
        prefix,
        tail,
        limits,
        origins,
        budget,
        binding,
        |final_sites, intermediate, refinement, forwarding, budget, binding| {
            promotion_sites::census_refined_forwarding_sites(
                final_sites,
                intermediate,
                refinement,
                forwarding,
                budget,
                binding,
            )
        },
    )
}
fn with_actual_refined_forwarding_sites<'g, 'w>(
    prefix: RefinedPrefix<'g>,
    tail: &'g ForwardingTail,
    limits: ForwardingLimits,
    origins: &mut Vec<FinalOrigin>,
    budget: &mut AssertOriginBudgetV1<'w>,
    binding: &PromotionBinding,
    use_sites: impl for<'s> FnOnce(
        promotion_sites::CheckedPromotedSites<'s, 'g>,
        &'s Inventory<'g>,
        &'s fe2o3_kernel_analysis::CheckedCanonicalKirInductionRefinementV1<'g>,
        &'s fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV1<'g>,
        &mut AssertOriginBudgetV1<'w>,
        &PromotionBinding,
    ) -> PResult<Box<[FormalMemoryObligations]>>,
) -> CResult<Box<[FormalMemoryObligations]>> {
    budget.charge_work(7)?;
    if tail.limits() != limits {
        return Err(CError::LimitsMismatch);
    }
    let (forwarding, receipt) = tail
        .replay_against(prefix.output(), budget)
        .map_err(CError::Continuation)?;
    binding_check(binding, budget)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let (output, receipt) = Inventory::derive(tail.output(), budget)
        .map_err(inventory_error)
        .map_err(PError::from)
        .map_err(|error| CError::Admission(Box::new(error)))?;
    binding_check(binding, budget)?;
    budget.reserve_storage(receipt.retained_storage())?;
    // Both retained F rows and temporary reconstructed L-to-R rows are paid
    // below the UnitLocal callback floor. No callback scratch may escape.
    budget.reserve_storage(size_of::<Vec<SourceOrigin>>())?;
    let mut intermediate_origins =
        super::origin_rows(prefix.origins().len(), budget).map_err(prefix_error)?;
    let kernels = super::with_actual_refinement_sites(
        prefix.licm(),
        prefix.tail(),
        prefix.limits(),
        &mut intermediate_origins,
        budget,
        binding,
        |sites, refinement, budget, binding| {
            let intermediate = sites.output();
            promotion_sites::with_checked_cross_block_forwarding_sites(
                sites,
                &forwarding,
                &output,
                origins,
                budget,
                binding,
                |final_sites, budget, binding| {
                    use_sites(
                        final_sites,
                        intermediate,
                        refinement,
                        &forwarding,
                        budget,
                        binding,
                    )
                },
            )
        },
    )
    .map_err(prefix_error)?;
    binding_check(binding, budget)?;
    budget.charge_work(1)?;
    if intermediate_origins.len() != prefix.origins().len() {
        return Err(CError::OriginsMismatch);
    }
    for (fresh, retained) in intermediate_origins.iter().zip(prefix.origins()) {
        budget.charge_work(size_of::<SourceOrigin>() + 1)?;
        if fresh != retained {
            return Err(CError::OriginsMismatch);
        }
    }
    // Existing promoted-site setup prepays this full unchanged root roster
    // before UnitLocal erasure. Its final F report is the sole escaping Box.
    budget.charge_work(2)?;
    if kernels.len() != tail.output().module().kernels.len()
        || kernels.len() != prefix.output().module().kernels.len()
    {
        return Err(CError::Admission(Box::new(PError::from(E::Formal(
            crate::ProductionFormalMemoryErrorV1::ObligationMismatch,
        )))));
    }
    Ok(kernels)
}
fn prepare_data(
    prefix: RefinedPrefix<'_>,
    limits: ForwardingLimits,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> CResult<FinalData> {
    prefix.replay(budget)?;
    binding_check(binding, budget)?;
    budget.reserve_storage(header)?;
    let tail = prepare_forwarding(prefix.output(), limits, budget).map_err(CError::Continuation)?;
    binding_check(binding, budget)?;
    budget.reserve_storage(tail.retained_storage())?;
    let mut origins = final_rows(tail.origins().len(), budget)?;
    let kernels = check_actual(prefix, &tail, limits, &mut origins, budget, binding)?;
    binding_check(binding, budget)?;
    let mut data = FinalData {
        tail,
        origins,
        kernels,
        limits,
        added: 0,
    };
    data.added = added(&data, header)?;
    budget.charge_work(1)?;
    Ok(data)
}
fn replay_data(
    prefix: RefinedPrefix<'_>,
    data: &FinalData,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> CResult<()> {
    scoped(
        required(prefix, data, header)?,
        budget,
        |budget, binding| {
            budget.charge_work(7)?;
            if data.limits != data.tail.limits() {
                return Err(CError::LimitsMismatch);
            }
            prefix.replay(budget)?;
            binding_check(binding, budget)?;
            budget.reserve_storage(size_of::<Vec<FinalOrigin>>())?;
            let mut origins = final_rows(data.origins.len(), budget)?;
            let kernels = check_actual(
                prefix,
                &data.tail,
                data.limits,
                &mut origins,
                budget,
                binding,
            )?;
            budget.charge_work(1)?;
            if origins.len() != data.origins.len() {
                return Err(CError::OriginsMismatch);
            }
            for (fresh, retained) in origins.iter().zip(&data.origins) {
                budget.charge_work(size_of::<FinalOrigin>() + 1)?;
                if fresh != retained {
                    return Err(CError::OriginsMismatch);
                }
            }
            if kernels != data.kernels {
                return Err(CError::Admission(Box::new(PError::from(E::Formal(
                    crate::ProductionFormalMemoryErrorV1::ObligationMismatch,
                )))));
            }
            binding_check(binding, budget)?;
            budget.charge_work(1)?;
            Ok(())
        },
    )
}
macro_rules! owner {
    ($prefix:ty, $owner:ident, $variant:ident) => {
        impl $prefix {
            /// Consumes actual refined R and forwards only on R, retaining both
            /// independently checked relations and fresh F reports. Reserve the
            /// returned addition. Refusal drops the consumed prefix, preserves
            /// caller-owned incoming credit, and never returns an R fallback.
            pub fn continue_cross_block_forwarding_v1(
                self,
                limits: ForwardingLimits,
                budget: &mut AssertOriginBudgetV1<'_>,
            ) -> CResult<($owner, ProductionRefinedCrossBlockForwardingStorageV1)> {
                let floor = RefinedPrefix::$variant(&self).floor()?;
                scoped(floor, budget, |budget, binding| {
                    let data = prepare_data(
                        RefinedPrefix::$variant(&self),
                        limits,
                        header::<Self, $owner>()?,
                        budget,
                        binding,
                    )?;
                    let receipt = ProductionRefinedCrossBlockForwardingStorageV1(data.added);
                    Ok(($owner { prefix: self, data }, receipt))
                })
            }
        }
        impl $owner {
            /// Actual consumed L-to-R source owner retained once.
            pub const fn prefix(&self) -> &$prefix {
                &self.prefix
            }
            /// Actual independently checked R-to-F canonical owner.
            pub const fn continuation(&self) -> &ForwardingTail {
                &self.data.tail
            }
            /// Actual final F, never relabeled R or historical P7 output.
            pub fn output(&self) -> &StoreOwner {
                self.data.tail.output()
            }
            /// Complete R-to-F rows retaining each Load's own source statement.
            pub fn origins(&self) -> &[FinalOrigin] {
                &self.data.origins
            }
            /// Complete L-to-R rows, including synthetic-overflow definitions.
            pub fn refinement_origins(&self) -> &[ProductionInductionRefinementOriginV1] {
                self.prefix.origins()
            }
            /// Fresh actual-final reports in original root order.
            pub fn kernels(&self) -> &[FormalMemoryObligations] {
                &self.data.kernels
            }
            /// Exact forwarding limits, without widening or clamping.
            pub const fn limits(&self) -> ForwardingLimits {
                self.data.limits
            }
            /// Exact refinement limits retained by the actual prefix.
            pub const fn refinement_limits(&self) -> RefinementLimits {
                self.prefix.limits()
            }
            /// Unreserved addition excluding all inherited source/owner backing.
            pub const fn additional_retained_storage_v1(&self) -> usize {
                self.data.added
            }
            /// Minimum actual refinement floor plus the complete new addition.
            pub fn retained_input_storage_floor_v1(&self) -> CResult<usize> {
                required(
                    RefinedPrefix::$variant(&self.prefix),
                    &self.data,
                    header::<$prefix, Self>()?,
                )
            }
            /// No source admission, numbered policy, artifact or launch grant.
            pub const fn grants_artifact_or_launch_authority(&self) -> bool {
                false
            }
            /// Replays the actual prefix, both pair endpoints, complete source
            /// joins and fresh F census on the cumulative active ledger.
            pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> CResult<()> {
                replay_data(
                    RefinedPrefix::$variant(&self.prefix),
                    &self.data,
                    header::<$prefix, Self>()?,
                    budget,
                )
            }
        }
    };
}
owner!(
    ProductionOwnedInductionRefinementContinuationV1,
    ProductionOwnedRefinedCrossBlockForwardingContinuationV1,
    Direct
);
owner!(
    ProductionOwnedUnitLocalInductionRefinementContinuationV1,
    ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1,
    Erased
);

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use std::cell::Cell;

    fn formal_refusal(result: CResult<()>) {
        match result {
            Err(CError::Admission(error)) => assert!(matches!(
                *error,
                PError::Admission(E::Formal(
                    crate::ProductionFormalMemoryErrorV1::ObligationMismatch
                ))
            )),
            other => panic!("fresh complete final report: {other:?}"),
        }
    }
    fn hostile(
        prefix: RefinedPrefix<'_>,
        data: &mut FinalData,
        header: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        data.origins.swap(0, 1);
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(CError::OriginsMismatch)
        ));
        data.origins.swap(0, 1);
        let last_index = data.origins.len() - 1;
        let last = data.origins[last_index];
        data.origins[last_index] = data.origins[0];
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(CError::OriginsMismatch)
        ));
        data.origins[last_index] = last;
        if let Some(false_site) = prefix.origins().iter().find_map(|row| match row.canonical_origin() {
            fe2o3_kernel_analysis::CanonicalKirInductionRefinementOriginV1::CheckedAddSplit { false_output, .. } => Some(false_output),
            _ => None,
        }) {
            let index = data.origins.iter().position(|row| row.canonical_origin().input == false_site).unwrap();
            let original = data.origins[index];
            assert_eq!(original.original_source_statement(), None);
            let other = *data.origins.iter().find(|row| row.original_source_statement().is_some()).unwrap();
            data.origins[index] = original.other_source_for_forwarding_test(other);
            assert!(matches!(replay_data(prefix, data, header, budget), Err(CError::OriginsMismatch)));
            data.origins[index] = original;
        }
        if let Some(index) = data
            .origins
            .iter()
            .position(|row| row.canonical_origin().store.is_some())
        {
            let original = data.origins[index];
            data.origins[index] = original.changed_source_for_forwarding_test();
            assert!(matches!(
                replay_data(prefix, data, header, budget),
                Err(CError::OriginsMismatch)
            ));
            let store = original.canonical_origin().store.unwrap();
            let store_row = *data
                .origins
                .iter()
                .find(|row| row.canonical_origin().input == store)
                .unwrap();
            assert_ne!(
                original.original_source_statement(),
                store_row.original_source_statement()
            );
            data.origins[index] = original.other_source_for_forwarding_test(store_row);
            assert!(matches!(
                replay_data(prefix, data, header, budget),
                Err(CError::OriginsMismatch)
            ));
            data.origins[index] = original;
        }
        let limits = data.limits;
        data.limits.memory.blocks += 1;
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(CError::LimitsMismatch)
        ));
        data.limits = limits;
        let reports = std::mem::take(&mut data.kernels);
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(CError::Resource(AssertOriginResourceV1::Accounting))
        ));
        let bytes = std::mem::size_of_val(reports.as_ref());
        data.added -= bytes;
        formal_refusal(replay_data(prefix, data, header, budget));
        data.added += bytes;
        data.kernels = reports;
        if data.kernels.len() == 2 {
            data.kernels.swap(0, 1);
            formal_refusal(replay_data(prefix, data, header, budget));
            data.kernels.swap(0, 1);
            let duplicate = data.kernels[0].clone();
            let second = std::mem::replace(&mut data.kernels[1], duplicate);
            formal_refusal(replay_data(prefix, data, header, budget));
            data.kernels[1] = second;
        }
        data.added += 1;
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(CError::Resource(AssertOriginResourceV1::Accounting))
        ));
        data.added -= 1;
        replay_data(prefix, data, header, budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
    fn parity(prefix: RefinedPrefix<'_>) {
        let measure = |split: bool| {
            let mut work = Work::new(1_000_000_000);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000_000);
            let floor = prefix.floor().unwrap();
            budget.reserve_storage(floor).unwrap();
            super::super::scoped(floor, &mut budget, |budget, binding| {
                budget.reserve_storage(size_of::<Vec<SourceOrigin>>())?;
                let mut rows = super::super::origin_rows(prefix.origins().len(), budget)?;
                let reports = if split {
                    super::super::with_actual_refinement_sites(
                        prefix.licm(),
                        prefix.tail(),
                        prefix.limits(),
                        &mut rows,
                        budget,
                        binding,
                        |sites, pair, budget, binding| {
                            promotion_sites::census_induction_refinement_sites(
                                sites, pair, budget, binding,
                            )
                        },
                    )?
                } else {
                    super::super::check_actual(
                        prefix.licm(),
                        prefix.tail(),
                        prefix.limits(),
                        &mut rows,
                        budget,
                        binding,
                    )?
                };
                assert_eq!(rows, prefix.origins());
                let retained = match prefix {
                    RefinedPrefix::Direct(owner) => owner.kernels(),
                    RefinedPrefix::Erased(owner) => owner.kernels(),
                };
                assert_eq!(reports.as_ref(), retained);
                drop(reports);
                Ok(())
            })
            .unwrap();
            assert_eq!(budget.storage(), floor);
            (budget.work(), budget.peak_storage())
        };
        assert_eq!(measure(false), measure(true));
    }
    fn partial(prefix: RefinedPrefix<'_>, header: usize, budget: &mut AssertOriginBudgetV1<'_>) {
        struct Candidate<'a> {
            data: Option<FinalData>,
            dropped: &'a Cell<bool>,
        }
        impl Drop for Candidate<'_> {
            fn drop(&mut self) {
                let data = self.data.take().unwrap();
                assert_eq!(data.origins.len(), data.tail.origins().len());
                assert_eq!(
                    data.kernels.len(),
                    data.tail.output().module().kernels.len()
                );
                drop(data);
                self.dropped.set(true);
            }
        }
        let sibling = vec![0x89u8; 43];
        let bytes = size_of::<Vec<u8>>() + sibling.capacity();
        budget.reserve_storage(bytes).unwrap();
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        for unwind in [false, true] {
            let dropped = Cell::new(false);
            let result: CResult<()> = scoped(prefix.floor().unwrap(), budget, |budget, binding| {
                let data =
                    prepare_data(prefix, ForwardingLimits::default(), header, budget, binding)?;
                let _live = Candidate {
                    data: Some(data),
                    dropped: &dropped,
                };
                if unwind {
                    panic!("actual complete refined-forwarding candidate");
                }
                Err(CError::OriginsMismatch)
            });
            if unwind {
                assert!(matches!(result, Err(CError::Panicked)));
            } else {
                assert!(matches!(result, Err(CError::OriginsMismatch)));
            }
            assert!(dropped.get());
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(sibling, [0x89; 43]);
        }
        drop(sibling);
        budget.release_storage(bytes).unwrap();
    }
    macro_rules! helpers {
        ($prefix:ty, $owner:ty, $variant:ident) => {
            impl $prefix {
                pub(crate) fn exercise_refined_forwarding_callbacks_v1(&self, budget: &mut AssertOriginBudgetV1<'_>) {
                    parity(RefinedPrefix::$variant(self));
                    partial(RefinedPrefix::$variant(self), header::<Self, $owner>().unwrap(), budget);
                }
            }
            impl $owner {
                pub(crate) fn exercise_refined_forwarding_hostile_v1(&mut self, budget: &mut AssertOriginBudgetV1<'_>) {
                    hostile(RefinedPrefix::$variant(&self.prefix), &mut self.data,
                        header::<$prefix, Self>().unwrap(), budget);
                    let original = self.prefix.data.origins[0];
                    self.prefix.data.origins[0] = original.changed_overflow_for_induction_test();
                    assert!(matches!(self.verify_equivalence(budget), Err(CError::Prefix(error))
                        if matches!(*error, IError::OriginsMismatch)));
                    self.prefix.data.origins[0] = original;
                    self.verify_equivalence(budget).unwrap();
                }
            }
        };
    }
    helpers!(
        ProductionOwnedInductionRefinementContinuationV1,
        ProductionOwnedRefinedCrossBlockForwardingContinuationV1,
        Direct
    );
    helpers!(
        ProductionOwnedUnitLocalInductionRefinementContinuationV1,
        ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1,
        Erased
    );

    #[test]
    fn refined_forwarding_source_headers_and_actual_row_capacity_are_separately_paid() {
        let direct = header::<
            ProductionOwnedInductionRefinementContinuationV1,
            ProductionOwnedRefinedCrossBlockForwardingContinuationV1,
        >()
        .unwrap();
        let erased = header::<
            ProductionOwnedUnitLocalInductionRefinementContinuationV1,
            ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1,
        >()
        .unwrap();
        assert_eq!(
            size_of::<ProductionOwnedRefinedCrossBlockForwardingContinuationV1>(),
            size_of::<ProductionOwnedInductionRefinementContinuationV1>()
                + size_of::<ForwardingTail>()
                + direct
        );
        assert_eq!(
            size_of::<ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1>(),
            size_of::<ProductionOwnedUnitLocalInductionRefinementContinuationV1>()
                + size_of::<ForwardingTail>()
                + erased
        );
        for bytes in [direct, erased] {
            assert!(
                bytes
                    >= size_of::<Vec<FinalOrigin>>() + size_of::<Box<[FormalMemoryObligations]>>()
            );
            let mut work = Work::new(100_000);
            let mut budget = AssertOriginBudgetV1::new(&mut work, bytes - 1);
            let result: CResult<()> = scoped(0, &mut budget, |budget, _| {
                budget.reserve_storage(bytes)?;
                panic!("header refusal precedes any backing allocation")
            });
            match result {
                Err(CError::Resource(AssertOriginResourceV1::Storage(error))) => {
                    assert_eq!((error.actual(), error.limit()), (bytes, bytes - 1))
                }
                other => panic!("exact composed header refusal: {other:?}"),
            }
            assert_eq!(
                (
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_storage()
                ),
                (0, 0, 0, Some(bytes))
            );
        }
        let mut work = Work::new(100_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000);
        scoped(0, &mut budget, |budget, _| -> CResult<()> {
            budget.reserve_storage(size_of::<Vec<FinalOrigin>>())?;
            let rows = final_rows(11, budget)?;
            assert_eq!(
                budget.storage(),
                size_of::<Vec<FinalOrigin>>() + rows.capacity() * size_of::<FinalOrigin>()
            );
            drop(rows);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), 0);
    }
    #[test]
    fn refined_forwarding_source_backing_short_and_size_overflow_have_exact_errors() {
        let bytes = 11 * size_of::<FinalOrigin>();
        let mut work = Work::new(100_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, bytes - 1);
        match final_rows(11, &mut budget) {
            Err(CError::Resource(AssertOriginResourceV1::Storage(error))) => {
                assert_eq!((error.actual(), error.limit()), (bytes, bytes - 1))
            }
            _ => panic!("exact requested row backing refusal"),
        }
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            (4, 0, 0, Some(bytes))
        );
        assert!(matches!(
            final_rows(usize::MAX, &mut budget),
            Err(CError::Resource(AssertOriginResourceV1::Arithmetic))
        ));
        assert_eq!(
            (budget.work(), budget.storage(), budget.failed_storage()),
            (8, 0, Some(bytes))
        );
    }
}
