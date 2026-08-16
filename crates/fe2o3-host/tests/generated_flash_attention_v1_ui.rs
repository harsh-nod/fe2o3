#[test]
fn exact_flash_attention_lifecycle_is_linear_and_opaque() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/generated_flash_attention_v1/*.rs");
}
