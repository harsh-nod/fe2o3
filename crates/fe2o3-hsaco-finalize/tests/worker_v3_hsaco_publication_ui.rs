#[test]
fn worker_v3_publication_evidence_is_opaque() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/worker_v3_publication/*.rs");
}
