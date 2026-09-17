use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BarrierSemantics, BasicBlock, BlockId,
    CanonicalKernelIrWorkBudgetV1 as Work, Constant, CopyNonOverlappingContract, Fence,
    Function as KirFunction, MemoryAccess, MemoryElementType, MemoryIntrinsicOperation,
    MemoryLayout, MemoryOrdering, Module, Operation as KirOperation, ScalarType, Signature,
    SynchronizationScope, Terminator, Type, ValueDef, ValueId, VerificationContractKeyV12,
    VerificationContractOperationV12, VerifiedCanonicalKernelIrModuleV12 as Owner,
    VolatileAccessContract, WorkgroupPipelineEventKindV12,
};

const WORK: usize = 100_000_000;
const STORAGE: usize = 64 * 1024 * 1024;
const FLOOR: usize = 31;

fn scalar() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn block_id(block: u32) -> Block {
    Block {
        function: Function(0),
        block,
    }
}
fn operation_id(block: u32, operation: u32) -> Operation {
    Operation {
        block: block_id(block),
        operation,
    }
}
fn store() -> KirOperation {
    KirOperation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(0),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )
}
fn load(value: u32) -> KirOperation {
    KirOperation::effect_free(
        ValueDef::new(ValueId(value), scalar()),
        OperationKind::Load {
            pointer: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )
}
fn block(id: u32, operations: Vec<KirOperation>, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}
fn module(blocks: Vec<BasicBlock>) -> Module {
    let mut module = Module::new("memory-ssa");
    module.functions.push(KirFunction::internal_helper(
        "f",
        Signature::new(
            vec![
                Type::pointer(scalar(), AddressSpace::Global, AccessMode::ReadWrite),
                scalar(),
                Type::BOOL,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        blocks,
    ));
    module
}
fn linear(operations: Vec<KirOperation>) -> Module {
    module(vec![block(
        91,
        operations,
        Terminator::Return { values: vec![] },
    )])
}
fn branch(target: u32) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    }
}
fn choice(yes: u32, no: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    }
}
fn diamond() -> Module {
    module(vec![
        block(90, vec![], choice(5, 400)),
        block(5, vec![store()], branch(7)),
        block(400, vec![store()], branch(7)),
        block(7, vec![load(3)], Terminator::Return { values: vec![] }),
    ])
}

fn with_inventory(
    module: Module,
    next: impl FnOnce(&CanonicalKirInventoryV1<'_>, &mut Budget<'_>),
) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, owner_storage) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget)
            .expect("fixture must verify before memory analysis");
    budget
        .reserve_storage(owner_storage.retained_storage())
        .unwrap();
    drop(module);
    let (inventory, inventory_storage) =
        CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    next(&inventory, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    drop(owner);
    budget
        .release_storage(owner_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

fn with_report(
    module: Module,
    next: impl FnOnce(&CanonicalKirMemorySsaV1<'_, '_>, &mut Budget<'_>),
) {
    with_inventory(module, |inventory, budget| {
        let incoming = budget.storage();
        let (report, storage) =
            CanonicalKirMemorySsaV1::derive(inventory, Default::default(), budget).unwrap();
        assert_eq!(budget.storage(), incoming);
        assert_eq!(
            storage.retained_storage(),
            size_of::<CanonicalKirMemorySsaV1<'_, '_>>()
                + report.nodes.capacity() * size_of::<Node>()
                + report.inputs.capacity() * size_of::<Input>()
                + report.blocks.capacity() * size_of::<BlockState>()
                + report.operations.capacity() * size_of::<Option<NodeId>>()
        );
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let floor = budget.storage();
        next(&report, budget);
        assert_eq!(budget.storage(), floor);
        drop(report);
        budget.release_storage(storage.retained_storage()).unwrap();
    });
}

fn operation(
    report: &CanonicalKirMemorySsaV1<'_, '_>,
    block: u32,
    operation: u32,
    budget: &mut Budget<'_>,
) -> NodeId {
    report
        .operation(operation_id(block, operation), budget)
        .unwrap()
        .unwrap()
}

#[test]
fn stores_define_versions_reads_reuse_them_and_pure_operations_have_no_node() {
    let pure = KirOperation::effect_free(
        ValueDef::new(ValueId(3), scalar()),
        OperationKind::Constant(Constant::U32(7)),
    );
    with_report(
        linear(vec![pure, store(), load(4), load(5)]),
        |report, budget| {
            assert!(report.belongs_to(report.inventory()));
            assert_eq!(report.node_count(), 5);
            let phi = report.block_entry(block_id(0), budget).unwrap();
            let inputs = report.phi_inputs(phi, budget).unwrap();
            assert_eq!(inputs.len(), 1);
            assert_eq!(inputs[0].source(), InputSource::Entry(Function(0)));
            assert!(
                matches!(report.node(inputs[0].state(), budget).unwrap(), Node::LiveOnEntry { function } if *function == Function(0))
            );
            assert_eq!(report.operation(operation_id(0, 0), budget).unwrap(), None);
            let stored = operation(report, 0, 1, budget);
            assert!(
                matches!(report.node(stored, budget).unwrap(), Node::Def { incoming, .. } if *incoming == phi)
            );
            for offset in [2, 3] {
                let read = operation(report, 0, offset, budget);
                assert!(
                    matches!(report.node(read, budget).unwrap(), Node::Use { incoming, .. } if *incoming == stored)
                );
            }
            assert_eq!(report.block_exit(block_id(0), budget).unwrap(), stored);
        },
    );
}

#[test]
fn diamond_joins_distinct_stores_in_edge_order_with_nonmonotonic_block_ids() {
    with_report(diamond(), |report, budget| {
        let phi = report.block_entry(block_id(3), budget).unwrap();
        let left = operation(report, 1, 0, budget);
        let right = operation(report, 2, 0, budget);
        assert_ne!(left, right);
        let incoming = report.phi_inputs(phi, budget).unwrap();
        assert_eq!(
            incoming,
            [
                Input {
                    source: InputSource::Edge(Edge {
                        source: block_id(1),
                        successor: 0
                    }),
                    state: left
                },
                Input {
                    source: InputSource::Edge(Edge {
                        source: block_id(2),
                        successor: 0
                    }),
                    state: right
                },
            ]
        );
        let read = operation(report, 3, 0, budget);
        assert!(
            matches!(report.node(read, budget).unwrap(), Node::Use { incoming, .. } if *incoming == phi)
        );
    });
}

#[test]
fn duplicate_successor_edges_are_not_collapsed() {
    let input = module(vec![
        block(900, vec![store()], choice(4, 4)),
        block(4, vec![load(3)], Terminator::Return { values: vec![] }),
    ]);
    with_report(input, |report, budget| {
        let stored = operation(report, 0, 0, budget);
        let phi = report.block_entry(block_id(1), budget).unwrap();
        assert_eq!(
            report.phi_inputs(phi, budget).unwrap(),
            [
                Input {
                    source: InputSource::Edge(Edge {
                        source: block_id(0),
                        successor: 0
                    }),
                    state: stored
                },
                Input {
                    source: InputSource::Edge(Edge {
                        source: block_id(0),
                        successor: 1
                    }),
                    state: stored
                },
            ]
        );
    });
}

#[test]
fn entry_backedge_keeps_explicit_live_on_entry_and_loop_carried_store() {
    let input = module(vec![
        block(77, vec![load(3), store()], choice(77, 9)),
        block(9, vec![], Terminator::Return { values: vec![] }),
    ]);
    with_report(input, |report, budget| {
        let phi = report.block_entry(block_id(0), budget).unwrap();
        let read = operation(report, 0, 0, budget);
        let stored = operation(report, 0, 1, budget);
        assert!(
            matches!(report.node(read, budget).unwrap(), Node::Use { incoming, .. } if *incoming == phi)
        );
        let inputs = report.phi_inputs(phi, budget).unwrap();
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].source(), InputSource::Entry(Function(0)));
        assert!(matches!(
            report.node(inputs[0].state(), budget).unwrap(),
            Node::LiveOnEntry { .. }
        ));
        assert_eq!(
            inputs[1],
            Input {
                source: InputSource::Edge(Edge {
                    source: block_id(0),
                    successor: 0
                }),
                state: stored
            }
        );
    });
}

#[test]
fn irreducible_and_disconnected_components_keep_their_structural_phis() {
    let input = module(vec![
        block(100, vec![], choice(30, 20)),
        block(30, vec![store()], choice(20, 80)),
        block(20, vec![store()], choice(30, 80)),
        block(80, vec![], Terminator::Return { values: vec![] }),
        block(1, vec![], branch(1)),
        block(2, vec![], Terminator::Return { values: vec![] }),
    ]);
    with_report(input, |report, budget| {
        for target in [1, 2, 3] {
            let phi = report.block_entry(block_id(target), budget).unwrap();
            assert_eq!(report.phi_inputs(phi, budget).unwrap().len(), 2);
        }
        let detached = report.block_entry(block_id(4), budget).unwrap();
        assert_eq!(
            report.phi_inputs(detached, budget).unwrap(),
            [Input {
                source: InputSource::Edge(Edge {
                    source: block_id(4),
                    successor: 0
                }),
                state: detached,
            }]
        );
        let isolated = report.block_entry(block_id(5), budget).unwrap();
        assert!(report.phi_inputs(isolated, budget).unwrap().is_empty());
    });
}

#[test]
fn calls_volatile_reads_and_fences_are_barriers_without_relabeling_raw_effects() {
    let volatile = KirOperation::effect_free(
        ValueDef::new(ValueId(4), scalar()),
        OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
            pointer: ValueId(0),
            element: MemoryElementType::Scalar(ScalarType::U32),
            address_space: AddressSpace::Global,
            layout: MemoryLayout::new(4, 4),
            contract: VolatileAccessContract::rust_allocation_load(),
        }),
    );
    let fence = KirOperation::new(
        vec![],
        OperationKind::Fence(Fence {
            memory_scope: SynchronizationScope::Device,
            semantics: BarrierSemantics::new(MemoryOrdering::Release, [AddressSpace::Global]),
        }),
    );
    let call = KirOperation::new(
        vec![],
        OperationKind::Call {
            callee: "external".into(),
            arguments: vec![],
        },
    );
    let mut input = linear(vec![call, load(3), volatile, fence, load(5)]);
    input.functions.push(KirFunction::declaration(
        "external",
        Signature::new(vec![], vec![]),
    ));
    with_report(input, |report, budget| {
        assert!(report.inventory().operations()[0].effects.is_empty());
        for index in [0, 2, 3] {
            let node = operation(report, 0, index, budget);
            assert!(matches!(
                report.node(node, budget).unwrap(),
                Node::Def { .. }
            ));
        }
        let call = operation(report, 0, 0, budget);
        let first_read = operation(report, 0, 1, budget);
        assert!(
            matches!(report.node(first_read, budget).unwrap(), Node::Use { incoming, .. } if *incoming == call)
        );
        let fence = operation(report, 0, 3, budget);
        let last_read = operation(report, 0, 4, budget);
        assert!(
            matches!(report.node(last_read, budget).unwrap(), Node::Use { incoming, .. } if *incoming == fence)
        );
    });
}

#[test]
fn ordered_contract_without_physical_effects_is_still_a_barrier() {
    let ordered = KirOperation::new(
        vec![],
        OperationKind::VerificationContract(
            VerificationContractOperationV12::WorkgroupPipelineEvent {
                contract: VerificationContractKeyV12::new(11),
                kind: WorkgroupPipelineEventKindV12::Stage,
                storage: ValueId(0),
                epoch: ValueId(1),
            },
        ),
    );
    let mut input = Module::new("memory-ssa-ordering");
    input.functions.push(KirFunction::internal_helper(
        "ordered",
        Signature::new(
            vec![
                Type::pointer(scalar(), AddressSpace::Workgroup, AccessMode::ReadWrite),
                Type::INDEX,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block(
            99,
            vec![ordered],
            Terminator::Return { values: vec![] },
        )],
    ));
    with_report(input, |report, budget| {
        assert!(report.inventory().effects().is_empty());
        let node = operation(report, 0, 0, budget);
        assert!(matches!(
            report.node(node, budget).unwrap(),
            Node::Def { .. }
        ));
    });
}

#[test]
fn verified_nomem_assembly_is_a_conservative_barrier_not_a_raw_write() {
    use fe2o3_kernel_ir::{
        AssemblyConstraint, AssemblyOperand, AssemblyOperandKind, AssemblyOption,
        AssemblySourceIdentity, InlineAssembly, InlineAssemblyTarget,
    };
    let assembly = KirOperation::new(
        vec![],
        OperationKind::InlineAssembly(InlineAssembly {
            target: InlineAssemblyTarget::AmdGpuGfx942,
            source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
            mnemonic: "s_nop".into(),
            operands: vec![AssemblyOperand {
                kind: AssemblyOperandKind::ImmediateI32(0),
                constraint: AssemblyConstraint::ImmediateI32,
            }],
            options: [AssemblyOption::NoMemory, AssemblyOption::NoStack].into(),
            declared_effects: Default::default(),
        }),
    );
    let capabilities = assembly.required_capabilities();
    let mut input = linear(vec![assembly, load(3)]);
    input
        .required_capabilities
        .extend(capabilities.iter().cloned());
    input.functions[0]
        .required_capabilities
        .extend(capabilities);
    with_report(input, |report, budget| {
        assert!(report.inventory().operations()[0].effects.is_empty());
        let barrier = operation(report, 0, 0, budget);
        assert!(matches!(
            report.node(barrier, budget).unwrap(),
            Node::Def { .. }
        ));
        let read = operation(report, 0, 1, budget);
        assert!(
            matches!(report.node(read, budget).unwrap(), Node::Use { incoming, .. } if *incoming == barrier)
        );
    });
}

#[test]
fn genuine_execution_operation_is_refused_before_v12_inventory_admission() {
    use fe2o3_kernel_ir::{
        ExecutionOperationV15, ExecutionRoleV15, Kernel, KernelIrEncodeError, LaunchDomain,
        LaunchExtent,
    };
    let mut input = Module::new("memory-ssa-execution-boundary");
    input.functions.push(KirFunction::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block(
            7,
            vec![KirOperation::effect_free(
                ValueDef::new(ValueId(0), Type::Execution(ExecutionRoleV15::Context)),
                OperationKind::Execution(ExecutionOperationV15::ContextIssue),
            )],
            Terminator::Return { values: vec![] },
        )],
    ));
    input.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    fe2o3_kernel_ir::verify_module_ref(&input).unwrap();
    // This is the real wire boundary, not a fabricated V12 owner or an executed
    // MemorySSA Execution classifier branch.
    assert!(matches!(
        fe2o3_kernel_ir::encode_module_v12(&input),
        Err(KernelIrEncodeError::UnsupportedInVersion {
            version: 12,
            feature: "execution lifecycle"
        })
    ));
}

#[test]
fn multi_effect_copy_is_one_clobber_with_all_physical_effects_censused() {
    let private = Type::pointer(scalar(), AddressSpace::Private, AccessMode::ReadWrite);
    let allocation = |value| {
        KirOperation::effect_free(
            ValueDef::new(ValueId(value), private.clone()),
            OperationKind::Alloca {
                element: scalar(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        )
    };
    let input = linear(vec![
        allocation(3),
        allocation(4),
        KirOperation::effect_free(
            ValueDef::new(ValueId(5), Type::INDEX),
            OperationKind::Constant(Constant::Index(1)),
        ),
        KirOperation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping {
                source: ValueId(3),
                destination: ValueId(4),
                count: ValueId(5),
                element: MemoryElementType::Scalar(ScalarType::U32),
                source_address_space: AddressSpace::Private,
                destination_address_space: AddressSpace::Private,
                layout: MemoryLayout::new(4, 4),
                contract: CopyNonOverlappingContract::supported_rust(),
            }),
        ),
    ]);
    with_report(input, |report, budget| {
        assert_eq!(report.inventory().operations()[3].effects.len(), 2);
        assert_eq!(report.inventory().effects().len(), 4);
        let copy = operation(report, 0, 3, budget);
        let previous = operation(report, 0, 1, budget);
        assert!(
            matches!(report.node(copy, budget).unwrap(), Node::Def { incoming, .. } if *incoming == previous)
        );
        assert_eq!(report.node_count(), 5);
    });
}

#[test]
fn fixed_constructor_work_formula_and_one_short_final_debit_are_literal() {
    // 40 + 11F + 5D + 14B + 11O + 4P + 22E + M. No measured calibration.
    for (input, expected) in [
        (Module::new("empty"), 40),
        (linear(vec![]), 70),
        (linear(vec![store()]), 86),
        (linear(vec![store(), load(3)]), 102),
        (diamond(), 248),
    ] {
        with_inventory(input, |inventory, parent| {
            let floor = parent.storage();
            for limit in [expected - 1, expected] {
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, STORAGE);
                budget.reserve_storage(floor).unwrap();
                let result =
                    CanonicalKirMemorySsaV1::derive(inventory, Default::default(), &mut budget);
                assert_eq!(budget.storage(), floor);
                if limit == expected {
                    let (report, storage) = result.unwrap();
                    assert_eq!(budget.work(), expected);
                    assert!(budget.peak_storage() >= floor + storage.retained_storage());
                    drop(report);
                    assert_eq!(work.failed_work(), None);
                } else {
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(error)))
                        if error.actual() == expected && error.limit() == expected - 1));
                    assert_eq!(budget.work(), expected - 9);
                    assert_eq!(work.failed_work(), Some(expected));
                }
            }
        });
    }
}

#[test]
fn header_and_partial_buffer_storage_fail_before_growth_with_exact_history() {
    with_inventory(linear(vec![]), |inventory, parent| {
        let floor = parent.storage();
        let header = size_of::<CanonicalKirMemorySsaV1<'_, '_>>();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, floor + header - 1);
        budget.reserve_storage(floor).unwrap();
        assert!(
            matches!(CanonicalKirMemorySsaV1::derive(inventory, Default::default(), &mut budget),
            Err(Error::Resource(Resource::Storage(error)))
                if error.actual() == floor + header && error.limit() == floor + header - 1)
        );
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (13, floor, floor)
        );
        assert_eq!(budget.failed_storage(), Some(floor + header));
    });
    // The actual first allocation remains live while the second requested
    // reservation fails. This does not inject allocator-dependent spare capacity.
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (nodes, bytes) = vector::<Node>(2, &mut budget).unwrap();
    assert_eq!(bytes, nodes.capacity() * size_of::<Node>());
    let mut denied_work = Work::new(WORK);
    let mut denied = Budget::new(&mut denied_work, FLOOR + bytes);
    denied.reserve_storage(FLOOR + bytes).unwrap();
    budget.release_storage(bytes).unwrap(); // Test-only explicit reservation transfer.
    assert!(
        matches!(vector::<Input>(1, &mut denied), Err(Error::Resource(Resource::Storage(error)))
        if error.actual() == FLOOR + bytes + size_of::<Input>() && error.limit() == FLOOR + bytes)
    );
    assert_eq!(
        (denied.work(), denied.storage(), denied.peak_storage()),
        (2, FLOOR + bytes, FLOOR + bytes)
    );
    assert_eq!(
        denied.failed_storage(),
        Some(FLOOR + bytes + size_of::<Input>())
    );
    drop(nodes);
    denied.release_storage(bytes).unwrap();
    assert_eq!(denied.storage(), FLOOR);
}

#[test]
fn constructor_failure_after_first_buffer_restores_the_original_floor() {
    with_inventory(linear(vec![]), |inventory, parent| {
        let floor = parent.storage();
        let mut work = Work::new(17);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        assert!(
            matches!(CanonicalKirMemorySsaV1::derive(inventory, Default::default(), &mut budget),
            Err(Error::Resource(Resource::Work(error))) if error.actual() == 18 && error.limit() == 17)
        );
        assert_eq!(budget.work(), 16); // Header13, then the complete first vector3.
        assert_eq!(budget.storage(), floor);
        assert!(
            budget.peak_storage()
                >= floor + size_of::<CanonicalKirMemorySsaV1<'_, '_>>() + 2 * size_of::<Node>()
        );
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(work.failed_work(), Some(18));
    });
}

#[test]
fn all_input_limits_refuse_before_report_allocation() {
    with_inventory(diamond(), |inventory, parent| {
        for resource in [
            CanonicalKirMemorySsaResourceV1::Functions,
            CanonicalKirMemorySsaResourceV1::Blocks,
            CanonicalKirMemorySsaResourceV1::Operations,
            CanonicalKirMemorySsaResourceV1::Effects,
            CanonicalKirMemorySsaResourceV1::Edges,
        ] {
            let mut limits = CanonicalKirMemorySsaLimitsV1::default();
            let actual = match resource {
                CanonicalKirMemorySsaResourceV1::Functions => {
                    limits.functions = 0;
                    inventory.functions().len()
                }
                CanonicalKirMemorySsaResourceV1::Blocks => {
                    limits.blocks = 0;
                    inventory.blocks().len()
                }
                CanonicalKirMemorySsaResourceV1::Operations => {
                    limits.operations = 0;
                    inventory.operations().len()
                }
                CanonicalKirMemorySsaResourceV1::Effects => {
                    limits.effects = 0;
                    inventory.effects().len()
                }
                CanonicalKirMemorySsaResourceV1::Edges => {
                    limits.edges = 0;
                    inventory.edges().len()
                }
            };
            let mut work = Work::new(WORK);
            let floor = parent.storage();
            let mut budget = Budget::new(&mut work, floor);
            budget.reserve_storage(floor).unwrap();
            assert!(
                matches!(CanonicalKirMemorySsaV1::derive(inventory, limits, &mut budget),
                Err(Error::InputLimit { resource: found, actual: count, limit: 0 }) if found == resource && count == actual)
            );
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (6, floor, floor)
            );
        }
    });
}

#[test]
fn queries_pay_fixed_work_before_invalid_locator_or_short_limit() {
    with_report(linear(vec![store()]), |report, parent| {
        let phi = report.block_entry(block_id(0), parent).unwrap();
        for (cost, kind) in [(8, 0), (4, 1), (4, 2), (2, 3), (3, 4)] {
            for limit in [cost - 1, cost] {
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, 0);
                let result = match kind {
                    0 => report
                        .operation(operation_id(0, u32::MAX), &mut budget)
                        .map(|_| ()),
                    1 => report
                        .block_entry(block_id(u32::MAX), &mut budget)
                        .map(|_| ()),
                    2 => report
                        .block_exit(block_id(u32::MAX), &mut budget)
                        .map(|_| ()),
                    3 => report.node(NodeId(usize::MAX), &mut budget).map(|_| ()),
                    _ => report
                        .phi_inputs(NodeId(usize::MAX), &mut budget)
                        .map(|_| ()),
                };
                if limit < cost {
                    assert!(
                        matches!(result, Err(Error::Resource(Resource::Work(error))) if error.actual() == cost && error.limit() == limit)
                    );
                    assert_eq!(budget.work(), 0);
                } else {
                    assert!(matches!(
                        result,
                        Err(Error::InvalidOperation(_))
                            | Err(Error::InvalidBlock(_))
                            | Err(Error::InvalidNode(_))
                    ));
                    assert_eq!(budget.work(), cost);
                }
                assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
            }
        }
        let live = report.phi_inputs(phi, parent).unwrap()[0].state();
        assert_eq!(report.phi_inputs(live, parent), Err(Error::NotPhi(live)));
    });
}

#[test]
fn deterministic_rows_do_not_substitute_equal_bytes_from_another_owner() {
    // Diagnostic copies below are test observations, not retained analysis payload.
    let collect = || {
        let mut observed = None;
        with_report(diamond(), |report, _| {
            observed = Some((
                report.nodes.clone(),
                report.inputs.clone(),
                report.operations.clone(),
            ));
        });
        observed.unwrap()
    };
    assert_eq!(collect(), collect());
    with_inventory(diamond(), |first, budget| {
        let source = diamond();
        let (other, other_storage) =
            Owner::from_module_ref_with_verification_budget_v12(&source, budget).unwrap();
        budget
            .reserve_storage(other_storage.retained_storage())
            .unwrap();
        let (second, second_storage) = CanonicalKirInventoryV1::derive(&other, budget).unwrap();
        budget
            .reserve_storage(second_storage.retained_storage())
            .unwrap();
        assert_eq!(first.identity(), second.identity());
        let (report, storage) =
            CanonicalKirMemorySsaV1::derive(first, Default::default(), budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert!(report.belongs_to(first));
        assert!(!report.belongs_to(&second));
        assert!(std::ptr::eq(report.inventory().owner(), first.owner()));
        drop(report);
        budget.release_storage(storage.retained_storage()).unwrap();
        drop(second);
        budget
            .release_storage(second_storage.retained_storage())
            .unwrap();
        drop(other);
        budget
            .release_storage(other_storage.retained_storage())
            .unwrap();
    });
}
