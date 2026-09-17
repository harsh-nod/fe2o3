//! Read-only derivation of the five affected policy digests from Rust syntax.

use sha2::{Digest, Sha256};
use std::{env, fs, path::Path};
use syn::{Expr, Item, Lit, LitStr, Token, punctuated::Punctuated};

fn string_constant(file: &syn::File, name: &str) -> String {
    let matches: Vec<_> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Const(item) if item.ident == name => Some(item),
            _ => None,
        })
        .collect();
    assert_eq!(matches.len(), 1, "expected one constant {name}");
    match matches[0].expr.as_ref() {
        Expr::Lit(value) => match &value.lit {
            Lit::Str(value) => value.value(),
            _ => panic!("expected string literal for {name}"),
        },
        Expr::Macro(value) if value.mac.path.is_ident("concat") => value
            .mac
            .parse_body_with(Punctuated::<LitStr, Token![,]>::parse_terminated)
            .expect("expected concat of string literals")
            .iter()
            .map(LitStr::value)
            .collect(),
        _ => panic!("unsupported policy constant expression for {name}"),
    }
}

fn main() {
    let mut args = env::args().skip(1);
    let root = args.next().expect("repository root");
    let check = match args.next().as_deref() {
        None => false,
        Some("--check") => true,
        Some(_) => panic!("only --check is supported"),
    };
    assert!(args.next().is_none());
    let manifests = [
        (
            "crates/fe2o3-kfd/src/queue_dispatch_binding.rs",
            "GFX942_AQL_DISPATCH_BINDING",
        ),
        (
            "crates/fe2o3-kfd/src/queue_live.rs",
            "GFX942_COMPUTE_AQL_SESSION",
        ),
        (
            "crates/fe2o3-kfd/src/queue.rs",
            "NATIVE_QUEUE_ADAPTER_FOUNDATION",
        ),
        (
            "crates/fe2o3-kfd/src/semantic_observation.rs",
            "KFD_SEMANTIC_OBSERVATION",
        ),
        (
            "crates/fe2o3-service-host/src/queue.rs",
            "SERVICE_QUEUE_OWNERSHIP",
        ),
    ];
    for (path, prefix) in manifests {
        let source = fs::read_to_string(Path::new(&root).join(path)).unwrap();
        let file = syn::parse_file(&source).unwrap();
        let manifest = string_constant(&file, &format!("{prefix}_MANIFEST_V1"));
        let pinned = string_constant(&file, &format!("{prefix}_MANIFEST_SHA256_V1"));
        let actual = format!("{:x}", Sha256::digest(manifest.as_bytes()));
        println!("{prefix}\tpinned={pinned}\tactual={actual}");
        if check {
            assert_eq!(pinned, actual, "policy digest mismatch: {prefix}");
        }
    }
}
