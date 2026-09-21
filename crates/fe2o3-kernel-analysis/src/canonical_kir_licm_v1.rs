//! Independent actual-pair relation for total fixed-integer loop-invariant motion.
use crate::{
    CanonicalKirInventoryErrorV1 as InventoryError, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirLoopErrorV1 as LoopError, CanonicalKirLoopLimitsV1 as Limits,
    CanonicalKirLoopsV1 as Loops, canonical_kir_private_cell_pair_resources_v1 as resources,
};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirControlFlowScopeErrorV1 as FlowError,
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirOperationCoordinateV1 as Site,
    OperationKind, ScalarType, Type, UnaryOp, VerifiedCanonicalKernelIrModuleV12 as Owner,
    with_canonical_kir_control_flow_v1,
};
use std::{fmt, mem::size_of};

/// Original natural-loop header and position in the complete movement order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirLicmHoistV1 {
    /// Coordinate in the unchanged input CFG.
    pub header: Block,
    /// Contiguous, unique zero-based order; each original operation moves once.
    pub sequence: u32,
}
/// One exact original operation, including every operation that was retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirLicmOriginV1 {
    /// Original coordinate, in the complete input operation order.
    pub input: Site,
    /// Exact coordinate in the freshly admitted output.
    pub output: Site,
    /// None means retained in its original block and relative order.
    pub hoist: Option<CanonicalKirLicmHoistV1>,
}
type Row = CanonicalKirLicmOriginV1;

/// Refusal domains for the independent relation; no failed check is a no-op.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalKirLicmErrorV1 {
    /// Cumulative work/storage or arithmetic admission failed.
    Resource(Resource),
    /// An actual endpoint inventory could not be constructed.
    Inventory(InventoryError),
    /// Original/final natural-loop analysis or replay failed.
    Loops(LoopError),
    /// Actual owner-bound dominance could not be established.
    ControlFlow(FlowError),
    /// Complete payload, lineage, or movement conditions disagree.
    Mismatch(&'static str),
    /// Scoped scratch unwound; this grants no continuation.
    Panicked,
}
type Error = CanonicalKirLicmErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
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
impl From<FlowError> for Error {
    fn from(value: FlowError) -> Self {
        Self::ControlFlow(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canonical total-integer LICM pair: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Unreserved witness header only; endpoint and lineage backing stays borrowed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirLicmStorageV1(usize);
impl CanonicalKirLicmStorageV1 {
    /// Reserve while retaining the returned witness.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Move-only relation borrowing both actual owners and the complete lineage.
/// Establishes only this canonical motion relation, not source/native authority.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::CheckedCanonicalKirLicmV1;
/// fn clone_pair<'a>(p: &CheckedCanonicalKirLicmV1<'a>) -> CheckedCanonicalKirLicmV1<'a> { p.clone() }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_analysis::check_canonical_kir_licm_v1;
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn detach(a: Owner, b: Owner, budget: &mut Budget<'_>) {
///     let (pair, _) = check_canonical_kir_licm_v1(&a, &b, &[], Default::default(), budget).unwrap();
///     drop(a);
///     let _ = pair.input();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_analysis::{check_canonical_kir_licm_v1, CanonicalKirLicmOriginV1};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn detach_rows(a: &Owner, b: &Owner, budget: &mut Budget<'_>) {
///     let rows = Vec::<CanonicalKirLicmOriginV1>::new();
///     let (pair, _) = check_canonical_kir_licm_v1(a, b, &rows, Default::default(), budget).unwrap();
///     drop(rows);
///     let _ = pair.origins();
/// }
/// ```
pub struct CheckedCanonicalKirLicmV1<'a> {
    input: &'a Owner,
    output: &'a Owner,
    origins: &'a [Row],
}
impl<'a> CheckedCanonicalKirLicmV1<'a> {
    /// Exact connected input, not a digest reconstruction.
    pub const fn input(&self) -> &'a Owner {
        self.input
    }
    /// Exact freshly admitted output checked by the relation.
    pub const fn output(&self) -> &'a Owner {
        self.output
    }
    /// Complete original-order operation lineage.
    pub const fn origins(&self) -> &'a [Row] {
        self.origins
    }
    /// Canonical relation only, never artifact or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Checks actual graph payloads and independently establishes every moved
/// operation's total scalar contract, natural-loop membership and preheader,
/// original availability and final SSA placement. Never calls a producer.
/// Constants/Not/bitwise/comparison/Select over Bool and fixed integers only;
/// Index, arithmetic, shifts, casts, floating point, memory, calls, intrinsic,
/// convergent and all other operations are excluded. CFG and ValueIds cannot
/// change. Fresh endpoint admission is required by the owner argument types.
///
/// Beyond inherited inventory/loop/CFG services, scratch is O(B+O); checks are
/// O(F*L+wire+H*(B+O+U)), L natural loops, H eligible loops, U operand
/// occurrences, plus linear roster initialization. Actual table
/// capacity is charged. Success transfers an unreserved witness; errors/panics
/// drop scratch before same-ledger cleanup, preserving work and denial history.
pub fn check_canonical_kir_licm_v1<'a>(
    input: &'a Owner,
    output: &'a Owner,
    origins: &'a [Row],
    limits: Limits,
    budget: &mut Budget<'_>,
) -> Result<(CheckedCanonicalKirLicmV1<'a>, CanonicalKirLicmStorageV1)> {
    resources::scoped(budget, |meter| check(input, output, origins, limits, meter))
}

fn check<'a>(
    input: &'a Owner,
    output: &'a Owner,
    rows: &'a [Row],
    limits: Limits,
    meter: &mut Meter<'_, '_>,
) -> Result<(CheckedCanonicalKirLicmV1<'a>, CanonicalKirLicmStorageV1)> {
    let witness = size_of::<CheckedCanonicalKirLicmV1<'_>>();
    meter.reserve(witness)?;
    let (a, a_size) = meter.derive(|b| Ok(Inventory::derive(input, b)?))?;
    meter.reserve(a_size.retained_storage())?;
    let (b, b_size) = meter.derive(|b| Ok(Inventory::derive(output, b)?))?;
    meter.reserve(b_size.retained_storage())?;
    let (loops, loops_size) = meter.derive(|b| Ok(Loops::derive(&a, limits, b)?))?;
    meter.reserve(loops_size.retained_storage())?;
    meter.derive(|b| Ok(loops.replay(&a, limits, b)?))?;
    meter.work(
        input
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(output.canonical().canonical_bytes().len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    headers(input.module(), output.module())?;
    if rows.len() != a.operations().len() || rows.len() != b.operations().len() {
        return Err(Error::Mismatch("complete operation cardinality"));
    }
    let (mut inverse, inverse_size) = meter.table(rows.len())?;
    let (mut order, order_size) = meter.table(rows.len())?;
    for _ in rows {
        meter.push(&mut inverse, usize::MAX)?;
        meter.push(&mut order, usize::MAX)?;
    }
    let mut moved = 0usize;
    for (index, (row, original)) in rows.iter().zip(a.operations()).enumerate() {
        meter.work(8)?;
        if row.input != original.coordinate {
            return Err(Error::Mismatch("original row order"));
        }
        let target = operation_index(&b, row.output)?;
        if inverse[target] != usize::MAX {
            return Err(Error::Mismatch("duplicate output operation"));
        }
        inverse[target] = index;
        if original.operation != b.operations()[target].operation {
            return Err(Error::Mismatch("exact operation payload and ValueIds"));
        }
        match row.hoist {
            None if row.input.block != row.output.block => {
                return Err(Error::Mismatch("retained block"));
            }
            None => {}
            Some(hoist) => {
                let sequence = hoist.sequence as usize;
                if sequence >= rows.len()
                    || order[sequence] != usize::MAX
                    || row.input.block == row.output.block
                    || row.input.block.function != row.output.block.function
                {
                    return Err(Error::Mismatch("unique bounded movement sequence"));
                }
                order[sequence] = index;
                moved = moved.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
        }
    }
    for (position, index) in order.iter().enumerate() {
        meter.work(1)?;
        if (position < moved) != (*index != usize::MAX) {
            return Err(Error::Mismatch("contiguous movement sequence"));
        }
    }
    for block in b.blocks() {
        let mut last_retained = None;
        let mut last_moved = None;
        for at in block.operations.clone() {
            meter.work(5)?;
            let row = &rows[inverse[at]];
            match row.hoist {
                None => {
                    if last_moved.is_some()
                        || last_retained.is_some_and(|p| p >= row.input.operation)
                    {
                        return Err(Error::Mismatch(
                            "unchanged retained order before appended hoists",
                        ));
                    }
                    last_retained = Some(row.input.operation);
                }
                Some(h) => {
                    if last_moved.is_some_and(|p| p >= h.sequence) {
                        return Err(Error::Mismatch("preheader append order"));
                    }
                    last_moved = Some(h.sequence);
                }
            }
        }
    }
    let (mut members, members_size) = meter.table(a.blocks().len())?;
    let (mut checked, checked_size) = meter.table(rows.len())?;
    for _ in a.blocks() {
        meter.push(&mut members, false)?;
    }
    for _ in rows {
        meter.push(&mut checked, false)?;
    }
    for function in a.functions() {
        meter.work(1)?;
        if function.function.body.is_none() {
            continue;
        }
        meter.derive(|budget| {
            with_canonical_kir_control_flow_v1(
                input,
                function.coordinate,
                Default::default(),
                budget,
                |flow, budget| {
                    for loop_index in 0..loops.loop_count() {
                        let fact = loops.natural_loop(loop_index, budget)?;
                        if fact.header().function != function.coordinate {
                            continue;
                        }
                        let Some(edge) = fact.unconditional_preheader() else {
                            continue;
                        };
                        if !fact.is_single_entry() {
                            continue;
                        }
                        budget.charge_work(a.blocks().len())?;
                        members.fill(false);
                        for member in loops.members(loop_index, budget)? {
                            budget.charge_work(3)?;
                            members[block_index(&a, *member)?] = true;
                        }
                        let preheader = edge.source;
                        if members[block_index(&a, preheader)?] {
                            return Err(Error::Mismatch("preheader outside loop"));
                        }
                        for at in function.operations.clone() {
                            budget.charge_work(3)?;
                            let row = &rows[at];
                            let Some(h) = row.hoist.filter(|h| h.header == fact.header()) else {
                                continue;
                            };
                            if checked[at]
                                || row.output.block != preheader
                                || !members[block_index(&a, row.input.block)?]
                                || !total_scalar(&a, at, budget)?
                            {
                                return Err(Error::Mismatch(
                                    "eligible total operation and exact loop/preheader",
                                ));
                            }
                            for use_at in a.operations()[at].operands.clone() {
                                budget.charge_work(5)?;
                                let definition = &a.definitions()[a.uses()[use_at].definition];
                                let (original_block, definition_op) = match definition.coordinate {
                                    Definition::FunctionArgument { function: f, .. }
                                        if f == function.coordinate =>
                                    {
                                        continue;
                                    }
                                    Definition::BlockArgument { block, .. } => (block, None),
                                    Definition::Result { operation, .. } => {
                                        (operation.block, Some(operation_index(&a, operation)?))
                                    }
                                    _ => return Err(Error::Mismatch("operand function")),
                                };
                                if members[block_index(&a, original_block)?]
                                    || !flow.dominates(original_block, preheader, budget)?
                                {
                                    let Some(producer) = definition_op.map(|i| &rows[i]) else {
                                        return Err(Error::Mismatch("loop-carried operand"));
                                    };
                                    let Some(previous) = producer.hoist else {
                                        return Err(Error::Mismatch("unhoisted loop operand"));
                                    };
                                    if previous.sequence >= h.sequence
                                        || members[block_index(&a, producer.output.block)?]
                                        || !flow.dominates(
                                            producer.output.block,
                                            preheader,
                                            budget,
                                        )?
                                    {
                                        return Err(Error::Mismatch("earlier invariant hoist"));
                                    }
                                }
                            }
                            checked[at] = true;
                        }
                    }
                    Ok(())
                },
            )
        })?;
        // Query the actual final graph, not just an unchanged-CFG assertion.
        meter.derive(|budget| {
            with_canonical_kir_control_flow_v1(
                output,
                function.coordinate,
                Default::default(),
                budget,
                |flow, budget| {
                    for at in function.operations.clone() {
                        budget.charge_work(2)?;
                        let row = &rows[at];
                        if row.hoist.is_none() {
                            continue;
                        }
                        let final_at = operation_index(&b, row.output)?;
                        for use_at in b.operations()[final_at].operands.clone() {
                            budget.charge_work(4)?;
                            let definition = &b.definitions()[b.uses()[use_at].definition];
                            let block = match definition.coordinate {
                                Definition::FunctionArgument { function: f, .. }
                                    if f == function.coordinate =>
                                {
                                    continue;
                                }
                                Definition::BlockArgument { block, .. } => block,
                                Definition::Result { operation, .. } => {
                                    if operation.block == row.output.block
                                        && operation.operation >= row.output.operation
                                    {
                                        return Err(Error::Mismatch(
                                            "final same-block operand order",
                                        ));
                                    }
                                    operation.block
                                }
                                _ => return Err(Error::Mismatch("final operand function")),
                            };
                            if !flow.dominates(block, row.output.block, budget)? {
                                return Err(Error::Mismatch("final operand dominance"));
                            }
                        }
                    }
                    Ok(())
                },
            )
        })?;
    }
    for (row, checked) in rows.iter().zip(&checked) {
        meter.work(2)?;
        if row.hoist.is_some() != *checked {
            return Err(Error::Mismatch("complete checked movement roster"));
        }
    }
    let (final_loops, final_size) = meter.derive(|bgt| Ok(Loops::derive(&b, limits, bgt)?))?;
    meter.reserve(final_size.retained_storage())?;
    meter.derive(|bgt| Ok(final_loops.replay(&b, limits, bgt)?))?;
    drop(final_loops);
    meter.release(final_size.retained_storage())?;
    drop(checked);
    meter.release(checked_size)?;
    drop(members);
    meter.release(members_size)?;
    drop(order);
    meter.release(order_size)?;
    drop(inverse);
    meter.release(inverse_size)?;
    drop(loops);
    meter.release(loops_size.retained_storage())?;
    drop(b);
    meter.release(b_size.retained_storage())?;
    drop(a);
    meter.release(a_size.retained_storage())?;
    meter.work(1)?;
    Ok((
        CheckedCanonicalKirLicmV1 {
            input,
            output,
            origins: rows,
        },
        CanonicalKirLicmStorageV1(witness),
    ))
}

fn scalar(ty: &Type) -> bool {
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
fn total_scalar(a: &Inventory<'_>, at: usize, budget: &mut Budget<'_>) -> Result<bool> {
    budget.charge_work(4)?;
    let row = &a.operations()[at];
    let operation = row.operation;
    if operation.results.len() != 1 || !scalar(&operation.results[0].ty) {
        return Ok(false);
    }
    // This allowlist is deliberately independent of the optimizer's selector.
    let allowed = match &operation.kind {
        OperationKind::Constant(value) => scalar(&value.ty()),
        OperationKind::Unary {
            op: UnaryOp::Not, ..
        }
        | OperationKind::Binary {
            op: BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor,
            ..
        }
        | OperationKind::Compare { .. }
        | OperationKind::Select { .. } => true,
        // Includes Index/arithmetic/shift/cast/float/memory/call/intrinsic,
        // atomics/barriers/execution/tile/wave/matrix and any future operation.
        _ => false,
    };
    if !allowed {
        return Ok(false);
    }
    for use_at in row.operands.clone() {
        budget.charge_work(2)?;
        if !scalar(a.definitions()[a.uses()[use_at].definition].ty) {
            return Ok(false);
        }
    }
    Ok(true)
}
fn block_index(a: &Inventory<'_>, block: Block) -> Result<usize> {
    let function = a
        .functions()
        .get(block.function.0 as usize)
        .ok_or(Error::Mismatch("function coordinate"))?;
    function
        .blocks
        .start
        .checked_add(block.block as usize)
        .filter(|at| *at < function.blocks.end && a.blocks()[*at].coordinate == block)
        .ok_or(Error::Mismatch("block coordinate"))
}
fn operation_index(a: &Inventory<'_>, site: Site) -> Result<usize> {
    let block = &a.blocks()[block_index(a, site.block)?];
    block
        .operations
        .start
        .checked_add(site.operation as usize)
        .filter(|at| *at < block.operations.end && a.operations()[*at].coordinate == site)
        .ok_or(Error::Mismatch("operation coordinate"))
}
fn headers(a: &fe2o3_kernel_ir::Module, b: &fe2o3_kernel_ir::Module) -> Result<()> {
    let fe2o3_kernel_ir::Module {
        id,
        functions,
        kernels,
        required_capabilities,
    } = a;
    if id != &b.id
        || kernels != &b.kernels
        || required_capabilities != &b.required_capabilities
        || functions.len() != b.functions.len()
    {
        return Err(Error::Mismatch("module payload"));
    }
    for (a, b) in functions.iter().zip(&b.functions) {
        let fe2o3_kernel_ir::Function {
            id,
            signature,
            role,
            body,
            required_capabilities,
        } = a;
        if id != &b.id
            || signature != &b.signature
            || role != &b.role
            || required_capabilities != &b.required_capabilities
        {
            return Err(Error::Mismatch("function payload"));
        }
        match (body, &b.body) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                let fe2o3_kernel_ir::FunctionBody { parameters, blocks } = a;
                if parameters != &b.parameters || blocks.len() != b.blocks.len() {
                    return Err(Error::Mismatch("function body payload"));
                }
                for (a, b) in blocks.iter().zip(&b.blocks) {
                    let fe2o3_kernel_ir::BasicBlock {
                        id,
                        parameters,
                        operations: _,
                        terminator,
                    } = a;
                    if id != &b.id || parameters != &b.parameters || terminator != &b.terminator {
                        return Err(Error::Mismatch(
                            "exact CFG, parameters and edge occurrences",
                        ));
                    }
                }
            }
            _ => return Err(Error::Mismatch("declaration/body")),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "canonical_kir_licm_v1_tests.rs"]
mod tests;
