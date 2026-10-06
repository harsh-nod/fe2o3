use super::*;
use fe2o3_kernel_ir::{
    CastKind, StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutV1,
    StorageOperationV1 as Storage,
};

fn pointer(access: AccessMode) -> Type {
    Type::pointer(
        Type::StorageObject(StorageLayoutIdV1(0)),
        AddressSpace::Private,
        access,
    )
}
fn write(value: u32) -> Operation {
    Operation::new(
        vec![],
        Kind::Storage(Storage::WriteValue {
            address: ValueId(40),
            value: ValueId(value),
            access: MemoryAccess::new(AddressSpace::Private, 8),
        }),
    )
}
fn spill() -> Module {
    let mut entry = BasicBlock::new(BlockId(91));
    entry.operations.push(value(
        40,
        pointer(AccessMode::ReadWrite),
        Kind::Alloca {
            element: Type::StorageObject(StorageLayoutIdV1(0)),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 8,
        },
    ));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(62),
        arguments: vec![],
    });
    let mut writer = BasicBlock::new(BlockId(62));
    writer.operations = vec![
        write(1),
        value(
            41,
            pointer(AccessMode::ReadOnly),
            Kind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: ValueId(40),
                to: pointer(AccessMode::ReadOnly),
            },
        ),
    ];
    writer.terminator = Some(Terminator::Branch {
        target: BlockId(17),
        arguments: vec![ValueId(41)],
    });
    let mut middle = BasicBlock::new(BlockId(17));
    middle
        .parameters
        .push(ValueDef::new(ValueId(42), pointer(AccessMode::ReadOnly)));
    middle.terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![ValueId(42)],
    });
    let mut reader = tile_block(7, 44);
    reader
        .parameters
        .push(ValueDef::new(ValueId(45), pointer(AccessMode::ReadOnly)));
    reader.operations.splice(
        0..0,
        [
            value(
                43,
                Type::Scalar(ScalarType::U64),
                Kind::Storage(Storage::ReadValue {
                    address: ValueId(45),
                    access: MemoryAccess::new(AddressSpace::Private, 1),
                }),
            ),
            value(
                44,
                Type::INDEX,
                Kind::Cast {
                    kind: CastKind::Bitcast,
                    value: ValueId(43),
                    to: Type::INDEX,
                },
            ),
        ],
    );
    let mut input = module(vec![entry, writer, middle, reader]);
    input.functions[0].signature.parameters[1] = Type::Scalar(ScalarType::U64);
    input.storage_layouts.push(StorageLayoutV1 {
        size: 8,
        alignment: 8,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U64),
    });
    input
}

#[test]
fn private_uniformity_exact_scalar_spill_and_single_edge_aliases() {
    check(&spill()).unwrap();
    let mut input = spill();
    // Inventory order must not substitute for CFG domination.
    input.functions[0].body.as_mut().unwrap().blocks.swap(1, 3);
    check(&input).unwrap();
}

#[test]
fn private_uniformity_refuses_competing_missing_and_undominated_writes() {
    for mode in 0..4 {
        let mut input = spill();
        let blocks = &mut input.functions[0].body.as_mut().unwrap().blocks;
        match mode {
            0 => {
                blocks[1].operations.extend([
                    value(
                        52,
                        Type::Scalar(ScalarType::U64),
                        Kind::Constant(Constant::U64(7)),
                    ),
                    write(52),
                ]);
            }
            1 => {
                blocks[1].operations.remove(0);
            }
            2 => {
                blocks[0].operations.push(value(
                    90,
                    Type::Scalar(ScalarType::U64),
                    Kind::Storage(Storage::ReadValue {
                        address: ValueId(40),
                        access: MemoryAccess::new(AddressSpace::Private, 8),
                    }),
                ));
            }
            _ => {
                // The pointer itself dominates every use, but one path to the
                // reader bypasses the sole writer. Alias identity is not enough.
                let restriction = blocks[1].operations.remove(1);
                blocks[0].operations.push(restriction);
                blocks[0].terminator = Some(Terminator::ConditionalBranch {
                    condition: ValueId(2),
                    then_target: BlockId(62),
                    then_arguments: vec![],
                    else_target: BlockId(17),
                    else_arguments: vec![],
                });
                blocks[1].terminator = Some(Terminator::Branch {
                    target: BlockId(17),
                    arguments: vec![],
                });
                blocks[2].parameters.clear();
                blocks[2].terminator = Some(Terminator::Branch {
                    target: BlockId(7),
                    arguments: vec![],
                });
                blocks[3].parameters.clear();
                let Kind::Storage(Storage::ReadValue { address, .. }) =
                    &mut blocks[3].operations[0].kind
                else {
                    unreachable!()
                };
                *address = ValueId(41);
            }
        }
        assert!(
            matches!(check(&input), Err(Error::VaryingTileInput(_))),
            "mode {mode}"
        );
    }
}

#[test]
fn private_uniformity_partial_object_write_is_not_a_whole_scalar_fact() {
    use fe2o3_kernel_ir::{StorageFieldV1, StorageProjectionV1};
    let mut input = spill();
    input.storage_layouts.push(input.storage_layouts[0].clone());
    input.storage_layouts[0] = StorageLayoutV1 {
        size: 16,
        alignment: 8,
        kind: StorageLayoutKindV1::Record(
            vec![
                StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(1),
                },
                StorageFieldV1 {
                    offset: 8,
                    layout: StorageLayoutIdV1(1),
                },
            ]
            .into_boxed_slice(),
        ),
    };
    let field = |access| {
        Type::pointer(
            Type::StorageObject(StorageLayoutIdV1(1)),
            AddressSpace::Private,
            access,
        )
    };
    let blocks = &mut input.functions[0].body.as_mut().unwrap().blocks;
    blocks[1].operations[0] = Operation::new(
        vec![],
        Kind::Storage(Storage::WriteValue {
            address: ValueId(48),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Private, 8),
        }),
    );
    blocks[1].operations.insert(
        0,
        value(
            48,
            field(AccessMode::ReadWrite),
            Kind::Storage(Storage::Project {
                base: ValueId(40),
                step: StorageProjectionV1::Field(0),
            }),
        ),
    );
    let Kind::Storage(Storage::ReadValue { address, .. }) = &mut blocks[3].operations[0].kind
    else {
        unreachable!()
    };
    *address = ValueId(49);
    blocks[3].operations.insert(
        0,
        value(
            49,
            field(AccessMode::ReadOnly),
            Kind::Storage(Storage::Project {
                base: ValueId(45),
                step: StorageProjectionV1::Field(0),
            }),
        ),
    );
    assert!(matches!(check(&input), Err(Error::VaryingTileInput(_))));
}

#[test]
fn private_uniformity_stored_lane_value_and_divergent_writer_stay_varying() {
    let mut varying_value = spill();
    let writer = &mut varying_value.functions[0].body.as_mut().unwrap().blocks[1];
    writer.operations.splice(
        0..1,
        [
            index(50, IndexKind::Local),
            value(
                51,
                Type::Scalar(ScalarType::U64),
                Kind::Cast {
                    kind: CastKind::Bitcast,
                    value: ValueId(50),
                    to: Type::Scalar(ScalarType::U64),
                },
            ),
            write(51),
        ],
    );
    assert!(matches!(
        check(&varying_value),
        Err(Error::VaryingTileInput(_))
    ));

    let mut divergent = spill();
    let blocks = &mut divergent.functions[0].body.as_mut().unwrap().blocks;
    blocks[0].operations.extend([
        index(50, IndexKind::Local),
        value(52, Type::INDEX, Kind::Constant(Constant::Index(0))),
        value(
            51,
            Type::BOOL,
            Kind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(50),
                rhs: ValueId(52),
            },
        ),
    ]);
    blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(51),
        then_target: BlockId(62),
        then_arguments: vec![],
        else_target: BlockId(99),
        else_arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(99));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    blocks.push(exit);
    assert!(matches!(check(&divergent), Err(Error::VaryingArrival(_))));
}

#[test]
fn private_uniformity_volatile_copy_escape_and_loop_are_not_value_receipts() {
    for mode in 0..4 {
        let mut input = spill();
        let blocks = &mut input.functions[0].body.as_mut().unwrap().blocks;
        match mode {
            0 => {
                let Kind::Storage(Storage::ReadValue { access, .. }) =
                    &mut blocks[3].operations[0].kind
                else {
                    unreachable!()
                };
                access.volatile = true;
            }
            1 => blocks[1].operations.push(Operation::new(
                vec![],
                Kind::Storage(Storage::CopyObject {
                    source: ValueId(40),
                    destination: ValueId(40),
                    source_access: MemoryAccess::new(AddressSpace::Private, 8),
                    destination_access: MemoryAccess::new(AddressSpace::Private, 8),
                    overlap: fe2o3_kernel_ir::StorageCopyOverlapV1::MayOverlap,
                }),
            )),
            2 => {
                blocks[1].operations.push(Operation::new(
                    vec![],
                    Kind::Call {
                        callee: "escape".into(),
                        arguments: vec![ValueId(40)],
                    },
                ));
                input.functions.push(IrFunction::external_import(
                    "escape",
                    Signature::new(vec![pointer(AccessMode::ReadWrite)], vec![]),
                ));
            }
            _ => {
                blocks[2].terminator = Some(Terminator::ConditionalBranch {
                    condition: ValueId(2),
                    then_target: BlockId(17),
                    then_arguments: vec![ValueId(42)],
                    else_target: BlockId(7),
                    else_arguments: vec![ValueId(42)],
                });
            }
        }
        assert!(check(&input).is_err(), "mode {mode}");
    }
}

#[test]
fn private_uniformity_exact_and_one_short_budgets() {
    let mut chain = spill();
    let blocks = &mut chain.functions[0].body.as_mut().unwrap().blocks;
    blocks[2].terminator = Some(Terminator::Branch {
        target: BlockId(1000),
        arguments: vec![ValueId(42)],
    });
    for n in (0..64).rev() {
        let mut block = BasicBlock::new(BlockId(1000 + n));
        block.parameters.push(ValueDef::new(
            ValueId(100 + n),
            pointer(AccessMode::ReadOnly),
        ));
        block.terminator = Some(Terminator::Branch {
            target: BlockId(if n == 63 { 7 } else { 1001 + n }),
            arguments: vec![ValueId(100 + n)],
        });
        blocks.push(block);
    }
    for input in [spill(), chain] {
        with_inventory(&input, |inventory| {
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(31).unwrap();
            check_canonical_tile_convergence_v160(inventory, Function(0), &mut budget).unwrap();
            let used = budget.work();
            let peak = budget.peak_storage();
            assert!(used < 1_000_000, "bounded reverse-order phi chain: {used}");
            for (w, s) in [(used, peak), (used - 1, peak), (used, peak - 1)] {
                let mut work = Work::new(w);
                let mut budget = Budget::new(&mut work, s);
                budget.reserve_storage(31).unwrap();
                let result =
                    check_canonical_tile_convergence_v160(inventory, Function(0), &mut budget);
                if w == used && s == peak {
                    result.unwrap();
                } else {
                    assert!(matches!(result, Err(Error::Resource(_))));
                }
                assert_eq!(budget.storage(), 31);
            }
        });
    }
}
