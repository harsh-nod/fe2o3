//! Deterministic guards for the dialect-only dependency and feature closure.

use std::{
    collections::{BTreeMap, BTreeSet},
    process::Command,
};

use fe2o3_pliron_llvm_smoke::{PLIRON_LLVM_FEATURES, PLIRON_LLVM_LICENSE, PLIRON_REVISION};
use serde_json::Value;

const PLIRON_SOURCE_PREFIX: &str = "git+https://github.com/harsh-nod/pliron.git?rev=";
const PLIRON_PACKAGES: &[&str] = &["pliron", "pliron-derive", "pliron-llvm"];
const FORBIDDEN_AUTHORITY_PACKAGES: &[&str] = &[
    "amd-comgr",
    "amd_comgr",
    "bindgen",
    "cc",
    "clang-sys",
    "cmake",
    "comgr",
    "comgr-sys",
    "llvm-sys",
    "pkg-config",
    "rocm-comgr-sys",
    "vcpkg",
];

fn metadata() -> Value {
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--locked",
            "--offline",
            "--format-version",
            "1",
            "--manifest-path",
            concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
        ])
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid cargo metadata JSON")
}

fn normal_build_closure(metadata: &Value, root: &str) -> BTreeSet<String> {
    let packages = metadata["packages"].as_array().expect("packages array");
    let root_id = packages
        .iter()
        .find(|package| package["name"] == root)
        .and_then(|package| package["id"].as_str())
        .expect("root package");
    let nodes: BTreeMap<&str, &Value> = metadata["resolve"]["nodes"]
        .as_array()
        .expect("resolve nodes")
        .iter()
        .map(|node| (node["id"].as_str().expect("node id"), node))
        .collect();

    let mut pending = vec![root_id.to_owned()];
    let mut closure = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !closure.insert(id.clone()) {
            continue;
        }
        let node = nodes.get(id.as_str()).expect("resolved package node");
        for dependency in node["deps"].as_array().expect("dependency array") {
            let is_normal_or_build = dependency["dep_kinds"]
                .as_array()
                .expect("dependency kinds")
                .iter()
                .any(|kind| kind["kind"].is_null() || kind["kind"] == "build");
            if is_normal_or_build {
                pending.push(
                    dependency["pkg"]
                        .as_str()
                        .expect("dependency package id")
                        .to_owned(),
                );
            }
        }
    }
    closure
}

#[test]
fn dialect_only_feature_and_dependency_closure_is_machine_authority_free() {
    let metadata = metadata();
    let packages = metadata["packages"].as_array().expect("packages array");
    let closure = normal_build_closure(&metadata, env!("CARGO_PKG_NAME"));

    for name in PLIRON_PACKAGES {
        let matches: Vec<_> = packages
            .iter()
            .filter(|package| package["name"] == *name)
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "expected one resolved identity for {name}"
        );
        let package = matches[0];
        assert!(closure.contains(package["id"].as_str().expect("package id")));
        assert_eq!(package["version"], "0.17.0");
        assert_eq!(package["license"], PLIRON_LLVM_LICENSE);
        assert_eq!(
            package["source"].as_str().expect("git source"),
            format!("{PLIRON_SOURCE_PREFIX}{PLIRON_REVISION}#{PLIRON_REVISION}")
        );
    }

    let llvm_package = packages
        .iter()
        .find(|package| package["name"] == "pliron-llvm")
        .expect("pliron-llvm package");
    let llvm_node = metadata["resolve"]["nodes"]
        .as_array()
        .expect("resolve nodes")
        .iter()
        .find(|node| node["id"] == llvm_package["id"])
        .expect("pliron-llvm resolve node");
    assert_eq!(
        llvm_node["features"],
        serde_json::json!(PLIRON_LLVM_FEATURES)
    );

    for package in packages
        .iter()
        .filter(|package| closure.contains(package["id"].as_str().expect("package id")))
    {
        let name = package["name"].as_str().expect("package name");
        assert!(
            !name.to_ascii_lowercase().contains("comgr")
                && !FORBIDDEN_AUTHORITY_PACKAGES.contains(&name),
            "dialect closure resolved forbidden machine authority package {name}"
        );
        assert!(
            package["links"].is_null(),
            "dialect closure acquired native link authority through {name}"
        );
    }
}
