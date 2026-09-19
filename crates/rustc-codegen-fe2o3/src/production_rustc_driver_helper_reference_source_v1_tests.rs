//! Genuine ordinary Rust reaching request preparation, then the unchanged
//! missing-runtime boundary. No signed refinement or post-proof stage is made.
use super::*;
use crate::production_reference_effect_join_v2::prepared_observation_v1::{self as observed, Case};

const CASE: &str = "FE2O3_TEST_HELPER_REFERENCE_CASE_V1";
const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::helper_reference_source::helper_reference_source_child";
const RUNTIME: &str = "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";

#[derive(Debug, Serialize, Deserialize)]
struct Report {
    case: Case,
    prepared: observed::Observation,
    runtime_root: String,
    runtime_detail: String,
}
impl Report {
    fn validate(&self, case: Case) -> Result<(), String> {
        if self.case != case || self.runtime_root != RUNTIME || self.runtime_detail.is_empty() {
            return Err("exact missing approved-runtime boundary changed".into());
        }
        self.prepared.validate(case)
    }
}

struct ReferenceCallbacks {
    case: Case,
    result: Option<Result<Report, String>>,
}
impl Callbacks for ReferenceCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use crate::production_pipeline::ProductionPipelineError as Pipeline;
        use crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1 as Projection;
        use crate::production_reference_effect_join_v2::ProductionReferenceEffectJoinErrorV2 as Join;
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let (result, prepared) =
                observed::observe(self.case, || transaction.verify_general_kernel_checks());
            let (root, detail) =
                match result {
                    Err(Pipeline::RankedProjection(Projection::ReferenceEffectJoin(
                        Join::ProofRuntimeUnavailable { root, detail },
                    ))) => (root, detail),
                    Err(error) => return Err(format!("unexpected pre-proof refusal: {error:?}")),
                    Ok(_) => return Err(
                        "test requires the absent-runtime boundary; no signed success is inferred"
                            .into(),
                    ),
                };
            let report = Report {
                case: self.case,
                prepared,
                runtime_root: root.into(),
                runtime_detail: detail,
            };
            report.validate(self.case)?;
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess helper; its parent supplies exact Cargo invocation and diagnostic expectation"]
fn helper_reference_source_child() {
    let Some(path) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let case: Case = serde_json::from_str(&env::var(CASE).unwrap()).unwrap();
    let mut callbacks = ReferenceCallbacks { case, result: None };
    let completed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustc_driver::run_compiler(&args, &mut callbacks);
    }));
    let result = if completed.is_err() {
        Err("rustc or observational callback panicked".to_owned())
    } else {
        callbacks
            .result
            .unwrap_or_else(|| Err("ordinary source callback did not execute".into()))
    };
    std::fs::write(
        env::var_os(CHILD_RESULT).expect("child result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "retained Defined helper request: {result:?}"
    );
}

fn fixture(workspace: &Path, case: Case, target: &str) -> corpus::Fixture {
    let base = "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device";
    let manifest = format!("{base}/Cargo.toml");
    let hash = |path: &str| {
        Sha256::digest(std::fs::read(workspace.join(path)).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    corpus::Fixture {
        fixture_id: format!("{target}-{}", case.feature()),
        target: target.into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hash(&manifest),
            package_manifest: manifest,
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hash("Cargo.lock"),
            source_paths: vec![
                format!("{base}/src/lib.rs"),
                format!("{base}/src/defined_helper_reference.rs"),
            ],
            // The capture helper does not interpret this corpus-only field. This
            // is not a tutorial-manifest or source-closure admission claim.
            source_closure_sha256: String::new(),
            cargo_target: corpus::CargoTarget {
                kind: "lib".into(),
                name: "fe2o3_production_extraction_fixture".into(),
                source_path: "src/lib.rs".into(),
            },
            default_features: false,
            features: vec![case.feature().into()],
            kernel_symbols: vec!["helper_reference".into()],
        },
    }
}

#[test]
#[ignore = "genuine Cargo/rustc AMD source capture; requires pinned toolchain and absent approved Verus runtime"]
fn retained_defined_helpers_prepare_exact_reference_requests_on_both_profiles() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-helper-reference-source");
    for target in ["gfx942", "gfx950"] {
        let target_dir = scratch.path().join(target);
        for case in [Case::Direct, Case::Nested, Case::Swapped, Case::Alternate] {
            let fixture = fixture(&workspace, case, target);
            let directory = scratch.path().join(&fixture.fixture_id);
            std::fs::create_dir(&directory).unwrap();
            let mut captured =
                corpus_cargo::capture(&workspace, &fixture, &directory, &target_dir).unwrap();
            // Retain real helper calls instead of qualifying rustc's inlined
            // body. This changes only the explicit test root invocation.
            captured.args.push("-Zinline-mir=no".into());
            let request = directory.join("args.json");
            let response = directory.join("result.json");
            std::fs::write(&request, serde_json::to_vec(&captured.args).unwrap()).unwrap();
            let mut command = Command::new(env::current_exe().unwrap());
            command
                .env_clear()
                .envs(captured.environment)
                .current_dir(captured.cwd)
                .env_remove("RUSTC_WRAPPER")
                .env_remove("RUSTC_WORKSPACE_WRAPPER")
                .env_remove(CHILD_PROOF_PROBE)
                .env(CHILD_ARGS, &request)
                .env(CHILD_RESULT, &response)
                .env(CASE, serde_json::to_string(&case).unwrap())
                .args(["--exact", CHILD, "--ignored", "--nocapture"]);
            progress::clear_inherited_jobserver(&mut command);
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}: {}",
                fixture.fixture_id,
                corpus_cargo::diagnostics(&output)
            );
            let result: Result<Report, String> =
                serde_json::from_slice(&std::fs::read(response).unwrap()).unwrap();
            let report = result.unwrap();
            report.validate(case).unwrap();
            eprintln!(
                "HELPER REFERENCE {} prepared exact request; approved-runtime unavailable; no proof or artifact authority",
                fixture.fixture_id
            );
        }
    }
}

#[test]
fn report_cannot_substitute_runtime_or_case_for_preparation_evidence() {
    let report = Report {
        case: Case::Direct,
        prepared: observed::Observation::default(),
        runtime_root: RUNTIME.into(),
        runtime_detail: "test-only unavailable".into(),
    };
    assert!(report.validate(Case::Direct).is_err());
    assert!(report.validate(Case::Nested).is_err());
}
