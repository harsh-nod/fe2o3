fn main() {
    std::hint::black_box(
        fe2o3_protected_service_profile::protected_service_secure_start_address_v1(),
    );
    // SAFETY: this sole startup entry claims the staged slots once, before other FD use.
    if unsafe { fe2o3_proof_custodian::run_inherited_conditional_fill_proof_controller_v1() }
        .is_err()
    {
        std::process::exit(98);
    }
}
