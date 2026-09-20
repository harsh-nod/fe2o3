//! Consuming source-bound private-cell promotion after the actual P8 output.
//! This continuation does not allocate a numbered schedule or native authority.
use super::*;
use fe2o3_kernel_opt::{
    OwnedPrivateCellPromotionContinuationV1 as PromotionTail,
    prepare_owned_private_cell_promotion_v1 as prepare_promotion,
};

/// Exact refusal of the source-owned private-cell continuation.
#[derive(Debug)]
pub enum ProductionPrivateCellPromotionContinuationErrorV1 {
    /// Cumulative work/storage or custody of the live ledger was refused.
    Resource(AssertOriginResourceV1),
    /// The retained source-to-P8 prefix was refused.
    Prefix(Box<ProductionCommutativeContinuationErrorV1>),
    /// The actual promotion transaction or independent full pair was refused.
    Continuation(fe2o3_kernel_opt::OwnedPrivateCellPromotionErrorV1),
    /// Source-coordinate transport or fresh output admission was refused.
    Admission(ProductionCheckedOutputAdmissionErrorPolicy3V1),
    /// No partly checked owner escapes a caught private-phase panic.
    Panicked,
}
type PError = ProductionPrivateCellPromotionContinuationErrorV1;
type PResult<T> = Result<T, PError>;
impl fmt::Display for PError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source-owned private-cell continuation: {self:?}")
    }
}
impl Error for PError {}
impl From<AssertOriginResourceV1> for PError {
    fn from(value: AssertOriginResourceV1) -> Self {
        Self::Resource(value)
    }
}
impl From<E> for PError {
    fn from(value: E) -> Self {
        Self::Admission(value)
    }
}
impl From<CError> for PError {
    fn from(value: CError) -> Self {
        Self::Prefix(Box::new(value))
    }
}

/// Added promoted output, complete origins and fresh reports, excluding P8.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionPrivateCellPromotionContinuationStorageV1(usize);
impl ProductionPrivateCellPromotionContinuationStorageV1 {
    /// Reserve this unreserved owning transfer before further controlled work.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

struct PromotionData {
    tail: PromotionTail,
    kernels: Box<[FormalMemoryObligations]>,
    added: usize,
}

/// Genuine Direct source/P8 owner, moved once, and the actual promoted output.
/// Source lifetime/layout obligations are replayed before any memory deletion.
/// Every output operation retains a checked original source-site join; every
/// remaining memory operation and every native/formal census is checked afresh.
/// Historical owners/reports are never relabeled as the promoted graph. Existing
/// source/ranked/formal engine metering exclusions remain unchanged. This is
/// bounded private-cell promotion, not general alias analysis or Rust-to-SSA.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOwnedPrivateCellPromotionContinuationV1;
/// fn clone(value: ProductionOwnedPrivateCellPromotionContinuationV1) { let _ = value.clone(); }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_lower_mir_kernel::ProductionOwnedPrivateCellPromotionContinuationV1;
/// fn detach(value: ProductionOwnedPrivateCellPromotionContinuationV1) { let _ = value.prefix; }
/// ```
pub struct ProductionOwnedPrivateCellPromotionContinuationV1 {
    prefix: ProductionOwnedCommutativeContinuationV1,
    data: PromotionData,
}

/// UnitLocal counterpart retaining original N, checked erasure E and P8 once.
/// No cloned source, detached source claim or borrowed self-reference is stored.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionOwnedUnitLocalPrivateCellPromotionContinuationV1 as New, ProductionOwnedUnitLocalCommutativeContinuationV1 as Old};
/// fn relabel(value: New) -> Old { value }
/// ```
pub struct ProductionOwnedUnitLocalPrivateCellPromotionContinuationV1 {
    prefix: ProductionOwnedUnitLocalCommutativeContinuationV1,
    data: PromotionData,
}

#[derive(Clone, Copy)]
enum Prefix8<'a> {
    Direct(&'a ProductionOwnedCommutativeContinuationV1),
    Erased(&'a ProductionOwnedUnitLocalCommutativeContinuationV1),
}
impl<'a> Prefix8<'a> {
    fn previous(self) -> Prefix<'a> {
        match self {
            Self::Direct(p) => Prefix::Direct(p.prefix()),
            Self::Erased(p) => Prefix::Erased(p.prefix()),
        }
    }
    fn historical(self) -> StorePrefix<'a> {
        self.previous().historical()
    }
    fn output(self) -> &'a StoreOwner {
        match self {
            Self::Direct(p) => p.output(),
            Self::Erased(p) => p.output(),
        }
    }
    fn continuation(self) -> &'a Tail {
        match self {
            Self::Direct(p) => p.continuation(),
            Self::Erased(p) => p.continuation(),
        }
    }
    fn floor(self) -> PResult<usize> {
        match self {
            Self::Direct(p) => p.retained_input_storage_floor_v1(),
            Self::Erased(p) => p.retained_input_storage_floor_v1(),
        }
        .map_err(Into::into)
    }
    fn replay(self, budget: &mut AssertOriginBudgetV1<'_>) -> PResult<()> {
        match self {
            Self::Direct(p) => p.verify_equivalence(budget),
            Self::Erased(p) => p.verify_equivalence(budget),
        }
        .map_err(Into::into)
    }
}

#[path = "production_checked_output_private_cell_scope_v1.rs"]
mod promotion_scope;
use promotion_scope::{PromotionBinding, promotion_scoped};
#[path = "production_checked_output_private_cell_sites_v1.rs"]
mod promotion_sites;

fn promotion_header<P, W>() -> PResult<usize> {
    size_of::<W>()
        .checked_sub(size_of::<P>())
        .and_then(|n| n.checked_sub(size_of::<PromotionTail>()))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn promotion_added(data: &PromotionData, header: usize) -> PResult<usize> {
    data.tail
        .retained_storage()
        .checked_add(header)
        .and_then(|n| n.checked_add(std::mem::size_of_val(data.kernels.as_ref())))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn promotion_required(prefix: Prefix8<'_>, data: &PromotionData, header: usize) -> PResult<usize> {
    if promotion_added(data, header)? != data.added {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    prefix
        .floor()?
        .checked_add(data.added)
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}

fn prepare_promotion_data(
    prefix: Prefix8<'_>,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> PResult<PromotionData> {
    // A final graph with no Loads cannot discharge the original source's
    // lifetime/layout requirements. Replay them before asking for deletion.
    prefix.replay(budget)?;
    binding.check(budget)?;
    let tail = prepare_promotion(prefix.output(), budget).map_err(PError::Continuation)?;
    binding.check(budget)?;
    budget.reserve_storage(tail.retained_storage())?;
    let kernels = check_promoted_output(prefix, &tail, budget, binding)?;
    binding.check(budget)?;
    budget.reserve_storage(header)?;
    let mut data = PromotionData {
        tail,
        kernels,
        added: 0,
    };
    data.added = promotion_added(&data, header)?;
    Ok(data)
}

fn replay_promotion_data(
    prefix: Prefix8<'_>,
    data: &PromotionData,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> PResult<()> {
    promotion_scoped(
        promotion_required(prefix, data, header)?,
        budget,
        |budget, binding| {
            let fresh = check_promoted_output(prefix, &data.tail, budget, binding)?;
            binding.check(budget)?;
            if fresh != data.kernels {
                return Err(
                    E::Formal(crate::ProductionFormalMemoryErrorV1::ObligationMismatch).into(),
                );
            }
            Ok(())
        },
    )
}

fn check_promoted_output(
    prefix: Prefix8<'_>,
    tail: &PromotionTail,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> PResult<Box<[FormalMemoryObligations]>> {
    with_promoted_output_sites(prefix, tail, budget, binding, promotion_sites::check_sites)
}

// Private checked metadata cannot escape its source/inventory/erasure scope.
// Keep the existing report-row prepayment and charge order for the census path.
fn with_promoted_output_sites<'w, R>(
    prefix: Prefix8<'_>,
    tail: &PromotionTail,
    budget: &mut AssertOriginBudgetV1<'w>,
    binding: &PromotionBinding,
    use_sites: impl for<'s, 'g> FnOnce(
        promotion_sites::CheckedPromotedSites<'s, 'g>,
        &mut AssertOriginBudgetV1<'w>,
        &PromotionBinding,
    ) -> PResult<R>,
) -> PResult<R> {
    budget.charge_work(3)?;
    prefix.replay(budget)?;
    binding.check(budget)?;
    let historical = prefix.historical();
    let source = historical.source();
    let (relation, storage) = tail
        .replay_against(prefix.output(), budget)
        .map_err(PError::Continuation)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (input, storage) =
        CanonicalKirInventoryV1::derive(prefix.output(), budget).map_err(inventory_error)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (output, storage) =
        CanonicalKirInventoryV1::derive(tail.output(), budget).map_err(inventory_error)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    // These returned rows survive the scoped UnitLocal erasure callback.
    let rows = output
        .owner()
        .module()
        .kernels
        .len()
        .checked_mul(size_of::<FormalMemoryObligations>())
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    budget.reserve_storage(rows)?;
    let (coordinates, storage) =
        fe2o3_kernel_analysis::check_canonical_kir_coordinate_preservation_v1(
            source.neutral()?,
            historical.bound(),
            budget,
        )
        .map_err(E::Coordinates)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (bound, storage) =
        CanonicalKirInventoryV1::derive(historical.bound(), budget).map_err(inventory_error)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let mapping = promotion_sites::PromotionMapping {
        input: &input,
        output: &output,
        relation: &relation,
    };
    match source {
        GeneralSourceContextV1::Direct(source) => {
            let sites = private_memory::source_statement_sites_v1(source, &bound, budget)?;
            binding.check(budget)?;
            promotion_sites::with_checked_sites(
                prefix, &bound, &sites, mapping, budget, binding, use_sites,
            )
        }
        GeneralSourceContextV1::Erased(source) => source
            .with_checked_erasure_v1(budget, |erasure, budget| {
                Ok(promotion_scoped(
                    budget.storage(),
                    budget,
                    |budget, inner| {
                        binding.check(budget)?;
                        if !std::ptr::eq(erasure.output(), coordinates.input()) {
                            return Err(E::SourceOutput(
                                ProductionSourceOutputErrorV1::InputCustody,
                            )
                            .into());
                        }
                        let map = ErasedSourceCoordinateMapV1 {
                            deletion: erasure,
                            floor: budget.storage(),
                        };
                        let sites = private_memory::erased_source_statement_sites_v1(
                            source, &map, &bound, budget,
                        )?;
                        inner.check(budget)?;
                        promotion_sites::with_checked_sites(
                            prefix, &bound, &sites, mapping, budget, inner, use_sites,
                        )
                    },
                ))
            })
            .map_err(E::Source)?,
    }
}

macro_rules! promotion_owner {
    ($prefix:ty, $owner:ident, $variant:ident) => {
        impl $prefix {
            /// Consumes genuine P8, replays all source obligations, then performs
            /// one bounded checked private-cell promotion. Unsupported cells are
            /// unchanged; execution/check failures abort without a fallback.
            /// Caller entry storage is preserved on every exit. On success only
            /// the added output/metadata/reports transfer unreserved; caller owns
            /// cleanup of the inherited reservation after consumed failures.
            pub fn continue_private_cell_promotion_v1(
                self,
                budget: &mut AssertOriginBudgetV1<'_>,
            ) -> PResult<($owner, ProductionPrivateCellPromotionContinuationStorageV1)> {
                let minimum = Prefix8::$variant(&self).floor()?;
                promotion_scoped(minimum, budget, |budget, binding| {
                    let data = prepare_promotion_data(
                        Prefix8::$variant(&self),
                        promotion_header::<Self, $owner>()?,
                        budget,
                        binding,
                    )?;
                    let receipt = ProductionPrivateCellPromotionContinuationStorageV1(data.added);
                    Ok(($owner { prefix: self, data }, receipt))
                })
            }
        }
        impl $owner {
            /// Exact source-bearing P8 prefix, retained once.
            pub const fn prefix(&self) -> &$prefix {
                &self.prefix
            }
            /// Actual promotion output/selection/origins, never a detached claim.
            pub const fn continuation(&self) -> &PromotionTail {
                &self.data.tail
            }
            /// The actual promoted canonical graph.
            pub fn output(&self) -> &StoreOwner {
                self.data.tail.output()
            }
            /// Fresh promoted-output reports in actual kernel order.
            pub fn kernels(&self) -> &[FormalMemoryObligations] {
                &self.data.kernels
            }
            /// Exact added owning receipt, excluding all inherited reservations.
            pub const fn additional_retained_storage_v1(&self) -> usize {
                self.data.added
            }
            /// This owner is not native, protected-runtime or launch authority.
            pub const fn grants_artifact_or_launch_authority(&self) -> bool {
                false
            }
            /// Inherited minimum plus added receipt; B remains separately prepaid.
            pub fn retained_input_storage_floor_v1(&self) -> PResult<usize> {
                promotion_required(
                    Prefix8::$variant(&self.prefix),
                    &self.data,
                    promotion_header::<$prefix, Self>()?,
                )
            }
            /// Fresh full source/P8 replay, promotion pair, source origins and
            /// promoted safety/report equality, without trusting cached success.
            pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> PResult<()> {
                replay_promotion_data(
                    Prefix8::$variant(&self.prefix),
                    &self.data,
                    promotion_header::<$prefix, Self>()?,
                    budget,
                )
            }
        }
    };
}
promotion_owner!(
    ProductionOwnedCommutativeContinuationV1,
    ProductionOwnedPrivateCellPromotionContinuationV1,
    Direct
);
promotion_owner!(
    ProductionOwnedUnitLocalCommutativeContinuationV1,
    ProductionOwnedUnitLocalPrivateCellPromotionContinuationV1,
    Erased
);

#[cfg(test)]
#[path = "production_checked_output_private_cell_internal_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "production_checked_output_private_cell_metadata_v1_tests.rs"]
mod metadata_tests;
