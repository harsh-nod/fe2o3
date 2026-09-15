//! Exact finite boolean identity bodies, not names or externally supplied summaries.

use std::collections::BTreeMap;

use super::{SemanticOptionDominanceErrorV1, WorkBudgetV1};
use crate::semantic_mir_v1::*;

const MAX_IDENTITY_LOCALS: usize = 16;
const MAX_IDENTITY_STATEMENTS: usize = 32;
const MAX_IDENTITY_BLOCKS: usize = 8;

pub(super) enum ArithmeticCallTransferV1<'a> {
    Returned(&'a SemanticOperandV1),
    Preserved,
    Unknown,
}

pub(super) struct ArithmeticIdentityCallsV1<'a> {
    function: &'a SemanticFunctionDeclV1,
    calls: Vec<Option<&'a SemanticDirectCallV1>>,
}

impl<'a> ArithmeticIdentityCallsV1<'a> {
    pub(super) fn analyze(
        types: &[SemanticTypeDeclV1],
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        function: &'a SemanticFunctionDeclV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<Self, SemanticOptionDominanceErrorV1> {
        budget.charge(function.blocks().len())?;
        let mut calls = Vec::new();
        calls
            .try_reserve_exact(function.blocks().len())
            .map_err(|_| SemanticOptionDominanceErrorV1::Storage)?;
        let mut summaries = BTreeMap::new();
        for block in function.blocks() {
            calls.push(None);
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            let Some(SemanticCallableDeclV1::Defined {
                function: callee_id,
            }) = callables.get(call.callee().index() as usize)
            else {
                continue;
            };
            let Some(callee) = functions.get(callee_id.index() as usize) else {
                continue;
            };
            budget.charge(1)?;
            let exact = match summaries.get(callee_id) {
                Some(exact) => *exact,
                None => {
                    let exact = exact_boolean_identity(types, callables, callee, budget)?;
                    summaries.insert(*callee_id, exact);
                    exact
                }
            };
            let Some(destination) = call.destination() else {
                continue;
            };
            let [operand] = call.arguments() else {
                continue;
            };
            if exact
                && operand.ty() == callee.abi().source_input_types()[0]
                && destination.place().ty() == operand.ty()
                && destination.place().projections().is_empty()
                && function
                    .locals()
                    .get(destination.place().local().index() as usize)
                    .is_some_and(|local| local.ty() == operand.ty())
                && call.unwind() == SemanticUnwindActionV1::Unreachable
                && call.variadic_argument_abis().is_empty()
                && destination.edge().role() == SemanticEdgeRoleV1::CallReturn
            {
                *calls.last_mut().unwrap() = Some(call);
            }
        }
        Ok(Self { function, calls })
    }

    pub(super) fn transfer(
        &self,
        function: &SemanticFunctionDeclV1,
        predecessor: usize,
        target: usize,
        local: usize,
        ty: SemanticTypeIdV1,
        overflow_field: bool,
    ) -> ArithmeticCallTransferV1<'a> {
        if !std::ptr::eq(function, self.function) {
            return ArithmeticCallTransferV1::Unknown;
        }
        let Some(call) = self.calls.get(predecessor).copied().flatten() else {
            return ArithmeticCallTransferV1::Unknown;
        };
        let destination = call.destination().unwrap();
        if destination.edge().target().index() as usize != target {
            return ArithmeticCallTransferV1::Unknown;
        }
        let operand = &call.arguments()[0];
        if destination.place().local().index() as usize == local {
            return if !overflow_field && ty == operand.ty() {
                ArithmeticCallTransferV1::Returned(operand)
            } else {
                ArithmeticCallTransferV1::Unknown
            };
        }
        if matches!(operand, SemanticOperandV1::Move(place) if place.local().index() as usize == local)
        {
            return ArithmeticCallTransferV1::Unknown;
        }
        ArithmeticCallTransferV1::Preserved
    }
}

fn exact_boolean_identity(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    budget: &mut WorkBudgetV1,
) -> Result<bool, SemanticOptionDominanceErrorV1> {
    let abi = function.abi();
    let [ty] = abi.source_input_types() else {
        return Ok(false);
    };
    let [argument] = abi.arguments() else {
        return Ok(false);
    };
    if function.role() != SemanticFunctionRoleV1::InternalHelper
        || function.kernel_entry().is_some()
        || function.entry().index() as usize >= function.blocks().len()
        || function.locals().len() > MAX_IDENTITY_LOCALS
        || function.blocks().len() > MAX_IDENTITY_BLOCKS
        || !matches!(
            types
                .get(ty.index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
        )
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.can_unwind()
        || abi.c_variadic()
        || abi.fixed_count() != 1
        || abi.source_output_type() != *ty
        || abi.source_argument_ownership() != [SemanticSourceArgumentOwnershipV1::ByValue]
        || argument.role() != SemanticAbiArgumentRoleV1::Source
        || ![argument.value(), abi.return_value()]
            .into_iter()
            .all(|value| {
                value.source_ty() == *ty
                    && value.adjusted().is_none()
                    && value.pointee_override().is_none()
                    && matches!(value.mode(), SemanticAbiPassModeV1::Direct(_))
            })
    {
        return Ok(false);
    }
    budget.charge(
        function
            .locals()
            .len()
            .saturating_add(function.blocks().len()),
    )?;
    let mut input = None;
    let mut output = None;
    for (index, local) in function.locals().iter().enumerate() {
        if local.ty() != *ty
            && !matches!(
                types
                    .get(local.ty().index() as usize)
                    .map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Unit)
            )
        {
            return Ok(false);
        }
        match local.role() {
            SemanticLocalRoleV1::Argument(0)
                if local.ty() == *ty && input.replace(index).is_none() =>
            {
                ()
            }
            SemanticLocalRoleV1::Return if local.ty() == *ty && output.replace(index).is_none() => {
                ()
            }
            SemanticLocalRoleV1::Temporary => (),
            _ => return Ok(false),
        }
    }
    if input.is_none() || output.is_none() {
        return Ok(false);
    }
    // Validate all retained blocks, including unreachable ones. A summary may
    // not hide unknown calls, memory effects, traps, or unsupported statements.
    for block in function.blocks() {
        if block.statements().len() > MAX_IDENTITY_STATEMENTS {
            return Ok(false);
        }
        budget.charge(block.statements().len().saturating_add(1))?;
        for statement in block.statements() {
            let exact = match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    exact_local(function, assignment.destination())
                        && assignment.value().result_type() == assignment.destination().ty()
                        && matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(operand)
                            if operand.ty() == assignment.destination().ty() && exact_operand(types, function, operand))
                }
                SemanticStatementKindV1::StorageLive(local)
                | SemanticStatementKindV1::StorageDead(local) => {
                    (local.index() as usize) < function.locals().len()
                }
                SemanticStatementKindV1::Nop => true,
                _ => false,
            };
            if !exact {
                return Ok(false);
            }
        }
        let edge_valid = |edge: SemanticControlFlowEdgeV1| {
            (edge.target().index() as usize) < function.blocks().len()
        };
        let exact = match block.terminator().kind() {
            SemanticTerminatorKindV1::Return => true,
            SemanticTerminatorKindV1::Goto(edge) => {
                edge.role() == SemanticEdgeRoleV1::Goto && edge_valid(*edge)
            }
            SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } => {
                discriminant.ty() == *ty
                    && exact_operand(types, function, discriminant)
                    && targets.values().len() <= 2
                    && targets.values().iter().all(|target| {
                        target.value() <= 1
                            && target.edge().role() == SemanticEdgeRoleV1::SwitchValue
                            && edge_valid(target.edge())
                    })
                    && targets.otherwise().role() == SemanticEdgeRoleV1::SwitchOtherwise
                    && edge_valid(targets.otherwise())
            }
            SemanticTerminatorKindV1::Call(call) => {
                exact_cold_path(types, callables, function, call)
                    && edge_valid(call.destination().unwrap().edge())
            }
            _ => false,
        };
        if !exact {
            return Ok(false);
        }
    }
    for flag in [false, true] {
        if evaluate(function, input.unwrap(), output.unwrap(), flag, budget)? != Some(flag) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn exact_local(function: &SemanticFunctionDeclV1, place: &SemanticPlaceV1) -> bool {
    place.projections().is_empty()
        && function
            .locals()
            .get(place.local().index() as usize)
            .is_some_and(|local| local.ty() == place.ty())
}

fn exact_operand(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    operand: &SemanticOperandV1,
) -> bool {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            exact_local(function, place)
        }
        SemanticOperandV1::Constant(constant) => match (
            types
                .get(constant.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape),
            constant.value(),
        ) {
            (Some(SemanticTypeShapeV1::Unit), SemanticConstantValueV1::ZeroSized) => true,
            (
                Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)),
                SemanticConstantValueV1::Scalar(value),
            ) => value.size_bytes() == 1 && value.bits() <= 1,
            _ => false,
        },
    }
}

fn exact_cold_path(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    call: &SemanticDirectCallV1,
) -> bool {
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        binding,
        operation: SemanticCompilerIntrinsicOperationV1::ColdPath,
        ..
    }) = callables.get(call.callee().index() as usize)
    else {
        return false;
    };
    let Some(destination) = call.destination() else {
        return false;
    };
    let abi = binding.abi();
    call.arguments().is_empty()
        && call.variadic_argument_abis().is_empty()
        && call.unwind() == SemanticUnwindActionV1::Unreachable
        && destination.edge().role() == SemanticEdgeRoleV1::CallReturn
        && exact_local(function, destination.place())
        && abi.source_input_types().is_empty()
        && abi.arguments().is_empty()
        && abi.source_argument_ownership().is_empty()
        && abi.fixed_count() == 0
        && !abi.can_unwind()
        && !abi.c_variadic()
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.source_output_type() == destination.place().ty()
        && abi.return_value().source_ty() == destination.place().ty()
        && abi.return_value().adjusted().is_none()
        && abi.return_value().pointee_override().is_none()
        && matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
        && matches!(
            types
                .get(destination.place().ty().index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Unit)
        )
}

#[derive(Clone, Copy)]
enum ValueV1 {
    Uninitialized,
    Boolean(bool),
    Unit,
}

fn read(
    values: &mut [ValueV1; MAX_IDENTITY_LOCALS],
    operand: &SemanticOperandV1,
) -> Option<ValueV1> {
    let value = match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            let value = values[place.local().index() as usize];
            if matches!(operand, SemanticOperandV1::Move(_)) {
                values[place.local().index() as usize] = ValueV1::Uninitialized;
            }
            value
        }
        SemanticOperandV1::Constant(constant) => match constant.value() {
            SemanticConstantValueV1::Scalar(value) => ValueV1::Boolean(value.bits() != 0),
            SemanticConstantValueV1::ZeroSized => ValueV1::Unit,
            _ => return None,
        },
    };
    (!matches!(value, ValueV1::Uninitialized)).then_some(value)
}

fn evaluate(
    function: &SemanticFunctionDeclV1,
    input: usize,
    output: usize,
    flag: bool,
    budget: &mut WorkBudgetV1,
) -> Result<Option<bool>, SemanticOptionDominanceErrorV1> {
    let mut values = [ValueV1::Uninitialized; MAX_IDENTITY_LOCALS];
    values[input] = ValueV1::Boolean(flag);
    let mut visited = [false; MAX_IDENTITY_BLOCKS];
    let mut block = function.entry().index() as usize;
    loop {
        budget.charge(1)?;
        if std::mem::replace(&mut visited[block], true) {
            return Ok(None);
        }
        let data = &function.blocks()[block];
        budget.charge(data.statements().len())?;
        for statement in data.statements() {
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
                        return Ok(None);
                    };
                    let Some(value) = read(&mut values, operand) else {
                        return Ok(None);
                    };
                    values[assignment.destination().local().index() as usize] = value;
                }
                SemanticStatementKindV1::StorageLive(local)
                | SemanticStatementKindV1::StorageDead(local) => {
                    values[local.index() as usize] = ValueV1::Uninitialized
                }
                SemanticStatementKindV1::Nop => (),
                _ => return Ok(None),
            }
        }
        block = match data.terminator().kind() {
            SemanticTerminatorKindV1::Return => {
                return Ok(match values[output] {
                    ValueV1::Boolean(value) => Some(value),
                    _ => None,
                });
            }
            SemanticTerminatorKindV1::Goto(edge) => edge.target().index() as usize,
            SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } => {
                let Some(ValueV1::Boolean(value)) = read(&mut values, discriminant) else {
                    return Ok(None);
                };
                targets
                    .values()
                    .iter()
                    .find(|target| target.value() == u128::from(value))
                    .map_or(targets.otherwise(), |target| target.edge())
                    .target()
                    .index() as usize
            }
            SemanticTerminatorKindV1::Call(call) => {
                let destination = call.destination().unwrap();
                values[destination.place().local().index() as usize] = ValueV1::Unit;
                destination.edge().target().index() as usize
            }
            _ => return Ok(None),
        };
    }
}

#[cfg(test)]
mod tests;
