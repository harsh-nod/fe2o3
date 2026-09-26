//! Real collected Rust reaches the closed production policy continuation.
use super::*;
use crate::production_pipeline::ProductionPipelineError;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as OwnedBudget,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::optimized_policies_v18_tests::optimized_policy_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Observation {
    required: String,
    preparation_refused: bool,
    execution_recipes: usize,
    lifecycle_operations: usize,
    native_completed: bool,
    defined_functions: usize,
    native_resource_cuts: usize,
    work: usize,
    retained: usize,
}

#[derive(Default)]
struct PolicyCallbacks {
    result: Option<Result<Observation, String>>,
}

impl Callbacks for PolicyCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let work = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                .map_err(|_| "test work limit conversion")?;
            // Capture all original sources before collection can consume MIR.
            // Each transaction keeps its own receipt; no retained capture is cloned.
            let mut transactions = transactions_with_original_sources_for_test_v1::<5>(tcx)?;
            let transaction = transactions
                .next()
                .ok_or("missing preparation-probe source")??;
            let mut denied = OwnedBudget::new(Work::new(work), 0);
            let refused = transaction.inspect_optimized_source_policies_v18(&mut denied);
            assert!(
                matches!(
                    &refused,
                    Err(ProductionPipelineError::SourceOwnedEntrance(
                        fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Storage(
                                _
                            )
                        )
                    ))
                ),
                "source preparation must refuse before policy: {:?}",
                refused.as_ref().err()
            );
            assert_eq!(denied.work(), 0);
            assert_eq!(denied.storage(), 0);
            assert!(denied.failed_storage().is_some());
            let transaction = transactions
                .next()
                .ok_or("missing typed execution-recipe source")??;
            let mut recipes_account = OwnedBudget::new(
                Work::new(work),
                crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
            );
            let recipes = transaction
                .inspect_optimized_execution_recipes_v18(&mut recipes_account)
                .map_err(|error| format!("actual typed lifecycle recipe: {error:?}"))?;
            let execution_recipes = recipes.1;
            assert!(execution_recipes >= 1, "real original context issuance");
            assert_eq!(
                (
                    recipes_account.failed_work(),
                    recipes_account.failed_storage()
                ),
                (None, None)
            );
            let transaction = transactions
                .next()
                .ok_or("missing native-policy source")??;
            let mut account = OwnedBudget::new(
                Work::new(work),
                crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
            );
            let (result, probe) =
                transaction.inspect_optimized_source_policies_observed_v18(&mut account);
            let probe = probe.ok_or("actual native candidate was never observed")?;
            assert!(probe.operation_count >= probe.lifecycle && probe.lifecycle > 0);
            use fe2o3_lower_mir_kernel::ProductionSourceNativeLifecycleErrorV18 as NativeError;
            let (required, defined_functions) = match (result, probe.first_unresolved) {
                (
                    Err(ProductionPipelineError::SourceNativeLifecycle(error)),
                    Some((coordinate, requirement)),
                ) => {
                    let NativeError::Unresolved(obligation) = error.as_ref() else {
                        return Err(format!("wrong actual census refusal: {error:?}"));
                    };
                    assert_eq!(obligation.coordinate(), coordinate);
                    assert_eq!(obligation.requirement(), requirement);
                    assert!(error.native_diagnostic().is_none());
                    assert_eq!((probe.native_entries, probe.consumers), (0, 0));
                    (format!("{requirement:?}"), 0)
                }
                (Ok(output), None) => {
                    assert!(!output.0.grants_authority());
                    assert!(output.1.defined_functions > 0);
                    assert_eq!((probe.native_entries, probe.consumers), (1, 1));
                    ("lifecycle-complete".to_owned(), output.1.defined_functions)
                }
                (other, expected) => {
                    return Err(format!(
                        "actual census {expected:?} disagrees with native result: error={:?}",
                        other.as_ref().err()
                    ));
                }
            };
            assert!(account.work() > 0);
            assert_eq!(
                (account.failed_work(), account.failed_storage()),
                (None, None)
            );
            let mut native_resource_cuts = 0;
            for storage_short in [false, true] {
                let transaction = transactions
                    .next()
                    .ok_or("missing actual native resource-cut source")??;
                let mut cut_account = OwnedBudget::new(
                    Work::new(work),
                    crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
                );
                let (result, cut_probe) = transaction
                    .inspect_optimized_source_policies_resource_cut_v18(
                        &mut cut_account,
                        storage_short,
                    );
                let cut_probe =
                    cut_probe.ok_or("resource control lost the exact actual candidate")?;
                assert_eq!(cut_probe.first_unresolved, probe.first_unresolved);
                match (result, cut_probe.first_unresolved) {
                    (Err(ProductionPipelineError::SourceNativeLifecycle(error)), None) => {
                        let NativeError::SourceAfterNative { source, diagnostic } = error.as_ref()
                        else {
                            panic!("native history erased by outer source cleanup: {error:?}");
                        };
                        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
                        use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
                        match source {
                            SourceError::Resource(Resource::Storage(limit)) if storage_short => {
                                assert_eq!(
                                    limit.actual(),
                                    crate::production_canonical_phase_policy_v1::STORAGE_LIMIT + 1
                                )
                            }
                            SourceError::Resource(Resource::Work(_)) if !storage_short => (),
                            other => panic!("wrong primary resource refusal: {other:?}"),
                        }
                        assert!(diagnostic.last_invocation().is_some());
                        assert!(diagnostic.observation().work_upper_bound() > 0);
                        assert_eq!((cut_probe.native_entries, cut_probe.consumers), (1, 0));
                        native_resource_cuts += 1;
                    }
                    (
                        Err(ProductionPipelineError::SourceNativeLifecycle(error)),
                        Some((coordinate, requirement)),
                    ) => {
                        let NativeError::Unresolved(obligation) = error.as_ref() else {
                            panic!("{error:?}");
                        };
                        assert_eq!(
                            (obligation.coordinate(), obligation.requirement()),
                            (coordinate, requirement)
                        );
                        assert!(error.native_diagnostic().is_none());
                        assert_eq!((cut_probe.native_entries, cut_probe.consumers), (0, 0));
                        assert_eq!(
                            (cut_account.failed_work(), cut_account.failed_storage()),
                            (None, None)
                        );
                    }
                    (other, _) => panic!(
                        "unexpected actual resource-cut result: error={:?}",
                        other.as_ref().err()
                    ),
                }
            }
            assert!(transactions.next().is_none());
            Ok(Observation {
                required,
                preparation_refused: true,
                execution_recipes,
                lifecycle_operations: probe.lifecycle,
                native_completed: defined_functions > 0,
                defined_functions,
                native_resource_cuts,
                work: account.work(),
                retained: account.storage(),
            })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires the actual-source request from its parent"]
fn optimized_policy_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = PolicyCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual production policy callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("policy report path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual optimized policy: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_rust_optimized_output_reaches_fixed_policy_missing_recipe_gate() {
    run_actual_sources::<Observation>(
        &[
            ("plain", "let value = seed;"),
            ("workgroup", "let value = ctx.with_workgroup(|_wg| 7_u32);"),
            (
                "repeated_callers",
                "#[inline(never)] fn shared(value: u32) -> u32 { value.wrapping_add(1) } #[inline(never)] fn left(value: u32) -> u32 { shared(value) } #[inline(never)] fn right(value: u32) -> u32 { shared(value) } let value = left(seed).wrapping_add(right(seed));",
            ),
        ],
        &[(0, 0), (3, 2)],
        CHILD,
        "SOURCE_OPTIMIZED_V18_NATIVE_POLICY",
        source,
        |_, _, label, observation, _| {
            assert_ne!(observation.required, "lifecycle-complete");
            assert!(!observation.native_completed);
            assert_eq!(observation.native_resource_cuts, 0);
            assert!(observation.preparation_refused);
            assert!(observation.execution_recipes >= 1);
            if label == "workgroup" {
                assert!(observation.execution_recipes >= 3);
            }
            assert!(observation.work > 0);
        },
    );
    run_actual_sources::<Observation>(
        &[
            ("context_only", "let _ = seed;"),
            ("workgroup_only", "let _ = ctx.with_workgroup(|_wg| seed);"),
        ],
        &[(0, 0), (3, 2)],
        CHILD,
        "SOURCE_OPTIMIZED_V18_NATIVE_LIFECYCLE",
        |body| {
            format!(
                r#"use fe2o3_device::{{kernel, KernelContext}};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn lifecycle_probe(mut ctx: KernelContext<'_>, seed: u32) {{ {body} }}
"#
            )
        },
        |_, _, _, observation, _| {
            assert_eq!(observation.required, "lifecycle-complete");
            assert!(observation.native_completed && observation.defined_functions > 0);
            assert_eq!(observation.native_resource_cuts, 2);
            assert!(observation.preparation_refused && observation.execution_recipes > 0);
            assert!(observation.lifecycle_operations > 0);
        },
    );
}
