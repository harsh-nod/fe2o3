use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

const PIPELINE: &str = "collected-executable-scalar-control-flow-v2";
const FIXTURE: &str = include_str!("fixtures/executable-scalar-control-flow-v1.rs");
static NEXT_OUTPUT: AtomicU64 = AtomicU64::new(0);
static BACKEND: OnceLock<PathBuf> = OnceLock::new();

struct TestOutputDir(PathBuf);

impl TestOutputDir {
    fn new(workspace: &Path) -> Self {
        let path = workspace.join(format!(
            "target/rustc-codegen-fe2o3-test-output/collected-scalar-cf-{}-{}",
            std::process::id(),
            NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale scalar-control-flow output");
        }
        std::fs::create_dir_all(path.join("artifacts")).expect("create scalar-control-flow output");
        Self(path)
    }
}

impl Drop for TestOutputDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn build_backend(workspace: &Path) -> PathBuf {
    BACKEND
        .get_or_init(|| {
            let output = Command::new(env!("CARGO"))
                .current_dir(workspace)
                .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
                .output()
                .expect("build rustc backend");
            assert!(
                output.status.success(),
                "backend build failed\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            workspace.join("target/debug/librustc_codegen_fe2o3.so")
        })
        .clone()
}

fn compile(
    workspace: &Path,
    backend: &Path,
    output: &TestOutputDir,
    source: &str,
    target: &str,
    pipeline: &str,
) -> Output {
    let source_path = output.0.join("fixture.rs");
    std::fs::write(&source_path, source).expect("write scalar-control-flow fixture");
    compile_path(
        workspace,
        backend,
        output,
        &source_path,
        target,
        pipeline,
        &[],
    )
}

fn compile_path(
    workspace: &Path,
    backend: &Path,
    output: &TestOutputDir,
    source_path: &Path,
    target: &str,
    pipeline: &str,
    extra_args: &[&str],
) -> Output {
    let mut command = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    command
        .current_dir(workspace)
        .arg(source_path)
        .args([
            "--edition=2024",
            "--crate-name",
            "fe2o3_scalar_control_flow_v1_fixture",
            "-C",
            "overflow-checks=off",
            "-Zmir-enable-passes=-JumpThreading",
        ])
        .args(extra_args)
        .arg(format!("-Zcodegen-backend={}", backend.display()))
        .arg("-o")
        .arg(output.0.join("fixture"))
        .env("FE2O3_VERBOSE", "1")
        .env("FE2O3_DUMP_LLVM", "1")
        .env("FE2O3_TARGET", target)
        .env("FE2O3_CODEGEN_PIPELINE", pipeline)
        .env("FE2O3_HSACO_DIR", output.0.join("artifacts"));
    command
        .output()
        .expect("compile scalar-control-flow fixture")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_rejected_without_fallback(output: &Output, expected: &str) {
    let stderr = stderr(output);
    assert!(!output.status.success(), "unexpected success\n{stderr}");
    assert!(
        stderr.contains(expected),
        "missing `{expected}` diagnostic\n{stderr}"
    );
    assert!(
        !stderr.contains("unsupported kernel shape for AMDGPU LLVM IR MVP")
            && !stderr.contains("selected legacy-v1")
            && !stderr.contains("emitted scalar_control_flow_v1"),
        "rejection entered a legacy/artifact fallback\n{stderr}"
    );
}

#[test]
fn authenticated_fixture_builds_role_preserving_contract_then_stops_at_mir_capture() {
    let workspace = workspace();
    let backend = build_backend(&workspace);
    let output = TestOutputDir::new(&workspace);
    let fixture =
        Path::new("crates/rustc-codegen-fe2o3/tests/fixtures/executable-scalar-control-flow-v1.rs");
    let compiled = compile_path(
        &workspace,
        &backend,
        &output,
        fixture,
        "gfx942:xnack-",
        PIPELINE,
        &[],
    );
    let stderr = stderr(&compiled);
    assert!(!compiled.status.success(), "unexpected success\n{stderr}");
    assert!(stderr.contains("[kernel] scalar_control_flow_v1"));
    assert!(stderr.contains("[internal-helper]"));
    assert!(
        stderr.contains(&format!(
            "{PIPELINE} authenticated collected KernelEntry export `scalar_control_flow_v1`"
        )),
        "missing authenticated export diagnostic\n{stderr}"
    );
    assert!(stderr.contains("exact reachable InternalHelper MIR"));
    assert!(stderr.contains("sealed as internal symbol"));
    assert!(stderr.contains("role-preserving composition contract"));
    assert!(stderr.contains("executable-MIR capture/import"));
    assert!(stderr.contains("no Kernel IR, LLVM, LLD, HSACO, or legacy fallback was entered"));
    assert!(!stderr.contains("define amdgpu_kernel"));
    assert_eq!(
        std::fs::read_dir(output.0.join("artifacts"))
            .expect("read empty artifact directory")
            .count(),
        0,
        "admission-only slice must not claim an artifact"
    );
}

#[test]
fn target_pipeline_identity_abi_and_collection_substitutions_reject_without_fallback() {
    let workspace = workspace();
    let backend = build_backend(&workspace);

    let wrong_target = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            &backend,
            &wrong_target,
            FIXTURE,
            "gfx942:xnack+",
            PIPELINE,
        ),
        "requires exact target `gfx942:xnack-`, found `gfx942:xnack+`",
    );

    let custom_pipeline = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            &backend,
            &custom_pipeline,
            FIXTURE,
            "gfx942:xnack-",
            "collected-executable-scalar-control-flow-v2-custom",
        ),
        "FE2O3_CODEGEN_PIPELINE must be unset or exactly",
    );

    let custom_llvm = TestOutputDir::new(&workspace);
    let fixture =
        Path::new("crates/rustc-codegen-fe2o3/tests/fixtures/executable-scalar-control-flow-v1.rs");
    assert_rejected_without_fallback(
        &compile_path(
            &workspace,
            &backend,
            &custom_llvm,
            fixture,
            "gfx942:xnack-",
            PIPELINE,
            &["-Cpasses=default<O1>"],
        ),
        "rejects custom LLVM pipeline selection",
    );

    let wrong_abi_source = FIXTURE
        .replace(
            "pub fn fe2o3_kernel_scalar_control_flow_v1(limit: u32)",
            "pub fn fe2o3_kernel_scalar_control_flow_v1(limit: u64)",
        )
        .replace(
            "nested_match_helper(limit);",
            "nested_match_helper(limit as u32);",
        )
        .replace("    fn(u32),", "    fn(u64),");
    let wrong_abi = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            &backend,
            &wrong_abi,
            &wrong_abi_source,
            "gfx942:xnack-",
            PIPELINE,
        ),
        "root ABI mismatch",
    );

    let wrong_helper_source = FIXTURE.replace("_ => sum += inner,", "_ => sum += inner + 1,");
    let wrong_helper = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            &backend,
            &wrong_helper,
            &wrong_helper_source,
            "gfx942:xnack-",
            PIPELINE,
        ),
        "portable closure MIR identity mismatch",
    );

    let wrong_helper_type_source = FIXTURE
        .replace(
            "fn nested_match_helper(limit: u32) -> u32",
            "fn nested_match_helper(limit: u64) -> u64",
        )
        .replace("0_u32", "0_u64")
        .replace(
            "nested_match_helper(limit);",
            "nested_match_helper(limit as u64);",
        );
    let wrong_helper_type = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            &backend,
            &wrong_helper_type,
            &wrong_helper_type_source,
            "gfx942:xnack-",
            PIPELINE,
        ),
        "helper ABI mismatch",
    );

    for changed_helper in [
        FIXTURE.replace("_ => sum += inner,", "_ => sum *= inner,"),
        FIXTURE.replace(
            "_ => sum += inner,",
            "_ => { if inner == 7 { sum += 1; } sum += inner },",
        ),
    ] {
        let output = TestOutputDir::new(&workspace);
        assert_rejected_without_fallback(
            &compile(
                &workspace,
                &backend,
                &output,
                &changed_helper,
                "gfx942:xnack-",
                PIPELINE,
            ),
            "portable closure MIR identity mismatch",
        );
    }

    let additional_root_source = FIXTURE.replace(
        "fn main() {}",
        r#"
#[unsafe(no_mangle)]
pub fn fe2o3_kernel_scalar_control_flow_extra(_: u32) {}

#[used]
#[allow(non_upper_case_globals)]
static __fe2o3_kernel_registration_scalar_control_flow_extra: (
    u64, u16, u16, &'static str, &'static str, fn(u32),
) = (
    0x4e52_4b33_4f32_4546, 1, 1,
    "scalar_control_flow_extra", "scalar_control_flow_extra",
    fe2o3_kernel_scalar_control_flow_extra,
);

fn main() {}
"#,
    );
    let additional_root = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            &backend,
            &additional_root,
            &additional_root_source,
            "gfx942:xnack-",
            PIPELINE,
        ),
        "requires exactly two collected functions, found 3",
    );
}
