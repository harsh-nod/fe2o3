use std::env;
use std::path::{Path, PathBuf};

const NATIVE_HSA_FEATURE_ENV: &str = "CARGO_FEATURE_NATIVE_HSA";
const HARDWARE_TEST_HOOKS_FEATURE_ENV: &str = "CARGO_FEATURE_HARDWARE_TEST_HOOKS";
const REQUIRED_HEADERS: &[&str] = &["hsa/hsa.h", "hsa/hsa_ext_amd.h", "hip/hip_runtime_api.h"];
const REQUIRED_LIBRARIES: &[&str] = &["libhsa-runtime64.so", "libamdhip64.so"];

fn main() {
    println!("cargo:rerun-if-env-changed=ROCM_PATH");
    println!("cargo:rerun-if-env-changed=HIP_PATH");
    println!("cargo:rerun-if-env-changed={NATIVE_HSA_FEATURE_ENV}");
    println!("cargo:rerun-if-env-changed={HARDWARE_TEST_HOOKS_FEATURE_ENV}");
    println!("cargo:rerun-if-changed=native/runtime.c");
    println!("cargo:rerun-if-changed=native/runtime.h");

    if env::var_os(NATIVE_HSA_FEATURE_ENV).is_none() {
        return;
    }

    let rocm = configured_rocm_path();
    let include = rocm.join("include");
    let missing_headers: Vec<_> = REQUIRED_HEADERS
        .iter()
        .map(|relative| include.join(relative))
        .filter(|path| !path.is_file())
        .collect();
    let library_directory = runtime_library_directory(&rocm);
    if !missing_headers.is_empty() || library_directory.is_none() {
        let mut missing: Vec<_> = missing_headers
            .iter()
            .map(|path| path.display().to_string())
            .collect();
        if library_directory.is_none() {
            missing.push(format!(
                "{} or {} containing both {}",
                rocm.join("lib").display(),
                rocm.join("lib64").display(),
                REQUIRED_LIBRARIES.join(" and ")
            ));
        }
        panic!(
            "feature `native-hsa` requires complete ROCm HSA/HIP development files under `{}`; missing: {}. Set ROCM_PATH or HIP_PATH to the ROCm root, or disable `native-hsa` to build the deterministic stub backend",
            rocm.display(),
            missing.join(", ")
        );
    }
    let lib = library_directory.expect("validated ROCm library directory exists");

    let mut native = cc::Build::new();
    native
        .file("native/runtime.c")
        .include(&include)
        .include("native")
        .define("__HIP_PLATFORM_AMD__", None)
        .define("HSA_LARGE_MODEL", None)
        .std("c11")
        .warnings(true)
        .warnings_into_errors(true);
    if env::var_os(HARDWARE_TEST_HOOKS_FEATURE_ENV).is_some() {
        native.define("FE2O3_HSA_NATIVE_TEST_HOOKS", None);
    }
    native.compile("fe2o3_hsa_runtime_abi");
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=dylib=hsa-runtime64");
    println!("cargo:rustc-link-lib=dylib=amdhip64");
}

fn configured_rocm_path() -> PathBuf {
    for variable in ["ROCM_PATH", "HIP_PATH"] {
        if let Some(value) = env::var_os(variable) {
            return PathBuf::from(value);
        }
    }
    PathBuf::from("/opt/rocm")
}

fn runtime_library_directory(rocm: &Path) -> Option<PathBuf> {
    ["lib", "lib64"]
        .into_iter()
        .map(|directory| rocm.join(directory))
        .find(|directory| {
            REQUIRED_LIBRARIES
                .iter()
                .all(|library| directory.join(library).is_file())
        })
}
