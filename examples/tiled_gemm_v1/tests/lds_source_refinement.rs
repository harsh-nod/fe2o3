use std::io::Write as _;
use std::process::{Command, Stdio};

use syn::visit::Visit;

const KERNEL_SOURCE: &str = include_str!("../src/kernel.rs");
const COMPILER_RECEIPT_SOURCE: &str =
    include_str!("../../../crates/rustc-codegen-fe2o3/src/collected_tiled_gemm_lds_slice1_v1.rs");
const CANONICAL_IR_SOURCE: &str =
    include_str!("../../../crates/fe2o3-kernel-ir/src/tiled_gemm_lds_v1.rs");
const REFINEMENT_PROOF: &str = include_str!("../verus/lds_tiled_slice1_source_refinement.rs");
const LENGTH_MUTATION: &str = include_str!("../verus/negative/lds_source_length_wrong.rs");
const BARRIER_MUTATION: &str =
    include_str!("../verus/negative/lds_source_publish_barrier_wrong.rs");
const OWNER_MUTATION: &str = include_str!("../verus/negative/lds_source_output_owner_wrong.rs");
const IDENTITY_MUTATION: &str =
    include_str!("../verus/negative/lds_source_correspondence_identity_wrong.rs");

fn rust_const(source: &str, name: &str) -> syn::ItemConst {
    let syntax = syn::parse_file(source).expect("binding source parses as Rust");
    syntax
        .items
        .into_iter()
        .find_map(|item| match item {
            syn::Item::Const(item) if item.ident == name => Some(item),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing Rust constant {name}"))
}

fn byte_array_const(source: &str, name: &str) -> Vec<u8> {
    let item = rust_const(source, name);
    let syn::Expr::Array(array) = item.expr.as_ref() else {
        panic!("{name} is not a byte array");
    };
    array
        .elems
        .iter()
        .map(|element| {
            let syn::Expr::Lit(literal) = element else {
                panic!("{name} contains a non-literal byte");
            };
            let syn::Lit::Int(value) = &literal.lit else {
                panic!("{name} contains a non-integer byte");
            };
            value.base10_parse::<u8>().expect("u8 identity byte")
        })
        .collect()
}

fn byte_string_const(source: &str, name: &str) -> Vec<u8> {
    let item = rust_const(source, name);
    let expression = match item.expr.as_ref() {
        syn::Expr::Reference(reference) => reference.expr.as_ref(),
        expression => expression,
    };
    let syn::Expr::Lit(literal) = expression else {
        panic!("{name} is not a byte-string literal");
    };
    let syn::Lit::ByteStr(value) = &literal.lit else {
        panic!("{name} does not reference a byte-string literal");
    };
    value.value()
}

fn string_const(source: &str, name: &str) -> String {
    let item = rust_const(source, name);
    let syn::Expr::Lit(literal) = item.expr.as_ref() else {
        panic!("{name} is not a string literal");
    };
    let syn::Lit::Str(value) = &literal.lit else {
        panic!("{name} is not a string literal");
    };
    value.value()
}

fn sha256_words(input: &[u8]) -> [u64; 4] {
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sha256sum is required by the authenticated Verus runner");
    child
        .stdin
        .as_mut()
        .expect("sha256sum stdin")
        .write_all(input)
        .expect("write SHA-256 input");
    let output = child.wait_with_output().expect("run sha256sum");
    assert!(output.status.success(), "sha256sum failed");
    let output = std::str::from_utf8(&output.stdout).expect("sha256sum UTF-8 output");
    let hex = output
        .split_ascii_whitespace()
        .next()
        .expect("sha256sum digest");
    assert_eq!(hex.len(), 64);
    std::array::from_fn(|word| {
        u64::from_str_radix(&hex[word * 16..(word + 1) * 16], 16).expect("SHA-256 word")
    })
}

fn verus_digest(name: &str) -> [u64; 4] {
    let declaration = format!("pub open spec fn {name}");
    let start = REFINEMENT_PROOF
        .find(&declaration)
        .unwrap_or_else(|| panic!("missing Verus digest {name}"));
    let body = &REFINEMENT_PROOF[start..];
    std::array::from_fn(|word| {
        let marker = format!("word{word}: 0x");
        let value = body
            .find(&marker)
            .map(|offset| &body[offset + marker.len()..])
            .unwrap_or_else(|| panic!("missing {marker} in {name}"));
        let digits = &value[..16];
        assert!(digits.bytes().all(|byte| byte.is_ascii_hexdigit()));
        u64::from_str_radix(digits, 16).expect("Verus SHA-256 word")
    })
}

fn function(source: &str, name: &str) -> syn::ItemFn {
    syn::parse_file(source)
        .expect("algorithm source parses as Rust")
        .items
        .into_iter()
        .find_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == name => Some(function),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing function {name}"))
}

#[derive(Default)]
struct OrderedCalls(Vec<String>);

impl<'ast> Visit<'ast> for OrderedCalls {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = call.func.as_ref()
            && let Some(segment) = path.path.segments.last()
        {
            self.0.push(segment.ident.to_string());
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.0.push(call.method.to_string());
        syn::visit::visit_expr_method_call(self, call);
    }
}

fn ordered_calls(source: &str, name: &str) -> Vec<String> {
    let function = function(source, name);
    let mut calls = OrderedCalls::default();
    calls.visit_block(&function.block);
    calls.0
}

fn assert_subsequence(actual: &[String], expected: &[&str]) {
    let mut cursor = 0;
    for expected in expected {
        cursor += actual[cursor..]
            .iter()
            .position(|actual| actual == expected)
            .unwrap_or_else(|| panic!("missing ordered call {expected}; calls={actual:?}"));
        cursor += 1;
    }
}

#[test]
fn verus_correspondence_digests_bind_the_real_receipt_and_canonical_module() {
    let portable_mir =
        byte_array_const(COMPILER_RECEIPT_SOURCE, "PORTABLE_MIR_SEMANTIC_IDENTITY_V1");
    assert_eq!(portable_mir.len(), 32);
    let portable_words = std::array::from_fn(|word| {
        u64::from_be_bytes(portable_mir[word * 8..(word + 1) * 8].try_into().unwrap())
    });
    assert_eq!(
        verus_digest("source_portable_mir_identity_v1"),
        portable_words
    );

    let correspondence = byte_string_const(COMPILER_RECEIPT_SOURCE, "CORRESPONDENCE_V1");
    assert_eq!(
        verus_digest("reviewed_correspondence_identity_v1"),
        sha256_words(&correspondence)
    );
    assert!(
        correspondence
            .windows("bounded reviewed correspondence only".len())
            .any(|window| window == b"bounded reviewed correspondence only")
    );
    assert!(
        correspondence
            .windows("not a compiler-refinement proof".len())
            .any(|window| window == b"not a compiler-refinement proof")
    );

    let module = string_const(CANONICAL_IR_SOURCE, "TILED_GEMM_LDS_V1_MODULE_ID");
    assert_eq!(module, "fe2o3::tiled_gemm_lds_v1");
    assert_eq!(
        verus_digest("canonical_module_identity_v1"),
        sha256_words(module.as_bytes())
    );
}

#[test]
fn attributed_source_and_exact_ir_have_the_bound_operation_order() {
    let source_calls = ordered_calls(KERNEL_SOURCE, "tiled_gemm_lds_slice1");
    assert_subsequence(
        &source_calls,
        &[
            "gfx942_lds_bf16_tile_pair_m16x16_v1",
            "write_mfma_fragment",
            "write_mfma_fragment",
            "gfx942_publish_lds_bf16_tile_pair_m16x16_v1",
            "read_mfma_fragment",
            "read_mfma_fragment",
            "multiply_accumulate",
            "checked_block",
            "get_block_mut",
            "get_block_mut",
            "get_block_mut",
            "get_block_mut",
        ],
    );
    for (call, expected) in [
        ("write_mfma_fragment", 2),
        ("gfx942_publish_lds_bf16_tile_pair_m16x16_v1", 1),
        ("read_mfma_fragment", 2),
        ("multiply_accumulate", 1),
        ("checked_block", 1),
        ("get_block_mut", 4),
    ] {
        assert_eq!(
            source_calls.iter().filter(|actual| *actual == call).count(),
            expected,
            "source call count for {call}"
        );
    }

    let ir_calls = ordered_calls(CANONICAL_IR_SOURCE, "tiled_gemm_lds_v1_module");
    assert_subsequence(
        &ir_calls,
        &[
            "static_bf16_lds_tile",
            "static_bf16_lds_tile",
            "global_load",
            "global_load",
            "lds_store",
            "lds_store",
            "workgroup_lds_barrier",
            "lds_load",
            "lds_load",
            "multiply_accumulate",
            "global_store",
        ],
    );
    for (call, expected) in [
        ("static_bf16_lds_tile", 2),
        ("global_load", 2),
        ("lds_store", 2),
        ("workgroup_lds_barrier", 1),
        ("lds_load", 2),
        ("multiply_accumulate", 1),
        ("global_store", 1),
    ] {
        assert_eq!(
            ir_calls.iter().filter(|actual| *actual == call).count(),
            expected,
            "canonical IR builder call count for {call}"
        );
    }
}

#[test]
fn proof_surface_pins_obligations_negative_fixtures_and_claim_boundary() {
    for marker in [
        "pub proof fn exact_source_guard_requires_exact_lengths_v1",
        "pub proof fn exact_attributed_source_selects_canonical_identity_v1",
        "pub proof fn attributed_slice1_source_obligations_refine_canonical_ir_v1",
        "all_slice1_global_input_indices_are_bounded_v1",
        "every_a_lds_read_is_initialized_in_same_epoch_v1",
        "every_b_lds_read_is_initialized_in_same_epoch_v1",
        "slice1_barrier_converges_for_all_64_lanes_v1",
        "fixed_tile_c_stores_are_disjoint_v1",
        "not a proof that rustc, LLVM, linking, or emitted machine code refine",
        "grants no descriptor, load, or launch authority",
    ] {
        assert!(REFINEMENT_PROOF.contains(marker), "missing marker {marker}");
    }

    for (source, marker) in [
        (
            LENGTH_MUTATION,
            "mutated_short_a_is_admitted_by_exact_source_guard_v1",
        ),
        (
            BARRIER_MUTATION,
            "mutated_read_at_publish_event_refines_canonical_ir_v1",
        ),
        (
            OWNER_MUTATION,
            "mutated_distinct_source_owners_may_alias_v1",
        ),
        (
            IDENTITY_MUTATION,
            "mutated_portable_mir_identity_refines_canonical_ir_v1",
        ),
    ] {
        assert!(source.contains(marker), "missing negative marker {marker}");
        for shortcut in ["admit(", "assume(", "#[verifier::external_body]"] {
            assert!(!source.contains(shortcut), "forbidden shortcut {shortcut}");
        }
    }
    for shortcut in ["admit(", "assume(", "#[verifier::external_body]"] {
        assert!(
            !REFINEMENT_PROOF.contains(shortcut),
            "forbidden shortcut {shortcut}"
        );
    }
}
