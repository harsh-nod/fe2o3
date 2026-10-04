//! Exact inert MIR36 packing and source-tail grammar. This is not source custody,
//! complete CFG checking, an instruction interpreter or artifact authority.
//! The sole source lowerer independently checks the real model and transport.
use super::*;

pub(super) const VALIDATION_WORK: usize = 512;

pub(super) fn uses_v36(request: &InertSemanticMirRequestV1) -> bool {
    request.callables.iter().any(|callable| {
        matches!(
            callable,
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Gfx942CompleteBody(_),
                ..
            }
        )
    }) || request.functions.iter().any(|function| {
        function.blocks.iter().any(|block| {
            matches!(&block.terminator.kind, SemanticTerminatorKindV1::Call(call)
                if call.complete_body_source_vnext.is_some())
        })
    })
}

/// Fixed packing grammar only. All eight blocks and sixteen instruction slots
/// are checked even if inactive. Structural CFG/initialization is NOT inferred.
pub(super) fn validate_packing(
    packed: SemanticCompleteBodyPackingVNext,
) -> Result<(), SemanticMirErrorV1> {
    let refuse = || SemanticMirErrorV1::InvalidCompleteBodyV36;
    if !(1..=8).contains(&packed.block_count) || !(1..=16).contains(&packed.instruction_count) {
        return Err(refuse());
    }
    let mut total = 0usize;
    for index in 0..8 {
        let word = (packed.block_words[index / 2] >> (32 * (index % 2))) as u32;
        if index >= usize::from(packed.block_count) {
            if word != 0 {
                return Err(refuse());
            }
            continue;
        }
        let count = ((word >> 8) & 31) as usize;
        let tag = (word >> 13) & 3;
        let first = (word >> 15) & 255;
        let second = (word >> 23) & 255;
        if word & 0x8000_0000 != 0
            || count > 16
            || tag == 3
            || (tag == 0 && second != 0)
            || (tag == 2 && (first != 0 || second != 0))
        {
            return Err(refuse());
        }
        total += count; // at most 8 * 16
        if total > 16 {
            return Err(refuse());
        }
    }
    if total != usize::from(packed.instruction_count) {
        return Err(refuse());
    }
    for index in 0..16 {
        let word = (packed.instruction_words[index / 4] >> (16 * (index % 4))) as u16;
        if index >= usize::from(packed.instruction_count) {
            if word != 0 {
                return Err(refuse());
            }
            continue;
        }
        // Same closed arithmetic descriptor grammar; no evaluation or role
        // initialization claim. Actual canonical roles are independently checked.
        let opcode = word & 7;
        let left = (word >> 4) & 7;
        let right = (word >> 7) & 7;
        if word & 0xfc00 != 0
            || opcode > 5
            || left > 4
            || (opcode == 0 && right != 0)
            || (opcode != 0 && right > 4)
        {
            return Err(refuse());
        }
    }
    Ok(())
}

pub(super) fn signature_matches(
    request: &InertSemanticMirRequestV1,
    abi: &SemanticFunctionAbiV1,
) -> bool {
    let inputs = abi.source_input_types();
    if abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.c_variadic()
        || abi.can_unwind()
        || inputs.len() != 10
        || abi.arguments().len() != 10
        || !request
            .types
            .get(abi.source_output_type().0 as usize)
            .is_some_and(|ty| matches!(ty.shape, SemanticTypeShapeV1::Unit))
        || !matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
    {
        return false;
    }
    if inputs
        .iter()
        .any(|input| request.types.get(input.0 as usize).is_none())
        || !inputs[1..5]
            .iter()
            .all(|input| is_unsigned_integer_with_bits(request, *input, 32))
        || !inputs[5..]
            .iter()
            .all(|input| is_unsigned_integer_with_bits(request, *input, 8))
        || !matches!(
            abi.arguments()[0].value().mode(),
            SemanticAbiPassModeV1::Pair { .. }
        )
        || !abi.arguments()[1..]
            .iter()
            .all(|arg| matches!(arg.value().mode(), SemanticAbiPassModeV1::Direct(_)))
    {
        return false;
    }
    // Pair layout/type validity is checked by ordinary ABI admission. Exact
    // trusted DisjointSlice<u32>, alignment and source FnAbi belong to the live
    // source producer, never to these inert shape observations alone.
    true
}

pub(super) fn validate_call_source(
    request: &InertSemanticMirRequestV1,
    function: &SemanticFunctionDeclV1,
    location: SemanticMirLocationV1,
    call: &SemanticDirectCallV1,
) -> Result<(), SemanticMirErrorV1> {
    let callee = request.callables.get(call.callee.0 as usize);
    let body = matches!(
        callee,
        Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::Gfx942CompleteBody(_),
            ..
        })
    );
    if !body && call.complete_body_source_vnext.is_none() {
        return Ok(());
    }
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        operation: SemanticCompilerIntrinsicOperationV1::Gfx942CompleteBody(packed),
        binding,
        ..
    }) = callee
    else {
        return Err(SemanticMirErrorV1::InvalidCompleteBodyV36);
    };
    validate_packing(*packed)?;
    let source = call
        .complete_body_source_vnext
        .ok_or(SemanticMirErrorV1::InvalidCompleteBodyV36)?;
    if !source.matches_function(function)
        || !signature_matches(request, &binding.abi)
        || call.arguments.len() != 10
        || !call.variadic_argument_abis.is_empty()
        || call.inline_assembly_source_v30.is_some()
        || call.ordered_region_source_v31.is_some()
        || call.ordered_program_source_v32.is_some()
        || !matches!(
            call.unwind,
            SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
        )
    {
        return Err(SemanticMirErrorV1::InvalidCompleteBodyV36);
    }
    let SemanticMirLocationV1::Terminator { block, .. } = location else {
        return Err(SemanticMirErrorV1::InvalidCompleteBodyV36);
    };
    let Some(block_decl) = function.blocks.get(block.0 as usize) else {
        return Err(SemanticMirErrorV1::InvalidCompleteBodyV36);
    };
    // Raw rustc coordinates and identity-sorted semantic indices are distinct.
    // The live producer joins the exact raw->semantic map; inert admission
    // bounds the raw coordinate and requires the full current block identity.
    if source.raw_block() as usize >= function.blocks.len()
        || source.block_identity() != *block_decl.identity.as_bytes()
    {
        return Err(SemanticMirErrorV1::InvalidCompleteBodyV36);
    }
    let mut registers = [0u8; 5];
    for (index, operand) in call.arguments.iter().enumerate() {
        if operand.ty() != binding.abi.source_input_types()[index] {
            return Err(SemanticMirErrorV1::InvalidCompleteBodyV36);
        }
        if index < 5 {
            let (place, moved) = match operand {
                SemanticOperandV1::Copy(place) => (place, false),
                SemanticOperandV1::Move(place) => (place, true),
                _ => return Err(SemanticMirErrorV1::InvalidCompleteBodyV36),
            };
            if !place.projections.is_empty() || (index == 0 && !moved) {
                return Err(SemanticMirErrorV1::InvalidCompleteBodyV36);
            }
        } else {
            let SemanticOperandV1::Constant(SemanticConstantV1 {
                value: SemanticConstantValueV1::Scalar(value),
                ..
            }) = operand
            else {
                return Err(SemanticMirErrorV1::InvalidCompleteBodyV36);
            };
            let physical = u8::try_from(value.bits())
                .map_err(|_| SemanticMirErrorV1::InvalidCompleteBodyV36)?;
            if value.size_bytes() != 1
                || !(8..=63).contains(&physical)
                || registers[..index - 5].contains(&physical)
            {
                return Err(SemanticMirErrorV1::InvalidCompleteBodyV36);
            }
            registers[index - 5] = physical;
        }
    }
    Ok(())
}

pub(super) fn encode_packing(
    writer: &mut CanonicalWriterV1,
    packed: SemanticCompleteBodyPackingVNext,
    wire_version: SemanticMirWireVersionV1,
) -> Result<(), SemanticMirErrorV1> {
    if wire_version != SemanticMirWireVersionV1::V36 {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: wire_version,
            required: SemanticMirWireVersionV1::V36,
        });
    }
    validate_packing(packed)?;
    writer.u8(92)?;
    writer.u8(0)?;
    writer.u8(packed.block_count)?;
    writer.u8(packed.instruction_count)?;
    for word in packed
        .block_words
        .into_iter()
        .chain(packed.instruction_words)
    {
        writer.u64(word)?;
    }
    Ok(())
}

pub(super) fn encode_source(
    writer: &mut CanonicalWriterV1,
    source: Option<SemanticCompleteBodySourceVNext>,
) -> Result<(), SemanticMirErrorV1> {
    let Some(source) = source else {
        return writer.u8(0);
    };
    writer.u8(1)?;
    for hash in source.root_axes().into_iter().chain([
        source.mir_body(),
        source.block_identity(),
        source.source_signature(),
        source.rustc_fn_abi(),
        source.frontend_bytes_sha256(),
    ]) {
        writer.identity(hash)?;
    }
    writer.u32(source.raw_block())
}
