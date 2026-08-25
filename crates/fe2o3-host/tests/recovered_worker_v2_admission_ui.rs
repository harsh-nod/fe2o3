#![cfg(feature = "qualification-oracles-test-only")]

#[test]
fn recovered_worker_v2_admission_authority_boundaries_are_sealed() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/recovered_worker_v2_admission/*.rs");
}
