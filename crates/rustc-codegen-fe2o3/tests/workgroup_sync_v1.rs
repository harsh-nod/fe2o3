use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

const LDS_PIPELINE: &str = "collected-lds-reduction-v1";
const ATOMIC_PIPELINE: &str = "collected-scoped-atomic-v1";
const LDS_CRATE_NAME: &str = "fe2o3_collected_lds_reduction_v1_fixture";
const ATOMIC_CRATE_NAME: &str = "fe2o3_collected_scoped_atomic_v1_fixture";
const LDS_METADATA: &str = "fe2o3-lds-reduction-v1-reviewed";
const ATOMIC_METADATA: &str = "fe2o3-scoped-atomic-v1-reviewed";
const LDS_CRATE_BINDING: &str = "fd63fb50f774e07f310d4b967e6fefbccf4a33d7abcf7096924037702cd8d0da";
const ATOMIC_CRATE_BINDING: &str =
    "dede4079399a3df33da7bcc9fc46bc84c3ab329642fa27241feaf10aff06388e";
const CARGO_METADATA_OBSERVATION: &str =
    "c1ab2dc02fa023687ac7394e15746c39668b5d46ad47c40eae012bc3f42d05c0";
const WORKSPACE_REMAP: &str = "/fe2o3-reviewed-workspace";
const LDS_SOURCE: &str = include_str!("../../../examples/workgroup_sync_v1/src/kernel.rs");
const ATOMIC_SOURCE: &str =
    include_str!("../../../examples/workgroup_sync_v1/src/scoped_atomic.rs");

static NEXT_OUTPUT: AtomicU64 = AtomicU64::new(0);
static FRONTEND_DEPENDENCIES: OnceLock<Result<(), String>> = OnceLock::new();

#[derive(Clone, Copy)]
enum ProfileKind {
    Lds,
    Atomic,
}

impl ProfileKind {
    const fn pipeline(self) -> &'static str {
        match self {
            Self::Lds => LDS_PIPELINE,
            Self::Atomic => ATOMIC_PIPELINE,
        }
    }

    const fn crate_name(self) -> &'static str {
        match self {
            Self::Lds => LDS_CRATE_NAME,
            Self::Atomic => ATOMIC_CRATE_NAME,
        }
    }

    const fn metadata(self) -> &'static str {
        match self {
            Self::Lds => LDS_METADATA,
            Self::Atomic => ATOMIC_METADATA,
        }
    }

    const fn crate_binding(self) -> &'static str {
        match self {
            Self::Lds => LDS_CRATE_BINDING,
            Self::Atomic => ATOMIC_CRATE_BINDING,
        }
    }

    const fn source_remap(self) -> &'static str {
        match self {
            Self::Lds => "/fe2o3-reviewed-workspace/lds-reduction-v1.rs",
            Self::Atomic => "/fe2o3-reviewed-workspace/scoped-atomic-v1.rs",
        }
    }

    const fn fixture(self) -> &'static str {
        match self {
            Self::Lds => "collected-lds-reduction-v1",
            Self::Atomic => "collected-scoped-atomic-v1",
        }
    }
}

struct TestOutput {
    path: PathBuf,
}

impl TestOutput {
    fn new(workspace: &Path) -> Self {
        let path = cargo_target(workspace).join(format!(
            "workgroup-sync-v1-{}-{}",
            std::process::id(),
            NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale workgroup-sync test output");
        }
        std::fs::create_dir_all(&path).expect("create workgroup-sync test output");
        Self { path }
    }
}

impl Drop for TestOutput {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[derive(Clone, Copy)]
struct CompilerProfile<'a> {
    target: &'a str,
    crate_name: &'a str,
    metadata: &'a str,
    crate_binding: &'a str,
    overflow_checks: bool,
}

impl<'a> CompilerProfile<'a> {
    const fn exact(kind: ProfileKind) -> CompilerProfile<'static> {
        CompilerProfile {
            target: "gfx942:xnack-",
            crate_name: kind.crate_name(),
            metadata: kind.metadata(),
            crate_binding: kind.crate_binding(),
            overflow_checks: false,
        }
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn cargo_target(workspace: &Path) -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(path) if Path::new(&path).is_absolute() => PathBuf::from(path),
        Some(path) => workspace.join(path),
        None => workspace.join("target"),
    }
}

fn profile_name() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

fn frontend_target(workspace: &Path) -> PathBuf {
    cargo_target(workspace).join("workgroup-sync-v1-frontend-target")
}

fn build_frontend_dependencies(workspace: &Path) -> Result<(), String> {
    FRONTEND_DEPENDENCIES
        .get_or_init(|| {
            let mut command = Command::new(env!("CARGO"));
            command.current_dir(workspace).args([
                "build",
                "--locked",
                "-p",
                "fe2o3-device",
                "-p",
                "fe2o3-host",
            ]);
            command
                .arg("--target-dir")
                .arg(frontend_target(workspace))
                .env("CARGO_INCREMENTAL", "0")
                .env("RUSTFLAGS", "-Zalways-encode-mir");
            let output = command.output().map_err(|error| error.to_string())?;
            if output.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "frontend dependency build failed:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                ))
            }
        })
        .clone()
}

fn compile(
    workspace: &Path,
    output: &TestOutput,
    label: &str,
    source: &str,
    kind: ProfileKind,
    profile: CompilerProfile<'_>,
) -> Output {
    build_frontend_dependencies(workspace).expect("build workgroup-sync frontend dependencies");
    let backend_target = cargo_target(workspace).join(profile_name());
    let frontend_target = frontend_target(workspace).join("debug");
    let backend = backend_target.join("librustc_codegen_fe2o3.so");
    let device = frontend_target.join("libfe2o3_device.rlib");
    let host = frontend_target.join("libfe2o3_host.rlib");
    for required in [&backend, &device, &host] {
        assert!(required.is_file(), "missing {}", required.display());
    }

    let source_path = output.path.join(format!("{label}.rs"));
    std::fs::write(&source_path, source).expect("write exact or hostile fixture");
    let artifact_dir = output.path.join(format!("{label}-artifacts"));
    std::fs::create_dir_all(&artifact_dir).expect("create empty artifact directory");
    let fixture_manifest = workspace
        .join("crates/rustc-codegen-fe2o3/tests/fixtures")
        .join(kind.fixture());

    Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .current_dir(workspace)
        .arg(&source_path)
        .arg(format!(
            "--remap-path-prefix={}={}",
            source_path.display(),
            kind.source_remap()
        ))
        .arg(format!(
            "--remap-path-prefix={}={WORKSPACE_REMAP}",
            workspace.display()
        ))
        .args(["--edition=2024", "--crate-type=lib", "--crate-name"])
        .arg(profile.crate_name)
        .arg("--extern")
        .arg(format!("fe2o3_device={}", device.display()))
        .arg("--extern")
        .arg(format!("fe2o3_host={}", host.display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            frontend_target.join("deps").display()
        ))
        .arg(format!("-Coverflow-checks={}", profile.overflow_checks))
        .arg(format!("-Cmetadata={}", profile.metadata))
        .arg("-Zmir-enable-passes=-JumpThreading")
        .arg(format!("-Zcodegen-backend={}", backend.display()))
        .arg("-o")
        .arg(output.path.join(format!("lib{label}.rlib")))
        .env("CARGO_MANIFEST_DIR", fixture_manifest)
        .env("FE2O3_CRATE_BINDING_ID_V1", profile.crate_binding)
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            CARGO_METADATA_OBSERVATION,
        )
        .env("FE2O3_HSACO_DIR", &artifact_dir)
        .env("FE2O3_TARGET", profile.target)
        .env("FE2O3_CODEGEN_PIPELINE", kind.pipeline())
        .output()
        .expect("run workgroup-sync compiler fixture")
}

fn mutation(source: &str, old: &str, new: &str) -> String {
    assert_eq!(source.matches(old).count(), 1, "non-unique mutation anchor");
    source.replacen(old, new, 1)
}

fn assert_rejected(result: &Output, label: &str) {
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "hostile case `{label}` compiled");
    assert!(
        !stderr.contains("authenticated exact source bytes"),
        "hostile case `{label}` reached authenticated receipt:\n{stderr}"
    );
}

#[test]
fn exact_sources_authenticate_complete_profiles() {
    let workspace = workspace();
    let output = TestOutput::new(&workspace);
    for (kind, source, label) in [
        (ProfileKind::Lds, LDS_SOURCE, "exact-lds"),
        (ProfileKind::Atomic, ATOMIC_SOURCE, "exact-atomic"),
    ] {
        let result = compile(
            &workspace,
            &output,
            label,
            source,
            kind,
            CompilerProfile::exact(kind),
        );
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            !result.status.success(),
            "admission-only profile emitted code"
        );
        for marker in [
            "authenticated exact source bytes",
            "wrapper/session-derived ordinary #[kernel(typed)] root",
            "exact rustc FnAbi, frozen trusted definitions and reviewed semantic-terminal manifest",
            "complete reachable portable-MIR closure modulo those identity-bound terminals",
            "reviewed source-to-profile and source-to-terminal correspondence only",
            "no generic lowering, terminal-body refinement, compiler-refinement proof, LLVM lowering, Worker V2",
        ] {
            assert!(stderr.contains(marker), "missing `{marker}`:\n{stderr}");
        }
    }
}

#[test]
fn hostile_lds_source_and_compiler_mutations_fail_closed() {
    let workspace = workspace();
    let output = TestOutput::new(&workspace);
    let sources = [
        ("lds-source-byte", format!("{LDS_SOURCE}\n// hostile\n")),
        (
            "lds-namespace",
            mutation(
                LDS_SOURCE,
                "6bc8f449f458cf8f31b4625b38b7204dd34f20beeabb80b55454a5666be749b5",
                "7bc8f449f458cf8f31b4625b38b7204dd34f20beeabb80b55454a5666be749b5",
            ),
        ),
        (
            "lds-element",
            mutation(LDS_SOURCE, "DynamicLds::<i32>", "DynamicLds::<u32>"),
        ),
        (
            "lds-extent",
            mutation(
                LDS_SOURCE,
                "DynamicLds::<i32>::exact_from_compiler::<64>",
                "DynamicLds::<i32>::exact_from_compiler::<32>",
            ),
        ),
        (
            "lds-launch-geometry",
            mutation(
                LDS_SOURCE,
                "launch(required = [64, 1, 1], max = [64, 1, 1])",
                "launch(required = [32, 1, 1], max = [32, 1, 1])",
            ),
        ),
        (
            "lds-epoch",
            mutation(
                LDS_SOURCE,
                "&mut lds_scope, epoch)",
                "&mut lds_scope, epoch.wrapping_add(1))",
            ),
        ),
        (
            "lds-collective",
            mutation(
                LDS_SOURCE,
                "group.reduce_sum(&context, &mut scratch, value)",
                "group.inclusive_scan_sum(&context, &mut scratch, value)",
            ),
        ),
        (
            "lds-output-owner",
            mutation(LDS_SOURCE, "if lane == 0 {", "if lane == 1 {"),
        ),
        (
            "lds-output-index",
            mutation(LDS_SOURCE, "output.get_mut_at(0)", "output.get_mut_at(1)"),
        ),
        (
            "lds-terminal-substitution",
            mutation(
                LDS_SOURCE,
                "let lane = thread::thread_idx_x();",
                "let lane = thread::block_dim_x();",
            ),
        ),
    ];
    for (label, source) in sources {
        let result = compile(
            &workspace,
            &output,
            label,
            &source,
            ProfileKind::Lds,
            CompilerProfile::exact(ProfileKind::Lds),
        );
        assert_rejected(&result, label);
    }
    for (label, profile) in hostile_compiler_profiles(ProfileKind::Lds) {
        let result = compile(
            &workspace,
            &output,
            label,
            LDS_SOURCE,
            ProfileKind::Lds,
            profile,
        );
        assert_rejected(&result, label);
    }
}

#[test]
fn hostile_atomic_source_and_compiler_mutations_fail_closed() {
    let workspace = workspace();
    let output = TestOutput::new(&workspace);
    let sources = [
        (
            "atomic-source-byte",
            format!("{ATOMIC_SOURCE}\n// hostile\n"),
        ),
        (
            "atomic-namespace",
            mutation(
                ATOMIC_SOURCE,
                "409357ef99d9ec78c960cca0e21a4e153c60af522c1c4d726a9f23b5c7271b91",
                "509357ef99d9ec78c960cca0e21a4e153c60af522c1c4d726a9f23b5c7271b91",
            ),
        ),
        (
            "atomic-ordering",
            mutation(
                ATOMIC_SOURCE,
                "target.fetch_add(values[lane], Ordering::Relaxed)",
                "target.fetch_add(values[lane], Ordering::SeqCst)",
            ),
        ),
        (
            "atomic-eligibility",
            mutation(ATOMIC_SOURCE, "eligible[lane] != 0", "eligible[lane] == 0"),
        ),
        (
            "atomic-operation",
            mutation(ATOMIC_SOURCE, "fetch_add", "fetch_max"),
        ),
        (
            "atomic-address-space",
            mutation(
                ATOMIC_SOURCE,
                "target: DeviceGlobalMutPtr<u32>",
                "target: fe2o3_device::DeviceWorkgroupMutPtr<u32>",
            ),
        ),
        (
            "atomic-scalar-type",
            mutation(
                ATOMIC_SOURCE,
                "target: DeviceGlobalMutPtr<u32>",
                "target: DeviceGlobalMutPtr<i32>",
            ),
        ),
        (
            "atomic-argument-roles",
            mutation(
                ATOMIC_SOURCE,
                "values: &[u32], eligible: &[u32]",
                "eligible: &[u32], values: &[u32]",
            ),
        ),
        (
            "atomic-terminal-substitution",
            mutation(
                ATOMIC_SOURCE,
                "let lane = thread::thread_idx_x() as usize;",
                "let lane = thread::block_dim_x() as usize;",
            ),
        ),
    ];
    for (label, source) in sources {
        let result = compile(
            &workspace,
            &output,
            label,
            &source,
            ProfileKind::Atomic,
            CompilerProfile::exact(ProfileKind::Atomic),
        );
        assert_rejected(&result, label);
    }
    for (label, profile) in hostile_compiler_profiles(ProfileKind::Atomic) {
        let result = compile(
            &workspace,
            &output,
            label,
            ATOMIC_SOURCE,
            ProfileKind::Atomic,
            profile,
        );
        assert_rejected(&result, label);
    }
}

fn hostile_compiler_profiles(kind: ProfileKind) -> Vec<(&'static str, CompilerProfile<'static>)> {
    vec![
        (
            "wrong-target",
            CompilerProfile {
                target: "gfx942",
                ..CompilerProfile::exact(kind)
            },
        ),
        (
            "wrong-crate",
            CompilerProfile {
                crate_name: "fe2o3_workgroup_sync_impostor",
                ..CompilerProfile::exact(kind)
            },
        ),
        (
            "wrong-metadata",
            CompilerProfile {
                metadata: "fe2o3-workgroup-sync-unreviewed",
                ..CompilerProfile::exact(kind)
            },
        ),
        (
            "wrong-binding",
            CompilerProfile {
                crate_binding: "aa63fb50f774e07f310d4b967e6fefbccf4a33d7abcf7096924037702cd8d0da",
                ..CompilerProfile::exact(kind)
            },
        ),
        (
            "overflow-policy",
            CompilerProfile {
                overflow_checks: true,
                ..CompilerProfile::exact(kind)
            },
        ),
    ]
}
