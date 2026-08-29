fn main() {
    std::hint::black_box(
        fe2o3_protected_service_profile::protected_service_secure_start_address_v1(),
    );
    if fe2o3_external_anchor_provisioner::run_inherited_external_anchor_provisioning_helper_v1()
        .is_err()
    {
        std::process::exit(1);
    }
}
