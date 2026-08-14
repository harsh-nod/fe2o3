use fe2o3_hsaco_finalize::{InspectedRawWorkerV2HsacoV1, InspectedScalarGemmV1WorkerV2HsacoV1};

#[test]
fn scalar_inspection_exposes_only_a_consuming_raw_ownership_bridge() {
    let bridge: fn(InspectedScalarGemmV1WorkerV2HsacoV1) -> InspectedRawWorkerV2HsacoV1 =
        InspectedScalarGemmV1WorkerV2HsacoV1::into_raw;

    let _ = bridge;
}
