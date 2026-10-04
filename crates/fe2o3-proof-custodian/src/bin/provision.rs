fn main() {
    std::hint::black_box(
        fe2o3_protected_service_profile::protected_service_secure_start_address_v1(),
    );
    if let Err(error) = fe2o3_proof_custodian::run_proof_custodian_provisioner_v1() {
        eprintln!("proof provisioning: {error}");
        std::process::exit(98);
    }
}
