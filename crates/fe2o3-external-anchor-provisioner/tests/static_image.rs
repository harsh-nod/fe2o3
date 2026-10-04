#[test]
#[ignore = "run through scripts/build-static-external-anchor-provisioning-helper.sh"]
fn release_image_is_loader_independent_static_elf() {
    let path = std::env::var_os("FE2O3_STATIC_EXTERNAL_ANCHOR_HELPER")
        .expect("static external-anchor helper image path is required");
    let bytes = std::fs::read(path).expect("read static external-anchor helper image");
    assert!(bytes.len() <= 128 * 1024 * 1024);
    let identity = fe2o3_runtime_protocol::sealed_static_application_identity_v1(&bytes)
        .expect("static external-anchor helper image must satisfy the production ELF profile");
    assert_ne!(identity, [0; 32]);
}
