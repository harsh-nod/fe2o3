#[test]
fn only_the_live_v3_envelope_can_persist_replay_custody() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/worker_v3_load_envelope/*.rs");
}
