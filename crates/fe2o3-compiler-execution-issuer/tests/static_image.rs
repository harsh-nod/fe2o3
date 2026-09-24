#[test]
#[ignore = "run through scripts/build-static-compiler-execution-issuer.sh"]
fn release_image_is_loader_independent_static_elf() {
    check_image("FE2O3_STATIC_COMPILER_EXECUTION_ISSUER");
}

#[test]
#[ignore = "run through scripts/build-static-compiler-execution-issuer.sh --native"]
fn native_release_image_is_loader_independent_static_elf() {
    check_image("FE2O3_STATIC_COMPILER_EXECUTION_ISSUER_NATIVE");
}

fn check_image(variable: &str) {
    let path = std::env::var_os(variable)
        .expect("static compiler-execution issuer image path is required");
    let bytes = std::fs::read(path).expect("read static compiler-execution issuer image");
    let identity = fe2o3_runtime_protocol::sealed_static_application_identity_v1(&bytes)
        .expect("static compiler-execution issuer must satisfy the production ELF profile");
    assert_ne!(identity, [0; 32]);
}
