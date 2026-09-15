//! Source-shaped workgroup-memory indices, distinct from global mappings.

use super::*;

pub(super) fn abi_matches(
    request: &InertSemanticMirRequestV1,
    abi: &SemanticFunctionAbiV1,
    operation: SemanticExecutionCapabilityOperationV1,
) -> bool {
    use SemanticExecutionCapabilityOperationV1 as Op;
    match operation {
        Op::WorkgroupMemoryIndexV2 {
            workgroup_reference,
            workgroup,
            option,
            witness,
        } => {
            abi.source_input_types() == [workgroup_reference]
                && abi.source_output_type() == option
                && abi.source_argument_ownership()
                    == [SemanticSourceArgumentOwnershipV1::SharedBorrow]
                && shared_reference_to(request, workgroup_reference, workgroup)
                && option_value_result_matches(request, option, witness)
                && witness_raw_type(request, witness).is_some()
        }
        Op::WorkgroupMemoryIndexIntoDisjoint {
            input_witness,
            output_witness,
        } => {
            abi.source_input_types() == [input_witness]
                && abi.source_output_type() == output_witness
                && abi.source_argument_ownership() == [SemanticSourceArgumentOwnershipV1::ByValue]
                && input_witness != output_witness
                && witness_raw_type(request, input_witness)
                    .is_some_and(|raw| witness_raw_type(request, output_witness) == Some(raw))
        }
        _ => true,
    }
}

fn witness_raw_type(
    request: &InertSemanticMirRequestV1,
    witness: SemanticTypeIdV1,
) -> Option<SemanticTypeIdV1> {
    let SemanticTypeShapeV1::Aggregate(fields) =
        request.types.get(witness.index() as usize)?.shape()
    else {
        return None;
    };
    let raw = *fields.fields().first()?;
    transparent_index_witness_matches(request, witness, raw).then_some(raw)
}

pub(super) fn is_scoped_index(operation: SemanticExecutionCapabilityOperationV1) -> bool {
    matches!(
        operation,
        SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexV2 { .. }
            | SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexIntoDisjoint { .. }
    )
}
