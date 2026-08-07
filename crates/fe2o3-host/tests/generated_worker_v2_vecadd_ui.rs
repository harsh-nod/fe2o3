#[test]
fn generated_worker_v2_vecadd_authority_and_resources_are_linear() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/generated_worker_v2_vecadd/*.rs");
}
