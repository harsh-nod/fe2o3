use super::*;

#[test]
fn later_listed_source_uses_one_original_read_without_reordering_the_graph() {
    for block_argument in [false, true] {
        let recipe = later_producer(block_argument, false);
        let old_bound = recipe.tree_work;
        let mut identities = Vec::new();
        for _ in 0..2 {
            let (mut session, stage, root) = construct(recipe.clone()).unwrap();
            let context = &session.inner.context;
            let function = function(&session, &stage);
            let blocks = function
                .get_region(context)
                .deref(context)
                .iter(context)
                .collect::<Vec<_>>();
            assert_eq!(blocks.len(), 3);
            let producer = blocks[2].deref(context).iter(context).collect::<Vec<_>>();
            let access_index = usize::from(!block_argument);
            assert!(Operation::is_op::<RankedAccessOp>(
                producer[access_index],
                context
            ));
            let read = SemanticTypedReadOp::from_operation(producer[access_index + 1]);
            let consumer = blocks[1]
                .deref(context)
                .iter(context)
                .find(|op| Operation::is_op::<SemanticTypedBinaryOp>(*op, context))
                .unwrap();
            assert_eq!(
                consumer.deref(context).operands().collect::<Vec<_>>(),
                vec![read.result(context); 2]
            );
            let ops = blocks
                .iter()
                .flat_map(|block| block.deref(context).iter(context))
                .collect::<Vec<_>>();
            assert_eq!(
                ops.iter()
                    .filter(|op| Operation::is_op::<SemanticTypedReadOp>(**op, context))
                    .count(),
                1
            );
            assert!(ranked_tree_work(blocks.len(), ops.len()).unwrap() <= old_bound);
            identities.push(
                crate::production_analysis::derive_pliron_ir_structural_identity_v1(
                    context, &function,
                )
                .unwrap(),
            );
            let verification = session.verify_production_ranked_kernel_pipeline(stage, root);
            if block_argument {
                assert!(
                    matches!(
                        verification,
                        Err(ProductionSessionErrorV1::RankedSemantic(_))
                    ),
                    "{verification:?}"
                );
            } else {
                let Err(ProductionSessionErrorV1::RankedBounds(error)) = verification else {
                    panic!("{verification:?}");
                };
                assert!(matches!(
                    error.report().findings(),
                    [crate::RankedBoundsFindingV1::UnprovedBound {
                        block: 2,
                        operation: 1,
                        access: AccessKindAttr::Read,
                        dimension: 0,
                        ..
                    }]
                ));
            }
        }
        assert_eq!(identities[0], identities[1]);
    }
}

#[test]
fn dependency_ready_read_on_one_diamond_arm_fails_native_dominance() {
    assert!(matches!(
        construct(later_producer(false, true)),
        Err(ProductionSessionErrorV1::Operation(
            OperationHandleError::OperationVerificationRejected
        ))
    ));
}

#[test]
fn later_listed_read_constructs_with_a_real_cfg_backedge() {
    let mut kernel = later_producer(true, false);
    let argument = |block| ProductionRankedValueV1::BlockArgument { block, argument: 0 };
    kernel.blocks[1].index_argument_count = 1;
    kernel.blocks[1].terminator = ProductionRankedTerminatorV1::BranchArgsAdd {
        value: argument(1),
        step: local(1),
        target: 2,
    };
    kernel.blocks[2].terminator = ProductionRankedTerminatorV1::IndexLessThanArgs {
        lhs: argument(2),
        rhs: ProductionRankedValueV1::Argument(0),
        true_arguments: vec![argument(2)],
        false_arguments: vec![],
        true_block: 1,
        false_block: 3,
    };
    kernel.blocks.push(ProductionRankedBlockV1::new(
        vec![],
        ProductionRankedTerminatorV1::Return,
    ));
    let kernel = ProductionRankedKernelV1::new("read_backedge", 1, kernel.blocks).unwrap();
    let (session, stage, _) = construct(kernel).unwrap();
    let context = &session.inner.context;
    let blocks = function(&session, &stage)
        .get_region(context)
        .deref(context)
        .iter(context)
        .collect::<Vec<_>>();
    let producer = blocks[2].deref(context).iter(context).collect::<Vec<_>>();
    let read = SemanticTypedReadOp::from_operation(producer[1]);
    let consumer = blocks[1].deref(context).iter(context).collect::<Vec<_>>();
    let binary = consumer
        .iter()
        .find(|op| Operation::is_op::<SemanticTypedBinaryOp>(**op, context))
        .unwrap();
    assert_eq!(
        binary.deref(context).operands().collect::<Vec<_>>(),
        vec![read.result(context); 2]
    );
    let add = IndexBinaryOp::from_operation(consumer[consumer.len() - 2]);
    let branch = consumer.last().unwrap();
    assert!(Operation::is_op::<BranchArgsOp>(*branch, context));
    assert_eq!(
        branch.deref(context).operands().collect::<Vec<_>>(),
        vec![add.result(context)]
    );
    assert_eq!(
        branch.deref(context).successors().collect::<Vec<_>>(),
        vec![blocks[2]]
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
    let (lhs, rhs) = operands(&mut kernel);
    for load in [lhs, rhs] {
        load.block = 2;
        load.operation = u32::from(!block_argument);
        load.indices = vec![index].into_boxed_slice();
    }
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
