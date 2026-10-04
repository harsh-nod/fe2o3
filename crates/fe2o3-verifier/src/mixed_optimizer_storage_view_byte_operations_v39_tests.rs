fn storage_view_header_oracle_v39() -> usize {
    use fe2o3_kernel_ir::{PointerType, StorageProjectionV1};
    type Fields = (usize, usize, usize, u64, u32, views::Action);
    assert_eq!(
        size_of::<StorageViewByteOperationV39>(),
        size_of::<Fields>()
    );
    size_of::<Fields>()
        + 2 * size_of::<Result<StorageViewByteOperationV39>>()
        + size_of::<views::Action>()
        + size_of::<views::ViewGuard>()
        + size_of::<(
            [&Inventory<'_>; 2],
            &mut Writer<'_, '_>,
            [&Type; 3],
            &PointerType,
            &[TagLayout],
            [&TagLayout; 3],
            [usize; 12],
            [u64; 6],
            [u128; 2],
            Option<(usize, u64, u64)>,
            Option<(u32, u64, u64, u32)>,
            TagEncoding,
            StorageProjectionV1,
            MemoryAccess,
            [Result<()>; 3],
            ByteMemoryStateNamesV30<'static>,
            ByteMemoryStateNamesV30<'static>,
            ByteMemoryContextNamesV30<'static>,
            [&str; 8],
        )>()
}

fn storage_view_module_v39(
    stored_bits: u16,
    scalar: Option<ScalarType>,
    niche: bool,
    copies: usize,
) -> Module {
    use fe2o3_kernel_ir::{
        StorageOperationV1 as Storage, StoragePointerV1, StorageProjectionV1 as Projection,
    };
    let bytes = u64::from(stored_bits / 8);
    let mut module = Module::new("general-storage-tag-dispatch");
    let pointer_layout = StoragePointerV1 {
        pointee: TagId(0),
        value_space: AddressSpace::Global,
        encoded_space: AddressSpace::Generic,
        access: AccessMode::ReadOnly,
        stored_bits,
    };
    module.storage_layouts = vec![
        TagLayout {
            size: 4,
            alignment: 4,
            kind: TagKind::Scalar(ScalarType::U32),
        },
        TagLayout {
            size: bytes,
            alignment: 1,
            kind: scalar.map_or(TagKind::Pointer(pointer_layout), TagKind::Scalar),
        },
        TagLayout {
            size: 0,
            alignment: 1,
            kind: TagKind::Record(vec![].into_boxed_slice()),
        },
        TagLayout {
            size: bytes + 3,
            alignment: 1,
            kind: TagKind::Record(
                vec![TagField {
                    offset: 3,
                    layout: TagId(1),
                }]
                .into_boxed_slice(),
            ),
        },
        TagLayout {
            size: bytes + 3,
            alignment: 1,
            kind: TagKind::Variants {
                encoding: if niche {
                    TagEncoding::Niche {
                        tag: TagField {
                            offset: 3,
                            layout: TagId(1),
                        },
                        untagged_variant: 1,
                        first_niche_variant: 0,
                        last_niche_variant: 0,
                        niche_start: 0,
                    }
                } else {
                    TagEncoding::Direct {
                        tag: TagField {
                            offset: 3,
                            layout: TagId(1),
                        },
                    }
                },
                variants: vec![
                    TagVariant {
                        discriminant: if niche { 0 } else { 41 },
                        direct_tag_bits: (!niche).then_some(17),
                        uninhabited: false,
                        layout: TagId(2),
                    },
                    TagVariant {
                        discriminant: if niche { 1 } else { 73 },
                        direct_tag_bits: (!niche).then_some(23),
                        uninhabited: false,
                        layout: TagId(3),
                    },
                ]
                .into_boxed_slice(),
            },
        },
    ];
    let pointer =
        |id, access| Type::pointer(Type::StorageObject(TagId(id)), AddressSpace::Global, access);
    let access = MemoryAccess::new(AddressSpace::Global, 1);
    let mut block = BasicBlock::new(BlockId(0));
    let mut next = 1;
    for _ in 0..copies {
        block.operations.push(KirOperation::new(
            vec![],
            OperationKind::Storage(Storage::SetDiscriminant {
                address: ValueId(0),
                variant: 0,
                access,
            }),
        ));
        block.operations.push(KirOperation::new(
            vec![ValueDef::new(ValueId(next), Type::Scalar(ScalarType::U128))],
            OperationKind::Storage(Storage::ReadDiscriminant {
                address: ValueId(0),
                access,
            }),
        ));
        block.operations.push(KirOperation::new(
            vec![ValueDef::new(
                ValueId(next + 1),
                pointer(2, AccessMode::ReadOnly),
            )],
            OperationKind::Storage(Storage::Project {
                base: ValueId(0),
                step: Projection::Variant { index: 0, access },
            }),
        ));
        block.operations.push(KirOperation::new(
            vec![ValueDef::new(
                ValueId(next + 2),
                pointer(3, AccessMode::WriteOnly),
            )],
            OperationKind::Storage(Storage::Project {
                base: ValueId(0),
                step: Projection::VariantForWrite { index: 1 },
            }),
        ));
        block.operations.push(KirOperation::new(
            vec![],
            OperationKind::Storage(Storage::SetDiscriminant {
                address: ValueId(0),
                variant: 1,
                access,
            }),
        ));
        next += 3;
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(KirFunction::internal_helper(
        "storage",
        Signature::new(vec![pointer(4, AccessMode::ReadWrite)], vec![]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

#[test]
fn byte_storage_views_dispatch_pointer_null_and_scalar_tags_with_logical_discriminants() {
    for (bits, scalar, niche) in [
        (8, None, true),
        (16, None, true),
        (32, None, true),
        (64, None, true),
        (128, None, true),
        (8, Some(ScalarType::U8), true),
        (128, Some(ScalarType::U128), true),
        (8, Some(ScalarType::I8), false),
        (128, Some(ScalarType::I128), false),
    ] {
        with_inventory(
            &storage_view_module_v39(bits, scalar, niche, 1),
            |inventory, physical, floor| {
                for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
                    let allocations = NoAllocations(inventory.owner());
                    let text = run(floor, LIMIT, LIMIT, |out| {
                        let contracts = Contracts::derive(inventory, width, out)?;
                        contracts.emit(1, out)?;
                        ByteFunctionV30::derive(
                            inventory,
                            physical,
                            Function(0),
                            ByteContext::classified(width, &contracts, 1),
                            &allocations,
                            out,
                        )?
                        .emit(101, out)
                    })
                    .0
                    .unwrap();
                    assert!(text.contains(if niche {
                        "discriminants: seq![0int,1int,]"
                    } else {
                        "discriminants: seq![41int,73int,]"
                    }));
                    assert!(text.contains(".discriminants[variant]"));
                    assert!(text.contains("MemoryTagReadPurposeV39::VariantValidation"));
                    assert!(text.contains("MemoryTagReadPurposeV39::DiscriminantRead"));
                    assert!(text.contains("byte_variant_projection_v39(s.memory, p, 4, 0, 0, 1)"));
                    assert!(text.contains(&format!(
                        "byte_offset: p.byte_offset + 3, ..p }}, {}, {}, little_endian)",
                        bits / 8,
                        if niche { 0 } else { 17 }
                    )));
                    assert!(text.contains(
                        "byte_target_view_contracts_match_1_v38(s.memory, little_endian)"
                    ));
                    assert!(!text.contains("MemoryValueV30::Scalar(pointer"));
                    assert!(!text.contains("assume("));
                }
            },
        );
    }
}

#[test]
fn byte_storage_variant_operations_stay_refused_without_owner_classified_context() {
    with_inventory(
        &storage_view_module_v39(64, None, true, 1),
        |inventory, physical, floor| {
            let allocations = NoAllocations(inventory.owner());
            let result = run(floor, LIMIT, LIMIT, |out| {
                ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    ByteContext::native(FormalIndexWidth::Bits64),
                    &allocations,
                    out,
                )
                .map(|_| ())
            })
            .0;
            assert!(matches!(
                result,
                Err(Error::Statement(
                    "actual tag operation requires checked view contracts"
                ))
            ));
        },
    );
}

#[test]
fn byte_storage_untagged_niche_is_inert_while_tag_overwrites_remain_ordered() {
    with_inventory(
        &storage_view_module_v39(64, None, true, 1),
        |inventory, physical, floor| {
            let allocations = NoAllocations(inventory.owner());
            let text = run(floor, LIMIT, LIMIT, |out| {
                let contracts = Contracts::derive(inventory, FormalIndexWidth::Bits64, out)?;
                ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    ByteContext::classified(FormalIndexWidth::Bits64, &contracts, 1),
                    &allocations,
                    out,
                )?
                .emit(107, out)
            })
            .0
            .unwrap();
            let noop = text
                .split_once("spec fn byte_operation_107_4_v30")
                .unwrap()
                .1
                .split("spec fn ")
                .next()
                .unwrap();
            assert!(noop.contains("let valid = s.valid;"));
            assert!(noop.contains("let values = s.values;"));
            assert!(noop.contains("let memory = s.memory;"));
            assert!(noop.contains("MemoryOperationEffectV30::Pure"));
            for forbidden in [
                "s.values[0]",
                "byte_range_live",
                "byte_read_tag",
                "byte_store",
                "byte_capture_tag",
                "write_epochs",
            ] {
                assert!(!noop.contains(forbidden), "{forbidden}");
            }
            let write = text
                .split_once("spec fn byte_operation_107_0_v30")
                .unwrap()
                .1
                .split("spec fn ")
                .next()
                .unwrap();
            assert!(write.contains("byte_store_v30"));
            assert!(write.contains("MemoryOperationEffectV30::Write"));
            assert!(!write.contains("byte_load"));
            assert!(!write.contains("byte_read_tag"));
            assert!(!write.contains("== 0"));
        },
    );
}

#[test]
fn byte_storage_view_emission_keeps_exact_and_one_short_resources_across_widths() {
    for bits in [8, 64, 128] {
        for copies in [1, 4, 16] {
            with_inventory(
                &storage_view_module_v39(bits, None, true, copies),
                |inventory, physical, floor| {
                    let allocations = NoAllocations(inventory.owner());
                    let generate = |out: &mut Writer<'_, '_>| {
                        let contracts =
                            Contracts::derive(inventory, FormalIndexWidth::Bits64, out)?;
                        contracts.emit(1, out)?;
                        ByteFunctionV30::derive(
                            inventory,
                            physical,
                            Function(0),
                            ByteContext::classified(FormalIndexWidth::Bits64, &contracts, 1),
                            &allocations,
                            out,
                        )?
                        .emit(111, out)
                    };
                    let measured = run(floor, LIMIT, LIMIT, generate);
                    let text = measured.0.unwrap();
                    assert_eq!(
                        text.matches("MemoryTagReadPurposeV39::VariantValidation")
                            .count(),
                        copies
                    );
                    assert_eq!(
                        text.matches("MemoryTagReadPurposeV39::DiscriminantRead")
                            .count(),
                        copies
                    );
                    assert_eq!(
                        run(floor, measured.1, measured.2, generate).0.unwrap(),
                        text
                    );
                    assert!(matches!(
                        run(floor, measured.1 - 1, measured.2, generate).0,
                        Err(Error::Resource(Resource::Work(_)))
                    ));
                    assert!(matches!(
                        run(floor, measured.1, measured.2 - 1, generate).0,
                        Err(Error::Resource(Resource::Storage(_)))
                    ));
                },
            );
        }
    }
}

#[test]
fn byte_storage_variant_helper_preserves_nested_views_and_checks_zero_width() {
    let text = super::super::byte_memory_v30::BYTE_MEMORY_V30;
    let read = text
        .split_once("spec fn byte_read_tag_v39")
        .unwrap()
        .1
        .split("spec fn ")
        .next()
        .unwrap();
    assert!(read.contains("byte_range_live_v30(memory, pointer, contract.object_bytes)"));
    assert!(
        read.contains("byte_range_aligned_v30(memory, byte_tag_address_v39(pointer, contract)")
    );
    let project = text
        .split_once("spec fn byte_variant_projection_v39")
        .unwrap()
        .1
        .split("spec fn ")
        .next()
        .unwrap();
    for required in [
        "byte_read_tag_v39",
        "byte_tag_observation_selects_v39",
        "byte_project_view_v38",
        "byte_capture_tag_guard_v38",
        "byte_extend_view_guard_v38",
    ] {
        assert!(project.contains(required));
    }
    assert!(!project.contains("payload_bytes == 0"));
    assert!(!project.contains("Seq::empty"));
    assert!(!project.contains("epochs:"));
    let laws = include_str!("mixed_optimizer_storage_view_dispatch_laws_v39.vrs");
    assert_eq!(laws.matches("proof fn ").count(), 6);
    assert!(!laws.contains("assume("));
    assert!(!laws.contains("external_body"));
}

#[test]
fn byte_storage_view_derivation_has_independent_linear_work_and_storage_oracles() {
    for copies in [1, 4, 16] {
        with_inventory(
            &storage_view_module_v39(64, None, true, copies),
            |inventory, physical, floor| {
                let allocations = NoAllocations(inventory.owner());
                let derive = |out: &mut Writer<'_, '_>| {
                    let contracts = Contracts::derive(inventory, FormalIndexWidth::Bits64, out)?;
                    ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        ByteContext::classified(FormalIndexWidth::Bits64, &contracts, 1),
                        &allocations,
                        out,
                    )
                    .map(|_| ())
                };
                // Classifier entry3, five rows(visit2+class4), pointer class4,
                // two variants2. Body owner1+context1+entry4, 1+3C definitions,
                // five actions/C: dispatch4+Alloca6+Storage6+32+View32,
                // context-owner1+class-owner1+pointer-class8+physical19.
                // Four declared memory effects/C cost3 each; empty control costs5.
                let work = 3
                    + 5 * (2 + 4)
                    + 4
                    + 2 * 2
                    + 1
                    + 1
                    + 4
                    + (1 + 3 * copies)
                    + 5 * copies * (4 + 6 + 6 + 32 + 32 + 1 + 1 + 8 + 19)
                    + 4 * copies * 3
                    + 5;
                let storage = floor
                    + super::super::super::SOURCE_LIMIT
                    + tag_header_oracle_v38()
                    + headers::<NoAllocations<'_>>()
                    + 5 * copies * size_of::<ByteOperationV30<'_, '_>>();
                let exact = run(floor, work, storage, derive);
                assert!(exact.0.unwrap().is_empty());
                assert_eq!((exact.1, exact.2), (work, storage));
                assert!(matches!(run(floor, work - 1, storage, derive).0,
                Err(Error::Resource(Resource::Work(error))) if error.limit() == work - 1 && error.actual() == work));
                assert!(matches!(run(floor, work, storage - 1, derive).0,
                Err(Error::Resource(Resource::Storage(error))) if error.limit() == storage - 1 && error.actual() == storage));
            },
        );
    }
}

#[test]
fn byte_storage_native_record_and_array_projection_keep_dynamic_bounds_and_packed_alignment() {
    use fe2o3_kernel_ir::{StorageOperationV1 as Storage, StorageProjectionV1 as Projection};
    let mut module = Module::new("native-packed-storage-projection");
    module.storage_layouts = vec![
        TagLayout {
            size: 4,
            alignment: 4,
            kind: TagKind::Scalar(ScalarType::U32),
        },
        TagLayout {
            size: 5,
            alignment: 1,
            kind: TagKind::Record(
                vec![TagField {
                    offset: 1,
                    layout: TagId(0),
                }]
                .into_boxed_slice(),
            ),
        },
        TagLayout {
            size: 16,
            alignment: 4,
            kind: TagKind::Array {
                element: TagId(0),
                length: 4,
                stride: 4,
            },
        },
    ];
    let pointer = |id| {
        Type::pointer(
            Type::StorageObject(TagId(id)),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        )
    };
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        KirOperation::new(
            vec![ValueDef::new(ValueId(3), pointer(0))],
            OperationKind::Storage(Storage::Project {
                base: ValueId(0),
                step: Projection::Field(0),
            }),
        ),
        KirOperation::new(
            vec![ValueDef::new(ValueId(4), pointer(0))],
            OperationKind::Storage(Storage::Project {
                base: ValueId(1),
                step: Projection::ArrayIndex(ValueId(2)),
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(KirFunction::internal_helper(
        "project",
        Signature::new(vec![pointer(1), pointer(2), Type::INDEX], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    with_inventory(&module, |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        let text = run(floor, LIMIT, LIMIT, |out| {
            ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                ByteContext::native(FormalIndexWidth::Bits64),
                &allocations,
                out,
            )?
            .emit(113, out)
        })
        .0
        .unwrap();
        assert!(text.contains("byte_project_view_v38(s.memory, p, 1, 4)"));
        assert!(text.contains("0 <= index < 4 && index < memory_value_modulus_v30(8)"));
        assert!(text.contains("byte_project_view_v38(s.memory, p, index * 4, 4)"));
        assert!(!text.contains("byte_range_aligned"));
        assert!(!text.contains("MemoryOperationEffectV30::TagRead"));
        assert!(!text.contains("byte_target_view_contracts_match"));
        assert_eq!(text.matches("MemoryOperationEffectV30::Pure").count(), 2);
    });
}
