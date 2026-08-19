#[test]
fn worker_v2_finalization_evidence_is_opaque() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/worker_v2_finalization/*.rs");
    tests.compile_fail("tests/ui/worker_v2_execution_custody/*.rs");
}
