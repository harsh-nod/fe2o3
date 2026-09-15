use dialect_kernel::{AccessKindAttr, MemorySpaceAttr, PipelineEventKindAttr};
use fe2o3_kernel_analysis::KernelCheckPassKindV1;
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedBlockV1 as Block, ProductionRankedCompileErrorV1,
    ProductionRankedKernelV1, ProductionRankedOperationV1 as Op,
    ProductionRankedTerminatorV1 as End, ProductionRankedValueIdV1 as Id,
    ProductionRankedValueV1 as Value, ProductionSessionErrorV1, ProductionSessionLimitsV1,
    compile_ranked_kernel_for_lowering_v1,
};

#[derive(Clone, Copy, Debug)]
enum Case {
    LinearChain,
    LinearAccess,
    EmptyDiamond,
    DuplicateSuccessor,
    NonTopologicalOrder,
    BeforeLifecycleTrap,
    NormalBypass,
    BranchOnlyAccess,
    EqualBranchCopies,
    Cycle,
    PartialLifecycleTrap,
}

fn local(index: u32) -> Value {
    Value::Local(Id::new(index))
}

fn event(kind: PipelineEventKindAttr) -> Op {
    Op::PipelineEvent {
        pipeline: local(1),
        epoch: local(2),
        slot: local(2),
        kind,
    }
}

fn access(kind: AccessKindAttr) -> Op {
    Op::Access {
        kind,
        view: local(0),
        indices: vec![local(2), local(4)],
    }
}

fn entry_operations() -> Vec<Op> {
    vec![
        Op::ExecutionLayout {
            grid_identity: 7,
            global_extents: [64, 1, 1],
            workgroup_extents: [64, 1, 1],
            subgroup_size: 64,
            full_physical_workgroups: true,
        },
        Op::ViewInSpace {
            result: Id::new(0),
            element_width: 16,
            writable: true,
            shape: vec![2, 64],
            dynamic_extents: vec![],
            memory_space: MemorySpaceAttr::Workgroup,
            allocation_origin: 301,
            noalias_class: 41,
        },
        Op::PipelineCreate {
            result: Id::new(1),
            view: local(0),
            buffers: 2,
            prefetch_distance: 1,
        },
        Op::IndexConstant {
            result: Id::new(2),
            value: 0,
        },
        Op::IndexConstant {
            result: Id::new(3),
            value: 1,
        },
        // Each lane touches only its own coordinate. The access-positive case
        // therefore has no unrelated cross-invocation race to obscure ordering.
        Op::InvocationIndex {
            result: Id::new(4),
            dimension: 0,
            launch_extent: 64,
        },
    ]
}

fn entry(end: End) -> Block {
    Block::new(entry_operations(), end)
}

fn branch(target: u32) -> End {
    End::Branch { target }
}

fn split(first_block: u32, second_block: u32) -> End {
    End::AnalysisSplit {
        control_dependencies: vec![],
        first_block,
        second_block,
    }
}

fn stage() -> Vec<Op> {
    vec![
        event(PipelineEventKindAttr::Stage),
        event(PipelineEventKindAttr::Commit),
    ]
}

fn finish() -> Vec<Op> {
    vec![
        event(PipelineEventKindAttr::Wait),
        event(PipelineEventKindAttr::Consume),
        event(PipelineEventKindAttr::Release),
    ]
}

fn complete_lifecycle() -> Vec<Op> {
    let mut operations = stage();
    operations.extend(finish());
    operations
}

fn recipe(case: Case) -> ProductionRankedKernelV1 {
    let blocks = match case {
        Case::LinearChain => vec![
            entry(branch(1)),
            Block::new(stage(), branch(2)),
            Block::new(
                vec![
                    event(PipelineEventKindAttr::Wait),
                    event(PipelineEventKindAttr::Consume),
                ],
                branch(3),
            ),
            Block::new(vec![event(PipelineEventKindAttr::Release)], End::Return),
        ],
        Case::LinearAccess => vec![
            entry(branch(1)),
            Block::new(
                vec![
                    event(PipelineEventKindAttr::Stage),
                    access(AccessKindAttr::Write),
                    event(PipelineEventKindAttr::Commit),
                ],
                branch(2),
            ),
            Block::new(
                vec![
                    event(PipelineEventKindAttr::Wait),
                    event(PipelineEventKindAttr::Consume),
                    access(AccessKindAttr::Read),
                ],
                branch(3),
            ),
            Block::new(vec![event(PipelineEventKindAttr::Release)], End::Return),
        ],
        Case::EmptyDiamond => {
            let mut tail = vec![event(PipelineEventKindAttr::Commit)];
            tail.extend(finish());
            vec![
                entry(branch(1)),
                Block::new(vec![event(PipelineEventKindAttr::Stage)], split(2, 3)),
                Block::new(vec![], branch(4)),
                Block::new(vec![], branch(4)),
                Block::new(tail, End::Return),
            ]
        }
        Case::DuplicateSuccessor => vec![
            entry(End::AnalysisSplitArgs {
                control_dependencies: vec![],
                first_arguments: vec![local(2)],
                second_arguments: vec![local(3)],
                first_block: 1,
                second_block: 1,
            }),
            // The two payloads are deliberately different and unused. The
            // actual epoch/slot remain direct constants, not selected edge data.
            Block::with_index_arguments(1, stage(), branch(2)),
            Block::new(finish(), End::Return),
        ],
        Case::NonTopologicalOrder => vec![
            entry(branch(2)),
            Block::new(finish(), End::Return),
            Block::new(stage(), branch(1)),
        ],
        Case::BeforeLifecycleTrap => vec![
            entry(split(1, 3)),
            Block::new(stage(), branch(2)),
            Block::new(finish(), End::Return),
            Block::new(vec![], End::Trap),
        ],
        Case::NormalBypass => vec![
            entry(split(1, 3)),
            Block::new(stage(), branch(2)),
            Block::new(finish(), End::Return),
            Block::new(vec![], End::Return),
        ],
        Case::BranchOnlyAccess => {
            let mut initial = entry_operations();
            initial.push(event(PipelineEventKindAttr::Stage));
            let mut tail = vec![event(PipelineEventKindAttr::Commit)];
            tail.extend(finish());
            vec![
                Block::new(initial, split(1, 2)),
                Block::new(vec![access(AccessKindAttr::Write)], branch(3)),
                Block::new(vec![], branch(3)),
                Block::new(tail, End::Return),
            ]
        }
        Case::EqualBranchCopies => vec![
            entry(split(1, 2)),
            Block::new(complete_lifecycle(), End::Return),
            Block::new(complete_lifecycle(), End::Return),
        ],
        Case::Cycle => vec![
            entry(branch(1)),
            Block::new(stage(), branch(2)),
            Block::new(finish(), split(1, 3)),
            Block::new(vec![], End::Return),
        ],
        Case::PartialLifecycleTrap => vec![
            entry(branch(1)),
            Block::new(stage(), split(2, 3)),
            Block::new(finish(), End::Return),
            Block::new(vec![], End::Trap),
        ],
    };
    let block_count = blocks.len();
    let kernel = ProductionRankedKernelV1::new("production_static_pipeline_cfg", 0, blocks)
        .unwrap_or_else(|error| panic!("{case:?} recipe admission: {error}"));
    // Recipe normalization is position preserving; success must not merely
    // result from flattening the cross-block fixture before protocol checking.
    assert_eq!(kernel.blocks().len(), block_count);
    assert!(kernel.blocks()[1..].iter().any(|block| {
        block
            .operations()
            .iter()
            .any(|operation| matches!(operation, Op::PipelineEvent { .. }))
    }));
    kernel
}

#[test]
fn static_cfg_lifecycles_pass_the_actual_public_production_pipeline() {
    for case in [
        Case::LinearChain,
        Case::LinearAccess,
        Case::EmptyDiamond,
        Case::DuplicateSuccessor,
        Case::NonTopologicalOrder,
        Case::BeforeLifecycleTrap,
    ] {
        let kernel = recipe(case);
        let creation_operation = kernel.blocks()[0]
            .operations()
            .iter()
            .position(|operation| matches!(operation, Op::PipelineCreate { .. }))
            .unwrap();
        let input = compile_ranked_kernel_for_lowering_v1(
            ProductionConstructionV1::ranked_kernel("static_pipeline_cfg_module", kernel).unwrap(),
            ProductionSessionLimitsV1::default(),
        )
        .unwrap_or_else(|error| panic!("{case:?} production admission: {error}"));
        assert!(input.all_mandatory_reports_are_clean(), "{case:?}");
        let report = input.pipeline_protocol_report();
        let [certificate] = report.certificates() else {
            panic!("{case:?}: expected one exact pipeline certificate");
        };
        assert_eq!(certificate.pipeline_block(), 0, "{case:?}");
        assert_eq!(
            certificate.pipeline_operation(),
            creation_operation,
            "{case:?}"
        );
        assert_eq!(certificate.buffers(), 2, "{case:?}");
        assert_eq!(certificate.prefetch_distance(), 1, "{case:?}");
        assert_eq!(certificate.concrete_epochs(), 1, "{case:?}");
        let accesses = usize::from(matches!(case, Case::LinearAccess));
        assert_eq!(certificate.staged_writes(), accesses, "{case:?}");
        assert_eq!(certificate.consuming_reads(), accesses, "{case:?}");
        assert!(certificate.access_refinement_proven(), "{case:?}");
        assert!(certificate.dynamic_loop().is_none(), "{case:?}");
        assert!(!report.grants_compiler_refinement_authority(), "{case:?}");
        assert!(!report.grants_artifact_or_launch_authority(), "{case:?}");
    }
}

#[test]
fn nonunique_or_partial_static_cfg_lifecycles_fail_the_pipeline_protocol_pass() {
    for case in [
        Case::NormalBypass,
        Case::BranchOnlyAccess,
        Case::EqualBranchCopies,
        Case::Cycle,
        Case::PartialLifecycleTrap,
    ] {
        let error = compile_ranked_kernel_for_lowering_v1(
            ProductionConstructionV1::ranked_kernel("bad_static_pipeline_cfg_module", recipe(case))
                .unwrap(),
            ProductionSessionLimitsV1::default(),
        )
        .err()
        .unwrap_or_else(|| panic!("{case:?}: unsafe or unproved schedule was accepted"));
        let ProductionRankedCompileErrorV1::Session(session) = error else {
            panic!("{case:?}: unrelated recipe failure: {error}");
        };
        assert!(matches!(
            session,
            ProductionSessionErrorV1::RankedPipeline(_)
        ));
        let repair = session.repair_hints();
        assert_eq!(repair.len(), 1, "{case:?}: {session}");
        assert_eq!(
            repair[0].pass(),
            KernelCheckPassKindV1::PipelineProtocol,
            "{case:?}: {session}"
        );
        assert!(
            session.to_string().contains("FE2O3-PIPELINE-001"),
            "{case:?}: {session}"
        );
    }
}
