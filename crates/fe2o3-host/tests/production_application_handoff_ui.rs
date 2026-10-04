#[test]
fn production_exports_only_the_worker_v3_application_entrypoint() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/production_application_handoff/*.rs");
}
