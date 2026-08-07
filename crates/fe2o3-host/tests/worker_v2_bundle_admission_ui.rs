#[test]
fn finalized_worker_v2_bundle_admission_is_opaque_and_inert() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/worker_v2_bundle_admission/*.rs");
}
