//! Ordinary Rust through the checked-output stage, without a shipping selector.
use super::*;
use fe2o3_kernel_ir::OperationKind;
use fe2o3_rustc_invocation::{
    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, PortablePackageIdentityV1, RustcInvocationV2,
    classify_rustc_invocation_v2, derive_cargo_metadata_build_observation_v2,
    ordered_rustc_codegen_metadata_v1, portable_rustc_metadata_v1,
};
use reserved_fe2o3_symbols::{CRATE_BINDING_ID_ENV_V1, derive_crate_binding_id_v1};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::ffi::OsString;
use std::process::Command;

#[path = "production_context_source_v29_tests.rs"]
mod context_source_v29_tests;

const CHILD_ARGS: &str = "FE2O3_TEST_CHECKED_OUTPUT_ARGS_V1";
const CHILD_RESULT: &str = "FE2O3_TEST_CHECKED_OUTPUT_RESULT_V1";
const CHILD_PROOF_PROBE: &str = "FE2O3_TEST_CHECKED_OUTPUT_PROOF_PROBE_V1";
const CHILD_TEST: &str =
    "production_rustc_driver_v1::checked_output_source_v1_tests::checked_output_source_child";

#[derive(Debug, Serialize, Deserialize)]
struct Observation {
    roots: Vec<String>,
    reads: usize,
    writes: usize,
    formal_accesses: usize,
    policy: u16,
    output_digest: [u8; 32],
    llvm_bytes: usize,
    descriptor_roots: usize,
    missing_proof_refused: bool,
}

#[derive(Default)]
struct CheckedOutputCallbacks {
    result: Option<Result<Observation, String>>,
    probe_missing_proof: bool,
}

impl Callbacks for CheckedOutputCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let ranked = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?
            .verify_general_kernel_checks()
            .map_err(|e| e.to_string())?;
            assert!(ranked.all_kernel_checks_are_clean());
            assert!(!ranked.grants_artifact_or_launch_authority());
            let stage = ranked
                .lower_checked_output_policy4_v1()
                .map_err(|e| e.to_string())?;
            let admitted = stage.output();
            assert!(!admitted.grants_artifact_or_launch_authority());
            assert!(std::ptr::eq(
                admitted.output(),
                admitted.checked_output().owner()
            ));
            let module = admitted.output().module();
            let mut observation = Observation {
                roots: module
                    .kernels
                    .iter()
                    .map(|k| k.id.as_str().to_owned())
                    .collect(),
                reads: 0,
                writes: 0,
                formal_accesses: admitted.kernels().iter().map(|k| k.accesses().len()).sum(),
                policy: admitted.checked_output().execution().policy_version(),
                output_digest: *admitted.output().canonical().identity().digest(),
                llvm_bytes: 0,
                descriptor_roots: 0,
                missing_proof_refused: false,
            };
            for operation in module
                .functions
                .iter()
                .filter_map(|f| f.body.as_ref())
                .flat_map(|body| &body.blocks)
                .flat_map(|block| &block.operations)
            {
                match &operation.kind {
                    OperationKind::Load { .. } | OperationKind::GuardedLoad { .. } => {
                        observation.reads += 1
                    }
                    OperationKind::Store { .. } | OperationKind::GuardedStore { .. } => {
                        observation.writes += 1
                    }
                    _ => {}
                }
            }
            if self.probe_missing_proof {
                use crate::production_native_source_lineage_v1::NativeSourceLineageErrorV1;
                use crate::production_pipeline::{
                    ProductionPipelineError, checked_output_policy4_v1::CheckedOutputStageErrorV1,
                };
                let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
                    usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                        .map_err(|e| e.to_string())?,
                );
                let mut budget =
                    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
                        &mut work,
                        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
                    );
                let floor = stage.retained_storage_floor_v1();
                budget.reserve_storage(floor).map_err(|e| e.to_string())?;
                let refused = stage.prepare_native_source_lineage_v1(&mut budget);
                assert_eq!(budget.storage(), floor);
                match refused {
                    Err(ProductionPipelineError::CheckedOutputStage(
                        CheckedOutputStageErrorV1::NativeSource(error),
                    )) if matches!(
                        *error,
                        NativeSourceLineageErrorV1::MissingSignedRankedReceipt { .. }
                    ) =>
                    {
                        observation.missing_proof_refused = true
                    }
                    Err(error) => return Err(format!("unexpected native proof refusal: {error}")),
                    Ok(_) => return Err("unsigned source acquired native proof custody".into()),
                }
                return Ok(observation);
            }
            let (handoff, descriptor) = stage
                .into_worker_handoff_extraction_v1()
                .map_err(|e| e.to_string())?;
            assert!(!descriptor.grants_launch_authority());
            let llvm = std::str::from_utf8(handoff.module_bytes()).map_err(|e| e.to_string())?;
            assert!(llvm.contains("amdgpu_kernel"));
            observation.llvm_bytes = llvm.len();
            observation.descriptor_roots = descriptor.table().kernels().len();
            Ok(observation)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess helper; invoked with an exact source/dependency request by its parent"]
fn checked_output_source_child() {
    let Some(path) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = CheckedOutputCallbacks {
        probe_missing_proof: env::var_os(CHILD_PROOF_PROBE).is_some(),
        ..CheckedOutputCallbacks::default()
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual-source callback did not run");
    std::fs::write(
        env::var_os(CHILD_RESULT).expect("child result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "checked native source route: {result:?}");
}

fn clean_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    command.env_clear();
    for key in [
        "HOME",
        "PATH",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
        "LD_LIBRARY_PATH",
        "TMPDIR",
    ] {
        if let Some(value) = env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .env("CARGO_BUILD_JOBS", "1")
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_PROFILE_RELEASE_DEBUG", "0")
        .env("FE2O3_HIP_SYS_DISABLE", "1")
        .env("FE2O3_HSA_RUNTIME_DISABLE", "1");
    command
}

fn output(command: &mut Command) -> std::process::Output {
    let result = command
        .output()
        .expect("execute source qualification command");
    assert!(
        result.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

fn artifact(messages: &[serde_json::Value], name: &str) -> PathBuf {
    let artifacts: Vec<_> = messages
        .iter()
        .filter(|m| m["reason"] == "compiler-artifact" && m["target"]["name"] == name)
        .collect();
    assert_eq!(
        artifacts.len(),
        1,
        "one actual Cargo artifact for {name}: {artifacts:?}"
    );
    let filenames = artifacts[0]["filenames"]
        .as_array()
        .expect("Cargo artifact filenames");
    // build-std can emit both for one artifact. This callback needs metadata;
    // use the standalone metadata when present, otherwise its library container.
    for extension in ["rmeta", "rlib"] {
        let matches: Vec<_> = filenames
            .iter()
            .filter_map(|p| p.as_str())
            .map(PathBuf::from)
            .filter(|p| p.extension().is_some_and(|ext| ext == extension))
            .collect();
        assert!(
            matches.len() <= 1,
            "ambiguous {extension} for {name}: {matches:?}"
        );
        if let Some(path) = matches.into_iter().next() {
            return path;
        }
    }
    panic!("missing metadata for actual Cargo artifact {name}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD dependencies, and ordinary-source compilation"]
fn ordinary_rust_fill_and_vecadd_reach_checked_native_output() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-checked-output-source");
    let target = scratch.path().join("target");
    let cargo = env!("CARGO");
    let metadata: serde_json::Value = serde_json::from_slice(
        &output(clean_command(cargo).current_dir(&workspace).args([
            "metadata",
            "--offline",
            "--locked",
            "--no-deps",
            "--format-version",
            "1",
        ]))
        .stdout,
    )
    .unwrap();
    let built = output(clean_command(cargo).current_dir(&workspace)
        .args(["check", "--offline", "--locked", "--release", "-Zbuild-std=core",
            "-p", "fe2o3-device", "--target", "amdgcn-amd-amdhsa",
            "--message-format=json-render-diagnostics", "--target-dir"])
        .arg(&target)
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32"));
    let messages: Vec<serde_json::Value> = built
        .stdout
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    let device = artifact(&messages, "fe2o3_device");
    let core = artifact(&messages, "core");
    let builtins = artifact(&messages, "compiler_builtins");
    assert!(device.starts_with(&target));
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let sysroot = output(clean_command(&rustc).args(["--print", "sysroot"]));
    let sysroot = String::from_utf8(sysroot.stdout).unwrap();
    for (name, reads, writes) in [("fill", 0, 1), ("vecadd", 2, 1)] {
        let package_dir = workspace.join("examples").join(name);
        let manifest = package_dir.join("Cargo.toml");
        let package = metadata["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| Path::new(p["manifest_path"].as_str().unwrap()) == manifest)
            .unwrap();
        let package_name = package["name"].as_str().unwrap();
        let version = package["version"].as_str().unwrap();
        let crate_name = package_name.replace('-', "_");
        let identity = PortablePackageIdentityV1::new(
            package_name,
            version,
            Sha256::digest(std::fs::read(&manifest).unwrap()).into(),
        )
        .unwrap();
        let mut args = vec![
            rustc.to_str().unwrap().to_owned(),
            "--crate-name".into(),
            crate_name.clone(),
            "--crate-type=lib".into(),
            format!("--edition={}", package["edition"].as_str().unwrap()),
            "--target=amdgcn-amd-amdhsa".into(),
            "-Ctarget-cpu=gfx942".into(),
            "-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32".into(),
            "-Copt-level=3".into(),
            "-Cdebug-assertions=off".into(),
            "-Coverflow-checks=on".into(),
            "-Zalways-encode-mir".into(),
            "-Zunstable-options".into(),
            "--emit=metadata".into(),
            "--sysroot".into(),
            sysroot.trim().into(),
            format!("--extern=fe2o3_device={}", device.display()),
            format!("--extern=noprelude:core={}", core.display()),
            format!(
                "--extern=noprelude:compiler_builtins={}",
                builtins.display()
            ),
            format!("-Ldependency={}", device.parent().unwrap().display()),
            format!("-Ldependency={}", target.join("release/deps").display()),
            "--out-dir".into(),
            scratch.path().display().to_string(),
            package_dir.join("src/lib.rs").display().to_string(),
        ];
        let original: Vec<OsString> = args.iter().map(OsString::from).collect();
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&original).unwrap()
        else {
            panic!("expected source compile")
        };
        let portable = portable_rustc_metadata_v1(compile, &identity).unwrap();
        args.push(format!("-Cmetadata={portable}"));
        let actual: Vec<OsString> = args.iter().map(OsString::from).collect();
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&actual).unwrap()
        else {
            panic!("expected bound source compile")
        };
        let observation = derive_cargo_metadata_build_observation_v2(
            &ordered_rustc_codegen_metadata_v1(compile).unwrap(),
        );
        let binding = derive_crate_binding_id_v1(&crate_name, [portable.as_str()]);
        let request = scratch.path().join(format!("{name}-args.json"));
        let response = scratch.path().join(format!("{name}-result.json"));
        std::fs::write(&request, serde_json::to_vec(&args).unwrap()).unwrap();
        let mut command = clean_command(env::current_exe().unwrap());
        command
            .current_dir(&workspace)
            .args(["--exact", CHILD_TEST, "--ignored", "--nocapture"])
            .env(CHILD_ARGS, &request)
            .env(CHILD_RESULT, &response)
            .env("CARGO_MANIFEST_DIR", &package_dir)
            .env("CARGO_PKG_NAME", package_name)
            .env("CARGO_PKG_VERSION", version)
            .env("CARGO_PRIMARY_PACKAGE", "1")
            .env(CRATE_BINDING_ID_ENV_V1, binding.to_hex())
            .env(
                CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                observation.to_hex(),
            );
        let child = output(&mut command);
        let result: Result<Observation, String> =
            serde_json::from_slice(&std::fs::read(&response).unwrap()).unwrap();
        let result = result.unwrap();
        assert_eq!(result.roots, [name]);
        assert_eq!((result.reads, result.writes), (reads, writes));
        assert_eq!(result.formal_accesses, reads + writes);
        assert_eq!(result.policy, 4);
        assert_eq!(result.descriptor_roots, 1);
        assert_ne!(result.output_digest, [0; 32]);
        assert!(result.llvm_bytes > 0);
        assert!(!result.missing_proof_refused);
        eprintln!(
            "actual-source checked native output {name}: {result:?}\n{}",
            String::from_utf8_lossy(&child.stdout)
        );
        if name == "fill" {
            let expected_output = result.output_digest;
            let proof_response = scratch.path().join("fill-proof-refusal.json");
            let probe = output(
                command
                    .env(CHILD_PROOF_PROBE, "1")
                    .env(CHILD_RESULT, &proof_response),
            );
            let result: Result<Observation, String> =
                serde_json::from_slice(&std::fs::read(proof_response).unwrap()).unwrap();
            let result = result.unwrap();
            assert!(result.missing_proof_refused);
            assert_eq!(result.policy, 4);
            assert_eq!(result.output_digest, expected_output);
            assert_eq!((result.llvm_bytes, result.descriptor_roots), (0, 0));
            eprintln!(
                "actual-source native proof correctly refused missing signed receipt: {}",
                String::from_utf8_lossy(&probe.stdout)
            );
        }
    }
}
