//! Consuming source-to-K continuation of the actual admitted redundant-store J.
//! No numbered policy, descriptor, native artifact or default route is allocated.
use super::*;
use fe2o3_pliron::{
    OwnedCommutativeBitwiseContinuationV1 as Tail,
    prepare_owned_commutative_bitwise_continuation_v1 as prepare_tail,
};
use std::mem::size_of;

/// Exact refusal of the source-owned post-J commutative continuation.
#[derive(Debug)]
pub enum ProductionCommutativeContinuationErrorV1 {
    /// The live canonical work/storage/accounting contract was refused.
    Resource(AssertOriginResourceV1),
    /// The complete retained source-to-J prefix was refused.
    Prefix(Box<ProductionRedundantStoreAdmissionErrorV1>),
    /// Actual execution, occurrence custody or independent J/K replay was refused.
    Continuation(fe2o3_pliron::CommutativeBitwiseOptimizationErrorV1),
    /// Genuine source coordinates or fresh final-output admission was refused.
    Admission(ProductionCheckedOutputAdmissionErrorPolicy3V1),
    /// A private phase unwound; no partly checked owner was returned.
    Panicked,
}
type CError = ProductionCommutativeContinuationErrorV1;
type CResult<T> = Result<T, CError>;
impl fmt::Display for CError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source-owned commutative continuation: {self:?}")
    }
}
impl Error for CError {}
impl From<AssertOriginResourceV1> for CError {
    fn from(value: AssertOriginResourceV1) -> Self {
        Self::Resource(value)
    }
}
impl From<E> for CError {
    fn from(value: E) -> Self {
        Self::Admission(value)
    }
}

/// Added K/metadata/fresh-report storage only, excluding the actual prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionCommutativeContinuationStorageV1(usize);
impl ProductionCommutativeContinuationStorageV1 {
    /// Reserve this unreserved transfer before another controlled allocation.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

struct Data {
    tail: Tail,
    kernels: Box<[FormalMemoryObligations]>,
    added: usize,
}

/// Actual Direct source/prefix retained once, actual post-J K and fresh K reports.
/// Private construction consumes the prefix; no equal-byte foreign tail can be
/// attached. Historical reports/graphs remain historical, never renamed K.
/// Source/ranked/formal engine metering exclusions are inherited; this is not a
/// new signed proof, policy record, artifact or default-pipeline permission.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOwnedCommutativeContinuationV1;
/// fn clone(value: ProductionOwnedCommutativeContinuationV1) { let _ = value.clone(); }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_lower_mir_kernel::ProductionOwnedCommutativeContinuationV1;
/// fn replace(value: ProductionOwnedCommutativeContinuationV1) { let _ = value.prefix; }
/// ```
pub struct ProductionOwnedCommutativeContinuationV1 {
    prefix: ProductionOwnedRedundantStoreContinuationV1,
    data: Data,
}

/// UnitLocal source/prefix counterpart preserving original N and checked E once.
/// It contains no borrow into itself and cannot be relabeled as an old owner.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionOwnedUnitLocalCommutativeContinuationV1 as New, ProductionOwnedUnitLocalRedundantStoreContinuationV1 as Old};
/// fn relabel(value: New) -> Old { value }
/// ```
pub struct ProductionOwnedUnitLocalCommutativeContinuationV1 {
    prefix: ProductionOwnedUnitLocalRedundantStoreContinuationV1,
    data: Data,
}

#[derive(Clone, Copy)]
enum Prefix<'a> {
    Direct(&'a ProductionOwnedRedundantStoreContinuationV1),
    Erased(&'a ProductionOwnedUnitLocalRedundantStoreContinuationV1),
}
impl<'a> Prefix<'a> {
    fn historical(self) -> StorePrefix<'a> {
        match self {
            Self::Direct(p) => StorePrefix::Direct(p.prefix()),
            Self::Erased(p) => StorePrefix::Erased(p.prefix()),
        }
    }
    fn output(self) -> &'a StoreOwner {
        match self {
            Self::Direct(p) => p.output(),
            Self::Erased(p) => p.output(),
        }
    }
    fn continuation(self) -> &'a fe2o3_kernel_opt::OwnedRedundantStoreContinuationV1 {
        match self {
            Self::Direct(p) => p.continuation(),
            Self::Erased(p) => p.continuation(),
        }
    }
    fn floor(self) -> CResult<usize> {
        match self {
            Self::Direct(p) => p.retained_input_storage_floor_v1(),
            Self::Erased(p) => p.retained_input_storage_floor_v1(),
        }
        .map_err(|e| CError::Prefix(Box::new(e)))
    }
    fn replay(self, budget: &mut AssertOriginBudgetV1<'_>) -> CResult<()> {
        match self {
            Self::Direct(p) => p.verify_equivalence(budget),
            Self::Erased(p) => p.verify_equivalence(budget),
        }
        .map_err(|e| CError::Prefix(Box::new(e)))
    }
}

#[path = "production_checked_output_commutative_scope_v1.rs"]
mod scope;
use scope::{Binding, scoped};
#[path = "production_checked_output_commutative_sites_v1.rs"]
mod source_sites;

fn header<P, W>() -> CResult<usize> {
    size_of::<W>()
        .checked_sub(size_of::<P>())
        .and_then(|n| n.checked_sub(size_of::<Tail>()))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn added(data: &Data, header: usize) -> CResult<usize> {
    data.tail
        .retained_storage()
        .checked_add(header)
        .and_then(|n| n.checked_add(std::mem::size_of_val(data.kernels.as_ref())))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn required(prefix: Prefix<'_>, data: &Data, header: usize) -> CResult<usize> {
    if added(data, header)? != data.added {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    prefix
        .floor()?
        .checked_add(data.added)
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}

fn prepare_data(
    prefix: Prefix<'_>,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &Binding,
) -> CResult<Data> {
    prefix.replay(budget)?;
    binding.check(budget)?;
    // The one actual service borrows this exact J and privately detaches only
    // owned output/rows before this source prefix is moved into the result.
    let tail = prepare_tail(prefix.output(), budget).map_err(CError::Continuation)?;
    binding.check(budget)?;
    budget.reserve_storage(tail.retained_storage())?;
    let kernels = check_output(prefix, &tail, budget, binding)?;
    binding.check(budget)?;
    budget.reserve_storage(header)?;
    let mut data = Data {
        tail,
        kernels,
        added: 0,
    };
    data.added = added(&data, header)?;
    Ok(data)
}

fn replay_data(
    prefix: Prefix<'_>,
    data: &Data,
    header: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> CResult<()> {
    scoped(
        required(prefix, data, header)?,
        budget,
        |budget, binding| {
            let fresh = check_output(prefix, &data.tail, budget, binding)?;
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

fn check_output(
    prefix: Prefix<'_>,
    tail: &Tail,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &Binding,
) -> CResult<Box<[FormalMemoryObligations]>> {
    budget.charge_work(3)?;
    prefix.replay(budget)?;
    binding.check(budget)?;
    let historical = prefix.historical();
    let source = historical.source();
    let (input, storage) =
        CanonicalKirInventoryV1::derive(prefix.output(), budget).map_err(inventory_error)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (output, storage) =
        CanonicalKirInventoryV1::derive(tail.output(), budget).map_err(inventory_error)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    // This single complete replay binds all retained operations AND predecessor
    // terminator operands, including nested commutative substitutions.
    let (relation, storage) = tail
        .check_inventories_v1(&input, &output, budget)
        .map_err(CError::Continuation)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    // Returned report rows must outlive the scoped UnitLocal erasure callback.
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
    match source {
        GeneralSourceContextV1::Direct(source) => {
            let sites = private_memory::source_statement_sites_v1(source, &bound, budget)?;
            binding.check(budget)?;
            source_sites::check(prefix, &bound, &sites, &relation, budget, binding)
        }
        GeneralSourceContextV1::Erased(source) => source
            .with_checked_erasure_v1(budget, |erasure, budget| {
                Ok(scoped(budget.storage(), budget, |budget, inner| {
                    binding.check(budget)?;
                    if !std::ptr::eq(erasure.output(), coordinates.input()) {
                        return Err(
                            E::SourceOutput(ProductionSourceOutputErrorV1::InputCustody).into()
                        );
                    }
                    let map = ErasedSourceCoordinateMapV1 {
                        deletion: erasure,
                        floor: budget.storage(),
                    };
                    let sites = private_memory::erased_source_statement_sites_v1(
                        source, &map, &bound, budget,
                    )?;
                    inner.check(budget)?;
                    source_sites::check(prefix, &bound, &sites, &relation, budget, inner)
                }))
            })
            .map_err(E::Source)?,
    }
}

macro_rules! consuming_owner {
    ($prefix:ty, $owner:ident, $variant:ident) => {
        impl $prefix {
            /// Consumes the exact admitted source-to-J prefix and executes one
            /// checked commutative continuation. The complete caller entry floor
            /// is preserved on every exit, including consumed failures. Caller
            /// owns inherited reservation cleanup; success transfers only added
            /// K/metadata/report storage unreserved. No foreign tail is accepted.
            pub fn continue_commutative_bitwise_cse_v1(
                self,
                budget: &mut AssertOriginBudgetV1<'_>,
            ) -> CResult<($owner, ProductionCommutativeContinuationStorageV1)> {
                let minimum = Prefix::$variant(&self).floor()?;
                scoped(minimum, budget, |budget, binding| {
                    let data = prepare_data(
                        Prefix::$variant(&self),
                        header::<Self, $owner>()?,
                        budget,
                        binding,
                    )?;
                    let receipt = ProductionCommutativeContinuationStorageV1(data.added);
                    Ok(($owner { prefix: self, data }, receipt))
                })
            }
        }
        impl $owner {
            /// The original source-bearing P7 prefix, moved exactly once.
            pub const fn prefix(&self) -> &$prefix {
                &self.prefix
            }
            /// Actual J/K execution and complete independently checked observations.
            pub const fn continuation(&self) -> &Tail {
                &self.data.tail
            }
            /// Actual new K, never the historical J/I or original N/E.
            pub fn output(&self) -> &StoreOwner {
                self.data.tail.output()
            }
            /// Fresh checked K formal obligations in actual kernel order.
            pub fn kernels(&self) -> &[FormalMemoryObligations] {
                &self.data.kernels
            }
            /// Additional receipt excluding all inherited reservations.
            pub const fn additional_retained_storage_v1(&self) -> usize {
                self.data.added
            }
            /// This owner grants no native artifact, protected or launch authority.
            pub const fn grants_artifact_or_launch_authority(&self) -> bool {
                false
            }
            /// Inherited minimum plus exact added storage; B stays separately prepaid.
            pub fn retained_input_storage_floor_v1(&self) -> CResult<usize> {
                required(
                    Prefix::$variant(&self.prefix),
                    &self.data,
                    header::<$prefix, Self>()?,
                )
            }
            /// Replays the entire actual source/prefix, complete J/K relation,
            /// exact source-site transport and fresh K safety/report equality.
            pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> CResult<()> {
                replay_data(
                    Prefix::$variant(&self.prefix),
                    &self.data,
                    header::<$prefix, Self>()?,
                    budget,
                )
            }
        }
    };
}
consuming_owner!(
    ProductionOwnedRedundantStoreContinuationV1,
    ProductionOwnedCommutativeContinuationV1,
    Direct
);
consuming_owner!(
    ProductionOwnedUnitLocalRedundantStoreContinuationV1,
    ProductionOwnedUnitLocalCommutativeContinuationV1,
    Erased
);

#[cfg(test)]
#[path = "production_checked_output_commutative_internal_v1_tests.rs"]
mod tests;

#[path = "production_checked_output_private_cell_v1.rs"]
mod private_cell;
pub use private_cell::{
    ProductionLicmErrorV1, ProductionLicmStorageV1, ProductionOwnedLicmContinuationV1,
    ProductionOwnedUnitLocalLicmContinuationV1,
};
pub use private_cell::{
    ProductionLoopInductionQueryErrorV1, ProductionLoopInductionQueryStorageV1,
    ProductionLoopInductionQueryV1,
};
pub use private_cell::{
    ProductionLoopPreheaderIncomingOriginV1, ProductionLoopPreheaderOriginV1,
    ProductionLoopPreheaderParameterOriginV1, ProductionLoopPreheadersErrorV1,
    ProductionLoopPreheadersStorageV1, ProductionOwnedLoopPreheadersContinuationV1,
    ProductionOwnedUnitLocalLoopPreheadersContinuationV1,
};
pub use private_cell::{
    ProductionOwnedPrivateCellPromotionContinuationV1,
    ProductionOwnedUnitLocalPrivateCellPromotionContinuationV1,
    ProductionPrivateCellPromotionContinuationErrorV1,
    ProductionPrivateCellPromotionContinuationStorageV1,
};
