use std::collections::BTreeSet;

use fe2o3_gfx950_advanced_attention::{
    GFX950_ADVANCED_ATTENTION_GRID_V1, GFX950_ADVANCED_ATTENTION_SOURCE_BLOCKER_V1,
    GFX950_ADVANCED_ATTENTION_SOURCE_LOWERING_SUPPORTED_V1, GFX950_ADVANCED_ATTENTION_WORKGROUP_V1,
};
use syn::{Item, Visibility};

const LIB_SOURCE: &str = include_str!("../src/lib.rs");
const SOURCE: &str = include_str!("../src/kernel.rs");

#[test]
fn source_contains_the_seven_expected_typed_kernels() {
    let file = syn::parse_file(SOURCE).expect("kernel source parses as ordinary Rust");
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
    let names: BTreeSet<_> = kernels
        .iter()
        .map(|function| function.sig.ident.to_string())
        .collect();
    assert_eq!(
        names,
        BTreeSet::from([
            "gfx950_attnres_aggregate".to_string(),
            "gfx950_compressed_hybrid_attention".to_string(),
            "gfx950_content_sparse_attention".to_string(),
            "gfx950_four_branch_residual".to_string(),
            "gfx950_kda_gdn_decode".to_string(),
            "gfx950_kda_gdn_prefill".to_string(),
            "gfx950_mhc_sinkhorn_mix".to_string(),
        ])
    );
    assert_eq!(kernels.len(), 7);
    for function in kernels {
        assert!(matches!(function.vis, Visibility::Public(_)));
        assert!(function.sig.unsafety.is_none());
        let attribute = function
            .attrs
            .iter()
            .find(|attribute| attribute.path().is_ident("kernel"))
            .unwrap();
        let arguments = attribute.meta.require_list().unwrap().tokens.to_string();
        assert!(arguments.contains("typed"));
        assert!(arguments.contains("namespace"));
        assert!(arguments.contains("required = [64 , 1 , 1]"));
        assert!(arguments.contains("max = [64 , 1 , 1]"));
        assert!(arguments.contains("max_grid = [1 , 1 , 1]"));
    }
}

#[test]
fn source_is_safe_fixed_shape_rust_without_hip_escape_hatches() {
    let lowercase = SOURCE.to_ascii_lowercase();
    assert!(!SOURCE.contains("unsafe"));
    assert!(!SOURCE.contains("include!"));
    assert_eq!(SOURCE.matches("macro_rules!").count(), 2);
    assert!(SOURCE.contains("#[cfg(target_arch = \"amdgpu\")]\nmacro_rules! decode_fp8_e4m3_v1"));
    assert!(
        SOURCE.contains(
            "#[cfg(target_arch = \"amdgpu\")]\nmacro_rules! consider_sparse_candidate_v1"
        )
    );
    assert!(!lowercase.contains("extern \"c\""));
    assert!(!lowercase.contains("hiplaunchkernel"));
    assert!(!lowercase.contains("std::process"));
    for marker in [
        "KDA_TAPS_V1",
        "PREFILL_TOKENS_V1",
        "ATTENTION_TOKENS_V1",
        "HEAD_DIMENSION_V1",
        "SELECTED_TOKENS_V1",
        "SINKHORN_ITERATIONS_V1",
        "thread::grid_leader()",
        "get_mut_exclusive",
        "math.exp_f32(-2.0 * value)",
        "exponent == 15 && mantissa == 7.0",
        "Gfx950Fp8MfmaAMatrix::row_major",
        "stage_k_transposed",
        "read_mfma_fragment",
        "multiply_accumulate_fp8",
        "reduce_sum_f32::<16>",
        "broadcast_f32::<16>",
    ] {
        assert!(
            SOURCE.contains(marker),
            "missing fixed source marker {marker}"
        );
    }
}

#[test]
fn package_states_the_production_source_and_evidence_boundary() {
    assert_eq!(GFX950_ADVANCED_ATTENTION_WORKGROUP_V1, [64, 1, 1]);
    assert_eq!(GFX950_ADVANCED_ATTENTION_GRID_V1, [1, 1, 1]);
    assert!(GFX950_ADVANCED_ATTENTION_SOURCE_LOWERING_SUPPORTED_V1);
    assert!(
        LIB_SOURCE.contains("GFX950_ADVANCED_ATTENTION_SOURCE_LOWERING_SUPPORTED_V1: bool = true")
    );
    assert!(GFX950_ADVANCED_ATTENTION_SOURCE_BLOCKER_V1.contains("formal compiler refinement"));
    assert!(GFX950_ADVANCED_ATTENTION_SOURCE_BLOCKER_V1.contains("protected publication"));

    for feature in [
        "kernel-kda-decode",
        "kernel-kda-prefill",
        "kernel-content-sparse-attention",
        "kernel-compressed-hybrid-attention",
        "kernel-attnres-aggregate",
        "kernel-four-branch-residual",
        "kernel-mhc-sinkhorn-mix",
    ] {
        assert!(LIB_SOURCE.contains(feature));
    }
}
