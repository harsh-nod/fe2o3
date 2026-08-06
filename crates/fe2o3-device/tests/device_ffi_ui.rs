#[test]
fn device_ffi_contracts_enforce_the_review_boundary() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/device_ffi_ui/pass/*.rs");
    tests.compile_fail("tests/device_ffi_ui/fail/*.rs");
}
