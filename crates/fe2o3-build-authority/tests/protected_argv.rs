use fe2o3_build_authority::PROTECTED_AUTHORITY_ARGV0;

#[test]
fn protected_authority_argv0_is_exact_and_nul_free() {
    assert_eq!(PROTECTED_AUTHORITY_ARGV0, b"/usr/libexec/fe2o3/cargo-fe2o3");
    assert!(!PROTECTED_AUTHORITY_ARGV0.contains(&0));
}
