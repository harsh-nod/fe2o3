//! Consuming source custody through one checked at-site private-load pass.
use super::*;
use fe2o3_kernel_analysis::CanonicalKirCrossBlockForwardingLimitsV1 as ForwardingLimits;
use fe2o3_kernel_opt::{
    OwnedCrossBlockForwardingV1 as ForwardingTail,
    prepare_owned_cross_block_forwarding_v1 as prepare_forwarding,
};
pub use promotion_sites::ProductionCrossBlockForwardingOriginV1;
type SourceOrigin = ProductionCrossBlockForwardingOriginV1;

/// Refusal of the actual source-owned private-load continuation.
#[derive(Debug)]
pub enum ProductionCrossBlockForwardingErrorV1 {
    /// Cumulative resources or active work-ledger custody failed.
    Resource(AssertOriginResourceV1),
    /// The actual retained source/LICM history failed replay.
    Prefix(Box<ProductionLicmErrorV1>),
    /// Canonical ownership, selection or independent graph replay failed.
    Continuation(fe2o3_kernel_opt::OwnedCrossBlockForwardingErrorV1),
    /// Actual source association or final safety census failed.
    Admission(Box<ProductionPrivateCellPromotionContinuationErrorV1>),
    /// Stored exact limits disagree with the actual canonical continuation.
    LimitsMismatch,
    /// Regenerated source rows disagree with retained original occurrences.
    OriginsMismatch,
    /// A private phase unwound without transferring a partial owner.
    Panicked,
}
type FError = ProductionCrossBlockForwardingErrorV1;
type FResult<T> = Result<T, FError>;
impl From<AssertOriginResourceV1> for FError {
    fn from(value: AssertOriginResourceV1) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for FError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source-owned cross-block private forwarding: {self:?}")
    }
}
impl Error for FError {}
fn prefix_error(error: LError) -> FError {
    match error {
        LError::Resource(error) => FError::Resource(error),
        LError::Panicked => FError::Panicked,
        other => FError::Prefix(Box::new(other)),
    }
}
fn metadata_error(error: LError) -> FError {
    match error {
        LError::Admission(error) => FError::Admission(error),
        other => prefix_error(other),
    }
}
fn binding_check(binding: &PromotionBinding, budget: &AssertOriginBudgetV1<'_>) -> FResult<()> {
    binding.check(budget).map_err(|error| match error {
        PError::Resource(error) => FError::Resource(error),
        PError::Panicked => FError::Panicked,
        other => FError::Admission(Box::new(other)),
    })
}

/// Unreserved added ownership, excluding the actual source prefix and siblings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionCrossBlockForwardingStorageV1(usize);
impl ProductionCrossBlockForwardingStorageV1 {
    /// Reserve this addition before further controlled work while it is live.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
struct ForwardingData {
    tail: ForwardingTail,
    origins: Vec<SourceOrigin>,
    kernels: Box<[FormalMemoryObligations]>,
    limits: ForwardingLimits,
    added: usize,
}

/// Genuine Direct source/LICM history plus actual final graph and fresh reports.
/// Selected copies retain their original Load statements, not their Store's
/// source location. No instruction motion, source admission widening, numbered
/// policy, default-pipeline, native or launch authority is implied. Existing
/// source/ranked/formal resource-domain exclusions remain unchanged.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOwnedCrossBlockForwardingContinuationV1 as Owner;
/// fn duplicate(v: &Owner) -> Owner { v.clone() }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_lower_mir_kernel::ProductionOwnedCrossBlockForwardingContinuationV1 as Owner;
/// fn detach(v: Owner) { let _ = v.prefix; }
/// ```
pub struct ProductionOwnedCrossBlockForwardingContinuationV1 {
    prefix: ProductionOwnedLicmContinuationV1,
    data: ForwardingData,
}
/// Genuine UnitLocal source, original roots and checked erasure retained once.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionOwnedUnitLocalCrossBlockForwardingContinuationV1 as New,
///     ProductionOwnedUnitLocalLicmContinuationV1 as Old};
/// fn relabel(v: New) -> Old { v }
/// ```
pub struct ProductionOwnedUnitLocalCrossBlockForwardingContinuationV1 {
    prefix: ProductionOwnedUnitLocalLicmContinuationV1,
    data: ForwardingData,
}
#[derive(Clone, Copy)]
enum LicmPrefix<'a> {
    Direct(&'a ProductionOwnedLicmContinuationV1),
    Erased(&'a ProductionOwnedUnitLocalLicmContinuationV1),
}
impl<'a> LicmPrefix<'a> {
    fn preheaders(self) -> PreheaderPrefix<'a> {
        match self {
            Self::Direct(owner) => PreheaderPrefix::Direct(&owner.prefix),
            Self::Erased(owner) => PreheaderPrefix::Erased(&owner.prefix),
        }
    }
    fn tail(self) -> &'a LicmTail {
        match self {
            Self::Direct(owner) => owner.continuation(),
            Self::Erased(owner) => owner.continuation(),
        }
    }
    fn output(self) -> &'a StoreOwner {
        self.tail().output()
    }
    fn floor(self) -> FResult<usize> {
        match self {
            Self::Direct(owner) => owner.retained_input_storage_floor_v1(),
            Self::Erased(owner) => owner.retained_input_storage_floor_v1(),
        }
        .map_err(prefix_error)
    }
    fn replay(self, budget: &mut AssertOriginBudgetV1<'_>) -> FResult<()> {
        match self {
            Self::Direct(owner) => owner.verify_equivalence(budget),
            Self::Erased(owner) => owner.verify_equivalence(budget),
        }
        .map_err(prefix_error)
    }
}
fn scoped<'w, T>(
    required: usize,
    budget: &mut AssertOriginBudgetV1<'w>,
    run: impl FnOnce(&mut AssertOriginBudgetV1<'w>, &PromotionBinding) -> FResult<T>,
) -> FResult<T> {
    // The typed inner error cannot transfer an output. Actual locals drop before
    // the inherited scope restores the original same-ledger incoming floor.
    match licm_scoped(required, budget, |budget, binding| Ok(run(budget, binding))) {
        Ok(result) => result,
        Err(error) => Err(prefix_error(error)),
    }
}
fn header<P, W>() -> FResult<usize> {
    size_of::<W>()
        .checked_sub(size_of::<P>())
        .and_then(|bytes| bytes.checked_sub(size_of::<ForwardingTail>()))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn added(data: &ForwardingData, header: usize) -> FResult<usize> {
    data.tail
        .retained_storage()
        .checked_add(header)
        .and_then(|bytes| {
            bytes.checked_add(
                data.origins
                    .capacity()
                    .checked_mul(size_of::<SourceOrigin>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(std::mem::size_of_val(data.kernels.as_ref())))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn required(prefix: LicmPrefix<'_>, data: &ForwardingData, header: usize) -> FResult<usize> {
    if data.added != added(data, header)? {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    prefix
        .floor()?
        .checked_add(data.added)
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
// Retained rows are allocated outside the UnitLocal callback scratch floor.
fn origin_rows(count: usize, budget: &mut AssertOriginBudgetV1<'_>) -> FResult<Vec<SourceOrigin>> {
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
    tail: &ForwardingTail,
    limits: ForwardingLimits,
    origins: &mut Vec<SourceOrigin>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> FResult<Box<[FormalMemoryObligations]>> {
    budget.charge_work(7)?;
    if tail.limits() != limits {
        return Err(FError::LimitsMismatch);
    }
    let (pair, storage) = tail
        .replay_against(prefix.output(), budget)
        .map_err(FError::Continuation)?;
    binding_check(binding, budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (output, storage) = Inventory::derive(tail.output(), budget)
        .map_err(inventory_error)
        .map_err(PError::from)
        .map_err(|error| FError::Admission(Box::new(error)))?;
    binding_check(binding, budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let reports = with_actual_licm_sites(
        prefix.preheaders(),
        prefix.tail(),
        budget,
        binding,
        |sites, budget, binding| {
            promotion_sites::check_cross_block_forwarding_sites(
                sites, &pair, &output, origins, budget, binding,
            )
        },
    )
    .map_err(metadata_error)?;
    binding_check(binding, budget)?;
    Ok(reports)
}
fn prepare_data(
    prefix: LicmPrefix<'_>,
    limits: ForwardingLimits,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> FResult<ForwardingData> {
    // This full replay includes original private-load lifetimes before any Load
    // disappears; output-only census cannot establish that source obligation.
    prefix.replay(budget)?;
    binding_check(binding, budget)?;
    budget.reserve_storage(header)?;
    let tail = prepare_forwarding(prefix.output(), limits, budget).map_err(FError::Continuation)?;
    binding_check(binding, budget)?;
    budget.reserve_storage(tail.retained_storage())?;
    let mut origins = origin_rows(tail.origins().len(), budget)?;
    let kernels = check_actual(prefix, &tail, limits, &mut origins, budget, binding)?;
    binding_check(binding, budget)?;
    let mut data = ForwardingData {
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
    data: &ForwardingData,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> FResult<()> {
    scoped(
        required(prefix, data, header)?,
        budget,
        |budget, binding| {
            budget.charge_work(7)?;
            if data.limits != data.tail.limits() {
                return Err(FError::LimitsMismatch);
            }
            prefix.replay(budget)?;
            binding_check(binding, budget)?;
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
                return Err(FError::OriginsMismatch);
            }
            for (actual, stored) in origins.iter().zip(&data.origins) {
                budget.charge_work(size_of::<SourceOrigin>() + 1)?;
                if actual != stored {
                    return Err(FError::OriginsMismatch);
                }
            }
            if kernels != data.kernels {
                return Err(FError::Admission(Box::new(PError::from(E::Formal(
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
            /// Consumes actual source/LICM custody and independently checks the
            /// complete same-site graph relation, original Load source sites and
            /// fresh final safety census. Reserve the returned owning addition.
            /// Failure drops the consumed owner but preserves incoming credit;
            /// the caller retains responsibility for that inherited reservation.
            pub fn continue_cross_block_forwarding_v1(
                self,
                limits: ForwardingLimits,
                budget: &mut AssertOriginBudgetV1<'_>,
            ) -> FResult<($owner, ProductionCrossBlockForwardingStorageV1)> {
                let floor = LicmPrefix::$variant(&self).floor()?;
                scoped(floor, budget, |budget, binding| {
                    let data = prepare_data(
                        LicmPrefix::$variant(&self),
                        limits,
                        header::<Self, $owner>()?,
                        budget,
                        binding,
                    )?;
                    let receipt = ProductionCrossBlockForwardingStorageV1(data.added);
                    Ok(($owner { prefix: self, data }, receipt))
                })
            }
        }
        impl $owner {
            /// Exact consumed source and LICM history, retained once.
            pub const fn prefix(&self) -> &$prefix {
                &self.prefix
            }
            /// Actual independent-pair checked canonical continuation.
            pub const fn continuation(&self) -> &ForwardingTail {
                &self.data.tail
            }
            /// Actual final graph, never a relabeled historical endpoint.
            pub fn output(&self) -> &StoreOwner {
                self.data.tail.output()
            }
            /// Complete original-order canonical and semantic statement lineage.
            pub fn origins(&self) -> &[SourceOrigin] {
                &self.data.origins
            }
            /// Fresh final reports in complete original kernel order.
            pub fn kernels(&self) -> &[FormalMemoryObligations] {
                &self.data.kernels
            }
            /// Exact analysis limits retained for every replay.
            pub const fn limits(&self) -> ForwardingLimits {
                self.data.limits
            }
            /// Unreserved addition excluding genuine source prefix and siblings.
            pub const fn additional_retained_storage_v1(&self) -> usize {
                self.data.added
            }
            /// Required genuine prefix plus actual new owning reservation.
            pub fn retained_input_storage_floor_v1(&self) -> FResult<usize> {
                required(
                    LicmPrefix::$variant(&self.prefix),
                    &self.data,
                    header::<$prefix, Self>()?,
                )
            }
            /// No source admission, policy, artifact or launch authority is added.
            pub const fn grants_artifact_or_launch_authority(&self) -> bool {
                false
            }
            /// Replays actual history and both graph endpoints, regenerates
            /// source lineage and repeats final safety checks on the same ledger.
            pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> FResult<()> {
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
    ProductionOwnedCrossBlockForwardingContinuationV1,
    Direct
);
owner!(
    ProductionOwnedUnitLocalLicmContinuationV1,
    ProductionOwnedUnitLocalCrossBlockForwardingContinuationV1,
    Erased
);

#[cfg(test)]
mod tests {
    use super::*;
    fn hostile(
        prefix: LicmPrefix<'_>,
        data: &mut ForwardingData,
        header: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        assert!(data.origins.len() >= 2);
        data.origins.swap(0, 1);
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(FError::OriginsMismatch)
        ));
        data.origins.swap(0, 1);
        let limits = data.limits;
        data.limits.memory.operations += 1;
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(FError::LimitsMismatch)
        ));
        data.limits = limits;
        if let Some(selected) = data
            .origins
            .iter()
            .position(|row| row.canonical_origin().store.is_some())
        {
            let original = data.origins[selected];
            data.origins[selected] = original.changed_source_for_forwarding_test();
            assert!(matches!(
                replay_data(prefix, data, header, budget),
                Err(FError::OriginsMismatch)
            ));
            data.origins[selected] = original;
            let store = original.canonical_origin().store.unwrap();
            let store = *data
                .origins
                .iter()
                .find(|row| row.canonical_origin().input == store)
                .unwrap();
            assert!(store.original_source_statement().is_some());
            assert_ne!(
                store.original_source_statement(),
                original.original_source_statement()
            );
            data.origins[selected] = original.other_source_for_forwarding_test(store);
            assert!(matches!(
                replay_data(prefix, data, header, budget),
                Err(FError::OriginsMismatch)
            ));
            data.origins[selected] = original;
            scoped(
                required(prefix, data, header).unwrap(),
                budget,
                |budget, binding| {
                    let (pair, receipt) = data
                        .tail
                        .replay_against(prefix.output(), budget)
                        .map_err(FError::Continuation)?;
                    budget.reserve_storage(receipt.retained_storage())?;
                    let (output, receipt) = Inventory::derive(data.tail.output(), budget)
                        .map_err(inventory_error)
                        .map_err(PError::from)
                        .map_err(|e| FError::Admission(Box::new(e)))?;
                    budget.reserve_storage(receipt.retained_storage())?;
                    with_actual_licm_sites(
                        prefix.preheaders(),
                        prefix.tail(),
                        budget,
                        binding,
                        |sites, budget, binding| {
                            promotion_sites::exercise_cross_block_source_refusals(
                                sites, &pair, &output, budget, binding,
                            )
                        },
                    )
                    .map_err(metadata_error)
                },
            )
            .unwrap();
        }
        data.added += 1;
        assert!(matches!(
            replay_data(prefix, data, header, budget),
            Err(FError::Resource(AssertOriginResourceV1::Accounting))
        ));
        data.added -= 1;
        let kernels = std::mem::take(&mut data.kernels);
        let bytes = std::mem::size_of_val(kernels.as_ref());
        data.added -= bytes;
        match replay_data(prefix, data, header, budget) {
            Err(FError::Admission(error)) => assert!(matches!(
                *error,
                PError::Admission(E::Formal(
                    crate::ProductionFormalMemoryErrorV1::ObligationMismatch
                ))
            )),
            _ => panic!("exact independently regenerated final report refusal"),
        }
        data.added += bytes;
        data.kernels = kernels;
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        replay_data(prefix, data, header, budget).unwrap();
    }
    impl ProductionOwnedCrossBlockForwardingContinuationV1 {
        pub(crate) fn exercise_cross_block_forwarding_refusals_v1(
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
    impl ProductionOwnedUnitLocalCrossBlockForwardingContinuationV1 {
        pub(crate) fn exercise_cross_block_forwarding_refusals_v1(
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
    #[test]
    fn source_cross_block_forwarding_scope_preserves_foreign_ledger_and_typed_unwind() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
        let mut work = Work::new(100_000);
        let mut other = Work::new(100_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000);
        let mut foreign = AssertOriginBudgetV1::new(&mut other, 1_000_000);
        budget.reserve_storage(31).unwrap();
        foreign.reserve_storage(31).unwrap();
        scoped(31, &mut budget, |actual, binding| {
            assert!(matches!(
                binding_check(binding, &foreign),
                Err(FError::Resource(AssertOriginResourceV1::Accounting))
            ));
            binding_check(binding, actual)
        })
        .unwrap();
        assert!(matches!(
            scoped(31, &mut budget, |_, _| -> FResult<()> {
                std::panic::panic_any(71u32)
            }),
            Err(FError::Panicked)
        ));
        assert_eq!((budget.storage(), foreign.storage()), (31, 31));
    }
}
