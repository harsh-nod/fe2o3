//! Real private caller/helper, all nine producers, and an independent profile.
use super::super::private_resources::PrivateAnalysisV1;
use super::*;
pub(crate) use crate::KernelCheckPassKindV1 as Pass;
pub(crate) use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisResourceLimitsV1 as Limits, ProductionAnalysisResourcePhaseV1 as Phase,
};
use crate::production_analysis::{
    pliron_ir_identity::LivePlironStructuralIdentityProviderV1,
    pliron_pass_contract::{
        PlironPassPreservationErrorV1 as Preservation,
        begin_production_pliron_pass_contract_session_v1,
    },
    pliron_pipeline::{self as pipeline, canonical_private_v1 as private_pipeline},
    pliron_report_validation::ProductionAnalysisReportValidationErrorV1 as Validation,
    pliron_resource_envelope::ProductionAnalysisInputCensusV1 as Census,
};
use pliron::{
    basic_block::BasicBlock as LiveBlock,
    context::Ptr,
    operation::Operation as LiveOperation,
    r#type::TypeHandle,
    value::{Use, Value},
};
use std::collections::{HashMap, HashSet};

#[path = "canonical_private_nine_oracle_v1_numbers.rs"]
pub(crate) mod numbers;
use numbers::{Cause, Oracle, PROFILE, Triple};

pub(crate) const PASS_ORDER: [Pass; 9] = [
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

pub(crate) fn triple(actual: Bound) -> Triple {
    Triple {
        w: actual.work_upper_bound(),
        r: actual.retained_storage_upper_bound(),
        p: actual.peak_storage_upper_bound(),
    }
}
pub(crate) fn assert_observation(
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
pub(crate) fn exact_limits(expected: Triple) -> Limits {
    let hard = Limits::production_hard_ceiling();
    assert!(expected.w <= hard.max_work());
    assert!(expected.p <= hard.max_peak_storage());
    Limits::new(expected.w, expected.p)
}
pub(crate) fn floor(ordinal: usize) -> Triple {
    if ordinal == 0 {
        Triple::ZERO
    } else {
        assert_eq!(ordinal, 1);
        Oracle::derive(0).complete
    }
}
pub(crate) fn assert_report(report: &pipeline::CanonicalPrivatePipelineReportV1, ordinal: usize) {
    assert_eq!(report.paired_stage_count(), 9);
    assert!(!report.grants_artifact_or_launch_authority());
    let report = report.reports();
    assert!(report.is_clean());
    assert_eq!(report.pass_order(), &PASS_ORDER);
    assert!(report.target_contract().is_none());
    assert_eq!(
        report.preservation().input_identity().canonical_len(),
        numbers::identity_text(numbers::Shape::at(ordinal)).1
    );
    assert!(report.preservation().is_exact_identity());
    assert_eq!(report.preservation().certificates().len(), 9);
    assert_eq!(report.report_validation().stages().len(), 9);
    for (position, pass) in PASS_ORDER.iter().enumerate() {
        assert_eq!(report.preservation().certificates()[position].pass(), *pass);
        let checkpoint = report.report_validation().stages()[position].checkpoint();
        assert_eq!(checkpoint.position(), position);
        assert_eq!(checkpoint.pass(), *pass);
    }
    assert!(!report.grants_compiler_refinement_authority());
    assert!(!report.grants_artifact_or_launch_authority());
    assert!(
        !report
            .report_validation()
            .grants_lowering_or_launch_authority()
    );
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
    // Public slices prove lengths, not private Vec capacities. The latter are
    // separately pinned empty-constructor premises, never a measured fallback.
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Expected {
    Quota {
        phase: Phase,
        cause: Cause,
        resource: &'static str,
    },
    Coverage,
    EntryPanic,
    TensorPanic,
    Mutation,
}
impl Expected {
    fn observation(self) -> (Option<(Phase, &'static str)>, bool) {
        match self {
            Self::Quota {
                phase, resource, ..
            } => (Some((phase, resource)), false),
            Self::EntryPanic | Self::TensorPanic => (None, true),
            _ => (None, false),
        }
    }
    fn assert(self, error: &Failure, ordinal: usize) {
        match self {
            Self::Quota {
                phase,
                cause,
                resource,
            } => {
                let expected = match cause {
                    Cause::Preservation => ProductionPlironPreloweringErrorV2::Preservation(
                        Preservation::ResourceLimit { resource },
                    ),
                    Cause::Validation(producing_pass) => {
                        ProductionPlironPreloweringErrorV2::ReportValidation(
                            Validation::ResourceLimit {
                                producing_pass,
                                resource,
                            },
                        )
                    }
                    Cause::Resource(producing_pass) => {
                        ProductionPlironPreloweringErrorV2::ResourceLimit {
                            phase,
                            producing_pass,
                            resource,
                        }
                    }
                };
                match error {
                    Failure::Analysis { function, cause } => {
                        assert_eq!(*function, ordinal);
                        assert_eq!(*cause, expected);
                    }
                    _ => panic!("expected exact quota cause: {error:?}"),
                }
            }
            Self::Coverage => assert!(
                matches!(
                    error,
                    Failure::PrivateRequirement {
                        requirement: CanonicalPrivateRequirementV1::StageCoverage,
                        ..
                    }
                ),
                "{error:?}"
            ),
            Self::EntryPanic => assert!(matches!(error, Failure::Panicked), "{error:?}"),
            Self::TensorPanic => assert!(
                matches!(error, Failure::Analysis { function, cause:
                ProductionPlironPreloweringErrorV2::Preservation(Preservation::AnalysisPanicked {
                    pass: Pass::TensorLayout,
                }) } if *function == ordinal),
                "{error:?}"
            ),
            Self::Mutation => assert!(
                matches!(error, Failure::Analysis { function, cause:
                ProductionPlironPreloweringErrorV2::Preservation(Preservation::StructuralIdentityChanged {
                    pass: Pass::TensorLayout, source_code: "FE2O3-PRESERVE-005", ..
                }) } if *function == ordinal),
                "{error:?}"
            ),
        }
    }
}

pub(crate) fn public_failure(ordinal: usize, limits: Limits, local: Triple, expected: Expected) {
    with_checked(&fixture(), |checked, budget| {
        let kir_floor = budget.storage();
        let called = std::cell::Cell::new(false);
        let error = with_private_checks(checked, budget, limits, |_, _| {
            called.set(true);
            Ok(())
        })
        .unwrap_err();
        assert!(!called.get());
        expected.assert(error.failure(), ordinal);
        let (denial, caught) = expected.observation();
        let history = error.last_invocation().expect("real private invocation");
        assert_eq!(history.function(), ordinal);
        assert_observation(history.floor(), floor(ordinal), None, false);
        assert_observation(history.invocation(), local, denial, caught);
        assert_observation(
            error.observation(),
            floor(ordinal).then(local),
            denial,
            caught,
        );
        assert_eq!(budget.storage(), kir_floor);
    });
}

// This is explicitly direct PrivateAnalysisV1 coverage. The production module
// consumer has no between-definition injection hook and no test-only selector.
pub(crate) fn direct_failure(
    ordinal: usize,
    local: Triple,
    expected: Expected,
    around: impl FnOnce(&mut dyn FnMut() -> Result<(), Failure>) -> Result<(), Failure>,
) {
    let complete = numbers::module();
    with_projection(&fixture(), |projection, budget| {
        let mut state = PrivateAnalysisV1::new(exact_limits(complete));
        let first = if ordinal == 1 {
            Some(projection.with_function(0, budget, |input| state.invoke(input))??)
        } else {
            None
        };
        assert_observation(state.observation(), floor(ordinal), None, false);
        projection.with_function(ordinal, budget, |input| {
            let result = {
                let mut run = || {
                    state.invoke(input).map(|outcome| {
                        drop(outcome);
                    })
                };
                around(&mut run)
            };
            let error = result.expect_err("hostile invocation cannot return a report");
            expected.assert(&error, ordinal);
            let (denial, caught) = expected.observation();
            let history = state.last.expect("real direct invocation");
            assert_eq!(history.function(), ordinal);
            assert_observation(history.floor(), floor(ordinal), None, false);
            assert_observation(history.invocation(), local, denial, caught);
            assert_observation(
                state.observation(),
                floor(ordinal).then(local),
                denial,
                caught,
            );
            if let Some(first) = &first {
                assert_eq!(
                    triple(first.resource_upper_bound),
                    Oracle::derive(0).complete
                );
                assert_report(&first.report, 0);
            }
        })?;
        drop(first);
        state.release_reports()?;
        let (denial, caught) = expected.observation();
        assert_observation(
            state.observation(),
            floor(ordinal).then(local).released(),
            denial,
            caught,
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn private_nine_oracle_accessible_abi_and_capacity_premises() {
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
    let mut pointers = Vec::new();
    for expected in [4, 4, 4, 4, 8] {
        pointers.push([0usize; 2]);
        assert_eq!(pointers.capacity(), expected);
    }
    let mut outer = Vec::new();
    outer.push(Vec::<usize>::new());
    assert_eq!(outer.capacity(), 4);
    // Prescan14/CheckedOrder13 and empty report Vec capacities are private
    // layout/source-review premises. No visibility changes assert them here.
}

#[test]
fn private_nine_oracle_literal_native_census_and_real_source_fixture() {
    let module = fixture();
    assert_eq!(module.functions[0].id.as_str(), "zeta");
    assert_eq!(module.functions[1].id.as_str(), "original_helper");
    with_projection(&module, |projection, budget| {
        for ordinal in 0..2 {
            projection.with_function(ordinal, budget, |input| {
                let s = numbers::Shape::at(ordinal);
                assert_eq!(input.ordinal(), ordinal);
                assert_eq!(input.operation_count(), s.o);
                let session = begin_production_pliron_pass_contract_session_v1(
                    LivePlironStructuralIdentityProviderV1::canonical_private(input),
                )
                .unwrap();
                let (i, k, _, _) = numbers::identity_text(s);
                assert_eq!(
                    session.input_census_v1(),
                    Census {
                        blocks: 1,
                        operations: s.o,
                        operands: s.a,
                        results: s.r,
                        attributes: s.attrs,
                        type_nodes: s.types,
                        identifier_bytes: i,
                        canonical_bytes: k,
                        max_operation_arity: s.arity,
                        ..Census::default()
                    }
                );
                assert_eq!(crate::production_analysis::pliron_progress::MAX_PLIRON_PROGRESS_REGIONS_V1, 4096);
                let graph = crate::production_analysis::pliron_progress::preflight_progress_graph_resource_upper_bound_v2(
                    session.input_census_v1(), Limits::production_hard_ceiling(),
                ).unwrap();
                assert_eq!(triple(graph), numbers::progress_graph(s));
                drop(session);
            })?;
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn private_nine_oracle_independent_profile_has_both_lookup_charges_and_all_stages() {
    assert_eq!(
        PROFILE,
        "PRIVATE35_NATIVE_DIRECT_SSA_RANK1_CALLER2_HELPER5_GRAPH_FIRST"
    );
    for ordinal in 0..2 {
        let o = Oracle::derive(ordinal);
        assert_eq!(
            numbers::identity_text(o.shape),
            [(455, 878, 90, 15), (1143, 2133, 310, 30)][ordinal]
        );
        assert_eq!(
            o.complete,
            [
                Triple {
                    w: 126720717,
                    r: 39462,
                    p: 19940272
                },
                Triple {
                    w: 212920668,
                    r: 57320,
                    p: 19956297
                },
            ][ordinal]
        );
        assert_eq!(o.stages.len(), 9);
        assert_eq!(o.gates.len(), [186, 216][ordinal]);
        assert_eq!(o.gates.iter().filter(|gate| gate.work_cut).count(), 85);
        assert_eq!(o.identity.structural[0].w, 512 * (o.shape.o + 1).pow(2) + 1);
        assert_eq!(o.shape.coverage().w, 32 * (o.shape.o + 1).pow(2) + 32);
        assert_eq!(o.progress, Triple::new(2112, 1056, 1072));
        assert_eq!(
            numbers::progress_graph(o.shape),
            [
                Triple::new(74033, 1152, 9326),
                Triple::new(88673, 1176, 9380)
            ][ordinal]
        );
        let graph = o
            .gates
            .iter()
            .position(|gate| gate.name == "progress graph")
            .unwrap();
        assert_eq!(o.gates[graph - 1].name, "pipeline prerequisite");
        assert_eq!(o.gates[graph + 1].name, "private prepare");
        assert_eq!(o.gates[graph + 1].phase, Phase::BarrierConvergence);
        assert!(
            o.identity.complete.w > o.identity.text.w + o.identity.closure.w + o.shape.lookup()
        );
        assert_eq!(
            o.gates
                .iter()
                .filter(|g| g.name == "private prepare")
                .count(),
            9
        );
        assert_eq!(
            o.gates
                .iter()
                .filter(|g| g.name == "private record")
                .count(),
            9
        );
        assert_eq!(o.gates.iter().filter(|g| g.name == "checkpoint").count(), 9);
        assert_eq!(
            o.gates
                .iter()
                .filter(|g| g.name == "ordinary record")
                .count(),
            9
        );
        assert_eq!(o.gates.last().unwrap().name, "private finish");
        assert!(o.denial(o.complete.w, o.complete.p).is_none());
    }
    assert_eq!(
        numbers::module(),
        Triple {
            w: 339641385,
            r: 96782,
            p: 19995759
        }
    );
}

#[test]
fn private_nine_oracle_actual_consumer_succeeds_at_exact_composed_limits() {
    let expected = numbers::module();
    with_checked(&fixture(), |checked, budget| {
        let source = checked.inventory(budget).unwrap().owner();
        let kir_floor = budget.storage();
        with_private_checks(checked, budget, exact_limits(expected), |view, budget| {
            assert!(std::ptr::eq(view.owner(budget)?, source));
            assert_eq!(view.function_count(budget)?, 2);
            assert_observation(view.observation(budget)?, expected, None, false);
            for ordinal in 0..2 {
                assert_report(view.report(ordinal, budget)?, ordinal);
                let history = view.history(ordinal, budget)?;
                assert_eq!(history.function(), ordinal);
                assert_observation(history.floor(), floor(ordinal), None, false);
                assert_observation(
                    history.invocation(),
                    Oracle::derive(ordinal).complete,
                    None,
                    false,
                );
            }
            assert_eq!(view.pending_obligations().iter().count(), 19);
            assert!(!view.ranked_verification_is_complete());
            assert!(!view.grants_artifact_or_launch_authority());
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), kir_floor);
    });
}

#[test]
fn private_nine_oracle_individual_real_outputs_and_receipts_are_separately_exact() {
    with_projection(&fixture(), |projection, budget| {
        for ordinal in 0..2 {
            let expected = Oracle::derive(ordinal).complete;
            projection.with_function(ordinal, budget, |input| {
                let limits = exact_limits(expected);
                let mut receipt = InvocationReceiptV1::new(Bound::default(), limits).unwrap();
                let outcome = private_pipeline::run(input, limits, Some(&mut receipt)).unwrap();
                assert_eq!(triple(outcome.resource_upper_bound), expected);
                assert_report(&outcome.report, ordinal);
                let observed = receipt.snapshot();
                assert_eq!(triple(observed.current), expected);
                assert_eq!(triple(observed.committed), expected);
                assert_eq!(observed.first_denial, None);
                assert!(!observed.caught_panic);
                assert_eq!(triple(receipt.complete().unwrap()), expected);
            })?;
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn private_nine_oracle_direct_state_keeps_caller_report_floor_until_paid_release() {
    with_projection(&fixture(), |projection, budget| {
        let total = numbers::module();
        let mut state = PrivateAnalysisV1::new(exact_limits(total));
        let first = projection.with_function(0, budget, |input| state.invoke(input))??;
        assert_observation(state.observation(), Oracle::derive(0).complete, None, false);
        let second = projection.with_function(1, budget, |input| state.invoke(input))??;
        assert_observation(state.observation(), total, None, false);
        assert_report(&first.report, 0);
        assert_report(&second.report, 1);
        assert!(
            !first
                .report
                .reports()
                .preservation()
                .exactly_matches_retained_output(second.report.reports().preservation())
        );
        drop((first, second));
        state.release_reports()?;
        assert_observation(state.observation(), total.released(), None, false);
        Ok(())
    })
    .unwrap();
}

pub(crate) fn callback_failure_matrix() {
    use crate::production_analysis::canonical_ranked_checks_v1::private::with_canonical_private_policy_checks_v1;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    let full = numbers::module();
    for mode in 0..4 {
        with_checked(&fixture(), |checked, budget| {
            let kir_floor = budget.storage();
            let mut other_work = Work::new(1 << 48);
            let mut other = Budget::new(&mut other_work, 1 << 32);
            let error = with_canonical_private_policy_checks_v1(
                checked,
                budget,
                |view, budget| -> Result<(), Failure> {
                    assert_observation(view.observation(budget)?, full, None, false);
                    match mode {
                        0 => Err(Failure::Callback("private-nine callback error")),
                        1 => panic!("private-nine callback panic"),
                        2 => {
                            assert!(view.function_count(&mut other).is_err());
                            Ok(())
                        }
                        _ => {
                            assert!(view.report(2, budget).is_err());
                            Ok(())
                        }
                    }
                },
            )
            .unwrap_err();
            match mode {
                0 => assert!(matches!(
                    error.failure(),
                    Failure::Callback("private-nine callback error")
                )),
                1 => assert!(matches!(error.failure(), Failure::Panicked)),
                2 => assert!(matches!(
                    error.failure(),
                    Failure::Resource(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
                    )
                )),
                _ => assert!(matches!(
                    error.failure(),
                    Failure::InvalidQuery { function: 2 }
                )),
            }
            assert_eq!(other.work(), 0);
            assert_observation(error.observation(), full.released(), None, false);
            let history = error.last_invocation().unwrap();
            assert_eq!(history.function(), 1);
            assert_observation(history.floor(), floor(1), None, false);
            assert_observation(
                history.invocation(),
                Oracle::derive(1).complete,
                None,
                false,
            );
            assert_eq!(budget.storage(), kir_floor);
        });
    }
}
