//! Bounded, inert facts for an exact masked-count assertion-success pattern.
//!
//! This query borrows an admitted semantic document. It does not authenticate
//! that document's producer, establish initialization of arbitrary source
//! operands, authorize assertion elision, or prove any native lowering. Each
//! consumer must supply its retained source and preserve its SSA/use checks.
//!
//! The initial contract is deliberately positional: the assertion block ends
//! with mask, optional same-width signed-to-unsigned cast, and comparison; its
//! sole success predecessor leads to a shift as the first successor statement.
//! The complete function CFG must be acyclic. Explicit storage lifetimes for
//! participating locals must be active within the assertion block itself.
//! No general interval, symbolic algebra, name, or workload rule is used.

use std::{convert::Infallible, error::Error, fmt, mem::size_of};

use crate::semantic_mir_v1::*;

pub const MAX_SEMANTIC_MASKED_SHIFT_WORK_V1: usize = 4_000_000;
pub const MAX_SEMANTIC_MASKED_SHIFT_STORAGE_V1: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticMaskedShiftLimitsV1 {
    work_units: usize,
    storage_bytes: usize,
}

impl SemanticMaskedShiftLimitsV1 {
    pub const fn new(work_units: usize, storage_bytes: usize) -> Self {
        Self {
            work_units,
            storage_bytes,
        }
    }
}

impl Default for SemanticMaskedShiftLimitsV1 {
    fn default() -> Self {
        Self::new(
            MAX_SEMANTIC_MASKED_SHIFT_WORK_V1,
            MAX_SEMANTIC_MASKED_SHIFT_STORAGE_V1,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticMaskedShiftErrorV1 {
    InvalidLimits,
    InvalidModel(&'static str),
    WorkLimit { actual: usize, limit: usize },
    StorageLimit { actual: usize, limit: usize },
    Allocation,
}

impl fmt::Display for SemanticMaskedShiftErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "semantic masked-shift query: {self:?}")
    }
}

impl Error for SemanticMaskedShiftErrorV1 {}

type R<T> = Result<T, SemanticMaskedShiftErrorV1>;

/// Dependency-neutral live meter for the inert query's work and table capacities.
///
/// Calls precede the corresponding work or requested allocation. Capacity excess
/// is charged immediately after allocation, before initialization. This interface
/// neither owns reservations nor releases them: a scoped caller must retain them
/// until every associated index/partial table has been dropped, including errors
/// and unwinding. It does not authenticate source or confer admission authority.
pub trait SemanticMaskedShiftMeterV1 {
    /// The caller's unchanged resource failure type.
    type Error;

    /// Admits the next logical work units before they are performed.
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error>;

    /// Admits coexisting table-capacity bytes before initialization.
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Self::Error>;
}

/// A query-local failure or the exact external meter failure, never a missing fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticMaskedShiftMeteredErrorV1<E> {
    /// The existing bounded semantic query rejected its input or local limit.
    Analysis(SemanticMaskedShiftErrorV1),
    /// The live caller's resource meter rejected the next operation.
    Meter(E),
}

impl<E> From<SemanticMaskedShiftErrorV1> for SemanticMaskedShiftMeteredErrorV1<E> {
    fn from(error: SemanticMaskedShiftErrorV1) -> Self {
        Self::Analysis(error)
    }
}

impl<E: fmt::Display> fmt::Display for SemanticMaskedShiftMeteredErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analysis(error) => error.fmt(formatter),
            Self::Meter(error) => write!(formatter, "semantic masked-shift meter: {error}"),
        }
    }
}

impl<E: Error + 'static> Error for SemanticMaskedShiftMeteredErrorV1<E> {}

type MR<T, M> =
    Result<T, SemanticMaskedShiftMeteredErrorV1<<M as SemanticMaskedShiftMeterV1>::Error>>;

struct LocalMeter;

impl SemanticMaskedShiftMeterV1 for LocalMeter {
    type Error = Infallible;
    fn charge_work(&mut self, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
    fn reserve_storage(&mut self, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
}

fn local_result<T>(result: MR<T, LocalMeter>) -> R<T> {
    result.map_err(|error| match error {
        SemanticMaskedShiftMeteredErrorV1::Analysis(error) => error,
        SemanticMaskedShiftMeteredErrorV1::Meter(error) => match error {},
    })
}

struct Budget {
    limits: SemanticMaskedShiftLimitsV1,
    work: usize,
    storage: usize,
}

impl Budget {
    fn metered<'budget, 'meter, M: SemanticMaskedShiftMeterV1>(
        &'budget mut self,
        meter: &'meter mut M,
    ) -> MeteredBudget<'budget, 'meter, M> {
        MeteredBudget { local: self, meter }
    }

    #[cfg(test)]
    fn account_capacity<T>(&mut self, requested: usize, actual: usize) -> R<()> {
        local_result(
            self.metered(&mut LocalMeter)
                .account_capacity::<T>(requested, actual),
        )
    }
}

struct MeteredBudget<'budget, 'meter, M> {
    local: &'budget mut Budget,
    meter: &'meter mut M,
}

impl<M: SemanticMaskedShiftMeterV1> MeteredBudget<'_, '_, M> {
    fn charge(&mut self, amount: usize) -> MR<(), M> {
        let actual = self.local.work.saturating_add(amount);
        if actual > self.local.limits.work_units {
            return Err(SemanticMaskedShiftErrorV1::WorkLimit {
                actual,
                limit: self.local.limits.work_units,
            }
            .into());
        }
        self.meter
            .charge_work(amount)
            .map_err(SemanticMaskedShiftMeteredErrorV1::Meter)?;
        self.local.work = actual;
        Ok(())
    }

    fn reserve_bytes(&mut self, bytes: usize) -> MR<(), M> {
        let actual = self.local.storage.saturating_add(bytes);
        if actual > self.local.limits.storage_bytes {
            return Err(SemanticMaskedShiftErrorV1::StorageLimit {
                actual,
                limit: self.local.limits.storage_bytes,
            }
            .into());
        }
        self.meter
            .reserve_storage(bytes)
            .map_err(SemanticMaskedShiftMeteredErrorV1::Meter)?;
        self.local.storage = actual;
        Ok(())
    }

    fn account_capacity<T>(&mut self, requested: usize, actual: usize) -> MR<(), M> {
        let excess = actual
            .checked_sub(requested)
            .and_then(|count| count.checked_mul(size_of::<T>()))
            .ok_or(SemanticMaskedShiftErrorV1::Allocation)?;
        self.reserve_bytes(excess)
    }

    fn table<T: Clone>(&mut self, count: usize, value: T) -> MR<Vec<T>, M> {
        self.charge(count)?;
        let bytes = count
            .checked_mul(size_of::<T>())
            .ok_or(SemanticMaskedShiftErrorV1::Allocation)?;
        self.reserve_bytes(bytes)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| SemanticMaskedShiftErrorV1::Allocation)?;
        self.account_capacity::<T>(count, result.capacity())?;
        result.resize(count, value);
        Ok(result)
    }
}

#[derive(Clone, Copy)]
struct Occurrence {
    mask_statement: u32,
    cast_statement: Option<u32>,
    comparison_statement: u32,
    successor: SemanticBlockIdV1,
}

/// An indexed query over one exact borrowed owner and function.
///
/// Work for construction and all later occurrence lookups shares one ledger.
/// Storage counts table element capacities, not allocator metadata or RSS.
/// Requested bytes are prepaid; any allocator-provided capacity excess is
/// accounted immediately after reservation, before element initialization.
/// Excess beyond the cap is rejected and every partial table is dropped. These local
/// limits are not a receipt transferring a production caller's resource budget.
pub struct SemanticMaskedShiftIndexV1<'a> {
    owner: &'a AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
    assertions: Vec<Option<Occurrence>>,
    consumers: Vec<Option<SemanticBlockIdV1>>,
    budget: Budget,
}

/// A private-construction view of an actual occurrence, not a proof receipt.
pub struct SemanticMaskedShiftFactV1<'a> {
    owner: &'a AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
    assertion: SemanticBlockIdV1,
    occurrence: Occurrence,
}

impl<'a> SemanticMaskedShiftFactV1<'a> {
    pub const fn owner(&self) -> &'a AdmittedInertSemanticMirV1 {
        self.owner
    }
    pub const fn function(&self) -> SemanticFunctionIdV1 {
        self.function
    }
    pub const fn assertion_block(&self) -> SemanticBlockIdV1 {
        self.assertion
    }
    pub const fn mask_statement(&self) -> u32 {
        self.occurrence.mask_statement
    }
    pub const fn cast_statement(&self) -> Option<u32> {
        self.occurrence.cast_statement
    }
    pub const fn comparison_statement(&self) -> u32 {
        self.occurrence.comparison_statement
    }
    pub const fn successor_block(&self) -> SemanticBlockIdV1 {
        self.occurrence.successor
    }
    pub const fn shift_statement(&self) -> u32 {
        0
    }
}

impl<'a> SemanticMaskedShiftIndexV1<'a> {
    pub fn analyze(
        owner: &'a AdmittedInertSemanticMirV1,
        function: SemanticFunctionIdV1,
        limits: SemanticMaskedShiftLimitsV1,
    ) -> R<Self> {
        local_result(Self::analyze_metered(
            owner,
            function,
            limits,
            &mut LocalMeter,
        ))
    }

    /// Builds the index while debiting the supplied live meter at each operation.
    ///
    /// The meter's accepted reservations include construction scratch and remain
    /// the caller's responsibility on every exit. Do not release them while the
    /// index remains live. This method does not bind future lookups to this meter;
    /// production users must wrap the index in a scope enforcing that ownership.
    pub fn analyze_metered<M: SemanticMaskedShiftMeterV1>(
        owner: &'a AdmittedInertSemanticMirV1,
        function: SemanticFunctionIdV1,
        limits: SemanticMaskedShiftLimitsV1,
        meter: &mut M,
    ) -> MR<Self, M> {
        if limits.work_units > MAX_SEMANTIC_MASKED_SHIFT_WORK_V1
            || limits.storage_bytes > MAX_SEMANTIC_MASKED_SHIFT_STORAGE_V1
        {
            return Err(SemanticMaskedShiftErrorV1::InvalidLimits.into());
        }
        let source = owner.functions().get(function.index() as usize).ok_or(
            SemanticMaskedShiftErrorV1::InvalidModel("function is outside the admitted owner"),
        )?;
        let mut budget = Budget {
            limits,
            work: 0,
            storage: 0,
        };
        let mut active = budget.metered(meter);
        let blocks = source.blocks().len();
        let locals = source.locals().len();
        let mut assertions = active.table(blocks, None)?;
        let mut consumers = active.table(blocks, None)?;
        let mut incoming = active.table(blocks, 0_usize)?;
        let mut incoming_owner = active.table(blocks, None)?;
        let mut reachable = active.table(blocks, false)?;
        let mut escaped = active.table(locals, false)?;
        let mut explicit_storage = active.table(locals, false)?;
        let mut frame_block = active.table(locals, usize::MAX)?;
        let mut frame_live = active.table(locals, false)?;
        for (block_index, block) in source.blocks().iter().enumerate() {
            active.charge(1)?;
            block
                .terminator()
                .kind()
                .try_for_each_edge::<SemanticMaskedShiftMeteredErrorV1<M::Error>>(|edge| {
                    active.charge(1)?;
                    let target = edge.target().index() as usize;
                    let count = incoming.get_mut(target).ok_or(
                        SemanticMaskedShiftErrorV1::InvalidModel("missing edge target"),
                    )?;
                    *count = count
                        .checked_add(1)
                        .ok_or(SemanticMaskedShiftErrorV1::Allocation)?;
                    incoming_owner[target] = (*count == 1).then_some(block_index);
                    Ok(())
                })?;
            for statement in block.statements() {
                active.charge(1)?;
                match statement.kind() {
                    SemanticStatementKindV1::StorageLive(local)
                    | SemanticStatementKindV1::StorageDead(local) => {
                        *explicit_storage.get_mut(local.index() as usize).ok_or(
                            SemanticMaskedShiftErrorV1::InvalidModel("missing storage local"),
                        )? = true;
                    }
                    SemanticStatementKindV1::Assign(assignment) => {
                        if let SemanticRvalueKindV1::Borrow { place, .. }
                        | SemanticRvalueKindV1::AddressOf { place, .. } =
                            assignment.value().kind()
                        {
                            *escaped.get_mut(place.local().index() as usize).ok_or(
                                SemanticMaskedShiftErrorV1::InvalidModel("missing borrowed local"),
                            )? = true;
                        }
                    }
                    _ => {}
                }
            }
        }

        // A linear Kahn traversal rejects cycles for this first, closed contract.
        // Every actual edge participates, including duplicate/cleanup edges.
        let mut remaining = active.table(blocks, 0_usize)?;
        remaining.copy_from_slice(&incoming);
        let mut queue = active.table(blocks, 0_usize)?;
        let (mut head, mut tail) = (0, 0);
        for (block, count) in incoming.iter().enumerate() {
            active.charge(1)?;
            if *count == 0 {
                queue[tail] = block;
                tail += 1;
            }
        }
        let entry = source.entry().index() as usize;
        *reachable
            .get_mut(entry)
            .ok_or(SemanticMaskedShiftErrorV1::InvalidModel(
                "missing entry block",
            ))? = true;
        while head < tail {
            active.charge(1)?;
            let block = queue[head];
            head += 1;
            source.blocks()[block]
                .terminator()
                .kind()
                .try_for_each_edge::<SemanticMaskedShiftMeteredErrorV1<M::Error>>(|edge| {
                    active.charge(1)?;
                    let target = edge.target().index() as usize;
                    let from_reachable = reachable[block];
                    reachable[target] |= from_reachable;
                    remaining[target] -= 1;
                    if remaining[target] == 0 {
                        queue[tail] = target;
                        tail += 1;
                    }
                    Ok(())
                })?;
        }
        if tail == blocks {
            for (block_index, block) in source.blocks().iter().enumerate() {
                active.charge(1)?;
                if !reachable[block_index]
                    || !matches!(
                        block.terminator().kind(),
                        SemanticTerminatorKindV1::Assert { .. }
                    )
                {
                    continue;
                }
                for statement in block.statements() {
                    active.charge(1)?;
                    let (local, live) = match statement.kind() {
                        SemanticStatementKindV1::StorageLive(local) => (*local, true),
                        SemanticStatementKindV1::StorageDead(local) => (*local, false),
                        _ => continue,
                    };
                    frame_block[local.index() as usize] = block_index;
                    frame_live[local.index() as usize] = live;
                }
                // Candidate checking has fixed positional reads, no recursion,
                // operand-tree search or per-candidate CFG/definition traversal.
                active.charge(96)?;
                let live = |local: SemanticLocalIdV1| {
                    let local = local.index() as usize;
                    !explicit_storage[local]
                        || (frame_block[local] == block_index && frame_live[local])
                };
                if let Some(occurrence) = candidate(
                    owner.types(),
                    source,
                    block_index,
                    &incoming,
                    &incoming_owner,
                    &escaped,
                    live,
                ) {
                    let successor = occurrence.successor.index() as usize;
                    assertions[block_index] = Some(occurrence);
                    consumers[successor] = Some(SemanticBlockIdV1::from_index(block_index as u32));
                }
            }
        }
        Ok(Self {
            owner,
            function,
            assertions,
            consumers,
            budget,
        })
    }

    pub const fn work_units(&self) -> usize {
        self.budget.work
    }
    pub const fn storage_bytes(&self) -> usize {
        self.budget.storage
    }
    pub const fn owner(&self) -> &'a AdmittedInertSemanticMirV1 {
        self.owner
    }
    pub const fn function(&self) -> SemanticFunctionIdV1 {
        self.function
    }

    pub fn assertion(
        &mut self,
        block: SemanticBlockIdV1,
    ) -> R<Option<SemanticMaskedShiftFactV1<'a>>> {
        local_result(self.assertion_metered(block, &mut LocalMeter))
    }

    /// Looks up an assertion using the index's local limit and supplied live meter.
    pub fn assertion_metered<M: SemanticMaskedShiftMeterV1>(
        &mut self,
        block: SemanticBlockIdV1,
        meter: &mut M,
    ) -> MR<Option<SemanticMaskedShiftFactV1<'a>>, M> {
        self.budget.metered(meter).charge(1)?;
        Ok(self.fact(block))
    }

    pub fn shift(
        &mut self,
        block: SemanticBlockIdV1,
        statement: u32,
    ) -> R<Option<SemanticMaskedShiftFactV1<'a>>> {
        local_result(self.shift_metered(block, statement, &mut LocalMeter))
    }

    /// Looks up a shift using the index's local limit and supplied live meter.
    pub fn shift_metered<M: SemanticMaskedShiftMeterV1>(
        &mut self,
        block: SemanticBlockIdV1,
        statement: u32,
        meter: &mut M,
    ) -> MR<Option<SemanticMaskedShiftFactV1<'a>>, M> {
        self.budget.metered(meter).charge(2)?;
        if statement != 0 {
            return Ok(None);
        }
        Ok(self
            .consumers
            .get(block.index() as usize)
            .copied()
            .flatten()
            .and_then(|block| self.fact(block)))
    }

    fn fact(&self, assertion: SemanticBlockIdV1) -> Option<SemanticMaskedShiftFactV1<'a>> {
        Some(SemanticMaskedShiftFactV1 {
            owner: self.owner,
            function: self.function,
            assertion,
            occurrence: self
                .assertions
                .get(assertion.index() as usize)
                .copied()
                .flatten()?,
        })
    }
}

fn integer(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> Option<(bool, u16)> {
    match types.get(ty.index() as usize)?.shape() {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed,
            bits: bits @ (8 | 16 | 32 | 64),
        }) => Some((*signed, *bits)),
        _ => None,
    }
}

fn plain(function: &SemanticFunctionDeclV1, place: &SemanticPlaceV1) -> Option<SemanticLocalIdV1> {
    (place.projections().is_empty()
        && function.locals().get(place.local().index() as usize)?.ty() == place.ty())
    .then_some(place.local())
}

fn copied(operand: &SemanticOperandV1) -> Option<&SemanticPlaceV1> {
    if let SemanticOperandV1::Copy(place) = operand {
        Some(place)
    } else {
        None
    }
}

fn consumed(operand: &SemanticOperandV1) -> Option<&SemanticPlaceV1> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => Some(place),
        _ => None,
    }
}

fn literal(
    types: &[SemanticTypeDeclV1],
    operand: &SemanticOperandV1,
    ty: SemanticTypeIdV1,
    bits: u128,
) -> bool {
    let Some((signed, width)) = integer(types, ty) else {
        return false;
    };
    let SemanticOperandV1::Constant(constant) = operand else {
        return false;
    };
    let SemanticConstantValueV1::Scalar(value) = constant.value() else {
        return false;
    };
    constant.ty() == ty
        && u16::from(value.size_bytes()) * 8 == width
        && value.bits() == bits
        && (!signed || bits & (1_u128 << (width - 1)) == 0)
}

fn assignment(block: &SemanticBasicBlockV1, statement: usize) -> Option<&SemanticAssignmentV1> {
    if let SemanticStatementKindV1::Assign(assignment) = block.statements().get(statement)?.kind() {
        Some(assignment)
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn candidate(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    incoming: &[usize],
    incoming_owner: &[Option<usize>],
    escaped: &[bool],
    live: impl Fn(SemanticLocalIdV1) -> bool,
) -> Option<Occurrence> {
    let block = function.blocks().get(block_index)?;
    let SemanticTerminatorKindV1::Assert {
        condition,
        expected: true,
        message,
        target,
        unwind: SemanticUnwindActionV1::Unreachable,
    } = block.terminator().kind()
    else {
        return None;
    };
    let SemanticAssertMessageV1::Overflow {
        operation,
        left: message_left,
        right: message_count,
    } = message
    else {
        return None;
    };
    if !matches!(
        operation,
        SemanticBinaryOpV1::ShiftLeft | SemanticBinaryOpV1::ShiftRight
    ) || target.role() != SemanticEdgeRoleV1::AssertSuccess
    {
        return None;
    }
    let successor = target.target().index() as usize;
    if successor == block_index
        || target.target() == function.entry()
        || incoming.get(successor) != Some(&1)
        || incoming_owner.get(successor) != Some(&Some(block_index))
    {
        return None;
    }
    let condition_place = consumed(condition)?;
    let condition_local = plain(function, condition_place)?;
    if !matches!(
        types.get(condition_place.ty().index() as usize)?.shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
    ) || escaped[condition_local.index() as usize]
        || !live(condition_local)
    {
        return None;
    }
    let count = copied(message_count)?;
    let count_local = plain(function, count)?;
    let count_type = integer(types, count.ty())?;
    let left = copied(message_left)?;
    let left_local = plain(function, left)?;
    let (_, width) = integer(types, left.ty())?;
    if escaped[count_local.index() as usize]
        || escaped[left_local.index() as usize]
        || !live(count_local)
        || !live(left_local)
    {
        return None;
    }
    let comparison_statement = block.statements().len().checked_sub(1)?;
    let comparison = assignment(block, comparison_statement)?;
    if comparison.destination() != condition_place
        || comparison.value().result_type() != condition_place.ty()
    {
        return None;
    }
    let SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::LessThan,
        left: compared_count,
        right: compared_width,
    } = comparison.value().kind()
    else {
        return None;
    };
    let compared = copied(compared_count)?;
    let compared_type = integer(types, compared.ty())?;
    if compared_type.0 || !literal(types, compared_width, compared.ty(), u128::from(width)) {
        return None;
    }
    let mut mask_statement = comparison_statement.checked_sub(1)?;
    let cast_statement = if compared != count {
        let local = plain(function, compared)?;
        if !count_type.0
            || compared_type != (false, count_type.1)
            || escaped[local.index() as usize]
            || !live(local)
        {
            return None;
        }
        let cast = assignment(block, mask_statement)?;
        if cast.destination() != compared || cast.value().result_type() != compared.ty() {
            return None;
        }
        let SemanticRvalueKindV1::Cast {
            kind: SemanticCastKindV1::Integer,
            operand,
        } = cast.value().kind()
        else {
            return None;
        };
        if copied(operand) != Some(count) {
            return None;
        }
        let result = Some(mask_statement as u32);
        mask_statement = mask_statement.checked_sub(1)?;
        result
    } else {
        if count_type.0 {
            return None;
        }
        None
    };
    let mask = assignment(block, mask_statement)?;
    if mask.destination() != count || mask.value().result_type() != count.ty() {
        return None;
    }
    let SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::BitAnd,
        left: input,
        right: limit,
    } = mask.value().kind()
    else {
        return None;
    };
    if input.ty() != count.ty() || !literal(types, limit, count.ty(), u128::from(width - 1)) {
        return None;
    }
    let shift = assignment(function.blocks().get(successor)?, 0)?;
    let SemanticRvalueKindV1::Binary {
        operation: actual_operation,
        left: actual_left,
        right: actual_count,
    } = shift.value().kind()
    else {
        return None;
    };
    if actual_operation != operation
        || consumed(actual_left) != Some(left)
        || consumed(actual_count) != Some(count)
        || shift.value().result_type() != left.ty()
        || shift.destination().ty() != left.ty()
    {
        return None;
    }
    plain(function, shift.destination())?;
    Some(Occurrence {
        mask_statement: mask_statement as u32,
        cast_statement,
        comparison_statement: comparison_statement as u32,
        successor: target.target(),
    })
}

#[cfg(test)]
#[path = "semantic_masked_shift_v1_tests.rs"]
mod tests;
