use super::*;

fn field(offset: u64, layout: u32) -> StorageFieldV1 {
    StorageFieldV1 {
        offset,
        layout: StorageLayoutIdV1(layout),
    }
}

#[test]
fn storage_layout_pointer_reference_is_not_by_value_containment() {
    let pointer = StorageLayoutKindV1::Pointer(StoragePointerV1 {
        pointee: StorageLayoutIdV1(0),
        value_space: AddressSpace::Private,
        encoded_space: AddressSpace::Generic,
        access: AccessMode::ReadWrite,
        stored_bits: 64,
    });
    assert_eq!(pointer.containment_count(), Some(0));
    assert_eq!(pointer.containment_child(0), None);
    let StorageLayoutKindV1::Pointer(pointer) = pointer else {
        unreachable!()
    };
    assert_ne!(pointer.value_space, pointer.encoded_space);
    assert_eq!(pointer.stored_bits, 64);
}

#[test]
fn storage_layout_slice_contains_descriptor_fields_not_payload() {
    let row = StorageLayoutKindV1::Slice {
        element: StorageLayoutIdV1(9),
        value_space: AddressSpace::Global,
        access: AccessMode::ReadOnly,
        data: field(0, 2),
        length: field(8, 3),
    };
    assert_eq!(row.containment_count(), Some(2));
    assert_eq!(row.containment_child(0), Some(StorageLayoutIdV1(2)));
    assert_eq!(row.containment_child(1), Some(StorageLayoutIdV1(3)));
    assert_eq!(row.containment_child(2), None);
}

#[test]
fn storage_layout_variants_include_each_storage_view_and_tag() {
    let row = StorageLayoutKindV1::Variants {
        encoding: StorageVariantEncodingV1::Direct { tag: field(0, 3) },
        variants: vec![
            StorageVariantV1 {
                discriminant: 5,
                direct_tag_bits: Some(1),
                uninhabited: false,
                layout: StorageLayoutIdV1(1),
            },
            StorageVariantV1 {
                discriminant: 8,
                direct_tag_bits: Some(2),
                uninhabited: false,
                layout: StorageLayoutIdV1(2),
            },
        ]
        .into_boxed_slice(),
    };
    assert_eq!(row.containment_count(), Some(3));
    for (index, id) in [1, 2, 3].into_iter().enumerate() {
        assert_eq!(row.containment_child(index), Some(StorageLayoutIdV1(id)));
    }
    assert_eq!(row.containment_child(3), None);
}

#[test]
fn storage_layout_packed_placement_never_invents_natural_alignment() {
    assert_eq!(field(0, 0).placement_alignment(16), Some(16));
    assert_eq!(field(8, 0).placement_alignment(16), Some(8));
    assert_eq!(field(3, 0).placement_alignment(16), Some(1));
    assert_eq!(field(16, 0).placement_alignment(1), Some(1));
    assert_eq!(
        field(1_u64 << 63, 0).placement_alignment(1_u32 << 31),
        Some(1_u32 << 31)
    );
    assert_eq!(field(0, 0).placement_alignment(0), None);
    assert_eq!(field(0, 0).placement_alignment(3), None);
}
