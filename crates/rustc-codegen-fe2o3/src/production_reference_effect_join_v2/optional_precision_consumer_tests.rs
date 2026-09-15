//! Mandatory consumer bridge for the optional producer's component regression.
use super::*;

pub(crate) fn required_output_guard(
    kernel: &ProductionRankedKernelV1,
    view: ProductionRankedValueV1,
    point: ProductionRankedValueV1,
) -> Result<ReferencePathPredicateV1, ProductionReferenceEffectJoinErrorV2> {
    let ir = ReferenceEffectIrV1 {
        argument_count: 2,
        local_count: 3,
        relations: vec![
            ReferenceArgumentRelationV1::PointCoordinate {
                reference_argument: 0,
                axis: 0,
            },
            ReferenceArgumentRelationV1::InvocationDisjointOutputSlice1D {
                argument: 0,
                element: ReferenceScalarTypeV1::F32,
            },
        ]
        .into_boxed_slice(),
        blocks: Box::default(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    };
    gpu_write_path_predicate_v2(
        kernel,
        &ir,
        &RankedGpuWriteV2 {
            block: 1,
            operation: 0,
            allocation_origin: 1,
            view,
            indices: vec![point],
            value: Err("guard component only"),
        },
    )
}
