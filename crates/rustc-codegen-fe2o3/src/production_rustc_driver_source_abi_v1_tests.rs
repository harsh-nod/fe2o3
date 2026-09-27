//! Exact tutorial-driver aggregate inputs, stopped before descriptor encoding.
use super::*;
use crate::compiler_descriptor::source_abi_v1::tests::Qualification;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

const CASE: &str = "FE2O3_TEST_SOURCE_ABI_CASE_V1";
const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::source_abi::source_abi_capture_child";
const CASES: [(&str, usize); 5] = [
    ("aggregate_pair_struct", 4),
    ("aggregate_pair_tuple", 4),
    ("aggregate_pair_array", 4),
    ("aggregate_zst", 2),
    ("aggregate_nested", 6),
];

#[derive(Debug, Serialize, Deserialize)]
struct Report {
    case: String,
    profile: String,
    qualification: Qualification,
}
impl Report {
    fn check(&self, case: &str, profile: &str) {
        let expected = CASES.iter().find(|(name, _)| *name == case).unwrap().1;
        assert_eq!(self.case, case);
        assert_eq!(self.profile, profile);
        let q = &self.qualification;
        assert_eq!((q.roots, q.logical, q.physical), (1, 3, expected));
        assert_eq!(
            (q.capture_mutants, q.endpoint_mutants, q.resource_cuts),
            (12, 12, 3)
        );
        assert_eq!(q.bridge_refusals, 2);
        assert_eq!(
            q.same_typed_endpoint_mutants,
            if case == "aggregate_zst" { 0 } else { 3 }
        );
    }
}
struct Capture {
    case: String,
    callbacks: usize,
    result: Option<Result<Report, String>>,
}
impl Callbacks for Capture {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.callbacks += 1;
        self.result = Some((|| {
            assert_eq!(self.callbacks, 1);
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let ranked = transaction
                .verify_general_kernel_checks()
                .map_err(|error| format!("{error:?}"))?;
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(29).unwrap();
            let qualification = ranked
                .source_abi_qualification_v1(&self.case, &mut budget)
                .map_err(|error| format!("{error:?}"))?;
            assert_eq!(budget.storage(), 29);
            Ok(Report {
                case: self.case.clone(),
                qualification,
                profile: tcx
                    .sess
                    .opts
                    .cg
                    .target_cpu
                    .as_deref()
                    .unwrap_or(tcx.sess.target.cpu.as_ref())
                    .to_owned(),
            })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess helper; parent supplies exact Cargo source and target invocation"]
fn source_abi_capture_child() {
    let Some(path) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let case = env::var(CASE).unwrap();
    assert!(CASES.iter().any(|(name, _)| *name == case));
    let mut callbacks = Capture {
        case,
        callbacks: 0,
        result: None,
    };
    let completed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustc_driver::run_compiler(&args, &mut callbacks);
    }));
    let result = if completed.is_err() {
        Err("rustc/source ABI callback panicked".to_owned())
    } else {
        callbacks
            .result
            .unwrap_or_else(|| Err("source ABI callback did not execute".into()))
    };
    std::fs::write(
        env::var_os(CHILD_RESULT).unwrap(),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "genuine source ABI prerequisite: {result:?}"
    );
}

fn fixture(workspace: &Path, case: &str, target: &str) -> corpus::Fixture {
    let base = "crates/rustc-codegen-fe2o3/tests/fixtures/production-ranked-bounds-device";
    let manifest = format!("{base}/Cargo.toml");
    let hash = |path: &str| {
        Sha256::digest(std::fs::read(workspace.join(path)).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    corpus::Fixture {
        fixture_id: format!("{target}-{case}"),
        target: target.into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hash(&manifest),
            package_manifest: manifest,
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hash("Cargo.lock"),
            source_paths: vec![format!("{base}/src/lib.rs")],
            // Existing Cargo invocation capture is reused. Tutorial manifest
            // authentication stays in its independent unchanged corpus parent.
            source_closure_sha256: String::new(),
            cargo_target: corpus::CargoTarget {
                kind: "lib".into(),
                name: "fe2o3_production_ranked_bounds_fixture".into(),
                source_path: "src/lib.rs".into(),
            },
            default_features: false,
            features: vec![case.into()],
            kernel_symbols: vec![case.into()],
        },
    }
}

#[test]
#[ignore = "genuine Cargo/rustc AMD source ABI and checked P4; requires pinned production toolchain"]
fn source_abi_all_five_aggregate_inputs_preserve_checked_entries_on_both_profiles() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-source-abi-capture");
    let mut completed = 0;
    for target in ["gfx942", "gfx950"] {
        let target_dir = scratch.path().join(target);
        for (case, _) in CASES {
            let selected = fixture(&workspace, case, target);
            let directory = scratch.path().join(&selected.fixture_id);
            std::fs::create_dir(&directory).unwrap();
            let captured =
                corpus_cargo::capture(&workspace, &selected, &directory, &target_dir).unwrap();
            for (optimization, mir) in [(0, 0), (3, 2)] {
                let mut args = captured.args.clone();
                args.extend([
                    format!("-Copt-level={optimization}"),
                    format!("-Zmir-opt-level={mir}"),
                ]);
                let request = directory.join(format!("args-{optimization}.json"));
                let response = directory.join(format!("result-{optimization}.json"));
                std::fs::write(&request, serde_json::to_vec(&args).unwrap()).unwrap();
                let mut command = Command::new(env::current_exe().unwrap());
                command
                    .env_clear()
                    .envs(captured.environment.clone())
                    .current_dir(&captured.cwd)
                    .env_remove("RUSTC_WRAPPER")
                    .env_remove("RUSTC_WORKSPACE_WRAPPER")
                    .env_remove(CHILD_PROOF_PROBE)
                    .env(CHILD_ARGS, &request)
                    .env(CHILD_RESULT, &response)
                    .env(CASE, case)
                    .args(["--exact", CHILD, "--ignored", "--nocapture"]);
                progress::clear_inherited_jobserver(&mut command);
                let output = command.output().unwrap();
                assert!(
                    output.status.success(),
                    "{} opt{optimization}: {}",
                    selected.fixture_id,
                    corpus_cargo::diagnostics(&output)
                );
                let report: Result<Report, String> =
                    serde_json::from_slice(&std::fs::read(response).unwrap()).unwrap();
                report.unwrap().check(case, target);
                completed += 1;
                eprintln!(
                    "SOURCE ABI {target} {case} opt{optimization}: exact capture and physical entry; descriptor/launch pending"
                );
            }
        }
    }
    assert_eq!(completed, 20);
}
