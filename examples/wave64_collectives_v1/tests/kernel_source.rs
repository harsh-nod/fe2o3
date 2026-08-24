use syn::{Item, ItemFn, Meta};

const SOURCE: &str = include_str!("../src/kernel.rs");

fn kernel_function() -> ItemFn {
    let file = syn::parse_file(SOURCE).expect("kernel source parses as ordinary Rust");
    file.items
        .into_iter()
        .find_map(|item| match item {
            Item::Fn(function) if function.sig.ident == "wave64_collectives_v1" => Some(function),
            _ => None,
        })
        .expect("attributed kernel function is present")
}

#[test]
fn source_is_one_ordinary_typed_kernel_without_algorithm_macros() {
    let kernel = kernel_function();
    let kernel_attributes: Vec<_> = kernel
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("kernel"))
        .collect();
    assert_eq!(kernel_attributes.len(), 1);
    let Meta::List(arguments) = &kernel_attributes[0].meta else {
        panic!("kernel attribute must carry the typed contract");
    };
    let tokens = arguments.tokens.to_string();
    assert!(tokens.contains("typed"));
    assert!(tokens.contains("launch"));
    assert!(tokens.contains("64"));
    assert!(!tokens.contains("namespace"));
    assert!(!SOURCE.contains("macro_rules!"));
}

#[test]
fn source_uses_explicit_mask_and_all_three_bounded_collectives() {
    for marker in [
        "active_mask: u64",
        "active_mask & (1_u64 << lane)",
        "wave.reduce_sum(&context, contribution)",
        "wave.inclusive_scan_sum(&context, contribution)",
        "wave.exclusive_scan_sum(&context, contribution)",
        "if active { reduction } else { 0.0 }",
        "if active { inclusive } else { 0.0 }",
        "if active { exclusive } else { 0.0 }",
    ] {
        assert!(SOURCE.contains(marker), "missing source contract {marker}");
    }
    assert!(SOURCE.contains("WaveLane::<Wave64>::current()"));
    assert!(SOURCE.contains("Gfx942Collectives::current()"));
    assert!(!SOURCE.contains("unsafe"));
}

#[test]
fn source_has_no_linker_or_comgr_escape_hatch() {
    let lowercase = SOURCE.to_ascii_lowercase();
    assert!(!lowercase.contains("comgr"));
    assert!(!lowercase.contains("command::new"));
    assert!(!lowercase.contains("std::process"));
}
