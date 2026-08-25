//! Compile-fail coverage for the feature-free production surface.

#[cfg(not(feature = "qualification-oracles-test-only"))]
#[test]
fn exact_runtime_is_absent_without_qualification() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/feature_free/exact_runtime_is_unavailable.rs");
}
