use std::collections::BTreeSet;

use fe2o3_flash_attention_v1::{
    FLASH_ATTENTION_KERNEL_NAMESPACE_V1, FLASH_ATTENTION_KERNEL_SOURCE_SHA256_V1,
    FLASH_ATTENTION_PROFILE_IDENTITY_V1, validate_kernel_source_identity_v1,
};
use sha2::{Digest, Sha256};
use syn::{Expr, ExprLit, Item, ItemFn, Lit, Meta, visit::Visit};

const SOURCE: &str = include_str!("../src/kernel.rs");

fn kernel_functions() -> Vec<ItemFn> {
    syn::parse_file(SOURCE)
        .expect("kernel source parses as ordinary Rust")
        .items
        .into_iter()
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
        .collect()
}

#[test]
fn source_is_exactly_one_ordinary_attributed_kernel() {
    let file = syn::parse_file(SOURCE).unwrap();
    let kernels = kernel_functions();
    assert_eq!(kernels.len(), 1);
    assert_eq!(
        kernels[0].sig.ident,
        "flash_attention_causal_f32_b1_h1_n8_d16_v1"
    );
    assert!(!file.items.iter().any(|item| matches!(item, Item::Macro(_))));
    assert!(!SOURCE.contains("include!"));
}

#[test]
fn attribute_uses_compiler_binding_and_exact_wave64_launch() {
    let kernel = kernel_functions().pop().unwrap();
    let attribute = kernel
        .attrs
        .iter()
        .find(|attribute| attribute.path().is_ident("kernel"))
        .unwrap();
    let Meta::List(arguments) = &attribute.meta else {
        panic!("kernel attribute must carry the typed contract");
    };
    let tokens = arguments.tokens.to_string();
    assert!(tokens.contains("typed"));
    assert!(!tokens.contains("namespace"));
    assert!(tokens.contains("required = [64 , 1 , 1]"));
    assert!(tokens.contains("max = [64 , 1 , 1]"));
}

#[test]
fn profile_namespace_is_the_sha256_of_the_canonical_identity() {
    let digest = Sha256::digest(FLASH_ATTENTION_PROFILE_IDENTITY_V1.as_bytes());
    assert_eq!(format!("{digest:x}"), FLASH_ATTENTION_KERNEL_NAMESPACE_V1);
}

#[test]
fn exact_kernel_source_bytes_are_identity_bound_and_mutations_are_rejected() {
    let digest = Sha256::digest(SOURCE.as_bytes());
    let actual = format!("{digest:x}");
    assert_eq!(actual, FLASH_ATTENTION_KERNEL_SOURCE_SHA256_V1);
    assert_eq!(validate_kernel_source_identity_v1(&actual), Ok(()));

    let mut wrong = actual.into_bytes();
    wrong[0] = if wrong[0] == b'0' { b'1' } else { b'0' };
    let wrong = String::from_utf8(wrong).unwrap();
    assert!(validate_kernel_source_identity_v1(&wrong).is_err());
}

struct CallCollector {
    names: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for CallCollector {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = node.func.as_ref()
            && let Some(segment) = path.path.segments.last()
        {
            self.names.insert(segment.ident.to_string());
        }
        syn::visit::visit_expr_call(self, node);
    }
}

#[test]
fn source_contains_online_attention_without_external_linker_escape_hatches() {
    for marker in [
        "while key_row <= query_row",
        "running_max",
        "running_sum",
        "previous_weight",
        "current_weight",
        "math.exp_f32",
        "numerator[0] / running_sum",
        "lane_index.checked_block::<1, 2>()",
        "output.get_block_mut(&output_block, 0)",
        "output.get_block_mut(&output_block, 1)",
    ] {
        assert!(SOURCE.contains(marker), "missing algorithm marker {marker}");
    }

    let lowercase = SOURCE.to_ascii_lowercase();
    assert!(!lowercase.contains("comgr"));
    assert!(!lowercase.contains("command::new"));
    assert!(!lowercase.contains("std::process"));
    assert!(!SOURCE.contains("get_mut_at"));
    let words: BTreeSet<_> = lowercase
        .split(|character: char| !character.is_ascii_alphanumeric())
        .collect();
    assert!(!words.contains("cuda"));
    assert!(!words.contains("hip"));
}

#[test]
fn source_has_no_string_selected_kernel_or_hidden_generated_body() {
    let file = syn::parse_file(SOURCE).unwrap();
    let mut collector = CallCollector {
        names: BTreeSet::new(),
    };
    collector.visit_file(&file);
    assert!(!collector.names.contains("include_str"));
    assert!(!collector.names.contains("include_bytes"));

    for item in file.items {
        if let Item::Const(item) = item
            && let Expr::Lit(ExprLit {
                lit: Lit::Str(value),
                ..
            }) = item.expr.as_ref()
        {
            assert!(!value.value().contains("kernel"));
        }
    }
}
