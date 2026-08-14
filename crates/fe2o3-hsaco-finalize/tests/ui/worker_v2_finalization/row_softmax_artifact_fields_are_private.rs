use fe2o3_hsaco_finalize::{
    FinalizedRowSoftmaxV1StructuralHsacoV1, InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1,
};

fn inspect_private_fields(
    inspected: &InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1,
    finalized: &FinalizedRowSoftmaxV1StructuralHsacoV1,
) {
    let _ = &inspected.raw;
    let _ = inspected.descriptor;
    let _ = &finalized.finalized;
    let _ = finalized.descriptor;
}

fn main() {}
