#[test]
fn generated_host_contract_v2_rejects_illegal_user_programs() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/generated_host_contract_v2/*.rs");
}
