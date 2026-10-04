//! Exact rustc-to-semantic scalar join for saturation terminal declarations.

use super::*;
use crate::production_rustc_intrinsic_v1::saturating_integer::integer_shape_v1;

pub(super) fn matches_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    types: &[SemanticTypeDeclV1],
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
    rust_inputs: &[Ty<'tcx>],
    rust_output: Ty<'tcx>,
) -> bool {
    let [left, right] = inputs else {
        return false;
    };
    let [rust_left, rust_right] = rust_inputs else {
        return false;
    };
    if *left != output
        || *right != output
        || *rust_left != rust_output
        || *rust_right != rust_output
    {
        return false;
    }
    let Some((signed, bits)) = integer_shape_v1(rust_output, tcx.data_layout.pointer_size().bits())
    else {
        return false;
    };
    types.get(output.index() as usize).is_some_and(|ty| {
        matches!(ty.shape(), SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: actual_signed, bits: actual_bits,
        }) if *actual_signed == signed && *actual_bits == bits)
    })
}

#[cfg(test)]
mod tests {
    use super::super::{TerminalIdentitySchemaV1, terminal_operation_tag_for_schema_v1};
    use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1 as Expansion;
    use fe2o3_mir_model::semantic_mir_v1::SemanticSaturatingIntegerOpV1 as Op;

    #[test]
    fn saturating_tags_are_distinct_without_changing_existing_terminal_bytes() {
        let schema = TerminalIdentitySchemaV1::CombinedV4;
        for (expansion, expected) in [
            (Expansion::RustcFabsF32, 113),
            (Expansion::MemoryVolatileLoad, 115),
            (Expansion::Bf16MatrixBColumnMajorLoadZeroFilledV1, 121),
            (Expansion::RustcSaturatingInteger(Op::Add), 127),
            (Expansion::RustcSaturatingInteger(Op::Subtract), 128),
        ] {
            assert_eq!(
                terminal_operation_tag_for_schema_v1(expansion, schema),
                expected
            );
        }
    }
}
