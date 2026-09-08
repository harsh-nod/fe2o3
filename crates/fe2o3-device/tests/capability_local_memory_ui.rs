#[test]
fn local_memory_capabilities_enforce_static_boundaries() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/pass/capability_local_memory_flow.rs");
    tests.pass("tests/ui/pass/capability_memory_prelude.rs");
    tests.pass("tests/ui/pass/capability_memory_migration_aliases.rs");
    tests.pass("tests/ui/pass/capability_raw_obligation_identity.rs");
    tests.compile_fail("tests/ui/fail/capability_local_memory_*.rs");
}
