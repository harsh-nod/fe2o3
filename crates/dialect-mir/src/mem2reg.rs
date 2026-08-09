use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::executable::terminator_edges;
use crate::{
    MirBasicBlock, MirBlockId, MirBlockParameter, MirBody, MirBodyForm, MirCall, MirEdge,
    MirExecutableModule, MirExternalCallRegistry, MirLocalId, MirLocalKind, MirMutability,
    MirOperand, MirPlace, MirProjection, MirRvalue, MirStatement, MirStatementKind,
    MirTerminatorKind, MirTypeKind, MirUnwindAction, MirValueId,
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
/// V1 deliberately inserts a parameter for every promoted local on every
/// non-entry block. This is not minimal SSA, but it makes joins and backedges
/// explicit without relying on a dominance-frontier implementation. Locals
/// with projections, address-taking, storage markers, call destinations,
/// drops, or non-entry initialization remain slots.
pub fn promote_module_to_ssa(
    module: &MirExecutableModule,
) -> Result<(MirExecutableModule, MirMem2RegReport), MirMem2RegError> {
    promote_module_to_ssa_with_registry(module, &MirExternalCallRegistry::default())
}

/// Promotes after resolving device imports through an external trust root.
pub fn promote_module_to_ssa_with_registry(
    module: &MirExecutableModule,
    registry: &MirExternalCallRegistry,
) -> Result<(MirExecutableModule, MirMem2RegReport), MirMem2RegError> {
    module
        .validate_with_registry(registry)
        .map_err(|error| MirMem2RegError::new(error.path(), error.reason()))?;
    let mut output = module.clone();
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
        let promoted = eligible_locals(module, function_index, &original);
        let function_items = projected_output_items(function_index, &original, &promoted)?;
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
            transform_body(function_index, &original, &promoted)?;
        function.body = body;
        reports.push(MirMem2RegFunctionReport {
            identity: function.identity.clone(),
            promoted_locals: promoted,
            inserted_parameters,
            inserted_definitions,
        });
    }

    output.validate_with_registry(registry).map_err(|error| {
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

fn projected_output_items(
    function_index: usize,
    body: &MirBody,
    promoted: &[MirLocalId],
) -> Result<usize, MirMem2RegError> {
    let path = format!("module.functions[{function_index}].body");
    let entry_arguments = promoted
        .iter()
        .filter(|local| body.locals[local.0 as usize].kind == MirLocalKind::Argument)
        .count();
    let non_entry_parameters = body
        .blocks
        .len()
        .saturating_sub(1)
        .checked_mul(promoted.len())
        .ok_or_else(|| MirMem2RegError::new(&path, "mem2reg parameter budget overflow"))?;
    let definitions = body
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .filter(|statement| {
            matches!(
                &statement.kind,
                MirStatementKind::Assign { place, .. }
                    if place.projection.is_empty() && promoted.contains(&place.local)
            )
        })
        .count();
    let edges = body
        .blocks
        .iter()
        .map(|block| terminator_edges(&block.terminator.kind).len())
        .try_fold(0_usize, |total, count| total.checked_add(count))
        .ok_or_else(|| MirMem2RegError::new(&path, "mem2reg edge budget overflow"))?;
    let edge_arguments = edges
        .checked_mul(promoted.len())
        .ok_or_else(|| MirMem2RegError::new(&path, "mem2reg edge-argument budget overflow"))?;
    entry_arguments
        .checked_add(non_entry_parameters)
        .and_then(|total| total.checked_add(definitions))
        .and_then(|total| total.checked_add(edge_arguments))
        .ok_or_else(|| MirMem2RegError::new(path, "mem2reg output item budget overflow"))
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
) -> Result<(MirBody, usize, usize), MirMem2RegError> {
    let promoted_set = promoted.iter().copied().collect::<BTreeSet<_>>();
    let mut allocator = ValueAllocator::default();
    let mut blocks = Vec::with_capacity(body.blocks.len());
    let mut inserted_parameters = 0;
    let mut inserted_definitions = 0;

    for (block_index, original) in body.blocks.iter().enumerate() {
        let block_id = MirBlockId(block_index as u32);
        let parameter_locals = if block_id == body.entry {
            promoted
                .iter()
                .copied()
                .filter(|local| body.locals[local.0 as usize].kind == MirLocalKind::Argument)
                .collect::<Vec<_>>()
        } else {
            promoted.to_vec()
        };
        let mut current = BTreeMap::new();
        let parameters = parameter_locals
            .iter()
            .map(|local| {
                let value = allocator.allocate(function_index, block_index)?;
                current.insert(*local, value);
                Ok(MirBlockParameter {
                    value,
                    ty: body.locals[local.0 as usize].ty,
                    origin: Some(*local),
                })
            })
            .collect::<Result<Vec<_>, MirMem2RegError>>()?;
        inserted_parameters += parameters.len();

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
                    let value = allocator.allocate(function_index, block_index)?;
                    current.insert(place.local, value);
                    statements.push(MirStatement {
                        kind: MirStatementKind::Define {
                            value,
                            ty: place.ty,
                            rvalue,
                        },
                        span: statement.span.clone(),
                    });
                    inserted_definitions += 1;
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
            promoted,
            &promoted_set,
        )?;
        blocks.push(MirBasicBlock {
            parameters,
            statements,
            terminator,
        });
    }

    Ok((
        MirBody {
            form: MirBodyForm::Ssa {
                promoted_locals: promoted.to_vec(),
            },
            locals: body.locals.clone(),
            blocks,
            entry: body.entry,
        },
        inserted_parameters,
        inserted_definitions,
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
    current: &BTreeMap<MirLocalId, MirValueId>,
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
    current: &BTreeMap<MirLocalId, MirValueId>,
    promoted: &BTreeSet<MirLocalId>,
) -> Result<MirOperand, MirMem2RegError> {
    match operand {
        MirOperand::Copy(place) | MirOperand::Move(place)
            if place.projection.is_empty() && promoted.contains(&place.local) =>
        {
            current
                .get(&place.local)
                .copied()
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
    current: &BTreeMap<MirLocalId, MirValueId>,
    promoted_order: &[MirLocalId],
    promoted: &BTreeSet<MirLocalId>,
) -> Result<(), MirMem2RegError> {
    let path = format!("module.functions[{function_index}].body.blocks[{block_index}].terminator");
    match terminator {
        MirTerminatorKind::Goto(edge) => {
            rewrite_edge(&path, edge, current, promoted_order)?;
        }
        MirTerminatorKind::SwitchInt {
            discr,
            targets,
            otherwise,
        } => {
            *discr = rewrite_operand(&path, discr, current, promoted)?;
            for (_, edge) in targets {
                rewrite_edge(&path, edge, current, promoted_order)?;
            }
            rewrite_edge(&path, otherwise, current, promoted_order)?;
        }
        MirTerminatorKind::Call(call) => {
            rewrite_call(&path, call, current, promoted_order, promoted)?;
        }
        MirTerminatorKind::Drop { target, unwind, .. } => {
            rewrite_edge(&path, target, current, promoted_order)?;
            rewrite_unwind(&path, unwind, current, promoted_order)?;
        }
        MirTerminatorKind::Assert {
            condition,
            target,
            unwind,
            ..
        } => {
            *condition = rewrite_operand(&path, condition, current, promoted)?;
            rewrite_edge(&path, target, current, promoted_order)?;
            rewrite_unwind(&path, unwind, current, promoted_order)?;
        }
        MirTerminatorKind::Return | MirTerminatorKind::Unreachable => {}
    }
    Ok(())
}

fn rewrite_call(
    path: &str,
    call: &mut MirCall,
    current: &BTreeMap<MirLocalId, MirValueId>,
    promoted_order: &[MirLocalId],
    promoted: &BTreeSet<MirLocalId>,
) -> Result<(), MirMem2RegError> {
    for argument in &mut call.arguments {
        *argument = rewrite_operand(path, argument, current, promoted)?;
    }
    if let Some(target) = &mut call.target {
        rewrite_edge(path, target, current, promoted_order)?;
    }
    rewrite_unwind(path, &mut call.unwind, current, promoted_order)
}

fn rewrite_unwind(
    path: &str,
    unwind: &mut MirUnwindAction,
    current: &BTreeMap<MirLocalId, MirValueId>,
    promoted_order: &[MirLocalId],
) -> Result<(), MirMem2RegError> {
    if let MirUnwindAction::Cleanup(edge) = unwind {
        rewrite_edge(path, edge, current, promoted_order)?;
    }
    Ok(())
}

fn rewrite_edge(
    path: &str,
    edge: &mut MirEdge,
    current: &BTreeMap<MirLocalId, MirValueId>,
    promoted: &[MirLocalId],
) -> Result<(), MirMem2RegError> {
    if !edge.arguments.is_empty() {
        return Err(MirMem2RegError::new(
            path,
            "place-form input edge unexpectedly contains arguments",
        ));
    }
    edge.arguments = promoted
        .iter()
        .map(|local| {
            current
                .get(local)
                .copied()
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
