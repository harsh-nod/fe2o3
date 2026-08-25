#![cfg(feature = "qualification-oracles-test-only")]

#[test]
fn launch_kernel_v2_bridge_remains_inert_and_sealed() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/launch_kernel_v2_bridge/*.rs");
}
