//! Inert shape of the exact gfx942 Wave64 shuffle-index primitive.

use super::*;

pub(super) fn signature_matches(
    request: &InertSemanticMirRequestV1,
    abi: &SemanticFunctionAbiV1,
    context: SemanticTypeIdV1,
    element: SemanticTypeIdV1,
) -> bool {
    let inputs = abi.source_input_types();
    inputs.len() == 3
        && request.types.get(context.0 as usize).is_some()
        && shared_reference_to(request, inputs[0], context)
        && inputs[1] == element
        && abi.source_output_type() == element
        && is_unsigned_integer_with_bits(request, inputs[2], 32)
        && request
            .types
            .get(inputs[2].0 as usize)
            .is_some_and(|ty| ty.rust_type_kind == SemanticRustTypeKindV1::Ordinary)
        && request.types.get(element.0 as usize).is_some_and(|ty| {
            ty.rust_type_kind == SemanticRustTypeKindV1::Ordinary
                && matches!(
                    ty.shape,
                    SemanticTypeShapeV1::Scalar(
                        SemanticScalarTypeV1::Integer { bits: 32, .. }
                            | SemanticScalarTypeV1::Float { bits: 32 }
                    )
                )
        })
    // Canonical/extern Rust and nonvariadic checks remain in the caller.
    // Preserve the actual library unwind ABI; this is not a nounwind intrinsic.
    // The layout carrier does not authenticate the collector's GPU profile.
}

pub(super) fn uses_wave64_shuffle(request: &InertSemanticMirRequestV1) -> bool {
    request.callables.iter().any(|callable| {
        matches!(
            callable,
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Gfx942Wave64ShuffleIndex { .. },
                ..
            }
        )
    })
}
