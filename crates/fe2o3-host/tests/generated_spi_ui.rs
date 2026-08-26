#[test]
fn generated_spi_is_explicit_unsafe_and_type_sealed() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/generated_spi/fail/*.rs");
    tests.pass("tests/ui/generated_spi/pass/*.rs");
}
