#[test]
fn source_capability_hierarchy_is_type_checked() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/pass/source_capability_hierarchy.rs");
    tests.pass("tests/ui/pass/source_capability_general_matrix.rs");
    tests.pass("tests/ui/pass/source_capability_reusable_phases.rs");
    tests.compile_fail("tests/ui/fail/source_capability_*.rs");
}
