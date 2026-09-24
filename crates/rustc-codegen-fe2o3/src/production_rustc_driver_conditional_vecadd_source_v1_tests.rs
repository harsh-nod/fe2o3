//! Actual manifest source plus an explicitly auxiliary annotated shared body.
//! These tests are unvalidated until run on the integrated conditional pipeline.
use super::*;
use crate::production_pipeline::ProductionPipelineError as Pipeline;
use crate::production_ranked_projection_v1::{
    ProductionRankedProjectionErrorV1 as Projection,
    ProductionRankedVerificationErrorV1 as Verification,
    conditional_output_observation_v1_tests as prepared,
    conditional_retention_observation_v1 as retention,
};
use crate::production_reference_effect_join_v2::{
    ProductionReferenceEffectJoinErrorV2 as Join, source_proof_freshness_v1 as proof,
};
use simulation::conditional_vecadd::{self as oracle, Mutation};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::conditional_bound_source::vecadd::conditional_vecadd_source_child";
const REQUEST: &str = "FE2O3_TEST_CONDITIONAL_VECADD_V1";
const BASE: &str = "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device";
const MANIFEST: &str = "config/tutorial-kernel-manifest-v1.json";
const LOAD_MISMATCH: &str =
    "safe reference load has no exact ranked GPU read with matching input, type, and index";
const GUARD_MISMATCH: &str =
    "GPU write has a logical path guard outside the exact memory-bounds selection";
const AMBIGUOUS_LOAD: &str = "safe reference load matches multiple ranked GPU reads";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Case {
    Manifest,
    Annotated,
    InputGuard,
    CpuRead,
    SourceArgument,
}

impl Case {
    fn feature(self) -> Option<&'static str> {
        match self {
            Self::Manifest => None,
            Self::Annotated => Some("conditional-vecadd"),
            Self::InputGuard => Some("conditional-vecadd-input-guard"),
            Self::CpuRead => Some("conditional-vecadd-cpu-read"),
            Self::SourceArgument => Some("conditional-vecadd-source-argument"),
        }
    }

    fn mutation(self) -> Mutation {
        match self {
            Self::Manifest | Self::Annotated => Mutation::None,
            Self::InputGuard => Mutation::InputGuard,
            Self::CpuRead => Mutation::CpuRead,
            Self::SourceArgument => Mutation::SourceArgument,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Stage {
    Oracle,
    Prepared,
    Bound,
    Consuming,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    case: Case,
    stage: Stage,
    source: Vec<(String, [u8; 32])>,
    args_sha256: [u8; 32],
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Report {
    case: Case,
    stage: Stage,
    detail: serde_json::Value,
    default_manifest_selection: bool,
    qualification_credit: bool,
    grants_artifact_or_launch_authority: bool,
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn hash(path: &Path) -> String {
    crate::encode_hex(&Sha256::digest(std::fs::read(path).unwrap()))
}

fn source_stamps(case: Case) -> Vec<(String, [u8; 32])> {
    let mut paths = vec![
        MANIFEST.to_owned(),
        "Cargo.lock".into(),
        "examples/vecadd/Cargo.toml".into(),
        "examples/vecadd/src/lib.rs".into(),
        "examples/vecadd/src/vecadd_body.rs".into(),
        format!("{BASE}/src/conditional_vecadd_reference.rs"),
    ];
    if case != Case::Manifest {
        paths.extend([
            format!("{BASE}/Cargo.toml"),
            format!("{BASE}/src/lib.rs"),
            format!("{BASE}/src/conditional_vecadd.rs"),
        ]);
    }
    paths
        .into_iter()
        .map(|path| {
            let digest = Sha256::digest(std::fs::read(workspace().join(&path)).unwrap()).into();
            (path, digest)
        })
        .collect()
}

fn fixture(case: Case, target: &str) -> corpus::Fixture {
    if case == Case::Manifest {
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(workspace().join(MANIFEST)).unwrap()).unwrap();
        let rows: Vec<_> = manifest["compilerFixtures"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["fixtureId"] == "gfx942-typed-vecadd-source")
            .collect();
        let [row] = rows.as_slice() else {
            panic!("one exact default Vecadd manifest row")
        };
        let fixture: corpus::Fixture = serde_json::from_value((*row).clone()).unwrap();
        assert_eq!(fixture.target, target, "do not relabel a manifest target");
        let input = &fixture.compiler_input;
        assert_eq!(input.package_manifest, "examples/vecadd/Cargo.toml");
        assert_eq!(
            input.package_manifest_sha256,
            hash(&workspace().join(&input.package_manifest))
        );
        assert_eq!(input.source_paths, ["examples/vecadd/src/lib.rs"]);
        assert_eq!(input.cargo_target.kind, "lib");
        assert_eq!(input.cargo_target.name, "fe2o3_vecadd");
        assert_eq!(input.cargo_target.source_path, "src/lib.rs");
        assert!(input.default_features && input.features.is_empty());
        assert_eq!(input.kernel_symbols, ["vecadd"]);
        assert_eq!(
            input.cargo_lock_sha256,
            hash(&workspace().join(&input.cargo_lock_path))
        );
        return fixture;
    }
    let manifest = format!("{BASE}/Cargo.toml");
    corpus::Fixture {
        fixture_id: format!("auxiliary-{target}-{}", case.feature().unwrap()),
        target: target.into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hash(&workspace().join(&manifest)),
            package_manifest: manifest,
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hash(&workspace().join("Cargo.lock")),
            source_paths: vec![
                format!("{BASE}/src/lib.rs"),
                format!("{BASE}/src/conditional_vecadd.rs"),
                format!("{BASE}/src/conditional_vecadd_reference.rs"),
                "examples/vecadd/src/vecadd_body.rs".into(),
            ],
            source_closure_sha256: String::new(),
            cargo_target: corpus::CargoTarget {
                kind: "lib".into(),
                name: "fe2o3_production_extraction_fixture".into(),
                source_path: "src/lib.rs".into(),
            },
            default_features: false,
            features: vec![case.feature().unwrap().into()],
            kernel_symbols: vec!["vecadd".into()],
        },
    }
}

struct VecaddCallbacks {
    case: Case,
    stage: Stage,
    calls: usize,
    result: Option<Result<Report, String>>,
}

fn fresh_proof(observation: &proof::Observation, expected: usize) {
    assert_eq!(observation.request_count, expected);
    assert_eq!(observation.normal_import_count, expected);
    assert_eq!(observation.binding.is_some(), expected == 1);
    assert_eq!(observation.original_receipt.is_some(), expected == 1);
    assert!(observation.stale_error.is_none());
    assert_eq!(observation.rejected_import_count, 0);
    assert_eq!(observation.original_reimport_count, 0);
    if let Some(receipt) = &observation.original_receipt {
        receipt.validate().unwrap();
    }
}

impl Callbacks for VecaddCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        let (result, retention) = retention::observe(|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let detail = match self.stage {
                Stage::Oracle => {
                    let result = transaction
                        .observe_pre_ranked_for_test_v1(|owner, _| {
                            assert!(!owner.grants_artifact_or_launch_authority());
                            oracle::observe(owner.executable().canonical(), self.case.mutation())
                                .map_err(|e| format!("{e:?}"))
                        })
                        .map_err(|e| e.to_string())??;
                    serde_json::json!({"pre_ranked_cpu_observation": result})
                }
                Stage::Prepared => {
                    let ((result, observation), proof) = proof::observe(None, || {
                        prepared::observe(|| transaction.verify_general_kernel_checks())
                    });
                    fresh_proof(&proof, 0);
                    if self.case == Case::Manifest {
                        assert!(
                            matches!(
                                result,
                                Err(Pipeline::RankedProjection(Projection::Incomplete(
                                    prepared::UNANNOTATED
                                )))
                            ),
                            "default manifest is unannotated: {:?}",
                            result.err()
                        );
                        assert!(observation.is_err());
                        serde_json::json!({"boundary": prepared::UNANNOTATED})
                    } else {
                        assert_eq!(self.case, Case::Annotated);
                        assert!(
                            matches!(
                                result,
                                Err(Pipeline::RankedProjection(Projection::Incomplete(
                                    prepared::STOP
                                )))
                            ),
                            "not the exact prepared boundary: {:?}",
                            result.err()
                        );
                        let observation = observation?;
                        assert_eq!(observation.kernel, "vecadd");
                        assert_ne!(observation.canonical_digest, [0; 32]);
                        assert_eq!(observation.source_argument, 2);
                        assert_eq!(observation.reference_argument, 2);
                        assert_eq!(observation.address_domain, "GlobalLaunch");
                        assert!(observation.work > 0);
                        serde_json::json!({"boundary": prepared::STOP, "prepared": observation})
                    }
                }
                Stage::Bound => {
                    assert_eq!(self.case, Case::Annotated);
                    let ((result, observation), proof) = proof::observe(None, || {
                        observed::observe(|| transaction.verify_general_kernel_checks())
                    });
                    assert!(
                        matches!(
                            result,
                            Err(Pipeline::RankedProjection(Projection::Incomplete(
                                observed::STOP
                            )))
                        ),
                        "not the exact post-bind boundary: {:?}",
                        result.err()
                    );
                    fresh_proof(&proof, 1);
                    let observation = observation?;
                    assert_eq!(observation.kernel, "vecadd");
                    assert_ne!(observation.canonical_digest, [0; 32]);
                    assert_eq!((observation.selected, observation.signed_receipts), (1, 1));
                    assert!(observation.memory_effects > 0 && observation.value_expressions > 0);
                    assert_eq!(observation.pending_checks, 9);
                    assert_eq!(observation.output.source_argument, 2);
                    assert_eq!(observation.output.raw_reference_argument, 3);
                    assert_eq!(observation.output.element_bytes, 4);
                    assert_eq!(observation.output.address_domain, "GlobalLaunch");
                    assert!(observation.work > 0);
                    serde_json::json!({"boundary": observed::STOP, "pending": observation})
                }
                Stage::Consuming => {
                    let (result, proof) =
                        proof::observe(None, || transaction.verify_general_kernel_checks());
                    match (self.case, result) {
                        (Case::Annotated, Ok(ranked)) => {
                            fresh_proof(&proof, 1);
                            assert!(!ranked.all_kernel_checks_are_clean());
                            assert!(!ranked.grants_artifact_or_launch_authority());
                            let [root] = ranked.ranked_roots() else {
                                return Err("one Vecadd root required".into());
                            };
                            let formula = root.conditional_formula_report_v1().ok_or(
                                "Vecadd did not consume an actual conditional formula execution",
                            )?;
                            let identities = [
                                formula.statement_identity(),
                                formula.generated_source_identity(),
                                formula.execution_identity(),
                                formula.receipt_identity(),
                            ];
                            for identity in &identities {
                                assert_ne!(identity.as_bytes(), &[0; 32]);
                            }
                            let report = serde_json::json!({
                                "statement": identities[0].as_bytes(), "generated_source": identities[1].as_bytes(),
                                "execution": identities[2].as_bytes(), "receipt": identities[3].as_bytes(),
                                "proof_retained_after_callback": false,
                            });
                            let Err(Pipeline::RankedVerification(
                                error @ Verification::ConditionalFinalizerRequired { .. },
                            )) = dispatch::Stage::lower(ranked)
                            else {
                                return Err(
                                    "Vecadd requires the exact conditional finalizer refusal"
                                        .into(),
                                );
                            };
                            assert!(error.to_string().contains("FE2O3-COND-FINALIZER-001"));
                            serde_json::json!({"boundary": error.to_string(), "conditional_formula": report})
                        }
                        (
                            Case::InputGuard,
                            Err(Pipeline::RankedProjection(Projection::ReferenceEffectJoin(
                                Join::UnsupportedGpuEffect { detail, .. },
                            ))),
                        ) if detail == GUARD_MISMATCH => {
                            fresh_proof(&proof, 0);
                            serde_json::json!({"boundary": detail, "negative": true})
                        }
                        (
                            Case::CpuRead,
                            Err(Pipeline::RankedProjection(Projection::ReferenceEffectJoin(
                                Join::UnsupportedReference(detail),
                            ))),
                        ) if detail == LOAD_MISMATCH => {
                            fresh_proof(&proof, 0);
                            serde_json::json!({"boundary": detail, "negative": true})
                        }
                        (
                            Case::SourceArgument,
                            Err(Pipeline::RankedProjection(Projection::ReferenceEffectJoin(
                                Join::UnsupportedReference(detail),
                            ))),
                        ) if detail == AMBIGUOUS_LOAD => {
                            fresh_proof(&proof, 0);
                            serde_json::json!({"boundary": detail, "negative": true})
                        }
                        (_, result) => {
                            return Err(format!(
                                "unexpected consuming boundary: {:?}",
                                result.err()
                            ));
                        }
                    }
                }
            };
            Ok(Report {
                case: self.case,
                stage: self.stage,
                detail,
                default_manifest_selection: self.case == Case::Manifest,
                qualification_credit: false,
                grants_artifact_or_launch_authority: false,
            })
        });
        self.result = Some(result.map(|mut report| {
            if self.case == Case::Annotated && self.stage == Stage::Consuming {
                retention::check(&retention, &report.detail["conditional_formula"]);
                report.detail["conditional_formula"]["proof_retained_after_callback"] = true.into();
                report.detail["retained_proof_events"] = serde_json::to_value(retention).unwrap();
            } else {
                assert!(
                    retention.events.is_empty(),
                    "unexpected retained proof phase"
                );
            }
            report
        }));
        Compilation::Stop
    }
}

#[test]
#[ignore = "actual-source child; parent supplies exact captured Cargo invocation"]
fn conditional_vecadd_source_child() {
    let request: Request =
        serde_json::from_str(&env::var(REQUEST).expect("parent request")).unwrap();
    let bytes = std::fs::read(env::var_os(CHILD_ARGS).expect("captured args")).unwrap();
    assert_eq!(
        request.args_sha256,
        <[u8; 32]>::from(Sha256::digest(&bytes))
    );
    assert_eq!(request.source, source_stamps(request.case));
    let args: Vec<String> = serde_json::from_slice(&bytes).unwrap();
    let mut callbacks = VecaddCallbacks {
        case: request.case,
        stage: request.stage,
        calls: 0,
        result: None,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert_eq!(callbacks.calls, 1, "actual rustc callback required");
    assert_eq!(request.source, source_stamps(request.case));
    let result = callbacks.result.expect("callback result");
    std::fs::write(
        env::var_os(CHILD_RESULT).expect("response"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "Vecadd source case {:?}/{:?}: {result:?}",
        request.case,
        request.stage
    );
}

fn run(cases: &[Case], target: &str, stages: &[Stage]) {
    let workspace = workspace();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-conditional-vecadd");
    for &case in cases {
        let before = source_stamps(case);
        let fixture = fixture(case, target);
        let directory = scratch.path().join(&fixture.fixture_id);
        std::fs::create_dir(&directory).unwrap();
        let captured = corpus_cargo::capture(
            &workspace,
            &fixture,
            &directory,
            &scratch.path().join(target),
        )
        .unwrap();
        let args = serde_json::to_vec(&captured.args).unwrap();
        let request_path = directory.join("args.json");
        std::fs::write(&request_path, &args).unwrap();
        for &stage in stages {
            let request = Request {
                case,
                stage,
                source: before.clone(),
                args_sha256: Sha256::digest(&args).into(),
            };
            let response = directory.join(format!("{stage:?}.json"));
            let mut command = Command::new(env::current_exe().unwrap());
            command
                .env_clear()
                .envs(captured.environment.iter().cloned())
                .current_dir(&captured.cwd)
                .env_remove("RUSTC_WRAPPER")
                .env_remove("RUSTC_WORKSPACE_WRAPPER")
                .env_remove(CHILD_PROOF_PROBE)
                .env(CHILD_ARGS, &request_path)
                .env(CHILD_RESULT, &response)
                .env(REQUEST, serde_json::to_string(&request).unwrap())
                .args([
                    "--exact",
                    CHILD,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ]);
            progress::clear_inherited_jobserver(&mut command);
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{case:?}/{stage:?}: {}",
                corpus_cargo::diagnostics(&output)
            );
            let report: Result<Report, String> =
                serde_json::from_slice(&std::fs::read(&response).unwrap()).unwrap();
            let report = report.unwrap();
            assert_eq!((report.case, report.stage), (case, stage));
            assert_eq!(report.default_manifest_selection, case == Case::Manifest);
            assert!(!report.qualification_credit && !report.grants_artifact_or_launch_authority);
            for entry in std::fs::read_dir(directory.join("compiler-output")).unwrap() {
                let path = entry.unwrap().path();
                assert!(
                    path.is_file() && path.extension().is_some_and(|extension| extension == "d"),
                    "diagnostic stop emitted a root artifact: {}",
                    path.display()
                );
            }
            assert_eq!(before, source_stamps(case));
            eprintln!(
                "VECADD SOURCE {}: {}",
                fixture.fixture_id,
                serde_json::json!({"request": request, "report": report})
            );
        }
    }
}

#[test]
#[ignore = "pinned nightly actual manifest source/CPU oracle; no protected runtime or GPU"]
fn actual_manifest_vecadd_source_requires_reference_annotation() {
    run(
        &[Case::Manifest],
        "gfx942",
        &[Stage::Oracle, Stage::Prepared],
    );
}

#[test]
#[ignore = "unvalidated integration: pinned nightly and admitted protected runtime; no GPU"]
fn actual_shared_body_vecadd_consumes_formula_then_requires_conditional_finalizer() {
    for target in ["gfx942", "gfx950"] {
        run(
            &[Case::Annotated],
            target,
            &[
                Stage::Oracle,
                Stage::Prepared,
                Stage::Bound,
                Stage::Consuming,
            ],
        );
    }
}

#[test]
#[ignore = "pinned nightly authenticated Rust negatives and CPU oracle; no GPU"]
fn actual_vecadd_input_guard_cpu_read_and_source_argument_negatives() {
    for target in ["gfx942", "gfx950"] {
        run(
            &[Case::InputGuard, Case::CpuRead, Case::SourceArgument],
            target,
            &[Stage::Oracle, Stage::Consuming],
        );
    }
}
