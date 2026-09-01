#[test]
fn worker_v3_verification_typestate_is_closed() {
    let cases = trybuild::TestCases::new();
    if cfg!(feature = "worker-v3-verifier-test-support") {
        for case in [
            "authenticated_cannot_clone.rs",
            "authenticated_roster_cannot_clone.rs",
            "authenticated_roster_custody_is_private.rs",
            "authenticated_roster_entry_blocks_owner_drop.rs",
            "authenticated_roster_entry_cannot_clone.rs",
            "authenticated_roster_entry_cannot_escape.rs",
            "authenticated_roster_load_and_launch_are_unavailable.rs",
            "compiler_execution_receipt_cannot_clone.rs",
            "currentness_token_is_unavailable.rs",
            "decision_cannot_clone.rs",
            "decision_constructor_is_private.rs",
            "generated_arguments_trait_requires_unsafe_impl.rs",
            "load_is_unavailable.rs",
            "loaded_cannot_clone.rs",
            "loaded_launch_is_unavailable.rs",
            "prepared_cannot_clone.rs",
            "prepared_cannot_dispatch_twice.rs",
            "protected_backend_requires_unsafe_impl.rs",
            "protected_roster_backend_requires_unsafe_impl.rs",
            "request_device_is_unavailable.rs",
            "roster_decision_cannot_clone.rs",
            "roster_decision_constructor_is_unavailable.rs",
            "roster_finalizer_cannot_escape.rs",
            "unload_while_prepared.rs",
        ] {
            cases.compile_fail(format!("tests/ui/worker_v3_verification_admission/{case}"));
        }
    } else {
        cases.compile_fail("tests/ui/worker_v3_verification_admission/*.rs");
    }
}
