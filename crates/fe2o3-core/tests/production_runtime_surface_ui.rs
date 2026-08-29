#![cfg(not(feature = "qualification-unsafe-launch"))]

#[test]
fn production_runtime_exposes_no_raw_module_or_launch_authority() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/production_runtime_surface/*.rs");
}
