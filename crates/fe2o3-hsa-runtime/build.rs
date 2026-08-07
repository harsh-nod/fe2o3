use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=ROCM_PATH");
    println!("cargo:rerun-if-env-changed=HIP_PATH");
    println!("cargo:rerun-if-env-changed=FE2O3_HSA_RUNTIME_DISABLE");
    println!("cargo:rerun-if-changed=native/runtime.c");
    println!("cargo:rerun-if-changed=native/runtime.h");
    println!("cargo:rustc-check-cfg=cfg(fe2o3_hsa_runtime)");

    let Some(rocm) = find_rocm_path() else {
        println!("cargo:warning=ROCm HSA/HIP development files not found; adapter is unavailable");
        return;
    };
    let include = rocm.join("include");
    let lib = rocm.join("lib");
    if !include.join("hsa/hsa.h").is_file()
        || !include.join("hsa/hsa_ext_amd.h").is_file()
        || !include.join("hip/hip_runtime_api.h").is_file()
        || !lib.join("libhsa-runtime64.so").is_file()
        || !lib.join("libamdhip64.so").is_file()
    {
        println!("cargo:warning=incomplete ROCm HSA/HIP development files; adapter is unavailable");
        return;
    }

    cc::Build::new()
        .file("native/runtime.c")
        .include(&include)
        .include("native")
        .define("__HIP_PLATFORM_AMD__", None)
        .define("HSA_LARGE_MODEL", None)
        .std("c11")
        .warnings(true)
        .warnings_into_errors(true)
        .compile("fe2o3_hsa_runtime_abi");
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=dylib=hsa-runtime64");
    println!("cargo:rustc-link-lib=dylib=amdhip64");
    println!("cargo:rustc-cfg=fe2o3_hsa_runtime");
}

fn find_rocm_path() -> Option<PathBuf> {
    if env::var_os("FE2O3_HSA_RUNTIME_DISABLE").is_some() {
        return None;
    }
    for variable in ["ROCM_PATH", "HIP_PATH"] {
        if let Ok(value) = env::var(variable) {
            let path = PathBuf::from(value);
            if path.join("lib/libhsa-runtime64.so").is_file() {
                return Some(path);
            }
        }
    }
    ["/opt/rocm", "/opt/rocm-7.2.0", "/opt/rocm-7.1.0"]
        .into_iter()
        .map(Path::new)
        .find(|path| path.join("lib/libhsa-runtime64.so").is_file())
        .map(Path::to_path_buf)
}
