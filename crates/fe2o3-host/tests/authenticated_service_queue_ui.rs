#[test]
fn authenticated_service_queue_typestate_is_closed() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/authenticated_service_queue/*.rs");
}
