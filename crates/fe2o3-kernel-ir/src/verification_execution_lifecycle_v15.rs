//! Same-graph affine execution ownership. This is not source or schedule authentication.

use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError, DiagnosticCode,
    ExecutionOperationV15 as Execution, ExecutionRoleV15 as Role, Function, FunctionRole,
    MeteredIndexedControlFlowV1, Module, Operation, OperationKind, Terminator, Type, ValueId,
    VerificationDefinitionSiteV1, VerificationDiagnosticCollectorV1,
    VerificationDiagnosticLocationV1, VerificationFunctionStateV1, clone_diagnostic_location_v1,
    emit_fixed_v1, function_diagnostic_location_v1,
};

pub(crate) fn invalid_execution_type_v15(
    mut ty: &Type,
    mut allow_direct: bool,
    budget: &mut Budget<'_>,
) -> Result<bool, ResourceError> {
    loop {
        budget.charge_work(1)?;
        match ty {
            Type::Execution(role) => return Ok(!allow_direct || role.validate().is_err()),
            Type::Pointer(pointer) => ty = &pointer.pointee,
            Type::Slice(slice) => ty = &slice.element,
            Type::Unit | Type::Scalar(_) | Type::Vector(_) => return Ok(false),
        }
        allow_direct = false;
    }
}

#[derive(Clone, Copy)]
struct Slot {
    value: ValueId,
    role: Role,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Live {
    Absent,
    Context,
    Borrowed { workgroup: usize },
    Workgroup { context: usize },
    Descendant { workgroup: usize },
}

fn reserve_vec<T>(count: usize) -> Result<Vec<T>, ResourceError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| ResourceError::Allocation)?;
    Ok(result)
}

fn slot_index(
    slots: &[Slot],
    value: ValueId,
    budget: &mut Budget<'_>,
) -> Result<Option<usize>, ResourceError> {
    budget.charge_work(
        slots
            .len()
            .checked_ilog2()
            .map_or(1, |log| log as usize + 2),
    )?;
    Ok(slots.binary_search_by_key(&value, |slot| slot.value).ok())
}

fn fail(
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    location: VerificationDiagnosticLocationV1<'_>,
    message: &'static str,
    budget: &mut Budget<'_>,
) -> Result<(), ResourceError> {
    emit_fixed_v1(
        diagnostics,
        location,
        DiagnosticCode::InvalidSemanticOperation,
        message,
        budget,
    )
}

pub(crate) fn verify_execution_lifecycle_v15(
    module: &Module,
    function: &Function,
    definitions: &VerificationFunctionStateV1<'_>,
    control_flow: Option<&MeteredIndexedControlFlowV1>,
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    budget: &mut Budget<'_>,
) -> Result<(), ResourceError> {
    let mut scratch = 0;
    let result = verify_inner(
        module,
        function,
        definitions,
        control_flow,
        diagnostics,
        budget,
        &mut scratch,
    );
    // Diagnostic buffers belong to the caller and survive this local scratch owner.
    let released = budget.release_storage(scratch);
    result.and(released)
}

fn verify_inner(
    module: &Module,
    function: &Function,
    definitions: &VerificationFunctionStateV1<'_>,
    control_flow: Option<&MeteredIndexedControlFlowV1>,
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    budget: &mut Budget<'_>,
    scratch: &mut usize,
) -> Result<(), ResourceError> {
    let Some(body) = &function.body else {
        return Ok(());
    };
    budget.charge_work(definitions.definition_rows().len())?;
    let count = definitions
        .definition_rows()
        .iter()
        .filter(|row| matches!(row.value.ty, Type::Execution(_)))
        .count();
    if count == 0 {
        return Ok(());
    }
    let location = function_diagnostic_location_v1(module, function, budget)?;
    let Some(control_flow) = control_flow else {
        return fail(
            diagnostics,
            location,
            "execution lifecycle requires a valid CFG",
            budget,
        );
    };
    let flow = control_flow.indexed_v15();
    let blocks = flow.block_count();
    let cells = blocks
        .checked_add(1)
        .and_then(|n| n.checked_mul(count))
        .ok_or(ResourceError::Arithmetic)?;
    let storage = cells
        .checked_mul(std::mem::size_of::<Live>())
        .and_then(|n| {
            count
                .checked_mul(std::mem::size_of::<Slot>())
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| {
            blocks
                .checked_mul(
                    std::mem::size_of::<Option<Vec<Live>>>() + std::mem::size_of::<usize>(),
                )
                .and_then(|m| n.checked_add(m))
        })
        .ok_or(ResourceError::Arithmetic)?;
    budget.reserve_storage(storage)?;
    *scratch = storage;
    budget.charge_work(definitions.definition_rows().len())?;
    let mut slots = reserve_vec(count)?;
    for row in definitions.definition_rows() {
        if let Type::Execution(role) = row.value.ty {
            if !matches!(
                row.value.site,
                VerificationDefinitionSiteV1::Operation(_, _)
            ) {
                return fail(
                    diagnostics,
                    location,
                    "execution roles cannot be function or block parameters",
                    budget,
                );
            }
            slots.push(Slot {
                value: ValueId(row.key),
                role: *role,
            });
        }
    }
    budget.charge_work(body.blocks.len())?;
    let mut issuances = 0_usize;
    for block in &body.blocks {
        budget.charge_work(
            block
                .operations
                .len()
                .checked_mul(2)
                .ok_or(ResourceError::Arithmetic)?,
        )?;
        issuances = issuances
            .checked_add(
                block
                    .operations
                    .iter()
                    .filter(|operation| {
                        matches!(
                            operation.kind,
                            OperationKind::Execution(Execution::ContextIssue)
                        )
                    })
                    .count(),
            )
            .ok_or(ResourceError::Arithmetic)?;
        budget.charge_work(blocks.checked_ilog2().map_or(1, |log| log as usize + 2))?;
        if !flow.is_reachable(block.id)
            && block
                .operations
                .iter()
                .any(|op| matches!(op.kind, OperationKind::Execution(_)))
        {
            return fail(
                diagnostics,
                clone_diagnostic_location_v1(&location, budget)?.at_block(block.id),
                "unreachable execution capability island",
                budget,
            );
        }
    }
    if issuances != 1 {
        return fail(
            diagnostics,
            location,
            "execution lifecycle requires exactly one context issuer per kernel entry",
            budget,
        );
    }
    let Some(entry) = body.blocks.first() else {
        return Ok(());
    };
    budget.charge_work(blocks.checked_ilog2().map_or(1, |log| log as usize + 2))?;
    let entry = flow
        .block_position(entry.id)
        .ok_or(ResourceError::Accounting)?;
    let mut incoming: Vec<Option<Vec<Live>>> = reserve_vec(blocks)?;
    budget.charge_work(blocks)?;
    incoming.resize_with(blocks, || None);
    let mut initial = reserve_vec(count)?;
    budget.charge_work(count)?;
    initial.resize(count, Live::Absent);
    incoming[entry] = Some(initial);
    // Each block receives one concrete ownership invariant. Deterministic transfer
    // checks it once; every incoming edge, including a backedge, must reproduce it.
    // Execution roles cannot be block parameters, so no nominal phi transfer is
    // omitted. This proves finite-path ownership safety, not loop termination.
    let mut pending = reserve_vec(blocks)?;
    pending.push(entry);
    let mut state = reserve_vec(count)?;
    let mut cursor = 0;
    while cursor < pending.len() {
        budget.charge_work(1)?;
        let position = pending[cursor];
        cursor += 1;
        let block_id = flow.block_id(position).ok_or(ResourceError::Accounting)?;
        let block = definitions
            .block(block_id, budget)?
            .ok_or(ResourceError::Accounting)?;
        let block_location = clone_diagnostic_location_v1(&location, budget)?.at_block(block_id);
        budget.charge_work(count)?;
        state.clear();
        state.extend_from_slice(
            incoming[position]
                .as_ref()
                .ok_or(ResourceError::Accounting)?,
        );
        budget.charge_work(block.operations.len())?;
        for (ordinal, operation) in block.operations.iter().enumerate() {
            let op_location =
                clone_diagnostic_location_v1(&block_location, budget)?.at_operation(ordinal);
            let valid = match &operation.kind {
                OperationKind::Execution(execution) => {
                    apply_execution(function, operation, execution, &slots, &mut state, budget)?
                }
                _ => ordinary_operation(operation, &slots, &state, budget)?,
            };
            if !valid {
                return fail(
                    diagnostics,
                    op_location,
                    "execution operation violates exact producer, acquisition or consumption state",
                    budget,
                );
            }
        }
        let Some(terminator) = &block.terminator else {
            continue;
        };
        if matches!(
            terminator,
            Terminator::Return { .. } | Terminator::Unreachable
        ) {
            budget.charge_work(count)?;
            if state
                .iter()
                .any(|live| !matches!(live, Live::Absent | Live::Context))
            {
                return fail(
                    diagnostics,
                    block_location,
                    "execution scope or descendant remains live at function exit",
                    budget,
                );
            }
        }
        if let Terminator::Return { values } = terminator {
            budget.charge_work(values.len())?;
            for value in values {
                if slot_index(&slots, *value, budget)?.is_some() {
                    return fail(
                        diagnostics,
                        block_location,
                        "execution role cannot be returned",
                        budget,
                    );
                }
            }
        }
        budget.charge_work(blocks.checked_ilog2().map_or(1, |log| log as usize + 2))?;
        let edges = flow
            .outgoing_edges(block_id)
            .ok_or(ResourceError::Accounting)?;
        budget.charge_work(edges.len())?;
        for edge in edges {
            let target = flow.edge_target(edge).ok_or(ResourceError::Accounting)?;
            budget.charge_work(blocks.checked_ilog2().map_or(1, |log| log as usize + 2))?;
            let target = flow
                .block_position(target)
                .ok_or(ResourceError::Accounting)?;
            budget.charge_work(count)?;
            if let Some(expected) = &incoming[target] {
                if expected != &state {
                    return fail(
                        diagnostics,
                        block_location,
                        "execution ownership states differ across a CFG join or backedge",
                        budget,
                    );
                }
            } else {
                let mut next = reserve_vec(count)?;
                next.extend_from_slice(&state);
                incoming[target] = Some(next);
                pending.push(target);
            }
        }
    }
    Ok(())
}

fn ordinary_operation(
    operation: &Operation,
    slots: &[Slot],
    state: &[Live],
    budget: &mut Budget<'_>,
) -> Result<bool, ResourceError> {
    budget.charge_work(operation.results.len())?;
    for result in &operation.results {
        if invalid_execution_type_v15(&result.ty, false, budget)? {
            return Ok(false);
        }
    }
    let mut role_operand = false;
    operation.kind.try_visit_operands(|value| {
        budget.charge_work(1)?;
        role_operand |= slot_index(slots, value, budget)?.is_some();
        Ok::<_, ResourceError>(())
    })?;
    if role_operand {
        return Ok(false);
    }
    if matches!(operation.kind, OperationKind::Call { .. }) {
        // This includes actual Trap/AssertFail operations. Retained callees
        // also lack the initial profile's same-function exit proof.
        budget.charge_work(state.len())?;
        if state
            .iter()
            .any(|live| !matches!(live, Live::Absent | Live::Context))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn result_slot(
    operation: &Operation,
    expected: Role,
    slots: &[Slot],
    state: &[Live],
    budget: &mut Budget<'_>,
) -> Result<Option<usize>, ResourceError> {
    budget.charge_work(1)?;
    let [result] = operation.results.as_slice() else {
        return Ok(None);
    };
    if result.ty != Type::Execution(expected) {
        return Ok(None);
    }
    let Some(index) = slot_index(slots, result.id, budget)? else {
        return Ok(None);
    };
    Ok((state[index] == Live::Absent).then_some(index))
}

fn apply_execution(
    function: &Function,
    operation: &Operation,
    execution: &Execution,
    slots: &[Slot],
    state: &mut [Live],
    budget: &mut Budget<'_>,
) -> Result<bool, ResourceError> {
    let payload_work = match execution {
        Execution::ScopeEnd { discarded, .. } => discarded.len().checked_add(1),
        _ => Some(1),
    }
    .ok_or(ResourceError::Arithmetic)?;
    budget.charge_work(payload_work)?;
    if execution.validate_payload().is_err() {
        return Ok(false);
    }
    match execution {
        Execution::ContextIssue => {
            if function.role != FunctionRole::KernelEntry {
                return Ok(false);
            }
            let Some(result) = result_slot(operation, Role::Context, slots, state, budget)? else {
                return Ok(false);
            };
            state[result] = Live::Context;
        }
        Execution::WorkgroupDerive { context } => {
            let Some(context) = slot_index(slots, *context, budget)? else {
                return Ok(false);
            };
            if state[context] != Live::Context {
                return Ok(false);
            }
            let Some(result) = result_slot(operation, Role::Workgroup, slots, state, budget)?
            else {
                return Ok(false);
            };
            state[context] = Live::Borrowed { workgroup: result };
            state[result] = Live::Workgroup { context };
        }
        Execution::ScopeEnd {
            workgroup,
            discarded,
        } => {
            if !operation.results.is_empty() {
                return Ok(false);
            }
            let Some(workgroup) = slot_index(slots, *workgroup, budget)? else {
                return Ok(false);
            };
            let Live::Workgroup { context } = state[workgroup] else {
                return Ok(false);
            };
            if state[context] != (Live::Borrowed { workgroup }) {
                return Ok(false);
            }
            budget.charge_work(slots.len())?;
            let mut cursor = 0;
            for (index, slot) in slots.iter().enumerate() {
                if state[index] == (Live::Descendant { workgroup }) {
                    if discarded.get(cursor) != Some(&slot.value) {
                        return Ok(false);
                    }
                    cursor += 1;
                }
            }
            if cursor != discarded.len() {
                return Ok(false);
            }
            budget.charge_work(slots.len())?;
            for live in state.iter_mut() {
                if *live == (Live::Descendant { workgroup }) {
                    *live = Live::Absent;
                }
            }
            state[workgroup] = Live::Absent;
            state[context] = Live::Context;
        }
        Execution::MaskedTileLoadU32 {
            workgroup,
            lanes,
            elements,
            ..
        } => {
            let Some(workgroup) = slot_index(slots, *workgroup, budget)? else {
                return Ok(false);
            };
            if !matches!(state[workgroup], Live::Workgroup { .. }) {
                return Ok(false);
            }
            let Some(result) = result_slot(
                operation,
                Role::MaskedTileU32 {
                    lanes: *lanes,
                    elements: *elements,
                },
                slots,
                state,
                budget,
            )?
            else {
                return Ok(false);
            };
            state[result] = Live::Descendant { workgroup };
        }
        Execution::TileIntoFragmentU32 {
            tile,
            lanes,
            elements,
        } => {
            let Some(tile) = slot_index(slots, *tile, budget)? else {
                return Ok(false);
            };
            if slots[tile].role
                != (Role::MaskedTileU32 {
                    lanes: *lanes,
                    elements: *elements,
                })
            {
                return Ok(false);
            }
            let Live::Descendant { workgroup } = state[tile] else {
                return Ok(false);
            };
            let Some(result) = result_slot(
                operation,
                Role::LaneFragmentU32 {
                    lanes: *lanes,
                    elements: *elements,
                },
                slots,
                state,
                budget,
            )?
            else {
                return Ok(false);
            };
            state[tile] = Live::Absent;
            state[result] = Live::Descendant { workgroup };
        }
        Execution::FragmentIntoPartsU32 {
            fragment,
            lanes,
            elements,
        } => {
            let Some(fragment) = slot_index(slots, *fragment, budget)? else {
                return Ok(false);
            };
            if slots[fragment].role
                != (Role::LaneFragmentU32 {
                    lanes: *lanes,
                    elements: *elements,
                })
            {
                return Ok(false);
            }
            if !matches!(state[fragment], Live::Descendant { .. }) {
                return Ok(false);
            }
            state[fragment] = Live::Absent;
        }
    }
    Ok(true)
}

#[cfg(test)]
#[path = "verification_execution_lifecycle_v15_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "verification_execution_lifecycle_v15_loop_tests.rs"]
mod loop_tests;
