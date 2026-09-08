#[test]
fn context_derived_indices_retain_their_kernel_brand() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/pass/context_derived_index.rs");
    tests.compile_fail("tests/ui/fail/context_index_cross_kernel_substitution.rs");
    tests.compile_fail("tests/ui/fail/context_2d_index_cross_kernel_substitution.rs");
}
