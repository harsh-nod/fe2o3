use super::*;
use fe2o3_kernel_analysis::check_canonical_kir_aggregate_ssa_v18;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    Constant, FixedVectorTypeV12, Function, MemoryAccess, Module, Operation, OperationKind as Kind,
    ScalarType, Signature, StorageFieldV1, StorageLayoutIdV1, StorageLayoutKindV1 as LayoutKind,
    StorageLayoutLimitsV1, StorageLayoutV1, StorageOperationV1 as Storage,
    StorageProjectionV1 as Projection, Terminator, Type, ValueDef, ValueId, VectorLayoutV12,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use fe2o3_kernel_opt::{
    OwnedAggregateSsaContinuationV18 as Continuation, prepare_owned_aggregate_ssa_v18,
};

const WORK: usize = 500_000_000;
const STORAGE: usize = 256 * 1024 * 1024;
const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 16,
    edges: 32,
    containment_depth: 8,
    object_bytes: 4096,
};
fn scalar() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn pointer(layout: u32) -> Type {
    Type::pointer(
        Type::StorageObject(StorageLayoutIdV1(layout)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    )
}
fn op(id: u32, ty: Type, kind: Kind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn write(address: u32, value: u32) -> Operation {
    Operation::new(
        vec![],
        Kind::Storage(Storage::WriteValue {
            address: ValueId(address),
            value: ValueId(value),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        }),
    )
}
fn read(id: u32, address: u32) -> Operation {
    op(
        id,
        scalar(),
        Kind::Storage(Storage::ReadValue {
            address: ValueId(address),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        }),
    )
}
fn block(id: u32, operations: Vec<Operation>, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}
fn branch(target: u32) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    }
}
fn cond(a: u32, b: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(a),
        then_arguments: vec![],
        else_target: BlockId(b),
        else_arguments: vec![],
    }
}
fn ret() -> Terminator {
    Terminator::Return {
        values: vec![ValueId(20), ValueId(21)],
    }
}
fn setup() -> Vec<Operation> {
    vec![
        op(
            10,
            pointer(1),
            Kind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(1)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        op(
            13,
            pointer(0),
            Kind::Storage(Storage::Project {
                base: ValueId(10),
                step: Projection::Field(0),
            }),
        ),
        op(
            14,
            pointer(0),
            Kind::Storage(Storage::Project {
                base: ValueId(10),
                step: Projection::Field(1),
            }),
        ),
    ]
}
fn module(blocks: Vec<BasicBlock>) -> Module {
    let mut m = Module::new("aggregate-memory-cfg-obligations");
    m.storage_layouts = vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: LayoutKind::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 4,
            kind: LayoutKind::Record(
                vec![
                    StorageFieldV1 {
                        offset: 0,
                        layout: StorageLayoutIdV1(0),
                    },
                    StorageFieldV1 {
                        offset: 4,
                        layout: StorageLayoutIdV1(0),
                    },
                ]
                .into_boxed_slice(),
            ),
        },
    ];
    m.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![scalar(), scalar(), Type::BOOL, Type::INDEX],
            vec![scalar(), scalar()],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        blocks,
    ));
    m
}
fn straight(initialized: bool) -> Module {
    let mut operations = setup();
    if initialized {
        operations.push(write(13, 0));
    }
    operations.extend([write(14, 1), read(20, 13), read(21, 14)]);
    module(vec![block(100, operations, ret())])
}
fn diamond(parallel: bool) -> Module {
    let mut operations = setup();
    operations.push(write(14, 1));
    module(vec![
        block(100, operations, cond(101, 102)),
        block(
            101,
            vec![write(13, 0)],
            if parallel {
                cond(103, 103)
            } else {
                branch(103)
            },
        ),
        block(102, vec![write(13, 1)], branch(103)),
        block(103, vec![read(20, 13), read(21, 14)], ret()),
    ])
}
fn loop_fixture() -> Module {
    let mut operations = setup();
    operations.extend([write(13, 0), write(14, 1)]);
    module(vec![
        block(100, operations, branch(101)),
        block(101, vec![read(20, 13)], cond(102, 103)),
        block(
            102,
            vec![
                op(
                    22,
                    scalar(),
                    Kind::Binary {
                        op: BinaryOp::Add,
                        lhs: ValueId(20),
                        rhs: ValueId(1),
                    },
                ),
                write(13, 22),
            ],
            branch(101),
        ),
        block(103, vec![read(21, 13)], ret()),
    ])
}
fn with_case<R>(
    module: Module,
    run: impl FnOnce(&Owner, &Continuation, usize, &mut Budget<'_>) -> R,
) -> R {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (input, receipt) =
        Owner::from_module_ref_with_verification_budget_v18(&module, LAYOUTS, &mut budget).unwrap();
    budget
        .reserve_storage(receipt.retained_storage() + 37)
        .unwrap();
    let continuation = prepare_owned_aggregate_ssa_v18(&input, LAYOUTS, &mut budget).unwrap();
    budget
        .reserve_storage(continuation.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let result = run(&input, &continuation, floor, &mut budget);
    assert_eq!(budget.storage(), floor);
    result
}
fn pair<'a>(
    input: &'a Owner,
    continuation: &'a Continuation,
    budget: &mut Budget<'_>,
) -> (Pair<'a>, usize) {
    let (pair, receipt) = check_canonical_kir_aggregate_ssa_v18(
        input,
        continuation.output(),
        continuation.witness(),
        budget,
    )
    .unwrap();
    (pair, receipt.retained_storage())
}
fn source(
    input: &Owner,
    continuation: &Continuation,
    budget: &mut Budget<'_>,
) -> (String, AggregateMemoryCfgSubjectV30) {
    let (pair, bytes) = pair(input, continuation, budget);
    budget.reserve_storage(bytes).unwrap();
    let floor = budget.storage();
    let request = prepare_aggregate_memory_cfg_obligation_v30(pair, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(request.retained_storage()).unwrap();
    assert!(std::ptr::eq(request.checked_pair().input(), input));
    assert!(std::ptr::eq(
        request.checked_pair().output(),
        continuation.output()
    ));
    assert!(std::ptr::eq(
        request.checked_pair().witness(),
        continuation.witness()
    ));
    assert!(!request.authenticates_executed_proof());
    assert!(!request.authenticates_original_source());
    assert!(!request.grants_artifact_or_launch_authority());
    let text = std::str::from_utf8(request.generated_source())
        .unwrap()
        .to_owned();
    let subject = request.subject();
    let retained = request.retained_storage();
    assert_eq!(
        retained,
        size_of::<AggregateMemoryCfgObligationV30<'_>>() + text.len()
    );
    drop(request);
    budget.release_storage(retained + bytes).unwrap();
    (text, subject)
}

#[test]
fn aggregate_memory_cfg_models_reset_store_read_and_concrete_select() {
    with_case(straight(true), |input, continuation, _, budget| {
        assert_eq!(continuation.promoted_allocations(), 1);
        let (text, subject) = source(input, continuation, budget);
        assert_eq!(subject.input(), *input.identity());
        assert_eq!(subject.output(), *continuation.output().identity());
        assert_eq!(subject.modeled_slots(), 2);
        assert!(text.contains("initialized.update(0, false)"));
        assert!(text.contains("private.update(0, base[0])"));
        assert!(text.contains("memory_valid && initialized[0]"));
        assert!(text.contains("select_value_v28("));
        assert!(text.contains("aggregate_snapshots_do_not_affect_original_step_v30"));
        assert!(text.contains("aggregate_function_trace_refinement_0_v30"));
        assert!(text.contains("aggregate_all_steps_refine_v30(op);"));
        assert!(!text.contains("assume("));
        assert!(!text.contains("external_body"));
    });
}

#[test]
fn aggregate_memory_cfg_diamond_parallel_edges_and_loop_bind_actual_phis() {
    for module in [diamond(false), diamond(true), loop_fixture()] {
        with_case(module, |input, continuation, _, budget| {
            assert!(continuation.inserted_parameters() > 0);
            let (text, subject) = source(input, continuation, budget);
            assert_eq!(
                subject.appended_parameters(),
                continuation.inserted_parameters()
            );
            assert!(text.contains("private[0]"));
            assert!(text.contains("n.initialized[0] && n.cells[0] == o.values["));
            assert!(text.contains("aggregate_block_refinement_3_v30"));
            assert!(text.contains("let next = next.update("));
        });
    }
}

#[test]
fn aggregate_memory_cfg_rejects_every_missing_read_phi_or_edge_cut_requirement() {
    for module in [diamond(true), loop_fixture()] {
        with_case(module, |input, continuation, floor, budget| {
            let (pair, bytes) = pair(input, continuation, budget);
            budget.reserve_storage(bytes).unwrap();
            let checked = budget
                .with_prepaid_scope(
                    budget.storage(),
                    1,
                    1,
                    SOURCE_LIMIT,
                    |budget| -> Result<usize> {
                        let (a, ar) = Inventory::derive_v18(input, budget)?;
                        budget.reserve_storage(ar.retained_storage())?;
                        let (b, br) = Inventory::derive_v18(continuation.output(), budget)?;
                        budget.reserve_storage(br.retained_storage())?;
                        let mut writer = Writer::new(budget)?;
                        semantics::check_aggregate_memory_omissions_v30(&a, &b, &pair, &mut writer)
                    },
                )
                .unwrap();
            assert!(checked > 0);
            drop(pair);
            budget.release_storage(bytes).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn aggregate_memory_cfg_preserves_typed_and_index_switch_edge_occurrences() {
    use fe2o3_kernel_ir::{IntegerSwitchCase, SwitchCase};
    for typed in [false, true] {
        let mut m = diamond(false);
        m.functions[0].body.as_mut().unwrap().blocks[1].terminator = Some(if typed {
            Terminator::IntegerSwitch {
                selector: ValueId(0),
                cases: vec![
                    IntegerSwitchCase {
                        value: Constant::U32(0),
                        target: BlockId(103),
                        arguments: vec![],
                    },
                    IntegerSwitchCase {
                        value: Constant::U32(1),
                        target: BlockId(103),
                        arguments: vec![],
                    },
                ],
                default_target: BlockId(103),
                default_arguments: vec![],
            }
        } else {
            Terminator::Switch {
                selector: ValueId(3),
                cases: vec![
                    SwitchCase {
                        value: 0,
                        target: BlockId(103),
                        arguments: vec![],
                    },
                    SwitchCase {
                        value: 1,
                        target: BlockId(103),
                        arguments: vec![],
                    },
                ],
                default_target: BlockId(103),
                default_arguments: vec![],
            }
        });
        with_case(m, |input, continuation, _, budget| {
            let (text, _) = source(input, continuation, budget);
            let source_block = text
                .split("spec fn aggregate_block_n_1")
                .nth(1)
                .unwrap()
                .split("spec fn aggregate_block_o_1")
                .next()
                .unwrap();
            assert_eq!(source_block.matches("events: seq![0int,").count(), 3);
            assert!(source_block.contains("== 0int"));
            assert!(source_block.contains("== 1int"));
        });
    }
}

#[test]
fn aggregate_memory_cfg_keeps_same_numbered_values_in_separate_function_owners() {
    let mut m = diamond(true);
    let mut second = loop_fixture().functions.remove(0);
    second.id = "second-independent-aggregate".into();
    m.functions.push(second);
    with_case(m, |input, continuation, _, budget| {
        assert_eq!(continuation.promoted_allocations(), 2);
        let (text, subject) = source(input, continuation, budget);
        assert_eq!(subject.modeled_blocks(), 8);
        assert!(text.contains("aggregate_function_trace_refinement_0_v30"));
        assert!(text.contains("aggregate_function_trace_refinement_1_v30"));
    });
}

#[test]
fn aggregate_memory_cfg_retains_uninitialized_allocation_as_shared_original_memory() {
    with_case(straight(false), |input, continuation, _, budget| {
        assert_eq!(continuation.promoted_allocations(), 0);
        let (text, subject) = source(input, continuation, budget);
        assert_eq!(subject.appended_parameters(), 0);
        assert!(!text.contains("initialized.update("));
        assert!(!text.contains("private.update("));
        assert!(text.contains("op("));
    });
}

#[test]
fn aggregate_memory_cfg_vector_leaf_copy_uses_concrete_typed_select() {
    let vector = FixedVectorTypeV12::new(
        ScalarType::U32,
        4,
        VectorLayoutV12::Interleaved { factor: 2 },
    );
    let ty = Type::Vector(vector);
    let mut m = Module::new("aggregate-memory-vector");
    m.storage_layouts.push(StorageLayoutV1 {
        size: 16,
        alignment: 16,
        kind: LayoutKind::Vector(vector),
    });
    m.functions.push(Function::internal_helper(
        "vector",
        Signature::new(vec![ty.clone()], vec![ty.clone()]),
        vec![ValueId(0)],
        vec![block(
            100,
            vec![
                op(
                    10,
                    pointer(0),
                    Kind::Alloca {
                        element: Type::StorageObject(StorageLayoutIdV1(0)),
                        count: None,
                        address_space: AddressSpace::Private,
                        alignment: 16,
                    },
                ),
                Operation::new(
                    vec![],
                    Kind::Storage(Storage::WriteValue {
                        address: ValueId(10),
                        value: ValueId(0),
                        access: MemoryAccess::new(AddressSpace::Private, 16),
                    }),
                ),
                op(
                    20,
                    ty,
                    Kind::Storage(Storage::ReadValue {
                        address: ValueId(10),
                        access: MemoryAccess::new(AddressSpace::Private, 16),
                    }),
                ),
            ],
            Terminator::Return {
                values: vec![ValueId(20)],
            },
        )],
    ));
    with_case(m, |input, continuation, _, budget| {
        assert_eq!(continuation.promoted_allocations(), 1);
        let (text, _) = source(input, continuation, budget);
        assert!(text.contains("select_value_v28("));
        assert!(text.contains("private.update(0, base[0])"));
    });
}

#[test]
fn aggregate_memory_cfg_exact_and_one_short_request_resources_restore_floor() {
    with_case(diamond(true), |input, continuation, owners, setup| {
        let run = |work, storage, setup: &mut Budget<'_>| {
            let (pair, bytes) = pair(input, continuation, setup);
            let mut work = Work::new(work);
            let mut budget = Budget::new(&mut work, storage);
            budget.reserve_storage(owners + bytes).unwrap();
            let floor = budget.storage();
            let result = prepare_aggregate_memory_cfg_obligation_v30(pair, &mut budget)
                .map(|request| drop(request));
            assert_eq!(budget.storage(), floor);
            (result, budget.work(), budget.peak_storage())
        };
        let (result, work, peak) = run(WORK, STORAGE, setup);
        result.unwrap();
        run(work, peak, setup).0.unwrap();
        assert!(matches!(
            run(work - 1, peak, setup).0,
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert!(matches!(
            run(work, peak - 1, setup).0,
            Err(Error::Resource(Resource::Storage(_)))
                | Err(Error::Inventory(CanonicalKirInventoryErrorV1::Resource(
                    Resource::Storage(_)
                )))
        ));
    });
}

#[test]
fn aggregate_memory_cfg_subject_binds_original_metadata_not_just_generated_text() {
    let first = with_case(straight(true), |input, continuation, _, budget| {
        source(input, continuation, budget)
    });
    let mut other = straight(true);
    other.id = "distinct-genuine-original-owner".into();
    let second = with_case(other, |input, continuation, _, budget| {
        source(input, continuation, budget)
    });
    assert_eq!(first.0, second.0);
    assert_ne!(first.1.input(), second.1.input());
    assert_ne!(first.1.statement_identity(), second.1.statement_identity());
}
