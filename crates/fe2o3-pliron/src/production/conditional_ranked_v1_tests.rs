use super::*;

include!("conditional_ranked_v1_execution_tests.rs");

include!("conditional_pipeline_v1_tests.rs");

#[cfg(feature = "internal-proof-staging")]
mod source259_full_conditional_observation {
    use super::{source259_execution::conditional_effect::effect_owner, *};
    use crate::production_analysis::{self as pa, conditional_execution_v1 as ce};
    use pa::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as ReceiptFailure, InvocationReceiptV1 as Receipt,
    };

    type Limits = pa::ProductionAnalysisResourceLimitsV1;
    const OWNER: Phase = Phase::PipelineVerification;

    fn hard() -> Limits {
        Limits::production_hard_ceiling()
    }

    fn prepare(
        pending: &ProductionConditionalRankedAnalysisV1,
    ) -> (
        ProductionAnalysisResourceContractV1,
        ConditionalPipelineSubjectV1<'_>,
    ) {
        let p = pending.analysis._analyses.resource_upper_bound();
        let mut resources = ProductionAnalysisResourceContractV1::new(hard());
        resources.admit_retained(OWNER, p).unwrap();
        let input = pending.prepare_pipeline_subject_v1(&mut resources).unwrap();
        let q = resources.cumulative();
        assert!(q.work_upper_bound() > p.work_upper_bound());
        assert!(q.retained_storage_upper_bound() > p.retained_storage_upper_bound());
        assert!(std::ptr::eq(input.recipe(), pending.kernel().unwrap()));
        (resources, input)
    }

    fn assert_selected_and_nested(
        report: &pa::ConditionalPipelineReportV1,
        pending: &ProductionConditionalRankedAnalysisV1,
    ) {
        assert_eq!(report.test_pass_count(), 9);
        let ownership = report.ownership();
        assert!(ownership.is_clean());
        assert_eq!(ownership.selected.len(), 1);
        let row = &ownership.selected[0];
        assert_eq!(row.recipe, pending.selections()[0]);
        let ce::SelectedResultV1::Checked(facts) = row.result else {
            panic!("selected rule did not run");
        };
        assert_eq!(facts.view, row.view);
        assert_eq!(facts.write.block, 1);
        assert_eq!(facts.write.operation, 0);
        assert_eq!(facts.normal_exits, [2, 2]);
        assert_eq!(
            ownership
                .residual
                .as_ref()
                .unwrap()
                .coverage_summary()
                .total_view_proved(),
            0
        );
        // Current is_clean requires Executed effect, Executed fresh nested
        // ownership, and complete selected bindings; NotRun cannot satisfy it.
        assert!(report.semantics().is_clean());
        assert!(!report.independently_validated());
    }

    fn positive(short: u8) {
        let pending = effect_owner(0);
        let original_p = pending.analysis._analyses.resource_upper_bound();
        let (resources, input) = prepare(&pending);
        let q = resources.cumulative();
        let first = pa::run_conditional_production_checks_v1(
            &input,
            pending._session.atomic_target.as_ref(),
            None,
            hard().remaining_after_retained(OWNER, q).unwrap(),
        )
        .unwrap();
        assert_selected_and_nested(&first.report, &pending);

        // The baseline report remains alive throughout the Some invocation.
        let floor = q
            .checked_then_retain(first.resource_upper_bound, OWNER)
            .unwrap();
        let total = floor
            .checked_then_retain(first.resource_upper_bound, OWNER)
            .unwrap();
        hard().require(OWNER, total).unwrap();
        let global = Limits::new(
            total.work_upper_bound() - usize::from(short == 1),
            total.peak_storage_upper_bound() - usize::from(short == 2),
        );

        if short == 2 && total.peak_storage_upper_bound() == floor.peak_storage_upper_bound() {
            // Historical peak masks the invocation boundary. This is an
            // inherited-floor refusal, not evidence of a producer denial.
            let error = Receipt::new(floor, global).err().unwrap();
            assert_eq!(error.phase, OWNER);
            assert_eq!(error.resource, "peak storage upper bound");
            return;
        }

        let mut receipt = Receipt::new(floor, global).unwrap();
        let observed = pa::run_conditional_production_checks_with_observation_v1(
            &input,
            pending._session.atomic_target.as_ref(),
            None,
            // Keep local manager limits unchanged; tighten only global receipt.
            hard().remaining_after_retained(OWNER, floor).unwrap(),
            Some(&mut receipt),
        );
        let state = receipt.snapshot();
        assert_eq!(state.current, state.committed);
        assert!(!state.caught_panic);
        assert_eq!(
            pending.analysis._analyses.resource_upper_bound(),
            original_p
        );
        assert_eq!(pending.pending_pipeline_checks().len(), 9);
        assert_eq!(
            input.context().ir_mutation_attempt_epoch().unwrap().value(),
            input.epoch()
        );
        global
            .require(
                OWNER,
                floor.checked_then_retain(state.current, OWNER).unwrap(),
            )
            .unwrap();

        if short == 0 {
            let observed = observed.unwrap();
            assert_eq!(observed.resource_upper_bound, first.resource_upper_bound);
            assert_eq!(state.first_denial, None);
            assert_eq!(state.current, observed.resource_upper_bound);
            assert_eq!(receipt.complete(), Ok(observed.resource_upper_bound));
            assert_selected_and_nested(&observed.report, &pending);
            // Includes private nested payloads, not merely clean/status getters.
            assert_eq!(observed.report, first.report);
        } else {
            assert!(observed.is_err());
            let denial = state.first_denial.expect("global quota was not latched");
            assert_eq!(
                denial.resource,
                if short == 1 {
                    "work upper bound"
                } else {
                    "peak storage upper bound"
                }
            );
            assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(denial)));
            if short == 1 {
                assert!(
                    state.current.work_upper_bound()
                        < first.resource_upper_bound.work_upper_bound()
                );
            } else {
                assert!(
                    state.current.peak_storage_upper_bound()
                        < first.resource_upper_bound.peak_storage_upper_bound()
                );
            }
        }
        // Reports/receipt/input all leave scope before pending.
    }

    #[test]
    fn actual_conditional_some_exact_receipt_and_full_report_parity() {
        positive(0);
    }

    #[test]
    fn actual_conditional_global_one_short_or_inherited_peak_refusal() {
        for short in [1, 2] {
            // Independent fixture and ledger for each boundary.
            positive(short);
        }
    }

    #[test]
    fn actual_conditional_semantic_refusals_are_not_resource_denials() {
        for case in 1..=5 {
            let pending = effect_owner(case);
            let original_p = pending.analysis._analyses.resource_upper_bound();
            let (resources, input) = prepare(&pending);
            let q = resources.cumulative();
            let mut receipt = Receipt::new(q, hard()).unwrap();
            let observed = pa::run_conditional_production_checks_with_observation_v1(
                &input,
                pending._session.atomic_target.as_ref(),
                None,
                hard().remaining_after_retained(OWNER, q).unwrap(),
                Some(&mut receipt),
            )
            .err()
            .expect("semantic refusal unexpectedly succeeded");
            let pa::PipelineErrorV1::ConditionalSemantic(observed) = observed else {
                panic!("case {case}: unexpected observed error {observed:?}");
            };
            assert!(!observed.report.is_clean());

            let state = receipt.snapshot();
            assert_eq!(state.first_denial, None, "case {case}");
            assert!(!state.caught_panic);
            assert_eq!(state.current, state.committed);
            assert!(state.current.work_upper_bound() > 0);
            assert!(state.current.retained_storage_upper_bound() > 0);
            assert_eq!(receipt.complete(), Ok(state.current));

            // Actual failed-run accounting covers its retained diagnostic owner.
            // Ordinary Err has no receipt, so it cannot be the measured baseline.
            let floor = q.checked_then_retain(state.current, OWNER).unwrap();
            let ordinary = pa::run_conditional_production_checks_v1(
                &input,
                pending._session.atomic_target.as_ref(),
                None,
                hard().remaining_after_retained(OWNER, floor).unwrap(),
            )
            .err()
            .expect("ordinary semantic refusal unexpectedly succeeded");
            let pa::PipelineErrorV1::ConditionalSemantic(ordinary) = ordinary else {
                panic!("case {case}: unexpected ordinary error {ordinary:?}");
            };
            assert!(!ordinary.report.is_clean());
            assert_eq!(ordinary.report, observed.report, "case {case}");
            assert_eq!(
                pending.analysis._analyses.resource_upper_bound(),
                original_p
            );
            assert_eq!(pending.pending_pipeline_checks().len(), 9);
            assert_eq!(
                input.context().ir_mutation_attempt_epoch().unwrap().value(),
                input.epoch()
            );
            // No fabricated successful bound is assigned to ordinary Err.
            drop(ordinary);
            drop(observed);
        }
    }
}
use dialect_kernel::{
    AccessKindAttr, DYNAMIC_EXTENT, OwnershipCoverageAttr, OwnershipPartitionAttr,
};

type Site = ProductionConditionalOwnershipSiteV1;
type Check = ProductionConditionalOwnershipCheckV1;
type Blocker = ProductionConditionalOwnershipBlockerV1;

fn local(index: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(index))
}

fn session() -> ProductionPlironSessionV1 {
    ProductionPlironSessionV1::new(
        ProductionSessionLimitsV1::default(),
        [
            dialect_gpu::dialect_registration().unwrap(),
            dialect_kernel::dialect_registration().unwrap(),
            dialect_proof::dialect_registration().unwrap(),
        ],
    )
    .unwrap()
}

fn recipe(expanding: bool, guarded: bool) -> (ProductionRankedKernelV1, [Site; 2]) {
    use ProductionRankedOperationV1 as O;
    use ProductionRankedTerminatorV1 as T;
    let mut entry = vec![
        O::ExecutionLayout {
            grid_identity: 41,
            global_extents: [DYNAMIC_EXTENT, 1, 1],
            workgroup_extents: [2, 1, 1],
            subgroup_size: 2,
            full_physical_workgroups: true,
        },
        O::InvocationIndex {
            result: ProductionRankedValueIdV1::new(0),
            dimension: 0,
            launch_extent: DYNAMIC_EXTENT,
        },
    ];
    for index in 1..=2 {
        entry.push(O::View {
            result: ProductionRankedValueIdV1::new(index),
            element_width: 32,
            writable: true,
            shape: vec![DYNAMIC_EXTENT],
            dynamic_extents: vec![ProductionRankedValueV1::Argument(0)],
            allocation_origin: u64::from(index),
            noalias_class: u64::from(index),
        });
    }
    entry.push(O::Dimension {
        result: ProductionRankedValueIdV1::new(3),
        view: local(1),
        dimension: 0,
    });
    if expanding {
        entry.push(O::SemanticExpression {
            result: ProductionRankedValueIdV1::new(4),
            expression: ProductionSemanticExpressionV2::Constant {
                scalar: ProductionSemanticScalarTypeV2::Bool,
                bits: 1,
            },
            numerical_contract: ProductionNumericalContractV2::exact_for(
                ProductionSemanticScalarTypeV2::Bool,
            ),
        });
    }
    let sites = [1, 2].map(|index| {
        let site = Site {
            block: 0,
            operation: entry.len() as u32,
            view: local(index),
        };
        entry.push(O::OwnershipContract {
            view: site.view,
            coverage: OwnershipCoverageAttr::TotalView,
            partition: OwnershipPartitionAttr::ExactSets,
        });
        site
    });
    let branch = if guarded {
        T::IndexLessThan {
            lhs: local(0),
            rhs: local(3),
            true_block: 1,
            false_block: 2,
        }
    } else {
        T::Branch { target: 1 }
    };
    let kernel = ProductionRankedKernelV1::new(
        "conditional",
        1,
        vec![
            ProductionRankedBlockV1::new(entry, branch),
            ProductionRankedBlockV1::new(
                [1, 2]
                    .map(|index| O::Access {
                        kind: AccessKindAttr::Write,
                        view: local(index),
                        indices: vec![local(0)],
                    })
                    .into(),
                T::Branch { target: 2 },
            ),
            ProductionRankedBlockV1::new(vec![], T::Return),
        ],
    )
    .unwrap();
    (kernel, sites)
}

fn construct(
    session: &mut ProductionPlironSessionV1,
    name: &str,
    expanding: bool,
    guarded: bool,
) -> (
    ProductionStageHandleV1<ConstructedGraphStageV1>,
    ProductionRootHandleV1<ConstructedGraphStageV1>,
    [Site; 2],
) {
    let (kernel, sites) = recipe(expanding, guarded);
    let registered = session
        .register_construction(ProductionConstructionV1::ranked_kernel(name, kernel).unwrap())
        .unwrap();
    let (stage, root) = session.construct_registered(registered).unwrap();
    (stage, root, sites)
}

mod source259_subject {
    use super::*;

    fn pending() -> ProductionConditionalRankedAnalysisV1 {
        let mut session = session();
        let (stage, root, sites) = construct(&mut session, "conditional_subject", true, true);
        session
            .prepare_conditional_ranked_analysis_v1(stage, root, &sites[..1])
            .unwrap()
    }

    fn resources(
        pending: &ProductionConditionalRankedAnalysisV1,
        limits: ProductionAnalysisResourceLimitsV1,
    ) -> ProductionAnalysisResourceContractV1 {
        let mut resources = ProductionAnalysisResourceContractV1::new(limits);
        resources
            .admit_retained(
                Phase::PipelineVerification,
                pending.analysis._analyses.resource_upper_bound(),
            )
            .unwrap();
        resources
    }

    #[test]
    fn subject_borrows_original_custody_and_exact_expanded_occurrences() {
        let pending = pending();
        let old = pending.legacy_report().clone();
        let mut resources = resources(
            &pending,
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        );
        let subject = pending.prepare_pipeline_subject_v1(&mut resources).unwrap();
        assert!(std::ptr::eq(subject.recipe(), pending.kernel().unwrap()));
        assert!(std::ptr::eq(
            subject.context(),
            &pending._session.inner.context
        ));
        assert!(std::ptr::eq(
            subject.identity(),
            &pending.analysis._identity
        ));
        assert_eq!(
            subject.function().get_operation(),
            pending.analysis.payload.function()
        );
        assert_eq!(
            subject.census(),
            pending.analysis._analyses.input_census().unwrap()
        );
        assert_eq!(subject.epoch(), pending.analysis._mutation_epoch);
        let original =
            &pending._session.constructed_roots[&pending._stage.identity].ownership_occurrences[0];
        assert_eq!(
            subject.occurrences(),
            &[(original.site, original.operation, original.view)]
        );
        assert!(pending.rows()[0].location().operation() > original.site.operation as usize);
        assert_eq!(pending.legacy_report(), &old);
        assert_eq!(pending.pending_pipeline_checks().len(), 9);
    }

    #[test]
    fn subject_requires_retained_floor_and_exact_combined_limits() {
        let pending = pending();
        let ceiling = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        let mut missing = ProductionAnalysisResourceContractV1::new(ceiling);
        assert!(matches!(
            pending.prepare_pipeline_subject_v1(&mut missing),
            Err(ProductionSessionErrorV1::AnalysisResourceLimit {
                phase: Phase::PipelineVerification,
                resource: "conditional pending owner floor",
                ..
            })
        ));
        assert_eq!(missing.cumulative(), Bound::default());
        let mut measured = resources(&pending, ceiling);
        drop(pending.prepare_pipeline_subject_v1(&mut measured).unwrap());
        let exact = measured.cumulative();
        let mut admitted = resources(
            &pending,
            ProductionAnalysisResourceLimitsV1::new(
                exact.work_upper_bound(),
                exact.peak_storage_upper_bound(),
            ),
        );
        drop(pending.prepare_pipeline_subject_v1(&mut admitted).unwrap());
        assert_eq!(admitted.cumulative(), exact);
        for (work_short, storage_short) in [(1, 0), (0, 1)] {
            let mut limited =
                ProductionAnalysisResourceContractV1::new(ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound() - work_short,
                    exact.peak_storage_upper_bound() - storage_short,
                ));
            let floor = pending.analysis._analyses.resource_upper_bound();
            if let Err(error) = limited.admit_retained(Phase::PipelineVerification, floor) {
                assert_eq!((work_short, storage_short), (0, 1));
                assert_eq!(
                    error,
                    ProductionAnalysisResourceLimitV1 {
                        phase: Phase::PipelineVerification,
                        resource: "peak storage upper bound",
                    }
                );
                assert_eq!(
                    floor.peak_storage_upper_bound(),
                    exact.peak_storage_upper_bound()
                );
                assert_eq!(pending.analysis._analyses.resource_upper_bound(), floor);
                continue;
            }
            assert!(matches!(
                pending.prepare_pipeline_subject_v1(&mut limited),
                Err(ProductionSessionErrorV1::AnalysisResourceLimit { .. })
            ));
            assert!(limited.cumulative().work_upper_bound() >= floor.work_upper_bound());
            assert!(
                limited.cumulative().retained_storage_upper_bound()
                    >= floor.retained_storage_upper_bound()
            );
        }
    }

    #[test]
    fn subject_rejects_same_text_mutation_attempt_without_resetting_the_floor() {
        let pending = pending();
        let mut resources = resources(
            &pending,
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        );
        let before = resources.cumulative();
        let pointer = pending.rows()[0].operation();
        drop(pointer.deref_mut(&pending._session.inner.context));
        assert!(matches!(
            pending.prepare_pipeline_subject_v1(&mut resources),
            Err(ProductionSessionErrorV1::RankedGraphChanged)
        ));
        assert!(resources.cumulative().work_upper_bound() > before.work_upper_bound());
        assert!(
            resources.cumulative().retained_storage_upper_bound()
                > before.retained_storage_upper_bound()
        );
    }

    #[test]
    fn subject_input_storage_is_admitted_before_hashing_or_allocation() {
        use crate::production_analysis::conditional_execution_v1::OccurrenceV1;
        let pending = pending();
        let floor = pending.analysis._analyses.resource_upper_bound();
        let padding = floor.peak_storage_upper_bound() - floor.retained_storage_upper_bound();
        let input_storage = std::mem::size_of::<ConditionalPipelineSubjectV1<'_>>()
            + std::mem::size_of::<ProductionConditionalPipelineAnalysisV1>()
            + pending.selections().len() * std::mem::size_of::<OccurrenceV1>();
        let limit = floor.peak_storage_upper_bound() + input_storage - 1;
        let mut ledger = resources(
            &pending,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, limit),
        );
        ledger
            .admit_retained(
                Phase::PipelineVerification,
                Bound::checked_phase(Phase::PipelineVerification, 0, padding, 0).unwrap(),
            )
            .unwrap();
        let prefix = ledger.cumulative();
        assert!(matches!(
            pending.prepare_pipeline_subject_v1(&mut ledger),
            Err(ProductionSessionErrorV1::AnalysisResourceLimit {
                phase: Phase::PipelineVerification,
                resource: "peak storage upper bound",
                ..
            })
        ));
        assert_eq!(ledger.cumulative(), prefix);
        assert_eq!(pending.analysis._analyses.resource_upper_bound(), floor);
    }

    #[test]
    fn subject_rejects_replacement_recipe_and_roster_without_trusting_metadata() {
        for replacement in 0..3 {
            let mut pending = pending();
            let mut resources = resources(
                &pending,
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            );
            let record = pending
                ._session
                .constructed_roots
                .get_mut(&pending._stage.identity)
                .unwrap();
            if replacement == 2 {
                record.ranked_kernel = Some(recipe(false, true).0);
            } else {
                record.ownership_occurrences[replacement].operation =
                    record.ranked_function.unwrap();
            }
            assert!(matches!(
                pending.prepare_pipeline_subject_v1(&mut resources),
                Err(ProductionSessionErrorV1::RankedGraphChanged)
            ));
        }
    }
}

#[test]
fn frozen_pending_recipe_borrows_preserve_actual_custody() {
    let prepare = || {
        let mut session = session();
        let (stage, root, sites) = construct(&mut session, "recipe_borrow", false, true);
        session
            .prepare_conditional_ranked_analysis_v1(stage, root, &sites[..1])
            .unwrap()
    };
    let first = prepare();
    let second = prepare();
    assert_eq!(first.kernel().unwrap(), second.kernel().unwrap());
    assert!(std::ptr::eq(
        first.kernel().unwrap(),
        first.kernel().unwrap()
    ));
    assert!(!std::ptr::eq(
        first.kernel().unwrap(),
        second.kernel().unwrap()
    ));
    assert!(!first.pending_pipeline_checks().is_empty());
}

#[test]
fn pending_owner_keeps_every_row_without_coverage_credit() {
    let mut session = session();
    let (stage, root, sites) = construct(&mut session, "pending", false, true);
    let pending = session
        .prepare_conditional_ranked_analysis_v1(stage, root, &sites[..1])
        .unwrap();
    assert_eq!(pending.rows().len(), 2);
    assert!(pending.rows()[0].selected());
    assert!(!pending.rows()[1].selected());
    assert_eq!(
        pending.rows()[0].coverage(),
        Check::Blocked(Blocker::CanonicalCoverageAndSourceReplay)
    );
    assert!(pending.rows()[0].regions().is_empty());
    assert!(!pending.legacy_report().is_clean());
    assert_eq!(
        pending
            .legacy_report()
            .coverage_summary()
            .total_view_declared(),
        2
    );
    assert_eq!(
        pending
            .legacy_report()
            .coverage_summary()
            .total_view_proved(),
        0
    );
    assert_eq!(pending.selections(), &sites[..1]);
    assert_eq!(
        pending.pending_pipeline_checks(),
        &crate::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
    );
}

#[test]
fn actual_emitted_contract_not_recipe_ordinal_survives_expansion() {
    let mut session = session();
    let (stage, root, sites) = construct(&mut session, "expanded", true, true);
    let pending = session
        .prepare_conditional_ranked_analysis_v1(stage, root, &sites[1..])
        .unwrap();
    assert!(!pending.rows()[0].selected());
    assert!(pending.rows()[1].selected());
    assert!(pending.rows()[1].location().operation() > sites[1].operation as usize);
    let record = pending
        ._session
        .constructed_roots
        .get(&pending._stage.identity)
        .unwrap();
    assert_eq!(
        record.ownership_occurrences[1].operation,
        pending.rows()[1].operation()
    );
    assert_eq!(
        record.ownership_occurrences[1].view,
        pending.rows()[1].view()
    );
}

#[test]
fn selections_refuse_empty_missing_noncontract_wrong_view_and_duplicate() {
    for case in 0..5 {
        let mut session = session();
        let (stage, root, sites) = construct(&mut session, "selection", false, true);
        let requests = match case {
            0 => vec![],
            1 => vec![Site {
                operation: u32::MAX,
                ..sites[0]
            }],
            2 => vec![Site {
                operation: 0,
                ..sites[0]
            }],
            3 => vec![Site {
                view: sites[1].view,
                ..sites[0]
            }],
            _ => vec![sites[0], sites[0]],
        };
        assert!(matches!(
            session.prepare_conditional_ranked_analysis_v1(stage, root, &requests),
            Err(ProductionSessionErrorV1::ConditionalOwnership(
                ProductionConditionalOwnershipErrorV1::Selection { .. }
            ))
        ));
    }
}

#[test]
fn foreign_and_cross_root_handles_refuse() {
    for foreign_stage in [false, true] {
        let mut first = session();
        let mut second = session();
        let (a_stage, a_root, sites) = construct(&mut first, "first", false, true);
        let (b_stage, b_root, _) = construct(&mut second, "second", false, true);
        let (stage, root) = if foreign_stage {
            (b_stage, a_root)
        } else {
            (a_stage, b_root)
        };
        assert!(matches!(
            first.prepare_conditional_ranked_analysis_v1(stage, root, &sites),
            Err(ProductionSessionErrorV1::ForeignSession)
        ));
    }
    let mut session = session();
    let (stage, _, sites) = construct(&mut session, "first", false, true);
    let (_, root, _) = construct(&mut session, "second", false, true);
    assert!(matches!(
        session.prepare_conditional_ranked_analysis_v1(stage, root, &sites),
        Err(ProductionSessionErrorV1::StageRootMismatch)
    ));
}

#[test]
fn stale_stage_and_binding_substitution_refuse() {
    for stale in [false, true] {
        let mut session = session();
        let (stage, root, sites) = construct(&mut session, "changed", false, true);
        if stale {
            session.constructed_roots.remove(&stage.identity);
        } else {
            let record = session.constructed_roots.get_mut(&stage.identity).unwrap();
            record.ownership_occurrences[0].operation = record.ownership_occurrences[1].operation;
        }
        let error = session
            .prepare_conditional_ranked_analysis_v1(stage, root, &sites[..1])
            .err()
            .unwrap();
        assert!(
            matches!(
                error,
                ProductionSessionErrorV1::StaleStage
                    | ProductionSessionErrorV1::ConditionalOwnership(
                        ProductionConditionalOwnershipErrorV1::Selection {
                            reason: "wrong live view",
                            ..
                        }
                    )
            ),
            "unexpected stale binding refusal: {error:?}"
        );
    }
}

#[test]
fn static_success_and_unrelated_alias_failure_keep_the_full_legacy_report() {
    use ProductionRankedOperationV1 as O;
    for aliased in [false, true] {
        let mut session = session();
        let (kernel, sites) = recipe(false, true);
        let blocks = kernel
            .blocks()
            .iter()
            .map(|block| {
                let operations = block
                    .operations()
                    .iter()
                    .cloned()
                    .map(|mut operation| {
                        match &mut operation {
                            O::ExecutionLayout { global_extents, .. } => global_extents[0] = 4,
                            O::InvocationIndex { launch_extent, .. } => *launch_extent = 4,
                            O::View {
                                shape,
                                dynamic_extents,
                                noalias_class,
                                ..
                            }
                            | O::ViewInSpace {
                                shape,
                                dynamic_extents,
                                noalias_class,
                                ..
                            } => {
                                shape[0] = 4;
                                dynamic_extents.clear();
                                if aliased {
                                    *noalias_class = 1;
                                }
                            }
                            _ => {}
                        }
                        operation
                    })
                    .collect();
                ProductionRankedBlockV1::new(operations, block.terminator().clone())
            })
            .collect();
        let kernel = ProductionRankedKernelV1::new("legacy", 1, blocks).unwrap();
        let static_views = kernel.blocks().iter().flat_map(|block| block.operations()).filter(|operation| matches!(operation,
            O::ViewInSpace { shape, dynamic_extents, .. } if shape == &[4] && dynamic_extents.is_empty()
        )).count();
        assert_eq!(static_views, 2, "fixture must exercise two static outputs");
        let registered = session
            .register_construction(
                ProductionConstructionV1::ranked_kernel("legacy", kernel).unwrap(),
            )
            .unwrap();
        let (stage, root) = session.construct_registered(registered).unwrap();
        let function = FuncOp::from_operation(
            session.constructed_roots[&stage.identity]
                .ranked_function
                .unwrap(),
        );
        let before =
            crate::run_pliron_hierarchical_ownership_check_v1(&session.inner.context, &function);
        assert_eq!(before.is_clean(), !aliased, "aliased={aliased}: {before:?}");
        if aliased {
            assert!(before.findings().iter().any(|finding| matches!(
                finding,
                crate::HierarchicalOwnershipFindingV1::MayAliasObservableWrite { .. }
            )));
        } else {
            assert_eq!(before.coverage_summary().total_view_proved(), 2);
        }
        let pending = session
            .prepare_conditional_ranked_analysis_v1(stage, root, &sites[..1])
            .unwrap();
        assert_eq!(pending.legacy_report(), &before);
        assert_eq!(pending.rows().len(), 2);
        assert!(!pending.rows()[1].selected());
        assert_eq!(pending.pending_pipeline_checks().len(), 9);
    }
}

#[test]
fn identical_replacement_of_selected_or_unselected_contract_refuses() {
    use dialect_kernel::OwnershipContractOp;
    for index in 0..2 {
        let mut session = session();
        let (stage, root, sites) = construct(&mut session, "replaced", false, true);
        let record = session.constructed_roots.get(&stage.identity).unwrap();
        let stale = record.ownership_occurrences[index].operation;
        let view = record.ownership_occurrences[index].view;
        let context = &mut session.inner.context;
        let replacement = OwnershipContractOp::new(
            context,
            view,
            OwnershipCoverageAttr::TotalView,
            OwnershipPartitionAttr::ExactSets,
        )
        .unwrap();
        replacement.get_operation().insert_before(context, stale);
        Operation::erase(stale, context);
        let error = session
            .prepare_conditional_ranked_analysis_v1(stage, root, &sites[..1])
            .err()
            .unwrap();
        assert!(
            matches!(
                error,
                ProductionSessionErrorV1::RankedGraphChanged
                    | ProductionSessionErrorV1::Operation(
                        OperationHandleError::OperationGraphChangedOutsideTransaction
                    )
                    | ProductionSessionErrorV1::ConditionalOwnership(
                        ProductionConditionalOwnershipErrorV1::Selection {
                            reason: "missing live operation",
                            ..
                        }
                    )
            ),
            "unexpected replacement refusal: {error:?}"
        );
    }
}

#[test]
fn mandatory_bounds_failure_is_not_masked_by_legacy_trace_exit() {
    let mut session = session();
    let (stage, root, sites) = construct(&mut session, "dirty", false, false);
    let pending = session
        .prepare_conditional_ranked_analysis_v1(stage, root, &sites[..1])
        .unwrap();
    assert!(pending.mandatory_bounds_failure().is_some());
    assert_eq!(pending.prerequisites(), Check::Rejected);
    assert_eq!(pending.rows().len(), 2);
    assert!(!pending.legacy_report().is_clean());
}

#[test]
fn analysis_panic_is_contained_and_consumes_the_session() {
    let mut session = session();
    let (stage, root, sites) = construct(&mut session, "panic", false, true);
    crate::production_analysis::panic_next_analysis_manager_prepare_for_test_v1();
    assert!(matches!(
        session.prepare_conditional_ranked_analysis_v1(stage, root, &sites),
        Err(ProductionSessionErrorV1::Operation(
            OperationHandleError::UpstreamPanicked
        ))
    ));
}

#[test]
fn pending_rehash_cannot_restart_the_inherited_work_budget() {
    let run = |one_short: bool| {
        let mut session = session();
        let _prior = construct(&mut session, "prior", false, true);
        let (stage, root, sites) = construct(&mut session, "pending", false, true);
        let inherited = session.ownership_binding_resources.work_upper_bound();
        let replacement = recipe(true, true).0;
        let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        let mut calibration = ProductionAnalysisResourceContractV1::new(limits);
        middle_end_evidence_v4::derive_exact_ranked_graph_identity_with_resources_v1(
            &replacement,
            &mut calibration,
        )
        .unwrap();
        let hash_work = calibration.cumulative().work_upper_bound();
        assert!(inherited > 0 && hash_work > 0);
        // Keep the genuine live graph and recorded identity. A completed hash
        // must find this mismatch before any later analysis can spend work.
        session
            .constructed_roots
            .get_mut(&stage.identity)
            .unwrap()
            .ranked_kernel = Some(replacement);
        session.analysis_resource_limits = ProductionAnalysisResourceLimitsV1::new(
            inherited + hash_work - usize::from(one_short),
            usize::MAX,
        );
        session.prepare_conditional_ranked_analysis_v1(stage, root, &sites[..1])
    };
    assert!(matches!(
        run(true),
        Err(ProductionSessionErrorV1::AnalysisResourceLimit {
            phase: Phase::StructuralIdentity,
            ..
        })
    ));
    assert!(matches!(
        run(false),
        Err(ProductionSessionErrorV1::RankedGraphChanged)
    ));
}

#[test]
fn inherited_capture_and_exact_pending_resource_limits() {
    let run = |limits: Option<ProductionAnalysisResourceLimitsV1>| {
        let mut session = session();
        let (first, _, _) = construct(&mut session, "prior", false, true);
        let prefix = session.ownership_binding_resources;
        session.constructed_roots.remove(&first.identity);
        assert_eq!(session.ownership_binding_resources, prefix);
        let (stage, root, sites) = construct(&mut session, "pending", true, true);
        assert!(session.ownership_binding_resources.work_upper_bound() > prefix.work_upper_bound());
        assert!(
            session
                .ownership_binding_resources
                .retained_storage_upper_bound()
                > prefix.retained_storage_upper_bound()
        );
        if let Some(limits) = limits {
            session.analysis_resource_limits = limits;
        }
        session.prepare_conditional_ranked_analysis_v1(stage, root, &sites[..1])
    };
    let generous = run(None).unwrap();
    let used = generous.analysis._analyses.resource_upper_bound();
    drop(generous);
    assert!(
        run(Some(ProductionAnalysisResourceLimitsV1::new(
            used.work_upper_bound(),
            used.peak_storage_upper_bound()
        )))
        .is_ok()
    );
    for limits in [
        ProductionAnalysisResourceLimitsV1::new(
            used.work_upper_bound() - 1,
            used.peak_storage_upper_bound(),
        ),
        ProductionAnalysisResourceLimitsV1::new(
            used.work_upper_bound(),
            used.peak_storage_upper_bound() - 1,
        ),
    ] {
        assert!(matches!(
            run(Some(limits)),
            Err(ProductionSessionErrorV1::AnalysisResourceLimit { .. })
        ));
    }
}
