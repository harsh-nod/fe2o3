//! Original Rust reaches the nominal publication candidate through the shared
//! Worker pipeline. Extraction custody cannot become protected custody.

use super::*;
use crate::production_pipeline::source_owned_v29::mixed_worker_v28::publication::{
    MixedPublicationErrorV28, MixedPublicationOpenGateV28, PreparedMixedPublicationV28,
};
use fe2o3_kernel_ir::{
    CanonicalGuardedGlobalReadErrorV1, CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrVerificationStorageLimitV1 as StorageLimit,
    FormalGuardedMemoryResourceErrorV1,
};
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_licm_tests::worker_orchestration_tests::publication_tests::mixed_publication_child";

#[derive(Debug, Serialize, Deserialize)]
struct PublicationObservation {
    roots: usize,
    generated_bytes: usize,
    original: [u8; 32],
    output: [u8; 32],
    statement: [u8; 32],
    exact_storage: usize,
    extraction_refused: bool,
}

std::thread_local! {
    static CALLBACKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
type Consumer = for<'a, 'v, 's, 'w> fn(
    PreparedMixedPublicationV28<'a, 'v, 's>,
    &mut Budget<'w>,
) -> Result<PublicationObservation, Error>;

fn storage_refusal(error: &Error) -> StorageLimit {
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(cause) = current {
        if let Some(Resource::Storage(limit)) = cause.downcast_ref::<Resource>() {
            return *limit;
        }
        if let Some(CanonicalGuardedGlobalReadErrorV1::Resource(
            FormalGuardedMemoryResourceErrorV1::Storage { actual, limit },
        )) = cause.downcast_ref::<CanonicalGuardedGlobalReadErrorV1>()
        {
            return StorageLimit::new(*actual, *limit);
        }
        current = cause.source();
    }
    panic!("typed publication storage refusal required: {error:?}");
}

fn work_refusal(error: &Error) -> fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1 {
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(cause) = current {
        if let Some(Resource::Work(limit)) = cause.downcast_ref::<Resource>() {
            return *limit;
        }
        if let Some(CanonicalGuardedGlobalReadErrorV1::Resource(
            FormalGuardedMemoryResourceErrorV1::Work(limit),
        )) = cause.downcast_ref::<CanonicalGuardedGlobalReadErrorV1>()
        {
            return *limit;
        }
        if let Some(limit) = cause.downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1>() {
            return *limit;
        }
        current = cause.source();
    }
    panic!("typed composed execution work refusal required: {error:?}");
}

fn observe(
    candidate: PreparedMixedPublicationV28<'_, '_, '_>,
    budget: &mut Budget<'_>,
) -> Result<PublicationObservation, Error> {
    CALLBACKS.with(|n| n.set(n.get() + 1));
    candidate.replay(budget)?;
    let subject = candidate.refinement_subject(budget)?;
    assert!(subject.models_original_to_final_composition());
    let expressions = subject.expressions();
    assert_eq!(
        candidate
            .source(budget)?
            .source_semantic(budget)?
            .semantic_sha256()
            .as_bytes(),
        &expressions.source_semantic_identity(),
    );
    assert_eq!(
        candidate
            .source(budget)?
            .source_ssa(budget)?
            .identity()
            .as_bytes(),
        &expressions.source_ssa_identity(),
    );
    assert_eq!(
        *candidate.native(budget)?.output(budget)?.identity(),
        expressions.output()
    );
    let generated_bytes = candidate.generated_source(budget)?.len();
    assert!(generated_bytes > 0);
    assert!(!candidate.grants_publication_or_artifact_authority());
    assert_eq!(
        candidate.open_gates(),
        &[
            MixedPublicationOpenGateV28::OriginalMirToKirRefinement,
            MixedPublicationOpenGateV28::ExecutedComposedRefinement,
            MixedPublicationOpenGateV28::MixedSemanticCapsuleTransport,
            MixedPublicationOpenGateV28::ProtectedCompilerExecutionJoin,
            MixedPublicationOpenGateV28::FinalArtifactIdentityAndAdmission,
            MixedPublicationOpenGateV28::SealedMixedWorkerFinalizerReplay,
            MixedPublicationOpenGateV28::ConcreteRuntimePremiseDischarge,
        ]
    );
    let worker = candidate.worker(budget)?;
    let roots = worker.root_count(budget)?;
    assert_eq!(roots, 2);
    assert!(!worker.llvm_ir(budget)?.is_empty());
    assert_eq!(worker.descriptor(budget)?.kernel_count(), roots);
    for root in 0..roots {
        let bytes = worker.contract(root, budget)?;
        let contract = decode_mixed_contract_v26(bytes, &mut |n| budget.charge_work(n)).unwrap();
        assert_eq!(
            contract.subjects().original_graph_identity,
            *expressions.input().digest()
        );
        assert_eq!(
            contract.subjects().output_graph_identity,
            *expressions.output().digest()
        );
    }
    let result = candidate.into_protected(budget);
    assert!(matches!(
        result,
        Err(Error::MixedPublication(
            MixedPublicationErrorV28::ExtractionOnly
        ))
    ));
    Ok(PublicationObservation {
        roots,
        generated_bytes,
        original: *expressions.input().digest(),
        output: *expressions.output().digest(),
        statement: subject.statement_identity(),
        exact_storage: budget.peak_storage(),
        extraction_refused: true,
    })
}

type ExecutionConsumer = for<'a, 'v, 's, 'w> fn(
    PreparedMixedPublicationV28<'a, 'v, 's>,
    &mut Budget<'w>,
) -> Result<(), Error>;

fn observe_execution_preparation(
    candidate: PreparedMixedPublicationV28<'_, '_, '_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let floor = budget.storage();
    let subject = candidate.refinement_subject(budget)?;
    let prepared = candidate.prepare_execution_for_test(budget)?;
    assert!(!prepared.authenticates_executed_proof());
    let witness = prepared
        .prefix_witness(budget)
        .map_err(Error::MixedRelocationExpressions)?;
    assert!(witness.len() > 352);
    assert_eq!(
        <[u8; 32]>::from(sha2::Sha256::digest(witness)),
        subject.expressions().prefix_execution_identity(),
    );
    prepared
        .discard(budget)
        .map_err(Error::MixedRelocationExpressions)?;
    assert_eq!(budget.storage(), floor);
    assert!(!candidate.grants_publication_or_artifact_authority());
    Ok(())
}

#[derive(Default)]
struct PublicationCallbacks {
    result: Option<Result<PublicationObservation, String>>,
}
impl Callbacks for PublicationCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            // This executes preparation only. No runtime is fabricated or run,
            // and no executed/refinement/publication owner is constructed.
            let run_preparation = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                let result = transaction()?.with_original_source_mixed_publication_on_account_v28(
                    &mut budget,
                    observe_execution_preparation as ExecutionConsumer,
                );
                Ok::<_, String>((result, budget.work(), budget.peak_storage()))
            };
            let (measured, execution_work, execution_storage) =
                run_preparation(500_000_000, 20_000_000)?;
            measured.map_err(|error| format!("composed execution preparation: {error:?}"))?;
            let (exact, exact_work, exact_storage) =
                run_preparation(execution_work, execution_storage)?;
            exact.map_err(|error| format!("exact composed execution preparation: {error:?}"))?;
            assert_eq!(
                (exact_work, exact_storage),
                (execution_work, execution_storage)
            );
            let (short_work, _, _) = run_preparation(execution_work - 1, execution_storage)?;
            let error = match short_work {
                Err(error) => error,
                Ok(_) => panic!("one-short composed execution work admitted"),
            };
            assert_eq!(work_refusal(&error).limit(), execution_work - 1);
            let (short_storage, _, _) = run_preparation(execution_work, execution_storage - 1)?;
            let error = match short_storage {
                Err(error) => error,
                Ok(_) => panic!("one-short composed execution storage admitted"),
            };
            assert_eq!(storage_refusal(&error).limit(), execution_storage - 1);
            let sticky = transaction()?.with_original_source_mixed_publication_test_limits_v28(
                500_000_000,
                20_000_000,
                |candidate, budget| -> Result<(), Error> {
                    let source = candidate.source(budget)?;
                    let floor = budget.storage();
                    let pending = candidate.prepare_execution_for_test(budget)?;
                    assert!(budget.storage() > floor);
                    let refused = budget.charge_work(500_000_001).unwrap_err();
                    let expected = match &refused {
                        Resource::Work(limit) => *limit,
                        error => panic!("expected the original work refusal: {error:?}"),
                    };
                    let selected = source.retain_query_resource_error_v18(refused);
                    for error in [
                        pending.prefix_witness(budget).unwrap_err(),
                        pending.discard(budget).unwrap_err(),
                    ] {
                        match error {
                            fe2o3_verifier::MixedOptimizerRelocationErrorV28::Source(
                                ProductionSourceOwnedViewErrorV18::Resource(Resource::Work(limit)),
                            ) => {
                                assert_eq!(limit.actual(), expected.actual());
                                assert_eq!(limit.limit(), expected.limit());
                            }
                            error => {
                                panic!("original sticky work refusal must stay selected: {error:?}")
                            }
                        }
                    }
                    assert_eq!(
                        budget.storage(),
                        floor,
                        "intact same-ledger preparation credit must settle after a query refusal"
                    );
                    Err(Error::Source(selected))
                },
            );
            let error = match sticky {
                Err(error) => error,
                Ok(_) => panic!("sticky composed execution refusal lost"),
            };
            assert_eq!(work_refusal(&error).limit(), 500_000_000);
            let measured = transaction()?
                .with_original_source_mixed_publication_test_limits_v28(
                    500_000_000,
                    20_000_000,
                    observe as Consumer,
                )
                .map_err(|e| format!("mixed publication measurement: {e:?}"))?
                .into_observation();
            let mut shared_work = Work::new(500_000_000);
            let mut shared = Budget::new(&mut shared_work, 20_000_000);
            let shared_ledger = shared.work_ledger_identity_v1();
            let on_account = transaction()?
                .with_original_source_mixed_publication_on_account_v28(
                    &mut shared,
                    |candidate, budget| {
                        assert!(budget.work_ledger_identity_v1() == shared_ledger);
                        observe(candidate, budget)
                    },
                )
                .map_err(|e| format!("original publication account: {e:?}"))?
                .into_observation();
            assert_eq!(on_account.statement, measured.statement);
            assert!(shared.work_ledger_identity_v1() == shared_ledger);
            CALLBACKS.with(|n| n.set(0));
            let exact = transaction()?
                .with_original_source_mixed_publication_test_limits_v28(
                    500_000_000,
                    measured.exact_storage,
                    observe as Consumer,
                )
                .map_err(|e| format!("mixed publication exact storage: {e:?}"))?
                .into_observation();
            assert_eq!(exact.statement, measured.statement);
            assert_eq!(exact.exact_storage, measured.exact_storage);
            CALLBACKS.with(|n| {
                assert_eq!(n.get(), 1);
                n.set(0);
            });
            let short = transaction()?.with_original_source_mixed_publication_test_limits_v28(
                500_000_000,
                measured.exact_storage - 1,
                observe as Consumer,
            );
            let error = match short {
                Err(error) => error,
                Ok(_) => panic!("one-short publication storage admitted"),
            };
            let limit = storage_refusal(&error);
            assert_eq!(limit.actual(), measured.exact_storage);
            assert_eq!(limit.limit(), measured.exact_storage - 1);
            CALLBACKS.with(|n| assert_eq!(n.get(), 0));
            let mut foreign_callbacks = 0;
            let foreign = transaction()?.with_original_source_mixed_publication_v28::<(), _>(
                |candidate, original| {
                    foreign_callbacks += 1;
                    let mut work = Work::new(100_000);
                    let mut foreign = Budget::new(&mut work, 20_000_000);
                    foreign.reserve_storage(original.storage())?;
                    assert!(
                        foreign.work_ledger_identity_v1() != original.work_ledger_identity_v1()
                    );
                    candidate.worker(&mut foreign)?;
                    Ok(())
                },
            );
            assert_eq!(foreign_callbacks, 1);
            assert!(matches!(
                foreign,
                Err(Error::Source(ProductionSourceOwnedViewErrorV18::Resource(
                    Resource::Accounting
                )))
            ));
            let selected =
                transaction()?.with_original_source_mixed_publication_v28::<(), _>(|_, _| {
                    Err(Error::Unsupported("selected publication consumer"))
                });
            assert!(matches!(
                selected,
                Err(Error::Unsupported("selected publication consumer"))
            ));
            let panic_transaction = transaction()?;
            let unwind = catch_unwind(AssertUnwindSafe(|| {
                panic_transaction.with_original_source_mixed_publication_v28::<(), _>(|_, _| {
                    std::panic::panic_any(829u32)
                })
            }));
            let payload = match unwind {
                Err(payload) => payload,
                Ok(_) => panic!("raw publication consumer unwind was swallowed"),
            };
            assert_eq!(*payload.downcast::<u32>().unwrap(), 829);
            Ok(exact)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "private actual-rustc child; invoked only by the parent fixture"]
fn mixed_publication_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = PublicationCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual publication callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("publication result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "actual mixed publication candidate: {result:?}"
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src/rustc-dev and authentic AMD dependencies"]
fn actual_original_mixed_publication_retains_nominal_chain_and_refuses_extraction_custody() {
    run_actual_sources::<PublicationObservation>(
        &[("straight", STRAIGHT), ("loop", LOOP)],
        &[(0, 0)],
        CHILD,
        "ORIGINAL_SOURCE_MIXED_PUBLICATION_V28",
        program,
        |_, _, _, report, _| {
            assert_eq!(report.roots, 2);
            assert!(report.generated_bytes > 0 && report.exact_storage > 0);
            assert_ne!(report.original, [0; 32]);
            assert_ne!(report.output, [0; 32]);
            assert_ne!(report.statement, [0; 32]);
            assert!(report.extraction_refused);
        },
    );
}
