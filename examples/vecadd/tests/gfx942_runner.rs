use std::os::unix::fs::PermissionsExt;
use std::process::Command;

const RUNNER: &str = include_str!("../run-gfx942.sh");
const SHARED_RUNNER: &str = include_str!("../../../scripts/tutorial-hardware-runner.sh");
const LIBRARY: &str = include_str!("../src/lib.rs");
const BINARY: &str = include_str!("../src/main.rs");

#[test]
fn manifest_library_owns_the_only_executable_kernel_definition() {
    let typed_kernels = |source: &str| {
        source
            .lines()
            .filter(|line| line.trim_start().starts_with("#[kernel("))
            .count()
    };
    assert_eq!(typed_kernels(LIBRARY), 1);
    assert_eq!(typed_kernels(BINARY), 0);
    assert!(LIBRARY.contains("include!(\"vecadd_body.rs\")"));
    assert!(LIBRARY.contains("pub fn vecadd("));
    assert!(LIBRARY.contains("reference = vecadd_cpu_reference"));
    assert!(LIBRARY.contains("launch(required = [64, 1, 1], max = [64, 1, 1])"));
    assert!(BINARY.contains("fe2o3_vecadd::simulate_bundle_v8"));
}

#[test]
fn runner_enters_only_the_bounded_production_application_route() {
    let route = format!("{RUNNER}\n{SHARED_RUNNER}");
    for required in [
        "set -euo pipefail",
        "environment=(-i LANG=C LC_ALL=C TZ=UTC FE2O3_TARGET=gfx942)",
        "--kill-after=10",
        "authority release run --locked",
        "--bin \"$binary\" -- --gfx942-qualification",
        "fe2o3_tutorial_hardware_finish gfx942",
    ] {
        assert!(route.contains(required), "missing `{required}`");
    }
    assert!(
        RUNNER
            .contains("fe2o3_tutorial_hardware_authority_run_gfx942 \"$example_dir\" fe2o3-vecadd")
    );
    for forbidden in [
        "run-gfx942-common",
        "engineering hsaco",
        "FE2O3_CRATE_BINDING_ID",
        "FE2O3_VECADD_HSACO",
        "/dev/kfd",
        "load_module",
        "hip",
    ] {
        assert!(!RUNNER.contains(forbidden), "retained `{forbidden}`");
    }
}

#[test]
fn generated_arguments_and_capability_authority_are_explicit() {
    for required in [
        "GeneratedHostReadSliceV1::new(&a)",
        "GeneratedHostReadSliceV1::new(&b)",
        "GeneratedHostWriteSliceV1::new(&mut storage",
        "fe2o3_vecadd::vecadd_gpu::Arguments::new",
        "run_generated_application_v1",
        "prepare_generated_application_invocation_v1",
        "vecadd_cpu_oracle",
        "vecadd_launch_geometry(ELEMENTS, ELEMENTS, ELEMENTS)",
        "publish_tutorial_runtime_semantic_observation_v1",
    ] {
        assert!(BINARY.contains(required), "missing `{required}`");
    }
    for forbidden in [
        "unsafe",
        "authenticate_inherited_worker_v3_capability_application_v1",
        "GeneratedHostMemoryConstraintV2",
        "GeneratedHostAxisConstraintV2",
        "admit_generated_host_contract_v2",
        "open_default_gfx942_device_v1",
        "prepare_capability_direct_kfd_invocation_v2",
        "prepare_generated_gfx942_kfd_invocation_v1",
        "run_generated_gfx942_application_v1",
        "GFX942_RUNNER_BLOCKER",
    ] {
        assert!(!BINARY.contains(forbidden), "retained `{forbidden}`");
    }
    assert!(!BINARY.contains("[64, 1, 1]"));
}

#[test]
fn runner_is_executable_and_rejects_an_unbounded_timeout_before_launch() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("run-gfx942.sh");
    assert_ne!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o111,
        0
    );
    let output = Command::new(path)
        .env("FE2O3_HARDWARE_TIMEOUT_SECONDS", "901")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("integer in 1..900"));
}
