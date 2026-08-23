#[test]
fn worker_v3_verification_typestate_is_closed() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/worker_v3_verification_admission/*.rs");
}
