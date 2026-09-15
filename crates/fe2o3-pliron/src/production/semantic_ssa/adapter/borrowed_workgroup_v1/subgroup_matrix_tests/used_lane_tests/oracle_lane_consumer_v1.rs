//! Closed lane leaf uses, not an issuer or a lifetime proof.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticDirectCallV1, SemanticSourceArgumentOwnershipV1};

/// Inspect only the selected call. MIR admission still owns each intrinsic's
/// complete numerical/layout contract and authenticated source binding.
fn leaf(
    call: &SemanticDirectCallV1,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    budget: &mut Budget,
) -> Result<Option<(usize, SemanticTypeIdV1, SemanticTypeIdV1)>, ProductionSemanticSsaErrorV1> {
    budget.charge(1)?;
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        binding, operation, ..
    }) = callables.get(call.callee().index() as usize)
    else {
        return Ok(None);
    };
    let (argument, lane, output, count) = match operation {
        SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero {
            lane, fragment, ..
        } => (0, *lane, *fragment, 1),
        SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoad {
            lane,
            option_fragment,
            ..
        } => (1, *lane, *option_fragment, 4),
        SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 {
            lane, fragment, ..
        }
        | SemanticCompilerIntrinsicOperationV1::Gfx950Fp4MatrixLoadM16K128 {
            lane, fragment, ..
        }
        | SemanticCompilerIntrinsicOperationV1::Gfx950Fp8MatrixLoadM16K128 {
            lane, fragment, ..
        } => (1, *lane, *fragment, 4),
        SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { .. } => {
            budget.charge(32)?;
            let Some(fact) =
                GlobalBf16BorrowV1::for_callable(types, &callables[call.callee().index() as usize])
            else {
                return Ok(None);
            };
            let (reference, owned) = fact.pairs()[1];
            return Ok(fact
                .accepts(call, 1, owned)
                .then_some((1, reference, owned)));
        }
        _ => return Ok(None),
    };
    // All selected operations have at most four source arguments. Check lengths
    // before scanning, and charge the actual scans and fixed pointer checks.
    budget.charge(12)?;
    let abi = binding.abi();
    if abi.c_variadic()
        || !abi.hidden_arguments().is_empty()
        || !call.variadic_argument_abis().is_empty()
        || abi.source_input_types().len() != count
        || call.arguments().len() != count
        || abi.source_argument_ownership().len() != count
        || abi.source_output_type() != output
        || abi.source_argument_ownership()[argument]
            != SemanticSourceArgumentOwnershipV1::SharedBorrow
        || call
            .destination()
            .is_none_or(|d| !d.place().projections().is_empty() || d.place().ty() != output)
    {
        return Ok(None);
    }
    budget.charge(count.saturating_mul(2))?;
    if !call
        .arguments()
        .iter()
        .map(SemanticOperandV1::ty)
        .eq(abi.source_input_types().iter().copied())
    {
        return Ok(None);
    }
    let reference = abi.source_input_types()[argument];
    Ok(
        (matrix_access_borrow::shared_pointee(types, reference) == Some(lane))
            .then_some((argument, reference, lane)),
    )
}

pub(super) fn terminator(
    terminator: &SemanticTerminatorKindV1,
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    by_reference: &BTreeMap<u32, usize>,
    candidates: &mut [SemanticBorrowCandidateV1],
    budget: &mut Budget,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    let SemanticTerminatorKindV1::Call(call) = terminator else {
        validate_reference_uses_in_terminator_v1(terminator, &[], by_reference, candidates);
        return Ok(());
    };
    let mut accepted = None;
    if let Some(destination) = call.destination() {
        invalidate_reference_place_v1(destination.place(), by_reference, candidates);
    }
    for (argument, operand) in call.arguments().iter().enumerate() {
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
            continue;
        };
        if !place.projections().is_empty() {
            invalidate_reference_place_v1(place, by_reference, candidates);
            continue;
        }
        let Some(&index) = by_reference.get(&place.local().index()) else {
            continue;
        };
        // Most calls do not use this leaf. Validate only a tracked use, once
        // per call, without rescanning unrelated callables or refunding work.
        let accepted = match accepted {
            Some(checked) => checked,
            None => {
                let checked = leaf(call, types, callables, budget)?;
                accepted = Some(checked);
                checked
            }
        };
        if accepted == Some((argument, place.ty(), candidates[index].source_type))
            && function
                .locals()
                .get(place.local().index() as usize)
                .is_some_and(|local| local.ty() == place.ty())
        {
            candidates[index].consumers = candidates[index].consumers.saturating_add(1);
            candidates[index].intrinsic_consumer = true;
        } else {
            candidates[index].valid = false;
        }
    }
    Ok(())
}
