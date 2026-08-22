use sha2::{Digest, Sha256};
use syn::{Item, Visibility};

use fe2o3_moe_top2_v1::{
    MOE_KERNEL_NAMESPACE_V1, MOE_KERNEL_SOURCE_SHA256_V1, MOE_PROFILE_IDENTITY_V1,
    validate_kernel_source_identity_v1,
};

const SOURCE: &str = include_str!("../src/kernel.rs");

#[test]
fn exact_ordinary_attributed_kernel_is_discovered() {
    let file = syn::parse_file(SOURCE).unwrap();
    let kernels: Vec<_> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function)
                if function
                    .attrs
                    .iter()
                    .any(|attribute| attribute.path().is_ident("kernel")) =>
            {
                Some(function)
            }
            _ => None,
        })
        .collect();

    assert_eq!(kernels.len(), 1);
    let kernel = kernels[0];
    assert_eq!(kernel.sig.ident, "moe_top2_route_f32_t8_e4_k2_c4_v1");
    assert!(matches!(kernel.vis, Visibility::Public(_)));
    assert!(kernel.sig.unsafety.is_none());
    assert_eq!(kernel.sig.inputs.len(), 8);

    let attribute = kernel
        .attrs
        .iter()
        .find(|attribute| attribute.path().is_ident("kernel"))
        .unwrap();
    let arguments = attribute.meta.require_list().unwrap().tokens.to_string();
    assert!(arguments.contains("typed"));
    assert!(arguments.contains(MOE_KERNEL_NAMESPACE_V1));
    assert!(arguments.contains("required = [64 , 1 , 1]"));
    assert!(arguments.contains("max = [64 , 1 , 1]"));
    assert!(arguments.contains("loop_bounds (8 , 4 , 16 , 16 , 4)"));
    assert!(SOURCE.contains("thread::grid_leader()"));
    assert!(SOURCE.contains("get_mut_exclusive"));
    assert!(SOURCE.contains("let mut staged_permutation"));
    assert!(file.items.iter().all(|item| !matches!(
        item,
        Item::Macro(item_macro) if item_macro.mac.path.is_ident("macro_rules")
    )));
}

#[test]
fn source_and_profile_namespace_identities_are_exact() {
    let source_digest = format!("{:x}", Sha256::digest(SOURCE.as_bytes()));
    assert_eq!(source_digest, MOE_KERNEL_SOURCE_SHA256_V1);
    assert_eq!(validate_kernel_source_identity_v1(&source_digest), Ok(()));

    let namespace_digest = format!("{:x}", Sha256::digest(MOE_PROFILE_IDENTITY_V1.as_bytes()));
    assert_eq!(namespace_digest, MOE_KERNEL_NAMESPACE_V1);
}

#[test]
fn any_source_identity_mutation_is_rejected() {
    let mut wrong = MOE_KERNEL_SOURCE_SHA256_V1.as_bytes().to_vec();
    wrong[0] = if wrong[0] == b'0' { b'1' } else { b'0' };
    let wrong = String::from_utf8(wrong).unwrap();
    assert!(validate_kernel_source_identity_v1(&wrong).is_err());
    assert!(validate_kernel_source_identity_v1("").is_err());
}

#[test]
fn source_states_the_pending_compiler_authority_boundary() {
    assert!(SOURCE.contains("MOE_TOP2_SOURCE_LOWERING_SUPPORTED_V1: bool = false"));
    assert!(SOURCE.contains("no authenticated MIR-to-Kernel-IR compiler profile"));
    assert!(SOURCE.contains("not explanatory pseudocode"));
    assert!(SOURCE.contains("not a `macro_rules!` facade"));
}
