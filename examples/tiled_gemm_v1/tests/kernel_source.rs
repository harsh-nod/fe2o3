use fe2o3_device::{Blocked, DisjointSlice, Index1D, KernelMarkerV1};
use fe2o3_tiled_gemm_v1::kernel::{
    __fe2o3_kernel_marker_tiled_gemm_lds_slice1, LDS_SLICE1_OPERAND_BYTES_V1,
    LDS_SLICE1_OPERAND_ELEMENTS_V1, LDS_SLICE1_SOURCE_BLOCKER_V1, LDS_SLICE1_SOURCE_BLOCKERS_V1,
    LDS_SLICE1_SOURCE_LOWERING_SUPPORTED_V1, LDS_SLICE1_SOURCE_TO_IR_SUPPORTED_V1,
    LDS_SLICE1_TOTAL_BYTES_V1, LDS_SLICE1_WORKGROUP_V1,
};
use syn::visit::Visit;

const SOURCE: &str = include_str!("../src/kernel.rs");

#[derive(Default)]
struct BodyCalls {
    functions: Vec<String>,
    methods: Vec<String>,
    macros: Vec<String>,
}

impl<'ast> Visit<'ast> for BodyCalls {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = call.func.as_ref()
            && let Some(segment) = path.path.segments.last()
        {
            self.functions.push(segment.ident.to_string());
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.methods.push(call.method.to_string());
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        if let Some(segment) = invocation.path.segments.last() {
            self.macros.push(segment.ident.to_string());
        }
        syn::visit::visit_macro(self, invocation);
    }
}

fn function<'a>(syntax: &'a syn::File, name: &str) -> &'a syn::ItemFn {
    syntax
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == name => Some(function),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing function `{name}`"))
}

fn calls(function: &syn::ItemFn) -> BodyCalls {
    let mut calls = BodyCalls::default();
    calls.visit_block(&function.block);
    calls
}

#[test]
fn attributed_kernel_and_generated_marker_compile_with_the_exact_abi() {
    type KernelFn = fn(&[u16], &[u16], DisjointSlice<f32, Blocked<Index1D, 16, 4>>);
    let function: KernelFn =
        <__fe2o3_kernel_marker_tiled_gemm_lds_slice1 as KernelMarkerV1>::FUNCTION;
    let _: KernelFn = function;
}

#[test]
fn ordinary_host_invocation_panics_before_mutating_output() {
    type KernelFn = fn(&[u16], &[u16], DisjointSlice<f32, Blocked<Index1D, 16, 4>>);
    let function: KernelFn =
        <__fe2o3_kernel_marker_tiled_gemm_lds_slice1 as KernelMarkerV1>::FUNCTION;
    let a = [0_u16; 256];
    let b = [0_u16; 256];
    let sentinel = f32::from_bits(0x7f7f_ffff);
    let mut output = [sentinel; 256];
    // SAFETY: `output` is live and exclusively borrowed by the view for the
    // duration of the caught invocation.
    let output_view = unsafe {
        DisjointSlice::<f32, Blocked<Index1D, 16, 4>>::from_raw_parts(
            output.as_mut_ptr(),
            output.len(),
        )
    };
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        function(&a, &b, output_view);
    }));
    assert!(failure.is_err());
    assert!(
        output
            .iter()
            .all(|value| value.to_bits() == sentinel.to_bits())
    );
}

#[test]
fn standalone_manifest_separates_host_marker_from_production_source() {
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.contains("default = [\"host-contract\"]"));
    assert!(manifest.contains("host-contract = []"));

    let library = include_str!("../src/lib.rs");
    assert!(library.contains("#[cfg(feature = \"host-contract\")]"));
    assert!(library.contains("#[path = \"kernel_host_contract.rs\"]"));
    assert!(library.contains("#[cfg(not(feature = \"host-contract\"))]"));
    assert!(library.contains("#[path = \"kernel.rs\"]"));

    let host_contract = include_str!("../src/kernel_host_contract.rs");
    assert!(host_contract.contains("#[kernel]"));
    assert!(!host_contract.contains("#[kernel(typed"));
    assert!(!host_contract.contains("namespace"));
    assert!(!host_contract.contains("CRATE_BINDING"));
    assert!(!host_contract.contains("KERNEL_BINDING"));

    let production = syn::parse_file(SOURCE).expect("production kernel source parses as Rust");
    let kernel = function(&production, "tiled_gemm_lds_slice1");
    let attribute = kernel
        .attrs
        .iter()
        .find(|attribute| attribute.path().is_ident("kernel"))
        .expect("production function has a kernel attribute");
    let syn::Meta::List(attribute) = &attribute.meta else {
        panic!("production kernel attribute is not a list");
    };
    let attribute = attribute.tokens.to_string();
    assert!(attribute.contains("typed"));
    assert!(!attribute.contains("namespace"));
}

#[test]
fn fixed_source_contract_reaches_ir_and_still_fails_closed_before_llvm() {
    assert_eq!(LDS_SLICE1_WORKGROUP_V1, [64, 1, 1]);
    assert_eq!(LDS_SLICE1_OPERAND_ELEMENTS_V1, 256);
    assert_eq!(LDS_SLICE1_OPERAND_BYTES_V1, 512);
    assert_eq!(LDS_SLICE1_TOTAL_BYTES_V1, 1024);
    assert!(std::hint::black_box(LDS_SLICE1_SOURCE_TO_IR_SUPPORTED_V1));
    assert!(!std::hint::black_box(
        LDS_SLICE1_SOURCE_LOWERING_SUPPORTED_V1
    ));
    assert_eq!(
        LDS_SLICE1_SOURCE_BLOCKER_V1,
        "the source-to-IR receipt stops before compiler descriptor construction"
    );
    assert_eq!(
        LDS_SLICE1_SOURCE_BLOCKERS_V1,
        [
            LDS_SLICE1_SOURCE_BLOCKER_V1,
            "the authenticated source path is not joined to the dedicated upstream-LLVM LDS lowering",
            "the reviewed source-to-IR correspondence is not a compiler-refinement proof",
            "protected Worker V3 publication custody, HSACO load, and launch remain fail-closed",
        ]
    );
}

#[test]
fn executable_function_body_contains_the_slice1_algorithm() {
    let syntax = syn::parse_file(SOURCE).expect("kernel source parses as Rust");
    let kernel = function(&syntax, "tiled_gemm_lds_slice1");
    assert!(matches!(kernel.vis, syn::Visibility::Public(_)));
    assert!(kernel.sig.unsafety.is_none());
    let attribute = kernel
        .attrs
        .iter()
        .find(|attribute| attribute.path().is_ident("kernel"))
        .expect("ordinary function has a kernel attribute");
    let syn::Meta::List(attribute) = &attribute.meta else {
        panic!("kernel attribute is not a list");
    };
    let attribute = attribute.tokens.to_string();
    assert!(attribute.contains("typed"));
    assert!(!attribute.contains("namespace"));
    assert!(attribute.contains("launch"));
    assert!(attribute.contains("required = [64 , 1 , 1]"));
    assert!(attribute.contains("max = [64 , 1 , 1]"));

    let calls = calls(kernel);
    for required in [
        "index_1d",
        "row_major",
        "current",
        "gfx942_lds_bf16_tile_pair_m16x16_v1",
        "gfx942_publish_lds_bf16_tile_pair_m16x16_v1",
    ] {
        assert!(
            calls.functions.iter().any(|call| call == required),
            "missing `{required}` call"
        );
    }
    for required in [
        "write_mfma_fragment",
        "read_mfma_fragment",
        "load_m16k16",
        "load_k16n16",
        "multiply_accumulate",
        "into_values",
        "checked_block",
        "get_block_mut",
    ] {
        assert!(
            calls.methods.iter().any(|call| call == required),
            "missing `{required}` method"
        );
    }
    assert_eq!(
        calls
            .methods
            .iter()
            .filter(|call| call.as_str() == "get_block_mut")
            .count(),
        4
    );
    assert!(!calls.methods.iter().any(|call| call == "get_mut_at"));
    assert!(!SOURCE.contains("unsafe"));
    assert!(calls.macros.is_empty());
    for forbidden in ["from_raw_parts", "unreachable_unchecked"] {
        assert!(!calls.functions.iter().any(|call| call == forbidden));
        assert!(!calls.methods.iter().any(|call| call == forbidden));
    }
}

#[test]
fn wg64_frontend_contract_is_macro_owned_without_a_handwritten_sidecar() {
    let syntax = syn::parse_file(SOURCE).expect("kernel source parses as Rust");
    assert!(!syntax.items.iter().any(|item| {
        matches!(item, syn::Item::Static(item) if item.ident == "__fe2o3_kernel_frontend_contract_v1_tiled_gemm_lds_slice1")
    }));
    assert!(!syntax.items.iter().any(|item| {
        matches!(item, syn::Item::Const(item) if item.ident == "LDS_SLICE1_FRONTEND_CONTRACT_V1")
    }));
}

#[test]
fn source_has_no_declarative_or_lookalike_kernel_body() {
    let syntax = syn::parse_file(SOURCE).expect("kernel source parses as Rust");
    assert!(!syntax.items.iter().any(|item| {
        matches!(item, syn::Item::Macro(item) if item.mac.path.is_ident("macro_rules"))
    }));

    assert_eq!(
        syntax
            .items
            .iter()
            .filter(|item| matches!(item, syn::Item::Fn(_)))
            .count(),
        1,
        "the attributed kernel body must not delegate to a source lookalike"
    );

    let comment_only =
        syn::parse_file("fn fake() { /* lds.write_mfma_fragment(); sync::syncthreads(); */ }")
            .unwrap();
    let comment_calls = calls(function(&comment_only, "fake"));
    assert!(comment_calls.functions.is_empty());
    assert!(comment_calls.methods.is_empty());
}
