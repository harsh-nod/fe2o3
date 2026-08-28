use dialect_kernel::{IndexBinaryKindAttr, MemorySpaceAttr, PipelineEventKindAttr};
use fe2o3_kernel_analysis::KernelCheckPassKindV1;
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
    ProductionRankedOperationV1, ProductionRankedTerminatorV1, ProductionRankedValueIdV1,
    ProductionRankedValueV1, ProductionSessionErrorV1, ProductionSessionLimitsV1,
    compile_ranked_kernel_for_lowering_v1,
};

fn id(value: u32) -> ProductionRankedValueIdV1 {
    ProductionRankedValueIdV1::new(value)
}

fn local(value: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(id(value))
}

fn argument(block: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::BlockArgument { block, argument: 0 }
}

fn event(
    epoch: ProductionRankedValueV1,
    slot: ProductionRankedValueV1,
    kind: PipelineEventKindAttr,
) -> ProductionRankedOperationV1 {
    ProductionRankedOperationV1::PipelineEvent {
        pipeline: local(1),
        epoch,
        slot,
        kind,
    }
}

fn dynamic_double_buffer(wrong_future_epoch: bool) -> ProductionRankedKernelV1 {
    let entry = ProductionRankedBlockV1::new(
        vec![
            ProductionRankedOperationV1::ViewInSpace {
                result: id(0),
                element_width: 16,
                writable: true,
                shape: vec![2, 64],
                dynamic_extents: vec![],
                memory_space: MemorySpaceAttr::Workgroup,
                allocation_origin: 0,
                noalias_class: 0,
            },
            ProductionRankedOperationV1::PipelineCreate {
                result: id(1),
                view: local(0),
                buffers: 2,
                prefetch_distance: 1,
            },
            ProductionRankedOperationV1::IndexConstant {
                result: id(2),
                value: 0,
            },
            ProductionRankedOperationV1::IndexConstant {
                result: id(3),
                value: 1,
            },
            ProductionRankedOperationV1::IndexConstant {
                result: id(4),
                value: 2,
            },
            event(local(2), local(2), PipelineEventKindAttr::Stage),
            event(local(2), local(2), PipelineEventKindAttr::Commit),
        ],
        ProductionRankedTerminatorV1::BranchArgs {
            arguments: vec![local(2)],
            target: 1,
        },
    );
    let header = ProductionRankedBlockV1::with_index_arguments(
        1,
        vec![],
        ProductionRankedTerminatorV1::IndexLessThanArgs {
            lhs: argument(1),
            rhs: ProductionRankedValueV1::Argument(0),
            true_arguments: vec![argument(1)],
            false_arguments: vec![],
            true_block: 2,
            false_block: 3,
        },
    );
    let staged_epoch = if wrong_future_epoch {
        argument(2)
    } else {
        local(5)
    };
    let staged_slot = if wrong_future_epoch {
        local(7)
    } else {
        local(6)
    };
    let body = ProductionRankedBlockV1::with_index_arguments(
        1,
        vec![
            ProductionRankedOperationV1::IndexBinary {
                result: id(5),
                kind: IndexBinaryKindAttr::Add,
                lhs: argument(2),
                rhs: local(3),
            },
            ProductionRankedOperationV1::IndexBinary {
                result: id(6),
                kind: IndexBinaryKindAttr::Remainder,
                lhs: local(5),
                rhs: local(4),
            },
            ProductionRankedOperationV1::IndexBinary {
                result: id(7),
                kind: IndexBinaryKindAttr::Remainder,
                lhs: argument(2),
                rhs: local(4),
            },
            event(staged_epoch, staged_slot, PipelineEventKindAttr::Stage),
            event(staged_epoch, staged_slot, PipelineEventKindAttr::Commit),
            event(argument(2), local(7), PipelineEventKindAttr::Wait),
            event(argument(2), local(7), PipelineEventKindAttr::Consume),
            event(argument(2), local(7), PipelineEventKindAttr::Release),
        ],
        ProductionRankedTerminatorV1::BranchArgsAdd {
            value: argument(2),
            step: local(3),
            target: 1,
        },
    );
    let exit = ProductionRankedBlockV1::new(
        vec![
            ProductionRankedOperationV1::IndexBinary {
                result: id(8),
                kind: IndexBinaryKindAttr::Remainder,
                lhs: ProductionRankedValueV1::Argument(0),
                rhs: local(4),
            },
            event(
                ProductionRankedValueV1::Argument(0),
                local(8),
                PipelineEventKindAttr::Wait,
            ),
            event(
                ProductionRankedValueV1::Argument(0),
                local(8),
                PipelineEventKindAttr::Discard,
            ),
            event(
                ProductionRankedValueV1::Argument(0),
                local(8),
                PipelineEventKindAttr::Release,
            ),
        ],
        ProductionRankedTerminatorV1::Return,
    );
    ProductionRankedKernelV1::new(
        "production_dynamic_pipeline",
        1,
        vec![entry, header, body, exit],
    )
    .expect("well-scoped runtime loop recipe")
}

#[test]
fn dynamic_pipeline_materializes_and_passes_the_unified_production_pipeline() {
    let input = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel(
            "production_dynamic_pipeline_module",
            dynamic_double_buffer(false),
        )
        .expect("ranked construction"),
        ProductionSessionLimitsV1::default(),
    )
    .expect("symbolic pipeline is accepted before lowering");
    assert!(input.all_mandatory_reports_are_clean());
    let [certificate] = input.pipeline_protocol_report().certificates() else {
        panic!("expected one pipeline certificate");
    };
    assert_eq!(certificate.buffers(), 2);
    assert_eq!(certificate.prefetch_distance(), 1);
    assert!(certificate.dynamic_loop().is_some());
}

#[test]
fn production_pipeline_rejects_a_loop_with_the_wrong_future_epoch() {
    let error = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel(
            "production_bad_pipeline_module",
            dynamic_double_buffer(true),
        )
        .expect("ranked construction"),
        ProductionSessionLimitsV1::default(),
    )
    .expect_err("wrong epoch must fail before lowering");
    let session = match error {
        fe2o3_pliron::ProductionRankedCompileErrorV1::Session(error) => error,
        other => panic!("unexpected compile error: {other}"),
    };
    assert!(matches!(
        session,
        ProductionSessionErrorV1::RankedPipeline(_)
    ));
    let repair = session.repair_hints();
    assert_eq!(repair.len(), 1);
    assert_eq!(repair[0].pass(), KernelCheckPassKindV1::PipelineProtocol);
}
