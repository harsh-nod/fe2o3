//! Exact neutral preheader subdivision of a connected canonical pair.
//! No motion, reducibility, no-wrap, progress, source or execution authority.
use crate::{
    CanonicalKirInventoryErrorV1 as InventoryError, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirLoopErrorV1 as LoopError, CanonicalKirLoopLimitsV1 as Limits,
    CanonicalKirLoopsV1 as Loops, canonical_kir_private_cell_pair_resources_v1 as resources,
};
use fe2o3_kernel_ir::{
    BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirEdgeCoordinateV1 as Edge, Module,
    Terminator, ValueId, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{fmt, mem::size_of};

#[path = "canonical_kir_loop_preheaders_check_v1.rs"]
mod check;

/// Inert original header/new appended block coordinates, not proof authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirLoopPreheaderV1 {
    pub header: Block,
    pub preheader: Block,
}
type Row = CanonicalKirLoopPreheaderV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalKirLoopPreheadersErrorV1 {
    Resource(Resource),
    Inventory(InventoryError),
    Loops(LoopError),
    Mismatch(&'static str),
    Panicked,
}
type Error = CanonicalKirLoopPreheadersErrorV1;
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
impl From<LoopError> for Error {
    fn from(value: LoopError) -> Self {
        Self::Loops(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "neutral preheader pair: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Unreserved new witness header only. Borrowed graph/row backing is excluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirLoopPreheadersStorageV1(usize);
impl CanonicalKirLoopPreheadersStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
type Storage = CanonicalKirLoopPreheadersStorageV1;

/// Move-only actual-pair relation borrowing both owners and all supplied rows.
/// Equal canonical bytes are permitted; no digest substitutes for this check.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::check_canonical_kir_loop_preheaders_v1;
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn detach(a: Owner, b: Owner, budget: &mut Budget<'_>) {
///     let (pair, _) = check_canonical_kir_loop_preheaders_v1(&a, &b, &[], Default::default(), budget).unwrap();
///     drop(a);
///     let _ = pair.input();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_analysis::{check_canonical_kir_loop_preheaders_v1, CanonicalKirLoopPreheaderV1};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn detach(a: &Owner, b: &Owner, budget: &mut Budget<'_>) {
///     let rows = Vec::<CanonicalKirLoopPreheaderV1>::new();
///     let (pair, _) = check_canonical_kir_loop_preheaders_v1(a, b, &rows, Default::default(), budget).unwrap();
///     drop(rows);
///     let _ = pair.preheaders();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_analysis::CheckedCanonicalKirLoopPreheadersV1;
/// fn duplicate<'a>(pair: &CheckedCanonicalKirLoopPreheadersV1<'a>)
///     -> CheckedCanonicalKirLoopPreheadersV1<'a> { pair.clone() }
/// ```
pub struct CheckedCanonicalKirLoopPreheadersV1<'a> {
    input: &'a Owner,
    output: &'a Owner,
    rows: &'a [Row],
}
impl<'a> CheckedCanonicalKirLoopPreheadersV1<'a> {
    pub const fn input(&self) -> &'a Owner {
        self.input
    }
    pub const fn output(&self) -> &'a Owner {
        self.output
    }
    pub const fn preheaders(&self) -> &'a [Row] {
        self.rows
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Independently derives and replays input AND output loops from their actual
/// inventories. Checks the complete eligible roster in original header order,
/// fresh deterministic IDs, every original payload and exact edge occurrence,
/// and each empty typed forwarding block. Never calls a producer or copies a
/// graph. Entry headers, existing preheaders and ineligible loops stay unchanged.
/// An empty selected roster requires byte-identical canonical input/output.
///
/// Beyond the existing bounded loop/CFG services, storage is O(F+E), and work
/// is O(F+B+D+E+R+wire bytes), including duplicate/unreachable incoming edges.
/// New scratch counts actual backing; inherited inventory/owner receipts retain
/// their documented logical accounting, not allocator/RSS bounds. Success
/// returns an unreserved witness receipt. Errors/panics drop local scratch
/// before same-ledger cleanup; accepted work/peak/first denial remain visible.
pub fn check_canonical_kir_loop_preheaders_v1<'a>(
    input: &'a Owner,
    output: &'a Owner,
    rows: &'a [Row],
    limits: Limits,
    budget: &mut Budget<'_>,
) -> Result<(CheckedCanonicalKirLoopPreheadersV1<'a>, Storage)> {
    scoped(budget, |meter| {
        check::check(input, output, rows, limits, meter)
    })
}
