fn atomic_slice_admission_source(declaration: &str, element: &str) -> String {
    format!(
        r#"
#![no_std]
{declaration}
use fe2o3_device::{{kernel, thread, WriteOnlyDisjointSlice}};
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn atomic_slice_admission(values: &[{element}], mut output: WriteOnlyDisjointSlice<u32>) {{
    if values.len() != 128 || output.len() != 128 {{ fe2o3_device::trap(); }}
    if !output.write(thread::index_1d(), 0) {{ fe2o3_device::trap(); }}
}}
"#
    )
}

fn run_atomic_slice_admission(source: &str) -> (std::process::Output, Option<String>) {
    let target = ScratchTarget::new();
    let fixture = materialize_source_safety_fixture(&target, source);
    let llvm = target.path().join("admission.ll");
    let output = Command::new(env!("CARGO"))
        .current_dir(fixture)
        .env("RUSTC_WORKSPACE_WRAPPER", env!("CARGO_BIN_EXE_fe2o3-rustc-extract"))
        .env("FE2O3_EXTRACT_CRATE_V1", "fe2o3_production_source_safety_fixture")
        .env("FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1", &llvm)
        .env("FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2", "55".repeat(32))
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env("CARGO_BUILD_JOBS", "2")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Zinline-mir=yes -Zmir-enable-passes=-JumpThreading -Copt-level=3 -Ctarget-cpu=gfx950 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32")
        .args(["check", "--offline", "-Zbuild-std=core", "--target", "amdgcn-amd-amdhsa", "--target-dir"])
        .arg(target.path().join("cargo"))
        .output().expect("run isolated atomic-slice source admission");
    let llvm = std::fs::read_to_string(llvm).ok();
    (output, llvm)
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD target and production extractor"]
fn shared_atomic_u32_source_admission_has_a_scalar_positive_control() {
    for (declaration, element) in [
        ("", "u32"),
        ("use core::sync::atomic::AtomicU32;", "AtomicU32"),
    ] {
        let (output, llvm) =
            run_atomic_slice_admission(&atomic_slice_admission_source(declaration, element));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            output.status.success(),
            "{element} admission failed:\n{stderr}"
        );
        let llvm = llvm.expect("successful source admission emitted LLVM");
        assert!(llvm.contains("atomic_slice_admission"));
        assert!(llvm.contains("store i32"));
        assert!(!llvm.contains("store atomic"));
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD target and production extractor"]
fn shared_atomic_u32_source_admission_rejects_nominal_and_width_spoofs() {
    for declaration in [
        "#[repr(transparent)] pub struct AtomicU32(core::cell::UnsafeCell<u32>);",
        "type AtomicU32 = core::cell::UnsafeCell<u32>;",
        "type AtomicU32 = core::sync::atomic::AtomicU64;",
    ] {
        let (output, llvm) =
            run_atomic_slice_admission(&atomic_slice_admission_source(declaration, "AtomicU32"));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(!output.status.success(), "spoof admitted: {declaration}");
        assert!(llvm.is_none(), "rejected source emitted LLVM");
        assert!(
            stderr.contains("unsupported shared-slice element type"),
            "wrong frontier:\n{stderr}"
        );
    }
}
