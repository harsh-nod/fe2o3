use super::*;
use fe2o3_kernel_analysis::{PlironSemanticMemoryErrorV1, prove_live_pliron_semantic_memory_v1};

type O = ProductionRankedOperationV1;
type X = ProductionSemanticExpressionV2;
type Mode = ProductionSemanticReadModeV2;

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
                O::ViewInSpace {
                    result: ProductionRankedValueIdV1::new(0),
                    element_width: 32,
                    writable: false,
                    shape: vec![64],
                    dynamic_extents: vec![],
                    memory_space: MemorySpaceAttr::Global,
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

fn materialize(
    kernel: &ProductionRankedKernelV1,
) -> Result<(ProductionPlironSessionV1, FuncOp), ProductionRankedKernelErrorV1> {
    let mut session = ProductionPlironSessionV1::new(
        ProductionSessionLimitsV1::default(),
        [
            dialect_kernel::dialect_registration().unwrap(),
            dialect_gpu::dialect_registration().unwrap(),
            dialect_proof::dialect_registration().unwrap(),
        ],
    )
    .unwrap();
    kernel.validate()?;
    let reads = RankedSemanticReadsV1::new(kernel)?;
    let schedule = RankedOperationScheduleV1::new(kernel, &reads)?;
    let context = &mut session.inner.context;
    let index: TypeHandle = IndexType::get(context).into();
    let ty = FunctionType::get(context, vec![index; kernel.argument_count()], vec![]);
    let function = FuncOp::new(context, "source_reads".try_into().unwrap(), ty);
    materialization_v1::materialize_body(context, &function, kernel, &schedule, reads, &[])?;
    pliron::operation::verify_operation(function.get_operation(), context)
        .map_err(|_| reject("completed source-read graph failed verification"))?;
    Ok((session, function))
}

fn change_loads(
    kernel: &mut ProductionRankedKernelV1,
    mut change: impl FnMut(&mut ProductionSemanticLoadV2),
) {
    let O::SemanticExpression {
        expression: X::Binary { lhs, rhs, .. },
        ..
    } = &mut kernel.blocks[0].operations[4]
    else {
        panic!("binary root")
    };
    for operand in [lhs, rhs] {
        let X::Load(load) = operand.as_mut() else {
            panic!("load leaf")
        };
        change(load);
    }
}

#[test]
fn repeated_loads_share_one_adjacent_actual_ssa_producer_not_a_free_symbol() {
    let kernel = kernel(Mode::UnorderedNonVolatile);
    let (session, function) = materialize(&kernel).unwrap();
    let context = &session.inner.context;
    let operations: Vec<_> = function
        .get_entry_block(context)
        .deref(context)
        .iter(context)
        .collect();
    let reads: Vec<_> = operations
        .iter()
        .copied()
        .filter(|op| Operation::is_op::<SemanticTypedReadOp>(*op, context))
        .collect();
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0], operations[4]);
    assert!(Operation::is_op::<RankedAccessOp>(operations[3], context));
    assert!(
        !operations
            .iter()
            .any(|op| Operation::is_op::<SemanticTypedSymbolOp>(*op, context))
    );
    let read = SemanticTypedReadOp::from_operation(reads[0]);
    let binary = operations
        .iter()
        .copied()
        .find(|op| Operation::is_op::<SemanticTypedBinaryOp>(*op, context))
        .unwrap();
    assert_eq!(
        binary.deref(context).operands().collect::<Vec<_>>(),
        vec![read.result(context); 2]
    );
    // Two Load nodes formerly consumed two operation slots; one producer is
    // now shared. The existing operation-count preflight remains conservative.
    assert_eq!(operations.len(), 8);
    let proof = prove_live_pliron_semantic_memory_v1(context, &function).unwrap();
    proof
        .with_live_reads(context, &function, |reads| {
            assert_eq!(reads.len(), 1);
            assert!(reads[0].reads_initial_memory());
            assert_eq!(reads[0].instances().len(), 64);
        })
        .unwrap();
}

#[test]
fn volatile_source_mode_is_retained_and_not_proved_as_initial_nonvolatile_memory() {
    let kernel = kernel(Mode::UnorderedVolatile);
    let (session, function) = materialize(&kernel).unwrap();
    let context = &session.inner.context;
    let producer = function
        .get_entry_block(context)
        .deref(context)
        .iter(context)
        .find(|op| Operation::is_op::<SemanticTypedReadOp>(*op, context))
        .unwrap();
    assert_eq!(
        SemanticTypedReadOp::from_operation(producer).volatility(context),
        Some(SemanticReadVolatilityAttr::Volatile)
    );
    assert!(matches!(
        prove_live_pliron_semantic_memory_v1(context, &function),
        Err(PlironSemanticMemoryErrorV1::UnsupportedRead { .. })
    ));
}

#[test]
fn conflicting_read_modes_and_forward_or_missing_requests_reject_before_materialization() {
    let mut conflicting = kernel(Mode::UnorderedNonVolatile);
    let mut first = true;
    change_loads(&mut conflicting, |load| {
        if first {
            load.read_mode = Mode::UnorderedVolatile;
            first = false;
        }
    });
    assert!(matches!(
        RankedSemanticReadsV1::new(&conflicting),
        Err(ProductionRankedKernelErrorV1::Materialization(
            "one source read site has conflicting load metadata"
        ))
    ));
    let mut forward = kernel(Mode::UnorderedNonVolatile);
    change_loads(&mut forward, |load| load.operation = 4);
    assert!(matches!(
        RankedSemanticReadsV1::new(&forward),
        Err(ProductionRankedKernelErrorV1::Materialization(
            "semantic read does not precede its consumer in the same block"
        ))
    ));
    let mut missing = kernel(Mode::UnorderedNonVolatile);
    change_loads(&mut missing, |load| load.block = 1);
    assert!(matches!(
        missing.validate(),
        Err(ProductionRankedKernelErrorV1::InvalidBlockTarget(1))
    ));
}

#[test]
fn actual_access_and_allocation_cannot_be_substituted_for_the_requested_read() {
    for wrong_site in [false, true] {
        let mut changed = kernel(Mode::UnorderedNonVolatile);
        change_loads(&mut changed, |load| {
            if wrong_site {
                load.operation = 2;
            } else {
                load.allocation_origin = 8;
            }
        });
        assert!(matches!(
            materialize(&changed),
            Err(ProductionRankedKernelErrorV1::InvalidReferenceContract)
        ));
    }
}

#[test]
fn producer_plan_charges_actual_bounded_traversal() {
    let mut large = kernel(Mode::UnorderedNonVolatile);
    large.blocks[0].operations = vec![
        O::IndexConstant {
            result: ProductionRankedValueIdV1::new(0),
            value: 0,
        };
        MAX_RANKED_BOUNDS_OPERATIONS + 1
    ];
    assert!(
        matches!(RankedSemanticReadsV1::new(&large), Err(ProductionRankedKernelErrorV1::ResourceLimit {
        resource: "semantic read materialization work", limit, actual,
    }) if limit == MAX_RANKED_BOUNDS_OPERATIONS && actual == limit + 1)
    );
}

fn later_producer(block_argument: bool, bypass: bool) -> ProductionRankedKernelV1 {
    let mut kernel = kernel(Mode::UnorderedNonVolatile);
    let index = if block_argument {
        ProductionRankedValueV1::BlockArgument {
            block: 2,
            argument: 0,
        }
    } else {
        local(3)
    };
    change_loads(&mut kernel, |load| {
        load.block = 2;
        load.operation = u32::from(!block_argument);
        load.indices = vec![index].into_boxed_slice();
    });
    let expression = kernel.blocks[0].operations.pop().unwrap();
    kernel.blocks[0].operations.pop().unwrap();
    kernel.blocks[0].terminator = if bypass {
        ProductionRankedTerminatorV1::IndexLessThan {
            lhs: local(1),
            rhs: ProductionRankedValueV1::Argument(0),
            true_block: 2,
            false_block: 3,
        }
    } else if block_argument {
        ProductionRankedTerminatorV1::BranchArgs {
            target: 2,
            arguments: vec![local(1)],
        }
    } else {
        ProductionRankedTerminatorV1::Branch { target: 2 }
    };
    kernel.blocks.push(ProductionRankedBlockV1::new(
        vec![expression],
        ProductionRankedTerminatorV1::Return,
    ));
    let mut producer = Vec::new();
    if !block_argument {
        producer.push(O::IndexUnsignedCast {
            result: ProductionRankedValueIdV1::new(3),
            source: local(1),
            bit_width: 32,
        });
    }
    producer.push(O::Access {
        kind: AccessKindAttr::Read,
        view: local(0),
        indices: vec![index],
    });
    kernel
        .blocks
        .push(ProductionRankedBlockV1::with_index_arguments(
            u32::from(block_argument),
            producer,
            ProductionRankedTerminatorV1::Branch { target: 1 },
        ));
    if bypass {
        kernel.blocks.push(ProductionRankedBlockV1::new(
            vec![],
            ProductionRankedTerminatorV1::Branch { target: 1 },
        ));
    }
    ProductionRankedKernelV1::new("later_producer", usize::from(bypass), kernel.blocks).unwrap()
}

#[test]
fn later_listed_source_and_producer_block_arguments_use_one_exact_ssa_read() {
    for block_argument in [false, true] {
        let kernel = later_producer(block_argument, false);
        let mut session = ProductionPlironSessionV1::new(
            ProductionSessionLimitsV1::default(),
            [
                dialect_kernel::dialect_registration().unwrap(),
                dialect_gpu::dialect_registration().unwrap(),
                dialect_proof::dialect_registration().unwrap(),
            ],
        )
        .unwrap();
        let registered = session
            .register_construction(
                ProductionConstructionV1::ranked_kernel("later_producer", kernel).unwrap(),
            )
            .unwrap();
        let (stage, root) = session.construct_registered(registered).unwrap();
        let function = FuncOp::from_operation(
            session.constructed_roots[&stage.identity]
                .ranked_function
                .unwrap(),
        );
        let context = &session.inner.context;
        let blocks: Vec<_> = function
            .get_region(context)
            .deref(context)
            .iter(context)
            .collect();
        assert_eq!(blocks.len(), 3);
        let producer: Vec<_> = blocks[2].deref(context).iter(context).collect();
        let access = usize::from(!block_argument);
        assert!(Operation::is_op::<RankedAccessOp>(
            producer[access],
            context
        ));
        assert!(Operation::is_op::<SemanticTypedReadOp>(
            producer[access + 1],
            context
        ));
        let read = SemanticTypedReadOp::from_operation(producer[access + 1]);
        let consumer = blocks[1]
            .deref(context)
            .iter(context)
            .find(|op| Operation::is_op::<SemanticTypedBinaryOp>(*op, context))
            .unwrap();
        assert_eq!(
            consumer.deref(context).operands().collect::<Vec<_>>(),
            vec![read.result(context); 2]
        );
        assert_eq!(
            blocks
                .iter()
                .flat_map(|block| block.deref(context).iter(context))
                .filter(|op| Operation::is_op::<SemanticTypedReadOp>(*op, context))
                .count(),
            1
        );
        let proof = prove_live_pliron_semantic_memory_v1(context, &function).unwrap();
        proof
            .with_live_reads(context, &function, |reads| {
                assert_eq!(reads.len(), 1);
                assert!(reads[0].reads_initial_memory());
                assert_eq!(reads[0].instances().len(), 64);
            })
            .unwrap();
        let _verified = session
            .verify_production_ranked_kernel_pipeline(stage, root)
            .unwrap();
    }
}

#[test]
fn dependency_ready_read_on_only_one_diamond_arm_fails_native_dominance() {
    let kernel = later_producer(false, true);
    let mut session = ProductionPlironSessionV1::new(
        ProductionSessionLimitsV1::default(),
        [
            dialect_kernel::dialect_registration().unwrap(),
            dialect_gpu::dialect_registration().unwrap(),
        ],
    )
    .unwrap();
    let registered = session
        .register_construction(ProductionConstructionV1::ranked_kernel("bypass", kernel).unwrap())
        .unwrap();
    assert!(matches!(
        session.construct_registered(registered),
        Err(ProductionSessionErrorV1::Operation(
            OperationHandleError::OperationVerificationRejected
        ))
    ));
    assert!(session.is_poisoned());
}
