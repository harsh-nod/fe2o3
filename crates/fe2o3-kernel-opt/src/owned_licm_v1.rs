//! Bounded owning total-integer LICM component, without a numbered policy.
use crate::private_cell_promotion_resources_v1 as resources;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1 as InventoryError, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirLicmErrorV1 as PairError, CanonicalKirLicmHoistV1 as Hoist,
    CanonicalKirLicmOriginV1 as Row, CanonicalKirLicmStorageV1 as PairStorage,
    CanonicalKirLoopErrorV1 as LoopError, CanonicalKirLoopLimitsV1 as Limits,
    CanonicalKirLoopsV1 as Loops, CheckedCanonicalKirLicmV1 as Pair, check_canonical_kir_licm_v1,
};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKernelIrReplayAdmissionErrorV12 as AdmissionError,
    CanonicalKernelIrReplayStorageV12 as OutputStorage,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirControlFlowScopeErrorV1 as FlowError,
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirOperationCoordinateV1 as Site,
    CanonicalKirUseCoordinateV1 as Use, Module, Operation, OperationKind, ScalarType, Type,
    UnaryOp, VerifiedCanonicalKernelIrIdentityV12 as Identity,
    VerifiedCanonicalKernelIrModuleV12 as Owner, with_canonical_kir_control_flow_v1,
};
use std::{fmt, mem::size_of};

/// Typed refusal from selection, bounded ownership, admission or pair replay.
#[derive(Debug)]
pub enum OwnedLicmErrorV1 {
    /// Cumulative work/storage, arithmetic or allocation refusal.
    Resource(Resource),
    /// Exact input inventory failure.
    Inventory(InventoryError),
    /// Natural-loop derivation or independent replay failure.
    Loops(LoopError),
    /// Owner-bound dominance failure.
    ControlFlow(FlowError),
    /// Fresh actual candidate admission failed.
    Admission(AdmissionError),
    /// Independent actual-pair checker refused the output.
    Pair(PairError),
    /// Internal coordinate/worklist invariant failed closed.
    Recipe(&'static str),
    /// Replay was requested against another complete canonical subject.
    ForeignInput,
    /// A scoped partial candidate unwound.
    Panicked,
}
type Error = OwnedLicmErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl From<Resource> for Error {
    fn from(v: Resource) -> Self {
        Self::Resource(v)
    }
}
impl From<InventoryError> for Error {
    fn from(v: InventoryError) -> Self {
        Self::Inventory(v)
    }
}
impl From<LoopError> for Error {
    fn from(v: LoopError) -> Self {
        Self::Loops(v)
    }
}
impl From<FlowError> for Error {
    fn from(v: FlowError) -> Self {
        Self::ControlFlow(v)
    }
}
impl From<AdmissionError> for Error {
    fn from(v: AdmissionError) -> Self {
        Self::Admission(v)
    }
}
impl From<PairError> for Error {
    fn from(v: PairError) -> Self {
        Self::Pair(v)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "owning total-integer LICM: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Closed actual output and complete input-to-output operation lineage.
/// The input identity is not source custody; a source pipeline must retain its
/// actual source owner separately and compose the relation before activation.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::OwnedLicmContinuationV1;
/// fn duplicate(v: &OwnedLicmContinuationV1) -> OwnedLicmContinuationV1 { v.clone() }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_kernel_opt::OwnedLicmContinuationV1;
/// fn detach(v: OwnedLicmContinuationV1) { let _ = v.output; }
/// ```
pub struct OwnedLicmContinuationV1 {
    output: Owner,
    output_storage: OutputStorage,
    input_identity: Identity,
    origins: Vec<Row>,
    retained: usize,
}
impl OwnedLicmContinuationV1 {
    /// Actual freshly admitted output, without mutable access.
    pub const fn output(&self) -> &Owner {
        &self.output
    }
    /// Complete canonical input identity, not a substitute for a replay owner.
    pub const fn input_identity(&self) -> &Identity {
        &self.input_identity
    }
    /// Every input operation, including retained operations, in original order.
    pub fn origins(&self) -> &[Row] {
        &self.origins
    }
    /// Unreserved output/header/actual lineage-capacity receipt; reserve before use.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// This component grants no artifact, source, native or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Requires the owning receipt live and independently checks actual endpoints.
    /// Returns an unreserved borrowed-witness receipt.
    pub fn replay_against<'a>(
        &'a self,
        input: &'a Owner,
        budget: &mut Budget<'_>,
    ) -> Result<(Pair<'a>, PairStorage)> {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        resources::scoped(budget, |meter| {
            meter.work(
                size_of::<Identity>()
                    .checked_add(3)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if self.retained != retained(self.output_storage, &self.origins)? {
                return Err(Resource::Accounting.into());
            }
            if input.canonical().identity() != &self.input_identity {
                return Err(Error::ForeignInput);
            }
            meter.derive(|b| {
                Ok(check_canonical_kir_licm_v1(
                    input,
                    &self.output,
                    &self.origins,
                    Limits::default(),
                    b,
                )?)
            })
        })
    }
}

/// Hoists only total one-result Bool/fixed-width-integer constants, Not,
/// BitAnd/BitOr/BitXor, comparisons and Select. Index, all arithmetic, shifts,
/// casts, floating point, memory, calls, intrinsics, atomics, barriers and
/// convergent/unknown operations are not selected. Preserves ValueIds and CFG.
/// Natural loops use original header order and actual dedicated unconditional
/// preheaders. FIFO def-use worklists select each operation at most once per
/// loop, and move it at most once globally. Final placement is checked after
/// all selections, then the actual output is admitted and independently replayed.
/// No source-name dispatch, source-bound owner or numbered/default policy.
///
/// Beyond inherited inventory/loop/CFG/admission costs, selection uses O(B+D+O+U)
/// scratch and O(F*L+H*(B+O+U)+D+U+O+B) work, L natural and H eligible loops.
/// All vectors pay actual capacity before use;
/// moving owned operations avoids new nested Type clones. The original candidate
/// reservation remains conservatively live while new operation vectors coexist.
/// Success restores the incoming floor and transfers one unreserved receipt;
/// failure/unwind drops partial objects before valid-ledger cleanup. Work, peak
/// and first-denial history are not refunded; caller sibling reservations remain.
pub fn prepare_owned_licm_v1(
    input: &Owner,
    budget: &mut Budget<'_>,
) -> Result<OwnedLicmContinuationV1> {
    resources::scoped(budget, |meter| {
        meter.reserve(header()?)?;
        let (inventory, is) = meter.derive(|b| Ok(Inventory::derive(input, b)?))?;
        meter.reserve(is.retained_storage())?;
        let (loops, ls) = meter.derive(|b| Ok(Loops::derive(&inventory, Limits::default(), b)?))?;
        meter.reserve(ls.retained_storage())?;
        meter.derive(|b| Ok(loops.replay(&inventory, Limits::default(), b)?))?;
        let origins = plan(&inventory, &loops, meter)?;
        let (mut candidate, cs) =
            meter.derive(|b| Ok(input.copy_module_for_transformation_v12(b)?))?;
        meter.reserve(cs.retained_storage())?;
        let extra_candidate = materialize(&inventory, &origins, &mut candidate, meter)?;
        let (output, output_storage) = meter.derive(|b| {
            Ok(Owner::from_module_ref_with_verification_budget_v12(
                &candidate, b,
            )?)
        })?;
        meter.reserve(output_storage.retained_storage())?;
        let ps = {
            let (_pair, ps) = meter.derive(|b| {
                Ok(check_canonical_kir_licm_v1(
                    input,
                    &output,
                    &origins,
                    Limits::default(),
                    b,
                )?)
            })?;
            meter.reserve(ps.retained_storage())?;
            ps
        };
        meter.release(ps.retained_storage())?;
        let retained = retained(output_storage, &origins)?;
        drop(candidate);
        meter.release(
            cs.retained_storage()
                .checked_add(extra_candidate)
                .ok_or(Resource::Arithmetic)?,
        )?;
        drop(loops);
        meter.release(ls.retained_storage())?;
        drop(inventory);
        meter.release(is.retained_storage())?;
        meter.work(1)?;
        Ok(OwnedLicmContinuationV1 {
            output,
            output_storage,
            input_identity: *input.canonical().identity(),
            origins,
            retained,
        })
    })
}

#[derive(Clone, Copy)]
struct Selection {
    header: Block,
    destination: Block,
    sequence: u32,
}

fn plan(a: &Inventory<'_>, loops: &Loops<'_, '_>, meter: &mut Meter<'_, '_>) -> Result<Vec<Row>> {
    let count = a.operations().len();
    let (mut selected, selected_size) = meter.table::<Option<Selection>>(count)?;
    let (mut pending, pending_size) = meter.table(count)?;
    let (mut queue, queue_size) = meter.table::<usize>(count)?;
    let (mut order, order_size) = meter.table::<usize>(count)?;
    let (mut members, members_size) = meter.table(a.blocks().len())?;
    let (mut heads, heads_size) = meter.table::<Option<usize>>(a.definitions().len())?;
    let (mut next, next_size) = meter.table::<Option<usize>>(a.uses().len())?;
    for _ in a.operations() {
        meter.push(&mut selected, None)?;
        meter.push(&mut pending, usize::MAX)?;
    }
    for _ in a.blocks() {
        meter.push(&mut members, false)?;
    }
    for _ in a.definitions() {
        meter.push(&mut heads, None)?;
    }
    for _ in a.uses() {
        meter.push(&mut next, None)?;
    }
    // Reverse construction yields ascending original use order in each chain.
    for (at, usage) in a.uses().iter().enumerate().rev() {
        meter.work(4)?;
        if matches!(usage.coordinate, Use::OperationOperand { .. }) {
            next[at] = heads[usage.definition];
            heads[usage.definition] = Some(at);
        }
    }
    for function in a.functions() {
        meter.work(1)?;
        if function.function.body.is_none() {
            continue;
        }
        meter.derive(|budget| {
            with_canonical_kir_control_flow_v1(
                a.owner(),
                function.coordinate,
                Default::default(),
                budget,
                |flow, budget| {
                    for loop_index in 0..loops.loop_count() {
                        let fact = loops.natural_loop(loop_index, budget)?;
                        if fact.header().function != function.coordinate || !fact.is_single_entry()
                        {
                            continue;
                        }
                        let Some(edge) = fact.unconditional_preheader() else {
                            continue;
                        };
                        let destination = edge.source;
                        budget.charge_work(
                            a.blocks()
                                .len()
                                .checked_add(count)
                                .ok_or(Resource::Arithmetic)?,
                        )?;
                        members.fill(false);
                        pending.fill(usize::MAX);
                        queue.clear();
                        for member in loops.members(loop_index, budget)? {
                            budget.charge_work(3)?;
                            members[block_index(a, *member)?] = true;
                        }
                        if members[block_index(a, destination)?] {
                            return Err(Error::Recipe("preheader inside loop"));
                        }
                        for at in function.operations.clone() {
                            budget.charge_work(4)?;
                            if selected[at].is_some()
                                || !members[block_index(a, a.operations()[at].coordinate.block)?]
                                || !candidate(a, at, budget)?
                            {
                                continue;
                            }
                            let mut unresolved = 0usize;
                            let mut admissible = true;
                            for use_at in a.operations()[at].operands.clone() {
                                budget.charge_work(4)?;
                                let definition = &a.definitions()[a.uses()[use_at].definition];
                                let (block, op) = match definition.coordinate {
                                    Definition::FunctionArgument { function: f, .. }
                                        if f == function.coordinate =>
                                    {
                                        continue;
                                    }
                                    Definition::BlockArgument { block, .. } => (block, None),
                                    Definition::Result { operation, .. } => {
                                        (operation.block, Some(operation_index(a, operation)?))
                                    }
                                    _ => return Err(Error::Recipe("operand function")),
                                };
                                if let Some(previous) = op.and_then(|i| selected[i]) {
                                    if members[block_index(a, previous.destination)?]
                                        || !flow.dominates(
                                            previous.destination,
                                            destination,
                                            budget,
                                        )?
                                    {
                                        admissible = false;
                                        break;
                                    }
                                } else if members[block_index(a, block)?] {
                                    if op.is_none() {
                                        admissible = false;
                                        break;
                                    }
                                    unresolved =
                                        unresolved.checked_add(1).ok_or(Resource::Arithmetic)?;
                                } else if !flow.dominates(block, destination, budget)? {
                                    admissible = false;
                                    break;
                                }
                            }
                            if admissible {
                                pending[at] = unresolved;
                                if unresolved == 0 {
                                    push_prepaid(&mut queue, at, count, budget)?;
                                }
                            }
                        }
                        let mut cursor = 0usize;
                        while cursor < queue.len() {
                            budget.charge_work(6)?;
                            let at = queue[cursor];
                            cursor = cursor.checked_add(1).ok_or(Resource::Arithmetic)?;
                            if selected[at].is_some() || pending[at] != 0 {
                                return Err(Error::Recipe("worklist uniqueness"));
                            }
                            let sequence =
                                u32::try_from(order.len()).map_err(|_| Resource::Arithmetic)?;
                            selected[at] = Some(Selection {
                                header: fact.header(),
                                destination,
                                sequence,
                            });
                            push_prepaid(&mut order, at, count, budget)?;
                            for definition in a.operations()[at].results.clone() {
                                let mut use_at = heads[definition];
                                let mut visits = 0usize;
                                while let Some(use_index) = use_at {
                                    budget.charge_work(7)?;
                                    visits = visits.checked_add(1).ok_or(Resource::Arithmetic)?;
                                    if visits > a.uses().len() {
                                        return Err(Error::Recipe("bounded def-use chain"));
                                    }
                                    let Use::OperationOperand { operation, .. } =
                                        a.uses()[use_index].coordinate
                                    else {
                                        return Err(Error::Recipe("operation def-use chain"));
                                    };
                                    let consumer = operation_index(a, operation)?;
                                    if pending[consumer] != usize::MAX && pending[consumer] != 0 {
                                        pending[consumer] = pending[consumer]
                                            .checked_sub(1)
                                            .ok_or(Resource::Accounting)?;
                                        if pending[consumer] == 0 {
                                            push_prepaid(&mut queue, consumer, count, budget)?;
                                        }
                                    }
                                    use_at = next[use_index];
                                }
                            }
                        }
                    }
                    // Nested-loop choices are complete: now check actual final
                    // placement, including definitions moved by a later loop.
                    for at in function.operations.clone() {
                        budget.charge_work(2)?;
                        let Some(current) = selected[at] else {
                            continue;
                        };
                        for use_at in a.operations()[at].operands.clone() {
                            budget.charge_work(5)?;
                            let definition = &a.definitions()[a.uses()[use_at].definition];
                            let block = match definition.coordinate {
                                Definition::FunctionArgument { function: f, .. }
                                    if f == function.coordinate =>
                                {
                                    continue;
                                }
                                Definition::BlockArgument { block, .. } => block,
                                Definition::Result { operation, .. } => {
                                    match selected[operation_index(a, operation)?] {
                                        Some(previous) => {
                                            if previous.destination == current.destination
                                                && previous.sequence >= current.sequence
                                            {
                                                return Err(Error::Recipe(
                                                    "final append dependency order",
                                                ));
                                            }
                                            previous.destination
                                        }
                                        None => operation.block,
                                    }
                                }
                                _ => return Err(Error::Recipe("final operand function")),
                            };
                            if !flow.dominates(block, current.destination, budget)? {
                                return Err(Error::Recipe("final operand dominance"));
                            }
                        }
                    }
                    Ok(())
                },
            )
        })?;
    }
    let (mut rows, _) = meter.table::<Row>(count)?;
    let (mut positions, positions_size) = meter.table(a.blocks().len())?;
    for _ in a.blocks() {
        meter.push(&mut positions, 0usize)?;
    }
    for (at, operation) in a.operations().iter().enumerate() {
        meter.work(4)?;
        let input = operation.coordinate;
        let mut row = Row {
            input,
            output: input,
            hoist: None,
        };
        if selected[at].is_none() {
            let position = &mut positions[block_index(a, input.block)?];
            row.output.operation = u32::try_from(*position).map_err(|_| Resource::Arithmetic)?;
            *position = position.checked_add(1).ok_or(Resource::Arithmetic)?;
        }
        meter.push(&mut rows, row)?;
    }
    for at in order {
        meter.work(5)?;
        let choice = selected[at].ok_or(Error::Recipe("selected order"))?;
        let position = &mut positions[block_index(a, choice.destination)?];
        rows[at].output = Site {
            block: choice.destination,
            operation: u32::try_from(*position).map_err(|_| Resource::Arithmetic)?,
        };
        rows[at].hoist = Some(Hoist {
            header: choice.header,
            sequence: choice.sequence,
        });
        *position = position.checked_add(1).ok_or(Resource::Arithmetic)?;
    }
    meter.release(order_size)?;
    drop(positions);
    meter.release(positions_size)?;
    drop(next);
    meter.release(next_size)?;
    drop(heads);
    meter.release(heads_size)?;
    drop(members);
    meter.release(members_size)?;
    drop(queue);
    meter.release(queue_size)?;
    drop(pending);
    meter.release(pending_size)?;
    drop(selected);
    meter.release(selected_size)?;
    Ok(rows)
}

fn materialize(
    a: &Inventory<'_>,
    rows: &[Row],
    candidate: &mut Module,
    meter: &mut Meter<'_, '_>,
) -> Result<usize> {
    let (mut slots, slots_size) = meter.table::<Option<Operation>>(rows.len())?;
    for _ in rows {
        meter.push(&mut slots, None)?;
    }
    let (mut final_counts, counts_size) = meter.table(a.blocks().len())?;
    for _ in a.blocks() {
        meter.push(&mut final_counts, 0usize)?;
    }
    for row in rows {
        meter.work(3)?;
        let count = &mut final_counts[block_index(a, row.output.block)?];
        *count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
    }
    let mut extra = 0usize;
    for function in a.functions() {
        meter.work(1)?;
        let Some(body) = &mut candidate.functions[function.coordinate.0 as usize].body else {
            continue;
        };
        for (local, block) in body.blocks.iter_mut().enumerate() {
            meter.work(3)?;
            let global = function
                .blocks
                .start
                .checked_add(local)
                .ok_or(Resource::Arithmetic)?;
            let (replacement, bytes) = meter.table::<Operation>(final_counts[global])?;
            extra = extra.checked_add(bytes).ok_or(Resource::Arithmetic)?;
            let old = std::mem::replace(&mut block.operations, replacement);
            for (position, operation) in old.into_iter().enumerate() {
                meter.work(3)?;
                let index = a.blocks()[global]
                    .operations
                    .start
                    .checked_add(position)
                    .ok_or(Resource::Arithmetic)?;
                if slots[index].replace(operation).is_some() {
                    return Err(Error::Recipe("unique owned operation"));
                }
            }
        }
    }
    let (mut offsets, offsets_size) = meter.table(a.blocks().len())?;
    let mut total = 0usize;
    for count in &final_counts {
        meter.push(&mut offsets, total)?;
        total = total.checked_add(*count).ok_or(Resource::Arithmetic)?;
    }
    if total != rows.len() {
        return Err(Error::Recipe("exact output cardinality"));
    }
    let (mut inverse, inverse_size) = meter.table(rows.len())?;
    for _ in rows {
        meter.push(&mut inverse, usize::MAX)?;
    }
    for (index, row) in rows.iter().enumerate() {
        meter.work(4)?;
        let block = block_index(a, row.output.block)?;
        let position = row.output.operation as usize;
        if position >= final_counts[block] {
            return Err(Error::Recipe("bounded output operation"));
        }
        let at = offsets[block]
            .checked_add(position)
            .ok_or(Resource::Arithmetic)?;
        if inverse[at] != usize::MAX {
            return Err(Error::Recipe("unique output slot"));
        }
        inverse[at] = index;
    }
    for function in a.functions() {
        meter.work(1)?;
        let Some(body) = &mut candidate.functions[function.coordinate.0 as usize].body else {
            continue;
        };
        for (local, block) in body.blocks.iter_mut().enumerate() {
            let global = function
                .blocks
                .start
                .checked_add(local)
                .ok_or(Resource::Arithmetic)?;
            for position in 0..final_counts[global] {
                meter.work(4)?;
                let index = inverse[offsets[global]
                    .checked_add(position)
                    .ok_or(Resource::Arithmetic)?];
                let operation = slots
                    .get_mut(index)
                    .and_then(Option::take)
                    .ok_or(Error::Recipe("complete owned operation transfer"))?;
                meter.push(&mut block.operations, operation)?;
            }
        }
    }
    drop(inverse);
    meter.release(inverse_size)?;
    drop(offsets);
    meter.release(offsets_size)?;
    drop(final_counts);
    meter.release(counts_size)?;
    drop(slots);
    meter.release(slots_size)?;
    Ok(extra)
}
fn push_prepaid<T>(
    values: &mut Vec<T>,
    value: T,
    bound: usize,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(2)?;
    if values.len() >= bound || values.len() == values.capacity() {
        return Err(Resource::Accounting.into());
    }
    values.push(value);
    Ok(())
}
fn fixed_scalar(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Scalar(
            ScalarType::Bool
                | ScalarType::I8
                | ScalarType::I16
                | ScalarType::I32
                | ScalarType::I64
                | ScalarType::U8
                | ScalarType::U16
                | ScalarType::U32
                | ScalarType::U64
        )
    )
}
fn candidate(a: &Inventory<'_>, at: usize, budget: &mut Budget<'_>) -> Result<bool> {
    budget.charge_work(4)?;
    let row = &a.operations()[at];
    if row.operation.results.len() != 1 || !fixed_scalar(&row.operation.results[0].ty) {
        return Ok(false);
    }
    let selected = match &row.operation.kind {
        OperationKind::Constant(value) => fixed_scalar(&value.ty()),
        OperationKind::Unary {
            op: UnaryOp::Not, ..
        }
        | OperationKind::Binary {
            op: BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor,
            ..
        }
        | OperationKind::Compare { .. }
        | OperationKind::Select { .. } => true,
        // Excludes Index/arithmetic/shifts/casts/float/memory/calls/intrinsics,
        // atomics/barriers/execution/tile/wave/matrix and every unknown opcode.
        _ => false,
    };
    if !selected {
        return Ok(false);
    }
    for use_at in row.operands.clone() {
        budget.charge_work(2)?;
        if !fixed_scalar(a.definitions()[a.uses()[use_at].definition].ty) {
            return Ok(false);
        }
    }
    Ok(true)
}
fn block_index(a: &Inventory<'_>, block: Block) -> Result<usize> {
    let f = a
        .functions()
        .get(block.function.0 as usize)
        .ok_or(Error::Recipe("function coordinate"))?;
    f.blocks
        .start
        .checked_add(block.block as usize)
        .filter(|at| *at < f.blocks.end && a.blocks()[*at].coordinate == block)
        .ok_or(Error::Recipe("block coordinate"))
}
fn operation_index(a: &Inventory<'_>, site: Site) -> Result<usize> {
    let b = &a.blocks()[block_index(a, site.block)?];
    b.operations
        .start
        .checked_add(site.operation as usize)
        .filter(|at| *at < b.operations.end && a.operations()[*at].coordinate == site)
        .ok_or(Error::Recipe("operation coordinate"))
}
fn header() -> Result<usize> {
    size_of::<OwnedLicmContinuationV1>()
        .checked_sub(size_of::<Owner>())
        .ok_or_else(|| Resource::Arithmetic.into())
}
fn retained(output: OutputStorage, rows: &Vec<Row>) -> Result<usize> {
    header()?
        .checked_add(output.retained_storage())
        .and_then(|n| n.checked_add(rows.capacity().checked_mul(size_of::<Row>())?))
        .ok_or_else(|| Resource::Arithmetic.into())
}

#[cfg(test)]
#[path = "owned_licm_v1_tests.rs"]
mod tests;
