#[test]
fn vmm_owners_and_raw_addresses_enforce_linear_typestates() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/vmm/*.rs");
}
