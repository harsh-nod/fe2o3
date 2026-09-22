//! Consumed genuine F source and an independently replayed bounded F-to-U clone.
use super::*;
use fe2o3_kernel_analysis::CanonicalKirLoopUnrollLimitsV1 as UnrollLimits;
use fe2o3_kernel_opt::{
    OwnedLoopUnrollStorageV1 as UnrollStorage, OwnedLoopUnrollV1 as UnrollTail,
    unroll_canonical_kir_loops_v1 as unroll,
};
pub use promotion_sites::ProductionLoopUnrollOriginV1;
type UnrollOrigin = ProductionLoopUnrollOriginV1;

#[derive(Debug)]
/// Refusal of a genuine source-owned F-to-U continuation, never an F fallback.
pub enum ProductionLoopUnrollErrorV1 {
    /// Cumulative work/storage arithmetic or active-ledger custody failed.
    Resource(AssertOriginResourceV1),
    /// The consumed source/refinement/forwarding prefix failed replay.
    Prefix(Box<ProductionRefinedCrossBlockForwardingErrorV1>),
    /// Canonical unroll selection, construction or independent pair failed.
    Continuation(fe2o3_kernel_opt::OwnedLoopUnrollErrorV1),
    /// Complete source transport or fresh U memory/native census failed.
    Admission(Box<ProductionPrivateCellPromotionContinuationErrorV1>),
    /// The unchanged source operation ceiling cannot cover requested U growth.
    SourceOperationsLimit {
        /// Requested ceiling, never clamped silently.
        requested: usize,
        /// Ceiling from the actual retained source owner.
        source_limit: usize,
    },
    /// Stored settings differ from the actual canonical tail.
    LimitsMismatch,
    /// Independently reconstructed complete source origins differ.
    OriginsMismatch,
    /// A private construction scope unwound without transferring an owner.
    Panicked,
}
type UError = ProductionLoopUnrollErrorV1;
type UResult<T> = Result<T, UError>;
impl From<AssertOriginResourceV1> for UError {
    fn from(value: AssertOriginResourceV1) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for UError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source-owned bounded loop unroll: {self:?}")
    }
}
impl Error for UError {}
fn prefix_error(error: CError) -> UError {
    match error {
        CError::Resource(error) => UError::Resource(error),
        CError::Panicked => UError::Panicked,
        CError::Admission(error) => UError::Admission(error),
        other => UError::Prefix(Box::new(other)),
    }
}
fn binding_check(binding: &PromotionBinding, budget: &AssertOriginBudgetV1<'_>) -> UResult<()> {
    super::binding_check(binding, budget).map_err(prefix_error)
}
fn scoped<'w, T>(
    required: usize,
    budget: &mut AssertOriginBudgetV1<'w>,
    run: impl FnOnce(&mut AssertOriginBudgetV1<'w>, &PromotionBinding) -> UResult<T>,
) -> UResult<T> {
    match super::scoped(required, budget, |budget, binding| Ok(run(budget, binding))) {
        Ok(result) => result,
        Err(error) => Err(prefix_error(error)),
    }
}

/// New addition only. Reserve it before controlled use of the returned owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionLoopUnrollStorageV1(usize);
impl ProductionLoopUnrollStorageV1 {
    /// Unreserved addition excluding all inherited source and sibling backing.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
struct UnrollData {
    tail: UnrollTail,
    tail_storage: UnrollStorage,
    origins: Vec<UnrollOrigin>,
    kernels: Box<[FormalMemoryObligations]>,
    limits: UnrollLimits,
    added: usize,
}

/// Actual Direct F retained once with U, complete source rows and fresh reports.
/// Existing opaque source/formal engine allocation exclusions are unchanged;
/// every new wrapper/header/vector backing introduced here is accounted.
/// Formal-report deep equality retains the existing comparison-work exclusion;
/// the paid report Box extent is not opaque report-internal allocator capacity.
/// This is neither a native output nor a numbered-policy/artifact authority.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOwnedLoopUnrollContinuationV1 as Owner;
/// fn duplicate(owner: &Owner) -> Owner { owner.clone() }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_lower_mir_kernel::ProductionOwnedLoopUnrollContinuationV1 as Owner;
/// fn detach(owner: Owner) { let _ = owner.prefix; }
/// ```
pub struct ProductionOwnedLoopUnrollContinuationV1 {
    prefix: ProductionOwnedRefinedCrossBlockForwardingContinuationV1,
    data: UnrollData,
}
/// UnitLocal original source, erasure, F and U remain in one move-only owner.
/// It uses the same documented accounting domain as the Direct owner above.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionOwnedUnitLocalLoopUnrollContinuationV1 as New,
///     ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1 as Old};
/// fn relabel(owner: New) -> Old { owner }
/// ```
pub struct ProductionOwnedUnitLocalLoopUnrollContinuationV1 {
    prefix: ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1,
    data: UnrollData,
}
#[derive(Clone, Copy)]
enum FinalPrefix<'a> {
    Direct(&'a ProductionOwnedRefinedCrossBlockForwardingContinuationV1),
    Erased(&'a ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1),
}
impl<'a> FinalPrefix<'a> {
    fn refined(self) -> RefinedPrefix<'a> {
        match self {
            Self::Direct(value) => RefinedPrefix::Direct(value.prefix()),
            Self::Erased(value) => RefinedPrefix::Erased(value.prefix()),
        }
    }
    fn tail(self) -> &'a ForwardingTail {
        match self {
            Self::Direct(v) => v.continuation(),
            Self::Erased(v) => v.continuation(),
        }
    }
    fn output(self) -> &'a StoreOwner {
        self.tail().output()
    }
    fn origins(self) -> &'a [FinalOrigin] {
        match self {
            Self::Direct(v) => v.origins(),
            Self::Erased(v) => v.origins(),
        }
    }
    fn limits(self) -> ForwardingLimits {
        match self {
            Self::Direct(v) => v.limits(),
            Self::Erased(v) => v.limits(),
        }
    }
    fn floor(self) -> UResult<usize> {
        match self {
            Self::Direct(v) => v.retained_input_storage_floor_v1(),
            Self::Erased(v) => v.retained_input_storage_floor_v1(),
        }
        .map_err(prefix_error)
    }
    fn replay(self, budget: &mut AssertOriginBudgetV1<'_>) -> UResult<()> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
        .map_err(prefix_error)
    }
}
fn check_limits(
    prefix: FinalPrefix<'_>,
    limits: UnrollLimits,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> UResult<()> {
    budget.charge_work(3)?;
    let source_limit = prefix.refined().licm().source_cap();
    if limits.loops.operations > source_limit {
        return Err(UError::SourceOperationsLimit {
            requested: limits.loops.operations,
            source_limit,
        });
    }
    Ok(())
}
fn header<P, W>() -> UResult<usize> {
    size_of::<W>()
        .checked_sub(size_of::<P>())
        .and_then(|n| n.checked_sub(size_of::<UnrollTail>()))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn added(data: &UnrollData, header: usize) -> UResult<usize> {
    header
        .checked_add(data.tail_storage.retained_storage())
        .and_then(|n| {
            n.checked_add(
                data.origins
                    .capacity()
                    .checked_mul(size_of::<UnrollOrigin>())?,
            )
        })
        .and_then(|n| n.checked_add(std::mem::size_of_val(data.kernels.as_ref())))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn required(prefix: FinalPrefix<'_>, data: &UnrollData, header: usize) -> UResult<usize> {
    if data.added != added(data, header)? {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    prefix
        .floor()?
        .checked_add(data.added)
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn rows(count: usize, budget: &mut AssertOriginBudgetV1<'_>) -> UResult<Vec<UnrollOrigin>> {
    budget.charge_work(4)?;
    let requested = count
        .checked_mul(size_of::<UnrollOrigin>())
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let mut value = Vec::new();
    value
        .try_reserve_exact(count)
        .map_err(|_| AssertOriginResourceV1::Allocation)?;
    let actual = value
        .capacity()
        .checked_mul(size_of::<UnrollOrigin>())
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    budget.reserve_storage(
        actual
            .checked_sub(requested)
            .ok_or(AssertOriginResourceV1::Accounting)?,
    )?;
    Ok(value)
}
fn check_actual(
    prefix: FinalPrefix<'_>,
    tail: &UnrollTail,
    limits: UnrollLimits,
    origins: &mut Vec<UnrollOrigin>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> UResult<Box<[FormalMemoryObligations]>> {
    let kernels = with_actual_unrolled_sites_v1(
        prefix,
        tail,
        limits,
        origins,
        budget,
        binding,
        |sites, intermediate, input, refinement, forwarding, pair, budget, binding| {
            promotion_sites::census_loop_unroll_sites(
                sites,
                intermediate,
                input,
                refinement,
                forwarding,
                pair,
                budget,
                binding,
            )
        },
    )?;
    budget.charge_work(2)?;
    if kernels.len() != tail.output().module().kernels.len() {
        return Err(UError::Admission(Box::new(PError::from(E::Formal(
            crate::ProductionFormalMemoryErrorV1::ObligationMismatch,
        )))));
    }
    Ok(kernels)
}
#[allow(clippy::too_many_arguments)]
fn with_actual_unrolled_sites_v1<'w>(
    prefix: FinalPrefix<'_>,
    tail: &UnrollTail,
    limits: UnrollLimits,
    origins: &mut Vec<UnrollOrigin>,
    budget: &mut AssertOriginBudgetV1<'w>,
    binding: &PromotionBinding,
    next: impl FnOnce(
        CheckedPromotedSites<'_, '_>,
        &CanonicalKirInventoryV1<'_>,
        &CanonicalKirInventoryV1<'_>,
        &fe2o3_kernel_analysis::CheckedCanonicalKirInductionRefinementV1<'_>,
        &fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV1<'_>,
        &fe2o3_kernel_analysis::CheckedCanonicalKirLoopUnrollPairV1<'_, '_, '_>,
        &mut AssertOriginBudgetV1<'w>,
        &PromotionBinding,
    ) -> PResult<Box<[FormalMemoryObligations]>>,
) -> UResult<Box<[FormalMemoryObligations]>> {
    budget.charge_work(7)?;
    check_limits(prefix, limits, budget)?;
    if tail.limits() != limits {
        return Err(UError::LimitsMismatch);
    }
    let (pair, receipt) = tail
        .replay(prefix.output(), limits, budget)
        .map_err(UError::Continuation)?;
    binding_check(binding, budget)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let (output, receipt) = Inventory::derive(tail.output(), budget)
        .map_err(inventory_error)
        .map_err(PError::from)
        .map_err(|e| UError::Admission(Box::new(e)))?;
    binding_check(binding, budget)?;
    budget.reserve_storage(receipt.retained_storage())?;
    // Both retained U rows and fresh F reconstruction precede UnitLocal scratch.
    budget.reserve_storage(size_of::<Vec<FinalOrigin>>())?;
    let mut previous = super::final_rows(prefix.origins().len(), budget).map_err(prefix_error)?;
    let kernels = super::with_actual_refined_forwarding_sites(
        prefix.refined(),
        prefix.tail(),
        prefix.limits(),
        &mut previous,
        budget,
        binding,
        |sites, intermediate, refinement, forwarding, budget, binding| {
            promotion_sites::with_checked_loop_unroll_sites(
                sites,
                intermediate,
                refinement,
                forwarding,
                &pair,
                &output,
                origins,
                budget,
                binding,
                next,
            )
        },
    )
    .map_err(prefix_error)?;
    binding_check(binding, budget)?;
    budget.charge_work(1)?;
    if previous.len() != prefix.origins().len() {
        return Err(UError::OriginsMismatch);
    }
    for (fresh, old) in previous.iter().zip(prefix.origins()) {
        budget.charge_work(size_of::<FinalOrigin>() + 1)?;
        if fresh != old {
            return Err(UError::OriginsMismatch);
        }
    }
    Ok(kernels)
}
fn prepare_data(
    prefix: FinalPrefix<'_>,
    limits: UnrollLimits,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> UResult<UnrollData> {
    check_limits(prefix, limits, budget)?;
    prefix.replay(budget)?;
    binding_check(binding, budget)?;
    budget.reserve_storage(header)?;
    let (tail, receipt) = unroll(prefix.output(), limits, budget).map_err(UError::Continuation)?;
    binding_check(binding, budget)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let mut origins = rows(tail.origins().operations.len(), budget)?;
    let kernels = check_actual(prefix, &tail, limits, &mut origins, budget, binding)?;
    binding_check(binding, budget)?;
    let mut data = UnrollData {
        tail,
        tail_storage: receipt,
        origins,
        kernels,
        limits,
        added: 0,
    };
    data.added = added(&data, header)?;
    budget.charge_work(1)?;
    Ok(data)
}

impl ProductionOwnedLoopUnrollContinuationV1 {
    // Namespace-only private facade to the shared promotion/source helpers.
    // No live-U owner, sealed producer witness or source boolean is fabricated.
    pub(crate) fn check_decoded_expanded_sites_v1(
        source: CanonicalOutputFormalSourceAnchorV1<'_>,
        history: &fe2o3_kernel_opt::CheckedLoopUnrollHistoryV1<'_>,
        scalar: &fe2o3_kernel_opt::CheckedScalarFixedPointOwnerV1,
        origins: &mut Vec<ProductionExpandedSourceOriginV1>,
        required: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> UResult<Box<[FormalMemoryObligations]>> {
        Self::check_replayed_expanded_sites_v1(
            source,
            history,
            ExpandedScalarViewV1::Live(scalar),
            origins,
            required,
            budget,
        )
    }

    pub(in crate::production_semantic_kir_v1::checked_output_admission_policy3_v1::general) fn check_replayed_expanded_sites_v1(
        source: CanonicalOutputFormalSourceAnchorV1<'_>,
        history: &fe2o3_kernel_opt::CheckedLoopUnrollHistoryV1<'_>,
        scalar: ExpandedScalarViewV1<'_, '_, '_, '_>,
        origins: &mut Vec<ProductionExpandedSourceOriginV1>,
        required: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> UResult<Box<[FormalMemoryObligations]>> {
        scoped(required, budget, |budget, binding| {
            let source = match source {
                CanonicalOutputFormalSourceAnchorV1::Direct(v) => GeneralSourceContextV1::Direct(v),
                CanonicalOutputFormalSourceAnchorV1::Erased(v) => GeneralSourceContextV1::Erased(v),
            };
            let reports = scalar
                .output()
                .module()
                .kernels
                .len()
                .checked_mul(size_of::<FormalMemoryObligations>())
                .ok_or(AssertOriginResourceV1::Arithmetic)?;
            budget.reserve_storage(reports)?;
            promotion_sites::check_decoded_expanded_sites_v1(
                source, history, scalar, origins, required, budget, binding,
            )
            .map_err(|error| UError::Admission(Box::new(error)))
        })
    }
}
fn replay_data(
    prefix: FinalPrefix<'_>,
    data: &UnrollData,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> UResult<()> {
    scoped(
        required(prefix, data, header)?,
        budget,
        |budget, binding| {
            budget.charge_work(7)?;
            if data.limits != data.tail.limits() {
                return Err(UError::LimitsMismatch);
            }
            if data.origins.len() != data.tail.origins().operations.len() {
                return Err(UError::OriginsMismatch);
            }
            prefix.replay(budget)?;
            binding_check(binding, budget)?;
            budget.reserve_storage(size_of::<Vec<UnrollOrigin>>())?;
            let mut origins = rows(data.origins.len(), budget)?;
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
                return Err(UError::OriginsMismatch);
            }
            for (fresh, retained) in origins.iter().zip(&data.origins) {
                budget.charge_work(size_of::<UnrollOrigin>() + 1)?;
                if fresh != retained {
                    return Err(UError::OriginsMismatch);
                }
            }
            if kernels != data.kernels {
                return Err(UError::Admission(Box::new(PError::from(E::Formal(
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
            /// Consumes F; a refusal never returns an F fallback. Incoming paid
            /// prefix/sibling credit stays caller-owned even after consumption.
            pub fn continue_bounded_loop_unroll_v1(
                self,
                limits: UnrollLimits,
                budget: &mut AssertOriginBudgetV1<'_>,
            ) -> UResult<($owner, ProductionLoopUnrollStorageV1)> {
                let floor = FinalPrefix::$variant(&self).floor()?;
                scoped(floor, budget, |budget, binding| {
                    let data = prepare_data(
                        FinalPrefix::$variant(&self),
                        limits,
                        header::<Self, $owner>()?,
                        budget,
                        binding,
                    )?;
                    let receipt = ProductionLoopUnrollStorageV1(data.added);
                    Ok(($owner { prefix: self, data }, receipt))
                })
            }
        }
        impl $owner {
            pub(crate) fn expanded_source_anchor_v1(
                &self,
            ) -> CanonicalOutputFormalSourceAnchorV1<'_> {
                let source = FinalPrefix::$variant(&self.prefix)
                    .refined()
                    .licm()
                    .preheaders()
                    .promoted()
                    .p8()
                    .historical()
                    .source();
                match source {
                    GeneralSourceContextV1::Direct(source) => {
                        CanonicalOutputFormalSourceAnchorV1::Direct(source)
                    }
                    GeneralSourceContextV1::Erased(source) => {
                        CanonicalOutputFormalSourceAnchorV1::Erased(source)
                    }
                }
            }
            pub(crate) fn check_expanded_source_v1(
                &self,
                core: &fe2o3_kernel_opt::CheckedScalarFixedPointOwnerV1,
                final_origins: &mut Vec<ProductionExpandedSourceOriginV1>,
                budget: &mut AssertOriginBudgetV1<'_>,
            ) -> UResult<Box<[FormalMemoryObligations]>> {
                scoped(
                    self.retained_input_storage_floor_v1()?,
                    budget,
                    |budget, binding| {
                        self.verify_equivalence(budget)?;
                        binding_check(binding, budget)?;
                        budget.reserve_storage(size_of::<Vec<UnrollOrigin>>())?;
                        let mut origins = rows(self.origins().len(), budget)?;
                        let reports = with_actual_unrolled_sites_v1(
                            FinalPrefix::$variant(&self.prefix),
                            &self.data.tail,
                            self.data.limits,
                            &mut origins,
                            budget,
                            binding,
                            |sites,
                             intermediate,
                             input,
                             refinement,
                             forwarding,
                             pair,
                             budget,
                             binding| {
                                let reports = check_expanded_source_v1(
                                    sites,
                                    intermediate,
                                    input,
                                    refinement,
                                    forwarding,
                                    pair,
                                    ExpandedScalarViewV1::Live(core),
                                    final_origins,
                                    budget,
                                )?;
                                binding.check(budget)?;
                                Ok(reports)
                            },
                        )?;
                        budget.charge_work(
                            origins
                                .len()
                                .checked_mul(size_of::<UnrollOrigin>())
                                .and_then(|n| n.checked_add(1))
                                .ok_or(AssertOriginResourceV1::Arithmetic)?,
                        )?;
                        if origins != self.data.origins {
                            return Err(UError::OriginsMismatch);
                        }
                        Ok(reports)
                    },
                )
            }
            /// Actual consumed source-bearing final-F owner, retained once.
            pub const fn prefix(&self) -> &$prefix {
                &self.prefix
            }
            /// Canonical owning tail with all six complete clone rosters.
            pub const fn continuation(&self) -> &UnrollTail {
                &self.data.tail
            }
            /// Actual U, never relabeled F or historical P8.
            pub fn output(&self) -> &StoreOwner {
                self.data.tail.output()
            }
            /// Complete operation source lineage, including explicit omissions.
            pub fn origins(&self) -> &[UnrollOrigin] {
                &self.data.origins
            }
            /// Fresh U-derived reports in the unchanged original root order.
            pub fn kernels(&self) -> &[FormalMemoryObligations] {
                &self.data.kernels
            }
            /// Exact component limits, with no widening or clamping.
            pub const fn limits(&self) -> UnrollLimits {
                self.data.limits
            }
            /// Added retained ownership excluding the actual F prefix.
            pub const fn additional_retained_storage_v1(&self) -> usize {
                self.data.added
            }
            /// Checked actual prefix floor plus the complete new addition.
            pub fn retained_input_storage_floor_v1(&self) -> UResult<usize> {
                required(
                    FinalPrefix::$variant(&self.prefix),
                    &self.data,
                    header::<$prefix, Self>()?,
                )
            }
            /// This owner grants no artifact, execution or launch authority.
            pub const fn grants_artifact_or_launch_authority(&self) -> bool {
                false
            }
            /// Replays F, the actual F/U pair, complete source joins and U census.
            pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> UResult<()> {
                replay_data(
                    FinalPrefix::$variant(&self.prefix),
                    &self.data,
                    header::<$prefix, Self>()?,
                    budget,
                )
            }
        }
    };
}
owner!(
    ProductionOwnedRefinedCrossBlockForwardingContinuationV1,
    ProductionOwnedLoopUnrollContinuationV1,
    Direct
);
owner!(
    ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1,
    ProductionOwnedUnitLocalLoopUnrollContinuationV1,
    Erased
);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_unroll_nested_refusal_and_unwind_preserve_rows_and_prior_denial() {
        struct Live<'a> {
            rows: Vec<UnrollOrigin>,
            dropped: &'a std::cell::Cell<bool>,
        }
        impl Drop for Live<'_> {
            fn drop(&mut self) {
                assert!(self.rows.capacity() >= 7);
                self.dropped.set(true);
            }
        }
        for unwind in [false, true] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
            work.charge_work(17).unwrap();
            let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000);
            let sibling = vec![0x73u8; 31];
            budget
                .reserve_storage(std::mem::size_of_val(&sibling) + sibling.capacity())
                .unwrap();
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            assert!(budget.reserve_storage(1_000_000).is_err());
            let failed = budget.failed_storage();
            let dropped = std::cell::Cell::new(false);
            let result: UResult<()> = scoped(floor, &mut budget, |budget, binding| {
                budget.reserve_storage(size_of::<Live<'_>>())?;
                let live = Live {
                    rows: rows(7, budget)?,
                    dropped: &dropped,
                };
                binding_check(binding, budget)?;
                let nested: UResult<()> = scoped(budget.storage(), budget, |budget, _| {
                    budget.reserve_storage(19)?;
                    if unwind {
                        panic!("partial source unroll row construction");
                    }
                    Err(UError::OriginsMismatch)
                });
                drop(live);
                nested
            });
            if unwind {
                assert!(matches!(result, Err(UError::Panicked)));
            } else {
                assert!(matches!(result, Err(UError::OriginsMismatch)));
            }
            assert!(dropped.get());
            assert_eq!((budget.storage(), budget.failed_storage()), (floor, failed));
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(sibling, [0x73; 31]);
        }
    }
    fn hostile(
        prefix: FinalPrefix<'_>,
        data: &mut UnrollData,
        header: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let index = data
            .origins
            .iter()
            .position(|row| row.original_source_statement().is_some())
            .unwrap();
        let original = data.origins[index];
        data.origins[index] = original.without_source_for_unroll_test();
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(UError::OriginsMismatch)
        ));
        data.origins[index] = original;
        let donor = *data
            .origins
            .iter()
            .find(|r| {
                r.original_source_statement().is_some()
                    && r.original_source_statement() != original.original_source_statement()
            })
            .unwrap();
        for changed in [
            original.donor_source_for_unroll_test(donor),
            original.wrong_copy_for_unroll_test(),
            original.wrong_root_for_unroll_test(),
        ] {
            data.origins[index] = changed;
            assert!(matches!(
                replay_data(prefix, data, header, budget),
                Err(UError::OriginsMismatch)
            ));
            data.origins[index] = original;
        }
        let removed = data.origins.pop().unwrap();
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(UError::OriginsMismatch)
        ));
        data.origins.push(removed);
        let old = data.origins[1];
        data.origins[1] = data.origins[0];
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(UError::OriginsMismatch)
        ));
        data.origins[1] = old;
        data.origins.swap(0, 1);
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(UError::OriginsMismatch)
        ));
        data.origins.swap(0, 1);
        let old_limits = data.limits;
        data.limits.max_iterations = 7;
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(UError::LimitsMismatch)
        ));
        data.limits = old_limits;
        let old = data.added;
        data.added += 1;
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(UError::Resource(AssertOriginResourceV1::Accounting))
        ));
        data.added = old;
        let reports = std::mem::replace(&mut data.kernels, Box::new([]));
        let old_added = data.added;
        data.added = added(data, header).unwrap();
        assert!(matches!(replay_data(prefix, data, header, budget),
            Err(UError::Admission(error)) if matches!(*error, PError::Admission(E::Formal(crate::ProductionFormalMemoryErrorV1::ObligationMismatch)))));
        data.kernels = reports;
        data.added = old_added;
        replay_data(prefix, data, header, budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
    macro_rules! hook {
        ($owner:ty, $prefix:ty, $variant:ident) => {
            impl $owner {
                pub(crate) fn exercise_loop_unroll_hostile_v1(
                    &mut self,
                    budget: &mut AssertOriginBudgetV1<'_>,
                ) {
                    hostile(
                        FinalPrefix::$variant(&self.prefix),
                        &mut self.data,
                        header::<$prefix, Self>().unwrap(),
                        budget,
                    );
                }
            }
        };
    }
    hook!(
        ProductionOwnedLoopUnrollContinuationV1,
        ProductionOwnedRefinedCrossBlockForwardingContinuationV1,
        Direct
    );
    hook!(
        ProductionOwnedUnitLocalLoopUnrollContinuationV1,
        ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1,
        Erased
    );
}
