//! Exact borrowed private-cell input/output relation, not a mutation permission.
//!
//! Every selected allocation is completely removed; Loads retain their results
//! through the closed integer `BitOr(value, value)` recipe. Source lifetime,
//! allocation/resource refinement, native behavior and execution authority are
//! not established. No graph is created, cloned, edited or relabeled here.

use crate::{
    CanonicalKirInventoryErrorV1 as InventoryError, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirPrivateCellAccessKindV1 as AccessKind,
    CanonicalKirPrivateCellCensusErrorV1 as CensusError,
    CanonicalKirPrivateCellCensusLimitsV1 as Limits, CanonicalKirPrivateCellCensusV1 as Census,
    canonical_kir_private_cell_pair_resources_v1 as resources,
};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirOperationCoordinateV1 as Coordinate, Module, Operation, OperationKind as Kind,
    ValueId, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{fmt, mem::size_of};

#[path = "canonical_kir_private_cell_pair_check_v1.rs"]
mod check;

/// Inert classification of one complete output occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirPrivateCellOriginKindV1 {
    /// The input operation is retained exactly.
    Retained,
    /// A Load retains its result IDs/types through the fixed integer copy.
    LoadCopy {
        /// Actual selected input allocation.
        allocation: Coordinate,
        /// Actual latest same-cell input Store.
        previous_store: Coordinate,
        /// Exact SSA value written by that Store.
        stored_value: ValueId,
    },
}
type OriginKind = CanonicalKirPrivateCellOriginKindV1;

/// Inert claimed input/output operation join; not independently checked evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPrivateCellOriginV1 {
    /// Original operation occurrence.
    pub input: Coordinate,
    /// Final operation occurrence.
    pub output: Coordinate,
    /// Exact retained/copy classification.
    pub kind: OriginKind,
}
type Origin = CanonicalKirPrivateCellOriginV1;

/// Closed pair denial, distinct from unsupported unselected allocations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalKirPrivateCellPromotionErrorV1 {
    /// Original resource denial.
    Resource(Resource),
    /// Actual connected inventory denial.
    Inventory(InventoryError),
    /// Fresh complete-use census denial.
    Census(CensusError),
    /// Exact pair or claimed occurrence mismatch.
    Mismatch(&'static str),
    /// A caught panic; no partial relation escapes.
    Panicked,
}
type Error = CanonicalKirPrivateCellPromotionErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
fn scoped<'w, T>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Meter<'_, 'w>) -> Result<T>,
) -> Result<T> {
    resources::scoped(budget, run)
}
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<InventoryError> for Error {
    fn from(value: InventoryError) -> Self {
        Self::Inventory(value)
    }
}
impl From<CensusError> for Error {
    fn from(value: CensusError) -> Self {
        Self::Census(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "private-cell pair: {self:?}")
    }
}
impl std::error::Error for Error {}

/// New retained witness header only; borrowed owners and claim backing excluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPrivateCellPromotionStorageV1(usize);
impl CanonicalKirPrivateCellPromotionStorageV1 {
    /// Reserve before further metered work while the returned witness lives.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
type Storage = CanonicalKirPrivateCellPromotionStorageV1;

/// Move-only, actual-pair relation borrowing exactly the supplied owners/claims.
/// Independently admitted equal-byte owners are valid semantic inputs, not
/// authenticated execution of a policy. Source/native authority remains absent.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::check_canonical_kir_private_cell_promotion_v1;
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn detach(input: Owner, output: Owner, budget: &mut Budget<'_>) {
///     let (checked, _) = check_canonical_kir_private_cell_promotion_v1(
///         &input, &output, &[], &[], Default::default(), budget).unwrap();
///     drop(input);
///     let _ = checked.input();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::{check_canonical_kir_private_cell_promotion_v1,
///     CanonicalKirPrivateCellOriginV1};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn detach(input: &Owner, output: &Owner, budget: &mut Budget<'_>) {
///     let rows: Vec<CanonicalKirPrivateCellOriginV1> = Vec::new();
///     let (checked, _) = check_canonical_kir_private_cell_promotion_v1(
///         input, output, &[], &rows, Default::default(), budget).unwrap();
///     drop(rows);
///     let _ = checked.origins();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::CheckedCanonicalKirPrivateCellPromotionV1;
/// fn duplicate(checked: CheckedCanonicalKirPrivateCellPromotionV1<'_>) {
///     let moved = checked;
///     let _ = checked.input();
///     let _ = moved.output();
/// }
/// ```
pub struct CheckedCanonicalKirPrivateCellPromotionV1<'a> {
    input: &'a Owner,
    output: &'a Owner,
    selected: &'a [Coordinate],
    origins: &'a [Origin],
}
impl<'a> CheckedCanonicalKirPrivateCellPromotionV1<'a> {
    /// Actual supplied input owner, not a reconstructed equivalent.
    pub const fn input(&self) -> &'a Owner {
        self.input
    }
    /// Actual supplied output owner.
    pub const fn output(&self) -> &'a Owner {
        self.output
    }
    /// Exact ordered unique selected allocation claims.
    pub const fn selected_allocations(&self) -> &'a [Coordinate] {
        self.selected
    }
    /// Complete actual output roster, including checked Load copies.
    pub const fn origins(&self) -> &'a [Origin] {
        self.origins
    }
    /// No source, mutation, execution, publication or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Rebuilds actual input/output inventories and the complete input census.
/// Selection is an ordered unique subset, but each selected allocation must be
/// removed completely. Every output occurrence has exactly one ordered origin.
/// Empty selection requires an exact no-op pair. No producer census is trusted.
///
/// New action metadata is linear in actual input operations, never array count
/// or sparse IDs. The two inherited inventories and one census/CFG analysis keep
/// their own bounded work/storage contracts. Inventory/CFG logical payload plus
/// actual new capacities is not an allocator/RSS bound. Borrowed owners/claims
/// remain caller-owned; existing reservations are the untouched entry floor.
///
/// Success returns an unreserved header receipt after dropping all scratch and
/// restoring entry storage. Errors/panics drop local owners before valid-ledger
/// cleanup; accepted work, peak and failure history are never reset. No external
/// callback, mutable graph or detached source proof enters this API.
pub fn check_canonical_kir_private_cell_promotion_v1<'a>(
    input: &'a Owner,
    output: &'a Owner,
    selected_allocations: &'a [Coordinate],
    origins: &'a [Origin],
    limits: Limits,
    budget: &mut Budget<'_>,
) -> Result<(CheckedCanonicalKirPrivateCellPromotionV1<'a>, Storage)> {
    scoped(budget, |meter| {
        check::check(input, output, selected_allocations, origins, limits, meter)
    })
}

#[cfg(test)]
#[path = "canonical_kir_private_cell_pair_v1_tests.rs"]
mod tests;
