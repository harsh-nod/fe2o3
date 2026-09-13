use super::*;
use crate::production_analysis::{
    ConditionalPrefixConditionV1, ConditionalPrefixDerivationErrorV1,
    ConditionalPrefixExtentSourceV1, HierarchicalOwnershipFindingV1,
    run_pliron_hierarchical_ownership_check_v1,
};

fn prefix_kernel(mode: Mode) -> ProductionRankedKernelV1 {
    type B = ProductionRankedBlockV1;
    type T = ProductionRankedTerminatorV1;
    let id = ProductionRankedValueIdV1::new;
    let arg = ProductionRankedValueV1::Argument;
    let scalar = ProductionSemanticScalarTypeV2::Float { bits: 32 };
    let view = |v| O::ViewInSpace {
        result: id(v),
        element_width: 32,
        writable: v == 0,
        shape: vec![0],
        dynamic_extents: vec![arg(v)],
        memory_space: dialect_kernel::MemorySpaceAttr::Global,
        allocation_origin: 10 + u64::from(v),
        noalias_class: 10 + u64::from(v),
    };
    let guard = |extent, yes| T::IndexLessThan {
        lhs: local(3),
        rhs: arg(extent),
        true_block: yes,
        false_block: 4,
    };
    let read = |v| O::Access {
        kind: AccessKindAttr::Read,
        view: local(v),
        indices: vec![local(3)],
    };
    let load = |block, v| {
        X::Load(ProductionSemanticLoadV2 {
            block,
            operation: 0,
            scalar,
            read_mode: mode,
            allocation_origin: 10 + u64::from(v),
            view: local(v),
            indices: vec![local(3)].into_boxed_slice(),
        })
    };
    let expression = X::Binary {
        operation: ProductionSemanticBinaryOpV2::Add,
        scalar,
        overflow: ProductionOverflowContractV2::Wrapping,
        lhs: Box::new(load(2, 1)),
        rhs: Box::new(load(3, 2)),
    };
    let numerical_contract = ProductionNumericalContractV2::exact_for_expression(&expression);
    ProductionRankedKernelV1::new(
        "conditional_prefix",
        3,
        vec![
            B::new(
                vec![
                    O::ExecutionLayout {
                        grid_identity: 1,
                        global_extents: [64, 1, 1],
                        workgroup_extents: [64, 1, 1],
                        subgroup_size: 64,
                        full_physical_workgroups: true,
                    },
                    view(0),
                    view(1),
                    view(2),
                    O::InvocationIndex {
                        result: id(3),
                        dimension: 0,
                        launch_extent: 64,
                    },
                    O::OwnershipContract {
                        view: local(0),
                        coverage: OwnershipCoverageAttr::TotalView,
                        partition: OwnershipPartitionAttr::ExactSets,
                    },
                ],
                guard(0, 1),
            ),
            B::new(vec![], guard(1, 2)),
            B::new(vec![read(1)], guard(2, 3)),
            B::new(
                vec![
                    read(2),
                    O::SemanticExpression {
                        result: id(4),
                        expression,
                        numerical_contract,
                    },
                    O::ValueAccess {
                        kind: AccessKindAttr::Write,
                        view: local(0),
                        indices: vec![local(3)],
                        value: local(4),
                    },
                ],
                T::Return,
            ),
            B::new(vec![], T::Return),
        ],
    )
    .unwrap()
}

fn ownership_report(recipe: ProductionRankedKernelV1) -> crate::HierarchicalOwnershipReportV1 {
    let (mut session, stage, root) = construct(recipe).unwrap();
    let expected = run_pliron_hierarchical_ownership_check_v1(
        &session.inner.context,
        &function(&session, &stage),
    );
    let error = session
        .verify_production_ranked_kernel_pipeline(stage, root)
        .unwrap_err();
    let ProductionSessionErrorV1::RankedOwnership(error) = error else {
        panic!("ownership boundary: {error:?}");
    };
    let report = error.report();
    assert_eq!(report.status(), expected.status());
    assert_eq!(report.findings(), expected.findings());
    assert_eq!(report.regions(), expected.regions());
    assert_eq!(report.coverage_summary(), expected.coverage_summary());
    assert!(!report.is_clean());
    assert!(!report.all_total_view_contracts_are_proved());
    report.clone()
}

#[test]
fn conditional_prefix_public_pipeline_retains_conditions_without_admission() {
    for mode in [Mode::UnorderedNonVolatile, Mode::UnorderedVolatile] {
        let report = ownership_report(prefix_kernel(mode));
        assert!(matches!(
            report.findings(),
            [HierarchicalOwnershipFindingV1::TraceIncomplete { .. }]
        ));
        assert_eq!(report.coverage_summary().total_view_declared(), 1);
        assert_eq!(report.coverage_summary().total_view_proved(), 0);
        let coverage = report
            .conditional_prefix_coverage()
            .expect("conditional prefix");
        assert!(report.conditional_prefix_failure().is_none());
        assert_eq!(coverage.conditions().len(), 3);
        assert_eq!(coverage.host_binding_obligations().len(), 4);
        assert_eq!(
            coverage.output().source(),
            ConditionalPrefixExtentSourceV1::RankedEntryArgument(0)
        );
        assert_eq!(coverage.launch().declared_workitems(), 64);
        let [term] = coverage.source_guard_dnf() else {
            panic!("one path")
        };
        assert_eq!(term.len(), 3);
        assert!(term.iter().all(|atom| atom.less_than()));
        for (index, condition) in coverage.conditions().iter().enumerate() {
            match condition {
                ConditionalPrefixConditionV1::OutputExtentAtMostInput { output, input } => {
                    assert_eq!(*output, coverage.output());
                    assert_eq!(
                        input.source(),
                        ConditionalPrefixExtentSourceV1::RankedEntryArgument(index as u32 + 1)
                    );
                }
                ConditionalPrefixConditionV1::OutputExtentAtMostActualWorkitems {
                    output,
                    launch,
                } => {
                    assert_eq!(index, 2);
                    assert_eq!(*output, coverage.output());
                    assert_eq!(*launch, coverage.launch());
                }
            }
        }
        assert!(!coverage.proves_unconditional_total_view());
        assert!(!coverage.grants_launch_authority());
        let same = ownership_report(prefix_kernel(mode));
        assert_eq!(coverage, same.conditional_prefix_coverage().unwrap());
        assert_eq!(
            coverage.canonical_bytes(),
            same.conditional_prefix_coverage()
                .unwrap()
                .canonical_bytes()
        );
    }
}

#[test]
fn conditional_prefix_missing_rhs_is_recorded_without_changing_ownership_failure() {
    let mut recipe = prefix_kernel(Mode::UnorderedNonVolatile);
    recipe.blocks[3].operations[2] = O::Access {
        kind: AccessKindAttr::Write,
        view: local(0),
        indices: vec![local(3)],
    };
    recipe.tree_work = recipe.validate().unwrap();
    let report = ownership_report(recipe);
    assert!(report.conditional_prefix_coverage().is_none());
    assert_eq!(
        report.conditional_prefix_failure(),
        Some(&ConditionalPrefixDerivationErrorV1::Unsupported(
            "write has no retained scalar RHS"
        ))
    );
}

#[test]
fn conditional_prefix_checks_unsupported_unused_arithmetic_too() {
    let scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: true,
        bits: 32,
    };
    let expression = X::Binary {
        operation: ProductionSemanticBinaryOpV2::Divide,
        scalar,
        overflow: ProductionOverflowContractV2::Wrapping,
        lhs: Box::new(X::Constant { scalar, bits: 1 }),
        rhs: Box::new(X::Constant { scalar, bits: 1 }),
    };
    let mut recipe = prefix_kernel(Mode::UnorderedNonVolatile);
    recipe.blocks[3].operations.push(O::SemanticExpression {
        result: ProductionRankedValueIdV1::new(5),
        numerical_contract: ProductionNumericalContractV2::exact_for_expression(&expression),
        expression,
    });
    recipe.tree_work = recipe.validate().unwrap();
    let report = ownership_report(recipe);
    assert!(report.conditional_prefix_coverage().is_none());
    assert_eq!(
        report.conditional_prefix_failure(),
        Some(&ConditionalPrefixDerivationErrorV1::Unsupported(
            "binary domain or operands"
        ))
    );
}

#[test]
fn conditional_prefix_tracks_value_changes_without_proving_value_equivalence() {
    let before = ownership_report(prefix_kernel(Mode::UnorderedNonVolatile));
    for replacement in [
        ProductionSemanticBinaryOpV2::Subtract,
        ProductionSemanticBinaryOpV2::Divide,
    ] {
        let mut recipe = prefix_kernel(Mode::UnorderedNonVolatile);
        let O::SemanticExpression {
            expression: X::Binary { operation, .. },
            ..
        } = &mut recipe.blocks[3].operations[1]
        else {
            unreachable!()
        };
        *operation = replacement;
        recipe.tree_work = recipe.validate().unwrap();
        let after = ownership_report(recipe);
        let (before, after) = (
            before.conditional_prefix_coverage().unwrap(),
            after.conditional_prefix_coverage().unwrap(),
        );
        assert_eq!(before.conditions(), after.conditions());
        assert_eq!(
            before.host_binding_obligations(),
            after.host_binding_obligations()
        );
        assert_ne!(before.graph_identity(), after.graph_identity());
        assert_ne!(before.canonical_bytes(), after.canonical_bytes());
        assert!(!after.grants_launch_authority());
    }
}

#[test]
fn conditional_prefix_checks_stored_width_and_reachable_traps() {
    let mut wide = prefix_kernel(Mode::UnorderedNonVolatile);
    let O::ViewInSpace { element_width, .. } = &mut wide.blocks[0].operations[1] else {
        unreachable!()
    };
    *element_width = 64;
    wide.tree_work = wide.validate().unwrap();
    let report = ownership_report(wide);
    assert!(report.conditional_prefix_coverage().is_none());
    assert_eq!(
        report.conditional_prefix_failure(),
        Some(&ConditionalPrefixDerivationErrorV1::Malformed(
            "stored value width"
        ))
    );

    let mut trapped = prefix_kernel(Mode::UnorderedNonVolatile);
    trapped.blocks[4].terminator = ProductionRankedTerminatorV1::Trap;
    trapped.tree_work = trapped.validate().unwrap();
    let report = ownership_report(trapped);
    assert!(report.conditional_prefix_coverage().is_none());
    assert!(matches!(
        report.conditional_prefix_failure(),
        Some(ConditionalPrefixDerivationErrorV1::UnsupportedOperation {
            operation: "kernel.trap",
            ..
        })
    ));
}

#[test]
fn conditional_prefix_original_reads_feed_add_and_the_actual_store() {
    let (session, stage, _) = construct(prefix_kernel(Mode::UnorderedVolatile)).unwrap();
    let context = &session.inner.context;
    let function = function(&session, &stage);
    let blocks = function
        .get_region(context)
        .deref(context)
        .iter(context)
        .collect::<Vec<_>>();
    let block_ops = |index: usize| {
        blocks[index]
            .deref(context)
            .iter(context)
            .collect::<Vec<_>>()
    };
    let mut reads = Vec::new();
    for index in [2, 3] {
        let ops = block_ops(index);
        let access = RankedAccessOp::from_operation(ops[0]);
        let read = SemanticTypedReadOp::from_operation(ops[1]);
        assert_eq!(access.kind(context), Some(AccessKindAttr::Read));
        assert_eq!(read.view(context), access.view(context));
        assert_eq!(read.indices(context), Some(access.indices(context)));
        assert_eq!(
            read.volatility(context),
            Some(SemanticReadVolatilityAttr::Volatile)
        );
        reads.push(read.result(context));
    }
    assert_ne!(reads[0], reads[1]);
    let ops = block_ops(3);
    let binary = SemanticTypedBinaryOp::from_operation(ops[2]);
    assert_eq!(binary.lhs(context), reads[0]);
    assert_eq!(binary.rhs(context), reads[1]);
    let root = SemanticTypedExpressionRootOp::from_operation(ops[3]);
    assert_eq!(root.expression(context), binary.result(context));
    let store = RankedAccessOp::from_operation(ops[4]);
    assert_eq!(store.kind(context), Some(AccessKindAttr::Write));
    assert_eq!(store.stored_value(context), Some(root.result(context)));
}
