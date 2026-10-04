//! Ordinary reference-annotated Rust, stopped at the actual prepared request.
//! No protected runtime, proof, ranked admission, artifact or GPU is invoked.
use super::*;
use crate::production_ranked_projection_v1::conditional_output_observation_v1_tests as observed;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::conditional_ranked_output::conditional_ranked_output_child";

#[derive(Default)]
struct RankedOutputCallbacks {
    result: Option<Result<Report, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
enum Report {
    Prepared(observed::Observation),
    UnannotatedRefused,
}
impl Callbacks for RankedOutputCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use crate::production_pipeline::ProductionPipelineError as Pipeline;
        use crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1 as Projection;
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let (result, observation) =
                observed::observe(|| transaction.verify_general_kernel_checks());
            match result {
                Err(Pipeline::RankedProjection(Projection::Incomplete(observed::STOP))) => {
                    observation.map(Report::Prepared)
                }
                Err(Pipeline::RankedProjection(Projection::Incomplete(observed::UNANNOTATED))) => {
                    if observation.is_ok() {
                        Err("unannotated root cannot have a prepared reference request".into())
                    } else {
                        Ok(Report::UnannotatedRefused)
                    }
                }
                Err(error) => Err(format!(
                    "unexpected conditional observation boundary: {error:?}"
                )),
                Ok(_) => Err("observation must stop before proving or admitting a root".into()),
            }
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires its parent's actual-source invocation"]
fn conditional_ranked_output_child() {
    let Some(request) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(request).unwrap()).unwrap();
    let mut callbacks = RankedOutputCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("actual rustc callback did not run");
    std::fs::write(
        env::var_os(CHILD_RESULT).expect("observation path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "conditional ranked correspondence: {result:?}"
    );
    println!("{result:?}");
}

fn check(path: &Path, roots: &[&str]) {
    let result: Result<Report, String> =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let Report::Prepared(result) = result.unwrap() else {
        panic!("reference-annotated source must reach the prepared request");
    };
    assert_eq!([result.kernel.as_str()].as_slice(), roots);
    assert_ne!(result.canonical_digest, [0; 32]);
    assert_eq!(result.source_argument, 0);
    assert_eq!(result.physical_parameter, 0);
    // The producer normalizes past the CPU point-coordinate prefix. Equality
    // here is not proof of the reference's logical ABI relation.
    assert_eq!(result.reference_argument, 0);
    // The child checked this operand's interpretation against the exact
    // canonical SliceLength value, not against its numerical ordinal.
    assert_eq!(result.ranked_extent_argument, 0);
    assert_eq!(result.address_domain, "GlobalLaunch");
    assert!(result.work > 0);
}

#[test]
#[ignore = "requires pinned nightly rust-src and actual unannotated source compilation; no GPU or Verus"]
fn unannotated_source_cannot_escape_the_pre_proof_observation() {
    ordinary_rust_source_cases(
        &[OrdinarySourceCase::UnannotatedFill],
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        false,
        Some(SourceObserver {
            child_test: CHILD,
            check: |path, _roots| {
                let result: Result<Report, String> =
                    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
                assert!(matches!(result, Ok(Report::UnannotatedRefused)));
            },
        }),
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src and actual AMD-target source compilation; no GPU or Verus"]
fn actual_reference_fill_joins_canonical_source_and_prepared_ranked_output() {
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
                check,
            }),
        );
    }
}
