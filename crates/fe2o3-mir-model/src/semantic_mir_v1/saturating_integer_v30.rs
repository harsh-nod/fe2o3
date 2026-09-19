//! Typed, inert integer saturation and independent capability-content fencing.

use super::*;

/// Defined integer arithmetic clamped to the result type's inclusive range.
///
/// Both operands and the result have exactly the same admitted integer type.
/// The operation does not wrap, trap on overflow, or grant source authority.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticSaturatingIntegerOpV1 {
    Add,
    Subtract,
}

pub(super) fn signature_matches(
    request: &InertSemanticMirRequestV1,
    abi: &SemanticFunctionAbiV1,
) -> bool {
    let output = abi.source_output_type();
    !abi.can_unwind()
        && abi.source_input_types() == [output, output]
        && request.types.get(output.0 as usize).is_some_and(|ty| {
            ty.rust_type_kind == SemanticRustTypeKindV1::Ordinary
                && matches!(
                    ty.shape,
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: _,
                        bits: 8 | 16 | 32 | 64,
                    })
                )
        })
}

pub(super) fn uses_saturating_integer(request: &InertSemanticMirRequestV1) -> bool {
    request.callables.iter().any(|callable| {
        matches!(
            callable,
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::SaturatingInteger(_),
                ..
            }
        )
    })
}

pub(super) fn contains_inert_execution(request: &InertSemanticMirRequestV1) -> bool {
    request
        .types
        .iter()
        .any(|ty| matches!(ty.rust_type_kind, SemanticRustTypeKindV1::Execution(_)))
        || request.callables.iter().any(|callable| {
            matches!(
                callable,
                SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Execution(_),
                    ..
                }
            )
        })
}

pub(super) fn check_current_production_content(
    request: &InertSemanticMirRequestV1,
) -> Result<(), SemanticMirErrorV1> {
    if contains_inert_execution(request) {
        // Preserve the existing production refusal independently of the
        // maximum version selected by other features in the same request.
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: SemanticMirWireVersionV1::V28,
            required: SemanticMirWireVersionV1::V29,
        });
    }
    Ok(())
}
