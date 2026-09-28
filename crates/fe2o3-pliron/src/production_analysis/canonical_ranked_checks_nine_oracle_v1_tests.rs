//! Actual fixed-nine tests for the explicitly selected V851 + native profile.
//! V857 pins and private-layout/capacity review premises are in the DIAG handoff.
use super::*;
use crate::KernelCheckPassKindV1 as Pass;
use crate::production_analysis::{
    pliron_ir_identity::LivePlironStructuralIdentityProviderV1,
    pliron_pass_contract::{
        PlironPassPreservationErrorV1 as Preservation,
        begin_production_pliron_pass_contract_session_v1,
    },
    pliron_pipeline as pipeline,
    pliron_resource_envelope::ProductionAnalysisInputCensusV1 as Census,
};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, Constant, Function, Operation, OperationKind, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId,
};
use pliron::{
    basic_block::BasicBlock as LiveBlock,
    context::Ptr,
    operation::Operation as LiveOperation,
    r#type::TypeHandle,
    value::{Use, Value},
};
use std::collections::{HashMap, HashSet};

#[path = "canonical_ranked_checks_nine_oracle_v1_numbers.rs"]
mod numbers;
use numbers::{Oracle, PEAK_RESOURCE, PROFILE, Triple, WORK_RESOURCE};

const PASS_ORDER: [Pass; 9] = [
    Pass::TensorLayout,
    Pass::MemoryBounds,
    Pass::AtomicLegality,
    Pass::RaceFreedom,
    Pass::HierarchicalOwnership,
    Pass::BarrierConvergence,
    Pass::PipelineProtocol,
    Pass::WorkgroupMemory,
    Pass::SemanticRefinement,
];

fn fixture(count: usize) -> Module {
    assert!((1..=2).contains(&count));
    let mut module = Module::new("nine-native-independent");
    for name in ["zeta", "alpha"].into_iter().take(count) {
        let mut entry = BasicBlock::new(BlockId(17));
        entry.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(12), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(8)),
        ));
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(91),
            arguments: vec![],
        });
        let mut exit = BasicBlock::new(BlockId(91));
        exit.terminator = Some(Terminator::Return {
            values: vec![ValueId(12)],
        });
        module.functions.push(Function::internal_helper(
            name,
            Signature::new(vec![], vec![Type::Scalar(ScalarType::U32)]),
            vec![],
            vec![entry, exit],
        ));
    }
    module
}

fn bound(actual: Bound) -> Triple {
    Triple {
        w: actual.work_upper_bound(),
        r: actual.retained_storage_upper_bound(),
        p: actual.peak_storage_upper_bound(),
    }
}

fn assert_observation(
    actual: CanonicalRankedPolicyResourceObservationV1,
    expected: Triple,
    denial: Option<(Phase, &'static str)>,
    caught: bool,
) {
    assert_eq!(
        (
            actual.work_upper_bound(),
            actual.retained_storage_units(),
            actual.peak_storage_units()
        ),
        (expected.w, expected.r, expected.p),
        "{PROFILE}"
    );
    assert_eq!(actual.first_denial(), denial);
    assert_eq!(actual.caught_panic(), caught);
}

fn exact_limits(expected: Triple) -> Limits {
    let hard = Limits::production_hard_ceiling();
    assert!(expected.w <= hard.max_work());
    assert!(expected.p <= hard.max_peak_storage());
    Limits::new(expected.w, expected.p)
}

fn assert_report(report: &ProductionPlironPreloweringReportV2) {
    assert_eq!(report.pass_order(), &PASS_ORDER);
    assert!(report.is_clean());
    let certificates = report.preservation().certificates();
    let validations = report.report_validation().stages();
    assert_eq!(certificates.len(), 9);
    assert_eq!(validations.len(), 9);
    for (position, pass) in PASS_ORDER.iter().enumerate() {
        assert_eq!(certificates[position].pass(), *pass);
        assert_eq!(validations[position].checkpoint().pass(), *pass);
        assert_eq!(validations[position].checkpoint().position(), position);
    }
    assert_eq!(report.preservation().input_identity().canonical_len(), 1113);
    assert!(report.preservation().is_exact_identity());
    assert!(!report.grants_compiler_refinement_authority());
    assert!(!report.grants_artifact_or_launch_authority());
    assert!(
        !report
            .report_validation()
            .grants_lowering_or_launch_authority()
    );
    assert!(report.target_contract().is_none());
    assert!(report.tensor_layout().findings().is_empty());
    assert!(report.bounds().findings().is_empty());
    assert!(report.atomics().findings().is_empty());
    assert!(report.race().findings().is_empty());
    assert!(report.ownership().findings().is_empty());
    assert!(report.barriers().findings().is_empty());
    assert!(report.pipeline_protocol().findings().is_empty());
    assert!(report.workgroup().findings().is_empty());
    let semantic = report.semantics();
    assert!(semantic.findings().is_empty());
    assert!(semantic.typed_root_commitments().is_empty());
    assert!(semantic.numerical_certificates().is_empty());
    assert!(semantic.progress().findings().is_empty());
    assert!(semantic.progress().certificates().is_empty());
    assert!(semantic.effect_refinement().findings().is_empty());
    // Slice APIs cannot prove private Vec capacities. The empty constructors'
    // zero capacities are a pinned-source premise, not an observed fallback.
}

fn assert_tensor_panic(error: &Failure, ordinal: usize) {
    assert!(
        matches!(
            error,
            Failure::Analysis {
                function,
                cause: ProductionPlironPreloweringErrorV2::Preservation(
                    Preservation::AnalysisPanicked { pass: Pass::TensorLayout }
                ),
            } if *function == ordinal
        ),
        "{error:?}"
    );
}

fn run_denial(
    ordinal: usize,
    limits: Limits,
    phase: Phase,
    resource: &'static str,
    local: Triple,
    label: &str,
) {
    run_denial_in_module(ordinal + 1, ordinal, limits, phase, resource, local, label);
}

fn expected_quota_cause(
    phase: Phase,
    resource: &'static str,
    label: &str,
) -> ProductionPlironPreloweringErrorV2 {
    use crate::production_analysis::pliron_report_validation::ProductionAnalysisReportValidationErrorV1;
    use ProductionPlironPreloweringErrorV2 as Error;
    let pass = match label {
        "Tensor" => Some(Pass::TensorLayout),
        "Bounds" => Some(Pass::MemoryBounds),
        "Atomic" => Some(Pass::AtomicLegality),
        "Race" => Some(Pass::RaceFreedom),
        "Ownership" => Some(Pass::HierarchicalOwnership),
        "Barrier" => Some(Pass::BarrierConvergence),
        "Pipeline" => Some(Pass::PipelineProtocol),
        "Workgroup" => Some(Pass::WorkgroupMemory),
        "Semantic" => Some(Pass::SemanticRefinement),
        _ => None,
    };
    match phase {
        Phase::StructuralIdentity | Phase::PassPreservation => {
            Error::Preservation(Preservation::ResourceLimit { resource })
        }
        Phase::ReportValidation => {
            assert!(pass.is_some());
            Error::ReportValidation(ProductionAnalysisReportValidationErrorV1::ResourceLimit {
                producing_pass: pass,
                resource,
            })
        }
        _ => Error::ResourceLimit {
            phase,
            producing_pass: pass.filter(|pass| {
                matches!(
                    pass,
                    Pass::BarrierConvergence
                        | Pass::PipelineProtocol
                        | Pass::WorkgroupMemory
                        | Pass::SemanticRefinement
                )
            }),
            resource,
        },
    }
}

fn run_denial_in_module(
    count: usize,
    ordinal: usize,
    limits: Limits,
    phase: Phase,
    resource: &'static str,
    local: Triple,
    label: &str,
) {
    let oracle = Oracle::derive();
    let floor = oracle.module(ordinal);
    with_checked(&fixture(count), |checked, budget| {
        let kir_floor = budget.storage();
        let called = Cell::new(false);
        let error = with_checks(checked, budget, limits, |_, _| {
            called.set(true);
            Ok(())
        })
        .unwrap_err();
        assert!(!called.get(), "{label}");
        let Failure::Analysis { function, cause } = error.failure() else {
            panic!("{label}: expected analysis quota failure: {error:?}");
        };
        assert_eq!(*function, ordinal, "{label}");
        assert_eq!(
            *cause,
            expected_quota_cause(phase, resource, label),
            "{label}"
        );
        let history = error.last_invocation().expect("actual invocation history");
        assert_eq!(history.function(), ordinal, "{label}");
        assert_observation(history.floor(), floor, None, false);
        assert_observation(history.invocation(), local, Some((phase, resource)), false);
        assert_observation(
            error.observation(),
            floor.then(local),
            Some((phase, resource)),
            false,
        );
        assert_eq!(budget.storage(), kir_floor, "{label}");
    });
}

#[test]
fn independent_native_oracle_asserts_accessible_abi_and_small_vec_policy() {
    assert_eq!(size_of::<usize>(), 8);
    assert_eq!(size_of::<Ptr<LiveBlock>>(), 16);
    assert_eq!(size_of::<Ptr<LiveOperation>>(), 16);
    assert_eq!(size_of::<Value>(), 32);
    assert_eq!(size_of::<TypeHandle>(), 16);
    assert_eq!(size_of::<Use<Value>>(), 24);
    assert_eq!(size_of::<Use<Ptr<LiveBlock>>>(), 24);
    assert_eq!(size_of::<Vec<usize>>(), 24);
    assert_eq!(size_of::<HashMap<Ptr<LiveOperation>, usize>>(), 48);
    assert_eq!(size_of::<HashSet<Ptr<LiveBlock>>>(), 48);
    assert_eq!(size_of::<Bound>(), 24);
    let mut words = Vec::new();
    words.push(0_usize);
    assert_eq!(words.capacity(), 4);
    let mut pointer_sized = Vec::new();
    pointer_sized.push([0_usize; 2]);
    assert_eq!(pointer_sized.capacity(), 4);
    let mut outer = Vec::new();
    outer.push(Vec::<usize>::new());
    assert_eq!(outer.capacity(), 4);
    // Private Prescan14 and CheckedOrder13 are reviewed source layouts, not
    // accessible sizeof assertions. No visibility expansion is requested.
}

#[test]
fn independent_native_oracle_fixture_has_literal_authenticated_census() {
    with_projection(&fixture(2), |projection, budget| {
        for ordinal in 0..2 {
            projection
                .with_function(ordinal, budget, |context, function| {
                    let session = begin_production_pliron_pass_contract_session_v1(
                        LivePlironStructuralIdentityProviderV1::new(context, function),
                    )
                    .unwrap();
                    assert_eq!(
                        session.input_census_v1(),
                        Census {
                            blocks: 2,
                            operations: 3,
                            operands: 1,
                            results: 1,
                            successors: 1,
                            attributes: 3,
                            type_nodes: 5,
                            identifier_bytes: 515,
                            canonical_bytes: 1113,
                            max_operation_arity: 1,
                            max_successor_arity: 1,
                            ..Census::default()
                        }
                    );
                    drop(session);
                })
                .unwrap();
        }
    });
}

#[test]
fn independent_native_oracle_arithmetic_is_pinned_to_one_profile() {
    assert_eq!(PROFILE, "V851_PLUS_NATIVE_V854_RESOLVED_LAUNCH_RANK1");
    let o = Oracle::derive();
    assert_eq!(numbers::identity_text(), (515, 1113, 160));
    assert_eq!(
        o.identity,
        Triple {
            w: 4701347,
            r: 22490,
            p: 328696
        }
    );
    assert_eq!((o.native_work, o.native_temporary), (61 * 584, 194));
    assert_eq!(
        o.progress,
        Triple {
            w: 36111,
            r: 3250,
            p: 3554
        }
    );
    assert_eq!(
        o.finish,
        Triple {
            w: 1126,
            r: 1124,
            p: 23614
        }
    );
    assert_eq!(o.cache_release, 17826886);
    assert_eq!(
        o.complete,
        Triple {
            w: 129828378,
            r: 46622,
            p: 19947359
        }
    );
    assert_eq!(
        o.module(2),
        Triple {
            w: 259656756,
            r: 93244,
            p: 19993981
        }
    );
    assert_eq!(o.stage_work_cuts().len(), 27);
    assert_eq!(o.extra_work_cuts().len(), 9);
    assert_eq!(o.storage_cuts().len(), 11);
}

#[test]
fn independent_native_oracle_actual_nine_stage_success_at_exact_one_and_two_limits() {
    let o = Oracle::derive();
    for count in [1, 2] {
        // Separate direct invocations bind report order by exact retained bytes.
        // Their measured receipts never supply the independent oracle's limits.
        let (mut reference_state, references) =
            with_projection(&fixture(count), |projection, budget| {
                let mut state = AnalysisState::new(exact_limits(o.module(count)));
                let reports = (0..count)
                    .map(|ordinal| {
                        projection
                            .with_function(ordinal, budget, |context, function| {
                                state.invoke(ordinal, context, function)
                            })
                            .unwrap()
                            .unwrap()
                    })
                    .collect::<Vec<_>>();
                assert_observation(state.observation(), o.module(count), None, false);
                (state, reports)
            });
        with_checked(&fixture(count), |checked, budget| {
            let kir_floor = budget.storage();
            with_checks(
                checked,
                budget,
                exact_limits(o.module(count)),
                |view, budget| {
                    assert_eq!(view.function_count(budget)?, count);
                    assert_observation(view.observation(budget)?, o.module(count), None, false);
                    for ordinal in 0..count {
                        let report = view.report(ordinal, budget)?;
                        assert_report(report);
                        for (reference_ordinal, reference) in references.iter().enumerate() {
                            assert_eq!(
                                report.preservation().exactly_matches_retained_output(
                                    reference.report.preservation()
                                ),
                                ordinal == reference_ordinal
                            );
                        }
                        let history = view.history(ordinal, budget)?;
                        assert_eq!(history.function(), ordinal);
                        assert_observation(history.floor(), o.module(ordinal), None, false);
                        assert_observation(history.invocation(), o.complete, None, false);
                    }
                    assert_eq!(view.pending_obligations().iter().count(), 19);
                    assert!(!view.ranked_verification_is_complete());
                    assert!(!view.grants_artifact_or_launch_authority());
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(budget.storage(), kir_floor);
        });
        drop(references);
        reference_state.release_reports().unwrap();
        assert_observation(
            reference_state.observation(),
            o.module(count).released(),
            None,
            false,
        );
    }
}

#[test]
fn independent_native_oracle_outcome_and_receipt_are_separate_exact_observations() {
    let o = Oracle::derive();
    with_projection(&fixture(1), |projection, budget| {
        projection
            .with_function(0, budget, |context, function| {
                let limits = exact_limits(o.complete);
                let mut receipt = InvocationReceiptV1::new(Bound::default(), limits).unwrap();
                let outcome = require_production_pliron_checks_with_observation_v1(
                    context,
                    function,
                    limits,
                    &mut receipt,
                )
                .unwrap();
                assert_report(&outcome.report);
                assert_eq!(bound(outcome.resource_upper_bound), o.complete);
                let observation = receipt.snapshot();
                assert_eq!(bound(observation.current), o.complete);
                assert_eq!(bound(observation.committed), o.complete);
                assert_eq!(observation.first_denial, None);
                assert!(!observation.caught_panic);
                assert_eq!(bound(receipt.complete().unwrap()), o.complete);
                drop(outcome);
            })
            .unwrap();
    });
}

#[test]
fn independent_native_oracle_all_27_work_cuts_reach_each_real_ordinal() {
    let o = Oracle::derive();
    for ordinal in [0, 1] {
        for cut in o.stage_work_cuts() {
            run_denial(
                ordinal,
                Limits::new(o.module(ordinal).w + cut.limit, o.module(ordinal + 1).p),
                cut.phase,
                WORK_RESOURCE,
                cut.accepted,
                cut.name,
            );
        }
    }
}

#[test]
fn independent_native_oracle_setup_dependency_and_finish_work_cuts() {
    let o = Oracle::derive();
    for ordinal in [0, 1] {
        for cut in o.extra_work_cuts() {
            run_denial(
                ordinal,
                Limits::new(o.module(ordinal).w + cut.limit, o.module(ordinal + 1).p),
                cut.phase,
                WORK_RESOURCE,
                cut.accepted,
                cut.name,
            );
        }
    }
}

#[test]
fn independent_native_oracle_strict_storage_prefixes_and_second_trace() {
    let o = Oracle::derive();
    for cut in o.storage_cuts() {
        run_denial(
            0,
            Limits::new(o.complete.w, cut.limit),
            cut.phase,
            PEAK_RESOURCE,
            cut.accepted,
            cut.name,
        );
    }
    let trace = o.storage_cuts().pop().unwrap();
    run_denial(
        1,
        Limits::new(o.module(2).w, o.module(2).p - 1),
        Phase::InvocationTrace,
        PEAK_RESOURCE,
        trace.accepted,
        "second trace",
    );
}

#[test]
fn independent_native_oracle_later_storage_cuts_are_masked_not_stage_coverage() {
    let o = Oracle::derive();
    // Each later producer's complete invocation-relative peak is dominated by
    // the first trace. Its historical peak-one-short is really a trace cut.
    let trace = o.storage_cuts().pop().unwrap();
    for stage in &o.stages {
        assert_eq!(stage.checkpoint.p, o.complete.p);
        assert_eq!(stage.record.p, o.complete.p);
        assert!(stage.before.r + stage.producer.p < o.complete.p);
    }
    run_denial(
        0,
        Limits::new(o.complete.w, o.stages[8].checkpoint.p - 1),
        Phase::InvocationTrace,
        PEAK_RESOURCE,
        trace.accepted,
        "masked Semantic checkpoint",
    );
    // Before a second-function identity prefix can be reached, a limit below
    // the previous function's historical peak already denies function0 trace.
    run_denial_in_module(
        2,
        0,
        Limits::new(o.module(2).w, o.complete.p - 1),
        Phase::InvocationTrace,
        PEAK_RESOURCE,
        trace.accepted,
        "masked second identity",
    );
}

#[test]
fn independent_native_oracle_entry_panic_has_zero_prefix() {
    let o = Oracle::derive();
    with_checked(&fixture(1), |checked, budget| {
        let floor = budget.storage();
        pipeline::panic_next_production_analysis_for_test_v1();
        let error =
            with_checks(checked, budget, exact_limits(o.complete), |_, _| Ok(())).unwrap_err();
        assert!(matches!(error.failure(), Failure::Panicked));
        assert_observation(error.observation(), Triple::ZERO, None, true);
        let history = error.last_invocation().unwrap();
        assert_eq!(history.function(), 0);
        assert_observation(history.floor(), Triple::ZERO, None, false);
        assert_observation(history.invocation(), Triple::ZERO, None, true);
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn independent_native_oracle_tensor_panic_retains_exact_typed_prefix() {
    let o = Oracle::derive();
    with_checked(&fixture(1), |checked, budget| {
        let floor = budget.storage();
        let _panic = pipeline::panic_after_first_production_stage_for_test_v1();
        let error =
            with_checks(checked, budget, exact_limits(o.complete), |_, _| Ok(())).unwrap_err();
        assert_tensor_panic(error.failure(), 0);
        assert_observation(error.observation(), o.tensor_panic(), None, true);
        let history = error.last_invocation().unwrap();
        assert_eq!(history.function(), 0);
        assert_observation(history.floor(), Triple::ZERO, None, false);
        assert_observation(history.invocation(), o.tensor_panic(), None, true);
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn independent_native_oracle_later_panic_is_direct_state_not_consumer_hook_coverage() {
    let o = Oracle::derive();
    with_projection(&fixture(2), |projection, budget| {
        let mut state = AnalysisState::new(exact_limits(o.module(2)));
        let first = projection
            .with_function(0, budget, |context, function| {
                state.invoke(0, context, function)
            })
            .unwrap()
            .unwrap();
        assert_eq!(bound(first.resource_upper_bound), o.complete);
        assert_observation(state.observation(), o.complete, None, false);
        let _panic = pipeline::panic_after_first_production_stage_for_test_v1();
        let error = projection
            .with_function(1, budget, |context, function| {
                state.invoke(1, context, function)
            })
            .unwrap()
            .err()
            .expect("actual second invocation Tensor panic");
        assert_tensor_panic(&error, 1);
        let history = state.last.unwrap();
        assert_eq!(history.function(), 1);
        assert_observation(history.floor(), o.complete, None, false);
        assert_observation(history.invocation(), o.tensor_panic(), None, true);
        let total = o.complete.then(o.tensor_panic());
        assert_observation(state.observation(), total, None, true);
        assert_eq!(bound(first.resource_upper_bound), o.complete);
        assert_report(&first.report);
        drop(first);
        state.release_reports().unwrap();
        assert_observation(state.observation(), total.released(), None, true);
    });
}

#[test]
fn independent_native_oracle_transient_mutation_keeps_admitted_checkpoint_not_commit() {
    let o = Oracle::derive();
    with_checked(&fixture(1), |checked, budget| {
        let floor = budget.storage();
        pipeline::transiently_mutate_next_production_analysis_for_test_v1();
        let error =
            with_checks(checked, budget, exact_limits(o.complete), |_, _| Ok(())).unwrap_err();
        assert!(
            matches!(
                error.failure(),
                Failure::Analysis {
                    function: 0,
                    cause: ProductionPlironPreloweringErrorV2::Preservation(
                        Preservation::MutationAttempted {
                            pass: Some(Pass::TensorLayout),
                            ..
                        }
                    ),
                }
            ),
            "{error:?}"
        );
        assert_observation(error.observation(), o.transient_mutation(), None, false);
        let history = error.last_invocation().unwrap();
        assert_eq!(history.function(), 0);
        assert_observation(history.floor(), Triple::ZERO, None, false);
        assert_observation(history.invocation(), o.transient_mutation(), None, false);
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn independent_native_oracle_callback_panic_releases_reports_without_analysis_panic() {
    let o = Oracle::derive();
    for count in [1, 2] {
        with_checked(&fixture(count), |checked, budget| {
            let floor = budget.storage();
            let error = with_checks(
                checked,
                budget,
                exact_limits(o.module(count)),
                |_, _| -> Result<(), Failure> { panic!("nine-oracle user callback") },
            )
            .unwrap_err();
            assert!(matches!(error.failure(), Failure::Panicked));
            assert_observation(error.observation(), o.module(count).released(), None, false);
            let history = error.last_invocation().unwrap();
            assert_eq!(history.function(), count - 1);
            assert_observation(history.floor(), o.module(count - 1), None, false);
            assert_observation(history.invocation(), o.complete, None, false);
            assert_eq!(budget.storage(), floor);
        });
    }
}
