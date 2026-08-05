#[test]
fn device_api_enforces_witness_boundaries() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/fail/*.rs");
}
