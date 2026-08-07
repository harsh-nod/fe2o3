#[test]
fn device_buffer_views_enforce_borrows_and_private_construction() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/device_buffer_view/*.rs");
}
