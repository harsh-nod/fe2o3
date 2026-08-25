#![cfg(feature = "qualification-oracles-test-only")]

#[test]
fn wave64_collectives_authority_and_borrows_are_closed() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/generated_wave64_collectives_v1/*.rs");
}
