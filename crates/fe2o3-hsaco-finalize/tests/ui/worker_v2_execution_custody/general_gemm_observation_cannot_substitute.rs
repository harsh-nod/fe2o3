use fe2o3_hsaco_finalize::OpaqueGeneralGemmPostLinkMachineObservationV1;

fn attempt_substitution(
    original: &OpaqueGeneralGemmPostLinkMachineObservationV1,
    donor: &OpaqueGeneralGemmPostLinkMachineObservationV1,
) {
    let _ = &original.worker_request;
    let _ = &original.worker_response;
    let _ = &original.kernel_symbol_sha256;
    let _ = &donor.prepared;
    let _ = &donor.structural_machine;
}

fn main() {}
