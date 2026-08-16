use std::{fs, hint::black_box};

use fe2o3_moe_expert_v1::{
    EXACT_MOE_EXPERT_PROFILE_V1, MOE_EXPERT_COMBINE_GRID_V1, MOE_EXPERT_EXECUTION_SUPPORTED_V1,
    MOE_EXPERT_SOURCE_BLOCKER_V1, MOE_EXPERT_SOURCE_TO_IR_SUPPORTED_V1, MOE_ROUTE_WEIGHT_POLICY_V1,
};
use syn::{Attribute, Item, ItemFn, Meta};

fn kernel_functions(source: &str) -> Vec<ItemFn> {
    syn::parse_file(source)
        .unwrap()
        .items
        .into_iter()
        .filter_map(|item| match item {
            Item::Fn(function) if function.attrs.iter().any(is_kernel) => Some(function),
            _ => None,
        })
        .collect()
}

fn is_kernel(attribute: &Attribute) -> bool {
    matches!(&attribute.meta, Meta::List(list) if list.path.is_ident("kernel"))
}

#[test]
fn source_contains_two_ordinary_attributed_kernels_without_macro_facades() {
    let source = fs::read_to_string("src/kernel.rs").unwrap();
    let functions = kernel_functions(&source);
    assert_eq!(functions.len(), 2);
    assert_eq!(
        functions
            .iter()
            .map(|function| function.sig.ident.to_string())
            .collect::<Vec<_>>(),
        [
            "moe_expert_gemm_bf16_m16_n16_k16_v1",
            "moe_expert_combine_f32_t8_k2_o16_v1",
        ]
    );
    assert!(
        !source
            .lines()
            .any(|line| line.trim_start().starts_with("macro_rules!"))
    );
    assert!(source.contains("gfx942_lds_bf16_tile_pair_m16x16_v1"));
    assert!(source.contains("DeviceMatrix::from_compiler"));
    assert!(source.contains("inverse[route]"));
    assert!(source.contains("route_weights[route]"));
}

#[test]
fn exact_profile_and_route_weight_order_are_explicit() {
    assert_eq!(EXACT_MOE_EXPERT_PROFILE_V1.tokens, 8);
    assert_eq!(EXACT_MOE_EXPERT_PROFILE_V1.experts, 4);
    assert_eq!(EXACT_MOE_EXPERT_PROFILE_V1.routes_per_token, 2);
    assert_eq!(EXACT_MOE_EXPERT_PROFILE_V1.capacity, 4);
    assert_eq!(EXACT_MOE_EXPERT_PROFILE_V1.input_width, 16);
    assert_eq!(EXACT_MOE_EXPERT_PROFILE_V1.output_width, 16);
    assert_eq!(EXACT_MOE_EXPERT_PROFILE_V1.tile_rows, 16);
    assert_eq!(MOE_EXPERT_COMBINE_GRID_V1, [2, 1, 1]);
    assert!(MOE_ROUTE_WEIGHT_POLICY_V1.contains("token-major rank-minor route-ID order"));
    assert!(MOE_ROUTE_WEIGHT_POLICY_V1.contains("without renormalization"));
}

#[test]
fn source_authority_remains_fail_closed() {
    assert!(!black_box(MOE_EXPERT_SOURCE_TO_IR_SUPPORTED_V1));
    assert!(!black_box(MOE_EXPERT_EXECUTION_SUPPORTED_V1));
    assert!(MOE_EXPERT_SOURCE_BLOCKER_V1.contains("no authenticated MIR-to-Kernel-IR"));
}
