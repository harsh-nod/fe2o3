//! Read-only unsigned induction facts, never source or transformation authority.
use super::{
    Block, Budget, CanonicalKirLoopErrorV1 as LoopError, CanonicalKirLoopLimitsV1 as Limits,
    CanonicalKirLoopsV1 as Loops, CanonicalKirRecurrenceV1 as Recurrence, Definition, Edge,
    Inventory, Operation, OperationKind, Resource, ScalarType, Terminator, Type,
};
use crate::canonical_kir_private_cell_pair_resources_v1 as resources;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkLedgerIdentityV1, CanonicalKirControlFlowScopeErrorV1 as CfgError,
    CanonicalKirControlFlowViewV1 as Cfg, ComparePredicate, ValueId,
    with_canonical_kir_control_flow_v1,
};
use std::{fmt, mem::size_of};

#[path = "canonical_kir_loop_induction_build_v1.rs"]
mod build;
#[path = "canonical_kir_loop_induction_check_v1.rs"]
mod check;

/// Closed unsupported cases, not failed proofs silently promoted to facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirInductionUnavailableV1 {
    /// Not unsigned U8/U16/U32/U64 with a positive literal step.
    Scalar,
    /// No unique dedicated preheader/backedge or ordinary single-entry loop.
    Entries,
    /// The header does not branch on this parameter's direct unsigned `<`.
    Guard,
    /// Bound is not defined outside the loop and available at its preheader.
    Bound,
    /// The exact taken body edge does not dominate the ordered update/latch.
    Control,
    /// A larger stride lacks supported nonwrapping literal arithmetic.
    Arithmetic,
}
use CanonicalKirInductionUnavailableV1 as Unavailable;

/// Mathematical distance to the first false guard, not an executed trip count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirGuardDistanceV1 {
    /// `initial >= bound ? 0 : bound - initial`, over mathematical integers.
    UnitStride {
        initial: Definition,
        bound: Definition,
    },
    /// Exact ceiling division from literal definitions, possibly zero.
    Literal(u64),
}
use CanonicalKirGuardDistanceV1 as Distance;

/// Scope of the no-wrap statement; no operation or overflow result is erased.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirGuardedUpdateV1 {
    /// Every execution of the matched update after the taken guard fits its type.
    NonWrapping,
    /// Literal initial/bound prove zero guard-distance; the body is not entered.
    NoUpdate,
}
use CanonicalKirGuardedUpdateV1 as Update;

/// Conditional normal-completion count, independent from guard-distance alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirIterationScopeV1 {
    /// Body paths/cycles/calls/abnormal control prevent the stronger statement.
    Unavailable,
    /// On entry through the preheader and completion through the normal header
    /// exit, N body/latch updates and N+1 header evaluations occur, where N is
    /// `guard_distance`. This does not establish total termination or safety.
    NormalHeaderCompletion,
}
use CanonicalKirIterationScopeV1 as Iterations;

/// Inert locators and arithmetic statement about one actual SSA recurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirGuardedInductionV1 {
    guard: Operation,
    body: Edge,
    exit: Edge,
    bound: Definition,
    update: Update,
    distance: Distance,
    iterations: Iterations,
}
impl CanonicalKirGuardedInductionV1 {
    pub const fn guard(self) -> Operation {
        self.guard
    }
    pub const fn body_edge(self) -> Edge {
        self.body
    }
    pub const fn exit_edge(self) -> Edge {
        self.exit
    }
    pub const fn bound(self) -> Definition {
        self.bound
    }
    pub const fn guarded_update(self) -> Update {
        self.update
    }
    pub const fn guard_distance(self) -> Distance {
        self.distance
    }
    pub const fn iteration_scope(self) -> Iterations {
        self.iterations
    }
}
use CanonicalKirGuardedInductionV1 as Fact;

/// Complete per-recurrence outcome; absence never means zero iterations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirInductionOutcomeV1 {
    Unavailable(Unavailable),
    Guarded(Fact),
}
use CanonicalKirInductionOutcomeV1 as Outcome;

/// Ordered row bound to the borrowed report's actual loop/recurrence roster.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirInductionRowV1 {
    loop_ordinal: usize,
    recurrence: Recurrence,
    outcome: Outcome,
}
impl CanonicalKirInductionRowV1 {
    pub const fn loop_ordinal(self) -> usize {
        self.loop_ordinal
    }
    pub const fn recurrence(self) -> Recurrence {
        self.recurrence
    }
    pub const fn outcome(self) -> Outcome {
        self.outcome
    }
}
use CanonicalKirInductionRowV1 as Row;

/// Typed refusal; budget errors and unsupported (successful) outcomes differ.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalKirInductionErrorV1 {
    Resource(Resource),
    Loops(LoopError),
    ControlFlow(CfgError),
    ForeignLoops,
    /// Replay must use all seven exact limits captured by derivation.
    LimitsMismatch,
    ReplayMismatch,
    Panicked,
}
use CanonicalKirInductionErrorV1 as Error;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl From<Resource> for Error {
    fn from(v: Resource) -> Self {
        Self::Resource(v)
    }
}
impl From<LoopError> for Error {
    fn from(v: LoopError) -> Self {
        Self::Loops(v)
    }
}
impl From<CfgError> for Error {
    fn from(v: CfgError) -> Self {
        Self::ControlFlow(v)
    }
}
impl From<crate::CanonicalKirInventoryErrorV1> for Error {
    fn from(v: crate::CanonicalKirInventoryErrorV1) -> Self {
        Self::Loops(v.into())
    }
}
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canonical induction facts: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Actual new report header/capacity, excluding borrowed owners and allocator
/// overhead. This is an unreserved addition, not an RSS measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirInductionStorageV1(usize);
impl CanonicalKirInductionStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Borrowed exact-owner analysis; no raw graph/hash/source attachment exists.
/// The captured floor is the actual incoming ledger floor, not a caller-number
/// certificate that the borrowed owner and inherited analyses were prepaid.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::{CanonicalKirLoopsV1, CanonicalKirInductionFactsV1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn borrowed(loops: CanonicalKirLoopsV1<'_, '_>, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let (facts, _) = CanonicalKirInductionFactsV1::derive(&loops, Default::default(), budget).unwrap();
///     drop(loops);
///     let _ = facts.rows();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_analysis::CanonicalKirInductionFactsV1;
/// fn duplicate<'l, 'i, 'g>(facts: &CanonicalKirInductionFactsV1<'l, 'i, 'g>)
///     -> CanonicalKirInductionFactsV1<'l, 'i, 'g> { facts.clone() }
/// ```
pub struct CanonicalKirInductionFactsV1<'l, 'i, 'g> {
    loops: &'l Loops<'i, 'g>,
    limits: Limits,
    rows: Vec<Row>,
    retained: usize,
    floor: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
}
impl<'l, 'i, 'g> CanonicalKirInductionFactsV1<'l, 'i, 'g> {
    /// Replays the input loops, derives bounded facts, then independently checks
    /// every actual row before transfer. Beyond inherited CFG/loop analysis,
    /// work is O((R+H)*(B+E+O)+D queries), scratch O(B+E), retained rows O(R).
    /// All capacities coexist with live inputs, are paid before initialization,
    /// and are dropped before same-ledger cleanup on error or unwind.
    pub fn derive(
        loops: &'l Loops<'i, 'g>,
        limits: Limits,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKirInductionStorageV1)> {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        resources::scoped(budget, |meter| {
            meter.derive(|b| Ok(loops.replay(loops.inventory(), limits, b)?))?;
            meter.reserve(size_of::<Self>())?;
            let (mut rows, bytes) = meter.table::<Row>(loops.recurrences.len())?;
            let (mut scratch, scratch_bytes) = Scratch::new(loops.inventory(), meter)?;
            meter.derive(|b| build::derive(loops, &mut rows, &mut scratch, b))?;
            drop(scratch);
            meter.release(scratch_bytes)?;
            meter.work(1)?;
            let retained = size_of::<Self>()
                .checked_add(bytes)
                .ok_or(Resource::Arithmetic)?;
            let report = Self {
                loops,
                limits,
                rows,
                retained,
                floor,
                ledger,
            };
            check::replay(&report, limits, meter)?;
            Ok((report, CanonicalKirInductionStorageV1(retained)))
        })
    }
    pub const fn loops(&self) -> &'l Loops<'i, 'g> {
        self.loops
    }
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Requires the original ledger, captured input floor and this report's
    /// live receipt. The exact derivation limits must match before fresh loop
    /// replay and independent row checks run, even if other limits admit input.
    pub fn replay(
        &self,
        loops: &Loops<'_, '_>,
        limits: Limits,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        budget.charge_work(2)?;
        if !std::ptr::eq(self.loops, loops) {
            return Err(Error::ForeignLoops);
        }
        budget.charge_work(7)?;
        if self.limits != limits {
            return Err(Error::LimitsMismatch);
        }
        let required = self
            .floor
            .checked_add(self.retained)
            .ok_or(Resource::Arithmetic)?;
        if self.ledger != budget.work_ledger_identity_v1() || budget.storage() < required {
            return Err(Resource::Accounting.into());
        }
        resources::scoped(budget, |meter| check::replay(self, limits, meter))
    }
}

// Shared helpers only resolve typed coordinates, initialize bounded scratch, or
// decode exact literals. Producer/checker make their own admission decisions.
fn block_index(i: &Inventory<'_>, c: Block, b: &mut Budget<'_>) -> Result<usize> {
    Ok(super::block_index(i, c, b)?)
}
fn operation<'a, 'g>(
    i: &'a Inventory<'g>,
    c: Operation,
    b: &mut Budget<'_>,
) -> Result<&'a crate::CanonicalKirOperationRefV1<'g>> {
    Ok(super::operation(i, c, b)?)
}
fn definition<'a, 'g>(
    i: &'a Inventory<'g>,
    c: Definition,
    b: &mut Budget<'_>,
) -> Result<&'a crate::CanonicalKirDefinitionRefV1<'g>> {
    b.charge_work(4)?;
    let (range, ordinal) = match c {
        Definition::FunctionArgument { function, argument } => {
            let f = i
                .functions()
                .get(function.0 as usize)
                .ok_or(Error::ReplayMismatch)?;
            (f.definitions.clone(), argument as usize)
        }
        Definition::BlockArgument { block, argument } => (
            i.blocks()[block_index(i, block, b)?].parameters.clone(),
            argument as usize,
        ),
        Definition::Result {
            operation: op,
            result,
        } => (operation(i, op, b)?.results.clone(), result as usize),
    };
    let index = range
        .start
        .checked_add(ordinal)
        .ok_or(Resource::Arithmetic)?;
    i.definitions()
        .get(index)
        .filter(|r| index < range.end && r.coordinate == c)
        .ok_or(Error::ReplayMismatch)
}
fn resolve(i: &Inventory<'_>, block: Block, id: ValueId, b: &mut Budget<'_>) -> Result<Definition> {
    Ok(i.definition_for_value(block.function, id, b)?
        .ok_or(Error::ReplayMismatch)?
        .coordinate)
}
fn edge<'a, 'g>(
    i: &'a Inventory<'g>,
    c: Edge,
    b: &mut Budget<'_>,
) -> Result<&'a crate::CanonicalKirEdgeRefV1<'g>> {
    let block = &i.blocks()[block_index(i, c.source, b)?];
    b.charge_work(3)?;
    let index = block
        .edges
        .start
        .checked_add(c.successor as usize)
        .ok_or(Resource::Arithmetic)?;
    i.edges()
        .get(index)
        .filter(|r| index < block.edges.end && r.coordinate == c)
        .ok_or(Error::ReplayMismatch)
}
fn definition_block(c: Definition) -> Option<Block> {
    match c {
        Definition::FunctionArgument { .. } => None,
        Definition::BlockArgument { block, .. } => Some(block),
        Definition::Result { operation, .. } => Some(operation.block),
    }
}
fn maximum(scalar: ScalarType) -> Option<u64> {
    Some(match scalar {
        ScalarType::U8 => u8::MAX.into(),
        ScalarType::U16 => u16::MAX.into(),
        ScalarType::U32 => u32::MAX.into(),
        ScalarType::U64 => u64::MAX,
        _ => return None,
    })
}
fn literal(
    i: &Inventory<'_>,
    c: Definition,
    scalar: ScalarType,
    b: &mut Budget<'_>,
) -> Result<Option<u64>> {
    b.charge_work(2)?;
    let Definition::Result {
        operation: op,
        result: 0,
    } = c
    else {
        return Ok(None);
    };
    let OperationKind::Constant(value) = &operation(i, op, b)?.operation.kind else {
        return Ok(None);
    };
    Ok(super::fixed_integer_bits(value)
        .filter(|(s, _)| *s == scalar)
        .map(|(_, v)| v))
}

struct Scratch {
    members: Vec<u8>,
    marks: Vec<u8>,
    degrees: Vec<usize>,
    pending: Vec<(usize, usize)>,
}
impl Scratch {
    fn new(i: &Inventory<'_>, meter: &mut Meter<'_, '_>) -> Result<(Self, usize)> {
        let n = i.blocks().len();
        meter.reserve(size_of::<Self>())?;
        let (mut members, a) = meter.table(n)?;
        let (mut marks, b) = meter.table(n)?;
        let (mut degrees, c) = meter.table(n)?;
        let (pending, d) = meter.table(n)?;
        meter.work(n.checked_mul(3).ok_or(Resource::Arithmetic)?)?;
        members.resize(n, 0u8);
        marks.resize(n, 0u8);
        degrees.resize(n, 0usize);
        let bytes = [a, b, c, d]
            .into_iter()
            .try_fold(size_of::<Self>(), |sum, x| {
                sum.checked_add(x).ok_or(Resource::Arithmetic)
            })?;
        Ok((
            Self {
                members,
                marks,
                degrees,
                pending,
            },
            bytes,
        ))
    }
    fn members(&mut self, loops: &Loops<'_, '_>, index: usize, b: &mut Budget<'_>) -> Result<()> {
        b.charge_work(self.members.len())?;
        self.members.fill(0);
        for block in loops.members(index, b)? {
            b.charge_work(1)?;
            self.members[block_index(loops.inventory(), *block, b)?] = 1;
        }
        Ok(())
    }
    fn reset(&mut self, b: &mut Budget<'_>) -> Result<()> {
        b.charge_work(
            self.marks
                .len()
                .checked_add(self.pending.len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        self.marks.fill(0);
        self.pending.clear();
        Ok(())
    }
    fn enqueue(&mut self, block: usize, next: usize, b: &mut Budget<'_>) -> Result<()> {
        b.charge_work(2)?;
        if self.pending.len() == self.pending.capacity() {
            return Err(Resource::Accounting.into());
        }
        self.pending.push((block, next));
        Ok(())
    }
}

#[cfg(test)]
#[path = "canonical_kir_loop_induction_facts_v1_tests.rs"]
mod tests;
