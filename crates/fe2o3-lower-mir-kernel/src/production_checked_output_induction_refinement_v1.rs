//! Source-owning continuation of checked unsigned induction-add refinement.
use super::*;
use fe2o3_kernel_analysis::CanonicalKirLoopLimitsV1 as RefinementLimits;
use fe2o3_kernel_opt::{
    OwnedInductionRefinementContinuationV1 as RefinementTail,
    prepare_owned_induction_refinement_v1 as prepare_refinement,
};
pub use promotion_sites::ProductionInductionRefinementOriginV1;
type SourceOrigin = ProductionInductionRefinementOriginV1;
#[path = "production_checked_output_refined_forwarding_v1.rs"]
mod refined_forwarding;
pub use refined_forwarding::{
    ProductionOwnedRefinedCrossBlockForwardingContinuationV1,
    ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1,
    ProductionRefinedCrossBlockForwardingErrorV1, ProductionRefinedCrossBlockForwardingStorageV1,
};

/// Refusal of the actual source-owned checked-induction continuation.
#[derive(Debug)]
pub enum ProductionInductionRefinementErrorV1 {
    /// Cumulative resources or custody of the active work ledger were refused.
    Resource(AssertOriginResourceV1),
    /// The retained genuine source/LICM prefix failed independent replay.
    Prefix(Box<ProductionLicmErrorV1>),
    /// Canonical selection, ownership or actual-pair replay was refused.
    Continuation(fe2o3_kernel_opt::OwnedInductionRefinementErrorV1),
    /// Final source associations or fresh private/native/formal census failed.
    Admission(Box<ProductionPrivateCellPromotionContinuationErrorV1>),
    /// The requested output ceiling exceeds the unchanged retained source cap.
    SourceOperationsLimit {
        /// Caller-supplied canonical operation ceiling, never silently clamped.
        requested: usize,
        /// The genuine original source's operation ceiling.
        source_limit: usize,
    },
    /// The retained exact seven-field Limits disagree with the canonical owner.
    LimitsMismatch,
    /// Complete independently regenerated source origins differ from stored rows.
    OriginsMismatch,
    /// A private phase unwound without transferring a partial owner.
    Panicked,
}
type IError = ProductionInductionRefinementErrorV1;
type IResult<T> = Result<T, IError>;
impl From<AssertOriginResourceV1> for IError {
    fn from(v: AssertOriginResourceV1) -> Self {
        Self::Resource(v)
    }
}
impl fmt::Display for IError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source-owned checked induction refinement: {self:?}")
    }
}
impl Error for IError {}
fn prefix_error(e: LError) -> IError {
    match e {
        LError::Resource(e) => IError::Resource(e),
        LError::Panicked => IError::Panicked,
        other => IError::Prefix(Box::new(other)),
    }
}
fn metadata_error(e: LError) -> IError {
    match e {
        LError::Admission(e) => IError::Admission(e),
        other => prefix_error(other),
    }
}
fn binding_check(binding: &PromotionBinding, budget: &AssertOriginBudgetV1<'_>) -> IResult<()> {
    binding.check(budget).map_err(|error| match error {
        PError::Resource(error) => IError::Resource(error),
        PError::Panicked => IError::Panicked,
        other => IError::Admission(Box::new(other)),
    })
}

/// Unreserved owning addition, excluding all actual prefix and sibling storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionInductionRefinementStorageV1(usize);
impl ProductionInductionRefinementStorageV1 {
    /// Reserve this amount before further controlled operations while live.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
struct RefinementData {
    tail: RefinementTail,
    origins: Vec<SourceOrigin>,
    kernels: Box<[FormalMemoryObligations]>,
    limits: RefinementLimits,
    added: usize,
}

/// Genuine Direct source/LICM prefix plus actual refined output and fresh reports.
/// Synthetic false origins identify the old overflow definition, not a Rust span.
/// Existing source/ranked/formal resource-domain exclusions remain unchanged.
/// No numbered policy, native artifact, default or launch authority is granted.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOwnedInductionRefinementContinuationV1 as Owner;
/// fn duplicate(v: &Owner) -> Owner { v.clone() }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_lower_mir_kernel::ProductionOwnedInductionRefinementContinuationV1 as Owner;
/// fn detach(v: Owner) { let _ = v.prefix; }
/// ```
pub struct ProductionOwnedInductionRefinementContinuationV1 {
    prefix: ProductionOwnedLicmContinuationV1,
    data: RefinementData,
}
/// Genuine UnitLocal original source, checked erasure and complete retained history.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionOwnedUnitLocalInductionRefinementContinuationV1 as New,
///     ProductionOwnedUnitLocalLicmContinuationV1 as Old};
/// fn relabel(v: New) -> Old { v }
/// ```
pub struct ProductionOwnedUnitLocalInductionRefinementContinuationV1 {
    prefix: ProductionOwnedUnitLocalLicmContinuationV1,
    data: RefinementData,
}
#[derive(Clone, Copy)]
enum LicmPrefix<'a> {
    Direct(&'a ProductionOwnedLicmContinuationV1),
    Erased(&'a ProductionOwnedUnitLocalLicmContinuationV1),
}
impl<'a> LicmPrefix<'a> {
    fn preheaders(self) -> PreheaderPrefix<'a> {
        match self {
            Self::Direct(v) => PreheaderPrefix::Direct(&v.prefix),
            Self::Erased(v) => PreheaderPrefix::Erased(&v.prefix),
        }
    }
    fn tail(self) -> &'a LicmTail {
        match self {
            Self::Direct(v) => v.continuation(),
            Self::Erased(v) => v.continuation(),
        }
    }
    fn output(self) -> &'a StoreOwner {
        self.tail().output()
    }
    fn floor(self) -> IResult<usize> {
        match self {
            Self::Direct(v) => v.retained_input_storage_floor_v1(),
            Self::Erased(v) => v.retained_input_storage_floor_v1(),
        }
        .map_err(prefix_error)
    }
    fn replay(self, budget: &mut AssertOriginBudgetV1<'_>) -> IResult<()> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
        .map_err(prefix_error)
    }
    fn source_cap(self) -> usize {
        self.preheaders()
            .promoted()
            .p8()
            .historical()
            .source()
            .limits()
            .max_operations
    }
}
fn scoped<'w, T>(
    required: usize,
    budget: &mut AssertOriginBudgetV1<'w>,
    run: impl FnOnce(&mut AssertOriginBudgetV1<'w>, &PromotionBinding) -> IResult<T>,
) -> IResult<T> {
    // Typed inner errors own no output credit. Failed locals drop before the
    // inherited scope restores this same-ledger incoming floor.
    match licm_scoped(required, budget, |b, binding| Ok(run(b, binding))) {
        Ok(result) => result,
        Err(error) => Err(prefix_error(error)),
    }
}
fn header<P, W>() -> IResult<usize> {
    size_of::<W>()
        .checked_sub(size_of::<P>())
        .and_then(|n| n.checked_sub(size_of::<RefinementTail>()))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn added(data: &RefinementData, header: usize) -> IResult<usize> {
    data.tail
        .retained_storage()
        .checked_add(header)
        .and_then(|n| {
            n.checked_add(
                data.origins
                    .capacity()
                    .checked_mul(size_of::<SourceOrigin>())?,
            )
        })
        .and_then(|n| n.checked_add(std::mem::size_of_val(data.kernels.as_ref())))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn required(prefix: LicmPrefix<'_>, data: &RefinementData, header: usize) -> IResult<usize> {
    if data.added != added(data, header)? {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    prefix
        .floor()?
        .checked_add(data.added)
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn check_limits(
    prefix: LicmPrefix<'_>,
    limits: RefinementLimits,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> IResult<()> {
    budget.charge_work(3)?;
    let source_limit = prefix.source_cap();
    if limits.operations > source_limit {
        return Err(IError::SourceOperationsLimit {
            requested: limits.operations,
            source_limit,
        });
    }
    Ok(())
}
// The wrapper header already pays the retained Vec header. Allocate backing
// outside UnitLocal erasure so its receipt is below every nested scratch floor.
fn origin_rows(count: usize, budget: &mut AssertOriginBudgetV1<'_>) -> IResult<Vec<SourceOrigin>> {
    budget.charge_work(4)?;
    let requested = count
        .checked_mul(size_of::<SourceOrigin>())
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| AssertOriginResourceV1::Allocation)?;
    let actual = rows
        .capacity()
        .checked_mul(size_of::<SourceOrigin>())
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    budget.reserve_storage(
        actual
            .checked_sub(requested)
            .ok_or(AssertOriginResourceV1::Accounting)?,
    )?;
    Ok(rows)
}
fn check_actual(
    prefix: LicmPrefix<'_>,
    tail: &RefinementTail,
    limits: RefinementLimits,
    origins: &mut Vec<SourceOrigin>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> IResult<Box<[FormalMemoryObligations]>> {
    with_actual_refinement_setup(
        prefix,
        tail,
        limits,
        budget,
        binding,
        |sites, pair, output, budget, binding| {
            promotion_sites::check_induction_refinement_sites(
                sites, pair, output, origins, budget, binding,
            )
        },
    )
}

fn with_actual_refinement_sites<'g, 'w, R>(
    prefix: LicmPrefix<'g>,
    tail: &'g RefinementTail,
    limits: RefinementLimits,
    origins: &mut Vec<SourceOrigin>,
    budget: &mut AssertOriginBudgetV1<'w>,
    binding: &PromotionBinding,
    use_sites: impl for<'s> FnOnce(
        promotion_sites::CheckedPromotedSites<'s, 'g>,
        &'s fe2o3_kernel_analysis::CheckedCanonicalKirInductionRefinementV1<'g>,
        &mut AssertOriginBudgetV1<'w>,
        &PromotionBinding,
    ) -> PResult<R>,
) -> IResult<R> {
    with_actual_refinement_setup(
        prefix,
        tail,
        limits,
        budget,
        binding,
        |sites, pair, output, budget, binding| {
            promotion_sites::with_checked_induction_refinement_sites(
                sites,
                pair,
                output,
                origins,
                budget,
                binding,
                |sites, budget, binding| use_sites(sites, pair, budget, binding),
            )
        },
    )
}

fn with_actual_refinement_setup<'g, 'w, R>(
    prefix: LicmPrefix<'g>,
    tail: &'g RefinementTail,
    limits: RefinementLimits,
    budget: &mut AssertOriginBudgetV1<'w>,
    binding: &PromotionBinding,
    use_sites: impl for<'s> FnOnce(
        promotion_sites::CheckedPromotedSites<'s, 'g>,
        &'s fe2o3_kernel_analysis::CheckedCanonicalKirInductionRefinementV1<'g>,
        &'s Inventory<'g>,
        &mut AssertOriginBudgetV1<'w>,
        &PromotionBinding,
    ) -> PResult<R>,
) -> IResult<R> {
    budget.charge_work(7)?;
    if tail.limits() != limits {
        return Err(IError::LimitsMismatch);
    }
    let (pair, storage) = tail
        .replay_against(prefix.output(), limits, budget)
        .map_err(IError::Continuation)?;
    binding_check(binding, budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (output, storage) = Inventory::derive(tail.output(), budget)
        .map_err(inventory_error)
        .map_err(PError::from)
        .map_err(|e| IError::Admission(Box::new(e)))?;
    binding_check(binding, budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let reports = with_actual_licm_sites(
        prefix.preheaders(),
        prefix.tail(),
        budget,
        binding,
        |sites, budget, binding| use_sites(sites, &pair, &output, budget, binding),
    )
    .map_err(metadata_error)?;
    binding_check(binding, budget)?;
    Ok(reports)
}
fn prepare_data(
    prefix: LicmPrefix<'_>,
    limits: RefinementLimits,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> IResult<RefinementData> {
    prefix.replay(budget)?;
    binding_check(binding, budget)?;
    check_limits(prefix, limits, budget)?;
    budget.reserve_storage(header)?;
    let tail = prepare_refinement(prefix.output(), limits, budget).map_err(IError::Continuation)?;
    binding_check(binding, budget)?;
    budget.reserve_storage(tail.retained_storage())?;
    let mut origins = origin_rows(tail.origins().len(), budget)?;
    let kernels = check_actual(prefix, &tail, limits, &mut origins, budget, binding)?;
    binding_check(binding, budget)?;
    let mut data = RefinementData {
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
    prefix: LicmPrefix<'_>,
    data: &RefinementData,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> IResult<()> {
    scoped(
        required(prefix, data, header)?,
        budget,
        |budget, binding| {
            budget.charge_work(7)?;
            if data.limits != data.tail.limits() {
                return Err(IError::LimitsMismatch);
            }
            prefix.replay(budget)?;
            binding_check(binding, budget)?;
            check_limits(prefix, data.limits, budget)?;
            budget.reserve_storage(size_of::<Vec<SourceOrigin>>())?;
            let mut origins = origin_rows(data.tail.origins().len(), budget)?;
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
                return Err(IError::OriginsMismatch);
            }
            for (a, b) in origins.iter().zip(&data.origins) {
                budget.charge_work(size_of::<SourceOrigin>() + 1)?;
                if a != b {
                    return Err(IError::OriginsMismatch);
                }
            }
            if kernels != data.kernels {
                return Err(IError::Admission(Box::new(PError::from(E::Formal(
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
    ($prefix:ty,$owner:ident,$variant:ident) => {
        impl $prefix {
            /// Consumes genuine source/LICM custody and checks the actual refined
            /// graph and every final source/native/formal obligation. The exact
            /// supplied limits are never clamped. Reserve the returned addition;
            /// consumed-prefix failure preserves the caller-owned incoming credit.
            pub fn continue_induction_refinement_v1(
                self,
                limits: RefinementLimits,
                budget: &mut AssertOriginBudgetV1<'_>,
            ) -> IResult<($owner, ProductionInductionRefinementStorageV1)> {
                let floor = LicmPrefix::$variant(&self).floor()?;
                scoped(floor, budget, |budget, binding| {
                    let data = prepare_data(
                        LicmPrefix::$variant(&self),
                        limits,
                        header::<Self, $owner>()?,
                        budget,
                        binding,
                    )?;
                    let receipt = ProductionInductionRefinementStorageV1(data.added);
                    Ok(($owner { prefix: self, data }, receipt))
                })
            }
        }
        impl $owner {
            /// Exact consumed source/LICM history, retained once.
            pub const fn prefix(&self) -> &$prefix {
                &self.prefix
            }
            /// Actual canonical continuation with independently checked lineage.
            pub const fn continuation(&self) -> &RefinementTail {
                &self.data.tail
            }
            /// Actual final graph, not a relabeled historical output.
            pub fn output(&self) -> &StoreOwner {
                self.data.tail.output()
            }
            /// Complete inert original-order source and synthetic associations.
            pub fn origins(&self) -> &[SourceOrigin] {
                &self.data.origins
            }
            /// Fresh final-graph reports in complete original kernel order.
            pub fn kernels(&self) -> &[FormalMemoryObligations] {
                &self.data.kernels
            }
            /// Exact seven-field limits supplied to construction and replay.
            pub const fn limits(&self) -> RefinementLimits {
                self.data.limits
            }
            /// Unreserved owning addition excluding the genuine source prefix.
            pub const fn additional_retained_storage_v1(&self) -> usize {
                self.data.added
            }
            /// Required prefix plus actual new owning reservation.
            pub fn retained_input_storage_floor_v1(&self) -> IResult<usize> {
                required(
                    LicmPrefix::$variant(&self.prefix),
                    &self.data,
                    header::<$prefix, Self>()?,
                )
            }
            /// No native artifact, wire, default or launch authority is granted.
            pub const fn grants_artifact_or_launch_authority(&self) -> bool {
                false
            }
            /// Fresh source/history replay, actual-pair checking, regenerated
            /// source origins and final safety census on the cumulative ledger.
            pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> IResult<()> {
                replay_data(
                    LicmPrefix::$variant(&self.prefix),
                    &self.data,
                    header::<$prefix, Self>()?,
                    budget,
                )
            }
        }
    };
}
owner!(
    ProductionOwnedLicmContinuationV1,
    ProductionOwnedInductionRefinementContinuationV1,
    Direct
);
owner!(
    ProductionOwnedUnitLocalLicmContinuationV1,
    ProductionOwnedUnitLocalInductionRefinementContinuationV1,
    Erased
);

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use std::{cell::Cell, rc::Rc};
    #[test]
    fn source_induction_refinement_binding_rejects_equal_credit_foreign_ledger() {
        let mut work = Work::new(100_000);
        let mut other_work = Work::new(100_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000);
        let mut other = AssertOriginBudgetV1::new(&mut other_work, 1_000_000);
        budget.reserve_storage(31).unwrap();
        other.reserve_storage(31).unwrap();
        scoped(31, &mut budget, |actual, binding| -> IResult<()> {
            assert_eq!(actual.storage(), other.storage());
            assert!(actual.work_ledger_identity_v1() != other.work_ledger_identity_v1());
            assert!(matches!(
                binding_check(binding, &other),
                Err(IError::Resource(AssertOriginResourceV1::Accounting))
            ));
            binding_check(binding, actual)
        })
        .unwrap();
        assert_eq!((budget.storage(), other.storage()), (31, 31));
    }
    fn hostile(
        prefix: LicmPrefix<'_>,
        data: &mut RefinementData,
        header: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        assert!(data.origins.len() >= 2);
        data.origins.swap(0, 1);
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(IError::OriginsMismatch)
        ));
        data.origins.swap(0, 1);
        assert_eq!(budget.storage(), floor);
        let first = data.origins[0];
        data.origins[0] = first.changed_source_for_induction_test();
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(IError::OriginsMismatch)
        ));
        data.origins[0] = first.changed_overflow_for_induction_test();
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(IError::OriginsMismatch)
        ));
        data.origins[0] = first;
        let limits = data.limits;
        data.limits.blocks += 1;
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(IError::LimitsMismatch)
        ));
        data.limits = limits;
        let kernels = std::mem::take(&mut data.kernels);
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(IError::Resource(AssertOriginResourceV1::Accounting))
        ));
        let bytes = std::mem::size_of_val(kernels.as_ref());
        data.added -= bytes;
        match replay_data(prefix, data, header, budget) {
            Err(IError::Admission(error)) => assert!(matches!(
                *error,
                PError::Admission(E::Formal(
                    crate::ProductionFormalMemoryErrorV1::ObligationMismatch
                ))
            )),
            other => panic!("exact fresh source refinement report refusal: {other:?}"),
        }
        data.added += bytes;
        data.kernels = kernels;
        if data.kernels.len() == 2 {
            data.kernels.swap(0, 1);
            match replay_data(prefix, data, header, budget) {
                Err(IError::Admission(error)) => assert!(matches!(
                    *error,
                    PError::Admission(E::Formal(
                        crate::ProductionFormalMemoryErrorV1::ObligationMismatch
                    ))
                )),
                other => panic!("exact source refinement report order refusal: {other:?}"),
            }
            data.kernels.swap(0, 1);
        }
        data.added += 1;
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(IError::Resource(AssertOriginResourceV1::Accounting))
        ));
        data.added -= 1;
        replay_data(prefix, data, header, budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
    impl ProductionOwnedInductionRefinementContinuationV1 {
        pub(crate) fn exercise_induction_refinement_hostile_v1(
            &mut self,
            budget: &mut AssertOriginBudgetV1<'_>,
        ) {
            hostile(
                LicmPrefix::Direct(&self.prefix),
                &mut self.data,
                header::<ProductionOwnedLicmContinuationV1, Self>().unwrap(),
                budget,
            );
        }
    }
    impl ProductionOwnedUnitLocalInductionRefinementContinuationV1 {
        pub(crate) fn exercise_induction_refinement_hostile_v1(
            &mut self,
            budget: &mut AssertOriginBudgetV1<'_>,
        ) {
            hostile(
                LicmPrefix::Erased(&self.prefix),
                &mut self.data,
                header::<ProductionOwnedUnitLocalLicmContinuationV1, Self>().unwrap(),
                budget,
            );
        }
    }
    fn partial_failure(
        prefix: LicmPrefix<'_>,
        header: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) {
        struct Candidate<'a> {
            data: Option<RefinementData>,
            dropped: &'a Cell<bool>,
        }
        impl Drop for Candidate<'_> {
            fn drop(&mut self) {
                let data = self.data.take().unwrap();
                assert!(!data.origins.is_empty());
                assert_eq!(
                    data.kernels.len(),
                    data.tail.output().module().kernels.len()
                );
                drop(data);
                self.dropped.set(true);
            }
        }
        let sibling = vec![0xa3u8; 47];
        let sibling_bytes = size_of::<Vec<u8>>() + sibling.capacity();
        budget.reserve_storage(sibling_bytes).unwrap();
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        for unwind in [false, true] {
            let dropped = Cell::new(false);
            let result: IResult<()> = scoped(prefix.floor().unwrap(), budget, |budget, binding| {
                let data =
                    prepare_data(prefix, RefinementLimits::default(), header, budget, binding)?;
                let _live = Candidate {
                    data: Some(data),
                    dropped: &dropped,
                };
                if unwind {
                    panic!("real completed source refinement candidate");
                }
                Err(IError::OriginsMismatch)
            });
            if unwind {
                assert!(matches!(result, Err(IError::Panicked)));
            } else {
                assert!(matches!(result, Err(IError::OriginsMismatch)));
            }
            assert!(dropped.get());
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(sibling, [0xa3; 47]);
        }
        drop(sibling);
        budget.release_storage(sibling_bytes).unwrap();
    }
    impl ProductionOwnedLicmContinuationV1 {
        pub(crate) fn exercise_induction_refinement_partial_failure_v1(
            &self,
            budget: &mut AssertOriginBudgetV1<'_>,
        ) {
            partial_failure(
                LicmPrefix::Direct(self),
                header::<Self, ProductionOwnedInductionRefinementContinuationV1>().unwrap(),
                budget,
            );
        }
    }
    impl ProductionOwnedUnitLocalLicmContinuationV1 {
        pub(crate) fn exercise_induction_refinement_partial_failure_v1(
            &self,
            budget: &mut AssertOriginBudgetV1<'_>,
        ) {
            partial_failure(
                LicmPrefix::Erased(self),
                header::<Self, ProductionOwnedUnitLocalInductionRefinementContinuationV1>()
                    .unwrap(),
                budget,
            );
        }
    }

    #[test]
    fn source_induction_refinement_nested_failure_and_panic_drop_before_refund() {
        struct Live {
            rows: Vec<SourceOrigin>,
            dropped: Rc<Cell<bool>>,
        }
        impl Drop for Live {
            fn drop(&mut self) {
                assert!(self.rows.capacity() >= 7);
                self.dropped.set(true);
            }
        }
        for unwind in [false, true] {
            let mut work = Work::new(100_000);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000);
            let sibling = vec![0x73u8; 31];
            budget.reserve_storage(sibling.capacity()).unwrap();
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let dropped = Rc::new(Cell::new(false));
            let result: IResult<()> = scoped(floor, &mut budget, |budget, binding| {
                budget.reserve_storage(size_of::<Live>())?;
                let live = Live {
                    rows: origin_rows(7, budget)?,
                    dropped: dropped.clone(),
                };
                binding_check(binding, budget)?;
                // Actual new retained-row allocator, with the same nested typed
                // Ok(Err) path as construction, not a synthetic source owner.
                let nested: IResult<()> = scoped(budget.storage(), budget, |budget, _| {
                    budget.reserve_storage(17)?;
                    if unwind {
                        panic!("partial source refinement")
                    };
                    Err(IError::OriginsMismatch)
                });
                drop(live);
                nested
            });
            if unwind {
                assert!(matches!(result, Err(IError::Panicked)));
            } else {
                assert!(matches!(result, Err(IError::OriginsMismatch)));
            }
            assert!(dropped.get());
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(sibling, [0x73; 31]);
        }
    }
    #[test]
    fn source_induction_refinement_actual_origin_capacity_and_short_allocation_are_bounded() {
        let mut work = Work::new(100_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000);
        budget.reserve_storage(41).unwrap();
        let floor = budget.storage();
        scoped(floor, &mut budget, |budget, binding| -> IResult<()> {
            budget.reserve_storage(size_of::<Vec<SourceOrigin>>())?;
            let before = budget.storage();
            let rows = origin_rows(11, budget)?;
            assert_eq!(
                budget.storage(),
                before + rows.capacity() * size_of::<SourceOrigin>()
            );
            binding_check(binding, budget)?;
            drop(rows);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
        let mut work = Work::new(100_000);
        let request = 11 * size_of::<SourceOrigin>();
        let mut budget = AssertOriginBudgetV1::new(&mut work, request - 1);
        let result = origin_rows(11, &mut budget);
        let Err(IError::Resource(AssertOriginResourceV1::Storage(limit))) = result else {
            panic!("exact new origin backing refusal")
        };
        assert_eq!((limit.actual(), limit.limit()), (request, request - 1));
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            (4, 0, 0, Some(request))
        );
    }
    #[test]
    fn source_induction_refinement_new_owner_headers_do_not_change_prefix_layouts() {
        for (new, prefix, calculated) in [
            (
                size_of::<ProductionOwnedInductionRefinementContinuationV1>(),
                size_of::<ProductionOwnedLicmContinuationV1>(),
                header::<
                    ProductionOwnedLicmContinuationV1,
                    ProductionOwnedInductionRefinementContinuationV1,
                >()
                .unwrap(),
            ),
            (
                size_of::<ProductionOwnedUnitLocalInductionRefinementContinuationV1>(),
                size_of::<ProductionOwnedUnitLocalLicmContinuationV1>(),
                header::<
                    ProductionOwnedUnitLocalLicmContinuationV1,
                    ProductionOwnedUnitLocalInductionRefinementContinuationV1,
                >()
                .unwrap(),
            ),
        ] {
            assert_eq!(new, prefix + size_of::<RefinementTail>() + calculated);
            assert!(calculated >= size_of::<Vec<SourceOrigin>>() + size_of::<RefinementLimits>());
        }
    }
}
