use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard, OnceLock};

#[path = "support/cargo_fe2o3.rs"]
mod cargo_fe2o3;

#[path = "support/artifact_path_guard.rs"]
mod artifact_path_guard;

fn backend_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn cargo_target_directory(workspace: &Path) -> PathBuf {
    let Some(configured) = std::env::var_os("CARGO_TARGET_DIR") else {
        return workspace.join("target");
    };
    let configured = PathBuf::from(configured);
    if configured.is_absolute() {
        configured
    } else {
        workspace.join(configured)
    }
}

fn genuine_build(workspace: &Path, target: &str, retained_llvm: Option<&Path>) -> Output {
    let mut command = cargo_fe2o3::qualification_command(workspace);
    command
        .current_dir(workspace)
        .args([
            "build",
            "-p",
            "fe2o3-half-math-compiler-fixture",
            "--bin",
            "tiled_gemm_frontend_v1",
        ])
        .env("FE2O3_TARGET", target)
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "kernel-ir-v1");
    if let Some(directory) = retained_llvm {
        command.env("FE2O3_TEST_RETAIN_TILED_GEMM_FRONTEND_LLVM_DIR", directory);
    }
    command
        .output()
        .expect("run genuine tiled GEMM frontend fixture")
}

fn provider_impostor_build(workspace: &Path, package: &str, managed_target: &Path) -> Output {
    let fixture =
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/tiled-gemm-provider-impostor");
    cargo_fe2o3::qualification_command(workspace)
        .current_dir(workspace)
        .args(["build", "--locked", "--manifest-path"])
        .arg(fixture.join("Cargo.toml"))
        .args(["-p", package, "--target-dir"])
        .arg(managed_target)
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "kernel-ir-v1")
        .output()
        .expect("compile external matrix provider impostor")
}

fn probe_real_gfx942_hsaco(imported_llvm: &Path, output_directory: &Path) {
    if std::env::var_os("FE2O3_TEST_REAL_GFX942_MATRIX_HSACO").is_none() {
        return;
    }
    let hsaco = output_directory.join("tiled_gemm_frontend_v1.probe.hsaco");
    let compile = Command::new("/opt/rocm/llvm/bin/clang")
        .args([
            "-x",
            "ir",
            "--target=amdgcn-amd-amdhsa",
            "-mcpu=gfx942:xnack-",
            "-mcode-object-version=6",
            "-nogpulib",
        ])
        .arg(imported_llvm)
        .arg("-o")
        .arg(&hsaco)
        .output()
        .expect("compile retained matrix LLVM with ROCm clang");
    assert!(
        compile.status.success(),
        "ROCm clang rejected retained matrix LLVM:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let readobj = Command::new("/opt/rocm/llvm/bin/llvm-readobj")
        .args(["--notes", "--symbols"])
        .arg(&hsaco)
        .output()
        .expect("inspect real matrix HSACO");
    assert!(
        readobj.status.success(),
        "llvm-readobj rejected real matrix HSACO:\n{}",
        String::from_utf8_lossy(&readobj.stderr)
    );
    let metadata = String::from_utf8_lossy(&readobj.stdout);
    for required in [
        "Format: elf64-amdgpu",
        "amdhsa.target:   'amdgcn-amd-amdhsa--gfx942:xnack-'",
        ".kernarg_segment_align: 8",
        ".kernarg_segment_size: 288",
        ".max_flat_workgroup_size: 64",
        ".reqd_workgroup_size:",
        ".wavefront_size: 64",
        ".symbol:         tiled_gemm_frontend_v1.kd",
    ] {
        assert!(
            metadata.contains(required),
            "real HSACO omitted `{required}`:\n{metadata}"
        );
    }
    for (index, offset, size) in [
        (0, 0, 2),
        (1, 2, 2),
        (2, 4, 2),
        (3, 6, 2),
        (4, 8, 2),
        (5, 10, 2),
        (6, 12, 2),
        (7, 14, 2),
        (8, 16, 4),
        (9, 20, 4),
        (10, 24, 4),
        (11, 28, 4),
    ] {
        let name = format!(".name:           arg{index}");
        let start = metadata
            .find(&name)
            .unwrap_or_else(|| panic!("real HSACO omitted `{name}`:\n{metadata}"));
        let argument = &metadata[start..metadata.len().min(start + 180)];
        assert!(
            argument.contains(&format!(".offset:         {offset}"))
                && argument.contains(&format!(".size:           {size}"))
                && argument.contains(".value_kind:     by_value"),
            "real HSACO argument {index} drifted:\n{argument}"
        );
    }
    assert!(metadata.contains(".offset:         32"));
    assert!(metadata.contains(".value_kind:     hidden_block_count_x"));
    for forbidden in ["DeviceMatrix", "fe2o3_device", "panic"] {
        assert!(
            !metadata.contains(forbidden),
            "real HSACO retained source stub marker `{forbidden}`:\n{metadata}"
        );
    }
}

#[test]
fn same_name_dependency_alias_and_hostile_rust_abis_fail_at_provider_binding() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let managed_target = std::env::temp_dir().join(format!(
        "fe2o3-matrix-provider-impostor-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&managed_target);
    let built = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args([
            "build",
            "--locked",
            "-p",
            "rustc-codegen-fe2o3",
            "--features",
            "qualification-oracles-test-only",
        ])
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .output()
        .expect("build codegen backend");
    assert!(
        built.status.success(),
        "backend build failed:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );

    for package in [
        "matrix-provider-impostor-exact",
        "matrix-provider-impostor-layout",
        "matrix-provider-impostor-fnabi",
    ] {
        let rejected = provider_impostor_build(&workspace, package, &managed_target);
        let stderr = String::from_utf8_lossy(&rejected.stderr);
        assert!(
            !rejected.status.success(),
            "external provider impostor `{package}` unexpectedly reached codegen"
        );
        assert!(
            stderr.contains(
                "trusted-provider rejection: diagnostic item `fe2o3_device_matrix_context_current_v1`"
            ) && stderr.contains("is outside the reviewed fe2o3-device source root"),
            "external provider impostor `{package}` missed the reviewed provider boundary:\n{stderr}"
        );
        assert!(
            !stderr.contains("selected kernel-ir-v1: verified"),
            "external provider impostor `{package}` acquired verified Kernel IR authority:\n{stderr}"
        );
    }
    std::fs::remove_dir_all(managed_target).expect("remove isolated provider-impostor target");
}

#[test]
fn genuine_matrix_items_reach_verified_ir_and_local_markers_fail_closed() {
    let _lock = backend_test_lock();
    let workspace = workspace();

    let retained = cargo_target_directory(&workspace).join(format!(
        "rustc-codegen-fe2o3-test-output/tiled-gemm-retained-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&retained);
    let exact = genuine_build(&workspace, "gfx942:xnack-", Some(&retained));
    let stderr = String::from_utf8_lossy(&exact.stderr);
    assert!(!exact.status.success());
    assert!(
        stderr.contains("selected kernel-ir-v1: verified 1 kernel(s), 1 function(s)"),
        "genuine matrix call did not reach verified kernel IR:\n{stderr}"
    );
    assert!(
        stderr.contains(
            "authenticated workgroup size WorkgroupSize { x: 64, y: 1, z: 1 } conflicts with the exact 256x1x1 executable profile"
        ),
        "genuine matrix call missed the executable-profile preflight boundary:\n{stderr}"
    );
    assert!(!stderr.contains("has no classified trusted device identity"));
    assert!(!stderr.contains("MIR is unavailable for a device-reachable item"));
    let retained_file = retained.join("tiled_gemm_frontend_v1.imported.gfx942-xnack-.ll");
    let llvm = std::fs::read_to_string(&retained_file).unwrap_or_else(|error| {
        panic!(
            "read retained imported LLVM observation {}: {error}\n{stderr}",
            retained_file.display()
        )
    });
    assert!(
        llvm.contains("TEST-ONLY RUSTC IMPORT OBSERVATION; NO ARTIFACT OR EXECUTION AUTHORITY")
    );
    assert!(llvm.contains("llvm.amdgcn.mfma.f32.16x16x16bf16.1k"));
    assert!(llvm.contains("-wavefrontsize32,+wavefrontsize64,-xnack"));
    assert!(llvm.contains("\"fp-contract\"=\"off\""));
    assert!(llvm.contains(dialect_amdgcn::GFX942_XNACK_MINUS_DATA_LAYOUT));
    assert!(llvm.contains("fe2o3.projected-kernarg-policy.v1 sha256="));
    assert!(llvm.contains(
        "fe2o3.projected-kernarg explicit-size=32 implicit-bytes=256 segment-size=288 segment-align=8 source=compiler-policy-not-rustc-observation"
    ));
    for (index, (source, lane, kind, offset, size, alignment)) in [
        (0, 0, "bf16", 0, 2, 2),
        (0, 1, "bf16", 2, 2, 2),
        (0, 2, "bf16", 4, 2, 2),
        (0, 3, "bf16", 6, 2, 2),
        (1, 0, "bf16", 8, 2, 2),
        (1, 1, "bf16", 10, 2, 2),
        (1, 2, "bf16", 12, 2, 2),
        (1, 3, "bf16", 14, 2, 2),
        (2, 0, "f32", 16, 4, 4),
        (2, 1, "f32", 20, 4, 4),
        (2, 2, "f32", 24, 4, 4),
        (2, 3, "f32", 28, 4, 4),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(
            llvm.contains(&format!(
                "fe2o3.projected-kernarg.param index={index} source={source} lane={lane} type={kind} offset={offset} size={size} align={alignment}"
            )),
            "missing kernarg parameter {index} at offset {offset}:\n{llvm}"
        );
    }
    for forbidden in ["DeviceMatrix", "fe2o3_device", "panic", "unreachable"] {
        assert!(
            !llvm.contains(forbidden),
            "retained panic stub marker `{forbidden}`:\n{llvm}"
        );
    }
    for forbidden_claim in [
        fe2o3_kernel_ir::MATRIX_SOURCE_ABI_OBSERVATION_NAMESPACE_V2,
        "observed-source-abi",
        "authenticated-source-abi",
    ] {
        assert!(
            !llvm.contains(forbidden_claim),
            "retained LLVM promoted source observation into a generic dialect claim `{forbidden_claim}`:\n{llvm}"
        );
    }
    probe_real_gfx942_hsaco(&retained_file, &retained);
    let _ = std::fs::remove_dir_all(&retained);

    let wrong_target = genuine_build(&workspace, "gfx942:xnack+", None);
    let stderr = String::from_utf8_lossy(&wrong_target.stderr);
    assert!(!wrong_target.status.success());
    assert!(
        stderr.contains("requires the exact gfx942:xnack- one-wave 64x1x1 kernel context"),
        "wrong target did not fail at the matrix context gate:\n{stderr}"
    );
    assert!(!stderr.contains("selected kernel-ir-v1: verified"));

    let built = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args([
            "build",
            "--locked",
            "-p",
            "rustc-codegen-fe2o3",
            "--features",
            "qualification-oracles-test-only",
        ])
        .output()
        .expect("build codegen backend");
    assert!(
        built.status.success(),
        "backend build failed:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = cargo_target_directory(&workspace).join(format!(
        "rustc-codegen-fe2o3-test-output/tiled-gemm-local-spoof-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&output);
    std::fs::create_dir_all(&output).expect("create local-spoof output directory");
    let mut spoof_command =
        Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    spoof_command
        .current_dir(&workspace)
        .arg("crates/rustc-codegen-fe2o3/tests/fixtures/tiled-gemm-local-marker-spoof.rs")
        .args(["--edition=2024", "--crate-type=lib", "--crate-name"])
        .arg("tiled_gemm_local_marker_spoof")
        .arg(format!(
            "-Zcodegen-backend={}",
            cargo_target_directory(&workspace)
                .join("debug/librustc_codegen_fe2o3.so")
                .display()
        ))
        .arg("-o")
        .arg(output.join("libtiled_gemm_local_marker_spoof.rlib"))
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_QUALIFICATION_ORACLE_V1", "kernel-ir-v1")
        .env("FE2O3_HSACO_DIR", output.join("artifacts"));
    artifact_path_guard::configure_command(
        &mut spoof_command,
        &output,
        "tiled GEMM local marker spoof",
    );
    let spoof = spoof_command
        .output()
        .expect("compile local matrix marker spoof");
    let _ = std::fs::remove_dir_all(&output);
    let stderr = String::from_utf8_lossy(&spoof.stderr);
    assert!(!spoof.status.success());
    assert!(
        stderr.contains(
            "trusted-provider rejection: diagnostic item `fe2o3_device_matrix_context_current_v1`"
        ) && stderr.contains("provider is the local compilation crate"),
        "local marker spoof missed the exact trusted-provider boundary:\n{stderr}"
    );
    assert!(!stderr.contains("selected kernel-ir-v1: verified"));
}
