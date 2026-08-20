#[test]
fn worker_v2_finalization_evidence_is_opaque() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/worker_v2_finalization/*.rs");
    tests.compile_fail("tests/ui/worker_v2_execution_custody/authorized_execution_is_one_shot.rs");
    tests.compile_fail("tests/ui/worker_v2_execution_custody/execution_cannot_clone.rs");
    tests.compile_fail("tests/ui/worker_v2_execution_custody/first_build_evidence_cannot_clone.rs");
    #[cfg(feature = "general-gemm-v1")]
    tests.compile_fail("tests/ui/worker_v2_execution_custody/general_gemm_*.rs");
}
