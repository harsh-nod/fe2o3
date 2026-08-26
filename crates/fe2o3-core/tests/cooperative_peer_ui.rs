#[test]
fn cooperative_and_peer_authority_cannot_be_forged_crossed_or_bypassed() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/cooperative_peer/capability_fields_are_private.rs");
    tests.compile_fail("tests/ui/cooperative_peer/peer_access_is_linear.rs");
    tests.compile_fail("tests/ui/cooperative_peer/peer_direction_cannot_be_substituted.rs");
}
