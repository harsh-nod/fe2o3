//! Pre-Cargo host-code policy for authority-bearing kernel compilations.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::pinned_executable::PinnedExecutable;
use crate::project::CargoProject;

const MAX_METADATA_BYTES: usize = 32 * 1024 * 1024;
const MAX_LOCK_BYTES: usize = 8 * 1024 * 1024;
const CRATES_IO_SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";
const TRUSTED_REGISTRY_BUILD_SCRIPTS: [(&str, &str); 15] = [
    ("cap-primitives", "4.0.2"),
    ("io-extras", "0.19.0"),
    ("io-lifetimes", "2.0.4"),
    ("io-lifetimes", "3.0.1"),
    ("libc", "0.2.189"),
    ("num-traits", "0.2.19"),
    ("object", "0.39.1"),
    ("proc-macro2", "1.0.106"),
    ("proc-macro2", "1.0.107"),
    ("quote", "1.0.45"),
    ("quote", "1.0.47"),
    ("rustix", "1.1.4"),
    ("serde_core", "1.0.229"),
    ("serde_json", "1.0.151"),
    ("zmij", "1.0.23"),
];
const TRUSTED_FE2O3_MACROS_FILES: [(&str, &str); 3] = [
    (
        "Cargo.toml",
        "8ea2a249a292e8d64d7ad01a2ea7e2d5d4c7fda1fdea1f247efc4e29caf0bfda",
    ),
    (
        "src/control_flow_v1.rs",
        "42d3140c6fa1b6353b3eb4806927c3c9d539e80d1635355509bf064866178528",
    ),
    (
        "src/lib.rs",
        "64496258d95f1971e5471b7855fbc93a1e8ecf5a0c041c1b6ef0f7f83a5884aa",
    ),
];
const TRUSTED_FE2O3_HIP_SYS_FILES: [(&str, &str); 11] = [
    (
        "Cargo.toml",
        "3cd753eed4ae6fb08908f6a848700bf5cd72d05af9251375b750d28e883ab49f",
    ),
    (
        "build.rs",
        "6716b1d5e20126371f532a6db7ad2ebde2d76fc697b1cf02a6eb589093f62901",
    ),
    (
        "native/cooperative_peer_abi.c",
        "1d926b137fc9fe24fb0f41d71c297049d80fca0a8b46b5b023ce793ccabd19a6",
    ),
    (
        "native/device_properties.c",
        "f53933a71a7f12c294249560762c4aaaedc069fab71b014a313e3638f1311b82",
    ),
    (
        "native/device_properties.h",
        "eb8fa417c9df9fb5772f9d32500fc23acb806b28b1d3869afe22e6d6fcf90238",
    ),
    (
        "native/device_properties_test.c",
        "dfa48a866840449d499b519208e5ce2804a8443ad5697fbfafd24e44804d6687",
    ),
    (
        "native/memory_topology_abi.c",
        "f48a2d2cee7049236fe12e9a08314ce856896c99b0c2083f1432fb4ffd08ab32",
    ),
    (
        "src/cooperative_peer.rs",
        "bdd7a5ee3e46a34ed1090531ea558491160caf05145d1ce40b8cc9938ca96623",
    ),
    (
        "src/lib.rs",
        "77810edeb4b346dd4d0c34cabb9d9527ebe1b9dfdff38d8667b63d4f1969b2b4",
    ),
    (
        "src/memory_topology.rs",
        "aa623132159837ad08cdfae32f276a6881b828f8b24690b1d6528d1d17d64559",
    ),
    (
        "src/unavailable_runtime.rs",
        "95cd2bc145e77c3f8c73741caa44b88feb3210541985af6ae938336d5c23c8c2",
    ),
];

pub(crate) struct AuthorizedKernelClosureV1 {
    snapshot: Vec<u8>,
}

impl AuthorizedKernelClosureV1 {
    pub(crate) fn observe(
        project: &CargoProject,
        args: &[std::ffi::OsString],
        cargo: &PinnedExecutable,
    ) -> Result<Self, String> {
        let mut command = cargo
            .command()
            .map_err(|error| format!("failed to prepare pinned Cargo metadata: {error}"))?;
        command
            .as_command_mut()
            .args(["metadata", "--format-version", "1"])
            .args(project.authority_metadata_args(args)?)
            .current_dir(project.invocation_dir().child_path());
        let output = command
            .output()
            .map_err(|error| format!("failed to run pinned Cargo metadata: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "could not resolve authoritative Cargo closure: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        if output.stdout.is_empty() || output.stdout.len() > MAX_METADATA_BYTES {
            return Err(format!(
                "authoritative Cargo metadata must contain 1 through {MAX_METADATA_BYTES} bytes"
            ));
        }
        let metadata: Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("failed to parse authoritative Cargo metadata: {error}"))?;
        Self::from_metadata(&metadata, args)
    }

    pub(crate) fn snapshot(&self) -> &[u8] {
        &self.snapshot
    }

    fn from_metadata(metadata: &Value, args: &[std::ffi::OsString]) -> Result<Self, String> {
        let packages = metadata
            .get("packages")
            .and_then(Value::as_array)
            .ok_or_else(|| "authoritative Cargo metadata has no package array".to_owned())?;
        let package_by_id = packages
            .iter()
            .map(|package| Ok((required_string(package, "id")?.to_owned(), package)))
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let resolve = metadata
            .get("resolve")
            .and_then(Value::as_object)
            .ok_or_else(|| "authoritative Cargo metadata has no resolved graph".to_owned())?;
        let nodes = resolve
            .get("nodes")
            .and_then(Value::as_array)
            .ok_or_else(|| "authoritative Cargo metadata has no resolved nodes".to_owned())?;
        let dependencies = nodes
            .iter()
            .map(|node| {
                let id = required_string(node, "id")?.to_owned();
                let values = node
                    .get("dependencies")
                    .and_then(Value::as_array)
                    .ok_or_else(|| format!("resolved package {id:?} has no dependency array"))?;
                let values = values
                    .iter()
                    .map(|value| {
                        value.as_str().map(str::to_owned).ok_or_else(|| {
                            format!("resolved package {id:?} has a non-string dependency")
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((id, values))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let roots = selected_roots(metadata, resolve, args, &package_by_id)?;
        let mut pending = roots;
        let mut closure = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !closure.insert(id.clone()) {
                continue;
            }
            let next = dependencies
                .get(&id)
                .ok_or_else(|| format!("selected package {id:?} has no resolved node"))?;
            pending.extend(next.iter().cloned());
        }

        let mut snapshot = b"fe2o3-authorized-kernel-closure-v1\0".to_vec();
        for id in &closure {
            let package = package_by_id
                .get(id)
                .ok_or_else(|| format!("resolved package {id:?} has no metadata record"))?;
            validate_host_code_package(package)?;
            append_field(&mut snapshot, id.as_bytes());
            for field in ["name", "version", "source", "checksum", "manifest_path"] {
                append_field(
                    &mut snapshot,
                    package
                        .get(field)
                        .and_then(Value::as_str)
                        .unwrap_or("-")
                        .as_bytes(),
                );
            }
            let mut next = dependencies
                .get(id)
                .cloned()
                .ok_or_else(|| format!("resolved package {id:?} has no dependency record"))?;
            next.sort();
            for dependency in next {
                append_field(&mut snapshot, dependency.as_bytes());
            }
        }

        let workspace_root = metadata
            .get("workspace_root")
            .and_then(Value::as_str)
            .ok_or_else(|| "authoritative Cargo metadata has no workspace root".to_owned())?;
        let lock_path = Path::new(workspace_root).join("Cargo.lock");
        let lock = fs::read(&lock_path).map_err(|error| {
            format!(
                "cannot read authoritative lockfile {}: {error}",
                lock_path.display()
            )
        })?;
        if lock.is_empty() || lock.len() > MAX_LOCK_BYTES {
            return Err(format!(
                "authoritative lockfile must contain 1 through {MAX_LOCK_BYTES} bytes"
            ));
        }
        append_field(&mut snapshot, &Sha256::digest(&lock));
        Ok(Self { snapshot })
    }
}

fn selected_roots(
    metadata: &Value,
    resolve: &serde_json::Map<String, Value>,
    args: &[std::ffi::OsString],
    packages: &BTreeMap<String, &Value>,
) -> Result<Vec<String>, String> {
    let selected = selected_package_names(args)?;
    if !selected.is_empty() {
        let workspace_members = string_array(metadata, "workspace_members")?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut roots = Vec::new();
        for selected in selected {
            let matches = packages
                .iter()
                .filter(|(id, package)| {
                    workspace_members.contains(*id)
                        && package.get("name").and_then(Value::as_str) == Some(&selected)
                })
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(format!(
                    "authoritative package selection {selected:?} matched {} workspace packages",
                    matches.len()
                ));
            }
            roots.push(matches[0].clone());
        }
        return Ok(roots);
    }
    if args.iter().any(|argument| argument == "--workspace") {
        return string_array(metadata, "workspace_members");
    }
    if let Some(root) = resolve.get("root").and_then(Value::as_str) {
        return Ok(vec![root.to_owned()]);
    }
    string_array(metadata, "workspace_default_members")
}

fn selected_package_names(args: &[std::ffi::OsString]) -> Result<Vec<String>, String> {
    let mut selected = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let value = args[index].to_string_lossy();
        if value == "-p" || value == "--package" {
            index += 1;
            let package = args
                .get(index)
                .and_then(|value| value.to_str())
                .ok_or_else(|| "authoritative --package requires one UTF-8 name".to_owned())?;
            selected.push(package.to_owned());
        } else if let Some(package) = value.strip_prefix("--package=") {
            if package.is_empty() {
                return Err("authoritative --package requires a non-empty name".to_owned());
            }
            selected.push(package.to_owned());
        }
        index += 1;
    }
    selected.sort();
    selected.dedup();
    Ok(selected)
}

fn validate_host_code_package(package: &Value) -> Result<(), String> {
    let name = required_string(package, "name")?;
    let reviewed_hip_sys = is_reviewed_fe2o3_hip_sys(package)?;
    if package.get("links").is_some_and(|value| !value.is_null()) && !reviewed_hip_sys {
        return Err(format!(
            "authoritative kernel closure rejects native links package {name:?}"
        ));
    }
    let targets = package
        .get("targets")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("package {name:?} has no target array"))?;
    let kinds = targets
        .iter()
        .flat_map(|target| {
            target
                .get("kind")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
        })
        .collect::<BTreeSet<_>>();
    if kinds.contains("custom-build")
        && !is_reviewed_registry_build_script(package)
        && !reviewed_hip_sys
    {
        return Err(format!(
            "authoritative kernel closure rejects unreviewed custom-build package {name:?}"
        ));
    }
    if kinds.contains("proc-macro") {
        validate_reviewed_fe2o3_macros(package)?;
    }
    Ok(())
}

fn is_reviewed_fe2o3_hip_sys(package: &Value) -> Result<bool, String> {
    if package.get("name").and_then(Value::as_str) != Some("fe2o3-hip-sys") {
        return Ok(false);
    }
    if package.get("version").and_then(Value::as_str) != Some("0.1.0")
        || package.get("source").is_some_and(|value| !value.is_null())
        || package.get("links").and_then(Value::as_str) != Some("amdhip64")
    {
        return Err(
            "authoritative kernel closure rejects an unreviewed fe2o3-hip-sys package".to_owned(),
        );
    }
    let expected = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-fe2o3 has a workspace crates directory")
        .join("fe2o3-hip-sys");
    let manifest = PathBuf::from(required_string(package, "manifest_path")?);
    if manifest != expected.join("Cargo.toml") {
        return Err(format!(
            "authoritative kernel closure rejects fe2o3-hip-sys from {}",
            manifest.display()
        ));
    }
    validate_reviewed_files(&expected, &TRUSTED_FE2O3_HIP_SYS_FILES, "native build")?;
    Ok(true)
}

fn is_reviewed_registry_build_script(package: &Value) -> bool {
    let Some(name) = package.get("name").and_then(Value::as_str) else {
        return false;
    };
    let Some(version) = package.get("version").and_then(Value::as_str) else {
        return false;
    };
    package.get("source").and_then(Value::as_str) == Some(CRATES_IO_SOURCE)
        && TRUSTED_REGISTRY_BUILD_SCRIPTS.contains(&(name, version))
}

fn validate_reviewed_fe2o3_macros(package: &Value) -> Result<(), String> {
    if package.get("name").and_then(Value::as_str) != Some("fe2o3-macros")
        || package.get("source").is_some_and(|value| !value.is_null())
    {
        return Err(
            "authoritative kernel closure rejects an unreviewed procedural macro".to_owned(),
        );
    }
    let expected = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-fe2o3 has a workspace crates directory")
        .join("fe2o3-macros");
    let manifest = PathBuf::from(required_string(package, "manifest_path")?);
    if manifest != expected.join("Cargo.toml") {
        return Err(format!(
            "authoritative kernel closure rejects fe2o3-macros from {}",
            manifest.display()
        ));
    }
    validate_reviewed_files(&expected, &TRUSTED_FE2O3_MACROS_FILES, "proc-macro")
}

fn validate_reviewed_files(root: &Path, files: &[(&str, &str)], kind: &str) -> Result<(), String> {
    for (relative, expected_digest) in files {
        let path = root.join(relative);
        let bytes = fs::read(&path).map_err(|error| {
            format!(
                "cannot read reviewed proc-macro source {}: {error}",
                path.display()
            )
        })?;
        let digest = hex(&Sha256::digest(&bytes));
        if digest != *expected_digest {
            return Err(format!(
                "reviewed {kind} source digest changed for {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Cargo metadata record has no string {field:?}"))
}

fn string_array(value: &Value, field: &str) -> Result<Vec<String>, String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("Cargo metadata has no {field:?} array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("Cargo metadata {field:?} contains a non-string"))
        })
        .collect()
}

fn append_field(snapshot: &mut Vec<u8>, field: &[u8]) {
    snapshot.extend_from_slice(&(field.len() as u64).to_le_bytes());
    snapshot.extend_from_slice(field);
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[(byte >> 4) as usize]));
        encoded.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(target_kind: &str, package_name: &str, links: Value) -> Value {
        serde_json::json!({
            "packages": [{
                "id": "path+file:///fixture#0.1.0",
                "name": package_name,
                "version": "0.1.0",
                "source": null,
                "checksum": null,
                "manifest_path": "/fixture/Cargo.toml",
                "links": links,
                "targets": [{"kind": [target_kind]}]
            }],
            "resolve": {
                "root": "path+file:///fixture#0.1.0",
                "nodes": [{"id": "path+file:///fixture#0.1.0", "dependencies": []}]
            },
            "workspace_members": ["path+file:///fixture#0.1.0"],
            "workspace_default_members": ["path+file:///fixture#0.1.0"],
            "workspace_root": "/fixture"
        })
    }

    #[test]
    fn host_code_policy_rejects_custom_build_proc_macro_and_native_links() {
        for (kind, name, links, expected) in [
            ("custom-build", "hostile-build", Value::Null, "custom-build"),
            (
                "proc-macro",
                "hostile-macro",
                Value::Null,
                "procedural macro",
            ),
            (
                "lib",
                "hostile-links",
                Value::String("native".into()),
                "native links",
            ),
        ] {
            let record = metadata(kind, name, links);
            let package = &record["packages"][0];
            assert!(
                validate_host_code_package(package)
                    .unwrap_err()
                    .contains(expected)
            );
        }
    }
}
