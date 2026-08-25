#[test]
fn published_direct_link_admission_is_inert_and_opaque() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/published_direct_link/*.rs");
}

#[cfg(not(feature = "qualification-oracles-test-only"))]
#[test]
fn feature_free_does_not_export_generated_kernel_binding() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/published_direct_link/feature_free/*.rs");
}

#[cfg(feature = "qualification-oracles-test-only")]
#[test]
fn qualification_admission_cannot_mint_generated_kernel_binding() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/published_direct_link/qualification/*.rs");
}
