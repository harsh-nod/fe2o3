#[test]
fn live_middle_end_evidence_is_move_only() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/middle_end_evidence_v4_not_clone.rs");
    cases.compile_fail("tests/ui/middle_end_evidence_v4_not_copy.rs");
    cases.compile_fail("tests/ui/middle_end_evidence_v5_not_clone.rs");
    cases.compile_fail("tests/ui/middle_end_evidence_v5_not_copy.rs");
    cases.compile_fail("tests/ui/default_api_cannot_self_authorize.rs");
}
