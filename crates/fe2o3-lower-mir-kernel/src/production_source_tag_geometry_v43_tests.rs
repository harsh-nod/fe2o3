// These raw-KIR fixtures test only the independently solved physical equations.
// Genuine owner fixtures separately authenticate original source tag identities.
#[test]
fn whole_enum_tag_history_uses_byte_ranges_without_any_projected_value_access() {
    for (scalar, bytes) in [
        (ScalarType::U8, 1),
        (ScalarType::U32, 4),
        (ScalarType::U64, 8),
    ] {
        let (function, slots, layouts, accesses) = tag_graph_v43(scalar, bytes, false);
        assert!(
            function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .all(|operation| !matches!(
                    operation.kind,
                    OperationKind::Storage(ScopedObjectOperationV29::Project { .. })
                ))
        );
        let (result, _, _, completed) =
            run_tag_graph_v43(&function, &slots, &layouts, &accesses, &[], LIMIT, LIMIT);
        result.unwrap();
        assert!(completed);
    }
}

#[test]
fn tag_geometry_fixed_headers_are_independent_of_the_production_formula() {
    use std::mem::size_of;
    type Error = ProductionSemanticKirErrorV1;
    type Fields = (ValueId, MemoryAccess, Option<u32>);
    assert_eq!(size_of::<Fields>(), size_of::<SourceAddressTagAccessV43>());
    assert_eq!(
        source_tag_geometry_headers_v43().unwrap(),
        size_of::<Fields>()
            + size_of::<Option<Fields>>()
            + size_of::<Result<Option<Fields>, Error>>()
            + 3 * size_of::<Option<(u64, u64)>>()
            + 3 * size_of::<Result<Option<(u64, u64)>, Error>>()
            + size_of::<SourceStaticPointerCellV29>()
            + 12 * size_of::<usize>()
            + 8 * size_of::<&()>()
    );
}

fn tag_graph_v43(
    scalar: ScalarType,
    bytes: u64,
    niche: bool,
) -> (
    Function,
    Vec<ScopedSourceSlotV29>,
    Vec<fe2o3_kernel_ir::StorageLayoutV1>,
    Vec<SourceAddressAccessV29>,
) {
    use fe2o3_kernel_ir::{
        StorageFieldV1 as Field, StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind,
        StorageLayoutV1 as Layout, StorageVariantEncodingV1 as Encoding,
        StorageVariantV1 as Variant,
    };
    let (mut function, mut slots, _) = typed_currentness_fixture();
    slots.truncate(1);
    slots[0].origin.source = ScopedAllocationSourceV29::OriginalObject {
        cell: 0,
        schema: Id(2),
    };
    slots[0].representation = ScopedSlotRepresentationV29::Object {
        schema: Id(2),
        bytes,
        alignment: 1,
    };
    let tag = Field {
        offset: 0,
        layout: Id(0),
    };
    let layouts = vec![
        Layout {
            size: bytes,
            alignment: 1,
            kind: Kind::Scalar(scalar),
        },
        Layout {
            size: bytes,
            alignment: 1,
            kind: Kind::Record(vec![].into()),
        },
        Layout {
            size: bytes,
            alignment: 1,
            kind: Kind::Variants {
                encoding: if niche {
                    Encoding::Niche {
                        tag,
                        untagged_variant: 1,
                        first_niche_variant: 0,
                        last_niche_variant: 0,
                        niche_start: 0,
                    }
                } else {
                    Encoding::Direct { tag }
                },
                variants: (0..2)
                    .map(|variant| Variant {
                        discriminant: variant,
                        direct_tag_bits: (!niche).then_some(variant),
                        uninhabited: false,
                        layout: Id(1),
                    })
                    .collect::<Vec<_>>()
                    .into(),
            },
        },
    ];
    let mut root = allocation(A, Type::StorageObject(Id(2)));
    let OperationKind::Alloca { alignment, .. } = &mut root.kind else {
        unreachable!()
    };
    *alignment = 1;
    function.body.as_mut().unwrap().blocks[0].operations = vec![
        root,
        Operation::new(
            vec![],
            OperationKind::Storage(ScopedObjectOperationV29::SetDiscriminant {
                address: A,
                variant: 0,
                access: MemoryAccess::new(AddressSpace::Private, 1),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(LOADED, Type::Scalar(ScalarType::U128)),
            OperationKind::Storage(ScopedObjectOperationV29::ReadDiscriminant {
                address: A,
                access: MemoryAccess::new(AddressSpace::Private, 1),
            }),
        ),
    ];
    let accesses = [1, 2]
        .map(|operation| SourceAddressAccessV29 {
            block: BlockId(77),
            operation,
            footprint: 0,
            slot: 0,
        })
        .to_vec();
    (function, slots, layouts, accesses)
}

fn run_tag_graph_v43(
    function: &Function,
    slots: &[ScopedSourceSlotV29],
    layouts: &[fe2o3_kernel_ir::StorageLayoutV1],
    accesses: &[SourceAddressAccessV29],
    kills: &[SourceAddressKillV29],
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize, bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut completed = false;
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::prepare_with_layouts(
            function, slots, None, accesses, layouts, budget,
        )?
        .solve(slots, accesses, kills, budget)?;
        scoped_slot_uses_v29::check_expanded_scalar_addresses_v29(
            function, &graph, slots, accesses, kills, budget,
        )?;
        check_source_address_currentness_v29(
            function,
            &graph,
            slots,
            accesses,
            &[],
            &[true; 3][..slots.len()],
            &[],
            &[],
            budget,
        )?;
        completed = true;
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR);
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn typed_tag_ranges_enter_actual_initialization_and_currentness_at_every_integer_width() {
    for (scalar, bytes) in [
        (ScalarType::U8, 1),
        (ScalarType::U16, 2),
        (ScalarType::U32, 4),
        (ScalarType::U64, 8),
        (ScalarType::U128, 16),
    ] {
        for niche in [false, true] {
            let (function, slots, layouts, accesses) = tag_graph_v43(scalar, bytes, niche);
            let (result, _, _, completed) =
                run_tag_graph_v43(&function, &slots, &layouts, &accesses, &[], LIMIT, LIMIT);
            result.unwrap();
            assert!(completed);
        }
    }
}

#[test]
fn tag_read_requires_its_own_complete_range_and_untagged_noop_does_not_initialize_it() {
    for niche in [false, true] {
        let (mut function, slots, layouts, accesses) = tag_graph_v43(ScalarType::U64, 8, niche);
        if niche {
            let OperationKind::Storage(ScopedObjectOperationV29::SetDiscriminant {
                variant, ..
            }) = &mut function.body.as_mut().unwrap().blocks[0].operations[1].kind
            else {
                unreachable!()
            };
            *variant = 1;
        } else {
            function.body.as_mut().unwrap().blocks[0]
                .operations
                .swap(1, 2);
        }
        let (result, _, _, completed) =
            run_tag_graph_v43(&function, &slots, &layouts, &accesses, &[], LIMIT, LIMIT);
        assert!(result.is_err());
        assert!(!completed);
    }
}

#[test]
fn actual_tag_rows_cannot_be_omitted_duplicated_retargeted_or_misaligned() {
    for mode in 0..5 {
        let (mut function, slots, layouts, mut accesses) = tag_graph_v43(ScalarType::U32, 4, false);
        match mode {
            0 => {
                accesses.remove(0);
            }
            1 => {
                accesses.insert(0, accesses[0]);
            }
            2 => {
                accesses[0].slot = 1;
            }
            3 => {
                accesses[0].footprint = 1;
            }
            4 => {
                let OperationKind::Storage(ScopedObjectOperationV29::SetDiscriminant {
                    access,
                    ..
                }) = &mut function.body.as_mut().unwrap().blocks[0].operations[1].kind
                else {
                    unreachable!()
                };
                access.alignment = 2;
            }
            _ => unreachable!(),
        }
        let (result, _, _, completed) =
            run_tag_graph_v43(&function, &slots, &layouts, &accesses, &[], LIMIT, LIMIT);
        assert!(result.is_err(), "mutation {mode}");
        assert!(!completed);
    }
}

#[test]
fn tag_graph_has_exact_work_and_storage_boundaries_with_restored_scratch() {
    let (function, slots, layouts, accesses) = tag_graph_v43(ScalarType::U128, 16, false);
    let run = |work, storage| {
        run_tag_graph_v43(&function, &slots, &layouts, &accesses, &[], work, storage)
    };
    let (result, work, storage, completed) = run(LIMIT, LIMIT);
    result.unwrap();
    assert!(completed);
    let (result, exact_work, exact_storage, completed) = run(work, storage);
    result.unwrap();
    assert!(completed);
    assert_eq!((exact_work, exact_storage), (work, storage));
    assert!(matches!(
        run(work - 1, storage).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    assert!(matches!(
        run(work, storage - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}

#[test]
fn an_initialized_tag_does_not_waive_the_original_allocation_lifetime() {
    let (mut function, slots, layouts, mut accesses) = tag_graph_v43(ScalarType::U8, 1, false);
    let alias_type = Type::pointer(
        Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(2)),
        AddressSpace::Private,
        AccessMode::ReadOnly,
    );
    let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
    operations.insert(
        1,
        Operation::effect_free(
            ValueDef::new(EXPOSED_A, alias_type.clone()),
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: A,
                to: alias_type,
            },
        ),
    );
    let OperationKind::Storage(ScopedObjectOperationV29::ReadDiscriminant { address, .. }) =
        &mut operations[3].kind
    else {
        unreachable!();
    };
    *address = EXPOSED_A;
    for access in &mut accesses {
        access.operation += 1;
    }
    let (positive, _, _, completed) =
        run_tag_graph_v43(&function, &slots, &layouts, &accesses, &[], LIMIT, LIMIT);
    positive.unwrap();
    assert!(completed);
    for (restart, derived) in [(false, false), (false, true), (true, true)] {
        let mut function = function.clone();
        if !derived {
            let OperationKind::Storage(ScopedObjectOperationV29::ReadDiscriminant {
                address, ..
            }) = &mut function.body.as_mut().unwrap().blocks[0].operations[3].kind
            else {
                unreachable!();
            };
            *address = A;
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut checked = false;
        let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
            let graph = SourceAddressMemoryV29::prepare_with_layouts(
                &function, &slots, None, &accesses, &layouts, budget,
            )?
            .solve(&slots, &accesses, &[], budget)?;
            scoped_slot_uses_v29::check_expanded_scalar_addresses_v29(
                &function,
                &graph,
                &slots,
                &accesses,
                &[],
                budget,
            )?;
            let events = [
                SourceAddressLifetimeV29 {
                    block: BlockId(77),
                    gap: 3,
                    sequence: 0,
                    slot: 0,
                    live: false,
                },
                SourceAddressLifetimeV29 {
                    block: BlockId(77),
                    gap: 3,
                    sequence: 1,
                    slot: 0,
                    live: true,
                },
            ];
            checked = true;
            check_source_address_currentness_v29(
                &function,
                &graph,
                &slots,
                &accesses,
                &[],
                &[true],
                &events[..if restart { 2 } else { 1 }],
                &[],
                budget,
            )
        });
        assert!(
            checked,
            "negative must reach currentness after initialized byte history"
        );
        assert!(result.is_err(), "stale alias survived restart={restart}");
        unsupported(result);
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn restarted_tag_backing_requires_fresh_initialization_not_a_stale_tag() {
    for rewrite in [false, true] {
        let (mut function, slots, layouts, mut accesses) = tag_graph_v43(ScalarType::U8, 1, false);
        if rewrite {
            let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
            operations.insert(2, operations[1].clone());
            accesses.push(SourceAddressAccessV29 {
                operation: 3,
                ..accesses[1]
            });
        }
        let kills = [SourceAddressKillV29 {
            source_order: [0; 5],
            block: BlockId(77),
            gap: 2,
            slot: 0,
        }];
        let lifetimes = [false, true].map(|live| SourceAddressLifetimeV29 {
            block: BlockId(77),
            gap: 2,
            sequence: usize::from(live),
            slot: 0,
            live,
        });
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut reached_history = false;
        let mut completed = false;
        let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
            let graph = SourceAddressMemoryV29::prepare_with_layouts(
                &function, &slots, None, &accesses, &layouts, budget,
            )?
            .solve(&slots, &accesses, &kills, budget)?;
            reached_history = true;
            scoped_slot_uses_v29::check_expanded_scalar_addresses_v29(
                &function, &graph, &slots, &accesses, &kills, budget,
            )?;
            check_source_address_currentness_v29(
                &function,
                &graph,
                &slots,
                &accesses,
                &kills,
                &[true],
                &lifetimes,
                &[],
                budget,
            )?;
            completed = true;
            Ok(())
        });
        assert!(reached_history, "rewrite={rewrite}: {result:?}");
        assert_eq!(result.is_ok(), rewrite, "rewrite={rewrite}: {result:?}");
        assert_eq!(completed, rewrite);
        if !rewrite {
            unsupported(result);
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

fn tag_pointer_overwrite_graph_v43(
    overlap: bool,
    untagged_noop: bool,
    rewrite_pointer: bool,
) -> (
    Function,
    Vec<ScopedSourceSlotV29>,
    Vec<fe2o3_kernel_ir::StorageLayoutV1>,
    Vec<SourceAddressAccessV29>,
) {
    use fe2o3_kernel_ir::{
        StorageFieldV1 as Field, StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind,
        StorageLayoutV1 as Layout, StorageVariantEncodingV1 as Encoding,
        StorageVariantV1 as Variant,
    };
    let (mut function, mut slots, mut layouts) = typed_currentness_fixture();
    slots.truncate(2);
    slots[1].origin.source = ScopedAllocationSourceV29::OriginalObject {
        cell: 1,
        schema: Id(3),
    };
    slots[1].representation = ScopedSlotRepresentationV29::Object {
        schema: Id(3),
        bytes: 16,
        alignment: 8,
    };
    layouts.push(Layout {
        size: 1,
        alignment: 1,
        kind: Kind::Scalar(ScalarType::U8),
    });
    let tag = Field {
        offset: if overlap || untagged_noop { 0 } else { 8 },
        layout: if untagged_noop { Id(1) } else { Id(2) },
    };
    layouts.push(Layout {
        size: 16,
        alignment: 8,
        kind: Kind::Variants {
            encoding: if untagged_noop {
                Encoding::Niche {
                    tag,
                    untagged_variant: 1,
                    first_niche_variant: 0,
                    last_niche_variant: 0,
                    niche_start: 0,
                }
            } else {
                Encoding::Direct { tag }
            },
            variants: (0..2)
                .map(|variant| Variant {
                    discriminant: variant,
                    direct_tag_bits: (!untagged_noop).then_some(variant),
                    uninhabited: false,
                    layout: Id(1),
                })
                .collect::<Vec<_>>()
                .into(),
        },
    });
    let pointer = Type::pointer(
        Type::StorageObject(Id(0)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let mut root = allocation(A, Type::StorageObject(Id(0)));
    let OperationKind::Alloca { alignment, .. } = &mut root.kind else {
        unreachable!()
    };
    *alignment = 4;
    let pointer_write = || {
        Operation::new(
            vec![],
            OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                address: EXPOSED_A,
                value: A,
                access: MemoryAccess::new(AddressSpace::Private, 8),
            }),
        )
    };
    let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
    *operations = vec![
        root,
        allocation(B, Type::StorageObject(Id(3))),
        Operation::effect_free(
            ValueDef::new(
                EXPOSED_A,
                Type::pointer(
                    Type::StorageObject(Id(1)),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::Storage(ScopedObjectOperationV29::Project {
                base: B,
                step: ScopedObjectProjectionV29::VariantForWrite { index: 1 },
            }),
        ),
        pointer_write(),
        Operation::new(
            vec![],
            OperationKind::Storage(ScopedObjectOperationV29::SetDiscriminant {
                address: B,
                variant: u32::from(untagged_noop),
                access: MemoryAccess::new(AddressSpace::Private, 1),
            }),
        ),
    ];
    let mut accesses = vec![
        SourceAddressAccessV29 {
            block: BlockId(77),
            operation: 3,
            footprint: 0,
            slot: 1,
        },
        SourceAddressAccessV29 {
            block: BlockId(77),
            operation: 4,
            footprint: 0,
            slot: 1,
        },
    ];
    if rewrite_pointer {
        accesses.push(SourceAddressAccessV29 {
            block: BlockId(77),
            operation: operations.len(),
            footprint: 0,
            slot: 1,
        });
        operations.push(pointer_write());
    }
    accesses.push(SourceAddressAccessV29 {
        block: BlockId(77),
        operation: operations.len(),
        footprint: 0,
        slot: 1,
    });
    operations.push(Operation::effect_free(
        ValueDef::new(LOADED, pointer),
        OperationKind::Storage(ScopedObjectOperationV29::ReadValue {
            address: EXPOSED_A,
            access: MemoryAccess::new(AddressSpace::Private, 8),
        }),
    ));
    accesses.push(SourceAddressAccessV29 {
        block: BlockId(77),
        operation: operations.len(),
        footprint: 0,
        slot: 0,
    });
    operations.push(Operation::new(
        vec![],
        OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
            address: LOADED,
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        }),
    ));
    (function, slots, layouts, accesses)
}

#[test]
fn tag_writes_invalidate_only_overlapping_pointer_cells_and_noop_preserves_them() {
    for (overlap, noop, rewrite, succeeds) in [
        (false, false, false, true),
        (true, false, false, false),
        (true, false, true, true),
        (true, true, false, true),
    ] {
        let (function, slots, layouts, accesses) =
            tag_pointer_overwrite_graph_v43(overlap, noop, rewrite);
        let (result, _, _, completed) =
            run_tag_graph_v43(&function, &slots, &layouts, &accesses, &[], LIMIT, LIMIT);
        assert_eq!(
            result.is_ok(),
            succeeds,
            "overlap={overlap}, noop={noop}, rewrite={rewrite}: {result:?}"
        );
        assert_eq!(completed, succeeds);
    }
}
