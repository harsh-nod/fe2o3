//! Actual Rust exercises the complete fixed production preparation, not a
//! test-created descriptor table stitched onto a final-native callback.

use super::*;
use crate::production_pipeline::source_owned_v29::target_result::mixed_licm_v28::worker_input_v26::PreparedMixedWorkerInputV26 as Worker;
use fe2o3_kernel_descriptor::mixed_conditional_v26::decode_mixed_contract_v26;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_licm_tests::worker_orchestration_tests::mixed_worker_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct WorkerObservation {
    roots: usize,
    original: [u8; 32],
    output: [u8; 32],
    descriptor_bindings: [[u8; 32]; 2],
    unused_arguments: usize,
    llvm_bytes: usize,
    target: u16,
    callbacks: [usize; 6],
    early_callbacks: usize,
    exact_storage: usize,
}

std::thread_local! {
    static STORAGE_CALLBACKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

type PeakConsumer = for<'n, 'h, 'v, 's, 't, 'wire, 'w> fn(
    &Worker<'n, 'h, 'v, 's, 't, 'wire>,
    &mut Budget<'w>,
) -> Result<usize, Error>;

fn observe_peak(
    worker: &Worker<'_, '_, '_, '_, '_, '_>,
    budget: &mut Budget<'_>,
) -> Result<usize, Error> {
    STORAGE_CALLBACKS.with(|count| count.set(count.get() + 1));
    assert_eq!(worker.root_count(budget)?, 2);
    Ok(budget.peak_storage())
}

#[derive(Default)]
struct WorkerCallbacks {
    result: Option<Result<WorkerObservation, String>>,
}

impl Callbacks for WorkerCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            let expected = transaction()?
                .with_original_source_mixed_licm_v28(|source, relocation, _, _, budget| {
                    Ok((
                        *source.canonical(budget)?.identity().digest(),
                        *relocation.tail(budget)?.output().identity().digest(),
                    ))
                })
                .map_err(|error| format!("independent final graph observation: {error:?}"))?
                .into_observation();
            let mut callbacks = [0; 6];
            let mut observation = transaction()?
                .with_original_source_mixed_worker_input_v28(|worker, budget| {
                    callbacks[0] += 1;
                    assert_eq!(worker.root_count(budget)?, 2);
                    assert_eq!(worker.open_gates().len(), 4);
                    assert!(!worker.grants_worker_or_artifact_authority());
                    let llvm_bytes = worker.llvm_ir(budget)?.len();
                    assert!(llvm_bytes > 0);
                    let table = worker.descriptor(budget)?;
                    assert_eq!(table.kernel_count(), 2);
                    assert_eq!(table.producer_version(), "source-owned-mixed-licm-v28");
                    let target_name = table.device_target().to_string();
                    let target = if target_name.starts_with("gfx942") {
                        942
                    } else {
                        assert!(target_name.starts_with("gfx950"));
                        950
                    };
                    let mut bindings = [[0; 32]; 2];
                    for (index, binding) in bindings.iter_mut().enumerate() {
                        let kernel = table.kernel(index, &mut |n| budget.charge_work(n)).unwrap();
                        assert_eq!(kernel.argument_count(), 5);
                        *binding = *kernel.kernel_id().as_bytes();
                    }
                    assert_ne!(bindings[0], bindings[1]);
                    let mut unused_arguments = 0;
                    for root in 0..2 {
                        let bytes = worker.contract(root, budget)?;
                        let contract =
                            decode_mixed_contract_v26(bytes, &mut |n| budget.charge_work(n))
                                .unwrap();
                        let subjects = contract.subjects();
                        assert_eq!(subjects.original_root, root as u32);
                        assert_eq!(subjects.original_graph_identity, expected.0);
                        assert_eq!(subjects.output_graph_identity, expected.1);
                        assert!(bindings.contains(&subjects.kernel_id));
                        assert_ne!(subjects.descriptor_identity, [0; 32]);
                        assert_eq!(subjects.source_argument_count, 5);
                        assert_eq!(contract.argument_count(), 3);
                        for index in 0..contract.argument_count() {
                            let argument = contract
                                .argument(index, &mut |n| budget.charge_work(n))
                                .unwrap();
                            if argument.source_argument == 2 {
                                assert_eq!((argument.reads, argument.writes), (0, 0));
                                unused_arguments += 1;
                            }
                        }
                    }
                    assert_eq!(unused_arguments, 2);
                    Ok(WorkerObservation {
                        roots: 2,
                        original: expected.0,
                        output: expected.1,
                        descriptor_bindings: bindings,
                        unused_arguments,
                        llvm_bytes,
                        target,
                        callbacks: [0; 6],
                        early_callbacks: 0,
                        exact_storage: 0,
                    })
                })
                .map_err(|error| format!("fixed mixed Worker preparation: {error:?}"))?
                .into_observation();

            let foreign =
                transaction()?.with_original_source_mixed_worker_input_v28::<(), _>(|worker, _| {
                    callbacks[1] += 1;
                    let mut work = Work::new(100_000);
                    let mut foreign = Budget::new(&mut work, 20_000_000);
                    worker.root_count(&mut foreign)?;
                    Ok(())
                });
            assert!(matches!(foreign, Err(Error::MixedLicmWorkerInput(_))));
            let restored = transaction()?.with_original_source_mixed_worker_input_v28::<(), _>(
                |worker, budget| {
                    callbacks[2] += 1;
                    let retained = worker.retained_storage(budget)?;
                    assert!(retained > 0);
                    budget.release_storage(retained)?;
                    let selected = worker.root_count(budget).unwrap_err();
                    budget.reserve_storage(retained)?;
                    assert!(
                        worker.root_count(budget).is_err(),
                        "restoring credit cannot repair custody"
                    );
                    Err(selected.into())
                },
            );
            assert!(matches!(restored, Err(Error::MixedLicmWorkerInput(_))));
            let missing = transaction()?.with_original_source_mixed_worker_input_v28::<(), _>(
                |worker, budget| {
                    callbacks[3] += 1;
                    worker.contract(2, budget)?;
                    Ok(())
                },
            );
            assert!(matches!(missing, Err(Error::MixedLicmWorkerInput(_))));
            let selected =
                transaction()?.with_original_source_mixed_worker_input_v28::<(), _>(|_, _| {
                    callbacks[4] += 1;
                    Err(Error::Unsupported("selected Worker consumer"))
                });
            assert!(matches!(
                selected,
                Err(Error::Unsupported("selected Worker consumer"))
            ));
            let transaction_for_panic = transaction()?;
            let unwind = catch_unwind(AssertUnwindSafe(|| {
                transaction_for_panic.with_original_source_mixed_worker_input_v28::<(), _>(
                    |_, _| {
                        callbacks[5] += 1;
                        std::panic::panic_any(809u32)
                    },
                )
            }));
            let payload = match unwind {
                Err(payload) => payload,
                Ok(_) => panic!("raw Worker panic swallowed"),
            };
            assert_eq!(*payload.downcast::<u32>().unwrap(), 809);
            assert_eq!(callbacks, [1; 6]);
            let mut early_callbacks = 0;
            for (work, storage) in [(0, 20_000_000), (500_000_000, 0)] {
                assert!(
                    transaction()?
                        .with_original_source_mixed_worker_test_limits_v28(work, storage, |_, _| {
                            early_callbacks += 1;
                            Ok(())
                        },)
                        .is_err()
                );
            }
            assert_eq!(early_callbacks, 0);
            // Identical function-pointer captures keep all entry/frame layouts
            // unchanged between independent measurement and boundary runs.
            let peak = transaction()?
                .with_original_source_mixed_worker_test_limits_v28(
                    500_000_000,
                    20_000_000,
                    observe_peak as PeakConsumer,
                )
                .map_err(|error| format!("measure whole mixed preparation: {error:?}"))?
                .into_observation();
            assert!(peak > 0);
            STORAGE_CALLBACKS.with(|count| count.set(0));
            let exact = transaction()?
                .with_original_source_mixed_worker_test_limits_v28(
                    500_000_000,
                    peak,
                    observe_peak as PeakConsumer,
                )
                .map_err(|error| format!("exact whole mixed preparation storage: {error:?}"))?
                .into_observation();
            assert_eq!(exact, peak);
            STORAGE_CALLBACKS.with(|count| {
                assert_eq!(count.get(), 1);
                count.set(0);
            });
            assert!(
                transaction()?
                    .with_original_source_mixed_worker_test_limits_v28(
                        500_000_000,
                        peak - 1,
                        observe_peak as PeakConsumer,
                    )
                    .is_err()
            );
            STORAGE_CALLBACKS.with(|count| assert_eq!(count.get(), 0));
            observation.callbacks = callbacks;
            observation.early_callbacks = early_callbacks;
            observation.exact_storage = peak;
            Ok(observation)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "private actual-rustc child; invoked only by the parent fixture"]
fn mixed_worker_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = WorkerCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual Worker callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("Worker result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "actual fixed mixed Worker preparation: {result:?}"
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src/rustc-dev and authentic AMD dependencies"]
fn actual_original_mixed_worker_preparation_retains_compiler_descriptors_and_final_licm_graph() {
    run_actual_sources::<WorkerObservation>(
        &[("straight", STRAIGHT), ("loop", LOOP)],
        &[(0, 0)],
        CHILD,
        "ORIGINAL_SOURCE_MIXED_WORKER_V28",
        program,
        |_, _, case, report, _| {
            assert_eq!((report.roots, report.unused_arguments), (2, 2));
            assert_eq!(report.callbacks, [1; 6]);
            assert_eq!(report.early_callbacks, 0);
            assert!(report.llvm_bytes > 0);
            assert!(report.exact_storage > 0);
            assert!(matches!(report.target, 942 | 950));
            assert_ne!(report.original, [0; 32]);
            assert_ne!(report.output, [0; 32]);
            if case == "loop" {
                assert_ne!(report.original, report.output);
            }
        },
    );
}

#[test]
fn actual_worker_matrix_uses_only_rust_inputs_and_the_fixed_preparation_api() {
    assert!(CHILD.ends_with("::worker_orchestration_tests::mixed_worker_child"));
    for body in [STRAIGHT, LOOP] {
        let source = program(body);
        assert_eq!(source.matches("#[kernel(").count(), 2);
        assert_eq!(source.matches("_unused: &[u32]").count(), 2);
    }
}
