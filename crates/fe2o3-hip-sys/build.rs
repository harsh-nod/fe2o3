use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=ROCM_PATH");
    println!("cargo:rerun-if-env-changed=HIP_PATH");
    println!("cargo:rerun-if-env-changed=FE2O3_HIP_SYS_DISABLE");
    println!("cargo:rerun-if-changed=native/device_properties.c");
    println!("cargo:rerun-if-changed=native/device_properties.h");
    println!("cargo:rerun-if-changed=native/cooperative_peer_abi.c");
    println!("cargo:rustc-check-cfg=cfg(fe2o3_hip_device_properties)");
    println!("cargo:rustc-check-cfg=cfg(fe2o3_hip_cooperative_peer)");
    println!("cargo:rustc-check-cfg=cfg(fe2o3_hip_runtime)");

    let rocm_path = find_rocm_path();
    if let Some(rocm_path) = &rocm_path {
        let lib_dir = rocm_path.join("lib");
        if lib_dir.is_dir() {
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            println!("cargo:rustc-link-lib=dylib=amdhip64");
            println!("cargo:rustc-cfg=fe2o3_hip_runtime");
        }
    }

    if let Some(include_dir) = rocm_path
        .as_deref()
        .map(|path| path.join("include"))
        .filter(|path| path.join("hip/hip_runtime_api.h").is_file())
    {
        compile_device_properties(&include_dir);
        println!("cargo:rustc-cfg=fe2o3_hip_device_properties");
        if compile_cooperative_peer_abi(&include_dir) {
            println!("cargo:rustc-cfg=fe2o3_hip_cooperative_peer");
        } else {
            println!(
                "cargo:warning=HIP headers do not provide the required cooperative-launch and \
                 peer-access ABI; those entry points will fail closed"
            );
        }
    } else {
        println!(
            "cargo:warning=HIP headers not found; device-property discovery will return \
             HIP_ERROR_NOT_SUPPORTED"
        );
    }
}

fn compile_device_properties(include_dir: &Path) {
    cc::Build::new()
        .file("native/device_properties.c")
        .include(include_dir)
        .include("native")
        .define("__HIP_PLATFORM_AMD__", None)
        .std("c11")
        .warnings(true)
        .warnings_into_errors(true)
        .compile("fe2o3_hip_device_properties");
}

fn compile_cooperative_peer_abi(include_dir: &Path) -> bool {
    cc::Build::new()
        .file("native/cooperative_peer_abi.c")
        .include(include_dir)
        .define("__HIP_PLATFORM_AMD__", None)
        .std("c11")
        .warnings(true)
        .warnings_into_errors(true)
        .try_compile("fe2o3_hip_cooperative_peer_abi")
        .is_ok()
}

fn find_rocm_path() -> Option<PathBuf> {
    if env::var_os("FE2O3_HIP_SYS_DISABLE").is_some() {
        return None;
    }

    for var in ["ROCM_PATH", "HIP_PATH"] {
        if let Ok(value) = env::var(var) {
            let path = PathBuf::from(value);
            if path.join("lib/libamdhip64.so").is_file() {
                return Some(path);
            }
        }
    }

    ["/opt/rocm", "/opt/rocm-7.2.0", "/opt/rocm-7.1.0"]
        .into_iter()
        .map(Path::new)
        .find(|path| path.join("lib/libamdhip64.so").is_file())
        .map(Path::to_path_buf)
}
