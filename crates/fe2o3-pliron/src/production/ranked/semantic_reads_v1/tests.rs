use super::*;

#[path = "scheduling_tests.rs"]
mod scheduling;

type O = ProductionRankedOperationV1;
type X = ProductionSemanticExpressionV2;
type Mode = ProductionSemanticReadModeV2;
type Stage = ProductionStageHandleV1<ConstructedGraphStageV1>;
type Root = ProductionRootHandleV1<ConstructedGraphStageV1>;

fn local(index: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(index))
}

fn kernel(mode: Mode) -> ProductionRankedKernelV1 {
    let scalar = ProductionSemanticScalarTypeV2::Float { bits: 32 };
    let load = X::Load(ProductionSemanticLoadV2 {
        block: 0,
        operation: 3,
        scalar,
        read_mode: mode,
        allocation_origin: 7,
        view: local(0),
        indices: vec![local(1)].into_boxed_slice(),
    });
    let expression = X::Binary {
        operation: ProductionSemanticBinaryOpV2::Add,
        scalar,
        overflow: ProductionOverflowContractV2::Wrapping,
        lhs: Box::new(load.clone()),
        rhs: Box::new(load),
    };
    let numerical_contract = ProductionNumericalContractV2::exact_for_expression(&expression);
    ProductionRankedKernelV1::new(
        "source_reads",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                O::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [64, 1, 1],
                    workgroup_extents: [64, 1, 1],
                    subgroup_size: 64,
                    full_physical_workgroups: true,
                },
                O::View {
                    result: ProductionRankedValueIdV1::new(0),
                    element_width: 32,
                    writable: false,
                    shape: vec![64],
                    dynamic_extents: vec![],
                    allocation_origin: 7,
                    noalias_class: 1,
                },
                O::InvocationIndex {
                    result: ProductionRankedValueIdV1::new(1),
                    dimension: 0,
                    launch_extent: 64,
                },
                O::Access {
                    kind: AccessKindAttr::Read,
                    view: local(0),
                    indices: vec![local(1)],
                },
                O::SemanticExpression {
                    result: ProductionRankedValueIdV1::new(2),
                    expression,
                    numerical_contract,
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap()
}

fn construct(
    kernel: ProductionRankedKernelV1,
) -> Result<(ProductionPlironSessionV1, Stage, Root), ProductionSessionErrorV1> {
    let mut session = ProductionPlironSessionV1::new(
        ProductionSessionLimitsV1::default(),
        [
            dialect_kernel::dialect_registration().unwrap(),
            dialect_gpu::dialect_registration().unwrap(),
        ],
    )
    .unwrap();
    let construction =
        ProductionConstructionV1::ranked_kernel("source_reads_module", kernel).unwrap();
    let registered = session.register_construction(construction)?;
    let (stage, root) = session.construct_registered(registered)?;
    Ok((session, stage, root))
}

fn function(session: &ProductionPlironSessionV1, stage: &Stage) -> FuncOp {
    FuncOp::from_operation(
        session.constructed_roots[&stage.identity]
            .ranked_function
            .unwrap(),
    )
}

fn operations(session: &ProductionPlironSessionV1, stage: &Stage) -> Vec<Ptr<Operation>> {
    let context = &session.inner.context;
    function(session, stage)
        .get_entry_block(context)
        .deref(context)
        .iter(context)
        .collect()
}

fn operands(
    kernel: &mut ProductionRankedKernelV1,
) -> (&mut ProductionSemanticLoadV2, &mut ProductionSemanticLoadV2) {
    let O::SemanticExpression {
        expression: X::Binary { lhs, rhs, .. },
        ..
    } = kernel.blocks[0].operations.last_mut().unwrap()
    else {
        panic!("binary")
    };
    let (X::Load(lhs), X::Load(rhs)) = (lhs.as_mut(), rhs.as_mut()) else {
        panic!("loads")
    };
    (lhs, rhs)
}

#[test]
fn owner_shares_repeated_loads_without_duplicating_the_access() {
    let recipe = kernel(Mode::UnorderedNonVolatile);
    let old_tree_bound = recipe.tree_work;
    let (session, stage, _) = construct(recipe).unwrap();
    let context = &session.inner.context;
    let ops = operations(&session, &stage);
    assert_eq!(ops.len(), 8);
    assert!(ranked_tree_work(1, ops.len()).unwrap() <= old_tree_bound);
    let access = RankedAccessOp::from_operation(ops[3]);
    let read = SemanticTypedReadOp::from_operation(ops[4]);
    let binary = SemanticTypedBinaryOp::from_operation(ops[5]);
    assert_eq!(read.view(context), access.view(context));
    assert_eq!(read.indices(context), Some(access.indices(context)));
    assert_eq!(binary.lhs(context), read.result(context));
    assert_eq!(binary.rhs(context), read.result(context));
    assert_eq!(
        read.volatility(context),
        Some(SemanticReadVolatilityAttr::NonVolatile)
    );
    assert!(!ops.iter().any(|op| {
        Operation::get_op_dyn(*op, context)
            .downcast_ref::<SemanticTypedSymbolOp>()
            .is_some()
    }));
    assert!(
        session.constructed_roots[&stage.identity]
            .production_pipeline_report
            .is_none()
    );
}

#[test]
fn owner_keeps_distinct_reads_of_the_same_address_distinct() {
    let mut recipe = kernel(Mode::UnorderedVolatile);
    let access = recipe.blocks[0].operations[3].clone();
    recipe.blocks[0].operations.insert(4, access);
    operands(&mut recipe).1.operation = 4;
    recipe.tree_work = recipe.validate().unwrap();
    let (session, stage, _) = construct(recipe).unwrap();
    let context = &session.inner.context;
    let ops = operations(&session, &stage);
    let reads = ops
        .iter()
        .filter_map(|ptr| {
            Operation::get_op_dyn(*ptr, context)
                .downcast_ref::<SemanticTypedReadOp>()
                .map(|op| op.clone())
        })
        .collect::<Vec<_>>();
    assert_eq!(reads.len(), 2);
    assert_ne!(reads[0].result(context), reads[1].result(context));
    assert_ne!(reads[0].symbol(context), reads[1].symbol(context));
    assert_eq!(reads[0].view(context), reads[1].view(context));
    for read in reads {
        assert_eq!(
            read.volatility(context),
            Some(SemanticReadVolatilityAttr::Volatile)
        );
    }
}

#[test]
fn one_source_site_cannot_have_conflicting_read_modes() {
    let mut recipe = kernel(Mode::UnorderedNonVolatile);
    operands(&mut recipe).1.read_mode = Mode::UnorderedVolatile;
    assert!(matches!(
        recipe.validate(),
        Err(ProductionRankedKernelErrorV1::Materialization(
            "one source read site has conflicting load metadata"
        ))
    ));
}

#[test]
fn a_later_producer_must_still_dominate_its_consumer() {
    let mut recipe = kernel(Mode::UnorderedNonVolatile);
    let (lhs, rhs) = operands(&mut recipe);
    for load in [lhs, rhs] {
        load.block = 1;
        load.operation = 0;
    }
    let access = recipe.blocks[0].operations[3].clone();
    recipe.blocks[0].terminator = ProductionRankedTerminatorV1::Branch { target: 1 };
    recipe.blocks.push(ProductionRankedBlockV1::new(
        vec![access],
        ProductionRankedTerminatorV1::Return,
    ));
    recipe.tree_work = recipe.validate().unwrap();
    assert!(matches!(
        construct(recipe),
        Err(ProductionSessionErrorV1::Operation(
            OperationHandleError::OperationVerificationRejected
        ))
    ));
}

#[test]
fn missing_read_block_is_a_source_correspondence_error_not_a_cfg_target() {
    for block in [1, 4095, 4096, u32::MAX] {
        let mut recipe = kernel(Mode::UnorderedNonVolatile);
        let (lhs, rhs) = operands(&mut recipe);
        for load in [lhs, rhs] {
            load.block = block;
        }
        let error = if block < 4096 {
            ProductionRankedKernelErrorV1::InvalidReferenceContract
        } else {
            ProductionRankedKernelErrorV1::InvalidSemanticExpression(
                crate::ProductionSemanticExpressionErrorV2::UnboundLoad,
            )
        };
        assert_eq!(
            ProductionRankedKernelV1::new("missing_read", 0, recipe.blocks),
            Err(error)
        );
    }
}

#[test]
fn completed_owner_verification_rejects_a_read_that_does_not_dominate() {
    let mut recipe = kernel(Mode::UnorderedNonVolatile);
    let (lhs, rhs) = operands(&mut recipe);
    for load in [lhs, rhs] {
        load.block = 1;
        load.operation = 0;
    }
    let expression = recipe.blocks[0].operations.pop().unwrap();
    let access = recipe.blocks[0].operations.pop().unwrap();
    recipe.argument_count = 1;
    recipe.blocks[0].terminator = ProductionRankedTerminatorV1::IndexLessThan {
        lhs: ProductionRankedValueV1::Argument(0),
        rhs: local(1),
        true_block: 1,
        false_block: 2,
    };
    recipe.blocks.push(ProductionRankedBlockV1::new(
        vec![access],
        ProductionRankedTerminatorV1::Branch { target: 2 },
    ));
    recipe.blocks.push(ProductionRankedBlockV1::new(
        vec![expression],
        ProductionRankedTerminatorV1::Return,
    ));
    recipe.tree_work = recipe.validate().unwrap();
    assert!(matches!(
        construct(recipe.clone()),
        Err(ProductionSessionErrorV1::Operation(
            OperationHandleError::OperationVerificationRejected
        ))
    ));
    recipe.blocks[0].terminator = ProductionRankedTerminatorV1::Branch { target: 1 };
    recipe.tree_work = recipe.validate().unwrap();
    assert!(
        construct(recipe).is_ok(),
        "the same producer must work when it dominates"
    );
}

#[test]
fn owner_snapshot_rejects_changed_read_volatility_before_analysis() {
    let (mut session, stage, root) = construct(kernel(Mode::UnorderedNonVolatile)).unwrap();
    let read = SemanticTypedReadOp::from_operation(operations(&session, &stage)[4]);
    read.set_attr_kernel_semantic_read_volatility(
        &mut session.inner.context,
        SemanticReadVolatilityAttr::Volatile,
    );
    crate::production_analysis::panic_next_production_analysis_for_test_v1();
    assert!(matches!(
        session.verify_production_ranked_kernel_pipeline(stage, root),
        Err(ProductionSessionErrorV1::Operation(
            OperationHandleError::OperationGraphChangedOutsideTransaction
        ))
    ));
    assert!(session.is_poisoned());
    let (mut clean, stage, root) = construct(kernel(Mode::UnorderedNonVolatile)).unwrap();
    assert!(matches!(
        clean.verify_production_ranked_kernel_pipeline(stage, root),
        Err(ProductionSessionErrorV1::Operation(
            OperationHandleError::UpstreamPanicked
        ))
    ));
    assert!(clean.is_poisoned());
}

#[test]
fn paired_source_reads_pass_bounds_but_not_unproved_value_semantics() {
    for mode in [Mode::UnorderedNonVolatile, Mode::UnorderedVolatile] {
        let (mut session, stage, root) = construct(kernel(mode)).unwrap();
        let bounds = crate::production_analysis::run_pliron_ranked_bounds_check_v1(
            &session.inner.context,
            &function(&session, &stage),
        );
        assert!(bounds.is_clean(), "{bounds:?}");
        let Err(ProductionSessionErrorV1::RankedSemantic(error)) =
            session.verify_production_ranked_kernel_pipeline(stage, root)
        else {
            panic!("memory pairing must not supply a value-semantics proof");
        };
        assert!(error.report().findings().iter().any(|finding| matches!(
            finding,
            crate::PlironSemanticRefinementFindingV1::TypedExpressionRejected { .. }
        )));
    }
}

#[test]
fn exact_identity_retains_read_mode_and_rejects_missing_metadata() {
    let (mut session, stage, _) = construct(kernel(Mode::UnorderedNonVolatile)).unwrap();
    let function = function(&session, &stage);
    let read = SemanticTypedReadOp::from_operation(operations(&session, &stage)[4]);
    let capture = crate::production_analysis::derive_pliron_ir_structural_identity_v1;
    let original = capture(&session.inner.context, &function).unwrap();
    let before = session
        .inner
        .context
        .ir_mutation_attempt_epoch()
        .unwrap()
        .value();
    read.set_attr_kernel_semantic_read_volatility(
        &mut session.inner.context,
        SemanticReadVolatilityAttr::Volatile,
    );
    let changed = capture(&session.inner.context, &function).unwrap();
    assert_ne!(original, changed);
    read.set_attr_kernel_semantic_read_volatility(
        &mut session.inner.context,
        SemanticReadVolatilityAttr::NonVolatile,
    );
    assert_eq!(
        original,
        capture(&session.inner.context, &function).unwrap()
    );
    // Exact graph bytes and mutation history are distinct. Pass preservation
    // compares both within its guarded scope.
    assert!(
        session
            .inner
            .context
            .ir_mutation_attempt_epoch()
            .unwrap()
            .value()
            >= before + 2
    );
    read.get_operation()
        .deref_mut(&mut session.inner.context)
        .attributes
        .0
        .remove(
            &pliron::identifier::Identifier::try_from("kernel_semantic_read_volatility").unwrap(),
        );
    assert!(capture(&session.inner.context, &function).is_err());
}

#[test]
fn read_identity_census_and_resource_limits_cover_the_actual_owner_graph() {
    use crate::production_analysis::{
        IdentityCaptureFailureV1, LivePlironStructuralIdentityProviderV1,
        PlironStructuralIdentityProviderV1, ProductionAnalysisResourceLimitsV1 as Limits,
    };
    let (session, stage, _) = construct(kernel(Mode::UnorderedNonVolatile)).unwrap();
    let context = &session.inner.context;
    let function = function(&session, &stage);
    let mut provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
    let captured = provider
        .capture_with_resource_limits_v1(Limits::production_hard_ceiling())
        .unwrap_or_else(|_| panic!("hard-ceiling identity capture"));
    let census = captured.input_census;
    assert_eq!(census.operations, 8);
    assert_eq!(census.ranked_accesses, 1);
    assert_eq!(census.semantic_definitions, 3);
    assert_eq!(census.operands, 7);
    assert_eq!(census.results, 5);
    let ops = operations(&session, &stage);
    assert_eq!(ops[4].deref(context).attributes.0.len(), 6);
    let expected_attributes = function.get_operation().deref(context).attributes.0.len()
        + function
            .get_entry_block(context)
            .deref(context)
            .attributes
            .0
            .len()
        + ops
            .iter()
            .map(|op| op.deref(context).attributes.0.len())
            .sum::<usize>();
    assert_eq!(census.attributes, expected_attributes);
    let bound = captured.resource_upper_bound;
    let work = bound.work_upper_bound();
    let storage = bound.peak_storage_upper_bound();
    let exact = provider
        .capture_with_resource_limits_v1(Limits::new(work, storage))
        .unwrap_or_else(|_| panic!("exact identity capture"));
    assert_eq!(exact.resource_upper_bound, bound);
    for limits in [
        Limits::new(work - 1, storage),
        Limits::new(work, storage - 1),
    ] {
        assert!(matches!(
            provider.capture_with_resource_limits_v1(limits),
            Err(IdentityCaptureFailureV1::ResourceLimit(_))
        ));
    }
}
