//! Source-owned neutral preheaders, without a numbered or native policy.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV1 as Inventory, CheckedCanonicalKirLoopPreheadersV1 as PreheaderPair,
};
use fe2o3_kernel_opt::{
    OwnedLoopPreheadersContinuationV1 as PreheaderTail,
    prepare_owned_loop_preheaders_v1 as prepare_preheaders,
};

#[path = "production_checked_output_loop_preheader_origins_v1.rs"]
mod origins;
use origins::Origins;
pub use origins::{
    ProductionLoopPreheaderIncomingOriginV1, ProductionLoopPreheaderOriginV1,
    ProductionLoopPreheaderParameterOriginV1,
};

/// Refusal of an unnumbered source-owning neutral preheader continuation.
#[derive(Debug)]
pub enum ProductionLoopPreheadersErrorV1 {
    /// Local work/storage, checked arithmetic, allocation or ledger custody failed.
    Resource(AssertOriginResourceV1),
    /// The retained genuine promotion/source prefix failed its own replay.
    Prefix(Box<ProductionPrivateCellPromotionContinuationErrorV1>),
    /// Canonical preheader construction or its independent actual pair was refused.
    Continuation(fe2o3_kernel_opt::OwnedLoopPreheadersErrorV1),
    /// Nested final-source metadata, private/native-operation or formal census failed.
    Admission(Box<ProductionPrivateCellPromotionContinuationErrorV1>),
    /// Complete ordered synthetic coordinate origins did not match the actual pair.
    Origins(&'static str),
    /// The outer private phase caught a panic without transferring a partial owner.
    Panicked,
}
type HError = ProductionLoopPreheadersErrorV1;
type HResult<T> = Result<T, HError>;
impl From<AssertOriginResourceV1> for HError {
    fn from(error: AssertOriginResourceV1) -> Self {
        Self::Resource(error)
    }
}
impl From<E> for HError {
    fn from(error: E) -> Self {
        Self::Admission(Box::new(error.into()))
    }
}
impl From<PError> for HError {
    fn from(error: PError) -> Self {
        Self::Admission(Box::new(error))
    }
}
impl fmt::Display for HError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source-owned neutral preheaders: {self:?}")
    }
}
impl Error for HError {}

/// Unreserved added receipt, excluding the retained genuine promoted prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionLoopPreheadersStorageV1(usize);
impl ProductionLoopPreheadersStorageV1 {
    /// Reserve this added owning receipt before further ledger-controlled work.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

struct PreheaderData {
    tail: PreheaderTail,
    origins: Origins,
    kernels: Box<[FormalMemoryObligations]>,
    added: usize,
}

/// Genuine Direct source/promotion plus actual freshly checked neutral CFG.
/// Synthetic block/edge/parameter origins are not Rust source spans. Original
/// assertions stay attached to their historical owners. Reports are derived
/// afresh on the actual final graph. No LICM, progress, native or launch proof
/// follows; existing source/ranked/formal-engine metering exclusions remain.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOwnedLoopPreheadersContinuationV1 as Owner;
/// fn duplicate(owner: &Owner) -> Owner { owner.clone() }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_lower_mir_kernel::ProductionOwnedLoopPreheadersContinuationV1 as Owner;
/// fn detach(owner: Owner) { let _ = owner.prefix; }
/// ```
pub struct ProductionOwnedLoopPreheadersContinuationV1 {
    prefix: ProductionOwnedPrivateCellPromotionContinuationV1,
    data: PreheaderData,
}

/// UnitLocal owner retains original N, checked E and the genuine prefix once.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionOwnedUnitLocalLoopPreheadersContinuationV1 as New,
///     ProductionOwnedUnitLocalPrivateCellPromotionContinuationV1 as Old};
/// fn relabel(owner: New) -> Old { owner }
/// ```
pub struct ProductionOwnedUnitLocalLoopPreheadersContinuationV1 {
    prefix: ProductionOwnedUnitLocalPrivateCellPromotionContinuationV1,
    data: PreheaderData,
}

#[derive(Clone, Copy)]
enum PromotedPrefix<'a> {
    Direct(&'a ProductionOwnedPrivateCellPromotionContinuationV1),
    Erased(&'a ProductionOwnedUnitLocalPrivateCellPromotionContinuationV1),
}
impl<'a> PromotedPrefix<'a> {
    fn p8(self) -> Prefix8<'a> {
        match self {
            Self::Direct(p) => Prefix8::Direct(&p.prefix),
            Self::Erased(p) => Prefix8::Erased(&p.prefix),
        }
    }
    fn tail(self) -> &'a PromotionTail {
        match self {
            Self::Direct(p) => &p.data.tail,
            Self::Erased(p) => &p.data.tail,
        }
    }
    fn output(self) -> &'a StoreOwner {
        self.tail().output()
    }
    fn floor(self) -> HResult<usize> {
        match self {
            Self::Direct(p) => p.retained_input_storage_floor_v1(),
            Self::Erased(p) => p.retained_input_storage_floor_v1(),
        }
        .map_err(|e| HError::Prefix(Box::new(e)))
    }
    fn replay(self, budget: &mut AssertOriginBudgetV1<'_>) -> HResult<()> {
        match self {
            Self::Direct(p) => p.verify_equivalence(budget),
            Self::Erased(p) => p.verify_equivalence(budget),
        }
        .map_err(|e| HError::Prefix(Box::new(e)))
    }
}

// The inherited scope alone owns cleanup. An Ok(Err(...)) is an unsuccessful
// inner computation, not an owning transfer: its locals have already dropped.
fn preheader_scoped<'w, T>(
    required: usize,
    budget: &mut AssertOriginBudgetV1<'w>,
    run: impl FnOnce(&mut AssertOriginBudgetV1<'w>, &PromotionBinding) -> HResult<T>,
) -> HResult<T> {
    match promotion_scoped(required, budget, |budget, binding| Ok(run(budget, binding))) {
        Ok(result) => result,
        Err(PError::Resource(error)) => Err(HError::Resource(error)),
        Err(PError::Panicked) => Err(HError::Panicked),
        Err(error) => Err(error.into()),
    }
}
fn preheader_header<P, W>() -> HResult<usize> {
    size_of::<W>()
        .checked_sub(size_of::<P>())
        .and_then(|n| n.checked_sub(size_of::<PreheaderTail>()))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn preheader_added(data: &PreheaderData, header: usize) -> HResult<usize> {
    let backing = data.origins.backing()?;
    data.tail
        .retained_storage()
        .checked_add(header)
        .and_then(|n| n.checked_add(backing))
        .and_then(|n| n.checked_add(std::mem::size_of_val(data.kernels.as_ref())))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn preheader_required(
    prefix: PromotedPrefix<'_>,
    data: &PreheaderData,
    header: usize,
) -> HResult<usize> {
    if preheader_added(data, header)? != data.added {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    prefix
        .floor()?
        .checked_add(data.added)
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}

fn check_actual_preheaders(
    prefix: PromotedPrefix<'_>,
    tail: &PreheaderTail,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> HResult<(Origins, Box<[FormalMemoryObligations]>)> {
    let (pair, receipt) = tail
        .replay_against(prefix.output(), budget)
        .map_err(HError::Continuation)?;
    binding.check(budget)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let (input, receipt) = Inventory::derive(prefix.output(), budget).map_err(inventory_error)?;
    binding.check(budget)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let (output, receipt) = Inventory::derive(tail.output(), budget).map_err(inventory_error)?;
    binding.check(budget)?;
    budget.reserve_storage(receipt.retained_storage())?;
    // These owned rows stay outside the nested UnitLocal metadata scope. Only
    // final reports cross it, covered by the existing pre-erasure row prepay.
    let origins = origins::derive(&pair, &input, &output, budget)?;
    binding.check(budget)?;
    let kernels = with_promoted_output_sites(
        prefix.p8(),
        prefix.tail(),
        budget,
        binding,
        |sites, budget, inner| {
            promotion_sites::check_loop_preheader_sites(sites, &pair, &output, budget, inner)
        },
    )?;
    binding.check(budget)?;
    Ok((origins, kernels))
}

fn prepare_preheader_data(
    prefix: PromotedPrefix<'_>,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> HResult<PreheaderData> {
    prefix.replay(budget)?;
    binding.check(budget)?;
    let tail = prepare_preheaders(prefix.output(), budget).map_err(HError::Continuation)?;
    binding.check(budget)?;
    budget.reserve_storage(tail.retained_storage())?;
    let (origins, kernels) = check_actual_preheaders(prefix, &tail, budget, binding)?;
    binding.check(budget)?;
    budget.reserve_storage(header)?;
    let mut data = PreheaderData {
        tail,
        origins,
        kernels,
        added: 0,
    };
    data.added = preheader_added(&data, header)?;
    Ok(data)
}
fn replay_preheader_data(
    prefix: PromotedPrefix<'_>,
    data: &PreheaderData,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> HResult<()> {
    preheader_scoped(
        preheader_required(prefix, data, header)?,
        budget,
        |budget, binding| {
            prefix.replay(budget)?;
            binding.check(budget)?;
            let (origins, kernels) = check_actual_preheaders(prefix, &data.tail, budget, binding)?;
            origins.check_equal(&data.origins, budget)?;
            binding.check(budget)?;
            if kernels != data.kernels {
                return Err(
                    E::Formal(crate::ProductionFormalMemoryErrorV1::ObligationMismatch).into(),
                );
            }
            Ok(())
        },
    )
}

macro_rules! preheader_owner {
    ($prefix:ty, $owner:ident, $variant:ident) => {
        impl $prefix {
            /// Consumes genuine promoted source, inserts eligible neutral
            /// preheaders, independently replays origins and freshly admits the
            /// final output. No fallback or source-name dispatch is used.
            /// The incoming floor survives all exits; reserve the added receipt
            /// before further work. Caller owns inherited credit on failure.
            pub fn continue_loop_preheaders_v1(
                self,
                budget: &mut AssertOriginBudgetV1<'_>,
            ) -> HResult<($owner, ProductionLoopPreheadersStorageV1)> {
                let minimum = PromotedPrefix::$variant(&self).floor()?;
                preheader_scoped(minimum, budget, |budget, binding| {
                    let data = prepare_preheader_data(
                        PromotedPrefix::$variant(&self),
                        preheader_header::<Self, $owner>()?,
                        budget,
                        binding,
                    )?;
                    let storage = ProductionLoopPreheadersStorageV1(data.added);
                    Ok(($owner { prefix: self, data }, storage))
                })
            }
        }
        impl $owner {
            /// Exact genuine promoted source prefix, retained once without cloning.
            pub const fn prefix(&self) -> &$prefix {
                &self.prefix
            }
            /// Actual canonical preheader output and complete independently checked rows.
            pub const fn continuation(&self) -> &PreheaderTail {
                &self.data.tail
            }
            /// Actual final canonical graph, not a relabeled historical endpoint.
            pub fn output(&self) -> &StoreOwner {
                self.data.tail.output()
            }
            /// Inert synthetic block origins in exact selected-header order.
            pub fn origins(&self) -> &[ProductionLoopPreheaderOriginV1] {
                self.data.origins.blocks()
            }
            /// Exact incoming edge occurrences, grouped by each block origin's range.
            pub fn incoming_origins(&self) -> &[ProductionLoopPreheaderIncomingOriginV1] {
                self.data.origins.incoming()
            }
            /// Ordered header/preheader definition pairs, never new Rust source locals.
            pub fn parameter_origins(&self) -> &[ProductionLoopPreheaderParameterOriginV1] {
                self.data.origins.parameters()
            }
            /// Fresh actual-final-graph reports in unchanged kernel roster order.
            pub fn kernels(&self) -> &[FormalMemoryObligations] {
                &self.data.kernels
            }
            /// Added owning receipt excluding all inherited prefix and sibling storage.
            pub const fn additional_retained_storage_v1(&self) -> usize {
                self.data.added
            }
            /// This source/canonical owner grants no artifact, runtime or launch authority.
            pub const fn grants_artifact_or_launch_authority(&self) -> bool {
                false
            }
            /// Minimum retained prefix plus added receipt, excluding caller-owned siblings.
            pub fn retained_input_storage_floor_v1(&self) -> HResult<usize> {
                preheader_required(
                    PromotedPrefix::$variant(&self.prefix),
                    &self.data,
                    preheader_header::<$prefix, Self>()?,
                )
            }
            /// Replays the genuine prefix, actual pair, complete synthetic origins and
            /// fresh final source/census reports while preserving the caller's entry floor.
            pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> HResult<()> {
                replay_preheader_data(
                    PromotedPrefix::$variant(&self.prefix),
                    &self.data,
                    preheader_header::<$prefix, Self>()?,
                    budget,
                )
            }
        }
    };
}
preheader_owner!(
    ProductionOwnedPrivateCellPromotionContinuationV1,
    ProductionOwnedLoopPreheadersContinuationV1,
    Direct
);
preheader_owner!(
    ProductionOwnedUnitLocalPrivateCellPromotionContinuationV1,
    ProductionOwnedUnitLocalLoopPreheadersContinuationV1,
    Erased
);

#[cfg(test)]
#[path = "production_checked_output_loop_preheaders_internal_v1_tests.rs"]
mod tests;

#[path = "production_checked_output_licm_v1.rs"]
mod licm;
pub use licm::{
    ProductionCrossBlockForwardingErrorV1, ProductionCrossBlockForwardingOriginV1,
    ProductionCrossBlockForwardingStorageV1, ProductionInductionRefinementErrorV1,
    ProductionInductionRefinementOriginV1, ProductionInductionRefinementStorageV1,
    ProductionOwnedCrossBlockForwardingContinuationV1,
    ProductionOwnedInductionRefinementContinuationV1,
    ProductionOwnedRefinedCrossBlockForwardingContinuationV1,
    ProductionOwnedUnitLocalCrossBlockForwardingContinuationV1,
    ProductionOwnedUnitLocalInductionRefinementContinuationV1,
    ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1,
    ProductionRefinedCrossBlockForwardingErrorV1, ProductionRefinedCrossBlockForwardingStorageV1,
};
pub use licm::{
    ProductionLicmErrorV1, ProductionLicmStorageV1, ProductionOwnedLicmContinuationV1,
    ProductionOwnedUnitLocalLicmContinuationV1,
};
pub use licm::{
    ProductionLoopInductionQueryErrorV1, ProductionLoopInductionQueryStorageV1,
    ProductionLoopInductionQueryV1,
};
