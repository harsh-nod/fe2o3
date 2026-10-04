#[test]
fn published_direct_link_admission_is_inert_and_opaque() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/published_direct_link/*.rs");
}
