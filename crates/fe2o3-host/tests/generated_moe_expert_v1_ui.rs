#![cfg(feature = "qualification-oracles-test-only")]

#[test]
fn generated_moe_expert_host_boundaries_are_compile_time_enforced() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/generated_moe_expert_v1/*.rs");
}
