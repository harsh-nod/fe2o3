fn atomic_slice_fixture() -> DeviceDescriptorTableV1 {
    let original = global_mut_pointer_fixture();
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_atomic_slice_u32());
    let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_atomic_slice_u32());
    let argument =
        LogicalArgumentV1::shared_atomic_slice_u32(0, name("channels"), &source, &layout, 0)
            .unwrap();
    let mut kernel = original.kernels[0].clone();
    kernel.arguments = vec![argument];
    kernel.abi_layout = KernelAbiLayoutV1::new(16, 16, 8).unwrap();
    DeviceDescriptorTableV1::new(
        original.canonical_code_object_digest,
        original.code_object_version,
        original.compiler,
        original.producer,
        original.device_target,
        vec![source],
        vec![layout],
        vec![kernel],
    )
    .unwrap()
}

#[test]
fn atomic_slice_descriptor_round_trip_preserves_distinct_shared_atomic_contract() {
    let table = atomic_slice_fixture();
    let source = table.type_records[0].descriptor();
    assert!(source.is_shared_atomic_slice_u32());
    assert!(!source.is_shared_slice());
    assert!(!source.is_disjoint_slice());
    assert_eq!(source.scalar_type(), ScalarTypeV1::U32);
    let argument = &table.kernels[0].arguments[0];
    assert_eq!(argument.ownership(), OwnershipSemantics::SharedBorrow);
    assert_eq!(argument.access(), AccessMode::ReadWrite);
    assert_eq!(argument.alias(), AliasSemantics::SharedAtomic);
    let bytes = encode_device_descriptor_table_v1(&table).unwrap();
    assert_eq!(decode_device_descriptor_table_v1(&bytes).unwrap(), table);
    assert_eq!(
        encode_device_descriptor_table_v1(&decode_device_descriptor_table_v1(&bytes).unwrap())
            .unwrap(),
        bytes
    );
    assert_ne!(
        table.type_records[0].identity(),
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::U32)).identity()
    );
    assert_ne!(
        table.layout_records[0].identity(),
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::U32))
            .identity()
    );
}

#[test]
fn atomic_slice_descriptor_rejects_wrong_element_unknown_tag_and_alias_mutants() {
    let table = atomic_slice_fixture();
    let bytes = encode_device_descriptor_table_v1(&table).unwrap();
    let type_offset = find(&bytes, table.type_records[0].identity().as_bytes()) + 32;
    for tag in [0, 6, 255] {
        let mut mutant = bytes.clone();
        mutant[type_offset] = tag;
        assert!(matches!(
            decode_error(&mutant),
            DecodeError::UnknownTag {
                kind: "type descriptor",
                ..
            }
        ));
    }
    for scalar in [ScalarTypeV1::I32, ScalarTypeV1::U64, ScalarTypeV1::F32] {
        let mut mutant = bytes.clone();
        mutant[type_offset + 1] = crate::encode::scalar_tag(scalar);
        assert!(matches!(decode_error(&mutant), DecodeError::Validation(_)));
    }
    let mut prefix = Vec::from(table.type_records[0].identity().as_bytes().as_slice());
    prefix.extend_from_slice(table.layout_records[0].identity().as_bytes());
    prefix.extend_from_slice(&[2, 4, 4, 0]);
    let alias = find(&bytes, &prefix) + 66;
    for tag in [1, 2, 3, 5, 255] {
        let mut mutant = bytes.clone();
        mutant[alias] = tag;
        assert!(decode_device_descriptor_table_v1(&mutant).is_err());
    }
    let mut mutant = bytes;
    mutant[type_offset] = 2;
    assert!(decode_device_descriptor_table_v1(&mutant).is_err());
}

#[test]
fn atomic_slice_descriptor_rejects_ordinary_record_and_component_substitution() {
    let table = atomic_slice_fixture();
    let source = &table.type_records[0];
    let layout = &table.layout_records[0];
    assert!(LogicalArgumentV1::shared_slice(0, name("bad"), source, layout, 0).is_err());
    assert!(
        LogicalArgumentV1::disjoint_slice(0, name("bad"), source, layout, AccessMode::ReadWrite, 0)
            .is_err()
    );
    let ordinary_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::U32));
    let ordinary_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::U32));
    assert!(
        LogicalArgumentV1::shared_atomic_slice_u32(
            0,
            name("bad"),
            &ordinary_source,
            &ordinary_layout,
            0
        )
        .is_err()
    );
    assert!(
        LogicalArgumentV1::shared_atomic_slice_u32(0, name("bad"), source, &ordinary_layout, 0)
            .is_err()
    );
    assert!(LogicalArgumentV1::shared_atomic_slice_u32(0, name("bad"), source, layout, 2).is_err());
    assert!(
        LogicalArgumentV1::shared_atomic_slice_u32(0, name("bad"), source, layout, u32::MAX - 7)
            .is_err()
    );
    for mutation in 0..4 {
        let mut kernel = table.kernels[0].clone();
        let arg = &mut kernel.arguments[0];
        match mutation {
            0 => arg.components[0].access = AccessMode::ReadOnly,
            1 => arg.components[0].alias = AliasSemantics::Exclusive,
            2 => arg.components[1].offset += 8,
            _ => arg.ownership = OwnershipSemantics::UniqueBorrow,
        }
        assert!(
            DeviceDescriptorTableV1::new(
                table.canonical_code_object_digest,
                table.code_object_version,
                table.compiler.clone(),
                table.producer.clone(),
                table.device_target,
                table.type_records.clone(),
                table.layout_records.clone(),
                vec![kernel],
            )
            .is_err()
        );
    }
}
