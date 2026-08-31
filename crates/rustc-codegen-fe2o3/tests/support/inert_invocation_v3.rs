use std::ffi::OsString;

use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcInvocationDescriptorV3, RustcUnitV2,
    encode_descriptor_v3,
};

const PINS: [[u8; 32]; 6] = [
    [0x11; 32], [0x22; 32], [0x33; 32], [0x44; 32], [0x55; 32], [0x66; 32],
];

pub fn canonical_inert_gfx942_invocation_hex() -> String {
    let rustc = RustcUnitV2::new(
        "/workspace/fe2o3",
        vec![
            "/opt/fe2o3/rustc".into(),
            "--crate-name".into(),
            "fe2o3_extraction_fixture".into(),
            "crates/fe2o3-device/src/lib.rs".into(),
            "--crate-type=lib".into(),
            "--edition=2024".into(),
            "-Cmetadata=0123".into(),
            "-Zcodegen-backend=/opt/fe2o3/librustc_codegen_fe2o3.so".into(),
        ],
    )
    .expect("valid inert extraction rustc unit");
    let environment = CompileEnvironmentV2::from_child_environment([
        os_entry("CARGO_CFG_TARGET_ARCH", "amdgcn"),
        os_entry("FE2O3_HSACO_DIR", "/workspace/fe2o3/target/fe2o3"),
        os_entry("FE2O3_TARGET", "gfx942:xnack-"),
        os_entry("FE2O3_VERIFY_KERNEL_IR", "1"),
    ])
    .expect("valid inert extraction environment");
    let descriptor_v2 = RustcInvocationDescriptorV2::new(PINS[3], PINS[5], rustc, environment)
        .expect("valid inert extraction V2 descriptor");
    let closure = CompilerClosureV2::new(PINS[0], PINS[1], PINS[2], PINS[3], PINS[4], PINS[5])
        .expect("valid inert extraction compiler closure");
    let descriptor = RustcInvocationDescriptorV3::new(descriptor_v2, closure)
        .expect("valid inert extraction V3 descriptor");
    encode_descriptor_v3(&descriptor)
        .expect("encode inert extraction V3 descriptor")
        .into_iter()
        .flat_map(|byte| {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            [
                HEX[usize::from(byte >> 4)] as char,
                HEX[usize::from(byte & 0x0f)] as char,
            ]
        })
        .collect()
}

fn os_entry(key: &str, value: &str) -> (OsString, OsString) {
    (key.into(), value.into())
}
