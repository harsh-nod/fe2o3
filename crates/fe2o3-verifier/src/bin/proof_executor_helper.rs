use std::process::ExitCode;

fn main() -> ExitCode {
    std::hint::black_box(
        fe2o3_protected_service_profile::protected_service_secure_start_address_v1(),
    );
    // SAFETY: This is the sole entry in a dedicated helper process. No threads or
    // Rust FD owners are created before the inherited entry, and refusal is final.
    #[allow(unsafe_code)]
    let result = unsafe { fe2o3_verifier::run_inherited_proof_executor_helper_v1() };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
