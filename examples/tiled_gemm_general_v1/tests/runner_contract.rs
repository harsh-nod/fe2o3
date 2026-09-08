use std::path::Path;
use std::process::Command;

const KERNEL: &str = include_str!("../src/kernel.rs");
const HOST: &str = include_str!("../src/main.rs");
const RUNNER: &str = include_str!("../run-target.sh");
const GFX942: &str = include_str!("../run-gfx942.sh");
const GFX950: &str = include_str!("../run-gfx950.sh");
const MANIFEST: &str = include_str!("../Cargo.toml");

#[test]
fn generic_source_does_not_choose_a_backend_or_schedule() {
    for forbidden in [
        "gfx942",
        "gfx950",
        "Gfx942",
        "Gfx950",
        "amdgcn",
        "AMDGPU",
        "MFMA schedule",
    ] {
        assert!(!KERNEL.contains(forbidden), "kernel contains {forbidden:?}");
    }
}

#[test]
fn backend_wrappers_bind_one_exact_processor() {
    assert!(GFX942.contains("FE2O3_GENERAL_GEMM_TARGET=gfx942"));
    assert!(!GFX942.contains("gfx950"));
    assert!(GFX950.contains("FE2O3_GENERAL_GEMM_TARGET=gfx950"));
    assert!(!GFX950.contains("gfx942"));

    for required in [
        "environment=(-i LANG=C LC_ALL=C TZ=UTC \"FE2O3_TARGET=$processor\")",
        "timeout_seconds",
        "--kill-after=10",
        "authority release \"$subcommand\" --locked",
        "--bin fe2o3-tiled-gemm-general-v1",
        "command+=(-- --qualification)",
        "fe2o3_tutorial_hardware_begin",
        "fe2o3_tutorial_hardware_finish \"$processor\"",
    ] {
        assert!(RUNNER.contains(required), "runner omits {required:?}");
    }
    for forbidden in [
        "FE2O3_GENERAL_GEMM_HSACO",
        "FE2O3_EXTRACT_",
        "RUSTC_WORKSPACE_WRAPPER",
        "load_module",
        "cargo run",
    ] {
        assert!(!RUNNER.contains(forbidden), "runner retained {forbidden:?}");
    }
}

#[test]
fn qualification_requires_the_exact_numerical_policy() {
    assert!(HOST.contains("actual[index].to_bits() != expected[index].to_bits()"));
    assert!(!HOST.contains("tolerance"));
    assert!(HOST.contains("BITWISE PASS"));
}

#[test]
fn advanced_host_uses_only_the_target_neutral_generated_contract() {
    for required in [
        "GeneratedHostReadSliceV1::new(&lhs)",
        "GeneratedHostReadSliceV1::new(&rhs)",
        "GeneratedHostReadWriteSliceV1::new",
        "tiled_gemm_general_v1_gpu::Arguments::new",
        "run_generated_application_v1",
        "prepare_generated_application_invocation_v1",
    ] {
        assert!(HOST.contains(required), "host omits {required:?}");
    }
    for forbidden in [
        "unsafe",
        "GpuContext",
        "DeviceBuffer",
        "KernelParams",
        "load_module",
        "launch_kernel",
        "FE2O3_GENERAL_GEMM_HSACO",
        "FE2O3_GENERAL_GEMM_TARGET",
        "prepare_generated_gfx942",
    ] {
        assert!(!HOST.contains(forbidden), "host retained {forbidden:?}");
    }
    assert!(MANIFEST.contains("generated-gfx950-hip-provider"));
    assert!(!MANIFEST.contains("fe2o3-core"));
}

#[test]
fn generated_path_covers_slices_and_all_manifest_scalar_widths() {
    assert_eq!(HOST.matches("GeneratedHostReadSliceV1::new").count(), 2);
    assert_eq!(
        HOST.matches("GeneratedHostReadWriteSliceV1::new").count(),
        1
    );
    for scalar in [
        "problem.rows",
        "problem.columns",
        "problem.reduction",
        "problem.lhs_stride",
        "problem.rhs_stride",
        "problem.output_stride",
        "problem.product_scale",
        "problem.output_scale",
    ] {
        assert!(HOST.contains(scalar), "missing dynamic scalar {scalar:?}");
    }
}

#[test]
fn shell_entrypoints_parse() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for script in [
        "run-target.sh",
        "run-gfx942.sh",
        "run-gfx950.sh",
        "run-benchmark.sh",
    ] {
        let status = Command::new("bash")
            .arg("-n")
            .arg(root.join(script))
            .status()
            .expect("run bash syntax check");
        assert!(status.success(), "{script} has invalid shell syntax");
    }
}

#[test]
fn host_rejects_unknown_modes_before_device_observation() {
    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-tiled-gemm-general-v1"))
        .arg("--unknown-mode")
        .output()
        .expect("run host parser");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
}
