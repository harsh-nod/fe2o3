fn main() {
    std::hint::black_box(
        fe2o3_protected_service_profile::protected_service_secure_start_address_v1(),
    );
    // SAFETY: this dedicated one-application executable is installed only under
    // the approved root manager unit, whose external whole-cgroup custodian
    // survives fail-stop. No other work, thread or child exists before entry.
    let result = unsafe { fe2o3_proof_custodian::run_fixed_native_application_manager_v1() };
    if let Err(error) = result {
        eprintln!("native application manager: {error}");
        std::process::exit(98);
    }
}
