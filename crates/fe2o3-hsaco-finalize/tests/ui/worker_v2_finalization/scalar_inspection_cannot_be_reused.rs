use fe2o3_hsaco_finalize::InspectedScalarGemmV1WorkerV2HsacoV1;

fn reuse(inspected: InspectedScalarGemmV1WorkerV2HsacoV1) {
    let raw = inspected.into_raw();
    let _ = inspected.exact_bytes();
    drop(raw);
}

fn main() {}
