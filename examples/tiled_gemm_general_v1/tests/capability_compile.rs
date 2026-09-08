use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Scratch(PathBuf);

impl Scratch {
    fn new(case: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-general-gemm-{case}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(path.join("src")).expect("create compile fixture");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn device_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/fe2o3-device")
        .canonicalize()
        .expect("canonical fe2o3-device path")
}

fn write_host_stub(scratch: &Scratch) {
    std::fs::create_dir_all(scratch.0.join("host-stub/src")).expect("create host stub");
    std::fs::write(
        scratch.0.join("host-stub/Cargo.toml"),
        "[package]\nname = \"fe2o3-host\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[lib]\npath = \"src/lib.rs\"\n",
    )
    .expect("write host-stub manifest");
    std::fs::write(scratch.0.join("host-stub/src/lib.rs"), "").expect("write host stub source");
}

#[test]
fn attributed_capability_source_typechecks_for_the_device_target() {
    let scratch = Scratch::new("device-compile");
    let target = std::env::var_os("FE2O3_GENERAL_GEMM_COMPILE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| scratch.0.join("target"));
    write_host_stub(&scratch);
    std::fs::write(
        scratch.0.join("Cargo.toml"),
        format!(
            "[package]\nname = \"fe2o3-general-gemm-device-compile\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n\n[dependencies]\nfe2o3-device = {{ path = {:?} }}\n\n[target.'cfg(not(target_arch = \"amdgpu\"))'.dependencies]\nfe2o3-host = {{ path = \"host-stub\" }}\n",
            device_path()
        ),
    )
    .expect("write device compile manifest");
    std::fs::write(
        scratch.0.join("src/lib.rs"),
        format!(
            "#![no_std]\npub mod contract {{ {} }}\npub mod kernel {{ {} }}\n\
             type GeneralKernelFn = fn(\
                 &[u16], &[u16], &mut [f32], \
                 u32, u32, u32, u32, u32, u32, f32, f32,\
             );\n\
             const _: GeneralKernelFn = \
                 <kernel::__fe2o3_kernel_marker_tiled_gemm_general_v1 as fe2o3_device::KernelMarkerV1>::FUNCTION;\n",
            include_str!("../src/contract.rs"),
            include_str!("../src/kernel.rs"),
        ),
    )
    .expect("write exact kernel source");

    for processor in ["gfx942", "gfx950"] {
        let output = Command::new(env!("CARGO"))
            .current_dir(&scratch.0)
            .env("RUSTUP_TOOLCHAIN", "nightly-2026-04-03")
            .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
            .env(
                "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
                format!(
                    "-Ctarget-cpu={processor} -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32"
                ),
            )
            .env_remove("RUSTC")
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .args([
                "check",
                "--offline",
                "-Zbuild-std=core",
                "--target",
                "amdgcn-amd-amdhsa",
                "--target-dir",
            ])
            .arg(target.join(processor))
            .output()
            .expect("check exact GEMM device source");
        assert!(
            output.status.success(),
            "GEMM capability source did not typecheck for {processor}:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn cpu_reference_covers_strides_tails_and_zero_reduction() {
    let scratch = Scratch::new("cpu-reference");
    let target = std::env::var_os("FE2O3_GENERAL_GEMM_REFERENCE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| scratch.0.join("target"));
    std::fs::write(
        scratch.0.join("Cargo.toml"),
        format!(
            "[package]\nname = \"fe2o3-general-gemm-reference-check\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n\n[dependencies]\nfe2o3-device = {{ path = {:?} }}\n",
            device_path()
        ),
    )
    .expect("write reference manifest");
    std::fs::write(
        scratch.0.join("src/contract.rs"),
        include_str!("../src/contract.rs"),
    )
    .expect("write exact numerical contract");
    std::fs::write(
        scratch.0.join("src/reference.rs"),
        include_str!("../src/reference.rs"),
    )
    .expect("write exact CPU reference");
    std::fs::write(
        scratch.0.join("src/lib.rs"),
        r#"
pub mod contract;
pub mod reference;

#[test]
fn invalid_dynamic_contracts_fail_closed() {
    use reference::{ReferenceProblemV1, evaluate_reference_v1};

    let problem = ReferenceProblemV1 {
        rows: 2,
        columns: 3,
        reduction: 4,
        lhs_stride: 3,
        rhs_stride: 3,
        output_stride: 3,
        product_scale: 1.0,
        output_scale: 0.0,
    };
    assert_eq!(
        evaluate_reference_v1(&[0; 8], &[0; 12], &[0.0; 6], problem),
        Err("lhs stride is smaller than the logical reduction extent")
    );

    let short = ReferenceProblemV1 { lhs_stride: 4, ..problem };
    assert_eq!(
        evaluate_reference_v1(&[0; 7], &[0; 12], &[0.0; 6], short),
        Err("reference input is shorter than its declared strided extent")
    );
}
"#,
    )
    .expect("write reference tests");

    let output = Command::new(env!("CARGO"))
        .current_dir(&scratch.0)
        .env("RUSTUP_TOOLCHAIN", "nightly-2026-04-03")
        .env("CARGO_TARGET_DIR", target)
        .env_remove("RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .args(["test", "--offline", "--lib", "--quiet"])
        .output()
        .expect("test exact CPU reference");
    assert!(
        output.status.success(),
        "CPU reference tests failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
