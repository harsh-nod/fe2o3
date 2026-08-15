#[test]
fn wave64_collectives_v1_finalized_admission_is_linear_and_opaque() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/wave64_collectives_v1_finalization/*.rs");
}
