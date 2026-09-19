//! Descriptive natural loops and exact SSA recurrences of an immutable owner.
//! No source join, no-wrap, termination, alias, motion, or rewrite authority.

use crate::CanonicalKirInventoryV1 as Inventory;
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirControlFlowScopeErrorV1, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirEdgeCoordinateV1 as Edge, CanonicalKirOperationCoordinateV1 as Operation,
    CheckedBinaryOperator, Constant, OperationKind, ScalarType, Terminator, Type,
    with_canonical_kir_control_flow_v1,
};
use std::{
    fmt,
    mem::size_of,
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirLoopLimitsV1 {
    pub functions: usize,
    pub blocks: usize,
    pub edges: usize,
    pub definitions: usize,
    pub operations: usize,
    pub loops: usize,
    /// Total retained member, edge and recurrence rows, not a B-by-B allocation.
    pub rows: usize,
}
impl Default for CanonicalKirLoopLimitsV1 {
    fn default() -> Self {
        Self {
            functions: 16_384,
            blocks: 65_536,
            edges: 262_144,
            definitions: 262_144,
            operations: 65_536,
            loops: 65_536,
            rows: 1_048_576,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalKirLoopErrorV1 {
    Resource(Resource),
    ControlFlow(CanonicalKirControlFlowScopeErrorV1),
    InputLimit {
        kind: &'static str,
        actual: usize,
        limit: usize,
    },
    InconsistentInventory,
    ForeignInventory,
    InvalidLoop(usize),
    ReplayMismatch,
    Panicked,
}
use CanonicalKirLoopErrorV1 as Error;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<crate::CanonicalKirInventoryErrorV1> for Error {
    fn from(error: crate::CanonicalKirInventoryErrorV1) -> Self {
        match error {
            crate::CanonicalKirInventoryErrorV1::Resource(error) => Self::Resource(error),
            crate::CanonicalKirInventoryErrorV1::InconsistentOwner => Self::InconsistentInventory,
        }
    }
}
impl From<CanonicalKirControlFlowScopeErrorV1> for Error {
    fn from(error: CanonicalKirControlFlowScopeErrorV1) -> Self {
        match error {
            CanonicalKirControlFlowScopeErrorV1::Resource(error) => Self::Resource(error),
            error => Self::ControlFlow(error),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canonical loop analysis: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Fixed-coordinate description. `step_bits` uses the exact scalar width; signed
/// values retain their two's-complement bits, not an inferred positive stride.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirRecurrenceV1 {
    parameter: Definition,
    initial_edge: Edge,
    initial: Definition,
    backedge: Edge,
    update: Definition,
    step: Definition,
    scalar: ScalarType,
    step_bits: u64,
    parameter_operand: u8,
    overflow: Option<Definition>,
}
impl CanonicalKirRecurrenceV1 {
    pub fn parameter(self) -> Definition {
        self.parameter
    }
    pub fn initial_edge(self) -> Edge {
        self.initial_edge
    }
    pub fn initial(self) -> Definition {
        self.initial
    }
    pub fn backedge(self) -> Edge {
        self.backedge
    }
    pub fn update(self) -> Definition {
        self.update
    }
    pub fn step(self) -> Definition {
        self.step
    }
    pub fn scalar(self) -> ScalarType {
        self.scalar
    }
    pub fn step_bits(self) -> u64 {
        self.step_bits
    }
    pub fn parameter_operand(self) -> u8 {
        self.parameter_operand
    }
    /// Some means checked Add with this actual overflow result; no use is erased.
    pub fn overflow(self) -> Option<Definition> {
        self.overflow
    }
}
use CanonicalKirRecurrenceV1 as Recurrence;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalKirNaturalLoopV1 {
    header: Block,
    members: Range<usize>,
    latches: Range<usize>,
    external: Range<usize>,
    exits: Range<usize>,
    recurrences: Range<usize>,
    single_entry: bool,
    preheader: Option<Edge>,
    dedicated_exits: bool,
}
impl CanonicalKirNaturalLoopV1 {
    pub fn header(&self) -> Block {
        self.header
    }
    pub fn is_single_entry(&self) -> bool {
        self.single_entry
    }
    pub fn unconditional_preheader(&self) -> Option<Edge> {
        self.preheader
    }
    pub fn has_dedicated_exits(&self) -> bool {
        self.dedicated_exits
    }
}
use CanonicalKirNaturalLoopV1 as NaturalLoop;

/// Actual new-vector capacities plus this report header. Excludes borrowed graph,
/// inventory, allocator metadata, stack headers, and the CFG's separate inherited
/// logical-cell accounting. Not a process RSS limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirLoopStorageV1(usize);
impl CanonicalKirLoopStorageV1 {
    pub fn retained_storage(self) -> usize {
        self.0
    }
}

/// Exact inventory/owner bound; no constructor accepts claimed facts or hashes.
/// A fact records a typed backedge equation, not execution/progress/no-wrap proof.
/// Re-derive after mutation. Missing recurrences never authorize transformations.
/// An outer natural loop may contain an irreducible region: no body-reducibility,
/// latch-reachability or instruction-movement property follows from the equation.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::{CanonicalKirInventoryV1, CanonicalKirLoopsV1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn inventory_cannot_drop(inventory: CanonicalKirInventoryV1<'_>, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let (loops, _) = CanonicalKirLoopsV1::derive(&inventory, Default::default(), budget).unwrap();
///     drop(inventory);
///     let _ = loops.loop_count();
/// }
/// ```
#[derive(Debug)]
pub struct CanonicalKirLoopsV1<'i, 'g> {
    inventory: &'i Inventory<'g>,
    loops: Vec<NaturalLoop>,
    members: Vec<Block>,
    edges: Vec<Edge>,
    recurrences: Vec<Recurrence>,
    retained: usize,
}
impl<'i, 'g> CanonicalKirLoopsV1<'i, 'g> {
    /// Reuses one existing live-metered CFG per defined function. Beyond that
    /// bounded algorithm, discovery is O(F+(H+1)*(B+E)+D*log(D+1)+O+R), H <= B.
    /// Definition queries retain the existing inventory's metered search cost.
    /// Capacity growth
    /// pays old/new coexistence and actual excess before initialization.
    /// Returns an UNRESERVED report receipt; caller reserves before further use.
    /// Every Result/panic exit restores the entry floor after owned values drop.
    pub fn derive(
        inventory: &'i Inventory<'g>,
        limits: CanonicalKirLoopLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKirLoopStorageV1)> {
        scoped(budget, |budget| {
            let report = Self::build(inventory, limits, budget)?;
            let receipt = CanonicalKirLoopStorageV1(report.retained);
            Ok((report, receipt))
        })
    }
    pub fn inventory(&self) -> &'i Inventory<'g> {
        self.inventory
    }
    pub fn belongs_to(&self, inventory: &Inventory<'_>) -> bool {
        std::ptr::eq(self.inventory, inventory)
    }
    pub fn loop_count(&self) -> usize {
        self.loops.len()
    }
    pub fn natural_loop(&self, index: usize, budget: &mut Budget<'_>) -> Result<&NaturalLoop> {
        budget.charge_work(2)?;
        self.loops.get(index).ok_or(Error::InvalidLoop(index))
    }
    pub fn members(&self, index: usize, budget: &mut Budget<'_>) -> Result<&[Block]> {
        let range = self.natural_loop(index, budget)?.members.clone();
        budget.charge_work(1)?;
        self.members.get(range).ok_or(Error::ReplayMismatch)
    }
    pub fn latch_edges(&self, index: usize, budget: &mut Budget<'_>) -> Result<&[Edge]> {
        let range = self.natural_loop(index, budget)?.latches.clone();
        budget.charge_work(1)?;
        self.edges.get(range).ok_or(Error::ReplayMismatch)
    }
    pub fn external_header_edges(&self, index: usize, budget: &mut Budget<'_>) -> Result<&[Edge]> {
        let range = self.natural_loop(index, budget)?.external.clone();
        budget.charge_work(1)?;
        self.edges.get(range).ok_or(Error::ReplayMismatch)
    }
    pub fn exit_edges(&self, index: usize, budget: &mut Budget<'_>) -> Result<&[Edge]> {
        let range = self.natural_loop(index, budget)?.exits.clone();
        budget.charge_work(1)?;
        self.edges.get(range).ok_or(Error::ReplayMismatch)
    }
    pub fn recurrences(&self, index: usize, budget: &mut Budget<'_>) -> Result<&[Recurrence]> {
        let range = self.natural_loop(index, budget)?.recurrences.clone();
        budget.charge_work(1)?;
        self.recurrences.get(range).ok_or(Error::ReplayMismatch)
    }
    /// Independent bounded replay reads actual edges/definitions and computes
    /// entry reachability with each possible header removed. It does NOT reuse
    /// producer dominator intervals, CFG flags or retained member claims.
    /// Worst-case O(F+(B+1)*(B+E)+R+D*log(D+1)+O); exhaustion remains an error.
    pub fn replay(
        &self,
        inventory: &Inventory<'_>,
        limits: CanonicalKirLoopLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        budget.charge_work(1)?;
        if !self.belongs_to(inventory) {
            return Err(Error::ForeignInventory);
        }
        scoped(budget, |budget| self.replay_inner(limits, budget))
    }
}

include!("canonical_kir_loops_resources_v1.rs");
include!("canonical_kir_loops_build_v1.rs");
include!("canonical_kir_loops_replay_v1.rs");

#[cfg(test)]
#[path = "canonical_kir_loops_v1_tests.rs"]
mod tests;
