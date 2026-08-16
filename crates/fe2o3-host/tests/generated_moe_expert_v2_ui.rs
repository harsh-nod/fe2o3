#[test]
fn generated_moe_expert_v2_boundaries_are_compile_time_enforced() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/generated_moe_expert_v2/*.rs");

    #[cfg(feature = "hardware-test-hooks")]
    cases.compile_fail("tests/ui/generated_moe_expert_v2_hardware_hooks/*.rs");
}
