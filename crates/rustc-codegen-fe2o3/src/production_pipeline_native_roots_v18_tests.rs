//! The private attachment is exercised inside the actual source/native scope.
use super::*;
use crate::production_pipeline::ProductionPipelineError;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as OwnedBudget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::{
    ProductionSourceNativeLifecycleErrorV18 as NativeError,
    ProductionSourceOwnedViewErrorV18 as SourceError,
};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::native_roots_v18_tests::native_root_child";

#[path = "production_pipeline_private_bridge_scratch_v18_tests.rs"]
mod private_bridge_scratch;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Observation {
    roots: usize,
    row_mutations: usize,
    completed_negative_scopes: usize,
    preserved_native_histories: usize,
}

#[derive(Default)]
struct RootCallbacks {
    result: Option<Result<Observation, String>>,
}

impl Callbacks for RootCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let mut transactions = transactions_with_original_sources_for_test_v1::<9>(tcx)?;
            let work = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                .map_err(|_| "test work limit conversion")?;
            let mut observation = Observation {
                roots: 0,
                row_mutations: 0,
                completed_negative_scopes: 0,
                preserved_native_histories: 0,
            };
            let mut dipped_error_storage = None;
            let mut dipped_panic_storage = None;
            let mut selected_error_storage = None;
            for mode in 0..9 {
                let transaction = transactions
                    .next()
                    .ok_or("missing original root transaction")??;
                let mut budget = OwnedBudget::new(
                    Work::new(work),
                    crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
                );
                let ((result, native), roots) =
                    crate::production_ranked_projection_v1::with_actual_root_controls_v18(
                        mode,
                        || transaction.inspect_optimized_source_policies_observed_v18(&mut budget),
                    );
                let roots = roots.ok_or_else(|| {
                    format!(
                        "root attachment did not complete mode{mode}: {:?}",
                        result.as_ref().err()
                    )
                })?;
                assert!(
                    roots.completed,
                    "caught panic cannot substitute for a completed control"
                );
                assert_eq!(roots.roots, 2);
                let native = native.ok_or("actual output/native observation is absent")?;
                assert!(native.lifecycle >= 2);
                if native.private_memory {
                    assert!(matches!(native.first_unresolved, Some((_,
                        fe2o3_pliron::CanonicalRankedSourceRequirementV18::Memory))));
                } else { assert!(native.first_unresolved.is_none()); }
                if mode == 0 {
                    let actual =
                        result.map_err(|error| format!("multi-root native positive: {error:?}"))?;
                    assert!(!actual.0.grants_authority());
                    assert_eq!(actual.1.defined_functions, 2);
                    assert_eq!(roots.mutations, 10);
                    assert_eq!((native.native_entries, native.consumers), (1, 1));
                    assert_eq!(
                        (budget.failed_work(), budget.failed_storage()),
                        (None, None)
                    );
                    observation.roots = roots.roots;
                    observation.row_mutations = roots.mutations;
                } else {
                    let Err(ProductionPipelineError::SourceNativeLifecycle(error)) = result else {
                        return Err(format!(
                            "mode{mode} swallowed source failure: {:?}",
                            result.as_ref().err()
                        ));
                    };
                    let NativeError::SourceAfterNative { source, diagnostic } = error.as_ref()
                    else {
                        return Err(format!(
                            "mode{mode} erased accepted native history: {error:?}"
                        ));
                    };
                    match (mode, source) {
                        (1, SourceError::Resource(Resource::Accounting)) => (),
                        (
                            2,
                            SourceError::Binding(
                                "native root attachment query is outside its roster",
                            ),
                        ) => (),
                        (
                            5 | 8,
                            SourceError::Binding("selected root attachment callback error"),
                        ) => (),
                        (6, SourceError::Resource(Resource::Accounting)) => (),
                        (7, SourceError::Binding(detail)) if *detail == if native.private_memory {
                            "private native source subject changed"
                        } else { "native root attachment substituted its original source" } => (),
                        (3, SourceError::Resource(Resource::Work(limit))) => {
                            assert_eq!(limit.limit(), work);
                            assert!(limit.actual() > work);
                        }
                        (4, SourceError::Resource(Resource::Storage(limit))) => {
                            let expected =
                                crate::production_canonical_phase_policy_v1::STORAGE_LIMIT;
                            assert_eq!((limit.limit(), limit.actual()), (expected, expected + 1));
                        }
                        (_, other) => {
                            return Err(format!(
                                "mode{mode} changed first source error: {other:?}"
                            ));
                        }
                    }
                    assert!(diagnostic.last_invocation().is_some());
                    assert!(diagnostic.observation().work_upper_bound() > 0);
                    assert_eq!(native.consumers, 0);
                    observation.completed_negative_scopes += 1;
                    observation.preserved_native_histories += 1;
                }
                match mode {
                    5 => dipped_error_storage = Some(budget.storage()),
                    6 => dipped_panic_storage = Some(budget.storage()),
                    8 => selected_error_storage = Some(budget.storage()),
                    _ => (),
                }
            }
            assert!(transactions.next().is_none());
            // Same original/native inputs and identical attachment allocation.
            // Merely latching Accounting without denying source cleanup would
            // release the same credits as mode8, less the removed single byte.
            let selected = selected_error_storage.unwrap();
            assert!(dipped_error_storage.unwrap() > selected);
            assert!(dipped_panic_storage.unwrap() > selected);
            Ok(observation)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires its actual original-source parent"]
fn native_root_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = RootCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("actual root callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("root report path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual native root attachment: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK and original source compilation"]
fn actual_native_root_attachment_keeps_bijection_custody_launch_and_failure_history() {
    run_actual_sources::<Observation>(
        &[
            ("context_roots", "let _ = seed;"),
            ("workgroup_roots", "let _ = ctx.with_workgroup(|_wg| seed);"),
            (
                "repeated_helpers",
                "#[inline(never)] fn shared(v: u32) -> u32 { v.wrapping_add(1) } #[inline(never)] fn left(v: u32) -> u32 { shared(v) } #[inline(never)] fn right(v: u32) -> u32 { shared(v) } let _ = left(seed).wrapping_add(right(seed));",
            ),
        ],
        &[(0, 0), (3, 2)],
        CHILD,
        "SOURCE_NATIVE_V18_ROOT_ATTACHMENT",
        |body| {
            format!(
                r#"use fe2o3_device::{{kernel, KernelContext}};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn first(mut ctx: KernelContext<'_>, seed: u32) {{ {body} }}
#[kernel(typed, launch(required = [32, 1, 1], max = [32, 1, 1]))]
pub fn second(mut ctx: KernelContext<'_>, seed: u32) {{ {body} }}
"#
            )
        },
        |_, _, _, observation, _| {
            assert_eq!(
                observation,
                Observation {
                    roots: 2,
                    row_mutations: 10,
                    completed_negative_scopes: 8,
                    preserved_native_histories: 8
                }
            );
        },
    );
}
