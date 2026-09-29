use super::*;
use crate::{RustcInvocationDescriptorV3, encode_descriptor_v2};
use fe2o3_build_authority::CompilerClosureV2;
use std::mem::size_of;

fn fixture() -> RustcInvocationDescriptorV2 {
    RustcInvocationDescriptorV2::new(
        [0x11; 32],
        [0x22; 32],
        RustcUnitV2::new(
            "/workspace/project",
            vec![
                "/toolchains/rustc".into(),
                "-Zcodegen-backend=/proc/./self/fd/198".into(),
            ],
        )
        .unwrap(),
        CompileEnvironmentV2::from_entries_for_test([
            ("FE2O3_HSACO_DIR", "/proc/self/fd/197"),
            ("FE2O3_TARGET", "gfx942:xnack-"),
        ])
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn retained_storage_counts_unused_string_and_vector_capacity_without_changing_identity() {
    for field in 0..6 {
        let mut descriptor = fixture();
        let wire = encode_descriptor_v2(&descriptor).unwrap();
        let before = descriptor.retained_storage_bytes().unwrap();
        let increase = match field {
            0 | 1 | 2 | 3 => {
                let string = match field {
                    0 => &mut descriptor.rustc.working_directory.0,
                    1 => &mut descriptor.rustc.argv[0].0,
                    2 => &mut descriptor.compile_environment.entries[0].key.0,
                    _ => &mut descriptor.compile_environment.entries[0].value.0,
                };
                let old = string.capacity();
                string.reserve_exact(MAX_DESCRIPTOR_BYTES_V2 + 17);
                string.capacity() - old
            }
            4 => {
                let old = descriptor.rustc.argv.capacity();
                descriptor.rustc.argv.reserve_exact(31);
                (descriptor.rustc.argv.capacity() - old) * size_of::<Argument>()
            }
            _ => {
                let old = descriptor.compile_environment.entries.capacity();
                descriptor.compile_environment.entries.reserve_exact(31);
                (descriptor.compile_environment.entries.capacity() - old)
                    * size_of::<CompileEnvironmentEntryV2>()
            }
        };
        assert!(increase > 0);
        assert_eq!(descriptor.retained_storage_bytes(), Some(before + increase));
        assert_eq!(encode_descriptor_v2(&descriptor).unwrap(), wire);
    }
}

#[test]
fn v3_counts_its_inline_closure_and_retains_exact_v2_backing() {
    let mut v2 = fixture();
    v2.rustc.argv[0].0.reserve_exact(8192);
    let backing = v2.retained_storage_bytes().unwrap() - size_of::<RustcInvocationDescriptorV2>();
    let closure = CompilerClosureV2::new(
        [0x31; 32], [0x32; 32], [0x33; 32], [0x11; 32], [0x35; 32], [0x22; 32],
    )
    .unwrap();
    let v3 = RustcInvocationDescriptorV3::new(v2, closure).unwrap();
    assert_eq!(
        v3.retained_storage_bytes(),
        Some(size_of::<RustcInvocationDescriptorV3>() + backing)
    );
}
