use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::control_flow::{MirControlFlowAnalysis, analyze_mir_control_flow};
use crate::executable::terminator_edges;
use crate::{
    MirBasicBlock, MirBlockId, MirBlockParameter, MirBody, MirBodyForm, MirCall, MirEdge,
    MirExecutableModule, MirExternalCallRegistry, MirLocalId, MirLocalKind, MirMutability,
    MirOperand, MirPlace, MirProjection, MirRvalue, MirStatement, MirStatementKind,
    MirTerminatorKind, MirTypeKind, MirUnwindAction, MirValueId, ValidatedMirExecutableModule,
};

pub const MAX_MEM2REG_OUTPUT_ITEMS: usize = 65_536;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirMem2RegFunctionReport {
    pub identity: String,
    pub promoted_locals: Vec<MirLocalId>,
    pub inserted_parameters: usize,
    pub inserted_definitions: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirMem2RegReport {
    pub functions: Vec<MirMem2RegFunctionReport>,
}

impl MirMem2RegReport {
    pub fn promoted_local_count(&self) -> usize {
        self.functions
            .iter()
            .map(|function| function.promoted_locals.len())
            .sum()
    }

    pub fn inserted_parameter_count(&self) -> usize {
        self.functions
            .iter()
            .map(|function| function.inserted_parameters)
            .sum()
    }

    pub fn inserted_definition_count(&self) -> usize {
        self.functions
            .iter()
            .map(|function| function.inserted_definitions)
            .sum()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirMem2RegError {
    path: String,
    reason: String,
}

impl MirMem2RegError {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    fn new(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for MirMem2RegError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.reason)
    }
}

impl std::error::Error for MirMem2RegError {}

/// Promotes a bounded subset of whole-place Copy locals into explicit SSA.
///
/// Parameters are placed at live iterated-dominance-frontier blocks. Values
/// defined in dominating blocks remain directly usable, while joins and loop
/// headers receive explicit edge arguments. Locals with projections,
/// address-taking, storage markers, call destinations, drops, or non-entry
/// initialization remain slots.
pub fn promote_module_to_ssa(
    module: &ValidatedMirExecutableModule,
) -> Result<(ValidatedMirExecutableModule, MirMem2RegReport), MirMem2RegError> {
    let source = module.as_module();
    let mut output = source.clone();
    let mut reports = Vec::with_capacity(output.functions.len());
    let mut output_items = 0_usize;

    for (function_index, function) in output.functions.iter_mut().enumerate() {
        if !matches!(function.body.form, MirBodyForm::Places) {
            return Err(MirMem2RegError::new(
                format!("module.functions[{function_index}].body.form"),
                "mem2reg accepts only place-form executable MIR",
            ));
        }
        let original = function.body.clone();
        let promoted = eligible_locals(source, function_index, &original);
        let control_flow = analyze_mir_control_flow(&original).map_err(|error| {
            MirMem2RegError::new(
                format!("module.functions[{function_index}].body"),
                format!("mem2reg requires reducible canonical control flow: {error}"),
            )
        })?;
        let plan = SsaPlan::build(&original, &promoted, &control_flow);
        let function_items = projected_output_items(function_index, &original, &plan)?;
        output_items = output_items
            .checked_add(function_items)
            .ok_or_else(|| MirMem2RegError::new("module", "mem2reg output item budget overflow"))?;
        if output_items > MAX_MEM2REG_OUTPUT_ITEMS {
            return Err(MirMem2RegError::new(
                format!("module.functions[{function_index}].body"),
                format!(
                    "mem2reg output requires {output_items} generated items, exceeding {MAX_MEM2REG_OUTPUT_ITEMS}"
                ),
            ));
        }
        let (body, inserted_parameters, inserted_definitions) =
            transform_body(function_index, &original, &promoted, &control_flow, &plan)?;
        function.body = body;
        reports.push(MirMem2RegFunctionReport {
            identity: function.identity.clone(),
            promoted_locals: promoted,
            inserted_parameters,
            inserted_definitions,
        });
    }

    let output = output
        .validate_with_registry(module.registry())
        .map_err(|error| {
            MirMem2RegError::new(
                error.path(),
                format!(
                    "mem2reg produced invalid executable MIR: {}",
                    error.reason()
                ),
            )
        })?;
    Ok((output, MirMem2RegReport { functions: reports }))
}

/// Compatibility constructor that validates untrusted data and immediately
/// delegates to the validated-only transform.
pub fn promote_module_to_ssa_with_registry(
    module: &MirExecutableModule,
    registry: &MirExternalCallRegistry,
) -> Result<(ValidatedMirExecutableModule, MirMem2RegReport), MirMem2RegError> {
    let validated = module
        .validate_with_registry(registry)
        .map_err(|error| MirMem2RegError::new(error.path(), error.reason()))?;
    promote_module_to_ssa(&validated)
}

fn projected_output_items(
    function_index: usize,
    body: &MirBody,
    plan: &SsaPlan,
) -> Result<usize, MirMem2RegError> {
    let path = format!("module.functions[{function_index}].body");
    let parameters = plan
        .parameter_locals
        .iter()
        .map(Vec::len)
        .try_fold(0_usize, usize::checked_add)
        .ok_or_else(|| MirMem2RegError::new(&path, "mem2reg parameter budget overflow"))?;
    let definitions = plan.definition_count;
    let edge_arguments = body
        .blocks
        .iter()
        .flat_map(|block| terminator_edges(&block.terminator.kind))
        .map(|edge| plan.parameter_locals[edge.target.0 as usize].len())
        .try_fold(0_usize, usize::checked_add)
        .ok_or_else(|| MirMem2RegError::new(&path, "mem2reg edge-argument budget overflow"))?;
    parameters
        .checked_add(definitions)
        .and_then(|total| total.checked_add(edge_arguments))
        .ok_or_else(|| MirMem2RegError::new(path, "mem2reg output item budget overflow"))
}

struct SsaPlan {
    parameter_locals: Vec<Vec<MirLocalId>>,
    definition_count: usize,
}

impl SsaPlan {
    fn build(
        body: &MirBody,
        promoted: &[MirLocalId],
        control_flow: &MirControlFlowAnalysis,
    ) -> Self {
        let promoted_set = promoted.iter().copied().collect::<BTreeSet<_>>();
        let mut definitions = vec![BTreeSet::new(); body.blocks.len()];
        let mut upward_exposed_uses = vec![BTreeSet::new(); body.blocks.len()];
        let mut definition_count = 0;

        for (block_index, block) in body.blocks.iter().enumerate() {
            if MirBlockId(block_index as u32) == body.entry {
                definitions[block_index].extend(
                    promoted.iter().copied().filter(|local| {
                        body.locals[local.0 as usize].kind == MirLocalKind::Argument
                    }),
                );
            }
            for statement in &block.statements {
                for local in &promoted_set {
                    if statement_reads_local(statement, *local)
                        && !definitions[block_index].contains(local)
                    {
                        upward_exposed_uses[block_index].insert(*local);
                    }
                }
                if let MirStatementKind::Assign { place, .. } = &statement.kind
                    && place.projection.is_empty()
                    && promoted_set.contains(&place.local)
                {
                    definitions[block_index].insert(place.local);
                    definition_count += 1;
                }
            }
            for local in &promoted_set {
                if terminator_reads_local(&block.terminator.kind, *local)
                    && !definitions[block_index].contains(local)
                {
                    upward_exposed_uses[block_index].insert(*local);
                }
            }
        }

        let mut live_in = vec![BTreeSet::new(); body.blocks.len()];
        let mut live_out = vec![BTreeSet::new(); body.blocks.len()];
        loop {
            let mut changed = false;
            for block_index in (0..body.blocks.len()).rev() {
                let block = MirBlockId(block_index as u32);
                let next_out = control_flow
                    .successors(block)
                    .expect("analysis covers every canonical block")
                    .iter()
                    .flat_map(|successor| live_in[successor.0 as usize].iter().copied())
                    .collect::<BTreeSet<_>>();
                let mut next_in = upward_exposed_uses[block_index].clone();
                next_in.extend(
                    next_out
                        .iter()
                        .filter(|local| !definitions[block_index].contains(local))
                        .copied(),
                );
                changed |= live_out[block_index] != next_out || live_in[block_index] != next_in;
                live_out[block_index] = next_out;
                live_in[block_index] = next_in;
            }
            if !changed {
                break;
            }
        }

        let mut parameter_locals = vec![Vec::new(); body.blocks.len()];
        parameter_locals[body.entry.0 as usize].extend(
            promoted
                .iter()
                .copied()
                .filter(|local| body.locals[local.0 as usize].kind == MirLocalKind::Argument),
        );
        for local in promoted {
            let definition_blocks = definitions
                .iter()
                .enumerate()
                .filter_map(|(index, locals)| {
                    locals.contains(local).then_some(MirBlockId(index as u32))
                })
                .collect::<BTreeSet<_>>();
            let frontiers = control_flow
                .iterated_dominance_frontier(&definition_blocks)
                .expect("definition blocks belong to the analyzed body");
            for block in frontiers {
                if block != body.entry && live_in[block.0 as usize].contains(local) {
                    parameter_locals[block.0 as usize].push(*local);
                }
            }
        }
        for locals in &mut parameter_locals {
            locals.sort();
            locals.dedup();
        }

        Self {
            parameter_locals,
            definition_count,
        }
    }
}

fn eligible_locals(
    module: &MirExecutableModule,
    function_index: usize,
    body: &MirBody,
) -> Vec<MirLocalId> {
    let mut disqualified = BTreeSet::new();
    let mut touched = BTreeSet::new();
    for block in &body.blocks {
        for statement in &block.statements {
            inspect_statement(statement, &mut touched, &mut disqualified);
        }
        inspect_terminator(&block.terminator.kind, &mut touched, &mut disqualified);
    }

    let function = &module.functions[function_index];
    let entry = &body.blocks[body.entry.0 as usize];
    body.locals
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(index, local)| {
            let id = MirLocalId(index as u32);
            let copy_type = match &module.types[local.ty.0 as usize].kind {
                MirTypeKind::Unit | MirTypeKind::Scalar(_) | MirTypeKind::RawPointer { .. } => true,
                MirTypeKind::Reference { mutability, .. } => {
                    *mutability == MirMutability::Immutable
                }
                _ => false,
            };
            if !copy_type || disqualified.contains(&id) || !touched.contains(&id) {
                return None;
            }
            if local.kind != MirLocalKind::Argument && !initialized_in_entry(entry, id) {
                return None;
            }
            debug_assert_eq!(function.body.locals[index].ty, local.ty);
            Some(id)
        })
        .collect()
}

fn initialized_in_entry(entry: &MirBasicBlock, local: MirLocalId) -> bool {
    let mut initialized = false;
    for statement in &entry.statements {
        if statement_reads_local(statement, local) && !initialized {
            return false;
        }
        if matches!(
            &statement.kind,
            MirStatementKind::Assign { place, .. }
                if place.local == local && place.projection.is_empty()
        ) {
            initialized = true;
        }
    }
    initialized
}

fn transform_body(
    function_index: usize,
    body: &MirBody,
    promoted: &[MirLocalId],
    control_flow: &MirControlFlowAnalysis,
    plan: &SsaPlan,
) -> Result<(MirBody, usize, usize), MirMem2RegError> {
    let promoted_set = promoted.iter().copied().collect::<BTreeSet<_>>();
    let mut allocator = ValueAllocator::default();
    let mut parameter_values = vec![Vec::new(); body.blocks.len()];
    let mut definition_values = BTreeMap::new();
    for (block_index, block) in body.blocks.iter().enumerate() {
        parameter_values[block_index] = plan.parameter_locals[block_index]
            .iter()
            .map(|local| {
                let value = allocator.allocate(function_index, block_index)?;
                Ok((*local, value))
            })
            .collect::<Result<Vec<_>, MirMem2RegError>>()?;
        for (statement_index, statement) in block.statements.iter().enumerate() {
            if let MirStatementKind::Assign { place, .. } = &statement.kind
                && place.projection.is_empty()
                && promoted_set.contains(&place.local)
            {
                let value = allocator.allocate(function_index, block_index)?;
                definition_values.insert((block_index, statement_index), (place.local, value));
            }
        }
    }

    enum RenameEvent {
        Enter(MirBlockId),
        Exit(Vec<MirLocalId>),
    }

    let mut blocks = vec![None; body.blocks.len()];
    let mut current = BTreeMap::<MirLocalId, Vec<MirValueId>>::new();
    let mut events = vec![RenameEvent::Enter(body.entry)];
    while let Some(event) = events.pop() {
        let block_id = match event {
            RenameEvent::Enter(block) => block,
            RenameEvent::Exit(pushed) => {
                for local in pushed.into_iter().rev() {
                    let values = current
                        .get_mut(&local)
                        .expect("a rename exit has a matching value stack");
                    values.pop();
                    if values.is_empty() {
                        current.remove(&local);
                    }
                }
                continue;
            }
        };
        let block_index = block_id.0 as usize;
        let original = &body.blocks[block_index];
        let mut pushed = Vec::new();
        let parameters = parameter_values[block_index]
            .iter()
            .map(|(local, value)| {
                current.entry(*local).or_default().push(*value);
                pushed.push(*local);
                MirBlockParameter {
                    value: *value,
                    ty: body.locals[local.0 as usize].ty,
                    origin: Some(*local),
                }
            })
            .collect::<Vec<_>>();
        let mut statements = Vec::with_capacity(original.statements.len());
        for (statement_index, statement) in original.statements.iter().enumerate() {
            let path = format!(
                "module.functions[{function_index}].body.blocks[{block_index}].statements[{statement_index}]"
            );
            match &statement.kind {
                MirStatementKind::Assign { place, value }
                    if place.projection.is_empty() && promoted_set.contains(&place.local) =>
                {
                    let rvalue = rewrite_rvalue(&path, value, &current, &promoted_set)?;
                    let (_, value) = definition_values[&(block_index, statement_index)];
                    current.entry(place.local).or_default().push(value);
                    pushed.push(place.local);
                    statements.push(MirStatement {
                        kind: MirStatementKind::Define {
                            value,
                            ty: place.ty,
                            rvalue,
                        },
                        span: statement.span.clone(),
                    });
                }
                MirStatementKind::Assign { place, value } => {
                    statements.push(MirStatement {
                        kind: MirStatementKind::Assign {
                            place: place.clone(),
                            value: rewrite_rvalue(&path, value, &current, &promoted_set)?,
                        },
                        span: statement.span.clone(),
                    });
                }
                MirStatementKind::Define { .. } => {
                    return Err(MirMem2RegError::new(
                        path,
                        "place-form input unexpectedly contains an SSA definition",
                    ));
                }
                _ => statements.push(statement.clone()),
            }
        }
        let mut terminator = original.terminator.clone();
        rewrite_terminator(
            function_index,
            block_index,
            &mut terminator.kind,
            &current,
            &plan.parameter_locals,
            &promoted_set,
        )?;
        blocks[block_index] = Some(MirBasicBlock {
            parameters,
            statements,
            terminator,
        });
        events.push(RenameEvent::Exit(pushed));
        for child in control_flow
            .dominator_tree_children(block_id)
            .expect("analysis covers every canonical block")
            .iter()
            .rev()
        {
            events.push(RenameEvent::Enter(*child));
        }
    }

    Ok((
        MirBody {
            form: MirBodyForm::Ssa {
                promoted_locals: promoted.to_vec(),
            },
            locals: body.locals.clone(),
            blocks: blocks
                .into_iter()
                .map(|block| block.expect("reachable dominator traversal visits every block"))
                .collect(),
            entry: body.entry,
        },
        plan.parameter_locals.iter().map(Vec::len).sum(),
        plan.definition_count,
    ))
}

#[derive(Default)]
struct ValueAllocator {
    next: u32,
}

impl ValueAllocator {
    fn allocate(
        &mut self,
        function_index: usize,
        block_index: usize,
    ) -> Result<MirValueId, MirMem2RegError> {
        let value = MirValueId(self.next);
        self.next = self.next.checked_add(1).ok_or_else(|| {
            MirMem2RegError::new(
                format!("module.functions[{function_index}].body.blocks[{block_index}]"),
                "SSA value identity overflow",
            )
        })?;
        Ok(value)
    }
}

fn rewrite_rvalue(
    path: &str,
    rvalue: &MirRvalue,
    current: &BTreeMap<MirLocalId, Vec<MirValueId>>,
    promoted: &BTreeSet<MirLocalId>,
) -> Result<MirRvalue, MirMem2RegError> {
    Ok(match rvalue {
        MirRvalue::Use(operand) => {
            MirRvalue::Use(rewrite_operand(path, operand, current, promoted)?)
        }
        MirRvalue::BinaryOp { op, lhs, rhs } => MirRvalue::BinaryOp {
            op: *op,
            lhs: rewrite_operand(path, lhs, current, promoted)?,
            rhs: rewrite_operand(path, rhs, current, promoted)?,
        },
        MirRvalue::CheckedBinaryOp { op, lhs, rhs } => MirRvalue::CheckedBinaryOp {
            op: *op,
            lhs: rewrite_operand(path, lhs, current, promoted)?,
            rhs: rewrite_operand(path, rhs, current, promoted)?,
        },
        MirRvalue::UnaryOp { op, operand } => MirRvalue::UnaryOp {
            op: *op,
            operand: rewrite_operand(path, operand, current, promoted)?,
        },
        MirRvalue::Cast { kind, operand, ty } => MirRvalue::Cast {
            kind: *kind,
            operand: rewrite_operand(path, operand, current, promoted)?,
            ty: *ty,
        },
        MirRvalue::Aggregate { kind, operands } => MirRvalue::Aggregate {
            kind: kind.clone(),
            operands: operands
                .iter()
                .map(|operand| rewrite_operand(path, operand, current, promoted))
                .collect::<Result<Vec<_>, _>>()?,
        },
        MirRvalue::Repeat { operand, count } => MirRvalue::Repeat {
            operand: rewrite_operand(path, operand, current, promoted)?,
            count: *count,
        },
        MirRvalue::Ref { .. }
        | MirRvalue::AddressOf { .. }
        | MirRvalue::Len(_)
        | MirRvalue::Discriminant(_)
        | MirRvalue::ThreadIndex1d => rvalue.clone(),
    })
}

fn rewrite_operand(
    path: &str,
    operand: &MirOperand,
    current: &BTreeMap<MirLocalId, Vec<MirValueId>>,
    promoted: &BTreeSet<MirLocalId>,
) -> Result<MirOperand, MirMem2RegError> {
    match operand {
        MirOperand::Copy(place) | MirOperand::Move(place)
            if place.projection.is_empty() && promoted.contains(&place.local) =>
        {
            current
                .get(&place.local)
                .and_then(|values| values.last().copied())
                .map(MirOperand::Value)
                .ok_or_else(|| {
                    MirMem2RegError::new(
                        path,
                        format!("promoted local {} is read before definition", place.local.0),
                    )
                })
        }
        _ => Ok(operand.clone()),
    }
}

fn rewrite_terminator(
    function_index: usize,
    block_index: usize,
    terminator: &mut MirTerminatorKind,
    current: &BTreeMap<MirLocalId, Vec<MirValueId>>,
    parameter_locals: &[Vec<MirLocalId>],
    promoted: &BTreeSet<MirLocalId>,
) -> Result<(), MirMem2RegError> {
    let path = format!("module.functions[{function_index}].body.blocks[{block_index}].terminator");
    match terminator {
        MirTerminatorKind::Goto(edge) => {
            rewrite_edge(&path, edge, current, parameter_locals)?;
        }
        MirTerminatorKind::SwitchInt {
            discr,
            targets,
            otherwise,
        } => {
            *discr = rewrite_operand(&path, discr, current, promoted)?;
            for (_, edge) in targets {
                rewrite_edge(&path, edge, current, parameter_locals)?;
            }
            rewrite_edge(&path, otherwise, current, parameter_locals)?;
        }
        MirTerminatorKind::Call(call) => {
            rewrite_call(&path, call, current, parameter_locals, promoted)?;
        }
        MirTerminatorKind::Drop { target, unwind, .. } => {
            rewrite_edge(&path, target, current, parameter_locals)?;
            rewrite_unwind(&path, unwind, current, parameter_locals)?;
        }
        MirTerminatorKind::Assert {
            condition,
            target,
            unwind,
            ..
        } => {
            *condition = rewrite_operand(&path, condition, current, promoted)?;
            rewrite_edge(&path, target, current, parameter_locals)?;
            rewrite_unwind(&path, unwind, current, parameter_locals)?;
        }
        MirTerminatorKind::Return | MirTerminatorKind::Unreachable => {}
    }
    Ok(())
}

fn rewrite_call(
    path: &str,
    call: &mut MirCall,
    current: &BTreeMap<MirLocalId, Vec<MirValueId>>,
    parameter_locals: &[Vec<MirLocalId>],
    promoted: &BTreeSet<MirLocalId>,
) -> Result<(), MirMem2RegError> {
    for argument in &mut call.arguments {
        *argument = rewrite_operand(path, argument, current, promoted)?;
    }
    if let Some(target) = &mut call.target {
        rewrite_edge(path, target, current, parameter_locals)?;
    }
    rewrite_unwind(path, &mut call.unwind, current, parameter_locals)
}

fn rewrite_unwind(
    path: &str,
    unwind: &mut MirUnwindAction,
    current: &BTreeMap<MirLocalId, Vec<MirValueId>>,
    parameter_locals: &[Vec<MirLocalId>],
) -> Result<(), MirMem2RegError> {
    if let MirUnwindAction::Cleanup(edge) = unwind {
        rewrite_edge(path, edge, current, parameter_locals)?;
    }
    Ok(())
}

fn rewrite_edge(
    path: &str,
    edge: &mut MirEdge,
    current: &BTreeMap<MirLocalId, Vec<MirValueId>>,
    parameter_locals: &[Vec<MirLocalId>],
) -> Result<(), MirMem2RegError> {
    if !edge.arguments.is_empty() {
        return Err(MirMem2RegError::new(
            path,
            "place-form input edge unexpectedly contains arguments",
        ));
    }
    edge.arguments = parameter_locals[edge.target.0 as usize]
        .iter()
        .map(|local| {
            current
                .get(local)
                .and_then(|values| values.last().copied())
                .map(MirOperand::Value)
                .ok_or_else(|| {
                    MirMem2RegError::new(
                        path,
                        format!("promoted local {} is undefined at an edge", local.0),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(())
}

fn inspect_statement(
    statement: &MirStatement,
    touched: &mut BTreeSet<MirLocalId>,
    disqualified: &mut BTreeSet<MirLocalId>,
) {
    match &statement.kind {
        MirStatementKind::Assign { place, value } => {
            inspect_place(place, PlaceUse::Destination, touched, disqualified);
            inspect_rvalue(value, touched, disqualified);
        }
        MirStatementKind::Define { .. } => {}
        MirStatementKind::SetDiscriminant { place, .. } | MirStatementKind::Deinit(place) => {
            inspect_place(place, PlaceUse::Unsupported, touched, disqualified);
        }
        MirStatementKind::StorageLive(local) | MirStatementKind::StorageDead(local) => {
            touched.insert(*local);
            disqualified.insert(*local);
        }
        MirStatementKind::Nop => {}
    }
}

fn inspect_rvalue(
    rvalue: &MirRvalue,
    touched: &mut BTreeSet<MirLocalId>,
    disqualified: &mut BTreeSet<MirLocalId>,
) {
    match rvalue {
        MirRvalue::Use(operand)
        | MirRvalue::UnaryOp { operand, .. }
        | MirRvalue::Cast { operand, .. }
        | MirRvalue::Repeat { operand, .. } => inspect_operand(operand, touched, disqualified),
        MirRvalue::BinaryOp { lhs, rhs, .. } | MirRvalue::CheckedBinaryOp { lhs, rhs, .. } => {
            inspect_operand(lhs, touched, disqualified);
            inspect_operand(rhs, touched, disqualified);
        }
        MirRvalue::Ref { place, .. } | MirRvalue::AddressOf { place, .. } => {
            inspect_place(place, PlaceUse::Address, touched, disqualified);
        }
        MirRvalue::Len(place) | MirRvalue::Discriminant(place) => {
            inspect_place(place, PlaceUse::Read, touched, disqualified);
        }
        MirRvalue::Aggregate { operands, .. } => {
            for operand in operands {
                inspect_operand(operand, touched, disqualified);
            }
        }
        MirRvalue::ThreadIndex1d => {}
    }
}

fn inspect_operand(
    operand: &MirOperand,
    touched: &mut BTreeSet<MirLocalId>,
    disqualified: &mut BTreeSet<MirLocalId>,
) {
    if let MirOperand::Copy(place) | MirOperand::Move(place) = operand {
        inspect_place(place, PlaceUse::Read, touched, disqualified);
    }
}

#[derive(Clone, Copy)]
enum PlaceUse {
    Read,
    Destination,
    Address,
    Unsupported,
}

fn inspect_place(
    place: &MirPlace,
    usage: PlaceUse,
    touched: &mut BTreeSet<MirLocalId>,
    disqualified: &mut BTreeSet<MirLocalId>,
) {
    touched.insert(place.local);
    if !place.projection.is_empty() || matches!(usage, PlaceUse::Address | PlaceUse::Unsupported) {
        disqualified.insert(place.local);
    }
    for projection in &place.projection {
        if let MirProjection::Index { local } = projection {
            touched.insert(*local);
            disqualified.insert(*local);
        }
    }
}

fn inspect_terminator(
    terminator: &MirTerminatorKind,
    touched: &mut BTreeSet<MirLocalId>,
    disqualified: &mut BTreeSet<MirLocalId>,
) {
    match terminator {
        MirTerminatorKind::SwitchInt { discr, .. }
        | MirTerminatorKind::Assert {
            condition: discr, ..
        } => inspect_operand(discr, touched, disqualified),
        MirTerminatorKind::Call(call) => {
            for argument in &call.arguments {
                inspect_operand(argument, touched, disqualified);
            }
            if let Some(destination) = &call.destination {
                inspect_place(destination, PlaceUse::Unsupported, touched, disqualified);
            }
        }
        MirTerminatorKind::Drop { place, .. } => {
            inspect_place(place, PlaceUse::Unsupported, touched, disqualified);
        }
        MirTerminatorKind::Goto(_) | MirTerminatorKind::Return | MirTerminatorKind::Unreachable => {
        }
    }
}

fn terminator_reads_local(terminator: &MirTerminatorKind, local: MirLocalId) -> bool {
    match terminator {
        MirTerminatorKind::SwitchInt { discr, .. }
        | MirTerminatorKind::Assert {
            condition: discr, ..
        } => operand_reads_local(discr, local),
        MirTerminatorKind::Call(call) => call
            .arguments
            .iter()
            .any(|operand| operand_reads_local(operand, local)),
        MirTerminatorKind::Drop { place, .. } => place_reads_local(place, local),
        MirTerminatorKind::Goto(_) | MirTerminatorKind::Return | MirTerminatorKind::Unreachable => {
            false
        }
    }
}

fn statement_reads_local(statement: &MirStatement, local: MirLocalId) -> bool {
    match &statement.kind {
        MirStatementKind::Assign { value, .. } => rvalue_reads_local(value, local),
        MirStatementKind::SetDiscriminant { place, .. } | MirStatementKind::Deinit(place) => {
            place_reads_local(place, local)
        }
        MirStatementKind::Define { rvalue, .. } => rvalue_reads_local(rvalue, local),
        MirStatementKind::StorageLive(_)
        | MirStatementKind::StorageDead(_)
        | MirStatementKind::Nop => false,
    }
}

fn rvalue_reads_local(rvalue: &MirRvalue, local: MirLocalId) -> bool {
    match rvalue {
        MirRvalue::Use(operand)
        | MirRvalue::UnaryOp { operand, .. }
        | MirRvalue::Cast { operand, .. }
        | MirRvalue::Repeat { operand, .. } => operand_reads_local(operand, local),
        MirRvalue::BinaryOp { lhs, rhs, .. } | MirRvalue::CheckedBinaryOp { lhs, rhs, .. } => {
            operand_reads_local(lhs, local) || operand_reads_local(rhs, local)
        }
        MirRvalue::Ref { place, .. }
        | MirRvalue::AddressOf { place, .. }
        | MirRvalue::Len(place)
        | MirRvalue::Discriminant(place) => place_reads_local(place, local),
        MirRvalue::Aggregate { operands, .. } => operands
            .iter()
            .any(|operand| operand_reads_local(operand, local)),
        MirRvalue::ThreadIndex1d => false,
    }
}

fn operand_reads_local(operand: &MirOperand, local: MirLocalId) -> bool {
    matches!(operand, MirOperand::Copy(place) | MirOperand::Move(place) if place_reads_local(place, local))
}

fn place_reads_local(place: &MirPlace, local: MirLocalId) -> bool {
    place.local == local
        || place
            .projection
            .iter()
            .any(|projection| matches!(projection, MirProjection::Index { local: index } if *index == local))
}
