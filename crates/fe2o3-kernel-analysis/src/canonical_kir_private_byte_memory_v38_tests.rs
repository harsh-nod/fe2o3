use super::*;
use fe2o3_kernel_ir::{
    AccessMode, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, Function as IrFunction,
    MemoryAccess, Module, Operation, ScalarType, Signature, StorageCopyOverlapV1 as Overlap,
    StorageFieldV1 as Field, StorageLayoutKindV1 as Kind, StorageLayoutLimitsV1,
    StorageLayoutV1 as Layout, StorageOperationV1 as Storage, StorageProjectionV1 as Projection,
    Terminator, ValueDef, VerifiedCanonicalKernelIrModuleV18,
};

const WORK: usize = 50_000_000;
const STORAGE: usize = 64 << 20;
const BOUNDARIES: usize = 4096;

fn access(alignment: u32) -> MemoryAccess {
    MemoryAccess::new(AddressSpace::Private, alignment)
}
fn pointer(layout: u32) -> Type {
    Type::pointer(
        Type::StorageObject(LayoutId(layout)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    )
}
fn scalar() -> Layout {
    Layout {
        size: 4,
        alignment: 1,
        kind: Kind::Scalar(ScalarType::U32),
    }
}
fn allocation(id: u32, layout: u32, alignment: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), pointer(layout)),
        OperationKind::Alloca {
            element: Type::StorageObject(LayoutId(layout)),
            count: None,
            address_space: AddressSpace::Private,
            alignment,
        },
    )
}
fn project(id: u32, base: u32, layout: u32, step: Projection) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), pointer(layout)),
        OperationKind::Storage(Storage::Project {
            base: ValueId(base),
            step,
        }),
    )
}
fn write(pointer: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Storage(Storage::WriteValue {
            address: ValueId(pointer),
            value: ValueId(20),
            access: access(1),
        }),
    )
}
fn read(id: u32, pointer: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
        OperationKind::Storage(Storage::ReadValue {
            address: ValueId(pointer),
            access: access(1),
        }),
    )
}
fn copy(source: u32, destination: u32, overlap: Overlap, alignment: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Storage(Storage::CopyObject {
            source: ValueId(source),
            destination: ValueId(destination),
            source_access: access(alignment),
            destination_access: access(alignment),
            overlap,
        }),
    )
}
fn constant(id: u32, value: u64) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::INDEX),
        OperationKind::Constant(Constant::Index(value)),
    )
}
fn branch(id: u32) -> Terminator {
    Terminator::Branch {
        target: BlockId(id),
        arguments: vec![],
    }
}
fn conditional(left: u32, right: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(900),
        then_target: BlockId(left),
        then_arguments: vec![],
        else_target: BlockId(right),
        else_arguments: vec![],
    }
}
fn ret() -> Terminator {
    Terminator::Return { values: vec![] }
}
fn block(id: u32, operations: Vec<Operation>, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}
fn module(layouts: Vec<Layout>, blocks: Vec<BasicBlock>) -> Module {
    let mut module = Module::new("sparse-private-physical-byte-initialization");
    module.storage_layouts = layouts;
    module.functions.push(IrFunction::internal_helper(
        "f",
        Signature::new(vec![Type::Scalar(ScalarType::U32), Type::BOOL], vec![]),
        vec![ValueId(20), ValueId(900)],
        blocks,
    ));
    module
}
fn with_inventory<T>(module: &Module, run: impl FnOnce(&Inventory<'_>, usize) -> T) -> T {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(23).unwrap();
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            StorageLayoutLimitsV1 {
                rows: 32,
                edges: 64,
                containment_depth: 16,
                object_bytes: 1 << 48,
            },
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (inventory, receipt) = Inventory::derive_v18(&owner, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    run(&inventory, budget.storage())
}
fn probe(
    inventory: &Inventory<'_>,
    floor: usize,
    work: usize,
    storage: usize,
    maximum: usize,
    check: impl FnOnce(&Analysis<'_, '_>),
) -> (R<()>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = Budget::new(&mut work, storage);
    budget.reserve_storage(floor).unwrap();
    let result = analyze_canonical_kir_private_bytes_v38(
        inventory,
        CanonicalKirPrivateByteLimitsV38 {
            max_boundaries: maximum,
        },
        &mut budget,
    );
    assert_eq!(budget.storage(), floor);
    let result = result.and_then(|(analysis, receipt)| {
        budget.reserve_storage(receipt.retained_storage())?;
        assert!(analysis.is_for(inventory));
        assert!(std::ptr::eq(
            analysis.inventory().owner(),
            inventory.owner()
        ));
        assert!(!analysis.grants_authority());
        check(&analysis);
        drop(analysis);
        budget.release_storage(receipt.retained_storage())?;
        Ok(())
    });
    assert_eq!(budget.storage(), floor);
    let peak = budget.peak_storage();
    drop(budget);
    (result, work.work(), peak)
}

fn padded_copy() -> Module {
    module(
        vec![
            scalar(),
            Layout {
                size: 16,
                alignment: 1,
                kind: Kind::Record(
                    vec![
                        Field {
                            offset: 0,
                            layout: LayoutId(0),
                        },
                        Field {
                            offset: 12,
                            layout: LayoutId(0),
                        },
                    ]
                    .into_boxed_slice(),
                ),
            },
        ],
        vec![block(
            100,
            vec![
                allocation(10, 1, 1),
                allocation(30, 1, 1),
                project(11, 10, 0, Projection::Field(0)),
                write(11),
                copy(10, 30, Overlap::NonOverlapping, 1),
                project(31, 30, 0, Projection::Field(0)),
                read(40, 31),
                project(32, 30, 0, Projection::Field(1)),
                read(41, 32),
            ],
            ret(),
        )],
    )
}

#[test]
fn private_bytes_copy_preserves_uninitialized_padding_and_reads_only_payload() {
    with_inventory(&padded_copy(), |inventory, floor| {
        probe(inventory, floor, WORK, STORAGE, BOUNDARIES, |proof| {
            assert!(proof.operation(4).unwrap().is_proven());
            assert_eq!(
                proof.operation(4).unwrap().range(0).unwrap().byte_length(),
                16
            );
            assert_eq!(
                proof.operation(6).unwrap().payload_initialized(),
                Some(true)
            );
            assert_eq!(
                proof.operation(6).unwrap().range(0).unwrap().byte_length(),
                4
            );
            assert_eq!(
                proof.operation(8).unwrap().payload_initialized(),
                Some(false)
            );
            assert!(
                proof
                    .operation(8)
                    .unwrap()
                    .requires(Obligation::Initialization)
            );
        })
        .0
        .unwrap();
    });
}

fn overlapping(overlap: Overlap, self_copy: bool) -> Module {
    module(
        vec![
            scalar(),
            Layout {
                size: 6,
                alignment: 1,
                kind: Kind::Record(
                    vec![Field {
                        offset: 2,
                        layout: LayoutId(0),
                    }]
                    .into_boxed_slice(),
                ),
            },
            Layout {
                size: 8,
                alignment: 1,
                kind: Kind::Union(
                    vec![
                        Field {
                            offset: 0,
                            layout: LayoutId(0),
                        },
                        Field {
                            offset: 0,
                            layout: LayoutId(1),
                        },
                    ]
                    .into_boxed_slice(),
                ),
            },
        ],
        vec![block(
            100,
            vec![
                allocation(10, 2, 1),
                project(11, 10, 0, Projection::Field(0)),
                project(13, 10, 1, Projection::Field(1)),
                project(12, 13, 0, Projection::Field(0)),
                write(11),
                copy(11, if self_copy { 11 } else { 12 }, overlap, 1),
                read(40, if self_copy { 11 } else { 12 }),
            ],
            ret(),
        )],
    )
}

#[test]
fn private_bytes_self_and_overlapping_copy_use_source_snapshot() {
    for self_copy in [false, true] {
        with_inventory(
            &overlapping(Overlap::MayOverlap, self_copy),
            |inventory, floor| {
                probe(inventory, floor, WORK, STORAGE, BOUNDARIES, |proof| {
                    assert!(proof.operation(5).unwrap().is_proven());
                    assert_eq!(
                        proof.operation(6).unwrap().payload_initialized(),
                        Some(true)
                    );
                })
                .0
                .unwrap();
            },
        );
    }
    with_inventory(
        &overlapping(Overlap::NonOverlapping, false),
        |inventory, floor| {
            probe(inventory, floor, WORK, STORAGE, BOUNDARIES, |proof| {
                assert!(proof.operation(5).unwrap().requires(Obligation::Operation));
                assert_eq!(
                    proof.operation(6).unwrap().payload_initialized(),
                    Some(false)
                );
            })
            .0
            .unwrap();
        },
    );
}

#[test]
fn private_bytes_cfg_diamond_meets_all_paths_not_first_reaching_store() {
    for right_initialized in [false, true] {
        let fixture = module(
            vec![scalar()],
            vec![
                block(100, vec![allocation(10, 0, 1)], conditional(200, 300)),
                block(200, vec![write(10)], branch(400)),
                block(400, vec![read(40, 10)], ret()),
                block(
                    300,
                    if right_initialized {
                        vec![write(10)]
                    } else {
                        vec![]
                    },
                    branch(400),
                ),
            ],
        );
        with_inventory(&fixture, |inventory, floor| {
            probe(inventory, floor, WORK, STORAGE, BOUNDARIES, |proof| {
                assert_eq!(
                    proof.operation(2).unwrap().payload_initialized(),
                    Some(right_initialized)
                );
            })
            .0
            .unwrap();
        });
    }
}

#[test]
fn private_bytes_loop_allocation_reset_cannot_inherit_previous_iteration_write() {
    let fixture = module(
        vec![scalar()],
        vec![
            block(100, vec![], branch(200)),
            block(
                200,
                vec![allocation(10, 0, 1), read(40, 10), write(10)],
                conditional(200, 300),
            ),
            block(300, vec![], ret()),
        ],
    );
    with_inventory(&fixture, |inventory, floor| {
        probe(inventory, floor, WORK, STORAGE, BOUNDARIES, |proof| {
            assert_eq!(
                proof.operation(1).unwrap().payload_initialized(),
                Some(false)
            );
        })
        .0
        .unwrap();
    });
}

fn sparse_array(length: u64) -> Module {
    module(
        vec![
            scalar(),
            Layout {
                size: length * 4,
                alignment: 1,
                kind: Kind::Array {
                    element: LayoutId(0),
                    length,
                    stride: 4,
                },
            },
        ],
        vec![block(
            100,
            vec![
                allocation(10, 1, 1),
                constant(50, length - 1),
                project(11, 10, 0, Projection::ArrayIndex(ValueId(50))),
                write(11),
                read(40, 11),
            ],
            ret(),
        )],
    )
}

#[test]
fn private_bytes_huge_sparse_layout_does_not_allocate_per_object_byte() {
    let mut backing = None;
    for length in [16, 1 << 36] {
        with_inventory(&sparse_array(length), |inventory, floor| {
            let result = probe(inventory, floor, WORK, STORAGE, 8, |proof| {
                assert_eq!(
                    proof.operation(4).unwrap().payload_initialized(),
                    Some(true)
                );
                assert_eq!(
                    proof.operation(2).unwrap().range(0).unwrap().byte_start(),
                    (length - 1) * 4
                );
                let actual = proof.retained_storage().unwrap();
                assert_eq!(*backing.get_or_insert(actual), actual);
            });
            result.0.unwrap();
            assert!(result.2 - floor < 65536);
        });
    }
}

#[test]
fn private_bytes_zero_width_still_requires_formation_bounds_and_alignment() {
    let empty = Layout {
        size: 0,
        alignment: 1,
        kind: Kind::Record(vec![].into_boxed_slice()),
    };
    let fixture = module(
        vec![
            empty,
            Layout {
                size: 0,
                alignment: 1,
                kind: Kind::Array {
                    element: LayoutId(0),
                    length: 1,
                    stride: 0,
                },
            },
        ],
        vec![block(
            100,
            vec![
                allocation(10, 0, 1),
                copy(10, 10, Overlap::MayOverlap, 8),
                copy(10, 10, Overlap::MayOverlap, 1),
                allocation(30, 1, 1),
                constant(50, 1),
                project(31, 30, 0, Projection::ArrayIndex(ValueId(50))),
            ],
            ret(),
        )],
    );
    with_inventory(&fixture, |inventory, floor| {
        probe(inventory, floor, WORK, STORAGE, BOUNDARIES, |proof| {
            assert!(proof.operation(1).unwrap().requires(Obligation::Alignment));
            assert!(proof.operation(2).unwrap().is_proven());
            assert!(proof.operation(5).unwrap().requires(Obligation::Bounds));
        })
        .0
        .unwrap();
    });
}

#[test]
fn private_bytes_boundary_ceiling_is_explicit_unknown_not_partial_success() {
    with_inventory(
        &overlapping(Overlap::MayOverlap, false),
        |inventory, floor| {
            probe(inventory, floor, WORK, STORAGE, 3, |proof| {
                assert!(
                    proof
                        .operation(5)
                        .unwrap()
                        .requires(Obligation::CopyBoundaryClosure)
                );
                assert!(
                    proof
                        .operation(6)
                        .unwrap()
                        .requires(Obligation::CopyBoundaryClosure)
                );
                assert_eq!(proof.operation(6).unwrap().payload_initialized(), None);
                assert!(!proof.operation(6).unwrap().is_proven());
            })
            .0
            .unwrap();
        },
    );
}

#[test]
fn private_bytes_exact_work_storage_and_each_one_short_preserve_scope_floor() {
    with_inventory(&padded_copy(), |inventory, floor| {
        let measured = probe(inventory, floor, WORK, STORAGE, BOUNDARIES, |_| {});
        measured.0.unwrap();
        let exact = probe(inventory, floor, measured.1, measured.2, BOUNDARIES, |_| {});
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2), (measured.1, measured.2));
        assert!(matches!(
            probe(
                inventory,
                floor,
                measured.1 - 1,
                measured.2,
                BOUNDARIES,
                |_| {}
            )
            .0,
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert!(matches!(
            probe(
                inventory,
                floor,
                measured.1,
                measured.2 - 1,
                BOUNDARIES,
                |_| {}
            )
            .0,
            Err(Error::Resource(Resource::Storage(_)))
        ));
    });
}

#[test]
fn private_bytes_function_domains_and_static_allocation_occurrences_remain_separate() {
    let mut fixture = module(
        vec![scalar()],
        vec![block(
            100,
            vec![allocation(10, 0, 1), write(10), read(40, 10)],
            ret(),
        )],
    );
    fixture.functions.push(IrFunction::internal_helper(
        "g",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block(100, vec![allocation(10, 0, 1), read(40, 10)], ret())],
    ));
    with_inventory(&fixture, |inventory, floor| {
        probe(inventory, floor, WORK, STORAGE, BOUNDARIES, |proof| {
            assert_eq!(
                proof.operation(2).unwrap().payload_initialized(),
                Some(true)
            );
            assert_eq!(
                proof.operation(4).unwrap().payload_initialized(),
                Some(false)
            );
            assert_ne!(
                proof
                    .operation(2)
                    .unwrap()
                    .range(0)
                    .unwrap()
                    .allocation_operation(),
                proof
                    .operation(4)
                    .unwrap()
                    .range(0)
                    .unwrap()
                    .allocation_operation()
            );
        })
        .0
        .unwrap();
    });
}

#[test]
fn private_bytes_dynamic_projection_keeps_explicit_address_and_initialization_obligations() {
    let mut fixture = sparse_array(8);
    let function = &mut fixture.functions[0];
    function.signature.parameters.push(Type::INDEX);
    function.body.as_mut().unwrap().parameters.push(ValueId(50));
    function.body.as_mut().unwrap().blocks[0]
        .operations
        .remove(1);
    with_inventory(&fixture, |inventory, floor| {
        probe(inventory, floor, WORK, STORAGE, BOUNDARIES, |proof| {
            assert!(proof.operation(1).unwrap().requires(Obligation::Bounds));
            assert!(proof.operation(3).unwrap().requires(Obligation::Address));
            assert!(
                proof
                    .operation(3)
                    .unwrap()
                    .requires(Obligation::Currentness)
            );
            assert!(
                proof
                    .operation(3)
                    .unwrap()
                    .requires(Obligation::Initialization)
            );
            assert_eq!(proof.operation(3).unwrap().payload_initialized(), None);
        })
        .0
        .unwrap();
    });
}

#[test]
fn private_bytes_padded_vector_layout_keeps_payload_and_object_extents_distinct() {
    let vector = fe2o3_kernel_ir::FixedVectorTypeV12::new(
        ScalarType::U32,
        3,
        fe2o3_kernel_ir::VectorLayoutV12::Contiguous,
    );
    let mut fixture = module(
        vec![Layout {
            size: 16,
            alignment: 16,
            kind: Kind::Vector(vector),
        }],
        vec![block(
            100,
            vec![
                allocation(10, 0, 16),
                allocation(30, 0, 16),
                write(10),
                copy(10, 30, Overlap::NonOverlapping, 16),
                Operation::effect_free(
                    ValueDef::new(ValueId(40), Type::Vector(vector)),
                    OperationKind::Storage(Storage::ReadValue {
                        address: ValueId(30),
                        access: access(16),
                    }),
                ),
            ],
            ret(),
        )],
    );
    fixture.functions[0].signature.parameters[0] = Type::Vector(vector);
    with_inventory(&fixture, |inventory, floor| {
        probe(inventory, floor, WORK, STORAGE, BOUNDARIES, |proof| {
            assert_eq!(
                proof.operation(2).unwrap().range(0).unwrap().byte_length(),
                12
            );
            assert_eq!(
                proof.operation(3).unwrap().range(0).unwrap().byte_length(),
                16
            );
            assert_eq!(
                proof.operation(4).unwrap().payload_initialized(),
                Some(true)
            );
            assert_eq!(
                proof.operation(4).unwrap().range(0).unwrap().byte_length(),
                12
            );
        })
        .0
        .unwrap();
    });
}

#[test]
fn private_bytes_stored_pointer_width_comes_from_layout_not_holder_or_index() {
    let value = Type::pointer(
        Type::StorageObject(LayoutId(0)),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let mut fixture = module(
        vec![
            scalar(),
            Layout {
                size: 16,
                alignment: 8,
                kind: Kind::Pointer(fe2o3_kernel_ir::StoragePointerV1 {
                    pointee: LayoutId(0),
                    value_space: AddressSpace::Global,
                    encoded_space: AddressSpace::Global,
                    access: AccessMode::ReadOnly,
                    stored_bits: 128,
                }),
            },
        ],
        vec![block(
            100,
            vec![
                allocation(10, 1, 8),
                write(10),
                Operation::effect_free(
                    ValueDef::new(ValueId(40), value.clone()),
                    OperationKind::Storage(Storage::ReadValue {
                        address: ValueId(10),
                        access: access(8),
                    }),
                ),
            ],
            ret(),
        )],
    );
    fixture.functions[0].signature.parameters[0] = value;
    with_inventory(&fixture, |inventory, floor| {
        probe(inventory, floor, WORK, STORAGE, BOUNDARIES, |proof| {
            assert_eq!(
                proof.operation(2).unwrap().range(0).unwrap().byte_length(),
                16
            );
            assert_eq!(
                proof.operation(2).unwrap().payload_initialized(),
                Some(true)
            );
            assert!(!proof.grants_authority());
        })
        .0
        .unwrap();
    });
}

#[test]
fn private_bytes_global_input_is_an_explicit_external_obligation() {
    let mut fixture = module(
        vec![scalar()],
        vec![block(
            100,
            vec![Operation::effect_free(
                ValueDef::new(ValueId(40), Type::Scalar(ScalarType::U32)),
                OperationKind::Storage(Storage::ReadValue {
                    address: ValueId(20),
                    access: MemoryAccess::new(AddressSpace::Global, 1),
                }),
            )],
            ret(),
        )],
    );
    fixture.functions[0].signature.parameters[0] = Type::pointer(
        Type::StorageObject(LayoutId(0)),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    with_inventory(&fixture, |inventory, floor| {
        probe(inventory, floor, WORK, STORAGE, BOUNDARIES, |proof| {
            let fact = proof.operation(0).unwrap();
            assert!(fact.requires(Obligation::ExternalMemory));
            assert!(fact.requires(Obligation::Currentness));
            assert!(fact.requires(Obligation::Initialization));
            assert!(!fact.is_proven());
            assert_eq!(fact.payload_initialized(), None);
        })
        .0
        .unwrap();
    });
}

#[test]
fn private_bytes_coordinates_are_bytes_not_legacy_scalar_elements() {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let alloca = Operation::effect_free(
        ValueDef::new(ValueId(10), pointer.clone()),
        OperationKind::Alloca {
            element: Type::Scalar(ScalarType::U32),
            count: Some(ValueId(50)),
            address_space: AddressSpace::Private,
            alignment: 1,
        },
    );
    let fixture = module(
        vec![scalar()],
        vec![block(
            100,
            vec![
                constant(50, 3),
                alloca,
                constant(51, 2),
                Operation::effect_free(
                    ValueDef::new(ValueId(11), pointer),
                    OperationKind::GetElementPointer {
                        base: ValueId(10),
                        offset: ValueId(51),
                    },
                ),
                Operation::new(
                    vec![],
                    OperationKind::Store {
                        pointer: ValueId(11),
                        value: ValueId(20),
                        access: access(1),
                    },
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(40), Type::Scalar(ScalarType::U32)),
                    OperationKind::Load {
                        pointer: ValueId(11),
                        access: access(1),
                    },
                ),
            ],
            ret(),
        )],
    );
    with_inventory(&fixture, |inventory, floor| {
        probe(inventory, floor, WORK, STORAGE, BOUNDARIES, |proof| {
            let range = proof.operation(3).unwrap().range(0).unwrap();
            assert_eq!((range.byte_start(), range.byte_length()), (8, 4));
            assert_eq!(
                proof.operation(5).unwrap().payload_initialized(),
                Some(true)
            );
        })
        .0
        .unwrap();
    });
}

#[test]
fn private_bytes_sparse_range_query_has_independent_sixteen_work_boundary() {
    let partitions = intervals::Partitions {
        boundaries: [0, 4, 8, 12]
            .into_iter()
            .map(|offset| intervals::Boundary {
                allocation: 0,
                offset,
            })
            .collect(),
        allocations: vec![intervals::Span {
            start: 0,
            length: 4,
        }],
        complete: true,
    };
    for limit in [15, 16] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, 0);
        let result = partitions.range(
            ByteRange {
                allocation: 0,
                start: 4,
                length: 4,
            },
            &mut budget,
        );
        if limit == 16 {
            assert_eq!(result.unwrap(), 1..2);
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
        }
        assert_eq!(budget.storage(), 0);
        if limit == 16 {
            assert_eq!(budget.work(), 16);
        }
    }
}

#[test]
fn private_bytes_retained_storage_and_foreign_inventory_have_independent_layout_checks() {
    with_inventory(&padded_copy(), |inventory, floor| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let (proof, receipt) = analyze_canonical_kir_private_bytes_v38(
            inventory,
            CanonicalKirPrivateByteLimitsV38 {
                max_boundaries: BOUNDARIES,
            },
            &mut budget,
        )
        .unwrap();
        let expected = size_of::<Analysis<'_, '_>>()
            + proof.definitions.capacity() * size_of::<Option<ByteAddress>>()
            + proof.operations.capacity() * size_of::<Fact>();
        assert_eq!(receipt.retained_storage(), expected);
        budget.reserve_storage(expected).unwrap();
        let (other, other_receipt) = Inventory::derive_v18(inventory.owner(), &mut budget).unwrap();
        budget
            .reserve_storage(other_receipt.retained_storage())
            .unwrap();
        assert!(!proof.is_for(&other));
        drop(other);
        budget
            .release_storage(other_receipt.retained_storage())
            .unwrap();
        drop(proof);
        budget.release_storage(expected).unwrap();
        assert_eq!(budget.storage(), floor);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut short = Budget::new(&mut work, expected - 1);
        assert!(matches!(
            short.reserve_storage(expected),
            Err(Resource::Storage(_))
        ));
        assert_eq!(short.storage(), 0);
    });
}

#[test]
fn private_bytes_overlap_snapshot_does_not_propagate_newly_written_initialization() {
    let record = |size, fields: Vec<Field>| Layout {
        size,
        alignment: 1,
        kind: Kind::Record(fields.into_boxed_slice()),
    };
    let mut fixture = module(
        vec![
            scalar(),
            Layout {
                size: 2,
                alignment: 1,
                kind: Kind::Scalar(ScalarType::U16),
            },
            record(
                6,
                vec![Field {
                    offset: 2,
                    layout: LayoutId(0),
                }],
            ),
            record(
                6,
                vec![
                    Field {
                        offset: 2,
                        layout: LayoutId(1),
                    },
                    Field {
                        offset: 4,
                        layout: LayoutId(1),
                    },
                ],
            ),
            Layout {
                size: 8,
                alignment: 1,
                kind: Kind::Union(
                    (0..4)
                        .map(|id| Field {
                            offset: 0,
                            layout: LayoutId(id),
                        })
                        .collect(),
                ),
            },
        ],
        vec![block(
            100,
            vec![
                allocation(10, 4, 1),
                project(11, 10, 0, Projection::Field(0)),
                project(12, 10, 1, Projection::Field(1)),
                project(13, 10, 2, Projection::Field(2)),
                project(14, 13, 0, Projection::Field(0)),
                project(15, 10, 3, Projection::Field(3)),
                project(16, 15, 1, Projection::Field(0)),
                project(17, 15, 1, Projection::Field(1)),
                write(12),
                copy(11, 14, Overlap::MayOverlap, 1),
                Operation::effect_free(
                    ValueDef::new(ValueId(40), Type::Scalar(ScalarType::U16)),
                    OperationKind::Storage(Storage::ReadValue {
                        address: ValueId(16),
                        access: access(1),
                    }),
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(41), Type::Scalar(ScalarType::U16)),
                    OperationKind::Storage(Storage::ReadValue {
                        address: ValueId(17),
                        access: access(1),
                    }),
                ),
            ],
            ret(),
        )],
    );
    fixture.functions[0].signature.parameters[0] = Type::Scalar(ScalarType::U16);
    with_inventory(&fixture, |inventory, floor| {
        probe(inventory, floor, WORK, STORAGE, BOUNDARIES, |proof| {
            assert!(proof.operation(9).unwrap().is_proven());
            assert_eq!(
                proof.operation(10).unwrap().payload_initialized(),
                Some(true)
            );
            assert_eq!(
                proof.operation(11).unwrap().payload_initialized(),
                Some(false)
            );
            assert!(
                proof
                    .operation(11)
                    .unwrap()
                    .requires(Obligation::Initialization)
            );
        })
        .0
        .unwrap();
    });
}

#[test]
fn private_bytes_empty_variant_formation_keeps_tag_and_active_view_obligations() {
    for unknown_parent in [false, true] {
        let layouts = vec![
            scalar(),
            Layout {
                size: 0,
                alignment: 1,
                kind: Kind::Record(vec![].into_boxed_slice()),
            },
            Layout {
                size: 4,
                alignment: 1,
                kind: Kind::Variants {
                    encoding: fe2o3_kernel_ir::StorageVariantEncodingV1::Direct {
                        tag: Field {
                            offset: 0,
                            layout: LayoutId(0),
                        },
                    },
                    variants: vec![fe2o3_kernel_ir::StorageVariantV1 {
                        discriminant: 0,
                        direct_tag_bits: Some(0),
                        uninhabited: false,
                        layout: LayoutId(1),
                    }]
                    .into_boxed_slice(),
                },
            },
        ];
        let mut ops = if unknown_parent {
            vec![]
        } else {
            vec![allocation(10, 2, 1)]
        };
        ops.push(Operation::effect_free(
            ValueDef::new(
                ValueId(11),
                Type::pointer(
                    Type::StorageObject(LayoutId(1)),
                    AddressSpace::Private,
                    AccessMode::WriteOnly,
                ),
            ),
            OperationKind::Storage(Storage::Project {
                base: ValueId(if unknown_parent { 20 } else { 10 }),
                step: Projection::VariantForWrite { index: 0 },
            }),
        ));
        let mut fixture = module(layouts, vec![block(100, ops, ret())]);
        if unknown_parent {
            fixture.functions[0].signature.parameters[0] = pointer(2);
        }
        with_inventory(&fixture, |inventory, floor| {
            probe(inventory, floor, WORK, STORAGE, BOUNDARIES, |proof| {
                let fact = proof.operation(usize::from(!unknown_parent)).unwrap();
                assert!(fact.requires(Obligation::TagContract));
                assert!(fact.requires(Obligation::ActiveView));
                assert!(!fact.is_proven());
            })
            .0
            .unwrap();
        });
    }
}

#[test]
fn private_bytes_obligation_visitor_is_complete_unique_and_stops_on_refusal() {
    let mut fact = Fact::NONE;
    for obligation in Obligation::ALL {
        fact.require(*obligation);
    }
    let mut visited = Vec::new();
    fact.try_visit_obligations(|obligation| {
        visited.push(obligation);
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(visited, Obligation::ALL);
    assert_eq!(visited.len(), Obligation::COUNT);
    assert!(!fact.is_proven());
    let mut count = 0;
    assert_eq!(
        fact.try_visit_obligations(|_| {
            count += 1;
            Err("refused")
        }),
        Err("refused")
    );
    assert_eq!(count, 1);
    Fact::NONE
        .try_visit_obligations(|_| -> Result<(), ()> { panic!("empty fact") })
        .unwrap();
}
