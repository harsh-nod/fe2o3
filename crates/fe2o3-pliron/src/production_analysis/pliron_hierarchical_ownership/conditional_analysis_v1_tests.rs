use super::*;
use crate::production_analysis::{
    pliron_ir_identity::LivePlironStructuralIdentityProviderV1,
    pliron_pass_contract::PlironStructuralIdentityProviderV1,
};
use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
use dialect_kernel::{
    BranchOp, DimensionOp, IndexLessThanBranchOp, IndexType, InvocationIndexOp, RankedViewType,
    ReturnOp,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, types::FunctionType},
    dialect::DialectName,
    operation::{Operation, verify_operation},
    r#type::TypeHandle,
};

type Check = ConditionalOwnershipCheckV1;
type Blocker = ConditionalOwnershipBlockerV1;
type SelectionError = ConditionalOwnershipSelectionErrorV1;

#[derive(Clone, Copy)]
struct FixtureOptions {
    dynamic: bool,
    guarded: bool,
    other: Option<(OwnershipCoverageAttr, usize, u64)>,
    duplicate: bool,
    partition: OwnershipPartitionAttr,
}

impl Default for FixtureOptions {
    fn default() -> Self {
        Self {
            dynamic: true,
            guarded: true,
            other: None,
            duplicate: false,
            partition: OwnershipPartitionAttr::ExactSets,
        }
    }
}

struct Fixture {
    function: FuncOp,
    contract: OwnershipContractOp,
    output: RankedViewOp,
    invocation: InvocationIndexOp,
    other: Option<(OwnershipContractOp, RankedViewOp)>,
}

impl Fixture {
    fn selection<'ctx>(&self, context: &'ctx Context) -> ConditionalOwnershipSelectionV1<'ctx> {
        ConditionalOwnershipSelectionV1::new(
            context,
            &self.function,
            self.contract.get_operation(),
            self.output.result(context),
        )
    }
}

fn setup() -> Context {
    let mut context = Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    context
}

fn block(context: &mut Context, function: &FuncOp, name: &str) -> Ptr<BasicBlock> {
    let block = BasicBlock::new(context, Some(name.try_into().unwrap()), vec![]);
    block.insert_at_back(function.get_region(context), context);
    block
}

fn fixture(context: &mut Context, options: FixtureOptions) -> Fixture {
    let index: TypeHandle = IndexType::get(context).into();
    let function = FuncOp::new(
        context,
        "conditional_ownership".try_into().unwrap(),
        FunctionType::get(context, vec![index], vec![]),
    );
    let entry = function.get_entry_block(context);
    let argument = entry.deref(context).arguments().next().unwrap();
    let write = block(context, &function, "write");
    let other_blocks = options.other.map(|_| {
        (
            block(context, &function, "other_guard"),
            block(context, &function, "other_write"),
        )
    });
    let exit = block(context, &function, "exit");
    let extent = if options.dynamic { DYNAMIC_EXTENT } else { 4 };
    let layout = ExecutionLayoutOp::new_with_domain(
        context,
        41,
        [extent, 1, 1],
        [2, 1, 1],
        2,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    );
    let invocation = InvocationIndexOp::new(context, 0, extent);
    let make_view = |context: &mut Context, origin, noalias| {
        let ty = RankedViewType::new(context, 32, true, vec![extent]).unwrap();
        RankedViewOp::new_in_space_with_allocation_contract(
            context,
            ty,
            if options.dynamic {
                vec![argument]
            } else {
                vec![]
            },
            MemorySpaceAttr::Global,
            origin,
            noalias,
        )
        .unwrap()
    };
    let output = make_view(context, 17, 17);
    let contract = OwnershipContractOp::new(
        context,
        output.result(context),
        OwnershipCoverageAttr::TotalView,
        options.partition,
    )
    .unwrap();
    let dimension = DimensionOp::new(context, output.result(context), 0).unwrap();
    for operation in [
        layout.get_operation(),
        invocation.get_operation(),
        output.get_operation(),
        contract.get_operation(),
        dimension.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    if options.duplicate {
        OwnershipContractOp::new(
            context,
            output.result(context),
            OwnershipCoverageAttr::TotalView,
            OwnershipPartitionAttr::ExactSets,
        )
        .unwrap()
        .get_operation()
        .insert_at_back(entry, context);
    }
    let other = options.other.map(|(coverage, writes, noalias)| {
        let view = make_view(context, 23, noalias);
        let contract = OwnershipContractOp::new(
            context,
            view.result(context),
            coverage,
            OwnershipPartitionAttr::ExactSets,
        )
        .unwrap();
        let dimension = DimensionOp::new(context, view.result(context), 0).unwrap();
        for operation in [
            view.get_operation(),
            contract.get_operation(),
            dimension.get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }
        let (guard, body) = other_blocks.unwrap();
        IndexLessThanBranchOp::new(
            context,
            invocation.result(context),
            dimension.result(context),
            body,
            exit,
        )
        .get_operation()
        .insert_at_back(guard, context);
        for _ in 0..writes {
            RankedAccessOp::new(
                context,
                AccessKindAttr::Write,
                view.result(context),
                vec![invocation.result(context)],
            )
            .unwrap()
            .get_operation()
            .insert_at_back(body, context);
        }
        BranchOp::new(context, exit)
            .get_operation()
            .insert_at_back(body, context);
        (contract, view)
    });
    if options.guarded {
        IndexLessThanBranchOp::new(
            context,
            invocation.result(context),
            dimension.result(context),
            write,
            exit,
        )
        .get_operation()
        .insert_at_back(entry, context);
    } else {
        BranchOp::new(context, write)
            .get_operation()
            .insert_at_back(entry, context);
    }
    RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        output.result(context),
        vec![invocation.result(context)],
    )
    .unwrap()
    .get_operation()
    .insert_at_back(write, context);
    BranchOp::new(context, other_blocks.map_or(exit, |(guard, _)| guard))
        .get_operation()
        .insert_at_back(write, context);
    ReturnOp::new(context)
        .get_operation()
        .insert_at_back(exit, context);
    verify_operation(function.get_operation(), context).unwrap();
    Fixture {
        function,
        contract,
        output,
        invocation,
        other,
    }
}

fn unlimited() -> ProductionAnalysisResourceLimitsV1 {
    ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX)
}

fn manager(
    context: &Context,
    function: &FuncOp,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<PlironAnalysisManagerV1, ProductionAnalysisResourceLimitV1> {
    let mut provider = LivePlironStructuralIdentityProviderV1::new(context, function);
    let capture = provider
        .capture_with_resource_limits_v1(unlimited())
        .ok()
        .unwrap();
    let inherited = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::StructuralIdentity,
        17,
        11,
        0,
    )
    .unwrap();
    let initial = capture
        .resource_upper_bound
        .checked_then_retain(
            inherited,
            ProductionAnalysisResourcePhaseV1::StructuralIdentity,
        )
        .unwrap();
    PlironAnalysisManagerV1::new_with_resource_contract(
        function,
        capture.input_census,
        initial,
        capture.resource_upper_bound.retained_storage_upper_bound(),
        limits,
    )
}

fn analyze<'ctx>(
    context: &'ctx Context,
    fixture: &Fixture,
) -> ConditionalOwnershipAnalysisV1<'ctx> {
    let mut manager = manager(context, &fixture.function, unlimited()).unwrap();
    run_conditional_ownership_analysis_v1(
        context,
        &fixture.function,
        &mut manager,
        &[fixture.selection(context)],
    )
    .unwrap()
}

fn legacy(context: &Context, fixture: &Fixture) -> HierarchicalOwnershipReportV1 {
    run_pliron_hierarchical_ownership_check_v1(context, &fixture.function)
}

#[test]
fn v1_static_report_and_counters_survive_pending_analysis() {
    let context = &mut setup();
    let fixture = fixture(
        context,
        FixtureOptions {
            dynamic: false,
            ..FixtureOptions::default()
        },
    );
    let before = legacy(context, &fixture);
    assert!(before.is_clean());
    assert_eq!(
        before.coverage_summary(),
        HierarchicalCoverageProofSummaryV1 {
            total_view_declared: 1,
            total_view_proved: 1,
            collective_contributions_declared: 0,
            collective_contributions_proved: 0,
        }
    );
    assert_eq!(before.regions().len(), 9); // Four invocations, two subgroups/workgroups, one grid.
    let pending = analyze(context, &fixture);
    assert_eq!(pending.legacy_report(), &before);
    assert_eq!(
        pending.rows()[0].coverage(),
        Check::Blocked(Blocker::UnsupportedSelectionProfile)
    );
    assert_eq!(legacy(context, &fixture), before);
}

#[test]
fn v1_dynamic_trace_exit_is_preserved_without_coverage_credit() {
    let context = &mut setup();
    let fixture = fixture(context, FixtureOptions::default());
    let expected = one_with_summary(
        HierarchicalOwnershipFindingV1::TraceIncomplete {
            detail: trace_failure_detail(crate::production_analysis::pliron_invocation_trace::PlironTraceFailureV1::DynamicLaunch { dimension: 0 }),
        },
        HierarchicalCoverageProofSummaryV1 {
            total_view_declared: 1, total_view_proved: 0,
            collective_contributions_declared: 0, collective_contributions_proved: 0,
        },
    );
    assert_eq!(legacy(context, &fixture), expected);
    let pending = analyze(context, &fixture);
    assert_eq!(pending.legacy_report(), &expected);
    assert_eq!(pending.prerequisites(), Check::Checked);
    assert_eq!(pending.rows().len(), 1);
    let row = &pending.rows()[0];
    assert_eq!(row.operation(), fixture.contract.get_operation());
    assert_eq!(row.view(), fixture.output.result(context));
    assert!(row.selected());
    assert_eq!(row.trace(), Check::Blocked(Blocker::Trace));
    assert_eq!(row.extent(), Check::Blocked(Blocker::Extent));
    assert_eq!(
        row.coverage(),
        Check::Blocked(Blocker::CanonicalCoverageAndSourceReplay)
    );
    assert!(row.regions().is_empty());
    assert_eq!(legacy(context, &fixture), expected);
}

#[test]
fn dirty_bounds_remain_visible_even_when_legacy_trace_exit_masks_them() {
    let context = &mut setup();
    let fixture = fixture(
        context,
        FixtureOptions {
            guarded: false,
            ..FixtureOptions::default()
        },
    );
    let before = legacy(context, &fixture);
    assert!(matches!(
        before.findings(),
        [HierarchicalOwnershipFindingV1::TraceIncomplete { .. }]
    ));
    assert_eq!(before.coverage_summary().total_view_proved(), 0);
    let pending = analyze(context, &fixture);
    assert_eq!(pending.legacy_report(), &before);
    assert_eq!(pending.prerequisites(), Check::Rejected);
    assert!(
        pending
            .mandatory_bounds_failure()
            .unwrap()
            .starts_with("mandatory ranked bounds failed: ")
    );
    assert_eq!(
        pending.rows()[0].coverage(),
        Check::Blocked(Blocker::Prerequisite)
    );
    assert_eq!(legacy(context, &fixture), before);
}

#[test]
fn alias_and_duplicate_contract_golden_failures_keep_precedence() {
    let context = &mut setup();
    let aliased = fixture(
        context,
        FixtureOptions {
            other: Some((OwnershipCoverageAttr::ExactView, 1, 17)),
            ..FixtureOptions::default()
        },
    );
    let before = legacy(context, &aliased);
    assert!(matches!(
        before.findings(),
        [HierarchicalOwnershipFindingV1::MayAliasObservableWrite {
            contracted_noalias_class: 17,
            alias_noalias_class: 17,
            ..
        }]
    ));
    let pending = analyze(context, &aliased);
    assert_eq!(pending.legacy_report(), &before);
    assert_eq!(pending.prerequisites(), Check::Rejected);
    assert!(
        pending
            .rows()
            .iter()
            .all(|row| row.coverage() == Check::Blocked(Blocker::Prerequisite))
    );
    assert_eq!(legacy(context, &aliased), before);

    let duplicate = fixture(
        context,
        FixtureOptions {
            duplicate: true,
            ..FixtureOptions::default()
        },
    );
    let before = legacy(context, &duplicate);
    assert!(matches!(
        before.findings(),
        [HierarchicalOwnershipFindingV1::DuplicateContract { .. }]
    ));
    assert_eq!(
        before.coverage_summary(),
        HierarchicalCoverageProofSummaryV1::default()
    );
    let mut manager = manager(context, &duplicate.function, unlimited()).unwrap();
    let error = run_conditional_ownership_analysis_v1(
        context,
        &duplicate.function,
        &mut manager,
        &[duplicate.selection(context)],
    )
    .unwrap_err();
    assert!(
        matches!(error, ConditionalOwnershipAnalysisErrorV1::Contracts(report) if report == before)
    );
    assert_eq!(legacy(context, &duplicate), before);
}

#[test]
fn residual_effect_domain_site_count_runs_without_a_whole_function_trace() {
    for writes in [0, 1, 2] {
        let context = &mut setup();
        let fixture = fixture(
            context,
            FixtureOptions {
                other: Some((OwnershipCoverageAttr::ExactEffectDomain, writes, 23)),
                ..FixtureOptions::default()
            },
        );
        let before = legacy(context, &fixture);
        let pending = analyze(context, &fixture);
        assert_eq!(pending.legacy_report(), &before);
        assert_eq!(pending.rows().len(), 2);
        let other = &pending.rows()[1];
        assert!(!other.selected());
        assert_eq!(other.operation(), fixture.other.unwrap().0.get_operation());
        assert_eq!(other.trace(), Check::NotApplicable);
        if writes == 1 {
            assert_eq!(other.effect_site(), Check::Checked);
        } else {
            assert_eq!(other.effect_site(), Check::Rejected);
            assert!(
                matches!(other.findings(), [HierarchicalOwnershipFindingV1::MalformedContract { detail, .. }]
                if detail.contains("exactly one"))
            );
        }
        assert_eq!(pending.rows()[0].trace(), Check::Blocked(Blocker::Trace));
        assert_eq!(before.coverage_summary().total_view_proved(), 0);
    }
}

#[test]
fn unselected_exact_and_collective_contracts_stay_blocked() {
    for coverage in [
        OwnershipCoverageAttr::ExactView,
        OwnershipCoverageAttr::TotalView,
        OwnershipCoverageAttr::CollectiveContributions,
    ] {
        let context = &mut setup();
        let fixture = fixture(
            context,
            FixtureOptions {
                other: Some((coverage, 0, 23)),
                ..FixtureOptions::default()
            },
        );
        let pending = analyze(context, &fixture);
        assert_eq!(pending.legacy_report(), &legacy(context, &fixture));
        let other = &pending.rows()[1];
        assert!(!other.selected());
        assert_eq!(other.trace(), Check::Blocked(Blocker::Trace));
        assert_eq!(other.extent(), Check::Blocked(Blocker::Extent));
        assert!(matches!(other.coverage(), Check::Blocked(_)));
        assert_eq!(
            pending
                .legacy_report()
                .coverage_summary()
                .total_view_proved(),
            0
        );
        assert_eq!(
            pending
                .legacy_report()
                .coverage_summary()
                .collective_contributions_proved(),
            0
        );
    }
}

#[test]
fn selected_dense_partition_is_not_promoted_to_conditional_coverage() {
    let context = &mut setup();
    let fixture = fixture(
        context,
        FixtureOptions {
            partition: OwnershipPartitionAttr::DenseRectangles,
            ..FixtureOptions::default()
        },
    );
    let pending = analyze(context, &fixture);
    assert_eq!(
        pending.rows()[0].coverage(),
        Check::Blocked(Blocker::UnsupportedSelectionProfile)
    );
    assert_eq!(pending.legacy_report(), &legacy(context, &fixture));
}

#[test]
fn available_unselected_finite_analysis_still_runs() {
    let context = &mut setup();
    let fixture = fixture(
        context,
        FixtureOptions {
            dynamic: false,
            other: Some((OwnershipCoverageAttr::ExactView, 1, 23)),
            ..FixtureOptions::default()
        },
    );
    let before = legacy(context, &fixture);
    assert!(before.is_clean());
    let pending = analyze(context, &fixture);
    assert_eq!(pending.legacy_report(), &before);
    assert_eq!(
        pending.rows()[0].coverage(),
        Check::Blocked(Blocker::UnsupportedSelectionProfile)
    );
    let other = &pending.rows()[1];
    assert!(!other.selected());
    assert_eq!(other.extent(), Check::Checked);
    assert_eq!(other.trace(), Check::Checked);
    assert_eq!(other.coverage(), Check::Checked);
    assert_eq!(other.regions().len(), 9);
    assert!(other.findings().is_empty());
}

fn expect_selection_error(
    context: &Context,
    fixture: &Fixture,
    selections: &[ConditionalOwnershipSelectionV1<'_>],
    expected_index: usize,
    expected: SelectionError,
) {
    let mut manager = manager(context, &fixture.function, unlimited()).unwrap();
    let before = manager.resource_upper_bound();
    let error =
        run_conditional_ownership_analysis_v1(context, &fixture.function, &mut manager, selections)
            .unwrap_err();
    assert!(
        matches!(error, ConditionalOwnershipAnalysisErrorV1::Selection { index, reason }
        if index == expected_index && reason == expected)
    );
    assert!(manager.resource_upper_bound().work_upper_bound() >= before.work_upper_bound());
    assert!(
        manager
            .resource_upper_bound()
            .retained_storage_upper_bound()
            >= before.retained_storage_upper_bound()
    );
}

#[test]
fn missing_duplicate_wrong_view_and_noncontract_selections_refuse() {
    let context = &mut setup();
    let fixture = fixture(
        context,
        FixtureOptions {
            other: Some((OwnershipCoverageAttr::ExactView, 0, 23)),
            ..FixtureOptions::default()
        },
    );
    let selection = fixture.selection(context);
    expect_selection_error(context, &fixture, &[], 0, SelectionError::Empty);
    expect_selection_error(
        context,
        &fixture,
        &[selection, selection],
        1,
        SelectionError::Duplicate,
    );
    let wrong_view = ConditionalOwnershipSelectionV1 {
        view: fixture.other.unwrap().1.result(context),
        ..selection
    };
    expect_selection_error(
        context,
        &fixture,
        &[wrong_view],
        0,
        SelectionError::WrongView,
    );
    let noncontract = ConditionalOwnershipSelectionV1 {
        operation: fixture.invocation.get_operation(),
        ..selection
    };
    expect_selection_error(
        context,
        &fixture,
        &[noncontract],
        0,
        SelectionError::NotContract,
    );
    let missing = ConditionalOwnershipSelectionV1 {
        operation: fixture.function.get_operation(),
        ..selection
    };
    expect_selection_error(
        context,
        &fixture,
        &[missing],
        0,
        SelectionError::MissingOperation,
    );
    let too_many = vec![selection; MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1 + 1];
    expect_selection_error(
        context,
        &fixture,
        &too_many,
        MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1,
        SelectionError::Limit,
    );
}

#[test]
fn cross_function_and_cross_context_selections_are_never_dereferenced() {
    let context = &mut setup();
    let first = fixture(context, FixtureOptions::default());
    let second = fixture(context, FixtureOptions::default());
    expect_selection_error(
        context,
        &first,
        &[second.selection(context)],
        0,
        SelectionError::Owner,
    );
    let other_context = &mut setup();
    let other = fixture(other_context, FixtureOptions::default());
    expect_selection_error(
        context,
        &first,
        &[other.selection(other_context)],
        0,
        SelectionError::Owner,
    );
    let wrong_context_only = ConditionalOwnershipSelectionV1 {
        context: other_context,
        ..first.selection(context)
    };
    expect_selection_error(
        context,
        &first,
        &[wrong_context_only],
        0,
        SelectionError::Owner,
    );
    let forged_function = ConditionalOwnershipSelectionV1 {
        function: first.function.get_operation(),
        ..second.selection(context)
    };
    expect_selection_error(
        context,
        &first,
        &[forged_function],
        0,
        SelectionError::MissingOperation,
    );
}

#[test]
fn replaced_contract_selection_is_stale_even_with_the_same_view_and_attributes() {
    let context = &mut setup();
    let fixture = fixture(context, FixtureOptions::default());
    let replacement = OwnershipContractOp::new(
        context,
        fixture.output.result(context),
        OwnershipCoverageAttr::TotalView,
        OwnershipPartitionAttr::ExactSets,
    )
    .unwrap();
    let stale_operation = fixture.contract.get_operation();
    replacement
        .get_operation()
        .insert_before(context, stale_operation);
    // Erasure removes the old operand use; retain only its raw identity.
    Operation::erase(stale_operation, context);
    verify_operation(fixture.function.get_operation(), context).unwrap();
    let stale = ConditionalOwnershipSelectionV1::new(
        context,
        &fixture.function,
        stale_operation,
        fixture.output.result(context),
    );
    // A fresh manager observes the current graph. Reusing a manager after
    // mutation is outside its existing immutable-function contract.
    expect_selection_error(
        context,
        &fixture,
        &[stale],
        0,
        SelectionError::MissingOperation,
    );
    let current = ConditionalOwnershipSelectionV1::new(
        context,
        &fixture.function,
        replacement.get_operation(),
        fixture.output.result(context),
    );
    let mut manager = manager(context, &fixture.function, unlimited()).unwrap();
    let pending =
        run_conditional_ownership_analysis_v1(context, &fixture.function, &mut manager, &[current])
            .unwrap();
    assert_eq!(pending.rows()[0].operation(), replacement.get_operation());
    assert_eq!(
        pending.rows()[0].coverage(),
        Check::Blocked(Blocker::CanonicalCoverageAndSourceReplay)
    );
}

#[test]
fn caller_manager_budget_is_inherited_and_full_entry_has_exact_boundaries() {
    let context = &mut setup();
    let fixture = fixture(context, FixtureOptions::default());
    let selection = fixture.selection(context);
    let mut generous = manager(context, &fixture.function, unlimited()).unwrap();
    let initial = generous.resource_upper_bound();
    assert!(initial.work_upper_bound() >= 17);
    assert!(initial.retained_storage_upper_bound() >= 11);
    let report = run_conditional_ownership_analysis_v1(
        context,
        &fixture.function,
        &mut generous,
        &[selection],
    )
    .unwrap();
    let exact = generous.resource_upper_bound();
    assert!(exact.work_upper_bound() > initial.work_upper_bound());
    assert!(exact.retained_storage_upper_bound() > initial.retained_storage_upper_bound());
    let mut bounded = manager(
        context,
        &fixture.function,
        ProductionAnalysisResourceLimitsV1::new(
            exact.work_upper_bound(),
            exact.peak_storage_upper_bound(),
        ),
    )
    .unwrap();
    let rerun = run_conditional_ownership_analysis_v1(
        context,
        &fixture.function,
        &mut bounded,
        &[selection],
    )
    .unwrap();
    assert_eq!(format!("{rerun:?}"), format!("{report:?}"));
    assert_eq!(bounded.resource_upper_bound(), exact);
    for limits in [
        ProductionAnalysisResourceLimitsV1::new(
            exact.work_upper_bound() - 1,
            exact.peak_storage_upper_bound(),
        ),
        ProductionAnalysisResourceLimitsV1::new(
            exact.work_upper_bound(),
            exact.peak_storage_upper_bound() - 1,
        ),
    ] {
        let mut bounded = match manager(context, &fixture.function, limits) {
            Ok(manager) => manager,
            Err(error) => {
                // The inherited identity capture may already set the peak.
                assert_eq!(
                    error,
                    ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::StructuralIdentity,
                        resource: "peak storage upper bound",
                    }
                );
                continue;
            }
        };
        let error = run_conditional_ownership_analysis_v1(
            context,
            &fixture.function,
            &mut bounded,
            &[selection],
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ConditionalOwnershipAnalysisErrorV1::Resource(_)
        ));
        assert!(bounded.resource_upper_bound().work_upper_bound() >= initial.work_upper_bound());
        assert!(
            bounded
                .resource_upper_bound()
                .retained_storage_upper_bound()
                >= initial.retained_storage_upper_bound()
        );
    }
    let before_repeat = generous.resource_upper_bound();
    run_conditional_ownership_analysis_v1(context, &fixture.function, &mut generous, &[selection])
        .unwrap();
    assert!(generous.resource_upper_bound().work_upper_bound() > before_repeat.work_upper_bound());
    assert!(
        generous
            .resource_upper_bound()
            .retained_storage_upper_bound()
            > before_repeat.retained_storage_upper_bound()
    );
}

#[test]
fn additional_inherited_work_and_retention_survive_as_exact_deltas() {
    let context = &mut setup();
    let fixture = fixture(context, FixtureOptions::default());
    let selection = fixture.selection(context);
    let mut baseline = manager(context, &fixture.function, unlimited()).unwrap();
    let mut inherited = manager(context, &fixture.function, unlimited()).unwrap();
    let extra = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
        23,
        29,
        0,
    )
    .unwrap();
    inherited
        .admit_retained_resource_upper_bound(
            ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
            extra,
        )
        .unwrap();
    let baseline_report = run_conditional_ownership_analysis_v1(
        context,
        &fixture.function,
        &mut baseline,
        &[selection],
    )
    .unwrap();
    let inherited_report = run_conditional_ownership_analysis_v1(
        context,
        &fixture.function,
        &mut inherited,
        &[selection],
    )
    .unwrap();
    assert_eq!(
        format!("{baseline_report:?}"),
        format!("{inherited_report:?}")
    );
    let baseline = baseline.resource_upper_bound();
    let inherited = inherited.resource_upper_bound();
    assert_eq!(
        inherited.work_upper_bound(),
        baseline.work_upper_bound() + 23
    );
    assert_eq!(
        inherited.retained_storage_upper_bound(),
        baseline.retained_storage_upper_bound() + 29
    );
    // The extra retention starts after initial identity capture, whose earlier
    // peak need not rise. Subsequent phases must keep the extra floor live.
    assert!(inherited.peak_storage_upper_bound() >= baseline.peak_storage_upper_bound());
    assert!(inherited.peak_storage_upper_bound() <= baseline.peak_storage_upper_bound() + 29);
}

#[test]
fn unmetered_manager_and_census_substitution_refuse() {
    let context = &mut setup();
    let fixture = fixture(context, FixtureOptions::default());
    let selection = fixture.selection(context);
    let mut unmetered = PlironAnalysisManagerV1::new(&fixture.function);
    assert!(matches!(
        run_conditional_ownership_analysis_v1(
            context,
            &fixture.function,
            &mut unmetered,
            &[selection]
        ),
        Err(ConditionalOwnershipAnalysisErrorV1::Resource(
            ProductionAnalysisResourceLimitV1 {
                resource: "conditional ownership requires metered census",
                ..
            }
        ))
    ));
    let mut provider = LivePlironStructuralIdentityProviderV1::new(context, &fixture.function);
    let capture = provider
        .capture_with_resource_limits_v1(unlimited())
        .ok()
        .unwrap();
    let mut census = capture.input_census;
    census.ownership_contracts = 0;
    let mut manager = PlironAnalysisManagerV1::new_with_resource_contract(
        &fixture.function,
        census,
        capture.resource_upper_bound,
        0,
        unlimited(),
    )
    .unwrap();
    assert!(matches!(
        run_conditional_ownership_analysis_v1(
            context,
            &fixture.function,
            &mut manager,
            &[selection]
        ),
        Err(ConditionalOwnershipAnalysisErrorV1::Resource(
            ProductionAnalysisResourceLimitV1 {
                resource: "conditional ownership contract census mismatch",
                ..
            }
        ))
    ));
}

#[test]
fn display_name_storage_is_prepaid_before_contract_collection() {
    let context = &mut setup();
    let fixture = fixture(context, FixtureOptions::default());
    let short_census = manager(context, &fixture.function, unlimited())
        .unwrap()
        .input_census()
        .unwrap();
    let label = "output".repeat(8_192);
    pliron::debug_info::set_operation_result_name(
        context,
        fixture.output.get_operation(),
        0,
        Some(label.as_str().try_into().unwrap()),
    );
    let mut generous = manager(context, &fixture.function, unlimited()).unwrap();
    let census = generous.input_census().unwrap();
    assert_eq!(census.identifier_bytes, short_census.identifier_bytes);
    let names = fixture
        .output
        .result(context)
        .unique_name_byte_len(context)
        .unwrap()
        * 4;
    let phase = ProductionAnalysisResourcePhaseV1::HierarchicalOwnership;
    let collection_admission = generous
        .resource_upper_bound()
        .checked_then_retain(pending_upper_bound(census, 1).unwrap(), phase)
        .unwrap()
        .checked_then_retain(
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, names, names, 0).unwrap(),
            phase,
        )
        .unwrap();
    let mut bounded = manager(
        context,
        &fixture.function,
        ProductionAnalysisResourceLimitsV1::new(
            collection_admission.work_upper_bound() - 1,
            collection_admission.peak_storage_upper_bound(),
        ),
    )
    .unwrap();
    assert!(matches!(
        run_conditional_ownership_analysis_v1(
            context,
            &fixture.function,
            &mut bounded,
            &[fixture.selection(context)],
        ),
        Err(ConditionalOwnershipAnalysisErrorV1::Resource(
            ProductionAnalysisResourceLimitV1 {
                resource: "work upper bound",
                ..
            }
        ))
    ));
    assert_eq!(bounded.cached_entries(), 1);
    let report = run_conditional_ownership_analysis_v1(
        context,
        &fixture.function,
        &mut generous,
        &[fixture.selection(context)],
    )
    .unwrap();
    assert!(
        generous
            .resource_upper_bound()
            .retained_storage_upper_bound()
            >= names
    );
    assert_eq!(
        report.rows()[0].coverage(),
        Check::Blocked(Blocker::CanonicalCoverageAndSourceReplay)
    );
}
