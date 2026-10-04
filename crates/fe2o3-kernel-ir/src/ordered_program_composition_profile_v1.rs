//! Finite structural policy. Fixed scratch, borrowed graph, no second interpreter.
use super::*;
use crate::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAME,
    AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAMESPACE,
    AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME, BasicBlock, BinaryOp, CastKind, Constant,
    FunctionBody, FunctionRole, Module, OperationKind, ScalarType, TargetCapability, Terminator,
    Type, UnaryOp, ValueId, WaveWidth, WorkgroupSize,
};
use std::{collections::BTreeSet, mem::size_of};
type E = OrderedProgramCompositionErrorV1;
const U32: Type = Type::Scalar(ScalarType::U32);
struct Scratch {
    definitions: [Option<OrderedProgramDefinitionV1>; 8],
    helpers: [Option<OrderedProgramHelperV1>; 2],
    calls: [Option<OrderedProgramCallV1>; 8],
    occurrences: [Option<OrderedProgramOccurrenceV1>; 8],
    definitions_len: usize,
    helpers_len: usize,
    calls_len: usize,
    occurrences_len: usize,
    instructions: usize,
    seen_definitions: [bool; 8],
    seen_calls: [bool; 8],
    seen_helpers: [bool; 2],
}
impl Scratch {
    fn new() -> Self {
        Self {
            definitions: [None; 8],
            helpers: [None; 2],
            calls: [None; 8],
            occurrences: [None; 8],
            definitions_len: 0,
            helpers_len: 0,
            calls_len: 0,
            occurrences_len: 0,
            instructions: 0,
            seen_definitions: [false; 8],
            seen_calls: [false; 8],
            seen_helpers: [false; 2],
        }
    }
}
fn add(a: usize, b: usize) -> Result<usize, E> {
    a.checked_add(b).ok_or(Resource::Arithmetic.into())
}
fn mul(a: usize, b: usize) -> Result<usize, E> {
    a.checked_mul(b).ok_or(Resource::Arithmetic.into())
}
fn u32_index(n: usize) -> Result<u32, E> {
    n.try_into().map_err(|_| E::Coordinate)
}

pub(super) fn build(
    canonical: VerifiedCanonicalKernelIrModuleV17,
    budget: &mut Budget<'_>,
) -> Result<
    (
        VerifiedOrderedProgramCompositionV1,
        OrderedProgramCompositionStorageV1,
    ),
    E,
> {
    let floor = budget.storage();
    budget.with_prepaid_scope(floor, 1, 1, size_of::<Scratch>(), move |budget| {
        let mut rows = Scratch::new();
        let module = canonical.module();
        budget.charge_work(8)?;
        if module.kernels.len() != 1 || module.functions.is_empty() || module.functions.len() > 3 {
            return Err(E::Root);
        }
        let kernel = &module.kernels[0];
        if kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1)) {
            return Err(E::Launch);
        }
        caps(&module.required_capabilities, true, budget)?;
        caps(&kernel.required_capabilities, true, budget)?;
        let root = function_index(module, &kernel.entry, budget)?.ok_or(E::Root)?;
        for (i, function) in module.functions.iter().enumerate() {
            budget.charge_work(1)?;
            if function.body.is_none()
                || function.role
                    != if i == root {
                        FunctionRole::KernelEntry
                    } else {
                        FunctionRole::InternalHelper
                    }
            {
                return Err(E::FunctionRole);
            }
            if i != root {
                if function.signature.parameters != [U32, U32, U32]
                    || function.signature.results != [U32]
                {
                    return Err(E::HelperSignature);
                }
                let key = OrderedProgramHelperKeyV1(u32_index(rows.helpers_len)?);
                rows.helpers[rows.helpers_len] = Some(OrderedProgramHelperV1 {
                    key,
                    function: u32_index(i)?,
                });
                rows.helpers_len += 1;
            }
        }
        // Canonical traversal order defines static keys; it never duplicates a helper.
        for (fi, function) in module.functions.iter().enumerate() {
            let body = function.body.as_ref().ok_or(E::FunctionRole)?;
            budget.charge_work(body.blocks.len())?;
            if body.blocks.is_empty() {
                return Err(E::PrefixControlFlow);
            }
            let before = rows.definitions_len;
            for (bi, block) in body.blocks.iter().enumerate() {
                budget.charge_work(add(block.parameters.len(), block.operations.len())?)?;
                if fi != root && block.parameters.iter().any(|p| !scalar(&p.ty)) {
                    return Err(E::HelperSignature);
                }
                for (oi, op) in block.operations.iter().enumerate() {
                    let site = OrderedProgramSiteV1 {
                        function: u32_index(fi)?,
                        block_ordinal: u32_index(bi)?,
                        block: block.id,
                        operation: u32_index(oi)?,
                    };
                    if fi != root {
                        helper_operation(body, op, budget)?;
                    } else if !root_operation(&op.kind) {
                        return Err(E::UnsupportedOperation);
                    }
                    match &op.kind {
                        OperationKind::Gfx942OrderedProgram(program) => {
                            budget.charge_work(4096)?;
                            program.validate_shape().map_err(|_| E::ProgramShape)?;
                            if rows.definitions_len == rows.definitions.len() {
                                return Err(E::DefinitionLimit);
                            }
                            let key =
                                OrderedProgramDefinitionKeyV1(u32_index(rows.definitions_len)?);
                            rows.definitions[rows.definitions_len] =
                                Some(OrderedProgramDefinitionV1 { key, site });
                            rows.definitions_len += 1;
                        }
                        OperationKind::Call { callee, arguments } => {
                            if fi != root {
                                return Err(E::HelperEffect);
                            }
                            budget.charge_work(add(arguments.len(), op.results.len())?)?;
                            if arguments.len() != 3
                                || op.results.len() != 1
                                || op.results[0].ty != U32
                            {
                                return Err(E::HelperSignature);
                            }
                            let callee_function =
                                function_index(module, callee, budget)?.ok_or(E::CallTarget)?;
                            let callee = rows.helpers[..rows.helpers_len]
                                .iter()
                                .flatten()
                                .find(|h| h.function as usize == callee_function)
                                .map(|h| h.key)
                                .ok_or(E::CallTarget)?;
                            if rows.calls_len == rows.calls.len() {
                                return Err(E::CallLimit);
                            }
                            let key = OrderedProgramCallKeyV1(u32_index(rows.calls_len)?);
                            rows.calls[rows.calls_len] =
                                Some(OrderedProgramCallV1 { key, site, callee });
                            rows.calls_len += 1;
                        }
                        _ => {}
                    }
                }
            }
            caps(
                &function.required_capabilities,
                fi == root || rows.definitions_len != before,
                budget,
            )?;
            if fi != root {
                helper_chain(body, budget)?;
            }
        }
        if rows.definitions_len == 0 {
            return Err(E::MissingProgram);
        }
        expand_root(module, root, &mut rows, budget)?;
        if rows.seen_calls[..rows.calls_len].iter().any(|v| !v)
            || rows.seen_definitions[..rows.definitions_len]
                .iter()
                .any(|v| !v)
        {
            return Err(E::PrefixControlFlow);
        }
        if rows.seen_helpers[..rows.helpers_len].iter().any(|v| !v) {
            return Err(E::UnreferencedHelper);
        }
        // Only the added owner header and exact new Vec capacities are retained.
        let inline = size_of::<VerifiedOrderedProgramCompositionV1>()
            .checked_sub(size_of::<VerifiedCanonicalKernelIrModuleV17>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(inline)?;
        let definitions = retained(&rows.definitions[..rows.definitions_len], budget)?;
        let helpers = retained(&rows.helpers[..rows.helpers_len], budget)?;
        let calls = retained(&rows.calls[..rows.calls_len], budget)?;
        let occurrences = retained(&rows.occurrences[..rows.occurrences_len], budget)?;
        let retained = budget
            .storage()
            .checked_sub(add(floor, size_of::<Scratch>())?)
            .ok_or(Resource::Accounting)?;
        Ok((
            VerifiedOrderedProgramCompositionV1 {
                canonical,
                root: u32_index(root)?,
                definitions,
                helpers,
                calls,
                occurrences,
                instructions: rows.instructions,
            },
            OrderedProgramCompositionStorageV1 { retained },
        ))
    })
}
fn retained<T: Copy>(rows: &[Option<T>], budget: &mut Budget<'_>) -> Result<Vec<T>, E> {
    budget.charge_work(rows.len())?;
    budget.reserve_storage(mul(rows.len(), size_of::<T>())?)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(rows.len())
        .map_err(|_| Resource::Allocation)?;
    if result.capacity() != rows.len() {
        return Err(Resource::Allocation.into());
    }
    for row in rows {
        result.push(row.ok_or(E::Coordinate)?);
    }
    Ok(result)
}
fn function_index(
    module: &Module,
    name: &crate::FunctionId,
    budget: &mut Budget<'_>,
) -> Result<Option<usize>, E> {
    for (i, function) in module.functions.iter().enumerate() {
        budget.charge_work(add(
            1,
            add(name.as_str().len(), function.id.as_str().len())?,
        )?)?;
        if &function.id == name {
            return Ok(Some(i));
        }
    }
    Ok(None)
}
fn caps(
    caps: &BTreeSet<TargetCapability>,
    required: bool,
    budget: &mut Budget<'_>,
) -> Result<(), E> {
    let (mut target, mut wave, mut program) = (false, false, false);
    for cap in caps {
        budget.charge_work(1)?;
        match cap {
            TargetCapability::WaveWidth(WaveWidth::Wave64) => wave = true,
            TargetCapability::WaveWidth(_) => return Err(E::TargetCapabilities),
            TargetCapability::Extension { namespace, name } => {
                budget.charge_work(add(namespace.len(), name.len())?)?;
                if namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE
                    && name == AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME
                {
                    target = true;
                } else if namespace == AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAMESPACE
                    && name == AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAME
                {
                    program = true;
                } else {
                    return Err(E::TargetCapabilities);
                }
            }
            // Ordinary root indexing/scalars may require this; no subgroup/LDS/atomic context is granted.
            TargetCapability::Int64 => {}
            _ => return Err(E::TargetCapabilities),
        }
    }
    if required && !(target && wave && program) {
        return Err(E::TargetCapabilities);
    }
    Ok(())
}
fn scalar(ty: &Type) -> bool {
    *ty == U32 || *ty == Type::BOOL
}
fn helper_operation(
    body: &FunctionBody,
    operation: &Operation,
    budget: &mut Budget<'_>,
) -> Result<(), E> {
    budget.charge_work(add(1, operation.results.len())?)?;
    if operation.results.iter().any(|v| !scalar(&v.ty)) {
        return Err(E::HelperSignature);
    }
    let admitted = match &operation.kind {
        OperationKind::Constant(Constant::U32(_) | Constant::Bool(_))
        | OperationKind::Unary {
            op: UnaryOp::Not, ..
        }
        | OperationKind::Compare { .. }
        | OperationKind::Select { .. }
        | OperationKind::Gfx942OrderedProgram(_) => true,
        OperationKind::Binary { op, rhs, .. } => match op {
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Checked(_) => true,
            BinaryOp::ShiftLeft | BinaryOp::ShiftRight => constant_shift(body, *rhs, budget)?,
            // Plain KIR arithmetic is defined-only, not wrapping. Checked
            // retains the full wrapped result and overflow flag without traps.
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Remainder => false,
        },
        OperationKind::Cast {
            kind: CastKind::ZeroExtend,
            to,
            ..
        } => *to == U32,
        _ => false,
    };
    if !admitted {
        return Err(E::HelperEffect);
    }
    Ok(())
}
fn constant_shift(body: &FunctionBody, value: ValueId, budget: &mut Budget<'_>) -> Result<bool, E> {
    for block in &body.blocks {
        budget.charge_work(add(1, block.operations.len())?)?;
        for op in &block.operations {
            budget.charge_work(op.results.len())?;
            if op.results.iter().any(|v| v.id == value) {
                return Ok(matches!(op.kind, OperationKind::Constant(Constant::U32(n)) if n < 32));
            }
        }
    }
    Ok(false)
}
fn root_operation(op: &OperationKind) -> bool {
    matches!(
        op,
        OperationKind::Constant(_)
            | OperationKind::Intrinsic(_)
            | OperationKind::Unary { .. }
            | OperationKind::Binary { .. }
            | OperationKind::Compare { .. }
            | OperationKind::Cast { .. }
            | OperationKind::Select { .. }
            | OperationKind::Call { .. }
            | OperationKind::SliceLength { .. }
            | OperationKind::SliceData { .. }
            | OperationKind::GetElementPointer { .. }
            | OperationKind::Load { .. }
            | OperationKind::Store { .. }
            | OperationKind::GuardedLoad { .. }
            | OperationKind::GuardedStore { .. }
            | OperationKind::Gfx942OrderedProgram(_)
    )
}
fn block_index(body: &FunctionBody, id: BlockId, budget: &mut Budget<'_>) -> Result<usize, E> {
    budget.charge_work(body.blocks.len())?;
    body.blocks
        .iter()
        .position(|b| b.id == id)
        .ok_or(E::Coordinate)
}
fn incoming(body: &FunctionBody, id: BlockId, budget: &mut Budget<'_>) -> Result<usize, E> {
    let mut count = 0;
    for block in &body.blocks {
        budget.charge_work(1)?;
        let mut edge = |target| -> Result<(), E> {
            if target == id {
                count = add(count, 1)?;
            }
            Ok(())
        };
        match block.terminator.as_ref().ok_or(E::Coordinate)? {
            Terminator::Branch { target, .. } => edge(*target)?,
            Terminator::ConditionalBranch {
                then_target,
                else_target,
                ..
            } => {
                edge(*then_target)?;
                edge(*else_target)?;
            }
            Terminator::Switch {
                cases,
                default_target,
                ..
            } => {
                budget.charge_work(cases.len())?;
                for c in cases {
                    edge(c.target)?;
                }
                edge(*default_target)?;
            }
            Terminator::IntegerSwitch {
                cases,
                default_target,
                ..
            } => {
                budget.charge_work(cases.len())?;
                for c in cases {
                    edge(c.target)?;
                }
                edge(*default_target)?;
            }
            Terminator::Return { .. } | Terminator::Unreachable => {}
        }
    }
    Ok(count)
}
fn helper_chain(body: &FunctionBody, budget: &mut Budget<'_>) -> Result<(), E> {
    let mut cursor = 0;
    for position in 0..body.blocks.len() {
        let block = &body.blocks[cursor];
        if incoming(body, block.id, budget)? != usize::from(position != 0) {
            return Err(E::HelperControlFlow);
        }
        match block.terminator.as_ref().ok_or(E::HelperControlFlow)? {
            Terminator::Branch { target, .. } => cursor = block_index(body, *target, budget)?,
            Terminator::Return { values }
                if values.len() == 1 && position + 1 == body.blocks.len() =>
            {
                return Ok(());
            }
            _ => return Err(E::HelperControlFlow),
        }
    }
    Err(E::HelperControlFlow)
}
fn occurrence(
    module: &Module,
    root: usize,
    definition: OrderedProgramDefinitionV1,
    incoming: Option<OrderedProgramCallKeyV1>,
    rows: &mut Scratch,
    budget: &mut Budget<'_>,
) -> Result<(), E> {
    budget.charge_work(1)?;
    if rows.occurrences_len == rows.occurrences.len() {
        return Err(E::OccurrenceLimit);
    }
    let site = definition.site;
    let op = &module.functions[site.function as usize]
        .body
        .as_ref()
        .ok_or(E::Coordinate)?
        .blocks[site.block_ordinal as usize]
        .operations[site.operation as usize];
    let OperationKind::Gfx942OrderedProgram(program) = &op.kind else {
        return Err(E::Coordinate);
    };
    rows.instructions = add(rows.instructions, usize::from(program.program().count()))?;
    if rows.instructions > ORDERED_PROGRAM_COMPOSITION_MAX_INSTRUCTIONS_V1 {
        return Err(E::InstructionLimit);
    }
    rows.seen_definitions[definition.key.0 as usize] = true;
    let key = OrderedProgramOccurrenceKeyV1(u32_index(rows.occurrences_len)?);
    rows.occurrences[rows.occurrences_len] = Some(OrderedProgramOccurrenceV1 {
        key,
        root: u32_index(root)?,
        incoming,
        definition: definition.key,
    });
    rows.occurrences_len += 1;
    Ok(())
}
fn expand_block(
    module: &Module,
    root: usize,
    function: usize,
    block: &BasicBlock,
    incoming: Option<OrderedProgramCallKeyV1>,
    rows: &mut Scratch,
    budget: &mut Budget<'_>,
) -> Result<(), E> {
    budget.charge_work(mul(block.operations.len(), 17)?)?;
    for (op, _) in block.operations.iter().enumerate() {
        if let Some(definition) = rows.definitions[..rows.definitions_len]
            .iter()
            .flatten()
            .find(|d| {
                d.site.function as usize == function
                    && d.site.block == block.id
                    && d.site.operation as usize == op
            })
            .copied()
        {
            occurrence(module, root, definition, incoming, rows, budget)?;
        }
        let call = rows.calls[..rows.calls_len]
            .iter()
            .flatten()
            .find(|c| {
                c.site.function as usize == function
                    && c.site.block == block.id
                    && c.site.operation as usize == op
            })
            .copied();
        if let Some(call) = call {
            rows.seen_calls[call.key.0 as usize] = true;
            rows.seen_helpers[call.callee.0 as usize] = true;
            let helper = rows.helpers[call.callee.0 as usize].ok_or(E::Coordinate)?;
            let body = module.functions[helper.function as usize]
                .body
                .as_ref()
                .ok_or(E::Coordinate)?;
            let mut cursor = 0;
            for _ in 0..body.blocks.len() {
                let helper_block = &body.blocks[cursor];
                // Helper calls were rejected, so this bounded traversal cannot recurse.
                budget.charge_work(mul(helper_block.operations.len(), 9)?)?;
                for (oi, _) in helper_block.operations.iter().enumerate() {
                    if let Some(def) = rows.definitions[..rows.definitions_len]
                        .iter()
                        .flatten()
                        .find(|d| {
                            d.site.function == helper.function
                                && d.site.block == helper_block.id
                                && d.site.operation as usize == oi
                        })
                        .copied()
                    {
                        occurrence(module, root, def, Some(call.key), rows, budget)?;
                    }
                }
                match helper_block.terminator.as_ref().ok_or(E::Coordinate)? {
                    Terminator::Branch { target, .. } => {
                        cursor = block_index(body, *target, budget)?
                    }
                    Terminator::Return { .. } => break,
                    _ => return Err(E::HelperControlFlow),
                }
            }
        }
    }
    Ok(())
}
fn expand_root(
    module: &Module,
    root: usize,
    rows: &mut Scratch,
    budget: &mut Budget<'_>,
) -> Result<(), E> {
    let body = module.functions[root].body.as_ref().ok_or(E::Root)?;
    let mut cursor = 0;
    for position in 0..body.blocks.len() {
        let block = &body.blocks[cursor];
        if incoming(body, block.id, budget)? != usize::from(position != 0) {
            return Err(E::PrefixControlFlow);
        }
        expand_block(module, root, root, block, None, rows, budget)?;
        match block.terminator.as_ref().ok_or(E::Coordinate)? {
            Terminator::Branch { target, .. } => cursor = block_index(body, *target, budget)?,
            _ => return Ok(()),
        }
    }
    Err(E::PrefixControlFlow)
}
