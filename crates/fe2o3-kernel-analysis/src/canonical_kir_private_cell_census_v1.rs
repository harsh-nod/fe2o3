//! Complete-use static private integer-cell eligibility, not a transformation.
//!
//! Facts borrow one actual immutable inventory. They establish only this closed
//! canonical shape: direct fixed allocation, optional one-level literal GEP,
//! one reachable access block, prior same-cell stores and no unknown intervening
//! effects. Source layout/storage epochs, admission, actual-pair preservation,
//! mutation selection, pass execution and launch authority remain unproved.
//! Stored-value computations (including traps) are not inspected or deleted.

use crate::CanonicalKirInventoryV1 as Inventory;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirControlFlowScopeErrorV1 as CfgError,
    CanonicalKirOperationCoordinateV1 as Coordinate, Constant, ControlFlowLimits,
    KirLocalMemoryEffectRefV1 as Effect, OperationKind as Kind, ScalarType, Type, ValueId,
    with_canonical_kir_control_flow_v1,
};
use std::{fmt, mem::size_of};

#[path = "canonical_kir_private_cell_build_v1.rs"]
mod build;
#[path = "canonical_kir_private_cell_resources_v1.rs"]
mod resources;
use resources::{Meter, scoped};

/// Census denial, distinct from a successfully excluded allocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalKirPrivateCellCensusErrorV1 {
    /// Original live resource denial.
    Resource(Resource),
    /// Existing scoped CFG analysis denial.
    ControlFlow(CfgError),
    /// Connected inventory or private table invariant failed.
    InconsistentInventory,
    /// A caught panic; no partial census is returned.
    Panicked,
}
type Error = CanonicalKirPrivateCellCensusErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<CfgError> for Error {
    fn from(error: CfgError) -> Self {
        Self::ControlFlow(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "static private-cell census: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Explicit structural limits. Array count never determines a table capacity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPrivateCellCensusLimitsV1 {
    /// Maximum positive literal element count; unsupported counts are excluded.
    pub array_count: u64,
    /// Existing once-per-function CFG limits and logical row-cell domain.
    pub control_flow: ControlFlowLimits,
}
impl Default for CanonicalKirPrivateCellCensusLimitsV1 {
    fn default() -> Self {
        Self {
            array_count: 1 << 32,
            control_flow: ControlFlowLimits::default(),
        }
    }
}
type Limits = CanonicalKirPrivateCellCensusLimitsV1;

/// Actual allocation and primitive layout, with complete eligible address/use census.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPrivateCellAllocationV1 {
    pub allocation: Coordinate,
    pub value: ValueId,
    pub element: ScalarType,
    pub count: u64,
    pub alignment: u32,
    /// None for a fully censused unused allocation/address set.
    pub access_block: Option<Block>,
}
type Allocation = CanonicalKirPrivateCellAllocationV1;

/// One actual direct allocation or one-level constant element GEP result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPrivateCellAddressV1 {
    pub allocation: Coordinate,
    pub producer: Coordinate,
    pub value: ValueId,
    pub element_offset: u64,
    pub alignment: u32,
}
type Address = CanonicalKirPrivateCellAddressV1;

/// Original memory occurrence; Load witnesses its exact last same-cell Store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirPrivateCellAccessKindV1 {
    Store {
        value: ValueId,
    },
    Load {
        result: ValueId,
        previous_store: Coordinate,
        stored_value: ValueId,
    },
}
type AccessKind = CanonicalKirPrivateCellAccessKindV1;

/// An original access, not an instruction to replace or delete it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPrivateCellAccessV1 {
    pub allocation: Coordinate,
    pub operation: Coordinate,
    pub pointer: ValueId,
    pub element_offset: u64,
    pub kind: AccessKind,
}
type Access = CanonicalKirPrivateCellAccessV1;

/// Exact new retained capacities plus census header, excluding borrowed input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPrivateCellCensusStorageV1(usize);
impl CanonicalKirPrivateCellCensusStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
type Storage = CanonicalKirPrivateCellCensusStorageV1;

/// Move-only complete census, borrowing the exact connected inventory.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::{CanonicalKirInventoryV1, CanonicalKirPrivateCellCensusV1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn cannot_detach(inventory: CanonicalKirInventoryV1<'_>, budget: &mut Budget<'_>) {
///     let (census, _) = CanonicalKirPrivateCellCensusV1::derive(&inventory, Default::default(), budget).unwrap();
///     drop(inventory);
///     let _ = census.allocations();
/// }
/// ```
pub struct CanonicalKirPrivateCellCensusV1<'i, 'g> {
    inventory: &'i Inventory<'g>,
    allocations: Vec<Allocation>,
    addresses: Vec<Address>,
    accesses: Vec<Access>,
}
impl<'i, 'g> CanonicalKirPrivateCellCensusV1<'i, 'g> {
    /// Complete immutable census with one definition index and sparse observed
    /// accesses, O(V + operations + blocks + functions + U log(U + 2)) new work,
    /// excluding inherited CFG analysis; V counts definitions and U all uses.
    /// Every operation, terminator and edge use is inspected, including
    /// unreachable blocks. A disqualifying use removes the entire allocation.
    ///
    /// Requested capacities are prepaid, actual excess reconciled before table
    /// initialization. Existing CFG storage retains its logical requested-cell
    /// contract, NOT a byte/RSS claim. Success restores entry storage and returns
    /// an unreserved receipt; reserve it before further metered work while the
    /// census lives. Errors/panics drop all local tables before cleanup. Work,
    /// peak and denial history persist; unrelated entry reservations are retained.
    pub fn derive(
        inventory: &'i Inventory<'g>,
        limits: Limits,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        scoped(budget, |meter| build::derive(inventory, limits, meter))
    }
    pub fn inventory(&self) -> &'i Inventory<'g> {
        self.inventory
    }
    pub fn belongs_to(&self, inventory: &Inventory<'_>) -> bool {
        std::ptr::eq(self.inventory, inventory)
    }
    pub fn allocations(&self) -> &[Allocation] {
        &self.allocations
    }
    pub fn addresses(&self) -> &[Address] {
        &self.addresses
    }
    /// Grouped by allocation, element offset and original operation order.
    pub fn accesses(&self) -> &[Access] {
        &self.accesses
    }
    pub const fn grants_transformation_authority(&self) -> bool {
        false
    }
}

fn element(ty: &Type) -> Option<(ScalarType, u32)> {
    let Type::Scalar(scalar) = ty else {
        return None;
    };
    let bytes = match scalar {
        ScalarType::I8 | ScalarType::U8 => 1,
        ScalarType::I16 | ScalarType::U16 => 2,
        ScalarType::I32 | ScalarType::U32 => 4,
        ScalarType::I64 | ScalarType::U64 => 8,
        _ => return None,
    };
    Some((*scalar, bytes))
}
fn pointer(ty: &Type, scalar: ScalarType) -> bool {
    matches!(ty, Type::Pointer(pointer) if pointer.address_space == AddressSpace::Private
        && pointer.access == AccessMode::ReadWrite && *pointer.pointee == Type::Scalar(scalar))
}

#[cfg(test)]
#[path = "canonical_kir_private_cell_census_v1_tests.rs"]
mod tests;
