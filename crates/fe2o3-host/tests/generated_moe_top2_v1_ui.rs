#[test]
fn exact_moe_top2_lifecycle_is_linear_and_opaque() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/generated_moe_top2_v1/*.rs");
}
