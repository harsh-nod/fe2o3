#![cfg(feature = "qualification-oracles-test-only")]

#[test]
fn exact_workgroup_sync_authority_borrows_and_profiles_are_closed() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/generated_workgroup_sync_v1/*.rs");
}
