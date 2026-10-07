fn main() {
    std::hint::black_box(
        fe2o3_protected_service_profile::protected_service_secure_start_address_v1(),
    );
    // SAFETY: this fixed entry claims its six inherited slots once before other FD use.
    if unsafe { fe2o3_proof_custodian::run_inherited_application_proof_controller_v1() }.is_err() {
        std::process::exit(98);
    }
}
