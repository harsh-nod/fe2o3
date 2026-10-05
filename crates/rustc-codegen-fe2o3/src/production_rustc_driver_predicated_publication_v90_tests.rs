//! Authentic rustc preparation only: no executed proof or publication authority.
use super::*;
use crate::production_pipeline::source_owned_v29::{
    mixed_worker_v28::publication::MixedPublicationErrorV28,
    predicated_worker_v90::publication::PreparedMixedPublicationV28 as Candidate,
};
use fe2o3_kernel_descriptor::{
    mixed_conditional_v26::{
        MixedDefinitionV26, MixedEdgeV26, MixedGuardPathV26, MixedIndexEnvelopeV26,
        MixedOperationV26,
    },
    mixed_conditional_v86::{MixedAccessGuardV86, decode_mixed_contract_v86},
};
use fe2o3_kernel_ir::{
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirOperationCoordinateV1 as Operation,
};
use fe2o3_lower_mir_kernel::{
    ProductionMixedRuntimeGuardV89 as Guard, ProductionSourceOwnedViewErrorV18,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1, SemanticOperandV1,
    SemanticTerminatorKindV1, SemanticWriteOnlyDisjointWriteKindV1,
};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_licm_tests::worker_orchestration_tests::predicated_publication_v90_tests::predicated_publication_child";
const RESOURCE_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_licm_tests::worker_orchestration_tests::predicated_publication_v90_tests::predicated_publication_resources_child";
const CASES: &[(&str, &str)] = &[
    ("fresh", "fresh"),
    ("named", "named"),
    ("disjoint", "disjoint"),
    ("mixed", "mixed"),
    ("cfg", "cfg"),
    ("loop", "loop"),
];
const WORK: usize = 500_000_000;
const STORAGE: usize = 20_000_000;
const FLOOR: usize = 37;

fn source_program(case: &str) -> String {
    if case == "cfg" || case == "loop" {
        return super::program(if case == "cfg" { STRAIGHT } else { LOOP });
    }
    let (arguments, body) = match case {
        "fresh" => ("", "let _ = output.write(thread::index_1d(), 7u32);"),
        "named" => (
            "",
            "let index = thread::index_1d(); let _ = output.write(index, 7u32);",
        ),
        "disjoint" => (
            "",
            "let index = thread::index_1d().into_disjoint(); let _ = output.write_disjoint(index, 7u32);",
        ),
        "mixed" => (
            "input: &[u32], ",
            "let index = thread::index_1d(); let i = index.get(); if i >= input.len() { return; } let value = input[i]; let _ = output.write(index, value);",
        ),
        _ => panic!("unregistered predicated source fixture"),
    };
    let mut text = "use fe2o3_device::{WriteOnlyDisjointSlice, kernel, thread};\n".to_owned();
    for name in ["predicated_first", "predicated_second"] {
        text.push_str(&format!(
            r#"
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn {name}({arguments}mut output: WriteOnlyDisjointSlice<u32>) {{ {body} }}
"#
        ));
    }
    text
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Observation {
    roots: usize,
    graphs: [[u8; 32]; 4],
    statement: [u8; 32],
    original_bytes: usize,
    typed_bytes: usize,
    cfg_reads: usize,
    cfg_writes: usize,
    predicated_writes: usize,
    // Actual imported call-operand kinds, not manufactured Copy authority.
    thread_operands: [usize; 3],
    extraction_refused: bool,
}

fn operation(row: Operation) -> MixedOperationV26 {
    MixedOperationV26 {
        function: row.block.function.0,
        block: row.block.block,
        operation: row.operation,
    }
}
fn definition(row: Definition) -> MixedDefinitionV26 {
    match row {
        Definition::FunctionArgument { function, argument } => {
            MixedDefinitionV26::FunctionArgument {
                function: function.0,
                argument,
            }
        }
        Definition::BlockArgument { block, argument } => MixedDefinitionV26::BlockArgument {
            function: block.function.0,
            block: block.block,
            argument,
        },
        Definition::Result {
            operation: op,
            result,
        } => MixedDefinitionV26::Result {
            operation: operation(op),
            result,
        },
    }
}
fn guard(row: Guard) -> MixedAccessGuardV86 {
    match row {
        Guard::CfgEdge { condition, edge } => MixedAccessGuardV86::CfgEdge {
            condition: definition(condition),
            edge: MixedEdgeV26 {
                function: edge.source.function.0,
                block: edge.source.block,
                successor: edge.successor,
            },
        },
        Guard::ExplicitPredicate {
            condition,
            bound_comparison,
        } => MixedAccessGuardV86::ExplicitPredicate {
            condition: definition(condition),
            bound_comparison: definition(bound_comparison),
        },
    }
}

type Consumer =
    for<'a, 'v, 's, 'w> fn(Candidate<'a, 'v, 's>, &mut Budget<'w>) -> Result<Observation, Error>;
fn observe(
    candidate: Candidate<'_, '_, '_>,
    budget: &mut Budget<'_>,
) -> Result<Observation, Error> {
    candidate.replay(budget)?;
    let subject = candidate.refinement_subject(budget)?;
    let source = candidate.source(budget)?;
    assert_eq!(
        *source.canonical(budget)?.identity(),
        subject.graph_identities()[0]
    );
    assert_eq!(
        *source.source_semantic(budget)?.semantic_sha256().as_bytes(),
        subject.source_semantic_identity()
    );
    assert_eq!(
        *source.source_ssa(budget)?.identity().as_bytes(),
        subject.source_ssa_identity()
    );
    let mut thread_operands = [0; 3];
    let semantic = source.source_semantic(budget)?;
    for function in semantic.functions() {
        for block in function.blocks() {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            let SemanticCallableDeclV1::CompilerIntrinsic {
                operation:
                    SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                        kind: SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint },
                        ..
                    },
                ..
            } = &semantic.callables()[call.callee().index() as usize]
            else {
                continue;
            };
            match (&call.arguments()[1], disjoint) {
                (SemanticOperandV1::Move(_), false) => thread_operands[0] += 1,
                (SemanticOperandV1::Copy(_), false) => thread_operands[1] += 1,
                (SemanticOperandV1::Move(_), true) => thread_operands[2] += 1,
                _ => panic!("unexpected authentic ThreadWrite witness operand"),
            }
        }
    }
    let native = candidate.native(budget)?;
    let relocation = native.relocation(budget)?;
    let prefix = relocation.prefix(budget)?.output(budget)?;
    assert_eq!(prefix.execution().policy_version(), 11);
    assert!((1..=32).contains(&prefix.execution().rounds()));
    assert_eq!(*prefix.owner().identity(), subject.graph_identities()[1]);
    assert_eq!(
        *relocation.tail(budget)?.output().identity(),
        subject.graph_identities()[2]
    );
    let consensus = native
        .store_consensus_v46(budget)?
        .expect("real StoreConsensus owner");
    assert_eq!(
        *consensus.output(budget)?.identity(),
        subject.graph_identities()[3]
    );
    assert_eq!(
        *native.output(budget)?.identity(),
        subject.graph_identities()[3]
    );
    let original = candidate
        .original_mir_request(budget)?
        .generated_source(budget)
        .map_err(Error::OriginalMir)?;
    let original = std::str::from_utf8(original).unwrap();
    for root in 0..2 {
        assert!(original.contains(&format!("proof fn invocation_paired_step_{root}_v36(")));
        assert!(original.contains(&format!(
            "proof fn invocation_paired_finite_trace_{root}_v36("
        )));
    }
    if thread_operands.iter().sum::<usize>() != 0 {
        assert!(
            original.contains(
                "InvocationSourceByteEventV36::ThreadWrite(InvocationSourceThreadWriteV88"
            )
        );
    }
    let typed = std::str::from_utf8(candidate.generated_source(budget)?).unwrap();
    for required in [
        "proof fn typed_final_native_source_trace_",
        "mod typed_prefix_v49 {",
        "mod forwarding_v46 {",
    ] {
        assert!(typed.contains(required));
    }
    let mut report = Observation {
        roots: 2,
        graphs: subject
            .graph_identities()
            .map(|identity| *identity.digest()),
        statement: subject.statement_identity(),
        original_bytes: original.len(),
        typed_bytes: typed.len(),
        cfg_reads: 0,
        cfg_writes: 0,
        predicated_writes: 0,
        thread_operands,
        extraction_refused: false,
    };
    // Only the complete actual-source parent retains these bounded model
    // bytes. Resource replays stay quiet and no executed proof is asserted.
    if std::env::args().any(|argument| argument == CHILD) {
        println!(
            "TYPED_SOURCE_MODEL_V93 {}",
            serde_json::json!({"statement": report.statement, "source": typed})
        );
    }
    let worker = candidate.worker(budget)?;
    assert_eq!(worker.root_count(budget)?, 2);
    assert_eq!(worker.descriptor(budget)?.kernel_count(), 2);
    assert!(!worker.llvm_ir(budget)?.is_empty());
    let rows = native.runtime_occurrences(budget)?;
    let mut seen = 0;
    for root in 0..2 {
        let bytes = worker.contract(root, budget)?;
        assert!(decode_mixed_contract_v26(bytes, &mut |n| budget.charge_work(n)).is_err());
        let contract = decode_mixed_contract_v86(bytes, &mut |n| budget.charge_work(n)).unwrap();
        let subjects = contract.subjects();
        assert_eq!(subjects.original_root, root as u32);
        assert_eq!(
            subjects.source_semantic_identity,
            subject.source_semantic_identity()
        );
        assert_eq!(subjects.original_graph_identity, report.graphs[0]);
        assert_eq!(subjects.output_graph_identity, report.graphs[3]);
        let mut retained = rows
            .iter()
            .filter(|row| row.output_operation().block.function.0 == subjects.output_function);
        for index in 0..contract.occurrence_count() {
            let row = contract
                .occurrence(index, &mut |n| budget.charge_work(n))
                .unwrap();
            let actual = retained
                .next()
                .expect("every wire row has its retained final occurrence");
            assert_eq!(row.original_instance as usize, actual.original_instance());
            assert_eq!(
                row.original_operation,
                operation(actual.original_operation())
            );
            assert_eq!(row.output_operation, operation(actual.output_operation()));
            assert_eq!(
                row.original_formation,
                operation(actual.original_address_formation())
            );
            assert_eq!(
                row.output_formation,
                operation(actual.output_address_formation())
            );
            assert_eq!(
                row.output_address_index,
                definition(actual.output_address_index())
            );
            assert_eq!(row.output_guard, guard(actual.output_guard()));
            assert!(actual.requires_address_formation_domain());
            assert_eq!(row.element_bytes, 4);
            assert_eq!(row.alignment, 4);
            assert!(!row.volatile);
            match row.output_guard {
                MixedAccessGuardV86::CfgEdge { .. } => {
                    assert!(matches!(row.path, MixedGuardPathV26::TrueEdge { .. }));
                    if row.writing {
                        report.cfg_writes += 1;
                    } else {
                        report.cfg_reads += 1;
                    }
                }
                MixedAccessGuardV86::ExplicitPredicate { .. } => {
                    assert!(row.writing);
                    assert_eq!(row.path, MixedGuardPathV26::ExplicitPredicate);
                    assert_eq!(
                        row.formation_envelope,
                        MixedIndexEnvelopeV26::InvocationAxis { axis: 0 }
                    );
                    assert_eq!(
                        row.access_envelope,
                        MixedIndexEnvelopeV26::LogicalExtent {
                            argument: row.argument
                        }
                    );
                    report.predicated_writes += 1;
                }
            }
            seen += 1;
        }
        assert!(
            retained.next().is_none(),
            "no retained occurrence may be omitted"
        );
        assert!(!contract.grants_artifact_or_launch_authority());
    }
    assert_eq!(seen, rows.len());
    let floor = budget.storage();
    let prepared = candidate.prepare_execution_for_test(budget, 1)?;
    assert!(!prepared.authenticates_executed_proof());
    assert_eq!(
        <[u8; 32]>::from(sha2::Sha256::digest(
            prepared
                .prefix_witness(budget)
                .map_err(Error::MixedRelocationExpressions)?
        )),
        subject.prefix_execution_identity()
    );
    prepared
        .discard(budget)
        .map_err(Error::MixedRelocationExpressions)?;
    assert_eq!(budget.storage(), floor);
    assert!(!candidate.grants_publication_or_artifact_authority());
    assert_eq!(candidate.open_gates().len(), 7);
    match candidate.into_protected(budget) {
        Err(Error::MixedPublication(MixedPublicationErrorV28::ExtractionOnly)) => (),
        Err(error) => return Err(error),
        Ok(_) => panic!("extraction-only candidate gained protected publication authority"),
    }
    report.extraction_refused = true;
    Ok(report)
}

fn resource(error: &Error) -> Resource {
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(cause) = current {
        if let Some(resource) = cause.downcast_ref::<Resource>() {
            return *resource;
        }
        if let Some(limit) = cause.downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1>() {
            return Resource::Work(*limit);
        }
        if let Some(error) =
            cause.downcast_ref::<fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1>()
        {
            match error {
                fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1::Resource(
                    fe2o3_kernel_ir::FormalGuardedMemoryResourceErrorV1::Work(limit),
                ) => return Resource::Work(*limit),
                fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1::Resource(
                    fe2o3_kernel_ir::FormalGuardedMemoryResourceErrorV1::Storage { actual, limit },
                ) => {
                    return Resource::Storage(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationStorageLimitV1::new(
                            *actual, *limit,
                        ),
                    );
                }
                _ => (),
            }
        }
        current = cause.source();
    }
    panic!("typed predicated resource refusal required: {error:?}");
}

#[derive(Default)]
struct PreparationCallbacks {
    boundaries: bool,
    result: Option<Result<Observation, String>>,
}
impl Callbacks for PreparationCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            // A fixed function-pointer capture keeps exact and short frame layouts identical.
            let run = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(FLOOR).unwrap();
                let ledger = budget.work_ledger_identity_v1();
                let pending = transaction()?;
                // Root-phase owners remain charged until their enclosing scope ends.
                let result = budget.with_prepaid_scope(FLOOR, 0, 0, 0, |budget| {
                    pending
                        .with_original_source_predicated_publication_on_account_v90(
                            budget,
                            observe as Consumer,
                        )
                        .map(|value| value.into_observation())
                });
                assert_eq!(budget.storage(), FLOOR);
                assert!(budget.work_ledger_identity_v1() == ledger);
                Ok::<_, String>((result, budget.work(), budget.peak_storage()))
            };
            let (measured, work, storage) = run(WORK, STORAGE)?;
            let measured =
                measured.map_err(|error| format!("actual predicated preparation: {error:?}"))?;
            if !self.boundaries {
                return Ok(measured);
            }
            let (exact, exact_work, exact_storage) = run(work, storage)?;
            assert_eq!(
                exact.map_err(|error| format!("exact predicated preparation: {error:?}"))?,
                measured
            );
            assert_eq!((exact_work, exact_storage), (work, storage));
            let error = run(work - 1, storage)?
                .0
                .expect_err("one-short work admitted");
            assert!(matches!(resource(&error), Resource::Work(limit) if limit.limit() == work - 1));
            let error = run(work, storage - 1)?
                .0
                .expect_err("one-short storage admitted");
            assert!(
                matches!(resource(&error), Resource::Storage(limit) if limit.limit() == storage - 1)
            );
            let mut callbacks = [0; 3];
            let foreign = transaction()?
                .with_original_source_predicated_publication_test_limits_v90(
                    WORK,
                    STORAGE,
                    |candidate, original| {
                        callbacks[0] += 1;
                        let mut work = Work::new(WORK);
                        let mut foreign = Budget::new(&mut work, STORAGE);
                        foreign.reserve_storage(original.storage())?;
                        let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
                        let error = candidate.check_lineage_account_v29(&foreign).unwrap_err();
                        assert!(matches!(
                            error,
                            Error::Source(ProductionSourceOwnedViewErrorV18::Resource(
                                Resource::Accounting
                            ))
                        ));
                        assert_eq!(
                            (foreign.work(), foreign.storage(), foreign.peak_storage()),
                            before
                        );
                        candidate.worker(&mut foreign)?;
                        Ok(())
                    },
                );
            assert!(matches!(
                foreign,
                Err(Error::Source(ProductionSourceOwnedViewErrorV18::Resource(
                    Resource::Accounting
                )))
            ));
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(FLOOR).unwrap();
            let selected_transaction = transaction()?;
            let selected = budget.with_prepaid_scope(FLOOR, 0, 0, 0, |budget| {
                selected_transaction
                    .with_original_source_predicated_publication_on_account_v90(
                        budget,
                        |_, _| -> Result<(), Error> {
                            callbacks[1] += 1;
                            Err(Error::Unsupported(
                                "selected predicated preparation consumer",
                            ))
                        },
                    )
                    .map(|value| value.into_observation())
            });
            assert!(matches!(
                selected,
                Err(Error::Unsupported(
                    "selected predicated preparation consumer"
                ))
            ));
            assert_eq!(budget.storage(), FLOOR);
            let panic_transaction = transaction()?;
            let unwind = catch_unwind(AssertUnwindSafe(|| {
                budget.with_prepaid_scope(FLOOR, 0, 0, 0, |budget| {
                    panic_transaction
                        .with_original_source_predicated_publication_on_account_v90(
                            budget,
                            |_, _| -> Result<(), Error> {
                                callbacks[2] += 1;
                                std::panic::panic_any(9090u32)
                            },
                        )
                        .map(|value| value.into_observation())
                })
            }));
            let payload = match unwind {
                Err(payload) => payload,
                Ok(_) => panic!("intentional predicated consumer panic did not escape"),
            };
            assert_eq!(*payload.downcast::<u32>().unwrap(), 9090);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(callbacks, [1; 3]);
            let mut early = 0;
            for (work, storage) in [(0, STORAGE), (WORK, 0)] {
                assert!(
                    transaction()?
                        .with_original_source_predicated_publication_test_limits_v90(
                            work,
                            storage,
                            |_, _| {
                                early += 1;
                                Ok(())
                            }
                        )
                        .is_err()
                );
            }
            assert_eq!(early, 0);
            Ok(measured)
        })());
        Compilation::Stop
    }
}

fn child(boundaries: bool) {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = PreparationCallbacks {
        boundaries,
        ..Default::default()
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual predicated publication callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("predicated publication result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "actual predicated publication preparation: {result:?}"
    );
}
#[test]
#[ignore = "private actual-rustc child; invoked only by the parent fixture"]
fn predicated_publication_child() {
    child(false);
}
#[test]
#[ignore = "private actual-rustc child; invoked only by the parent fixture"]
fn predicated_publication_resources_child() {
    child(true);
}

fn validate(case: &str, report: &Observation) {
    assert_eq!(report.roots, 2);
    assert!(report.original_bytes > 0 && report.typed_bytes > 0 && report.extraction_refused);
    assert!(report.graphs.iter().all(|identity| *identity != [0; 32]));
    assert_ne!(report.statement, [0; 32]);
    let expected = match case {
        "fresh" | "named" | "disjoint" => (0, 0, 2),
        "mixed" => (2, 0, 2),
        "cfg" | "loop" => (2, 2, 0),
        _ => panic!("unknown case"),
    };
    assert_eq!(
        (
            report.cfg_reads,
            report.cfg_writes,
            report.predicated_writes
        ),
        expected
    );
    if case == "disjoint" {
        assert_eq!(report.thread_operands, [0, 0, 2]);
    } else if expected.2 == 2 {
        assert_eq!(report.thread_operands[0] + report.thread_operands[1], 2);
        assert_eq!(report.thread_operands[2], 0);
    } else {
        assert_eq!(report.thread_operands, [0; 3]);
    }
}

#[test]
fn predicated_publication_fixture_roster_preserves_two_roots_and_linear_witnesses() {
    assert_eq!(CASES.len(), 6);
    for (_, case) in CASES {
        let source = source_program(case);
        assert_eq!(source.matches("#[kernel(").count(), 2);
        assert!(!source.contains("unsafe") && !source.contains(".clone()"));
    }
    assert!(source_program("disjoint").contains(".into_disjoint()"));
    assert!(!source_program("fresh").contains("let index ="));
    assert!(source_program("named").contains("let index ="));
}

#[test]
#[ignore = "requires pinned nightly rust-src/rustc-dev; actual two-target preparation, not proof execution"]
fn actual_original_predicated_publication_retains_final_graph_models_and_full_guards() {
    run_actual_sources::<Observation>(
        CASES,
        &[(0, 0)],
        CHILD,
        "ORIGINAL_SOURCE_PREDICATED_PUBLICATION_V90",
        source_program,
        |_, _, case, report, _| validate(case, &report),
    );
}
#[test]
#[ignore = "requires pinned nightly rust-src/rustc-dev; authentic preparation accounting and custody only"]
fn actual_original_predicated_publication_preserves_exact_budgets_and_custody() {
    run_actual_sources::<Observation>(
        &[("named", "named")],
        &[(0, 0)],
        RESOURCE_CHILD,
        "ORIGINAL_SOURCE_PREDICATED_PUBLICATION_RESOURCES_V90",
        source_program,
        |_, _, case, report, _| validate(case, &report),
    );
}
