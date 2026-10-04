#[test]
#[ignore = "run through scripts/build-static-compiler-execution-supervisor.sh"]
fn release_image_is_loader_independent_static_elf() {
    let path = std::env::var_os("FE2O3_STATIC_COMPILER_EXECUTION_SUPERVISOR")
        .expect("static compiler-execution supervisor image path is required");
    let bytes = std::fs::read(path).expect("read static compiler-execution supervisor image");
    assert!(
        bytes.len()
            <= fe2o3_compiler_execution_protocol::MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V1
                as usize
    );
    let identity = fe2o3_runtime_protocol::sealed_static_application_identity_v1(&bytes).expect(
        "static compiler-execution supervisor image must satisfy the production ELF profile",
    );
    assert_ne!(identity, [0; 32]);
}
