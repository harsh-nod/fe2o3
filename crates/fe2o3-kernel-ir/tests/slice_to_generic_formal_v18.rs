use fe2o3_kernel_ir::*;

fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn slice(space: AddressSpace) -> Type {
    Type::slice(Type::Scalar(ScalarType::U32), space, AccessMode::ReadOnly)
}
fn pointer(space: AddressSpace) -> Type {
    Type::pointer(Type::Scalar(ScalarType::U32), space, AccessMode::ReadOnly)
}
fn kernel(parameters: Vec<Type>, mut blocks: Vec<BasicBlock>) -> Module {
    for block in &mut blocks {
        if block.terminator.is_none() {
            block.terminator = Some(Terminator::Return { values: vec![] });
        }
    }
    let values = (0..parameters.len() as u32).map(ValueId).collect();
    let mut module = Module::new("slice-formal");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(parameters, vec![]),
        values,
        blocks,
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}
fn expose(id: u32, source: u32) -> Operation {
    op(
        id,
        slice(AddressSpace::Generic),
        OperationKind::Cast {
            kind: CastKind::SliceToGeneric,
            value: ValueId(source),
            to: slice(AddressSpace::Generic),
        },
    )
}

#[test]
fn whole_descriptor_exposure_preserves_concrete_formal_roots_and_inert_receipts() {
    for space in [
        AddressSpace::Global,
        AddressSpace::Constant,
        AddressSpace::Private,
        AddressSpace::Workgroup,
    ] {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = vec![
            expose(1, 0),
            op(
                2,
                pointer(AddressSpace::Generic),
                OperationKind::SliceData { slice: ValueId(1) },
            ),
            op(
                3,
                Type::Scalar(ScalarType::U32),
                OperationKind::Load {
                    pointer: ValueId(2),
                    access: MemoryAccess::new(AddressSpace::Generic, 4),
                },
            ),
        ];
        let report = derive_kernel_memory_obligations(
            &kernel(vec![slice(space)], vec![block]),
            &KernelId::new("kernel"),
            ExplicitLaunchExtent1d::Exact(1),
            FormalIndexWidth::Bits64,
        )
        .unwrap();
        assert!(report.is_complete(), "{report:?}");
        let accesses = report.obligations().accesses();
        if space == AddressSpace::Private {
            assert!(accesses.is_empty());
        } else {
            assert_eq!(accesses.len(), 1);
            assert_eq!(accesses[0].allocation().parameter_index(), 0);
            assert_eq!(accesses[0].address_space(), space);
            let receipt = InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(
                report.obligations(),
            )
            .unwrap();
            receipt.revalidate().unwrap();
            let replay = InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(
                receipt.canonical_bytes().to_vec(),
            )
            .unwrap();
            assert_eq!(receipt.canonical_bytes(), replay.canonical_bytes());
        }
    }
}

// Bound and data may use the original or widened whole descriptor. In every
// case they must resolve to one exact source, not merely an equal slice type.
fn guarded(
    space: AddressSpace,
    length_after: bool,
    data_after: bool,
    foreign: bool,
    bounded: bool,
) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    let parameters = vec![slice(space), Type::INDEX, Type::BOOL, slice(space)];
    if space != AddressSpace::Generic {
        entry.operations.extend([expose(4, 0), expose(5, 3)]);
    }
    let length = if length_after && space != AddressSpace::Generic {
        4
    } else {
        0
    };
    entry.operations.extend([
        op(
            6,
            Type::INDEX,
            OperationKind::SliceLength {
                slice: ValueId(length),
            },
        ),
        op(
            7,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(1),
                rhs: ValueId(6),
            },
        ),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(if bounded { 7 } else { 2 }),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let widened = data_after && space != AddressSpace::Generic;
    let data = match (widened, foreign) {
        (true, false) => 4,
        (true, true) => 5,
        (false, false) => 0,
        (false, true) => 3,
    };
    let data_space = if widened {
        AddressSpace::Generic
    } else {
        space
    };
    let mut read = BasicBlock::new(BlockId(1));
    read.operations = vec![
        op(
            10,
            pointer(data_space),
            OperationKind::SliceData {
                slice: ValueId(data),
            },
        ),
        op(
            11,
            pointer(data_space),
            OperationKind::GetElementPointer {
                base: ValueId(10),
                offset: ValueId(1),
            },
        ),
        op(
            12,
            Type::Scalar(ScalarType::U32),
            OperationKind::Load {
                pointer: ValueId(11),
                access: MemoryAccess::new(data_space, 4),
            },
        ),
    ];
    kernel(parameters, vec![entry, read, BasicBlock::new(BlockId(2))])
}

fn query(module: &Module, expected: bool, global_effects: usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000_000);
    budget.reserve_storage(17).unwrap();
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            StorageLayoutLimitsV1 {
                rows: 8,
                edges: 16,
                containment_depth: 8,
                object_bytes: 1024,
            },
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    with_canonical_guarded_global_reads_v18(
        &owner,
        Default::default(),
        &mut budget,
        |view, budget| {
            assert_eq!(
                view.function_effects(CanonicalKirFunctionCoordinateV1(0), budget)?,
                (global_effects, 0, 0)
            );
            let result = view.read_at(
                CanonicalKirOperationCoordinateV1 {
                    block: CanonicalKirBlockCoordinateV1 {
                        function: CanonicalKirFunctionCoordinateV1(0),
                        block: 1,
                    },
                    operation: 2,
                },
                budget,
            )?;
            match result {
                CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(read) => {
                    assert!(expected);
                    assert_eq!(read.domain().slice(), ValueId(0));
                    assert_eq!(read.domain().pointer(), ValueId(11));
                    assert_eq!(read.domain().index(), ValueId(1));
                }
                CanonicalGuardedGlobalReadOutcomeV18::NotProved(reason) => {
                    assert!(!expected, "{reason:?}")
                }
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 17);
}

#[test]
fn actual_v18_guarded_reads_keep_length_data_correlation_across_whole_slice_widening() {
    for length_after in [false, true] {
        for data_after in [false, true] {
            for foreign in [false, true] {
                for bounded in [false, true] {
                    query(
                        &guarded(
                            AddressSpace::Global,
                            length_after,
                            data_after,
                            foreign,
                            bounded,
                        ),
                        !foreign && bounded,
                        1,
                    );
                }
            }
        }
    }
}

#[test]
fn unresolved_generic_and_non_global_descriptors_never_become_global_read_proofs() {
    query(
        &guarded(AddressSpace::Generic, false, false, false, true),
        false,
        1,
    );
    query(
        &guarded(AddressSpace::Private, true, true, false, true),
        false,
        0,
    );
}

#[test]
fn equal_typed_selected_foreign_slices_cannot_mint_a_unique_formal_allocation() {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        expose(3, 0),
        expose(4, 1),
        op(
            5,
            slice(AddressSpace::Generic),
            OperationKind::Select {
                condition: ValueId(2),
                true_value: ValueId(3),
                false_value: ValueId(4),
            },
        ),
        op(
            6,
            pointer(AddressSpace::Generic),
            OperationKind::SliceData { slice: ValueId(5) },
        ),
        op(
            7,
            Type::Scalar(ScalarType::U32),
            OperationKind::Load {
                pointer: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Generic, 4),
            },
        ),
    ];
    let report = derive_kernel_memory_obligations(
        &kernel(
            vec![
                slice(AddressSpace::Global),
                slice(AddressSpace::Global),
                Type::BOOL,
            ],
            vec![block],
        ),
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(1),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(!report.is_complete());
}

#[test]
fn same_root_slice_loop_keeps_its_bound_but_conflicting_backedges_do_not() {
    for conflicting in [false, true] {
        for foreign_space in [AddressSpace::Global, AddressSpace::Private] {
            let mut source = guarded(AddressSpace::Global, true, true, false, true);
            let function = &mut source.functions[0];
            function.signature.parameters[3] = slice(foreign_space);
            let blocks = &mut function.body.as_mut().unwrap().blocks;
            let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
                &mut blocks[0].terminator
            else {
                unreachable!()
            };
            then_arguments.push(ValueId(4));
            blocks[1]
                .parameters
                .push(ValueDef::new(ValueId(20), slice(AddressSpace::Generic)));
            blocks[1].operations[0].kind = OperationKind::SliceData { slice: ValueId(20) };
            blocks[1].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(2),
                then_target: BlockId(1),
                then_arguments: vec![ValueId(if conflicting { 5 } else { 20 })],
                else_target: BlockId(2),
                else_arguments: vec![],
            });
            query(&source, !conflicting, 1);
        }
    }
}

fn same_descriptor_loop() -> Module {
    let mut source = guarded(AddressSpace::Global, true, true, false, true);
    let blocks = &mut source.functions[0].body.as_mut().unwrap().blocks;
    let Some(Terminator::ConditionalBranch { then_arguments, .. }) = &mut blocks[0].terminator
    else {
        unreachable!()
    };
    then_arguments.push(ValueId(4));
    blocks[1]
        .parameters
        .push(ValueDef::new(ValueId(20), slice(AddressSpace::Generic)));
    blocks[1].operations[0].kind = OperationKind::SliceData { slice: ValueId(20) };
    blocks[1].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(20)],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    source
}

#[test]
fn same_descriptor_multi_block_and_nested_loops_keep_the_original_guard() {
    for nested in [false, true] {
        let mut source = same_descriptor_loop();
        let blocks = &mut source.functions[0].body.as_mut().unwrap().blocks;
        blocks[1].terminator = Some(Terminator::Branch {
            target: BlockId(3),
            arguments: vec![],
        });
        let mut latch = BasicBlock::new(BlockId(3));
        latch.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(if nested { 4 } else { 1 }),
            then_arguments: if nested { vec![] } else { vec![ValueId(20)] },
            else_target: BlockId(if nested { 1 } else { 2 }),
            else_arguments: if nested { vec![ValueId(20)] } else { vec![] },
        });
        blocks.push(latch);
        if nested {
            let mut inner = BasicBlock::new(BlockId(4));
            inner.terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(2),
                then_target: BlockId(3),
                then_arguments: vec![],
                else_target: BlockId(1),
                else_arguments: vec![ValueId(20)],
            });
            blocks.push(inner);
        }
        query(&source, true, 1);
    }
}

#[test]
fn entering_guard_does_not_cover_a_changed_index_or_an_alternate_entry() {
    for change in 0..4 {
        let mut source = same_descriptor_loop();
        let blocks = &mut source.functions[0].body.as_mut().unwrap().blocks;
        match change {
            0 => {
                blocks[0].operations.push(op(
                    8,
                    Type::INDEX,
                    OperationKind::Constant(Constant::Index(0)),
                ));
                let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
                    &mut blocks[0].terminator
                else {
                    unreachable!()
                };
                then_arguments.push(ValueId(1));
                blocks[1]
                    .parameters
                    .push(ValueDef::new(ValueId(21), Type::INDEX));
                blocks[1].operations[1].kind = OperationKind::GetElementPointer {
                    base: ValueId(10),
                    offset: ValueId(21),
                };
                let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
                    &mut blocks[1].terminator
                else {
                    unreachable!()
                };
                then_arguments.push(ValueId(8));
            }
            1 => {
                let Some(Terminator::ConditionalBranch { else_target, .. }) =
                    &mut blocks[0].terminator
                else {
                    unreachable!()
                };
                *else_target = BlockId(3);
                let mut alternate = BasicBlock::new(BlockId(3));
                alternate.terminator = Some(Terminator::Branch {
                    target: BlockId(1),
                    arguments: vec![ValueId(4)],
                });
                blocks.push(alternate);
            }
            2 => {
                let Some(Terminator::ConditionalBranch {
                    else_target,
                    else_arguments,
                    ..
                }) = &mut blocks[0].terminator
                else {
                    unreachable!()
                };
                *else_target = BlockId(1);
                *else_arguments = vec![ValueId(4)];
            }
            3 => {
                blocks[0].terminator = Some(Terminator::Branch {
                    target: BlockId(1),
                    arguments: vec![ValueId(4)],
                });
                let Some(Terminator::ConditionalBranch { condition, .. }) =
                    &mut blocks[1].terminator
                else {
                    unreachable!()
                };
                *condition = ValueId(7);
            }
            _ => unreachable!(),
        }
        query(&source, false, 1);
    }
}

#[test]
fn unreachable_incoming_descriptor_does_not_erase_the_real_entering_guard() {
    let mut source = same_descriptor_loop();
    let mut unreachable = BasicBlock::new(BlockId(3));
    unreachable.operations.push(expose(30, 3));
    unreachable.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(30)],
    });
    source.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(unreachable);
    query(&source, true, 1);
}
