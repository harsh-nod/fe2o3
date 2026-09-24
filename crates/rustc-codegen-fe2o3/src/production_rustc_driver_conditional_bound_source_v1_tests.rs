//! Actual reference-annotated source and protected receipts, with no clean-stage exit.
use super::*;
use crate::production_ranked_projection_v1::conditional_bound_observation_v1_tests as observed;

#[path = "production_rustc_driver_conditional_vecadd_source_v1_tests.rs"]
mod vecadd;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::conditional_bound_source::conditional_bound_source_child";
const ABSENT_RUNTIME_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::conditional_bound_source::absent_runtime_child";

#[derive(Default)]
struct BoundSourceCallbacks {
    result: Option<Result<Report, String>>,
    expect_missing_runtime: bool,
}

#[derive(Debug, Serialize, Deserialize)]
enum Report {
    Pending(observed::Observation),
    UnannotatedRefused,
    RuntimeUnavailable,
}

impl Callbacks for BoundSourceCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use crate::production_pipeline::ProductionPipelineError as Pipeline;
        use crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1 as Projection;
        use crate::production_reference_effect_join_v2::ProductionReferenceEffectJoinErrorV2 as Join;
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let (result, observation) =
                observed::observe(|| transaction.verify_general_kernel_checks());
            match result {
                Err(Pipeline::RankedProjection(Projection::Incomplete(observed::STOP))) => {
                    observation.map(Report::Pending)
                }
                Err(Pipeline::RankedProjection(Projection::Incomplete(observed::UNANNOTATED))) => {
                    if observation.is_ok() {
                        Err("unannotated source cannot have a proof-bound observation".into())
                    } else {
                        Ok(Report::UnannotatedRefused)
                    }
                }
                Err(Pipeline::RankedProjection(Projection::ReferenceEffectJoin(
                    Join::ProofRuntimeUnavailable { .. },
                ))) if self.expect_missing_runtime => {
                    assert!(observation.is_err());
                    Ok(Report::RuntimeUnavailable)
                }
                Err(error) => Err(format!("unexpected post-bind boundary: {error:?}")),
                Ok(_) => Err("post-bind observer must not yield a checked output stage".into()),
            }
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires its parent's actual-source invocation"]
fn conditional_bound_source_child() {
    run_child(false);
}

#[test]
#[ignore = "process helper; requires its parent's absent-runtime source invocation"]
fn absent_runtime_child() {
    run_child(true);
}

fn run_child(expect_missing_runtime: bool) {
    let request = env::var_os(CHILD_ARGS).expect("actual-source parent request");
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(request).unwrap()).unwrap();
    let mut callbacks = BoundSourceCallbacks {
        expect_missing_runtime,
        ..Default::default()
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("actual rustc callback did not run");
    let response = PathBuf::from(env::var_os(CHILD_RESULT).expect("observation path"));
    // The parent places root compiler outputs here. Dependency metadata lives
    // under target/; protected-proof intermediates are not compiler outputs.
    for entry in std::fs::read_dir(response.parent().unwrap()).unwrap() {
        let path = entry.unwrap().path();
        assert!(
            !matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("rmeta" | "rlib" | "o" | "obj" | "ll" | "bc" | "hsaco" | "so")
            ),
            "diagnostic stop produced root compiler output: {}",
            path.display()
        );
    }
    std::fs::write(response, serde_json::to_vec(&result).unwrap()).unwrap();
    assert!(
        result.is_ok(),
        "actual protected post-bind observation: {result:?}"
    );
    println!("{result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src and admitted protected Verus runtime; no GPU"]
fn actual_reference_fill_retains_pending_checks_after_proof_binding() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        ordinary_rust_source_cases(
            &[
                OrdinarySourceCase::ReferenceFill,
                OrdinarySourceCase::ProofFill,
            ],
            profile,
            false,
            Some(SourceObserver {
                child_test: CHILD,
                check: |path, roots| {
                    let result: Result<Report, String> =
                        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
                    let Report::Pending(result) =
                        result.expect("runtime absence is not a positive observation")
                    else {
                        panic!("reference-annotated source must reach actual proof binding");
                    };
                    assert_eq!([result.kernel.as_str()].as_slice(), roots);
                    assert_ne!(result.canonical_digest, [0; 32]);
                    assert_eq!(result.selected, 1);
                    assert_eq!(result.signed_receipts, 1);
                    assert!(result.memory_effects > 0 && result.value_expressions > 0);
                    assert_eq!(result.pending_checks, 9);
                    assert!(result.work > 0);
                    assert_eq!(result.output.source_argument, 0);
                    assert_eq!(result.output.adjusted_argument, 0);
                    assert_eq!(result.output.physical_argument, 0);
                    assert_eq!(result.output.raw_reference_argument, 1);
                    assert_eq!(result.output.element_bytes, 4);
                    assert_eq!(result.output.address_domain, "GlobalLaunch");
                },
            }),
        );
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src and actual unannotated source compilation; no GPU or Verus"]
fn unannotated_source_cannot_escape_the_post_bind_observation() {
    ordinary_rust_source_cases(
        &[OrdinarySourceCase::UnannotatedFill],
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        false,
        Some(SourceObserver {
            child_test: CHILD,
            check: |path, _| {
                let result: Result<Report, String> =
                    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
                assert!(matches!(result, Ok(Report::UnannotatedRefused)));
            },
        }),
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src and absence of the protected runtime; no GPU"]
fn missing_runtime_cannot_be_counted_as_a_post_bind_observation() {
    assert!(
        !Path::new("/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5")
            .exists()
    );
    ordinary_rust_source_cases(
        &[
            OrdinarySourceCase::ReferenceFill,
            OrdinarySourceCase::ProofFill,
        ],
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        false,
        Some(SourceObserver {
            child_test: ABSENT_RUNTIME_CHILD,
            check: |path, _| {
                let result: Result<Report, String> =
                    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
                assert!(matches!(result, Ok(Report::RuntimeUnavailable)));
            },
        }),
    );
}
