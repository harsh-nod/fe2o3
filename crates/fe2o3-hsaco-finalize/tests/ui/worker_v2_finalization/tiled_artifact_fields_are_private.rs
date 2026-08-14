use fe2o3_hsaco_finalize::{
    FinalizedTiledGemmV1StructuralHsacoV1, InspectedTiledGemmV1StructuralWorkerV2HsacoV1,
};

fn inspect_private_fields(
    inspected: &InspectedTiledGemmV1StructuralWorkerV2HsacoV1,
    finalized: &FinalizedTiledGemmV1StructuralHsacoV1,
) {
    let _ = &inspected.raw;
    let _ = inspected.descriptor;
    let _ = &finalized.finalized;
    let _ = finalized.descriptor;
}

fn main() {}
