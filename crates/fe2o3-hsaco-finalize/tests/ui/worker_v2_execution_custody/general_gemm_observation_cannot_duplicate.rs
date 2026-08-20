use fe2o3_hsaco_finalize::OpaqueGeneralGemmPostLinkMachineObservationV1;

fn require_clone<T: Clone>() {}
fn require_copy<T: Copy>() {}

fn main() {
    require_clone::<OpaqueGeneralGemmPostLinkMachineObservationV1>();
    require_copy::<OpaqueGeneralGemmPostLinkMachineObservationV1>();
}
