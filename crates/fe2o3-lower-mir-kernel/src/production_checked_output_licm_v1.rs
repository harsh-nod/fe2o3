//! Genuine source/preheader custody through checked total-integer LICM.
#[path = "production_checked_output_loop_induction_query_v1.rs"]
mod loop_induction_query;
use super::*;
use fe2o3_kernel_opt::{
    OwnedLicmContinuationV1 as LicmTail, prepare_owned_licm_v1 as prepare_licm,
};
pub use loop_induction_query::{
    ProductionLoopInductionQueryErrorV1, ProductionLoopInductionQueryStorageV1,
    ProductionLoopInductionQueryV1,
};

/// Refusal of an unnumbered source-owned total-integer LICM continuation.
#[derive(Debug)]
pub enum ProductionLicmErrorV1 {
    /// Cumulative budget, allocation, arithmetic or ledger custody failed.
    Resource(AssertOriginResourceV1),
    /// The genuine retained preheader/source prefix failed replay.
    Prefix(Box<ProductionLoopPreheadersErrorV1>),
    /// Canonical selection, ownership or independent pair replay failed.
    Continuation(fe2o3_kernel_opt::OwnedLicmErrorV1),
    /// Reconstructed actual-source sites or final safety census failed.
    Admission(Box<ProductionPrivateCellPromotionContinuationErrorV1>),
    /// The private phase unwound without transferring a partial owner.
    Panicked,
}
type LError = ProductionLicmErrorV1;
type LResult<T> = Result<T, LError>;
impl From<AssertOriginResourceV1> for LError {
    fn from(value: AssertOriginResourceV1) -> Self {
        Self::Resource(value)
    }
}
impl From<PError> for LError {
    fn from(value: PError) -> Self {
        Self::Admission(Box::new(value))
    }
}
impl fmt::Display for LError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source-owned total-integer LICM: {self:?}")
    }
}
impl Error for LError {}

/// Unreserved owning addition, excluding the retained genuine source prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionLicmStorageV1(usize);
impl ProductionLicmStorageV1 {
    /// Reserve this addition before further ledger-controlled operations.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

struct LicmData {
    tail: LicmTail,
    kernels: Box<[FormalMemoryObligations]>,
    added: usize,
}

/// Consumed genuine Direct preheader/source plus actual checked LICM output.
/// Operation origins are not new Rust spans. Synthetic preheader origins stay
/// bound to the retained historical neutral pair, not to an empty final block.
/// Final source lifetime/native/formal checks are repeated on the actual output.
/// Existing independently limited source/ranked/formal services retain their
/// metering exclusions. This grants no native, numbered-policy or launch proof.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOwnedLicmContinuationV1 as Owner;
/// fn copy(owner: &Owner) -> Owner { owner.clone() }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_lower_mir_kernel::ProductionOwnedLicmContinuationV1 as Owner;
/// fn detach(owner: Owner) { let _ = owner.prefix; }
/// ```
pub struct ProductionOwnedLicmContinuationV1 {
    prefix: ProductionOwnedLoopPreheadersContinuationV1,
    data: LicmData,
}

/// UnitLocal source continuation retaining original N, checked E and history once.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionOwnedUnitLocalLicmContinuationV1 as New,
///     ProductionOwnedUnitLocalLoopPreheadersContinuationV1 as Old};
/// fn relabel(owner: New) -> Old { owner }
/// ```
pub struct ProductionOwnedUnitLocalLicmContinuationV1 {
    prefix: ProductionOwnedUnitLocalLoopPreheadersContinuationV1,
    data: LicmData,
}

#[derive(Clone, Copy)]
enum PreheaderPrefix<'a> {
    Direct(&'a ProductionOwnedLoopPreheadersContinuationV1),
    Erased(&'a ProductionOwnedUnitLocalLoopPreheadersContinuationV1),
}
impl<'a> PreheaderPrefix<'a> {
    fn promoted(self) -> PromotedPrefix<'a> {
        match self {
            Self::Direct(v) => PromotedPrefix::Direct(&v.prefix),
            Self::Erased(v) => PromotedPrefix::Erased(&v.prefix),
        }
    }
    fn tail(self) -> &'a PreheaderTail {
        match self {
            Self::Direct(v) => v.continuation(),
            Self::Erased(v) => v.continuation(),
        }
    }
    fn output(self) -> &'a StoreOwner {
        self.tail().output()
    }
    fn floor(self) -> LResult<usize> {
        match self {
            Self::Direct(v) => v.retained_input_storage_floor_v1(),
            Self::Erased(v) => v.retained_input_storage_floor_v1(),
        }
        .map_err(|e| LError::Prefix(Box::new(e)))
    }
    fn replay(self, budget: &mut AssertOriginBudgetV1<'_>) -> LResult<()> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
        .map_err(|e| LError::Prefix(Box::new(e)))
    }
}

fn licm_scoped<'w, T>(
    required: usize,
    budget: &mut AssertOriginBudgetV1<'w>,
    run: impl FnOnce(&mut AssertOriginBudgetV1<'w>, &PromotionBinding) -> LResult<T>,
) -> LResult<T> {
    match promotion_scoped(required, budget, |budget, binding| Ok(run(budget, binding))) {
        Ok(result) => result,
        Err(PError::Resource(e)) => Err(LError::Resource(e)),
        Err(PError::Panicked) => Err(LError::Panicked),
        Err(e) => Err(e.into()),
    }
}
fn licm_header<P, W>() -> LResult<usize> {
    size_of::<W>()
        .checked_sub(size_of::<P>())
        .and_then(|n| n.checked_sub(size_of::<LicmTail>()))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn licm_added(data: &LicmData, header: usize) -> LResult<usize> {
    data.tail
        .retained_storage()
        .checked_add(header)
        .and_then(|n| n.checked_add(std::mem::size_of_val(data.kernels.as_ref())))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn licm_required(prefix: PreheaderPrefix<'_>, data: &LicmData, header: usize) -> LResult<usize> {
    if licm_added(data, header)? != data.added {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    prefix
        .floor()?
        .checked_add(data.added)
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}

fn check_actual_licm(
    prefix: PreheaderPrefix<'_>,
    tail: &LicmTail,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> LResult<Box<[FormalMemoryObligations]>> {
    let (licm, storage) = tail
        .replay_against(prefix.output(), budget)
        .map_err(LError::Continuation)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let promoted = prefix.promoted();
    let (preheaders, storage) = prefix
        .tail()
        .replay_against(promoted.output(), budget)
        .map_err(|e| LError::Prefix(Box::new(HError::Continuation(e))))?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (input, storage) = Inventory::derive(prefix.output(), budget)
        .map_err(inventory_error)
        .map_err(PError::from)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (output, storage) = Inventory::derive(tail.output(), budget)
        .map_err(inventory_error)
        .map_err(PError::from)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let reports = with_promoted_output_sites(
        promoted.p8(),
        promoted.tail(),
        budget,
        binding,
        |sites, budget, inner| {
            promotion_sites::check_licm_after_preheaders_sites(
                sites,
                &preheaders,
                &licm,
                &input,
                &output,
                budget,
                inner,
            )
        },
    )?;
    binding.check(budget)?;
    Ok(reports)
}

fn prepare_licm_data(
    prefix: PreheaderPrefix<'_>,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> LResult<LicmData> {
    prefix.replay(budget)?;
    binding.check(budget)?;
    let tail = prepare_licm(prefix.output(), budget).map_err(LError::Continuation)?;
    binding.check(budget)?;
    budget.reserve_storage(tail.retained_storage())?;
    let kernels = check_actual_licm(prefix, &tail, budget, binding)?;
    binding.check(budget)?;
    budget.reserve_storage(header)?;
    let mut data = LicmData {
        tail,
        kernels,
        added: 0,
    };
    data.added = licm_added(&data, header)?;
    budget.charge_work(1)?;
    Ok(data)
}
fn replay_licm_data(
    prefix: PreheaderPrefix<'_>,
    data: &LicmData,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> LResult<()> {
    licm_scoped(
        licm_required(prefix, data, header)?,
        budget,
        |budget, binding| {
            prefix.replay(budget)?;
            binding.check(budget)?;
            let kernels = check_actual_licm(prefix, &data.tail, budget, binding)?;
            if kernels != data.kernels {
                return Err(PError::from(E::Formal(
                    crate::ProductionFormalMemoryErrorV1::ObligationMismatch,
                ))
                .into());
            }
            binding.check(budget)?;
            budget.charge_work(1)?;
            Ok(())
        },
    )
}

macro_rules! licm_owner {
    ($prefix:ty, $owner:ident, $variant:ident) => {
        impl $prefix {
            /// Consumes a genuine preheader/source owner, hoists eligible total
            /// scalar operations and independently checks complete origins and
            /// final source safety. No fallback or source-name dispatch occurs.
            /// Reserve the returned added receipt before subsequent work. The
            /// incoming floor survives every exit; caller owns inherited credit
            /// if consumption fails. All scratch uses the original work ledger.
            pub fn continue_licm_v1(
                self,
                budget: &mut AssertOriginBudgetV1<'_>,
            ) -> LResult<($owner, ProductionLicmStorageV1)> {
                let minimum = PreheaderPrefix::$variant(&self).floor()?;
                licm_scoped(minimum, budget, |budget, binding| {
                    let data = prepare_licm_data(
                        PreheaderPrefix::$variant(&self),
                        licm_header::<Self, $owner>()?,
                        budget,
                        binding,
                    )?;
                    let storage = ProductionLicmStorageV1(data.added);
                    Ok(($owner { prefix: self, data }, storage))
                })
            }
        }
        impl $owner {
            /// Exact consumed preheader/source history, retained once.
            pub const fn prefix(&self) -> &$prefix {
                &self.prefix
            }
            /// Actual canonical continuation with complete input-ordered origins.
            pub const fn continuation(&self) -> &LicmTail {
                &self.data.tail
            }
            /// Actual final graph, never a relabeled historical endpoint.
            pub fn output(&self) -> &StoreOwner {
                self.data.tail.output()
            }
            /// Every retained/moved operation; these are not newly issued Rust spans.
            pub fn operation_origins(&self) -> &[fe2o3_kernel_analysis::CanonicalKirLicmOriginV1] {
                self.data.tail.origins()
            }
            /// Fresh actual-final-graph reports in unchanged kernel roster order.
            pub fn kernels(&self) -> &[FormalMemoryObligations] {
                &self.data.kernels
            }
            /// Unreserved addition excluding all retained prefix and sibling backing.
            pub const fn additional_retained_storage_v1(&self) -> usize {
                self.data.added
            }
            /// Required genuine prefix plus added owning receipt.
            pub fn retained_input_storage_floor_v1(&self) -> LResult<usize> {
                licm_required(
                    PreheaderPrefix::$variant(&self.prefix),
                    &self.data,
                    licm_header::<$prefix, Self>()?,
                )
            }
            /// No artifact, runtime, numbered-policy or launch authority is issued.
            pub const fn grants_artifact_or_launch_authority(&self) -> bool {
                false
            }
            /// Replays actual source/prefix/LICM endpoints and complete site joins,
            /// then rederives and compares final safety reports on the same ledger.
            pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> LResult<()> {
                replay_licm_data(
                    PreheaderPrefix::$variant(&self.prefix),
                    &self.data,
                    licm_header::<$prefix, Self>()?,
                    budget,
                )
            }
        }
    };
}
licm_owner!(
    ProductionOwnedLoopPreheadersContinuationV1,
    ProductionOwnedLicmContinuationV1,
    Direct
);
licm_owner!(
    ProductionOwnedUnitLocalLoopPreheadersContinuationV1,
    ProductionOwnedUnitLocalLicmContinuationV1,
    Erased
);

#[cfg(test)]
#[path = "production_checked_output_licm_internal_v1_tests.rs"]
mod tests;
