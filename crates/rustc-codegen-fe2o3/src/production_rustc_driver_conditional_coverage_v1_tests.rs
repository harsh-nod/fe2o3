//! Observe conditional coverage on ordinary Rust's actual canonical graph.
//! Stops before ranked admission, proofs, native lowering, artifacts or GPU execution.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    ConditionalTotalViewAnalysisV1, derive_conditional_total_view_from_verified_v1,
};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::conditional_coverage::conditional_coverage_child";

#[derive(Debug, Serialize, Deserialize)]
struct RootObservation {
    root: String,
    conditional: bool,
    unsupported: Option<String>,
    address_domain: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CoverageObservation {
    roots: Vec<RootObservation>,
    canonical_digest: [u8; 32],
    work: usize,
    peak_storage: usize,
    simulation: Option<Vec<simulation::ConditionalCoverageScenario>>,
}

#[derive(Default)]
struct CoverageCallbacks {
    result: Option<Result<CoverageObservation, String>>,
}

impl Callbacks for CoverageCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .map_err(|error| error.to_string())?;
            transaction
                .observe_pre_ranked_for_test_v1(|materialized| {
                    assert!(!materialized.grants_artifact_or_launch_authority());
                    let owner = materialized.executable();
                    // This separate test-observation phase is not a claim about
                    // the work used by the preceding production materializer.
                    let mut work = Work::new(1_000_000);
                    let mut budget = Budget::new(&mut work, 4 * 1024 * 1024);
                    let mut roots = Vec::new();
                    for kernel in &owner.module().kernels {
                        let analysis = derive_conditional_total_view_from_verified_v1(
                            owner.verified_module_ref_v1(),
                            &kernel.id,
                            &mut budget,
                        )
                        .map_err(|error| error.to_string())?;
                        let (conditional, unsupported, address_domain) = match analysis {
                            ConditionalTotalViewAnalysisV1::Established(facts) => {
                                (true, None, Some(format!("{:?}", facts.address_domain())))
                            }
                            ConditionalTotalViewAnalysisV1::Unsupported(reason) => {
                                (false, Some(format!("{reason:?}")), None)
                            }
                        };
                        roots.push(RootObservation {
                            root: kernel.id.as_str().to_owned(),
                            conditional,
                            unsupported,
                            address_domain,
                        });
                    }
                    assert_eq!(budget.storage(), 0);
                    let simulation = match simulation::requested().map_err(|e| format!("{e:?}"))? {
                        Some(simulation::Case::Fill) => Some(
                            simulation::observe_conditional_fill_prefix(owner.canonical())
                                .map_err(|e| format!("{e:?}"))?,
                        ),
                        None => None,
                        Some(_) => return Err("unsupported source coverage test oracle".into()),
                    };
                    Ok(CoverageObservation {
                        roots,
                        canonical_digest: *owner.canonical().identity().digest(),
                        work: budget.work(),
                        peak_storage: budget.peak_storage(),
                        simulation,
                    })
                })
                .map_err(|error| error.to_string())?
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an actual-source request from its parent"]
fn conditional_coverage_child() {
    let Some(request) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(request).unwrap()).unwrap();
    let mut callbacks = CoverageCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("actual rustc callback did not run");
    std::fs::write(
        env::var_os(CHILD_RESULT).expect("observation path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "conditional coverage observation: {result:?}"
    );
    println!("{result:?}");
}

fn check(response: &Path, expected_roots: &[&str], conditional: bool) {
    let result: Result<CoverageObservation, String> =
        serde_json::from_slice(&std::fs::read(response).unwrap()).unwrap();
    let result = result.unwrap();
    assert_ne!(result.canonical_digest, [0; 32]);
    assert!(result.work > 0);
    // Source materialization order is not the final descriptor's display order.
    // Keep observations in actual graph order, but compare the complete roster.
    let mut roots = result
        .roots
        .iter()
        .map(|root| root.root.as_str())
        .collect::<Vec<_>>();
    let mut expected = expected_roots.to_vec();
    roots.sort_unstable();
    expected.sort_unstable();
    assert_eq!(roots, expected, "{result:?}");
    for root in &result.roots {
        assert_eq!(root.conditional, conditional, "{result:?}");
        assert_eq!(root.unsupported.is_none(), conditional, "{result:?}");
        assert_eq!(
            root.address_domain.as_deref(),
            conditional.then_some("GlobalLaunch"),
            "the source computes the direct address before testing the guard: {result:?}",
        );
        if !conditional {
            assert!(
                root.unsupported.as_deref().unwrap().starts_with("Call {"),
                "the retained helper must be the unsupported premise: {result:?}",
            );
        }
    }
    if conditional {
        let scenarios = result
            .simulation
            .as_ref()
            .expect("exact source simulations");
        assert_eq!(scenarios.len(), 5);
        for (scenario, length) in scenarios.iter().zip([0, 1, 63, 64, 65]) {
            assert_eq!(scenario.length, length);
            assert_eq!(scenario.global_x, 64);
            assert_eq!(scenario.expected_written_elements, length.min(64));
            assert_eq!(scenario.expected_total_output, length <= 64);
        }
    } else {
        assert!(result.simulation.is_none());
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src and actual AMD-target source compilation; no GPU or Verus"]
fn ordinary_fill_has_conditional_total_output_coverage() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        ordinary_rust_source_cases(
            &[OrdinarySourceCase::Fill],
            profile,
            false,
            Some(SourceObserver {
                child_test: CHILD,
                check: |path, roots| check(path, roots, true),
            }),
        );
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src and actual AMD-target source compilation; no GPU or Verus"]
fn retained_helper_coverage_is_not_inferred_from_a_guarded_store() {
    ordinary_rust_source_cases(
        &[OrdinarySourceCase::SharedUnitHelper],
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        false,
        Some(SourceObserver {
            child_test: CHILD,
            check: |path, roots| check(path, roots, false),
        }),
    );
}
