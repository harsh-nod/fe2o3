#[test]
fn prepared_launch_types_cannot_be_forged_or_crossed() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/prepared_launch/*.rs");
}
