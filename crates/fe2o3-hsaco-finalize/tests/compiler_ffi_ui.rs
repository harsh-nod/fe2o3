#[test]
fn staged_ffi_plan_surface_is_opaque() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/compiler_ffi/*.rs");
}
