#[test]
fn recovered_worker_v3_admission_typestate_is_closed() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/recovered_worker_v3_admission/*.rs");
}
