use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    FixedVectorTypeV12, Function, MemoryAccess, ScalarType, Signature, StorageCopyOverlapV1,
    StorageFieldV1, StorageLayoutIdV1, StorageLayoutKindV1 as LayoutKind, StorageLayoutV1,
    StorageOperationV1 as Storage, StorageProjectionV1 as Projection,
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
fn block(id: u32, operations: Vec<Operation>, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}
fn setup() -> Vec<Operation> {
    vec![
        op(
            10,
            pointer(2),
            Kind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(2)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        op(11, Type::INDEX, Kind::Constant(Constant::Index(1))),
        op(
            12,
            pointer(1),
            Kind::Storage(Storage::Project {
                base: ValueId(10),
                step: Projection::ArrayIndex(ValueId(11)),
            }),
        ),
        op(
            13,
            pointer(0),
            Kind::Storage(Storage::Project {
                base: ValueId(12),
                step: Projection::Field(0),
            }),
        ),
        op(
            14,
            pointer(0),
            Kind::Storage(Storage::Project {
                base: ValueId(12),
                step: Projection::Field(1),
            }),
        ),
    ]
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
fn ret() -> Terminator {
    Terminator::Return {
        values: vec![ValueId(20), ValueId(21)],
    }
}
fn module(blocks: Vec<BasicBlock>) -> Module {
    let mut m = Module::new("aggregate-secondary-ssa");
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
        StorageLayoutV1 {
            size: 16,
            alignment: 4,
            kind: LayoutKind::Array {
                element: StorageLayoutIdV1(1),
                length: 2,
                stride: 8,
            },
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
fn straight() -> Module {
    let mut operations = setup();
    operations.extend([write(13, 0), write(14, 1), read(20, 13), read(21, 14)]);
    module(vec![block(100, operations, ret())])
}
fn diamond(parallel: bool) -> Module {
    let mut operations = setup();
    operations.push(write(14, 1));
    let left = if parallel {
        cond(103, 103)
    } else {
        branch(103)
    };
    module(vec![
        block(100, operations, cond(101, 102)),
        block(101, vec![write(13, 0)], left),
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
fn blocks(m: &mut Module) -> &mut Vec<BasicBlock> {
    &mut m.functions[0].body.as_mut().unwrap().blocks
}
fn admit(m: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v18(m, LAYOUTS, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, receipt.retained_storage())
}
fn with_input(m: Module, run: impl FnOnce(&Owner, &mut Budget<'_>)) {
    let (owner, bytes) = admit(&m);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(bytes + 37).unwrap();
    let floor = budget.storage();
    run(&owner, &mut budget);
    assert_eq!(budget.storage(), floor);
}
fn release(out: OwnedAggregateSsaContinuationV18, budget: &mut Budget<'_>) {
    let bytes = out.retained_storage();
    drop(out);
    budget.release_storage(bytes).unwrap();
}
fn exercise(input: &Owner, budget: &mut Budget<'_>) -> OwnedAggregateSsaContinuationV18 {
    let floor = budget.storage();
    let out = prepare_owned_aggregate_ssa_v18(input, LAYOUTS, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(out.retained_storage()).unwrap();
    out.replay_against(input, budget).unwrap();
    assert!(!out.grants_authority());
    out
}
fn noop(m: Module) {
    with_input(m, |input, budget| {
        let out = exercise(input, budget);
        assert_eq!(out.promoted_allocations(), 0);
        assert_eq!(out.output().canonical_bytes(), input.canonical_bytes());
        release(out, budget);
    });
}

#[test]
fn aggregate_ssa_nested_array_record_keeps_distinct_dynamic_values_and_layouts() {
    with_input(straight(), |input, budget| {
        let out = exercise(input, budget);
        assert_eq!(out.promoted_allocations(), 1);
        assert_eq!(out.inserted_parameters(), 0);
        assert_eq!(
            out.output().module().storage_layouts,
            input.module().storage_layouts
        );
        let operations = &out.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks[0]
            .operations;
        for (id, value) in [(20, 0), (21, 1)] {
            let row = operations
                .iter()
                .find(|op| op.results.first().is_some_and(|r| r.id == ValueId(id)))
                .unwrap();
            assert!(
                matches!(row.kind, Kind::Select { true_value, false_value, .. } if true_value == ValueId(value) && false_value == ValueId(value))
            );
        }
        assert!(
            !operations
                .iter()
                .any(|op| matches!(op.kind, Kind::Alloca { .. } | Kind::Storage(_)))
        );
        release(out, budget);
    });
}
#[test]
fn aggregate_ssa_diamond_and_parallel_edges_transport_exact_store_values() {
    for parallel in [false, true] {
        with_input(diamond(parallel), |input, budget| {
            let out = exercise(input, budget);
            assert_eq!(out.promoted_allocations(), 1);
            assert!(out.inserted_parameters() > 0);
            let body = out.output().module().functions[0].body.as_ref().unwrap();
            let parameter = body.blocks[3]
                .parameters
                .iter()
                .find(|p| p.ty == scalar())
                .unwrap()
                .id;
            assert!(
                matches!(body.blocks[3].operations[0].kind, Kind::Select { true_value, false_value, .. } if true_value == parameter && false_value == parameter)
            );
            let left = body.blocks[1].terminator.as_ref().unwrap();
            assert_eq!(materialize::arguments(left, 0).unwrap()[0], ValueId(0));
            if parallel {
                assert_eq!(materialize::arguments(left, 1).unwrap()[0], ValueId(0));
            }
            assert_eq!(
                materialize::arguments(body.blocks[2].terminator.as_ref().unwrap(), 0).unwrap()[0],
                ValueId(1)
            );
            release(out, budget);
        });
    }
}
#[test]
fn aggregate_ssa_loop_reconstructs_header_and_backedge_with_original_arithmetic() {
    with_input(loop_fixture(), |input, budget| {
        let out = exercise(input, budget);
        let b = &out.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks;
        assert_eq!(out.promoted_allocations(), 1);
        assert!(out.inserted_parameters() > 0);
        assert_eq!(
            materialize::arguments(b[0].terminator.as_ref().unwrap(), 0).unwrap()[0],
            ValueId(0)
        );
        assert_eq!(
            materialize::arguments(b[2].terminator.as_ref().unwrap(), 0).unwrap()[0],
            ValueId(22)
        );
        assert_eq!(
            b[2].operations[0],
            input.module().functions[0].body.as_ref().unwrap().blocks[2].operations[0]
        );
        release(out, budget);
    });
}
#[test]
fn aggregate_ssa_parallel_edges_preserve_existing_parameter_prefix_and_phi_order() {
    let mut m = diamond(true);
    blocks(&mut m)[3]
        .parameters
        .push(ValueDef::new(ValueId(30), scalar()));
    if let Some(Terminator::ConditionalBranch {
        then_arguments,
        else_arguments,
        ..
    }) = &mut blocks(&mut m)[1].terminator
    {
        then_arguments.push(ValueId(0));
        else_arguments.push(ValueId(1));
    }
    if let Some(Terminator::Branch { arguments, .. }) = &mut blocks(&mut m)[2].terminator {
        arguments.push(ValueId(1));
    }
    with_input(m, |input, budget| {
        let out = exercise(input, budget);
        let b = &out.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks;
        assert_eq!(b[3].parameters[0], ValueDef::new(ValueId(30), scalar()));
        assert_eq!(
            materialize::arguments(b[1].terminator.as_ref().unwrap(), 0).unwrap(),
            [ValueId(0), ValueId(0)]
        );
        assert_eq!(
            materialize::arguments(b[1].terminator.as_ref().unwrap(), 1).unwrap(),
            [ValueId(1), ValueId(0)]
        );
        assert_eq!(
            materialize::arguments(b[2].terminator.as_ref().unwrap(), 0).unwrap(),
            [ValueId(1), ValueId(1)]
        );
        release(out, budget);
    });
}
#[test]
fn aggregate_ssa_concrete_select_preserves_scalar_and_vector_value_types() {
    use fe2o3_kernel_ir::VectorLayoutV12;
    let vector = FixedVectorTypeV12::new(
        ScalarType::F32,
        4,
        VectorLayoutV12::Interleaved { factor: 2 },
    );
    for (ty, layout, size, alignment) in [
        (Type::BOOL, LayoutKind::Scalar(ScalarType::Bool), 1, 1),
        (
            Type::Scalar(ScalarType::F64),
            LayoutKind::Scalar(ScalarType::F64),
            8,
            8,
        ),
        (Type::Vector(vector), LayoutKind::Vector(vector), 16, 16),
    ] {
        let mut m = Module::new("aggregate-leaf-type-identity");
        m.storage_layouts.push(StorageLayoutV1 {
            size,
            alignment,
            kind: layout,
        });
        let operations = vec![
            op(
                10,
                pointer(0),
                Kind::Alloca {
                    element: Type::StorageObject(StorageLayoutIdV1(0)),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment,
                },
            ),
            Operation::new(
                vec![],
                Kind::Storage(Storage::WriteValue {
                    address: ValueId(10),
                    value: ValueId(0),
                    access: MemoryAccess::new(AddressSpace::Private, alignment),
                }),
            ),
            op(
                20,
                ty.clone(),
                Kind::Storage(Storage::ReadValue {
                    address: ValueId(10),
                    access: MemoryAccess::new(AddressSpace::Private, alignment),
                }),
            ),
        ];
        m.functions.push(Function::internal_helper(
            "f",
            Signature::new(vec![ty.clone()], vec![ty.clone()]),
            vec![ValueId(0)],
            vec![block(
                100,
                operations,
                Terminator::Return {
                    values: vec![ValueId(20)],
                },
            )],
        ));
        with_input(m, |input, budget| {
            let out = exercise(input, budget);
            assert_eq!(out.promoted_allocations(), 1);
            let copy = &out.output().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks[0]
                .operations[1];
            assert_eq!(copy.results[0].ty, ty);
            assert!(matches!(
                copy.kind,
                Kind::Select {
                    true_value: ValueId(0),
                    false_value: ValueId(0),
                    ..
                }
            ));
            release(out, budget);
        });
    }
}
#[test]
fn aggregate_ssa_multiple_writes_and_non_topological_block_order_are_exact() {
    let mut m = diamond(false);
    blocks(&mut m)[1].operations.push(write(13, 1));
    blocks(&mut m).swap(1, 3);
    with_input(m, |input, budget| {
        let out = exercise(input, budget);
        assert_eq!(out.promoted_allocations(), 1);
        let b = &out.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks;
        assert_eq!(
            materialize::arguments(b[3].terminator.as_ref().unwrap(), 0).unwrap()[0],
            ValueId(1)
        );
        release(out, budget);
    });
}

#[test]
fn aggregate_ssa_switch_occurrences_preserve_case_default_and_phi_order() {
    use fe2o3_kernel_ir::{IntegerSwitchCase, SwitchCase};
    for typed in [false, true] {
        let mut m = diamond(false);
        blocks(&mut m)[1].terminator = Some(if typed {
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
        with_input(m, |input, budget| {
            let out = exercise(input, budget);
            assert_eq!(out.promoted_allocations(), 1);
            let actual = &out.output().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks;
            assert_eq!(actual[3].parameters.len(), 1);
            for ordinal in 0..3 {
                assert_eq!(
                    materialize::arguments(actual[1].terminator.as_ref().unwrap(), ordinal)
                        .unwrap(),
                    [ValueId(0)]
                );
            }
            assert_eq!(
                materialize::arguments(actual[2].terminator.as_ref().unwrap(), 0).unwrap(),
                [ValueId(1)]
            );
            release(out, budget);
        });
    }
}

#[test]
fn aggregate_ssa_packed_record_uses_actual_placement_alignment() {
    let mut m = straight();
    m.storage_layouts[1] = StorageLayoutV1 {
        size: 10,
        alignment: 1,
        kind: LayoutKind::Record(
            vec![
                StorageFieldV1 {
                    offset: 1,
                    layout: StorageLayoutIdV1(0),
                },
                StorageFieldV1 {
                    offset: 5,
                    layout: StorageLayoutIdV1(0),
                },
            ]
            .into_boxed_slice(),
        ),
    };
    m.storage_layouts[2] = StorageLayoutV1 {
        size: 20,
        alignment: 1,
        kind: LayoutKind::Array {
            element: StorageLayoutIdV1(1),
            length: 2,
            stride: 10,
        },
    };
    for operation in &mut blocks(&mut m)[0].operations {
        match &mut operation.kind {
            Kind::Alloca { alignment, .. } => *alignment = 1,
            Kind::Storage(
                Storage::ReadValue { access, .. } | Storage::WriteValue { access, .. },
            ) => access.alignment = 1,
            _ => {}
        }
    }
    with_input(m, |input, budget| {
        let out = exercise(input, budget);
        assert_eq!(out.promoted_allocations(), 1);
        assert_eq!(
            out.output().module().storage_layouts,
            input.module().storage_layouts
        );
        release(out, budget);
    });
}

#[test]
fn aggregate_ssa_fresh_admission_preserves_callers_layout_limit() {
    with_input(straight(), |input, budget| {
        let floor = budget.storage();
        let mut layouts = LAYOUTS;
        layouts.rows = 2;
        assert!(matches!(
            prepare_owned_aggregate_ssa_v18(input, layouts, budget),
            Err(Error::Admission(_))
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(input.module().storage_layouts.len(), 3);
    });
}
#[test]
fn aggregate_ssa_read_before_initialization_keeps_the_entire_allocation() {
    let mut m = diamond(false);
    blocks(&mut m)[2].operations.clear();
    noop(m);
    let mut m = straight();
    blocks(&mut m)[0].operations.remove(5);
    noop(m);
    let mut m = loop_fixture();
    blocks(&mut m)[0].operations.remove(5);
    noop(m);
}
#[test]
fn aggregate_ssa_dynamic_oob_volatile_and_escaping_addresses_remain_memory() {
    let mut m = straight();
    if let Kind::Storage(Storage::Project { step, .. }) = &mut blocks(&mut m)[0].operations[2].kind
    {
        *step = Projection::ArrayIndex(ValueId(3));
    }
    noop(m);
    let mut m = straight();
    blocks(&mut m)[0].operations[1].kind = Kind::Constant(Constant::Index(2));
    noop(m);
    let mut m = straight();
    if let Kind::Storage(Storage::ReadValue { access, .. }) =
        &mut blocks(&mut m)[0].operations[7].kind
    {
        access.volatile = true;
    }
    noop(m);
    let mut m = straight();
    m.functions[0].signature.results = vec![pointer(2)];
    blocks(&mut m)[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(10)],
    });
    noop(m);
}
#[test]
fn aggregate_ssa_whole_object_copy_preserves_both_allocation_histories() {
    let mut m = straight();
    let operations = &mut blocks(&mut m)[0].operations;
    operations.insert(
        1,
        op(
            30,
            pointer(2),
            Kind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(2)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
    );
    operations.push(Operation::new(
        vec![],
        Kind::Storage(Storage::CopyObject {
            source: ValueId(10),
            destination: ValueId(30),
            source_access: MemoryAccess::new(AddressSpace::Private, 4),
            destination_access: MemoryAccess::new(AddressSpace::Private, 4),
            overlap: StorageCopyOverlapV1::NonOverlapping,
        }),
    ));
    noop(m);
}

#[test]
fn aggregate_ssa_union_pointer_payload_and_opaque_call_are_not_scalar_authority() {
    let mut m = straight();
    m.storage_layouts[1].kind = LayoutKind::Union(
        vec![
            StorageFieldV1 {
                offset: 0,
                layout: StorageLayoutIdV1(0),
            },
            StorageFieldV1 {
                offset: 0,
                layout: StorageLayoutIdV1(0),
            },
        ]
        .into_boxed_slice(),
    );
    noop(m);
    let mut m = straight();
    m.storage_layouts[0] = StorageLayoutV1 {
        size: 8,
        alignment: 8,
        kind: LayoutKind::Pointer(fe2o3_kernel_ir::StoragePointerV1 {
            pointee: StorageLayoutIdV1(3),
            value_space: AddressSpace::Global,
            encoded_space: AddressSpace::Global,
            access: AccessMode::ReadWrite,
            stored_bits: 64,
        }),
    };
    m.storage_layouts[1] = StorageLayoutV1 {
        size: 16,
        alignment: 8,
        kind: LayoutKind::Record(
            vec![
                StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(0),
                },
                StorageFieldV1 {
                    offset: 8,
                    layout: StorageLayoutIdV1(0),
                },
            ]
            .into_boxed_slice(),
        ),
    };
    m.storage_layouts[2] = StorageLayoutV1 {
        size: 32,
        alignment: 8,
        kind: LayoutKind::Array {
            element: StorageLayoutIdV1(1),
            length: 2,
            stride: 16,
        },
    };
    m.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: LayoutKind::Scalar(ScalarType::U32),
    });
    let value_ty = Type::pointer(scalar(), AddressSpace::Global, AccessMode::ReadWrite);
    m.functions[0].signature.parameters[0] = value_ty.clone();
    m.functions[0].signature.parameters[1] = value_ty.clone();
    m.functions[0].signature.results = vec![value_ty.clone(), value_ty.clone()];
    if let Kind::Alloca { alignment, .. } = &mut blocks(&mut m)[0].operations[0].kind {
        *alignment = 8;
    }
    blocks(&mut m)[0].operations[7].results[0].ty = value_ty.clone();
    blocks(&mut m)[0].operations[8].results[0].ty = value_ty;
    noop(m);
    let mut m = straight();
    m.functions.push(Function::external_import(
        "opaque",
        Signature::new(vec![], vec![]),
    ));
    blocks(&mut m)[0].operations.insert(
        7,
        Operation::new(
            vec![],
            Kind::Call {
                callee: "opaque".into(),
                arguments: vec![],
            },
        ),
    );
    noop(m);
}

#[test]
fn aggregate_ssa_sparse_fixed_array_does_not_expand_unobserved_elements() {
    let mut m = straight();
    m.storage_layouts[2].size = 2048;
    if let LayoutKind::Array { length, .. } = &mut m.storage_layouts[2].kind {
        *length = 256;
    }
    with_input(m, |input, budget| {
        let out = exercise(input, budget);
        assert_eq!(out.promoted_allocations(), 1);
        assert_eq!(out.inserted_parameters(), 0);
        assert_eq!(out.witness.actions().len(), 9);
        release(out, budget);
    });
}

#[test]
fn aggregate_ssa_unreachable_address_use_keeps_original_storage() {
    let mut m = straight();
    blocks(&mut m).push(block(
        900,
        vec![
            op(
                30,
                pointer(0),
                Kind::Alloca {
                    element: Type::StorageObject(StorageLayoutIdV1(0)),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ),
            write(30, 0),
            read(50, 30),
        ],
        Terminator::Unreachable,
    ));
    with_input(m, |input, budget| {
        let out = exercise(input, budget);
        assert_eq!(out.promoted_allocations(), 1);
        assert_eq!(
            out.output().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks[1],
            input.module().functions[0].body.as_ref().unwrap().blocks[1]
        );
        release(out, budget);
    });
}

#[test]
fn aggregate_ssa_new_value_overflow_refuses_without_changing_input() {
    let mut m = straight();
    blocks(&mut m)[0]
        .operations
        .push(op(u32::MAX, scalar(), Kind::Constant(Constant::U32(1))));
    with_input(m, |input, budget| {
        let before = *input.identity();
        let floor = budget.storage();
        assert!(matches!(
            prepare_owned_aggregate_ssa_v18(input, LAYOUTS, budget),
            Err(Error::Check(
                fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18::Resource(
                    Resource::Arithmetic
                )
            ))
        ));
        assert_eq!(input.identity(), &before);
        assert_eq!(budget.storage(), floor);
    });
}
#[test]
fn aggregate_ssa_retains_unrelated_global_effects_and_trapping_producers() {
    let mut m = straight();
    m.functions[0].signature.parameters.push(Type::pointer(
        scalar(),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    m.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(99));
    blocks(&mut m)[0].operations.insert(
        5,
        op(
            40,
            scalar(),
            Kind::Binary {
                op: BinaryOp::Divide,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
    );
    blocks(&mut m)[0].operations.push(Operation::new(
        vec![],
        Kind::Store {
            pointer: ValueId(99),
            value: ValueId(20),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    with_input(m, |input, budget| {
        let out = exercise(input, budget);
        assert_eq!(out.promoted_allocations(), 1);
        let before = &input.module().functions[0].body.as_ref().unwrap().blocks[0].operations;
        let after = &out.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks[0]
            .operations;
        assert!(after.iter().any(|op| op == &before[5]));
        assert_eq!(after.last(), before.last());
        release(out, budget);
    });
}
#[test]
fn aggregate_ssa_unsupported_allocation_does_not_disable_independent_scalar_cells() {
    let mut m = straight();
    let operations = &mut blocks(&mut m)[0].operations;
    operations.insert(
        1,
        op(
            30,
            pointer(2),
            Kind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(2)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
    );
    operations.insert(
        2,
        op(
            31,
            pointer(1),
            Kind::Storage(Storage::Project {
                base: ValueId(30),
                step: Projection::ArrayIndex(ValueId(3)),
            }),
        ),
    );
    with_input(m, |input, budget| {
        let out = exercise(input, budget);
        assert_eq!(out.promoted_allocations(), 1);
        let before = &input.module().functions[0].body.as_ref().unwrap().blocks[0].operations;
        let after = &out.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks[0]
            .operations;
        assert!(after.iter().any(|op| op == &before[1]));
        assert!(after.iter().any(|op| op == &before[2]));
        release(out, budget);
    });
}
#[test]
fn aggregate_ssa_replay_rejects_admitted_output_tampering() {
    with_input(diamond(true), |input, budget| {
        let out = exercise(input, budget);
        let mut bad = out.output().module().clone();
        if let Kind::Select {
            true_value,
            false_value,
            ..
        } = &mut blocks(&mut bad)[3].operations[0].kind
        {
            *true_value = ValueId(0);
            *false_value = ValueId(0);
        }
        let (bad, bytes) = admit(&bad);
        budget.reserve_storage(bytes).unwrap();
        let result = resources::scoped(budget, |meter| check(input, &bad, &out.witness, meter));
        assert!(matches!(
            result,
            Err(Error::Check(
                fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18::Inconsistent(
                    "exact original load SSA value"
                )
            ))
        ));
        drop(bad);
        budget.release_storage(bytes).unwrap();
        out.replay_against(input, budget).unwrap();
        release(out, budget);
    });
}
fn resource(error: &Error) -> Option<Resource> {
    let mut current: &(dyn std::error::Error + 'static) = error;
    loop {
        if let Some(resource) = current.downcast_ref::<Resource>() {
            return Some(*resource);
        }
        current = current.source()?;
    }
}
#[test]
fn aggregate_ssa_exact_and_one_short_work_storage_restore_original_floor() {
    let (input, bytes) = admit(&diamond(true));
    let floor = bytes + 37;
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let out = prepare_owned_aggregate_ssa_v18(&input, LAYOUTS, &mut budget).unwrap();
    let (required_work, required_storage) = (budget.work(), budget.peak_storage());
    drop(out);
    assert_eq!(budget.storage(), floor);
    for (work_limit, storage_limit, expected) in [
        (required_work, required_storage, 0),
        (required_work - 1, required_storage, 1),
        (required_work, required_storage - 1, 2),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = prepare_owned_aggregate_ssa_v18(&input, LAYOUTS, &mut budget);
        match (expected, result) {
            (0, Ok(out)) => drop(out),
            (1, Err(error)) => assert!(matches!(resource(&error), Some(Resource::Work(_)))),
            (2, Err(error)) => assert!(matches!(resource(&error), Some(Resource::Storage(_)))),
            _ => panic!("exact resource boundary"),
        }
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn aggregate_ssa_many_uninitialized_allocations_have_a_paid_shrinking_retry_bound() {
    let mut previous_work = 0;
    for n in [8usize, 16] {
        let mut m = Module::new("many-uninitialized-aggregate-leaves");
        m.storage_layouts.push(StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: LayoutKind::Scalar(ScalarType::U32),
        });
        let mut operations = Vec::new();
        for j in 0..n as u32 {
            operations.push(op(
                1000 + j,
                pointer(0),
                Kind::Alloca {
                    element: Type::StorageObject(StorageLayoutIdV1(0)),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ));
            operations.push(read(10000 + j, 1000 + j));
        }
        m.functions.push(Function::internal_helper(
            "f",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block(
                100,
                operations,
                Terminator::Return { values: vec![] },
            )],
        ));
        let (input, bytes) = admit(&m);
        let floor = bytes + 37;
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let out = prepare_owned_aggregate_ssa_v18(&input, LAYOUTS, &mut budget).unwrap();
        assert_eq!(out.promoted_allocations(), 0);
        assert_eq!(out.output().canonical_bytes(), input.canonical_bytes());
        // For this family one undefined variable excludes one whole allocation.
        // Both production derivation and independent replay prepay the shrinking
        // planner caps; the remaining quadratic envelope covers census/admission.
        let planner_bound: usize = (1..=n).map(|k| 128 * (3 * k + 2) * (k + 2) * (k + 2)).sum();
        let total_bound = 2 * planner_bound + n * n * 65_536;
        assert!(budget.work() >= 2 * planner_bound);
        assert!(budget.work() < total_bound);
        assert!(budget.work() > previous_work);
        previous_work = budget.work();
        drop(out);
        assert_eq!(budget.storage(), floor);
        let first_cap = 128 * (3 * n + 2) * (n + 2) * (n + 2);
        let mut refused_work = Work::new(first_cap - 1);
        let mut refused = Budget::new(&mut refused_work, STORAGE);
        refused.reserve_storage(floor).unwrap();
        let error = match prepare_owned_aggregate_ssa_v18(&input, LAYOUTS, &mut refused) {
            Err(error) => error,
            Ok(_) => panic!("planner work must be prepaid"),
        };
        assert!(matches!(resource(&error), Some(Resource::Work(_))));
        assert_eq!(refused.storage(), floor);
        assert!(refused.failed_work().is_some());
    }
}
