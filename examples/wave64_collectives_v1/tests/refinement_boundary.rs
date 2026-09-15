use sha2::{Digest as _, Sha256};

const LIBRARY: &str = include_str!("../src/lib.rs");
const RETIREMENT: &str = include_str!("../historical/README.md");
const ADAPTER: &[u8] = include_bytes!("../historical/source_kir_refinement_v1.rs.txt");
const CASES: &[u8] = include_bytes!("../historical/source_kir_refinement_v1_tests.rs.txt");

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn retired_adapter_and_cases_remain_exact_historical_artifacts() {
    assert_eq!(
        digest(ADAPTER),
        "4a72320247f4b7433a58a982166c36b75ed70ff140fdec010658e2f3ce49a63a"
    );
    assert_eq!(
        digest(CASES),
        "10831fdb25f252572bfb42ec728f3545078baa3b072d8f2d189343599a02b1b3"
    );
    assert_eq!(
        std::str::from_utf8(ADAPTER)
            .unwrap()
            .matches("#[test]")
            .count(),
        2
    );
    assert_eq!(
        std::str::from_utf8(CASES)
            .unwrap()
            .matches("#[test]")
            .count(),
        9
    );
    assert!(RETIREMENT.contains("11 retired Rust cases are not current passing coverage"));
}

#[test]
fn live_crate_has_no_retired_module_export_or_direct_compiler_profile_dependency() {
    let syntax = syn::parse_file(LIBRARY).unwrap();
    assert!(!syntax.items.iter().any(|item| {
        matches!(item, syn::Item::Mod(module) if module.ident == "source_kir_refinement")
    }));
    for item in &syntax.items {
        if let syn::Item::Use(import) = item {
            let spelling = quote::quote!(#import).to_string();
            assert!(!spelling.contains("source_kir_refinement"));
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(!root.join("src/source_kir_refinement.rs").exists());
    assert!(!root.join("tests/source_kir_refinement.rs").exists());
    assert!(
        !include_str!("../Cargo.toml")
            .lines()
            .any(|line| line.starts_with("fe2o3-kernel-ir ="))
    );
}

#[test]
fn current_qualification_stays_unsupported_and_old_schema_is_not_rebound() {
    for marker in [
        "Current source-to-KIR qualification: unsupported.",
        "No current source-to-KIR receipt is available.",
        "da2722bd3ce349228644300b13bb45d4683d1ebd60f8b7749e7764ec6569e894",
        "not the identity of the current general Kernel IR",
        "No feature or environment selector",
    ] {
        assert!(
            RETIREMENT.contains(marker),
            "missing retirement boundary {marker}"
        );
    }
    assert!(
        include_str!("../README.md")
            .contains("Current source-to-KIR qualification remains unsupported")
    );
}
