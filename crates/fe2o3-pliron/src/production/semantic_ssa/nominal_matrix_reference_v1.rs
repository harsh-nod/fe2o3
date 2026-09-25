//! Exact BF16 helper reference nomination, NOT an issuance/convergence proof.
//! Source declarations alone do not nominate: actual call occurrences, ABI,
//! root producer/borrow and helper receiver must join. The caller still applies
//! its unchanged ALL-use reference escape/consumer analysis before closing.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiArgumentRoleV1, SemanticCanonAbiV1, SemanticExternAbiV1,
    SemanticMfmaAccumulatorDistributionV1, SemanticMfmaOperandRoleV1, SemanticMfmaProfileV1,
    SemanticMfmaRegisterDistributionV1, SemanticScalarTypeV1,
};
const BLOCKS: usize = 32;
const ITEMS: usize = 4096;
const SCRATCH_WORDS: usize = 128;

struct Source<'a> {
    types: &'a [SemanticTypeDeclV1],
    functions: &'a [SemanticFunctionDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    roots: &'a [SemanticFunctionIdV1],
}
#[derive(Clone, Copy)]
struct Call<'a> {
    block: usize,
    call: &'a fe2o3_mir_model::semantic_mir_v1::SemanticDirectCallV1,
    binding: &'a fe2o3_mir_model::semantic_mir_v1::SemanticNonBodyCallableBindingV1,
    operation: &'a SemanticCompilerIntrinsicOperationV1,
}
// Fixed logical borrowed scratch only, not RSS or compiler stack size.
const _: () = assert!(
    std::mem::size_of::<Source<'static>>()
        + 8 * std::mem::size_of::<Option<Call<'static>>>()
        + 32 * std::mem::size_of::<usize>()
        <= SCRATCH_WORDS * std::mem::size_of::<usize>()
);

pub(super) fn nominates(
    semantic: &AdmittedInertSemanticMirV1,
    helper: SemanticFunctionIdV1,
    ordinal: u32,
    local: u32,
    reference_type: SemanticTypeIdV1,
    meter: &mut Meter<'_>,
) -> Result<bool, Error> {
    // Prepay even a rejected probe. No type scan, allocation or borrowed row
    // construction occurs before this fixed check debit.
    meter.work(128)?;
    if ordinal != 0 || semantic.functions().len() != 2 || semantic.roots().len() != 1 {
        return Ok(false);
    }
    meter.storage(SCRATCH_WORDS)?;
    let view = Source {
        types: semantic.types(),
        functions: semantic.functions(),
        callables: semantic.callables(),
        roots: semantic.roots(),
    };
    nominate(&view, helper, local, reference_type, meter)
}
fn whole(operand: &SemanticOperandV1) -> Option<&SemanticPlaceV1> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() =>
        {
            Some(place)
        }
        _ => None,
    }
}
fn closed_abi(function: &SemanticFunctionDeclV1, reference: SemanticTypeIdV1) -> bool {
    use SemanticSourceArgumentOwnershipV1::{ByValue, SharedBorrow};
    let abi = function.abi();
    // hidden_arguments() scans the adjusted argument prefix, so establish its
    // exact four-row bound before invoking it under the fixed header prepay.
    abi.source_input_types().len() == 4
        && abi.arguments().len() == 4
        && !abi.can_unwind()
        && !abi.c_variadic()
        && abi.hidden_arguments().is_empty()
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.fixed_count() == 4
        && abi.source_input_types()[0] == reference
        && abi.source_argument_ownership() == [SharedBorrow, ByValue, ByValue, ByValue]
        && abi.arguments().iter().enumerate().all(|(i, a)| {
            a.role() == SemanticAbiArgumentRoleV1::Source
                && a.ty() == abi.source_input_types()[i]
                && a.value().adjusted().is_none()
                && a.value().pointee_override().is_none()
        })
        && abi.return_value().adjusted().is_none()
        && abi.return_value().pointee_override().is_none()
}
fn array_f32(source: &Source<'_>, ty: SemanticTypeIdV1) -> bool {
    let Some(SemanticTypeShapeV1::Array { element, length: 4 }) = source
        .types
        .get(ty.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return false;
    };
    matches!(
        source
            .types
            .get(element.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float {
            bits: 32
        }))
    )
}
fn destination(
    call: &fe2o3_mir_model::semantic_mir_v1::SemanticDirectCallV1,
) -> Option<&SemanticPlaceV1> {
    let dest = call.destination()?.place();
    (dest.projections().is_empty()
        && matches!(
            call.unwind(),
            fe2o3_mir_model::semantic_mir_v1::SemanticUnwindActionV1::Unreachable
        ))
    .then_some(dest)
}
fn exact_mfma(
    call: Call<'_>,
    context: SemanticTypeIdV1,
    helper: &SemanticFunctionDeclV1,
    receiver: u32,
) -> bool {
    let SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
        context: actual,
        lhs_fragment,
        rhs_fragment,
        accumulator_fragment,
        lhs,
        rhs,
        accumulator,
    } = call.operation
    else {
        return false;
    };
    if *actual != context
        || lhs.role != SemanticMfmaOperandRoleV1::A
        || rhs.role != SemanticMfmaOperandRoleV1::B
        || lhs.profile != SemanticMfmaProfileV1::Bf16F32M16N16K16
        || rhs.profile != lhs.profile
        || accumulator.profile != lhs.profile
        || lhs.wave_width != 64
        || rhs.wave_width != 64
        || accumulator.wave_width != 64
        || lhs.register_distribution != SemanticMfmaRegisterDistributionV1::Tile16x16
        || rhs.register_distribution != lhs.register_distribution
        || accumulator.distribution != SemanticMfmaAccumulatorDistributionV1::RowMajor
    {
        return false;
    }
    let inputs = helper.abi().source_input_types();
    if inputs[1..] != [*lhs_fragment, *rhs_fragment, *accumulator_fragment]
        || call.binding.abi().source_input_types() != inputs
        || call.call.arguments().len() != 4
    {
        return false;
    }
    let Some(first) = whole(&call.call.arguments()[0]) else {
        return false;
    };
    first.local().index() == receiver
        && first.ty() == inputs[0]
        && call
            .call
            .arguments()
            .iter()
            .zip(inputs)
            .all(|(arg, ty)| arg.ty() == *ty)
        && destination(call.call).is_some_and(|p| p.ty() == *accumulator_fragment)
}
fn nominate(
    source: &Source<'_>,
    helper_id: SemanticFunctionIdV1,
    local: u32,
    reference: SemanticTypeIdV1,
    meter: &mut Meter<'_>,
) -> Result<bool, Error> {
    // Closed nomination scope, not a new general source-function bound.
    if source.functions.len() != 2
        || source.roots.len() != 1
        || source.types.len() > ITEMS
        || source.callables.len() > ITEMS
    {
        return Ok(false);
    }
    let root_id = source.roots[0];
    if root_id == helper_id {
        return Ok(false);
    }
    let Some(root) = source.functions.get(root_id.index() as usize) else {
        return Ok(false);
    };
    let Some(helper) = source.functions.get(helper_id.index() as usize) else {
        return Ok(false);
    };
    let Some(SemanticTypeShapeV1::Pointer(pointer)) = source
        .types
        .get(reference.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Ok(false);
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.mutability() != SemanticMutabilityV1::Immutable
        || root.role() != SemanticFunctionRoleV1::KernelRoot
        || root.kernel_entry().is_none()
        || helper.role() != SemanticFunctionRoleV1::InternalHelper
        || helper.export().is_some()
        || !closed_abi(helper, reference)
        || !helper
            .locals()
            .get(local as usize)
            .is_some_and(|d| d.ty() == reference && d.role() == SemanticLocalRoleV1::Argument(0))
        || !source
            .types
            .get(pointer.pointee().index() as usize)
            .is_some_and(|t| t.rust_type_kind() == SemanticRustTypeKindV1::Ordinary)
    {
        return Ok(false);
    }
    let context = pointer.pointee();
    // Account the fixed borrowed scan state and bounded length reads first.
    // Meter storage is an accumulated planning report; no refund/reset occurs.
    meter.work(2 + BLOCKS)?;
    let mut blocks = 0usize;
    let mut items = 0usize;
    for function in source.functions {
        blocks = sum(blocks, function.blocks().len())?;
        items = sum(items, function.locals().len())?;
        if blocks > BLOCKS || items > ITEMS {
            return Ok(false);
        }
        for block in function.blocks() {
            items = sum(items, block.statements().len())?;
            if items > ITEMS {
                return Ok(false);
            }
        }
    }
    // Every later field/shape/call/statement visit fits <=64 work units per
    // item/block, including the root's second assignment-only pass.
    meter.work(sum(512, product(sum(items, blocks)?, 64)?)?)?;
    let mut current = None;
    let mut mfma = None;
    let mut values = None;
    let mut direct = None;
    for (fi, function) in source.functions.iter().enumerate() {
        let id = SemanticFunctionIdV1::from_index(fi as u32);
        for (bi, block) in function.blocks().iter().enumerate() {
            match block.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => {
                    let Some(callee) = source.callables.get(call.callee().index() as usize) else {
                        return Ok(false);
                    };
                    match callee {
                        SemanticCallableDeclV1::Defined { function } => {
                            if id != root_id
                                || *function != helper_id
                                || direct.replace(call).is_some()
                            {
                                return Ok(false);
                            }
                        }
                        SemanticCallableDeclV1::CompilerIntrinsic {
                            operation, binding, ..
                        } => {
                            let row = Call {
                                block: bi,
                                call,
                                binding,
                                operation,
                            };
                            match operation {
                                SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent { context: actual } => {
                                    if id != root_id || *actual != context || current.replace(row).is_some() {
                                        return Ok(false);
                                    }
                                }
                                SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate { .. } => {
                                    if id != helper_id || mfma.replace(row).is_some() { return Ok(false); }
                                }
                                SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues { .. } => {
                                    if id != helper_id || values.replace(row).is_some() { return Ok(false); }
                                }
                                _ if id == helper_id => return Ok(false),
                                _ => {}
                            }
                        }
                        _ => return Ok(false),
                    }
                }
                SemanticTerminatorKindV1::TailCall(_) => return Ok(false),
                _ => {}
            }
        }
    }
    let (Some(current), Some(mfma), Some(values), Some(direct)) = (current, mfma, values, direct)
    else {
        return Ok(false);
    };
    if !exact_mfma(mfma, context, helper, local)
        || current.binding.abi().source_output_type() != context
        || !current.call.arguments().is_empty()
        || direct.arguments().len() != 4
        || destination(direct).is_none()
        || !direct
            .arguments()
            .iter()
            .zip(helper.abi().source_input_types())
            .all(|(a, t)| a.ty() == *t)
    {
        return Ok(false);
    }
    let Some(current_dest) = destination(current.call) else {
        return Ok(false);
    };
    let Some(root_receiver) = whole(&direct.arguments()[0]) else {
        return Ok(false);
    };
    if current_dest.ty() != context || root_receiver.ty() != reference {
        return Ok(false);
    }
    let SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
        fragment,
        values: output,
    } = values.operation
    else {
        return Ok(false);
    };
    let Some(mfma_dest) = destination(mfma.call) else {
        return Ok(false);
    };
    let Some(values_dest) = destination(values.call) else {
        return Ok(false);
    };
    if values.call.arguments().len() != 1
        || mfma.block == values.block
        || mfma_dest.ty() != *fragment
        || values_dest.ty() != *output
        || helper.abi().source_output_type() != *output
        || !array_f32(source, *output)
        || destination(direct).is_none_or(|d| d.ty() != *output)
        || !whole(&values.call.arguments()[0])
            .is_some_and(|p| p.local() == mfma_dest.local() && p.ty() == *fragment)
    {
        return Ok(false);
    }
    // Exact actual root producer -> whole shared borrow -> sole helper receiver.
    // No source SSA is invented here. More complex copies/reborrows remain
    // unavailable for this nomination; the later checked relation is separate.
    let mut borrowed = false;
    let mut context_assignments = 0usize;
    let mut receiver_assignments = 0usize;
    for block in root.blocks() {
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(dest) = call.destination()
            && (dest.place().local() == root_receiver.local()
                || (dest.place().local() == current_dest.local()
                    && !std::ptr::eq(call, current.call)))
        {
            return Ok(false);
        }
        for statement in block.statements() {
            if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                if assignment.destination().local() == current_dest.local() {
                    context_assignments += 1;
                }
                if assignment.destination().local() == root_receiver.local() {
                    receiver_assignments += 1;
                    borrowed = assignment.destination().projections().is_empty()
                        && assignment.destination().ty() == reference
                        && matches!(assignment.value().kind(), SemanticRvalueKindV1::Borrow {
                            kind: fe2o3_mir_model::semantic_mir_v1::SemanticBorrowKindV1::Shared, place
                        } if place.projections().is_empty() && place.local() == current_dest.local()
                            && place.ty() == context);
                }
            }
            // Generic address/escape analysis below, not this nomination,
            // decides whether any remaining uses are storage-observing.
        }
    }
    Ok(borrowed && receiver_assignments == 1 && context_assignments == 0)
}
#[cfg(test)]
#[path = "nominal_matrix_reference_v1_tests.rs"]
mod tests;
