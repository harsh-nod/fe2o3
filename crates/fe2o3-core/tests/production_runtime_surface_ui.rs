#![cfg(not(feature = "qualification-unsafe-launch"))]

#[test]
fn production_runtime_exposes_no_raw_module_or_launch_authority() {
    let tests = trybuild::TestCases::new();
    if cfg!(feature = "qualification-legacy-hip-runtime") {
        tests.compile_fail("tests/ui/production_runtime_surface/*.rs");
    } else {
        tests.compile_fail("tests/ui/default_runtime_surface/*.rs");
    }
}
