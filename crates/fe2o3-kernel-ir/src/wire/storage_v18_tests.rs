use super::*;
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, StorageCopyOverlapV1, StorageFieldV1,
    StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind, StorageLayoutV1 as Row,
    StorageOperationV1 as Op, StoragePointerV1, StorageProjectionV1 as Step,
    StorageVariantEncodingV1 as Encoding, StorageVariantV1,
};
use std::mem::size_of;

fn field(offset: u64, layout: u32) -> StorageFieldV1 {
    StorageFieldV1 {
        offset,
        layout: Id(layout),
    }
}
pub(crate) fn rows() -> Vec<Row> {
    vec![
        Row {
            size: 8,
            alignment: 8,
            kind: Kind::Scalar(ScalarType::U64),
        },
        Row {
            size: 16,
            alignment: 16,
            kind: Kind::Vector(FixedVectorTypeV12::new(
                ScalarType::U32,
                4,
                VectorLayoutV12::Contiguous,
            )),
        },
        Row {
            size: 8,
            alignment: 8,
            kind: Kind::Pointer(StoragePointerV1 {
                pointee: Id(3),
                value_space: AddressSpace::Private,
                encoded_space: AddressSpace::Generic,
                access: AccessMode::ReadWrite,
                stored_bits: 64,
            }),
        },
        Row {
            size: 24,
            alignment: 8,
            kind: Kind::Record(vec![field(0, 2), field(16, 0)].into_boxed_slice()),
        },
        Row {
            size: 16,
            alignment: 16,
            kind: Kind::Union(vec![field(0, 0), field(0, 1)].into_boxed_slice()),
        },
        Row {
            size: 24,
            alignment: 8,
            kind: Kind::Array {
                element: Id(0),
                length: 3,
                stride: 8,
            },
        },
        Row {
            size: 16,
            alignment: 8,
            kind: Kind::Slice {
                element: Id(3),
                value_space: AddressSpace::Private,
                access: AccessMode::ReadWrite,
                data: field(0, 2),
                length: field(8, 9),
            },
        },
        Row {
            size: 32,
            alignment: 8,
            kind: Kind::Variants {
                encoding: Encoding::Direct { tag: field(24, 0) },
                variants: vec![
                    StorageVariantV1 {
                        discriminant: 91,
                        direct_tag_bits: Some(3),
                        uninhabited: false,
                        layout: Id(3),
                    },
                    StorageVariantV1 {
                        discriminant: u128::MAX,
                        direct_tag_bits: Some(9),
                        uninhabited: false,
                        layout: Id(5),
                    },
                ]
                .into_boxed_slice(),
            },
        },
        Row {
            size: 24,
            alignment: 8,
            kind: Kind::Variants {
                encoding: Encoding::Niche {
                    tag: field(0, 0),
                    untagged_variant: 0,
                    first_niche_variant: 1,
                    last_niche_variant: 1,
                    niche_start: u64::MAX as u128,
                },
                variants: vec![
                    StorageVariantV1 {
                        discriminant: 8,
                        direct_tag_bits: None,
                        uninhabited: false,
                        layout: Id(3),
                    },
                    StorageVariantV1 {
                        discriminant: 9,
                        direct_tag_bits: None,
                        uninhabited: false,
                        layout: Id(5),
                    },
                    StorageVariantV1 {
                        discriminant: 10,
                        direct_tag_bits: None,
                        uninhabited: true,
                        layout: Id(0),
                    },
                ]
                .into_boxed_slice(),
            },
        },
        Row {
            size: 8,
            alignment: 8,
            kind: Kind::Scalar(ScalarType::Index),
        },
    ]
}
fn roundtrip(module: &Module) -> Vec<u8> {
    let bytes = encode_module(module, KERNEL_IR_VERSION_V18).unwrap();
    assert_eq!(
        decode_module(&bytes, KERNEL_IR_VERSION_V18, false).unwrap(),
        *module
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let extent =
        count_module_with_work_v1(module, KERNEL_IR_VERSION_V18, &mut work, false).unwrap();
    assert_eq!(extent.wire_bytes(), bytes.len());
    assert!(
        compare_module_encoding_v1(module, KERNEL_IR_VERSION_V18, &bytes, Some(&mut work)).unwrap()
    );
    bytes
}

#[test]
fn storage_wire_every_row_preserves_order_padding_overlap_and_pointer_cycle() {
    let mut module = Module::new("rows");
    module.storage_layouts = rows();
    roundtrip(&module);
    module.storage_layouts[3].kind =
        Kind::Record(vec![field(16, 0), field(0, 2)].into_boxed_slice());
    let reversed = roundtrip(&module);
    module.storage_layouts = rows();
    assert_ne!(reversed, roundtrip(&module));
}

#[test]
fn storage_wire_explicit_roles_never_infer_from_body_or_kernel_roster() {
    let mut module = Module::new("roles");
    for (index, role) in [
        FunctionRole::KernelEntry,
        FunctionRole::InternalHelper,
        FunctionRole::DeviceFfiExport,
        FunctionRole::ExternalImport,
    ]
    .into_iter()
    .enumerate()
    {
        // Wire transport preserves even semantically inconsistent combinations;
        // the owning structural admission separately rejects them.
        let mut function = Function::definition(
            format!("f{index}"),
            Signature::new(vec![], vec![]),
            vec![],
            vec![],
        );
        function.role = role;
        module.functions.push(function);
    }
    let bytes = roundtrip(&module);
    for version in [1, 12, 15, 16, 17] {
        assert!(encode_module(&module, version).is_err());
        assert!(matches!(
            decode_module(&bytes, version, true),
            Err(KernelIrDecodeError::UnknownVersion(18))
        ));
    }
}

#[test]
fn storage_wire_all_operations_and_access_facts_roundtrip() {
    let mut access = MemoryAccess::new(AddressSpace::Private, 8);
    access.volatile = true;
    let operations = [
        Op::Project {
            base: ValueId(2),
            step: Step::Field(9),
        },
        Op::Project {
            base: ValueId(3),
            step: Step::ArrayIndex(ValueId(4)),
        },
        Op::Project {
            base: ValueId(5),
            step: Step::Variant { index: 6, access },
        },
        Op::ReadValue {
            address: ValueId(7),
            access,
        },
        Op::WriteValue {
            address: ValueId(8),
            value: ValueId(9),
            access,
        },
        Op::CopyObject {
            source: ValueId(10),
            destination: ValueId(11),
            source_access: access,
            destination_access: access,
            overlap: StorageCopyOverlapV1::NonOverlapping,
        },
        Op::CopyObject {
            source: ValueId(12),
            destination: ValueId(13),
            source_access: access,
            destination_access: access,
            overlap: StorageCopyOverlapV1::MayOverlap,
        },
    ];
    for operation in operations {
        let kind = OperationKind::Storage(operation);
        let mut writer = Writer::new(18, None);
        encode_operation_kind(&mut writer, &kind).unwrap();
        let mut reader = Reader::new(&writer.bytes, None);
        reader.version = 18;
        assert_eq!(decode_operation_kind(&mut reader).unwrap(), kind);
        assert!(reader.is_finished());
        for version in [1, 12, 15, 16, 17] {
            let mut old = Writer::new(version, None);
            assert!(matches!(
                encode_operation_kind(&mut old, &kind),
                Err(KernelIrEncodeError::UnsupportedInVersion { .. })
            ));
            assert!(old.bytes.is_empty());
        }
    }
}

#[test]
fn storage_wire_nested_type_id_is_lossless_but_never_a_standalone_authority() {
    let ty = Type::pointer(
        Type::StorageObject(Id(u32::MAX)),
        AddressSpace::Generic,
        AccessMode::ReadOnly,
    );
    let mut writer = Writer::new(18, None);
    encode_type(&mut writer, &ty, 0).unwrap();
    assert_eq!(writer.bytes, [3, 5, 1, 13, 255, 255, 255, 255]);
    let mut reader = Reader::new(&writer.bytes, None);
    reader.version = 18;
    assert_eq!(decode_type(&mut reader, 0).unwrap(), ty);
    for version in [1, 12, 15, 16, 17] {
        assert!(matches!(
            encode_type(&mut Writer::new(version, None), &ty, 0),
            Err(KernelIrEncodeError::UnsupportedInVersion {
                feature: "module-owned storage type",
                ..
            })
        ));
    }
}

#[test]
fn storage_wire_ordinary_empty_legacy_bytes_and_new_row_section_are_literal() {
    let module = Module::new("m");
    for version in [1_u16, 12, 15, 16, 17] {
        let mut expected = b"FE2O3KI\0".to_vec();
        expected.extend_from_slice(&version.to_le_bytes());
        expected.extend_from_slice(&[
            0, 0, 37, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, b'm', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        assert_eq!(encode_module(&module, version).unwrap(), expected);
    }
    let bytes = roundtrip(&module);
    assert_eq!(bytes.len(), 41);
    assert_eq!(&bytes[8..10], &18_u16.to_le_bytes());
    assert_eq!(&bytes[37..], &[0; 4]);
}

#[test]
fn storage_wire_all_truncations_counts_tags_and_options_refuse_before_authority() {
    let mut module = Module::new("m");
    module.storage_layouts = rows();
    let bytes = roundtrip(&module);
    for end in 0..bytes.len() {
        assert!(
            decode_module(&bytes[..end], 18, false).is_err(),
            "prefix {end}"
        );
    }
    for (offset, value) in [(10, 1), (16, 1), (53, 255)] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert!(decode_module(&bad, 18, false).is_err(), "mutation {offset}");
    }
    let mut bad = bytes.clone();
    bad[37..41].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        decode_module(&bad, 18, false),
        Err(KernelIrDecodeError::LimitExceeded {
            field: "storage rows",
            ..
        })
    ));
    for bytes in [
        vec![40, 255],
        vec![40, 1, 0, 0, 0, 0, 255],
        vec![40, 4, 0, 0, 0, 0, 0, 0, 0, 0],
    ] {
        let mut reader = Reader::new(&bytes, None);
        reader.version = 18;
        assert!(decode_operation_kind(&mut reader).is_err());
    }
}

#[test]
fn storage_wire_capability_sets_reject_duplicates_and_noncanonical_order() {
    let mut module = Module::new("m");
    module.required_capabilities =
        BTreeSet::from([TargetCapability::Float16, TargetCapability::BFloat16]);
    let bytes = roundtrip(&module);
    // The module capability count starts at byte33, followed by two tag bytes.
    assert_eq!(&bytes[33..39], &[2, 0, 0, 0, 1, 2]);
    for tags in [[1, 1], [2, 1]] {
        let mut bad = bytes.clone();
        bad[37..39].copy_from_slice(&tags);
        assert!(matches!(
            decode_module(&bad, 18, false),
            Err(KernelIrDecodeError::NonCanonical)
        ));
    }
}

#[test]
fn storage_wire_scalar_row_exact_work_and_preallocation_boundary_are_source_derived() {
    // Count: count token + row visit + size/alignment/kind/scalar tokens = 6.
    // Emit/read: count4 + row visit1 + size8 + alignment4 + kind1 + scalar1 = 19.
    // Reader additionally pays one remaining-byte count check, for 20.
    let rows = vec![Row {
        size: 8,
        alignment: 8,
        kind: Kind::Scalar(ScalarType::U64),
    }];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(6);
    let mut counter = Writer::counter(18, &mut work);
    storage_layout_v18::encode(&mut counter, &rows).unwrap();
    assert_eq!(counter.length(), 18);
    assert_eq!(work.work(), 6);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(19);
    let mut writer = Writer::new(18, Some(&mut work));
    storage_layout_v18::encode(&mut writer, &rows).unwrap();
    let bytes = writer.bytes;
    assert_eq!(work.work(), 19);
    for (limit, success) in [(size_of::<Row>(), true), (size_of::<Row>() - 1, false)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(20);
        let mut budget = Budget::new(&mut work, 7 + limit);
        budget.reserve_storage(7).unwrap();
        let result = {
            let mut reader = Reader::new(&bytes, Some(DecodeBudgetV12::Resources(&mut budget)));
            reader.version = 18;
            storage_layout_v18::decode(&mut reader)
        };
        assert_eq!(result.is_ok(), success);
        if success {
            assert_eq!(result.unwrap(), rows);
            assert_eq!(budget.work(), 20);
            assert_eq!(budget.storage(), 7 + size_of::<Row>());
        } else {
            assert_eq!(budget.work(), 5);
            assert_eq!(budget.storage(), 7);
            assert_eq!(budget.failed_storage(), Some(7 + size_of::<Row>()));
        }
    }
}

#[test]
fn storage_wire_scalar_row_one_short_work_keeps_the_real_prefix() {
    let rows = vec![Row {
        size: 8,
        alignment: 8,
        kind: Kind::Scalar(ScalarType::U64),
    }];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(18);
    let result = storage_layout_v18::encode(&mut Writer::new(18, Some(&mut work)), &rows);
    assert!(matches!(result, Err(KernelIrEncodeError::WorkLimit(_))));
    assert_eq!(work.work(), 18);
    assert_eq!(work.failed_work(), Some(19));
}

#[test]
fn storage_wire_profile_composes_execution_and_both_ordered_payloads_without_widening_old_profiles()
{
    use crate::{
        Gfx942OrderedProgramRegistersV1, Gfx942OrderedProgramV1, Gfx942OrderedRegionRegistersV1,
        Gfx942OrderedRegionV1, Gfx942U32ProgramV1,
    };
    let mut module = crate::verification_execution_lifecycle_v15::tests::fixture(1);
    module.storage_layouts = rows();
    let source = AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]);
    let region = Gfx942OrderedRegionV1::new(
        source,
        Gfx942OrderedRegionRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
        [ValueId(0), ValueId(1), ValueId(2)],
    )
    .unwrap();
    let mut descriptors = [0; 16];
    descriptors[..3].copy_from_slice(&[0x85, 0x133, 0x19d]);
    let program = Gfx942OrderedProgramV1::new(
        source,
        Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
        [ValueId(0), ValueId(1), ValueId(2)],
        Gfx942U32ProgramV1::from_descriptors(3, descriptors).unwrap(),
    )
    .unwrap();
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.push(Operation::new(
        vec![],
        OperationKind::Gfx942OrderedRegion(region),
    ));
    operations.push(Operation::new(
        vec![],
        OperationKind::Gfx942OrderedProgram(program),
    ));
    roundtrip(&module);
    for version in [12, 15, 16, 17] {
        assert!(encode_module(&module, version).is_err());
    }
    module.storage_layouts.clear();
    for version in [12, 15, 16, 17] {
        assert!(encode_module(&module, version).is_err());
    }
}

// Pinned-host size premises only, not quota totals or portable Rust ABI.
pub(crate) fn header_layout_premises() -> [usize; 3] {
    [
        size_of::<Writer<'_>>(),
        size_of::<Reader<'_, '_>>(),
        size_of::<Result<Writer<'_>, KernelIrEncodeError>>(),
    ]
}

#[test]
fn storage_wire_type_depth_includes_the_first_rejecting_callee() {
    for depth in [MAX_TYPE_DEPTH_V1, MAX_TYPE_DEPTH_V1 + 1] {
        let mut ty = Type::Scalar(ScalarType::U64);
        let mut bytes = Vec::new();
        for _ in 0..depth {
            ty = Type::pointer(ty, AddressSpace::Private, AccessMode::ReadWrite);
            bytes.extend_from_slice(&[3, 1, 2]);
        }
        bytes.extend_from_slice(&[2, 9]);
        let mut writer = Writer::new(18, None);
        let encoded = encode_type(&mut writer, &ty, 0);
        let mut reader = Reader::new(&bytes, None);
        reader.version = 18;
        let decoded = decode_type(&mut reader, 0);
        if depth == MAX_TYPE_DEPTH_V1 {
            encoded.unwrap();
            assert_eq!(writer.bytes, bytes);
            assert_eq!(decoded.unwrap(), ty);
        } else {
            assert!(matches!(
                encoded,
                Err(KernelIrEncodeError::TypeNestingTooDeep { .. })
            ));
            assert!(matches!(
                decoded,
                Err(KernelIrDecodeError::TypeNestingTooDeep { .. })
            ));
        }
    }
}

#[test]
fn storage_wire_variant_options_booleans_and_explicit_role_tags_are_closed() {
    let mut module = Module::new("m");
    module.storage_layouts.push(Row {
        size: 8,
        alignment: 8,
        kind: Kind::Variants {
            encoding: Encoding::Direct { tag: field(0, 0) },
            variants: vec![StorageVariantV1 {
                discriminant: 1,
                direct_tag_bits: Some(5),
                uninhabited: false,
                layout: Id(0),
            }]
            .into_boxed_slice(),
        },
    });
    // Wire-only hostile fixture: self containment is intentionally not structural
    // authority. Offsets follow the stated module/row/encoding field widths.
    let bytes = roundtrip(&module);
    for (offset, kind) in [
        (41 + 13 + 13 + 4 + 16, "storage direct tag"),
        (
            41 + 13 + 13 + 4 + 16 + 1 + 16,
            "storage uninhabited variant",
        ),
    ] {
        let mut bad = bytes.clone();
        bad[offset] = 2;
        assert!(matches!(decode_module(&bad,18,false),
            Err(KernelIrDecodeError::UnknownTag {kind: actual,tag:2}) if actual == kind));
    }
    module.storage_layouts.clear();
    module.functions.push(Function::external_import(
        "f",
        Signature::new(vec![], vec![]),
    ));
    let mut bytes = roundtrip(&module);
    bytes[41 + 4 + 1] = 0;
    assert!(matches!(
        decode_module(&bytes, 18, false),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "V18 function role",
            tag: 0
        })
    ));
}

#[test]
fn storage_wire_enum_construction_uses_new_literal_tags_and_exact_primitive_work() {
    let cases = [
        (
            Op::Project {
                base: ValueId(2),
                step: Step::VariantForWrite { index: 3 },
            },
            vec![40, 1, 2, 0, 0, 0, 4, 3, 0, 0, 0],
            5,
            7,
        ),
        (
            Op::SetDiscriminant {
                address: ValueId(2),
                variant: 3,
                access: MemoryAccess::new(AddressSpace::Private, 8),
            },
            vec![40, 5, 2, 0, 0, 0, 3, 0, 0, 0, 1, 8, 0, 0, 0, 0],
            7,
            15,
        ),
        (
            Op::ReadDiscriminant {
                address: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            },
            vec![40, 6, 2, 0, 0, 0, 1, 8, 0, 0, 0, 0],
            6,
            11,
        ),
    ];
    for (operation, literal, count_work, short_prefix) in cases {
        let kind = OperationKind::Storage(operation);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(count_work);
        {
            let mut writer = Writer::counter(18, &mut work);
            encode_operation_kind(&mut writer, &kind).unwrap();
            assert_eq!(writer.length(), literal.len());
        }
        assert_eq!(work.work(), count_work);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(literal.len());
        let bytes = {
            let mut writer = Writer::new(18, Some(&mut work));
            encode_operation_kind(&mut writer, &kind).unwrap();
            writer.bytes
        };
        assert_eq!(bytes, literal);
        assert_eq!(work.work(), literal.len());
        let mut work = CanonicalKernelIrWorkBudgetV1::new(literal.len() - 1);
        assert!(matches!(
            encode_operation_kind(&mut Writer::new(18, Some(&mut work)), &kind),
            Err(KernelIrEncodeError::WorkLimit(_))
        ));
        assert_eq!(work.work(), short_prefix);
        assert_eq!(work.failed_work(), Some(literal.len()));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(literal.len());
        {
            let mut reader = Reader::new(&literal, Some(DecodeBudgetV12::Work(&mut work)));
            reader.version = 18;
            assert_eq!(decode_operation_kind(&mut reader).unwrap(), kind);
            assert!(reader.is_finished());
        }
        assert_eq!(work.work(), literal.len());
        for version in [1, 12, 15, 16, 17] {
            let mut old = Writer::new(version, None);
            assert!(matches!(
                encode_operation_kind(&mut old, &kind),
                Err(KernelIrEncodeError::UnsupportedInVersion { .. })
            ));
            assert!(old.bytes.is_empty());
            let mut reader = Reader::new(&literal, None);
            reader.version = version;
            assert!(matches!(
                decode_operation_kind(&mut reader),
                Err(KernelIrDecodeError::UnknownTag { tag: 40, .. })
            ));
        }
        for end in 0..literal.len() {
            let mut reader = Reader::new(&literal[..end], None);
            reader.version = 18;
            assert!(decode_operation_kind(&mut reader).is_err());
        }
    }
}

#[test]
fn storage_wire_enum_construction_preserves_untrusted_access_until_shared_admission() {
    for variant in [0, u32::MAX] {
        for volatile in [false, true] {
            let kind = OperationKind::Storage(Op::SetDiscriminant {
                address: ValueId(u32::MAX),
                variant,
                access: MemoryAccess {
                    address_space: AddressSpace::Constant,
                    alignment: 3,
                    volatile,
                },
            });
            let mut writer = Writer::new(18, None);
            encode_operation_kind(&mut writer, &kind).unwrap();
            let mut reader = Reader::new(&writer.bytes, None);
            reader.version = 18;
            assert_eq!(decode_operation_kind(&mut reader).unwrap(), kind);
        }
    }
    let malformed = [
        vec![40, 1, 0, 0, 0, 0, 5],
        vec![40, 7],
        vec![40, 5, 2, 0, 0, 0, 3, 0, 0, 0, 1, 8, 0, 0, 0, 2],
    ];
    for bytes in malformed {
        let mut reader = Reader::new(&bytes, None);
        reader.version = 18;
        assert!(matches!(
            decode_operation_kind(&mut reader),
            Err(KernelIrDecodeError::UnknownTag { .. })
        ));
    }
}

#[test]
fn storage_wire_discriminant_read_preserves_untrusted_facts_until_admission() {
    for address_space in [
        AddressSpace::Private,
        AddressSpace::Global,
        AddressSpace::Workgroup,
        AddressSpace::Constant,
        AddressSpace::Generic,
    ] {
        for volatile in [false, true] {
            let kind = OperationKind::Storage(Op::ReadDiscriminant {
                address: ValueId(u32::MAX),
                access: MemoryAccess {
                    address_space,
                    alignment: 3,
                    volatile,
                },
            });
            let mut writer = Writer::new(18, None);
            encode_operation_kind(&mut writer, &kind).unwrap();
            let mut reader = Reader::new(&writer.bytes, None);
            reader.version = 18;
            assert_eq!(decode_operation_kind(&mut reader).unwrap(), kind);
            assert!(reader.is_finished());
        }
    }
    for (offset, value) in [(6, 255), (11, 2)] {
        let mut bytes = vec![40, 6, 2, 0, 0, 0, 1, 8, 0, 0, 0, 0];
        bytes[offset] = value;
        let mut reader = Reader::new(&bytes, None);
        reader.version = 18;
        assert!(matches!(
            decode_operation_kind(&mut reader),
            Err(KernelIrDecodeError::UnknownTag { .. })
        ));
    }
}
