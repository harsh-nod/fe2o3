use fe2o3_build_authority::{
    CompilerClosureErrorV2, CompilerClosureV2, derive_compiler_closure_identity_v1,
    derive_rustc_executable_runtime_identity_v1,
};
use sha2::{Digest, Sha256};

const RETAINED_OBJECT_BINDING_DOMAIN_V1: &[u8] = b"fe2o3-compiler-retained-object-binding-v1\0";

/// Binds the admitted tool images and the retained rustc lib-tree directory object.
///
/// This is intentionally not an ELF runtime closure: it does not identify the ELF interpreter,
/// system DT_NEEDED objects, loader configuration, Cargo's runtime dependencies, or the backend's
/// runtime dependencies.
pub(crate) fn compiler_closure_sha256_v1(
    cargo_sha256: &[u8; 32],
    rustc_sha256: &[u8; 32],
    rustc_lib_tree_sha256: &[u8; 32],
    backend_sha256: &[u8; 32],
) -> [u8; 32] {
    let rustc_identity =
        derive_rustc_executable_runtime_identity_v1(*rustc_sha256, *rustc_lib_tree_sha256);
    derive_compiler_closure_identity_v1(*cargo_sha256, rustc_identity, *backend_sha256)
}

/// Constructs the canonical closure for the complete Cargo-to-backend compiler path.
#[cfg_attr(not(test), allow(dead_code))] // Staged for the protected-release V2 call-site patch.
pub(crate) fn compiler_closure_v2_from_pins(
    cargo_sha256: &[u8; 32],
    cargo_binding_trampoline_sha256: &[u8; 32],
    cargo_fe2o3_binding_wrapper_sha256: &[u8; 32],
    rustc_sha256: &[u8; 32],
    rustc_lib_tree_sha256: &[u8; 32],
    backend_sha256: &[u8; 32],
) -> Result<CompilerClosureV2, CompilerClosureErrorV2> {
    CompilerClosureV2::new(
        *cargo_sha256,
        *cargo_binding_trampoline_sha256,
        *cargo_fe2o3_binding_wrapper_sha256,
        *rustc_sha256,
        *rustc_lib_tree_sha256,
        *backend_sha256,
    )
}

pub(crate) fn retained_object_binding_sha256_v1(
    compiler_closure_sha256: &[u8; 32],
    rustc_lib_tree_device: u64,
    rustc_lib_tree_inode: u64,
    rustc_lib_tree_mode: u32,
) -> [u8; 32] {
    let mut binding = Sha256::new();
    binding.update(RETAINED_OBJECT_BINDING_DOMAIN_V1);
    binding.update(compiler_closure_sha256);
    binding.update(rustc_lib_tree_device.to_le_bytes());
    binding.update(rustc_lib_tree_inode.to_le_bytes());
    binding.update(rustc_lib_tree_mode.to_le_bytes());
    binding.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::{
        compiler_closure_sha256_v1, compiler_closure_v2_from_pins,
        retained_object_binding_sha256_v1,
    };

    #[test]
    fn compiler_closure_is_content_only_and_binds_every_component() {
        let baseline = compiler_closure_sha256_v1(&[1; 32], &[2; 32], &[3; 32], &[6; 32]);
        for changed in [
            compiler_closure_sha256_v1(&[7; 32], &[2; 32], &[3; 32], &[6; 32]),
            compiler_closure_sha256_v1(&[1; 32], &[7; 32], &[3; 32], &[6; 32]),
            compiler_closure_sha256_v1(&[1; 32], &[2; 32], &[7; 32], &[6; 32]),
            compiler_closure_sha256_v1(&[1; 32], &[2; 32], &[3; 32], &[7; 32]),
        ] {
            assert_ne!(baseline, changed);
        }
    }

    #[test]
    fn retained_object_binding_is_separate_from_the_canonical_closure() {
        let closure = compiler_closure_sha256_v1(&[1; 32], &[2; 32], &[3; 32], &[4; 32]);
        assert_ne!(
            retained_object_binding_sha256_v1(&closure, 5, 6, 0o40555),
            retained_object_binding_sha256_v1(&closure, 5, 7, 0o40555)
        );
    }

    #[test]
    fn compiler_closure_v2_binds_both_wrapper_images() {
        let baseline = compiler_closure_v2_from_pins(
            &[1; 32], &[2; 32], &[3; 32], &[4; 32], &[5; 32], &[6; 32],
        )
        .unwrap();
        let changed_trampoline = compiler_closure_v2_from_pins(
            &[1; 32], &[7; 32], &[3; 32], &[4; 32], &[5; 32], &[6; 32],
        )
        .unwrap();
        let changed_wrapper = compiler_closure_v2_from_pins(
            &[1; 32], &[2; 32], &[7; 32], &[4; 32], &[5; 32], &[6; 32],
        )
        .unwrap();

        assert_ne!(
            baseline.identity_sha256(),
            changed_trampoline.identity_sha256()
        );
        assert_ne!(
            baseline.identity_sha256(),
            changed_wrapper.identity_sha256()
        );
    }

    // This content-only identity remains distinct from the six-pin protected compiler closure.
    #[test]
    fn content_only_compiler_closure_golden_vector() {
        assert_eq!(
            compiler_closure_sha256_v1(&[0x05; 32], &[0x06; 32], &[0x07; 32], &[0x08; 32]),
            [
                0x1f, 0xea, 0xcf, 0xc5, 0x87, 0x9b, 0x85, 0x3c, 0x7b, 0xa5, 0x5c, 0x34, 0x53, 0x93,
                0x98, 0xe8, 0x57, 0xc0, 0xf9, 0x7d, 0x68, 0x6c, 0xbb, 0x63, 0xcf, 0x99, 0x79, 0x5a,
                0x6a, 0xa0, 0x9e, 0xc9,
            ]
        );
    }
}
