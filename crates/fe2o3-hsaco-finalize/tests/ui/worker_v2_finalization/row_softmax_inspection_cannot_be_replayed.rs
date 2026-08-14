use fe2o3_hsaco_finalize::{
    InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1,
    finalize_row_softmax_v1_structural_worker_v2_hsaco_v1,
};

fn replay(inspected: InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1) {
    let _first = finalize_row_softmax_v1_structural_worker_v2_hsaco_v1(inspected);
    let _second = finalize_row_softmax_v1_structural_worker_v2_hsaco_v1(inspected);
}

fn main() {}
