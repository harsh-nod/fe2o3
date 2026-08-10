#[test]
fn tile_allocation_and_stream_lifetimes_are_linear() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/tile_interop/*.rs");
}
