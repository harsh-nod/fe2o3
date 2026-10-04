#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, id};

use fe2o3_static_preexec_manifest::{
    PREEXEC_MANIFEST_BYTES_V1, StaticPreexecDescriptorV1, StaticPreexecManifestV1,
    StaticPreexecObjectIdentityV1,
};

fn rust_fixture() -> [u8; PREEXEC_MANIFEST_BYTES_V1] {
    let objects = [
        StaticPreexecObjectIdentityV1::new(
            0x5152_5354_5556_5758,
            0x6162_6364_6566_6768,
            0x7172_7374_7576_7778,
            0x8182_8384,
        ),
        StaticPreexecObjectIdentityV1::new(
            0x9192_9394_9596_9798,
            0xa1a2_a3a4_a5a6_a7a8,
            0xb1b2_b3b4_b5b6_b7b8,
            0xc1c2_c3c4,
        ),
        StaticPreexecObjectIdentityV1::new(
            0xd1d2_d3d4_d5d6_d7d8,
            0xe1e2_e3e4_e5e6_e7e8,
            0xf1f2_f3f4_f5f6_f7f8,
            0x0102_0304,
        ),
    ];
    StaticPreexecManifestV1::new(
        0x1122_3344,
        0x0102_0304_0506_0708,
        StaticPreexecObjectIdentityV1::new(
            0x1112_1314_1516_1718,
            0x2122_2324_2526_2728,
            0x3132_3334_3536_3738,
            0x4142_4344,
        ),
        objects
            .into_iter()
            .enumerate()
            .map(|(index, object)| {
                StaticPreexecDescriptorV1::for_index(index, index as i32, object).unwrap()
            })
            .collect(),
    )
    .unwrap()
    .encode()
}

#[test]
fn c_header_layout_and_native_struct_bytes_match_the_rust_codec() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let include = crate_root.join("../../tools/fe2o3-static-preexec-launcher/include");
    let source = crate_root.join("tests/c_header_oracle.c");
    let temporary = env::temp_dir().join(format!("fe2o3-static-preexec-manifest-{}", id()));
    let executable = temporary.join("c-header-oracle");
    fs::create_dir_all(&temporary).unwrap();

    let compiler = env::var_os("CC").unwrap_or_else(|| "cc".into());
    let compile = Command::new(compiler)
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg("-I")
        .arg(include)
        .arg(source)
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("C compiler must be available for the authoritative ABI oracle");
    assert!(
        compile.status.success(),
        "C ABI oracle compilation failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let output = Command::new(&executable).output().unwrap();
    let _ = fs::remove_dir_all(&temporary);
    assert!(
        output.status.success(),
        "C ABI oracle failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout.len(), PREEXEC_MANIFEST_BYTES_V1);
    assert_eq!(output.stdout.as_slice(), rust_fixture());
}
