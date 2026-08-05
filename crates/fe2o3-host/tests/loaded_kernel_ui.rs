#[test]
fn loaded_kernel_authority_cannot_be_forged_crossed_or_detached() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/loaded_kernel/*.rs");
}
