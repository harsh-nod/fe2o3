use super::*;

#[test]
fn finite_table_comparison_covers_every_field_without_expanding_pointer_recursion() {
    let original = all_layouts();
    let mut module = unit_module();
    module.storage_layouts = original.clone();
    inspect(
        module.clone(),
        module,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| accepted(a, b, rows, floor),
    );
    // These private comparator probes intentionally include malformed mutations;
    // only the positive table above claims canonical admission.
    let mut variants = Vec::new();
    let mut mutate = |row: usize, change: fn(&mut StorageLayoutV1)| {
        let mut v = original.clone();
        change(&mut v[row]);
        variants.push(v);
    };
    mutate(0, |r| r.size += 1);
    mutate(0, |r| r.alignment *= 2);
    mutate(0, |r| r.kind = Kind::Scalar(ScalarType::I64));
    mutate(1, |r| r.kind = Kind::Scalar(ScalarType::U128));
    mutate(2, |r| {
        if let Kind::Pointer(p) = &mut r.kind {
            p.pointee = Id(0);
        }
    });
    mutate(2, |r| {
        if let Kind::Pointer(p) = &mut r.kind {
            p.value_space = AddressSpace::Global;
        }
    });
    mutate(2, |r| {
        if let Kind::Pointer(p) = &mut r.kind {
            p.encoded_space = AddressSpace::Private;
        }
    });
    mutate(2, |r| {
        if let Kind::Pointer(p) = &mut r.kind {
            p.access = AccessMode::ReadOnly;
        }
    });
    mutate(2, |r| {
        if let Kind::Pointer(p) = &mut r.kind {
            p.stored_bits = 32;
        }
    });
    mutate(3, |r| {
        if let Kind::Record(f) = &mut r.kind {
            f[1].offset -= 1;
        }
    });
    mutate(3, |r| {
        if let Kind::Record(f) = &mut r.kind {
            f[1].layout = Id(2);
        }
    });
    mutate(4, |r| {
        if let Kind::Union(f) = &mut r.kind {
            f.swap(0, 1);
        }
    });
    mutate(5, |r| {
        if let Kind::Array { element, .. } = &mut r.kind {
            *element = Id(1);
        }
    });
    mutate(5, |r| {
        if let Kind::Array { length, .. } = &mut r.kind {
            *length += 1;
        }
    });
    mutate(5, |r| {
        if let Kind::Array { stride, .. } = &mut r.kind {
            *stride += 1;
        }
    });
    mutate(6, |r| {
        if let Kind::Slice { element, .. } = &mut r.kind {
            *element = Id(0);
        }
    });
    mutate(6, |r| {
        if let Kind::Slice { value_space, .. } = &mut r.kind {
            *value_space = AddressSpace::Global;
        }
    });
    mutate(6, |r| {
        if let Kind::Slice { access, .. } = &mut r.kind {
            *access = AccessMode::ReadOnly;
        }
    });
    mutate(6, |r| {
        if let Kind::Slice { data, .. } = &mut r.kind {
            data.offset = 1;
        }
    });
    mutate(6, |r| {
        if let Kind::Slice { length, .. } = &mut r.kind {
            length.layout = Id(0);
        }
    });
    mutate(7, |r| {
        if let Kind::Variants {
            encoding: Encoding::Direct { tag },
            ..
        } = &mut r.kind
        {
            tag.offset -= 1;
        }
    });
    mutate(7, |r| {
        if let Kind::Variants { variants, .. } = &mut r.kind {
            variants[0].discriminant += 1;
        }
    });
    mutate(7, |r| {
        if let Kind::Variants { variants, .. } = &mut r.kind {
            variants[0].direct_tag_bits = Some(4);
        }
    });
    mutate(7, |r| {
        if let Kind::Variants { variants, .. } = &mut r.kind {
            variants[0].uninhabited = true;
        }
    });
    mutate(7, |r| {
        if let Kind::Variants { variants, .. } = &mut r.kind {
            variants[0].layout = Id(0);
        }
    });
    mutate(8, |r| {
        if let Kind::Variants {
            encoding: Encoding::Niche { tag, .. },
            ..
        } = &mut r.kind
        {
            tag.layout = Id(9);
        }
    });
    mutate(8, |r| {
        if let Kind::Variants {
            encoding: Encoding::Niche {
                untagged_variant, ..
            },
            ..
        } = &mut r.kind
        {
            *untagged_variant = 1;
        }
    });
    mutate(8, |r| {
        if let Kind::Variants {
            encoding:
                Encoding::Niche {
                    first_niche_variant,
                    ..
                },
            ..
        } = &mut r.kind
        {
            *first_niche_variant = 0;
        }
    });
    mutate(8, |r| {
        if let Kind::Variants {
            encoding: Encoding::Niche {
                last_niche_variant, ..
            },
            ..
        } = &mut r.kind
        {
            *last_niche_variant = 2;
        }
    });
    mutate(8, |r| {
        if let Kind::Variants {
            encoding: Encoding::Niche { niche_start, .. },
            ..
        } = &mut r.kind
        {
            *niche_start -= 1;
        }
    });
    drop(mutate);
    variants.push(original[..9].to_vec());
    assert_eq!(variants.len(), 31);
    for changed in variants {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 17);
        budget.reserve_storage(17).unwrap();
        assert_eq!(same_table(&original, &changed, &mut budget), Ok(false));
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn every_storage_execution_nonoperand_payload_is_exact_and_never_pure() {
    let access = MemoryAccess::new(AddressSpace::Private, 8);
    let volatile = MemoryAccess {
        volatile: true,
        ..access
    };
    let s = |s| OperationKind::Storage(s);
    let e = |e| OperationKind::Execution(e);
    let pairs = vec![
        (
            s(Storage::Project {
                base: ValueId(1),
                step: Projection::Field(0),
            }),
            s(Storage::Project {
                base: ValueId(1),
                step: Projection::Field(1),
            }),
        ),
        (
            s(Storage::Project {
                base: ValueId(1),
                step: Projection::Variant { index: 0, access },
            }),
            s(Storage::Project {
                base: ValueId(1),
                step: Projection::Variant {
                    index: 0,
                    access: volatile,
                },
            }),
        ),
        (
            s(Storage::Project {
                base: ValueId(1),
                step: Projection::VariantForWrite { index: 0 },
            }),
            s(Storage::Project {
                base: ValueId(1),
                step: Projection::VariantForWrite { index: 1 },
            }),
        ),
        (
            s(Storage::ReadValue {
                address: ValueId(1),
                access,
            }),
            s(Storage::ReadValue {
                address: ValueId(1),
                access: volatile,
            }),
        ),
        (
            s(Storage::WriteValue {
                address: ValueId(1),
                value: ValueId(2),
                access,
            }),
            s(Storage::WriteValue {
                address: ValueId(1),
                value: ValueId(2),
                access: volatile,
            }),
        ),
        (
            s(Storage::CopyObject {
                source: ValueId(1),
                destination: ValueId(2),
                source_access: access,
                destination_access: access,
                overlap: StorageCopyOverlapV1::NonOverlapping,
            }),
            s(Storage::CopyObject {
                source: ValueId(1),
                destination: ValueId(2),
                source_access: volatile,
                destination_access: access,
                overlap: StorageCopyOverlapV1::NonOverlapping,
            }),
        ),
        (
            s(Storage::SetDiscriminant {
                address: ValueId(1),
                variant: 0,
                access,
            }),
            s(Storage::SetDiscriminant {
                address: ValueId(1),
                variant: 1,
                access,
            }),
        ),
        (
            e(Execution::ContextIssue),
            e(Execution::WorkgroupDerive {
                context: ValueId(1),
            }),
        ),
        (
            e(Execution::ScopeEnd {
                workgroup: ValueId(1),
                discarded: vec![],
            }),
            e(Execution::ScopeEnd {
                workgroup: ValueId(1),
                discarded: vec![ValueId(2)],
            }),
        ),
        (
            e(Execution::MaskedTileLoadU32 {
                workgroup: ValueId(1),
                input: ValueId(2),
                base: ValueId(3),
                lanes: 64,
                elements: 1,
            }),
            e(Execution::MaskedTileLoadU32 {
                workgroup: ValueId(1),
                input: ValueId(2),
                base: ValueId(3),
                lanes: 32,
                elements: 1,
            }),
        ),
        (
            e(Execution::TileIntoFragmentU32 {
                tile: ValueId(1),
                lanes: 64,
                elements: 1,
            }),
            e(Execution::TileIntoFragmentU32 {
                tile: ValueId(1),
                lanes: 64,
                elements: 2,
            }),
        ),
        (
            e(Execution::FragmentIntoPartsU32 {
                fragment: ValueId(1),
                lanes: 64,
                elements: 1,
            }),
            e(Execution::FragmentIntoPartsU32 {
                fragment: ValueId(1),
                lanes: 32,
                elements: 1,
            }),
        ),
    ];
    for (a, b) in pairs {
        let mut work = Work::new(2);
        let mut budget = Budget::new(&mut work, 0);
        assert_eq!(payload::operation(&a, &a, &mut budget), Ok(true));
        assert_eq!(payload::operation(&a, &b, &mut budget), Ok(false));
        assert_eq!(budget.work(), 2);
        assert!(!payload::pure(&a) && !payload::pure(&b));
    }
}
