use std::collections::BTreeSet;

use fe2o3_gfx950_advanced_attention::{
    GFX950_ADVANCED_ATTENTION_BUNDLE_V8_SUPPORTED_V1,
    GFX950_ADVANCED_ATTENTION_FULLY_TYPED_MEMORY_V1, GFX950_ADVANCED_ATTENTION_GRID_V1,
    GFX950_ADVANCED_ATTENTION_SOURCE_BLOCKER_V1,
    GFX950_ADVANCED_ATTENTION_SOURCE_LOWERING_SUPPORTED_V1, GFX950_ADVANCED_ATTENTION_WORKGROUP_V1,
    GFX950_KDA_WORKGROUP_V2, batch_count_for_launch_v1,
};
use syn::{FnArg, Item, Type, Visibility};

const LIB_SOURCE: &str = include_str!("../src/lib.rs");
const CARGO_MANIFEST: &str = include_str!("../Cargo.toml");
const SOURCE: &str = include_str!("../src/kernel.rs");
const ABLATION_SOURCE: &str = include_str!("../src/ablation.rs");
const KDA_BASELINE_SOURCE: &str = include_str!("../src/kda_baseline.rs");
const ABLATION_REGISTRY: &str = include_str!("../ablation-variants-v1.json");
const PERFORMANCE_AUDIT: &str = include_str!("../performance-audit-mi350-gpu4-v1.json");
const RUNNER_SOURCE: &str = include_str!("../run-gfx950.sh");

fn outer_type_name(argument: &FnArg) -> Option<&syn::Ident> {
    let FnArg::Typed(argument) = argument else {
        return None;
    };
    let Type::Path(argument) = argument.ty.as_ref() else {
        return None;
    };
    argument.path.segments.last().map(|segment| &segment.ident)
}

fn assert_typed_kernel_signature(function: &syn::ItemFn) {
    let mut arguments = function.sig.inputs.iter();
    assert_eq!(
        arguments.next().and_then(outer_type_name),
        Some(&syn::Ident::new("KernelContext", function.sig.ident.span())),
        "{} must receive compiler-issued context first",
        function.sig.ident
    );
    for argument in arguments {
        let name = outer_type_name(argument).map(ToString::to_string);
        assert!(
            matches!(name.as_deref(), Some("Global" | "u32")),
            "{} retained an untyped memory or unsupported scalar argument",
            function.sig.ident,
        );
    }
}

fn assert_no_compatibility_surface(label: &str, source: &str) {
    for forbidden in [
        "::current(",
        "DeviceMath::current",
        "thread::",
        "DisjointSlice",
        "StridedReadView2D",
        "q: &[u8]",
        "k: &[u8]",
        "Gfx950Subgroup::current",
        "Gfx950LdsTransposeTile",
        "context.subgroup_lane",
        "context.matrix()",
        "WaveLane::<Wave64>::current()",
        "Gfx950Matrix::current()",
        "Gfx950Fp8MfmaAMatrix::row_major",
    ] {
        assert!(
            !source.contains(forbidden),
            "{label} retained {forbidden:?}"
        );
    }
}

#[test]
fn source_contains_the_eight_expected_typed_kernels() {
    let file = syn::parse_file(SOURCE).expect("kernel source parses as ordinary Rust");
    let kernels: Vec<_> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function)
                if function.attrs.iter().any(|attribute| {
                    attribute.path().is_ident("kernel")
                        || (attribute.path().is_ident("cfg_attr")
                            && attribute
                                .meta
                                .require_list()
                                .is_ok_and(|list| list.tokens.to_string().contains("kernel")))
                }) =>
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
            "gfx950_deepseek_sparse_attention".to_string(),
            "gfx950_four_branch_residual".to_string(),
            "gfx950_kda_chunkwise_prefill".to_string(),
            "gfx950_kda_decode".to_string(),
            "gfx950_mhc_sinkhorn_mix".to_string(),
        ])
    );
    assert_eq!(kernels.len(), 8);
    assert_eq!(SOURCE.matches("KernelContext<'_>").count(), 8);
    for function in kernels {
        assert_typed_kernel_signature(function);
        assert!(matches!(function.vis, Visibility::Public(_)));
        assert!(function.sig.unsafety.is_none());
        let attributes = function
            .attrs
            .iter()
            .filter_map(|attribute| attribute.meta.require_list().ok())
            .map(|list| list.tokens.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(attributes.contains("typed"));
        assert!(!attributes.contains("namespace"));
        assert!(attributes.contains("required = [256 , 1 , 1]"));
        assert!(attributes.contains("max = [256 , 1 , 1]"));
        assert!(attributes.contains("max_grid = [4 , 1 , 1]"));
    }
}

#[test]
fn source_contains_the_three_standalone_ablation_kernels() {
    let file = syn::parse_file(ABLATION_SOURCE).expect("ablation source parses as ordinary Rust");
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
    assert_eq!(kernels.len(), 3);
    assert_eq!(
        ABLATION_SOURCE
            .matches("context: KernelContext<'_>")
            .count(),
        3
    );
    assert_eq!(
        kernels
            .iter()
            .map(|function| function.sig.ident.to_string())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "gfx950_attnres_aggregate".to_string(),
            "gfx950_four_branch_residual".to_string(),
            "gfx950_mhc_sinkhorn_mix".to_string(),
        ])
    );
    assert!(!ABLATION_SOURCE.contains("unsafe"));
    assert_eq!(ABLATION_SOURCE.matches("required = [256, 1, 1]").count(), 3);
    assert_eq!(ABLATION_SOURCE.matches("max_grid = [4, 1, 1]").count(), 3);
    assert!(
        !ABLATION_SOURCE
            .to_ascii_lowercase()
            .contains("extern \"c\"")
    );
    for function in &kernels {
        assert_typed_kernel_signature(function);
    }
    assert_no_compatibility_surface("ablation", ABLATION_SOURCE);

    let baseline =
        syn::parse_file(KDA_BASELINE_SOURCE).expect("KDA baseline parses as ordinary Rust");
    let baseline_kernels = baseline
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
        .collect::<Vec<_>>();
    assert_eq!(baseline_kernels.len(), 2);
    assert_eq!(KDA_BASELINE_SOURCE.matches("KernelContext<'_>").count(), 2);
    for function in baseline_kernels {
        assert_typed_kernel_signature(function);
    }
    assert_no_compatibility_surface("KDA baseline", KDA_BASELINE_SOURCE);
}

#[test]
fn source_is_safe_fixed_shape_rust_without_hip_escape_hatches() {
    let lowercase = SOURCE.to_ascii_lowercase();
    assert!(!SOURCE.contains("unsafe"));
    assert!(!SOURCE.contains("include!"));
    assert_eq!(SOURCE.matches("macro_rules!").count(), 3);
    assert!(
        SOURCE.contains(
            "#[cfg(any(target_arch = \"amdgpu\", test))]\nmacro_rules! decode_fp8_e4m3_v1"
        )
    );
    assert!(
        SOURCE.contains(
            "#[cfg(target_arch = \"amdgpu\")]\nmacro_rules! consider_sparse_candidate_v1"
        )
    );
    assert!(!lowercase.contains("extern \"c\""));
    assert!(!lowercase.contains("hiplaunchkernel"));
    assert!(!lowercase.contains("std::process"));
    for marker in [
        "KDA_STATE_ELEMENTS_V1",
        "PREFILL_TOKENS_V1",
        "ATTENTION_TOKENS_V1",
        "HEAD_DIMENSION_V1",
        "SELECTED_TOKENS_V1",
        "SINKHORN_ITERATIONS_V1",
        "KernelContext<'_>",
        "Global<'_, f32, ReadOnly>",
        "Global<'_, u8, ReadOnly>",
        "DisjointWrite<Index1D>",
        "ExclusiveReadWrite",
        "context.invocation()",
        "context.math()",
        "context.numerical_policy::<StrictIeee>()",
        "device_math.with_numerical_policy(&policy)",
        "context.with_workgroup",
        "workgroup.subgroup::<SubgroupWidth64>()",
        "subgroup.gfx950_wave16(workgroup.epoch())",
        "partition16_reduce_sum_v1",
        "partition16_broadcast_v1",
        "partition4_reduce_sum_v1",
        "fp8_attention_score_v1",
        "GlobalReadView2DF32V1::checked",
        "checked_2d_extent_v1(base, rows, columns, stride, values.len())",
        "batch_count_for_launch_v1(grid.x(), 256)",
        "final_state.store(context.invocation().index_1d().into_disjoint(), state)",
        "kda_chunk_wy_v1!",
    ] {
        assert!(
            SOURCE.contains(marker),
            "missing fixed source marker {marker}"
        );
    }
    for symbol in [
        "gfx950_kda_decode",
        "gfx950_kda_chunkwise_prefill",
        "gfx950_content_sparse_attention",
        "gfx950_deepseek_sparse_attention",
        "gfx950_compressed_hybrid_attention",
        "gfx950_attnres_aggregate",
        "gfx950_four_branch_residual",
        "gfx950_mhc_sinkhorn_mix",
    ] {
        assert_eq!(
            SOURCE.matches(&format!("pub fn {symbol}(")).count(),
            1,
            "{symbol} has more than one ordinary production body"
        );
    }

    assert_no_compatibility_surface("canonical", SOURCE);
    for forbidden in [
        "FE2O3_CODEGEN_PIPELINE",
        "exact transcript",
        "kernel-name selector",
    ] {
        assert!(
            !SOURCE.contains(forbidden),
            "canonical retained {forbidden:?}"
        );
    }
    let admitted = SOURCE
        .split("#[cfg(test)]\nmod tests")
        .next()
        .expect("canonical admitted source");
    assert!(!admitted.contains("reference::"));
}

#[test]
fn launch_derived_batch_counts_cover_grid_tails_and_reject_invalid_shapes() {
    assert_eq!(batch_count_for_launch_v1(1, 256), Some(1));
    assert_eq!(batch_count_for_launch_v1(4, 256), Some(4));
    assert_eq!(batch_count_for_launch_v1(1, 64), Some(4));
    assert_eq!(batch_count_for_launch_v1(3, 64), Some(12));
    assert_eq!(batch_count_for_launch_v1(1, 16), Some(16));
    assert_eq!(batch_count_for_launch_v1(4, 16), Some(64));
    for invalid in [(0, 16), (5, 16), (1, 0), (1, 17), (1, 512)] {
        assert_eq!(batch_count_for_launch_v1(invalid.0, invalid.1), None);
    }
}

#[test]
fn package_states_the_production_source_and_evidence_boundary() {
    assert_eq!(GFX950_ADVANCED_ATTENTION_WORKGROUP_V1, [256, 1, 1]);
    assert_eq!(GFX950_KDA_WORKGROUP_V2, [256, 1, 1]);
    assert_eq!(GFX950_ADVANCED_ATTENTION_GRID_V1, [4, 1, 1]);
    assert!(!GFX950_ADVANCED_ATTENTION_SOURCE_LOWERING_SUPPORTED_V1);
    assert!(GFX950_ADVANCED_ATTENTION_FULLY_TYPED_MEMORY_V1);
    assert!(!GFX950_ADVANCED_ATTENTION_BUNDLE_V8_SUPPORTED_V1);
    assert!(
        LIB_SOURCE.contains("GFX950_ADVANCED_ATTENTION_SOURCE_LOWERING_SUPPORTED_V1: bool = false")
    );
    assert!(GFX950_ADVANCED_ATTENTION_SOURCE_BLOCKER_V1.contains("source path now composes"));
    assert!(GFX950_ADVANCED_ATTENTION_SOURCE_BLOCKER_V1.contains("production V13"));
    assert!(GFX950_ADVANCED_ATTENTION_SOURCE_BLOCKER_V1.contains("Bundle V8"));
    assert!(GFX950_ADVANCED_ATTENTION_SOURCE_BLOCKER_V1.contains("protected publication"));

    for feature in [
        "kernel-kda-decode",
        "kernel-kda-prefill",
        "kernel-content-sparse-attention",
        "kernel-deepseek-sparse-attention",
        "kernel-compressed-hybrid-attention",
        "kernel-attnres-aggregate",
        "kernel-four-branch-residual",
        "kernel-mhc-sinkhorn-mix",
    ] {
        assert!(LIB_SOURCE.contains(feature));
    }

    for variant in [
        "kernel-kda-decode-baseline-v1",
        "kernel-kda-prefill-baseline-v1",
        "kernel-content-sparse-attention-reciprocal-reuse-v1",
        "kernel-deepseek-sparse-attention-leader-exp-v1",
        "kernel-compressed-hybrid-attention-division-baseline-v1",
        "kernel-attnres-aggregate-explicit-reuse-v1",
        "kernel-four-branch-residual-explicit-v1",
        "kernel-mhc-sinkhorn-mix-scalar-v1",
    ] {
        assert!(
            CARGO_MANIFEST.contains(variant),
            "missing feature {variant}"
        );
        assert!(
            LIB_SOURCE.contains(variant) || SOURCE.contains(variant),
            "missing source gate {variant}"
        );
        assert!(
            RUNNER_SOURCE.contains(variant),
            "missing runner case {variant}"
        );
        assert!(
            ABLATION_REGISTRY.contains(variant),
            "missing registry entry {variant}"
        );
    }

    assert!(SOURCE.contains("kernel-content-sparse-attention-reciprocal-reuse-v1"));
    assert!(SOURCE.contains("kernel-compressed-hybrid-attention-division-baseline-v1"));
    for rejected in [
        "content-sparse-selected-score-scalar-v1",
        "compressed-hybrid-seven-score-scalar-v1",
        "attention-lds-double-buffer-v1",
        "mixing-lds-staging-v1",
    ] {
        assert!(
            ABLATION_REGISTRY.contains(rejected),
            "missing rejected variant {rejected}"
        );
    }
    assert!(RUNNER_SOURCE.contains("cd -- \"$ATTEMPT_DIR\""));
    assert!(RUNNER_SOURCE.contains("\"$(basename -- \"$HSACO\")\""));
    assert!(!RUNNER_SOURCE.contains("--mcpu=gfx950 \"$HSACO\""));
}

#[test]
fn performance_audit_covers_every_kernel_without_overclaiming() {
    let audit: serde_json::Value =
        serde_json::from_str(PERFORMANCE_AUDIT).expect("performance audit is valid JSON");
    assert_eq!(
        audit["schema"],
        "fe2o3.gfx950.attention-performance-audit.v1"
    );
    let operators = audit["operators"]
        .as_array()
        .expect("operators is an array");
    assert_eq!(operators.len(), 8);
    for operator in operators {
        assert_eq!(operator["exactly_comparable"], false);
        assert!(operator["public_candidate"].is_string());
        assert!(operator["comparability_reason"].is_string());
        assert!(operator["canonical"]["median_ns"].as_u64().is_some());
        assert!(operator["canonical"]["p95_ns"].as_f64().is_some());
        assert!(operator["ablation"].is_object());
        let floor = operator["resource_floor"]["floor_ns"]
            .as_f64()
            .expect("resource floor is numeric");
        let canonical_median = operator["canonical"]["median_ns"]
            .as_f64()
            .expect("canonical median is numeric");
        let canonical_over_floor = operator["resource_floor"]["canonical_measured_over_floor"]
            .as_f64()
            .expect("canonical resource ratio is numeric");
        assert!((canonical_median / floor - canonical_over_floor).abs() <= 0.001);
        assert!(operator["resource_floor"]["measured_over_floor"].is_null());
        assert!(operator["resource_floor"]["fastest_measured_over_floor"].is_null());
        assert!(operator["model_claim"].is_string());
    }
    assert!(
        audit["claim_boundary"]
            .as_str()
            .is_some_and(|claim| claim.contains("No external SOTA"))
    );
}
