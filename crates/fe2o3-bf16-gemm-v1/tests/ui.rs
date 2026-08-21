#![allow(missing_docs)]

#[test]
fn authority_types_remain_opaque_and_linear() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
