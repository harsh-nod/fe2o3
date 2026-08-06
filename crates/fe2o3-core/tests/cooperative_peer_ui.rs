#[test]
fn cooperative_and_peer_authority_cannot_be_forged_crossed_or_bypassed() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/cooperative_peer/*.rs");
}
