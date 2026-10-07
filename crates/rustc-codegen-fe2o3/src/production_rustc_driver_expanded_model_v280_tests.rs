//! Genuine private V259 -> V280 progression, separate from V259 input positives.
use super::*;
use fe2o3_verifier::{ExpandedSupportCensusV280, MixedOptimizerRefinementErrorV26};
use sha2::{Digest, Sha256};

#[path = "production_rustc_driver_expanded_aggregate_diagnostic_v280_tests.rs"]
mod aggregate_diagnostic;

#[path = "production_rustc_driver_expanded_model_export_v282_tests.rs"]
mod model_export;

const MODEL_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::expanded_source_tests::expanded_model_tests::expanded_model_child";
const REFUSAL_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::expanded_source_tests::expanded_model_tests::expanded_model_refusal_child";
const ACCOUNT_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::expanded_source_tests::expanded_model_tests::expanded_model_account_child";
const ACCOUNT_CASE: &str = "FE2O3_TEST_EXPANDED_MODEL_ACCOUNT_V280";

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum AccountCase {
    Payload,
    Overreport,
    Underreport,
    Refuse,
    Panic,
    Refund,
    Foreign,
}

const ACCOUNT_CASES: [AccountCase; 7] = [
    AccountCase::Payload,
    AccountCase::Overreport,
    AccountCase::Underreport,
    AccountCase::Refuse,
    AccountCase::Panic,
    AccountCase::Refund,
    AccountCase::Foreign,
];

enum CallbackPayload {
    Inline,
    Heap(Box<[u8; 17]>),
}

fn is_accounting_refusal(error: &(dyn std::error::Error + 'static)) -> bool {
    matches!(
        error.downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1>(),
        Some(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting)
    ) || error.source().is_some_and(is_accounting_refusal)
}

fn is_selected_refusal(error: &(dyn std::error::Error + 'static)) -> bool {
    matches!(
        error.downcast_ref::<SourceError>(),
        Some(SourceError::Unsupported(
            "selected expanded model consumer error"
        ))
    ) || error.source().is_some_and(is_selected_refusal)
}

struct AccountCallbacks {
    case: AccountCase,
    result: Option<Result<(), String>>,
}

impl Callbacks for AccountCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut work = Work::new(500_000_000);
            let mut budget = Budget::new(&mut work, 20_000_000);
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let slot = std::ptr::from_ref(&budget) as usize;
            // Production retains root-phase charges until the owning transaction
            // ends. The callback's dynamic payload is settled inside that scope.
            let run = |budget: &mut Budget<'_>| {
                let mut called = 0;
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    transaction.with_original_source_expanded_model_v280(
                        budget,
                        |source, original, tile, _, _, pair, model, budget| {
                            called += 1;
                            pair.check(source, original, tile, budget)?;
                            assert!(
                                !model
                                    .generated_source(budget)
                                    .map_err(SourceError::ExpandedModel)?
                                    .is_empty()
                            );
                            match self.case {
                                AccountCase::Payload => {
                                    budget.reserve_storage(17)?;
                                    Ok((CallbackPayload::Heap(Box::new([9; 17])), 17))
                                }
                                AccountCase::Overreport => Ok((CallbackPayload::Inline, 1)),
                                AccountCase::Underreport => {
                                    budget.reserve_storage(1)?;
                                    Ok((CallbackPayload::Inline, 0))
                                }
                                AccountCase::Refuse => Err(SourceError::Unsupported(
                                    "selected expanded model consumer error",
                                )),
                                AccountCase::Panic => std::panic::panic_any(280usize),
                                AccountCase::Refund => {
                                    budget.release_storage(1)?;
                                    Ok((CallbackPayload::Inline, 0))
                                }
                                AccountCase::Foreign => {
                                    let mut work = Work::new(500_000_000);
                                    let mut foreign = Budget::new(&mut work, 20_000_000);
                                    foreign.reserve_storage(budget.storage())?;
                                    let before = budget.work();
                                    let error = model
                                        .census(&mut foreign)
                                        .err()
                                        .expect("foreign model account must refuse");
                                    assert_eq!(budget.work(), before);
                                    Err(SourceError::ExpandedModel(error))
                                }
                            }
                        },
                    )
                }));
                assert_eq!(
                    called, 1,
                    "the actual generated model must precede every callback case"
                );
                assert_eq!(slot, std::ptr::from_ref(budget) as usize);
                assert!(ledger == budget.work_ledger_identity_v1());
                match self.case {
                    AccountCase::Panic => {
                        let payload = outcome
                            .err()
                            .expect("original model callback panic must propagate");
                        assert_eq!(*payload.downcast::<usize>().unwrap(), 280);
                    }
                    AccountCase::Payload => {
                        let result = outcome.unwrap()?;
                        let CallbackPayload::Heap(payload) = result.into_observation() else {
                            panic!("owned payload")
                        };
                        assert_eq!(*payload, [9; 17]);
                        let retained = budget.storage();
                        let root_phase = retained.checked_sub(17).expect("paid payload backing");
                        drop(payload);
                        budget.release_storage(17).unwrap();
                        assert_eq!(budget.storage(), root_phase);
                    }
                    AccountCase::Refuse => {
                        let error = outcome.unwrap().err().expect("selected consumer refusal");
                        assert!(
                            is_selected_refusal(&error),
                            "original error was replaced: {error:?}"
                        );
                    }
                    _ => {
                        let error = outcome.unwrap().err().expect("accounting refusal");
                        assert!(is_accounting_refusal(&error), "wrong refusal: {error:?}");
                    }
                }
                Ok::<_, SourceError>(())
            };
            let headers = [
                std::mem::size_of_val(&run)
                    .checked_mul(2)
                    .ok_or_else(|| "model account frame arithmetic".to_owned())?,
                std::mem::align_of_val(&run),
                2 * std::mem::size_of::<Result<(), SourceError>>(),
                8 * std::mem::size_of::<usize>(),
            ]
            .into_iter()
            .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
            .ok_or_else(|| "model account frame arithmetic".to_owned())?;
            budget
                .with_prepaid_scope(floor, 1, 1, headers, run)
                .map_err(|error| format!("model account transaction: {error:?}"))?;
            assert_eq!(slot, std::ptr::from_ref(&budget) as usize);
            assert!(ledger == budget.work_ledger_identity_v1());
            assert_eq!(
                budget.storage(),
                floor,
                "complete owned transaction must return the original floor"
            );
            Ok(())
        })());
        Compilation::Stop
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct ModelObservation {
    graphs: [[u8; 32]; 3],
    runtime_and_instances: [u8; 32],
    references: [u8; 32],
    model: [u8; 32],
    census: [u8; 32],
    counts: [usize; 6],
    helper_instances: [usize; 2],
    model_consumer_called: bool,
}

fn number(hash: &mut Sha256, value: usize, budget: &mut Budget<'_>) -> Result<(), SourceError> {
    budget.charge_work(8)?;
    hash.update(
        u64::try_from(value)
            .map_err(|_| fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?
            .to_le_bytes(),
    );
    Ok(())
}

fn census_identity(
    rows: &ExpandedSupportCensusV280<'_>,
    budget: &mut Budget<'_>,
) -> Result<[u8; 32], SourceError> {
    let mut hash = Sha256::new();
    for length in [
        rows.roots.len(),
        rows.instances.len(),
        rows.calls.len(),
        rows.cuts.len(),
        rows.candidates.len(),
        rows.zero_edges.len(),
    ] {
        number(&mut hash, length, budget)?;
    }
    for root in rows.roots {
        for value in [
            root.source_function as usize,
            root.original_function,
            root.target_function.0 as usize,
            root.instances.start,
            root.instances.end,
            root.cuts.start,
            root.cuts.end,
        ] {
            number(&mut hash, value, budget)?;
        }
    }
    for row in rows.instances {
        for value in [
            row.root,
            row.instance,
            row.source_function as usize,
            row.active as usize,
            row.incoming.is_some() as usize,
            row.incoming.map_or(0, |x| x.0),
            row.incoming.map_or(0, |x| x.1 as usize),
            row.locals.start,
            row.locals.end,
            row.cuts.start,
            row.cuts.end,
            row.calls.start,
            row.calls.end,
        ] {
            number(&mut hash, value, budget)?;
        }
    }
    for row in rows.calls {
        for value in [
            row.root,
            row.caller,
            row.block as usize,
            row.callable as usize,
            row.kind as usize,
            row.reachable as usize,
            row.child.is_some() as usize,
            row.child.unwrap_or(0),
        ] {
            number(&mut hash, value, budget)?;
        }
    }
    for row in rows.cuts {
        for value in [
            row.root,
            row.instance,
            row.block as usize,
            row.candidates.start,
            row.candidates.end,
            row.zero_edges.start,
            row.zero_edges.end,
            row.zero_rank,
        ] {
            number(&mut hash, value, budget)?;
        }
    }
    for row in rows.candidates {
        for value in [
            row.block,
            row.operation.is_some() as usize,
            row.operation.unwrap_or(0),
            row.prefix,
        ] {
            number(&mut hash, value, budget)?;
        }
    }
    for &(from, to) in rows.zero_edges {
        number(&mut hash, from, budget)?;
        number(&mut hash, to, budget)?;
    }
    Ok(hash.finalize().into())
}

fn is_domain_refusal(error: &(dyn std::error::Error + 'static)) -> bool {
    if matches!(
        error.downcast_ref::<MixedOptimizerRefinementErrorV26>(),
        Some(MixedOptimizerRefinementErrorV26::Statement(
            "expanded generation differs from its retained source or runtime"
        ))
    ) {
        return true;
    }
    error.source().is_some_and(is_domain_refusal)
}

struct ModelCallbacks {
    refusal: bool,
    result: Option<Result<Option<ModelObservation>, String>>,
}

impl Callbacks for ModelCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut work = Work::new(500_000_000);
            let mut budget = Budget::new(&mut work, 20_000_000);
            let mut called = 0;
            let result = transaction.with_original_source_expanded_model_v280(
                &mut budget,
                |source, original, tile, roots, _, pair, model, budget| {
                    called += 1;
                    let scratch =
                        2 * std::mem::size_of::<Sha256>() + 24 * std::mem::size_of::<usize>();
                    budget.reserve_storage(scratch)?;
                    pair.check(source, original, tile, budget)?;
                    assert_eq!(pair.reference_count(budget)?, 0);
                    let subject = pair.subject(budget)?;
                    assert_eq!(subject.references, empty_reference_input_identity());
                    let bytes = model
                        .generated_source(budget)
                        .map_err(SourceError::ExpandedModel)?;
                    budget.charge_work(bytes.len())?;
                    let model_identity = Sha256::digest(bytes).into();
                    assert!(!bytes.is_empty());
                    assert!(
                        bytes.len() <= fe2o3_verifier::MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3
                    );
                    let rows = model.census(budget).map_err(SourceError::ExpandedModel)?;
                    assert_eq!(rows.roots.len(), roots.len());
                    assert_eq!(rows.roots.len(), source.root_count(budget)?);
                    let (mut instance_end, mut cut_end) = (0, 0);
                    for (root, row) in rows.roots.iter().enumerate() {
                        budget.charge_work(1)?;
                        let (function, original_function) = source.root(root, budget)?;
                        assert_eq!(
                            (row.source_function, row.original_function),
                            (function.index(), original_function)
                        );
                        assert_eq!(
                            row.target_function,
                            pair.roots(budget)?[root].target_function
                        );
                        assert_eq!(row.instances.start, instance_end);
                        assert_eq!(row.cuts.start, cut_end);
                        assert_eq!(row.instances.len(), source.instance_count(root, budget)?);
                        for (instance, entry) in
                            rows.instances[row.instances.clone()].iter().enumerate()
                        {
                            budget.charge_work(1)?;
                            let (function, incoming) = source.instance(root, instance, budget)?;
                            assert_eq!(
                                (entry.root, entry.instance, entry.source_function),
                                (root, instance, function.index())
                            );
                            assert_eq!(
                                entry.active,
                                source.instance_active(root, instance, budget)?
                            );
                            assert_eq!(
                                entry.incoming,
                                incoming.map(|(caller, block)| (caller, block.index()))
                            );
                            let blocks = source.source_semantic(budget)?.functions()
                                [function.index() as usize]
                                .blocks();
                            assert_eq!(entry.cuts.len(), blocks.len());
                            for (block, pc) in entry.cuts.clone().enumerate() {
                                budget.charge_work(1)?;
                                let cut = &rows.cuts[pc];
                                assert_eq!(
                                    (cut.root, cut.instance, cut.block),
                                    (root, instance, block as u32)
                                );
                                if !entry.active {
                                    assert!(cut.candidates.is_empty());
                                }
                            }
                            if let Some((parent, block)) = incoming {
                                assert!(parent < instance);
                                assert_eq!(
                                    original.defined_call_instance(root, parent, block, budget)?,
                                    instance
                                );
                            } else {
                                assert_eq!(instance, 0);
                            }
                        }
                        instance_end = row.instances.end;
                        cut_end = row.cuts.end;
                    }
                    assert_eq!(
                        (instance_end, cut_end),
                        (rows.instances.len(), rows.cuts.len())
                    );
                    let (helper_instances, mapped) =
                        expanded_roots_tests::original_helper_instances(source, original, budget)?;
                    assert!(mapped.into_iter().all(|count| count > 0));
                    let observation = ModelObservation {
                        graphs: subject.graphs.map(|identity| *identity.digest()),
                        runtime_and_instances: subject.runtime_and_instances,
                        references: subject.references,
                        model: model_identity,
                        census: census_identity(&rows, budget)?,
                        counts: [
                            rows.roots.len(),
                            rows.instances.len(),
                            rows.calls.len(),
                            rows.cuts.len(),
                            rows.candidates.len(),
                            rows.zero_edges.len(),
                        ],
                        helper_instances,
                        model_consumer_called: true,
                    };
                    model_export::observe(bytes, &observation, budget)?;
                    budget.release_storage(scratch)?;
                    Ok((observation, 0))
                },
            );
            if self.refusal {
                let error = result
                    .err()
                    .ok_or_else(|| "out-of-domain model was accepted".to_owned())?;
                assert_eq!(called, 0, "domain refusal must precede the model consumer");
                assert!(
                    is_domain_refusal(&error),
                    "exact model domain refusal: {error:?}"
                );
                Ok(None)
            } else {
                let observed = result
                    .map_err(|error| format!("actual expanded support: {error:?}"))?
                    .into_observation();
                assert_eq!(called, 1);
                Ok(Some(observed))
            }
        })());
        Compilation::Stop
    }
}

fn child(refusal: bool) {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = ModelCallbacks {
        refusal,
        result: None,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("actual expanded model callback");
    std::fs::write(
        env::var_os(RESULT).unwrap(),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "expanded support: {result:?}");
}

#[test]
#[ignore = "process helper; exact source request supplied by its parent"]
fn expanded_model_child() {
    child(false);
}

#[test]
#[ignore = "process helper; exact source request supplied by its parent"]
fn expanded_model_refusal_child() {
    child(true);
}

#[test]
#[ignore = "process helper; exact finite source and accounting case supplied by its parent"]
fn expanded_model_account_case_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = AccountCallbacks {
        case: serde_json::from_str(&env::var(ACCOUNT_CASE).unwrap()).unwrap(),
        result: None,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("actual model accounting callback");
    std::fs::write(
        env::var_os(RESULT).unwrap(),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "model accounting: {result:?}");
}

#[test]
#[ignore = "process helper; exact finite source request supplied by its parent"]
fn expanded_model_account_child() {
    if env::var_os(ARGS).is_none() {
        return;
    }
    let response = PathBuf::from(env::var_os(RESULT).unwrap());
    // Each case gets new one-shot MIR custody rather than recollecting a session.
    for (ordinal, case) in ACCOUNT_CASES.into_iter().enumerate() {
        let case_response = response.with_extension(format!("model-account-{ordinal}.json"));
        assert!(!case_response.exists());
        let child = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                &ACCOUNT_CHILD.replace(
                    "expanded_model_account_child",
                    "expanded_model_account_case_child",
                ),
                "--ignored",
                "--nocapture",
            ])
            .env(ACCOUNT_CASE, serde_json::to_string(&case).unwrap())
            .env(RESULT, &case_response)
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "model case {case:?}: {}\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
        let result: Result<(), String> =
            serde_json::from_slice(&std::fs::read(case_response).unwrap()).unwrap();
        result.unwrap();
    }
    let result: Result<usize, String> = Ok(ACCOUNT_CASES.len());
    std::fs::write(response, serde_json::to_vec(&result).unwrap()).unwrap();
}

#[test]
#[ignore = "requires pinned nightly rust-src and authentic ordinary AMD source compilation"]
fn actual_rustc_expanded_support_model_covers_complete_roots_and_refuses_domain_overflow() {
    run_actual_sources::<Option<ModelObservation>>(
        &[
            ("two", "two"),
            ("two", "two"),
            ("mixed", "mixed"),
            ("scalar", "scalar"),
            ("helper", "helper"),
        ],
        &[(0, 0), (3, 0)],
        MODEL_CHILD,
        "EXPANDED_SUPPORT_V280",
        |case| expanded_roots_tests::root_source_with_grid(case, Some(3)),
        |_, _, label, outcome, observations| {
            let observed = outcome.as_ref().expect("complete model must be generated");
            assert!(observed.model_consumer_called);
            assert_eq!(observed.counts[0], 2);
            assert!(observed.counts[1] >= 2 && observed.counts[3] > 0);
            assert_ne!(observed.model, [0; 32]);
            assert_ne!(observed.census, [0; 32]);
            if let Some(previous) = observations.get(label) {
                assert_eq!(&outcome, previous);
            } else {
                observations.insert(label.to_owned(), outcome);
            }
        },
    );
    run_actual_sources::<Option<ModelObservation>>(
        &[("default", "default"), ("one-over", "one-over")],
        &[(0, 0), (3, 0)],
        REFUSAL_CHILD,
        "EXPANDED_SUPPORT_DOMAIN_V280",
        |case| {
            expanded_roots_tests::root_source_with_grid(
                "two",
                match case {
                    "default" => None,
                    "one-over" => Some(67_108_864),
                    _ => unreachable!(),
                },
            )
        },
        |_, _, _, outcome, _| assert!(outcome.is_none()),
    );
    run_actual_sources::<usize>(
        &[("model-account", "two")],
        &[(0, 0)],
        ACCOUNT_CHILD,
        "EXPANDED_SUPPORT_ACCOUNT_V280",
        |case| expanded_roots_tests::root_source_with_grid(case, Some(3)),
        |_, _, _, completed, _| assert_eq!(completed, ACCOUNT_CASES.len()),
    );
}
