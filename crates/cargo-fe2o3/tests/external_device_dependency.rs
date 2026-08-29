const WORKSPACE_MANIFEST: &str = include_str!("../../../Cargo.toml");

#[test]
fn external_git_consumers_cannot_select_a_device_impostor_fixture() {
    let declaration = WORKSPACE_MANIFEST
        .lines()
        .find(|line| line.starts_with("fe2o3-device = "))
        .expect("workspace must declare fe2o3-device");

    assert!(
        declaration.contains("path = \"crates/fe2o3-device\"")
            && declaration.contains("version = \"=0.1.0\""),
        "fe2o3-device must bind both the workspace path and exact package version: {declaration}"
    );
}
