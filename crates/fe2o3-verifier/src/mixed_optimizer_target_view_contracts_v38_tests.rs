use super::super::target_view_contracts_v38::{
    TargetByteTagClassV38 as TagClass, TargetByteViewContractsV38 as Contracts,
};
use fe2o3_kernel_ir::{
    StorageFieldV1 as TagField, StorageLayoutIdV1 as TagId, StorageLayoutKindV1 as TagKind,
    StorageLayoutV1 as TagLayout, StorageVariantEncodingV1 as TagEncoding,
    StorageVariantV1 as TagVariant,
};

fn tag_module_v38(copies: usize, scalar: Option<ScalarType>, niche: bool) -> Module {
    let mut module = memory_module(1);
    for _ in 0..copies {
        let tag = module.storage_layouts.len() as u32;
        let size = scalar.map_or(8, |scalar| {
            scalar_bytes(scalar, FormalIndexWidth::Bits64).unwrap()
        });
        module.storage_layouts.push(TagLayout {
            size: size as u64,
            alignment: 1,
            kind: scalar.map_or_else(
                || {
                    TagKind::Pointer(fe2o3_kernel_ir::StoragePointerV1 {
                        pointee: TagId(tag + 1),
                        value_space: AddressSpace::Private,
                        encoded_space: AddressSpace::Generic,
                        access: AccessMode::ReadWrite,
                        stored_bits: 64,
                    })
                },
                TagKind::Scalar,
            ),
        });
        module.storage_layouts.push(TagLayout {
            size: 0,
            alignment: 1,
            kind: TagKind::Record(vec![].into_boxed_slice()),
        });
        let field = TagField {
            offset: 3,
            layout: TagId(tag),
        };
        let encoding = if niche {
            TagEncoding::Niche {
                tag: field,
                untagged_variant: 0,
                first_niche_variant: 1,
                last_niche_variant: 1,
                niche_start: if scalar == Some(ScalarType::Bool) {
                    2
                } else {
                    0
                },
            }
        } else {
            TagEncoding::Direct { tag: field }
        };
        let high = if size == 16 {
            u128::MAX
        } else {
            (1u128 << (size * 8)) - 1
        };
        module.storage_layouts.push(TagLayout {
            size: size as u64 + 3,
            alignment: 1,
            kind: TagKind::Variants {
                encoding,
                variants: vec![
                    TagVariant {
                        discriminant: if niche { 0 } else { 123 },
                        direct_tag_bits: (!niche).then_some(high),
                        uninhabited: false,
                        layout: TagId(tag + 1),
                    },
                    TagVariant {
                        discriminant: if niche { 1 } else { 456 },
                        direct_tag_bits: (!niche).then_some(0),
                        uninhabited: true,
                        layout: TagId(tag + 1),
                    },
                ]
                .into_boxed_slice(),
            },
        });
    }
    module
}

fn tag_header_oracle_v38() -> usize {
    type Fields<'a, 'b> = (
        &'a Inventory<'b>,
        FormalIndexWidth,
        usize,
        usize,
        Ledger,
        Cell<Option<Resource>>,
    );
    assert_eq!(size_of::<Contracts<'_, '_>>(), size_of::<Fields<'_, '_>>());
    size_of::<Contracts<'_, '_>>()
        + 2 * size_of::<Result<Contracts<'_, '_>>>()
        + 2 * size_of::<Result<TagClass>>()
        + 2 * size_of::<Result<()>>()
        + 2 * size_of::<std::result::Result<(), Resource>>()
        + 2 * size_of::<TagClass>()
        + size_of::<(
            [&Inventory<'_>; 2],
            [&TagLayout; 4],
            &mut Writer<'_, '_>,
            &[TagLayout],
            std::slice::Iter<'static, TagLayout>,
            std::iter::Enumerate<std::slice::Iter<'static, TagLayout>>,
            std::slice::Iter<'static, TagVariant>,
            [usize; 12],
            [u128; 2],
            [&str; 2],
            TagEncoding,
            FormalIndexWidth,
        )>()
}

#[test]
fn byte_target_tag_registry_preserves_owner_ids_physical_tags_and_logical_discriminants() {
    for scalar in [
        ScalarType::I8,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U64,
        ScalarType::I128,
        ScalarType::U128,
    ] {
        with_inventory(
            &tag_module_v38(1, Some(scalar), false),
            |inventory, _, floor| {
                let text = run(floor, LIMIT, LIMIT, |out| {
                    let contracts = Contracts::derive(inventory, FormalIndexWidth::Bits64, out)?;
                    assert_eq!(contracts.row_class(TagId(0), out)?, TagClass::NotVariant);
                    assert_eq!(contracts.row_class(TagId(2), out)?, TagClass::DirectScalar);
                    assert!(contracts.row_class(TagId(3), out).is_err());
                    contracts.emit(73, out)
                })
                .0
                .unwrap();
                let bytes = scalar_bytes(scalar, FormalIndexWidth::Bits64).unwrap();
                let high = if bytes == 16 {
                    u128::MAX
                } else {
                    (1u128 << (bytes * 8)) - 1
                };
                assert!(text.contains("owner: 73, rows: Map::empty()"));
                assert_eq!(text.matches(".insert(").count(), 1);
                assert!(text.contains(&format!(".insert(2, MemoryTagContractV38 {{ object_bytes: {}, tag_offset: 3, tag_width: {bytes}", bytes + 3)));
                assert!(text.contains(
                    "little_endian, inhabited: seq![true,false,], discriminants: seq![123int,456int,]"
                ));
                assert!(text.contains(&format!(
                    "MemoryTagEncodingV38::Direct {{ tags: seq![{high}int,0int,] }}"
                )));
                assert!(text.contains(
                    "memory.view_contracts == byte_target_view_contracts_73_v38(little_endian)"
                ));
                assert!(!text.contains("assume("));
            },
        );
    }
}

#[test]
fn byte_target_scalar_niches_are_physical_only_and_pointer_niches_are_explicitly_absent() {
    for scalar in [
        Some(ScalarType::Bool),
        Some(ScalarType::U8),
        Some(ScalarType::U128),
        None,
    ] {
        let mut module = tag_module_v38(1, scalar, true);
        if scalar.is_none() {
            let TagKind::Variants {
                encoding: TagEncoding::Niche { niche_start, .. },
                ..
            } = &mut module.storage_layouts[2].kind
            else {
                unreachable!()
            };
            *niche_start = 1;
        }
        with_inventory(&module, |inventory, _, floor| {
            let text = run(floor, LIMIT, LIMIT, |out| {
                let contracts = Contracts::derive(inventory, FormalIndexWidth::Bits64, out)?;
                assert_eq!(
                    contracts.row_class(TagId(2), out)?,
                    if scalar.is_some() {
                        TagClass::ScalarNiche
                    } else {
                        TagClass::UnsupportedPointerNiche
                    }
                );
                contracts.emit(91, out)
            })
            .0
            .unwrap();
            if let Some(scalar) = scalar {
                let max = match scalar {
                    ScalarType::Bool => 1,
                    ScalarType::U8 => 255,
                    ScalarType::U128 => u128::MAX,
                    _ => unreachable!(),
                };
                assert!(text.contains(&format!("untagged_valid_bits: seq![(0,{max}int)]")));
                assert!(text.contains(
                    "MemoryTagEncodingV38::Niche { untagged: 0, first: 1, last: 1, start:"
                ));
            } else {
                assert!(!text.contains(".insert("));
                assert!(!text.contains("MemoryTagEncodingV38::Niche"));
            }
        });
    }
}

#[test]
fn byte_target_null_pointer_niches_use_stored_width_and_no_integer_validity_range() {
    for bits in [8u16, 16, 32, 64, 128] {
        for index in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
            let mut module = tag_module_v38(1, None, true);
            let TagKind::Pointer(pointer) = &mut module.storage_layouts[0].kind else {
                unreachable!()
            };
            pointer.stored_bits = bits;
            module.storage_layouts[0].size = u64::from(bits / 8);
            module.storage_layouts[2].size = u64::from(bits / 8) + 3;
            let TagKind::Variants { variants, .. } = &mut module.storage_layouts[2].kind else {
                unreachable!()
            };
            variants[1].uninhabited = false;
            with_inventory(&module, |inventory, _, floor| {
                let text = run(floor, LIMIT, LIMIT, |out| {
                    let contracts = Contracts::derive(inventory, index, out)?;
                    assert_eq!(
                        contracts.row_class(TagId(2), out)?,
                        TagClass::PointerNullNiche
                    );
                    contracts.emit(82, out)
                })
                .0
                .unwrap();
                assert_eq!(text.matches(".insert(").count(), 1);
                assert!(text.contains(&format!(
                    "tag_offset: 3, tag_width: {}, little_endian",
                    bits / 8
                )));
                assert!(text.contains("inhabited: seq![true,true,]"));
                assert!(text.contains("untagged_valid_bits: seq![]"));
                assert!(
                    text.contains("MemoryTagEncodingV38::PointerNullNiche { nonnull: 0, null: 1 }")
                );
                assert!(!text.contains("MemoryTagEncodingV38::Niche {"));
                assert!(!text.contains("memory_value_modulus"));
            });
        }
    }
}

#[test]
fn byte_target_pointer_niche_rejects_multi_value_niches_without_approximating_null() {
    let mut module = tag_module_v38(1, None, true);
    let TagKind::Variants { encoding, variants } = &mut module.storage_layouts[2].kind else {
        unreachable!()
    };
    let TagEncoding::Niche {
        last_niche_variant, ..
    } = encoding
    else {
        unreachable!()
    };
    *last_niche_variant = 2;
    let mut entries = variants.to_vec();
    entries.push(TagVariant {
        discriminant: 2,
        direct_tag_bits: None,
        uninhabited: false,
        layout: TagId(1),
    });
    *variants = entries.into_boxed_slice();
    with_inventory(&module, |inventory, _, floor| {
        let text = run(floor, LIMIT, LIMIT, |out| {
            let contracts = Contracts::derive(inventory, FormalIndexWidth::Bits64, out)?;
            assert_eq!(
                contracts.row_class(TagId(2), out)?,
                TagClass::UnsupportedPointerNiche
            );
            contracts.emit(84, out)
        })
        .0
        .unwrap();
        assert!(!text.contains(".insert("));
        assert!(!text.contains("PointerNullNiche"));
    });
}

#[test]
fn byte_target_pointer_niche_resources_have_independent_width_and_linear_row_oracles() {
    for copies in [1, 4, 16] {
        with_inventory(
            &tag_module_v38(copies, None, true),
            |inventory, _, floor| {
                let storage = floor + super::super::super::SOURCE_LIMIT + tag_header_oracle_v38();
                let work = 3 + 6 * (3 * copies) + 2 * (2 * copies) + 4 * copies;
                let derive = |out: &mut Writer<'_, '_>| {
                    Contracts::derive(inventory, FormalIndexWidth::Bits64, out).map(|_| ())
                };
                let exact = run(floor, work, storage, derive);
                assert!(exact.0.unwrap().is_empty());
                assert_eq!((exact.1, exact.2), (work, storage));
                assert!(matches!(run(floor, work - 1, storage, derive).0,
                Err(Error::Resource(Resource::Work(error))) if error.limit() == work - 1 && error.actual() == work));
                assert!(matches!(run(floor, work, storage - 1, derive).0,
                Err(Error::Resource(Resource::Storage(error))) if error.limit() == storage - 1 && error.actual() == storage));
                let emit = |out: &mut Writer<'_, '_>| {
                    Contracts::derive(inventory, FormalIndexWidth::Bits64, out)?.emit(86, out)
                };
                let measured = run(floor, LIMIT, storage, emit);
                let text = measured.0.unwrap();
                let emitted =
                    work + 1 + 6 * (3 * copies) + 4 * copies + 2 * (2 * copies) + text.len();
                assert_eq!((measured.1, measured.2), (emitted, storage));
                assert_eq!(run(floor, emitted, storage, emit).0.unwrap(), text);
                assert!(matches!(run(floor, emitted - 1, storage, emit).0,
                Err(Error::Resource(Resource::Work(error))) if error.limit() == emitted - 1 && error.actual() == emitted));
            },
        );
    }
}

#[test]
fn byte_target_tag_contract_derivation_and_emission_have_linear_independent_resource_oracles() {
    assert_eq!(
        super::super::target_view_contracts_v38::headers(),
        tag_header_oracle_v38()
    );
    for (copies, variants) in [(1, 2), (4, 2), (16, 2), (1, 7), (1, 16), (8, 16)] {
        let mut module = tag_module_v38(copies, Some(ScalarType::U8), false);
        for row in &mut module.storage_layouts {
            if let TagKind::Variants {
                variants: entries, ..
            } = &mut row.kind
            {
                let payload = entries[0].layout;
                *entries = (0..variants)
                    .map(|index| TagVariant {
                        discriminant: 1000 + index as u128,
                        direct_tag_bits: Some(index as u128),
                        uninhabited: false,
                        layout: payload,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
            }
        }
        with_inventory(&module, |inventory, _, floor| {
            let derive = |out: &mut Writer<'_, '_>| {
                Contracts::derive(inventory, FormalIndexWidth::Bits64, out).map(|_| ())
            };
            let work = 3 + 6 * (3 * copies) + 2 * (variants * copies);
            let storage = floor + super::super::super::SOURCE_LIMIT + tag_header_oracle_v38();
            let exact = run(floor, work, storage, derive);
            assert!(exact.0.unwrap().is_empty());
            assert_eq!((exact.1, exact.2), (work, storage));
            assert!(matches!(run(floor, work - 1, storage, derive).0,
                Err(Error::Resource(Resource::Work(error))) if error.limit() == work - 1 && error.actual() == work));
            assert!(matches!(run(floor, work, storage - 1, derive).0,
                Err(Error::Resource(Resource::Storage(error))) if error.limit() == storage - 1 && error.actual() == storage));
            let emit = |out: &mut Writer<'_, '_>| {
                Contracts::derive(inventory, FormalIndexWidth::Bits64, out)?.emit(8, out)
            };
            let measured = run(floor, LIMIT, storage, emit);
            let text = measured.0.unwrap();
            let emitted_work = work + 1 + 6 * (3 * copies) + 3 * (variants * copies) + text.len();
            assert_eq!((measured.1, measured.2), (emitted_work, storage));
            assert_eq!(run(floor, emitted_work, storage, emit).0.unwrap(), text);
            assert!(matches!(run(floor, emitted_work - 1, storage, emit).0,
                Err(Error::Resource(Resource::Work(error))) if error.limit() == emitted_work - 1 && error.actual() == emitted_work));
        });
    }
}

#[test]
fn byte_target_tag_contract_custody_retains_undercut_and_funded_foreign_ledger_failures() {
    for foreign in [false, true] {
        with_inventory(
            &tag_module_v38(1, Some(ScalarType::U8), false),
            |inventory, _, floor| {
                run(floor, LIMIT, LIMIT, |out| {
                    let contracts = Contracts::derive(inventory, FormalIndexWidth::Bits64, out)?;
                    let before = out.budget.work();
                    if foreign {
                        let mut work = Work::new(LIMIT);
                        let mut budget = Budget::new(&mut work, LIMIT);
                        budget.reserve_storage(out.budget.storage())?;
                        let mut other = Writer::new(&mut budget)?;
                        assert!(matches!(
                            contracts.row_class(TagId(2), &mut other),
                            Err(Error::Resource(Resource::Accounting))
                        ));
                        assert_eq!(other.budget.work(), 0);
                        assert!(other.finish()?.is_empty());
                    } else {
                        out.budget.release_storage(1)?;
                        assert!(matches!(
                            contracts.row_class(TagId(2), out),
                            Err(Error::Resource(Resource::Accounting))
                        ));
                        out.budget.reserve_storage(1)?;
                    }
                    assert!(matches!(
                        contracts.row_class(TagId(2), out),
                        Err(Error::Resource(Resource::Accounting))
                    ));
                    assert!(matches!(
                        contracts.emit(1, out),
                        Err(Error::Resource(Resource::Accounting))
                    ));
                    assert_eq!(out.budget.work(), before);
                    Ok(())
                })
                .0
                .unwrap();
            },
        );
    }
}

#[test]
fn byte_target_tag_contract_rejects_foreign_owner_and_mismatched_index_width_before_text() {
    let module = tag_module_v38(1, Some(ScalarType::Index), false);
    with_inventory(&module, |inventory, _, floor| {
        with_inventory(&module, |other, _, _| {
            assert!(
                run(floor, LIMIT, LIMIT, |out| {
                    let contracts = Contracts::derive(inventory, FormalIndexWidth::Bits64, out)?;
                    assert!(contracts.check_owner(other.owner(), out).is_err());
                    contracts.check_owner(inventory.owner(), out)
                })
                .0
                .unwrap()
                .is_empty()
            );
        });
        for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Unknown] {
            assert!(matches!(
                run(floor, LIMIT, LIMIT, |out| Contracts::derive(
                    inventory, width, out
                )
                .map(|_| ()))
                .0,
                Err(Error::Statement(_))
            ));
        }
    });
}
