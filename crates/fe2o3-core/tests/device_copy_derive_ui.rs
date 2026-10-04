#[test]
fn device_copy_derive_ui() {
    let tests = trybuild::TestCases::new();
    // trybuild links pass fixtures as binaries, which would require libamdhip64
    // on generic CI. Real-trait pass coverage is compiled separately through
    // `device_copy_derive_compile`; compile-fail fixtures do not link.
    tests.compile_fail("tests/ui/device_copy/fail/*.rs");
}
