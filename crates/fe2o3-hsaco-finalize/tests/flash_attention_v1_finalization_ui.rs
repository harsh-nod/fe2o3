#[test]
fn flash_attention_v1_finalization_is_linear_and_opaque() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/flash_attention_v1_finalization/*.rs");
}
