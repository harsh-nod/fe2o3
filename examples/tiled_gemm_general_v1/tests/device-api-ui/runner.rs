fn main() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(std::env::current_dir().unwrap(), source);
    assert_eq!(
        std::env::var_os("CARGO_MANIFEST_DIR"),
        Some(source.as_os_str().to_owned())
    );
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/pass/*.rs");
    tests.compile_fail("tests/ui/fail/*.rs");
}
