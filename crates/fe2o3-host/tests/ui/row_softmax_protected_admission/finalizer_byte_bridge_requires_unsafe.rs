use fe2o3_hsaco_finalize::PreparedProtectedRowSoftmaxV1AdmissionV1;

fn leak(admission: &PreparedProtectedRowSoftmaxV1AdmissionV1) -> Vec<u8> {
    admission.load_exact_finalized_with_reviewed_runtime_v1(|bytes, _identity| bytes.to_vec())
}

fn main() {}
