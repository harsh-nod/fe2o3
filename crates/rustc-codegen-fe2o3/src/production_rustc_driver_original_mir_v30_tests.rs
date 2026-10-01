//! Genuine rustc capture through the existing Worker FinalConsumer. No runtime
//! is fabricated or invoked, and generated source is not an executed receipt.
use super::*;

#[path = "production_rustc_driver_reference_enum_v45_tests.rs"]
mod reference_enum_tests;

#[path = "production_rustc_driver_enum_frontend_v49_tests.rs"]
mod enum_frontend_tests;

const ORIGINAL_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_licm_tests::worker_orchestration_tests::publication_tests::original_mir_v30_tests::original_mir_worker_child";
const SCALAR: &str = "let temporary = a ^ b; let _result = temporary | a;";
const CONTROL: &str = "let _result = if a == b { a ^ b } else { a | b };";
const WITNESS: &str = r#"
let index = fe2o3_device::thread::index_1d();
let borrowed = &index;
let copied_reference = borrowed;
let ordinary = copied_reference.get();
let moved_index = index;
let disjoint = moved_index.into_disjoint();
let moved_disjoint = disjoint;
let borrowed_disjoint = &moved_disjoint;
let copied_disjoint_reference = borrowed_disjoint;
let owned = copied_disjoint_reference.get();
let _result = ordinary ^ owned;
"#;

fn original_program(body: &str) -> String {
    let kernel = |name| {
        format!(
            r#"
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn {name}(a: u32, b: u32) {{ {body} }}
"#
        )
    };
    format!(
        "use fe2o3_device::kernel;\n{}{}",
        kernel("z_original"),
        kernel("a_original")
    )
}

#[derive(Debug, Serialize, Deserialize)]
struct OriginalObservation {
    census: [usize; 6],
    statement: [u8; 32],
    work: usize,
    peak: usize,
    witness_protocol: [bool; 6],
    reference_enums: reference_enum_tests::ReferenceEnumCensus,
}

fn original_witness_event_protocol(source: &str, roots: usize) -> [bool; 3] {
    let mut found = vec![[false; 3]; roots];
    for definition in source
        .split("open spec fn invocation_source_byte_event_")
        .skip(1)
    {
        let Some((name, body)) = definition
            .split_once("(block: int, statement: int) -> Option<InvocationSourceByteEventV36> {\n")
        else {
            continue;
        };
        let Some((root, instance)) = name
            .strip_suffix("_v36")
            .and_then(|name| name.split_once('_'))
        else {
            continue;
        };
        let (Ok(root), Ok(_)) = (root.parse::<usize>(), instance.parse::<usize>()) else {
            continue;
        };
        let Some(events) = found.get_mut(root) else {
            continue;
        };
        let Some((body, _)) = body.split_once("\n}\n") else {
            continue;
        };
        events[0] |= body.contains("Some(InvocationSourceByteEventV36::WitnessBorrow {");
        for transfer in body
            .split("Some(InvocationSourceByteEventV36::WitnessTransfer {")
            .skip(1)
        {
            let Some((fields, _)) = transfer.split_once('}') else {
                continue;
            };
            events[1] |= fields.contains("reference: false, moved: true");
            events[2] |= fields.contains("reference: true, moved: false");
        }
    }
    std::array::from_fn(|event| roots > 0 && found.iter().all(|root| root[event]))
}

#[test]
fn original_witness_event_oracle_requires_actual_rows_in_every_root() {
    let helper = "open spec fn generic_dispatch(event: Option<InvocationSourceByteEventV36>) {\n match event { Some(InvocationSourceByteEventV36::WitnessBorrow { destination, .. }) => destination, Some(InvocationSourceByteEventV36::WitnessTransfer { reference: false, moved: true, .. }) => 0, _ => 1 }\n}\n";
    assert_eq!(original_witness_event_protocol(helper, 2), [false; 3]);
    let root = |root| {
        format!(
            "open spec fn invocation_source_byte_event_{root}_0_v36(block: int, statement: int) -> Option<InvocationSourceByteEventV36> {{\n if block == 0 && statement == 0 {{ Some(InvocationSourceByteEventV36::WitnessBorrow {{ destination: 1 }}) }} else if block == 0 && statement == 1 {{ Some(InvocationSourceByteEventV36::WitnessTransfer {{ destination: 2, reference: false, moved: true }}) }} else if block == 0 && statement == 2 {{ Some(InvocationSourceByteEventV36::WitnessTransfer {{ destination: 3, reference: true, moved: false }}) }} else {{ None }}\n}}\n"
        )
    };
    let first = format!("{helper}{}", root(0));
    assert_eq!(original_witness_event_protocol(&first, 2), [false; 3]);
    let both = format!("{first}{}", root(1));
    assert_eq!(original_witness_event_protocol(&both, 2), [true; 3]);
    let no_copy = both.replace(
        "reference: true, moved: false",
        "reference: true, moved: true",
    );
    assert_eq!(
        original_witness_event_protocol(&no_copy, 2),
        [true, true, false]
    );
    assert_eq!(original_witness_event_protocol(&both, 0), [false; 3]);
}

fn original_observe(
    candidate: PreparedMixedPublicationV28<'_, '_, '_>,
    budget: &mut Budget<'_>,
) -> Result<OriginalObservation, Error> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    candidate.replay(budget)?;
    let request = candidate.original_mir_request(budget)?;
    request
        .check_original_source(candidate.source(budget)?, budget)
        .map_err(Error::OriginalMir)?;
    let subject = request.subject(budget).map_err(Error::OriginalMir)?;
    let source = std::str::from_utf8(
        request
            .generated_source(budget)
            .map_err(Error::OriginalMir)?,
    )
    .unwrap();
    // Inspect generated obligations; this is not an executed proof of readiness.
    for root in 0..2 {
        assert!(source.contains(&format!("invocation_paired_step_{root}_v36")));
        assert!(source.contains(&format!("invocation_paired_initial_trace_{root}_v36")));
        assert!(source.contains(&format!(
            "open spec fn invocation_paired_native_inputs_{root}_v38"
        )));
        let readiness = source
            .split(&format!(
                "proof fn invocation_paired_source_ready_{root}_v38"
            ))
            .nth(1)
            .unwrap()
            .split("open spec fn")
            .next()
            .unwrap();
        let required = readiness.split(" requires ").nth(1).unwrap();
        let (premise, consequence) = required.split_once(" ensures ").unwrap();
        assert_eq!(
            premise.trim(),
            format!("invocation_paired_native_inputs_{root}_v38(arguments, external, execution),")
        );
        assert!(consequence.contains(&format!(
            "invocation_source_initial_runtime_{root}_v36(arguments, external, execution).machine.valid"
        )));
    }
    assert!(source.contains("invocation_source_logical_write_v38"));
    assert!(!source.contains("assume("));
    assert_eq!(&subject.census()[..2], &[2, 2]);
    assert!(subject.census()[3] >= 4);
    let optimized = candidate.refinement_subject(budget)?;
    assert_eq!(
        subject.canonical_identity(),
        optimized.graph_identities()[0]
    );
    let composed = std::str::from_utf8(candidate.generated_source(budget)?).unwrap();
    assert!(composed.contains("proof fn typed_final_native_source_trace_"));
    assert!(composed.contains("mod forwarding_v46 {"));
    assert_eq!(
        optimized.graph_identities()[3],
        *candidate.native(budget)?.output(budget)?.identity()
    );
    assert_eq!(
        subject.semantic_identity(),
        optimized.source_semantic_identity()
    );
    assert_eq!(subject.ssa_identity(), optimized.source_ssa_identity());
    assert_eq!(candidate.worker(budget)?.root_count(budget)?, 2);
    assert!(!candidate.worker(budget)?.llvm_ir(budget)?.is_empty());
    assert!(!request.authenticates_executed_proof());
    assert!(!request.grants_artifact_or_launch_authority());
    assert!(
        candidate
            .open_gates()
            .contains(&MixedPublicationOpenGateV28::OriginalMirToKirRefinement)
    );
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    let witness_events = original_witness_event_protocol(source, subject.census()[0]);
    let reference_enums =
        reference_enum_tests::observe_reference_enums(candidate.source(budget)?, source, budget)?;
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    Ok(OriginalObservation {
        census: subject.census(),
        statement: subject.statement_identity(),
        work: budget.work(),
        peak: budget.peak_storage(),
        witness_protocol: [
            source.contains("invocation_source_issue_witness_v38(cursor.source"),
            witness_events[0],
            source.contains("invocation_source_read_witness_v38(cursor.source"),
            source.contains("invocation_source_convert_witness_v38(cursor.source"),
            witness_events[1],
            witness_events[2],
        ],
        reference_enums,
    })
}

#[derive(Default)]
struct OriginalCallbacks {
    result: Option<Result<OriginalObservation, String>>,
}

// The on-account API retains root-phase charges until its enclosing transaction
// ends. Call this only after its continuation and all original bindings are gone.
fn finish_original_root_phase(budget: &mut Budget<'_>) {
    let retained = budget.storage().checked_sub(41).unwrap();
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 41);
}

impl Callbacks for OriginalCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            let run = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(41).unwrap();
                let mut entered = 0;
                let result = transaction()?.with_original_source_mixed_publication_on_account_v28(
                    &mut budget,
                    |candidate, budget| {
                        entered += 1;
                        original_observe(candidate, budget)
                    },
                );
                let result = result.map(|result| result.into_observation());
                finish_original_root_phase(&mut budget);
                Ok::<_, String>((result, entered, budget.work(), budget.peak_storage()))
            };
            let (measured, entered, work, peak) = run(500_000_000, 64_000_000)?;
            let mut measured = match measured {
                Ok(value) => {
                    assert_eq!(entered, 1);
                    value
                }
                Err(error) => return Err(format!("original MIR Worker request: {error:?}")),
            };
            let (exact, entered, exact_work, exact_peak) = run(work, peak)?;
            let exact =
                exact.map_err(|error| format!("exact original MIR Worker request: {error:?}"))?;
            assert_eq!(entered, 1);
            assert_eq!(exact.statement, measured.statement);
            assert_eq!(exact.reference_enums, measured.reference_enums);
            assert_eq!((exact_work, exact_peak), (work, peak));
            for short_work in [true, false] {
                let (short, _, _, _) = run(
                    work - usize::from(short_work),
                    peak - usize::from(!short_work),
                )?;
                let error = match short {
                    Err(error) => error,
                    Ok(_) => panic!("one-short original MIR Worker account admitted"),
                };
                if short_work {
                    let limit = work_refusal(&error);
                    assert_eq!((limit.actual(), limit.limit()), (work, work - 1));
                } else {
                    let limit = storage_refusal(&error);
                    assert_eq!((limit.actual(), limit.limit()), (peak, peak - 1));
                }
            }
            let mut foreign_calls = 0;
            let foreign = transaction()?.with_original_source_mixed_publication_v28::<(), _>(
                |candidate, original| {
                    foreign_calls += 1;
                    let request = candidate.original_mir_request(original)?;
                    let mut work = Work::new(500_000_000);
                    let mut foreign = Budget::new(&mut work, 64_000_000);
                    foreign.reserve_storage(original.storage())?;
                    let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
                    let error = request.generated_source(&foreign).unwrap_err();
                    assert_eq!(
                        (foreign.work(), foreign.storage(), foreign.peak_storage()),
                        before
                    );
                    assert!(matches!(
                        error,
                        fe2o3_verifier::MixedOptimizerRefinementErrorV26::Source(
                            ProductionSourceOwnedViewErrorV18::Resource(Resource::Accounting)
                        )
                    ));
                    let before = (original.work(), original.storage(), original.peak_storage());
                    assert!(matches!(
                        request.generated_source(original),
                        Err(fe2o3_verifier::MixedOptimizerRefinementErrorV26::Source(
                            ProductionSourceOwnedViewErrorV18::Resource(Resource::Accounting)
                        ))
                    ));
                    assert_eq!(
                        (original.work(), original.storage(), original.peak_storage()),
                        before
                    );
                    Err(Error::OriginalMir(error))
                },
            );
            assert_eq!(foreign_calls, 1);
            // The outer source scope retains its first custody failure before
            // the callback's later OriginalMir error wrapper is considered.
            assert!(
                matches!(
                    foreign,
                    Err(Error::Source(ProductionSourceOwnedViewErrorV18::Resource(
                        Resource::Accounting
                    )))
                ),
                "outer original MIR custody refusal: {:?}",
                foreign.as_ref().err()
            );
            let mut work = Work::new(500_000_000);
            let mut budget = Budget::new(&mut work, 64_000_000);
            budget.reserve_storage(41).unwrap();
            let selected = transaction()?
                .with_original_source_mixed_publication_on_account_v28::<(), _>(
                    &mut budget,
                    |_, _| Err(Error::Unsupported("original MIR consumer refusal")),
                );
            assert!(matches!(
                selected,
                Err(Error::Unsupported("original MIR consumer refusal"))
            ));
            drop(selected);
            finish_original_root_phase(&mut budget);
            let pending = transaction()?;
            let unwind = catch_unwind(AssertUnwindSafe(|| {
                pending.with_original_source_mixed_publication_on_account_v28::<(), _>(
                    &mut budget,
                    |_, _| std::panic::panic_any(930u32),
                )
            }));
            let payload = match unwind {
                Err(payload) => payload,
                Ok(_) => panic!("original MIR consumer unwind was swallowed"),
            };
            assert_eq!(*payload.downcast::<u32>().unwrap(), 930);
            finish_original_root_phase(&mut budget);
            measured.work = exact_work;
            measured.peak = peak;
            Ok(measured)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "private actual-rustc child; invoked only by the parent fixture"]
fn original_mir_worker_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = OriginalCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual original MIR callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("original MIR result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "actual original MIR Worker candidate: {result:?}"
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src/rustc-dev and authentic AMD dependencies"]
fn actual_original_mir_cfg_request_reaches_worker_with_scalar_and_control() {
    run_actual_sources::<OriginalObservation>(
        &[
            ("scalar", SCALAR),
            ("control", CONTROL),
            ("witness_borrow_read", WITNESS),
        ],
        &[(0, 0)],
        ORIGINAL_CHILD,
        "ORIGINAL_MIR_WORKER_V36",
        original_program,
        |_, _, case, report, _| {
            assert!(report.work > 0 && report.peak > 0);
            assert_eq!(&report.census[..2], &[2, 2]);
            assert_ne!(report.statement, [0; 32]);
            assert_eq!(report.witness_protocol, [case == "witness_borrow_read"; 6]);
        },
    );
}
