use fe2o3_contracts::{
    AddressSpaceIdV1, AllocationProvenanceIdV1, AllocationSpecV1, ByteRegionV1,
    MAX_STATIC_VIEW_ELEMENTS_V1, PermissionKindV1, SpecificationFactV1, StaticViewContractErrorV1,
    StaticViewContractV1,
};

fn allocation(provenance: u32, address_space: u16, bytes: u64) -> AllocationSpecV1 {
    AllocationSpecV1::new(
        AllocationProvenanceIdV1::new(provenance).unwrap(),
        AddressSpaceIdV1::new(address_space).unwrap(),
        0x1_0000,
        bytes,
        0x2_0000,
    )
    .unwrap()
}

fn contract(
    permission: PermissionKindV1,
    start: u64,
    count: u64,
) -> Result<StaticViewContractV1, StaticViewContractErrorV1> {
    let allocation = allocation(7, 1, 64);
    let parent = ByteRegionV1::for_allocation(allocation, 0, 64).unwrap();
    StaticViewContractV1::new(allocation, parent, 16, start, count, 4, 4, permission)
}

#[test]
fn valid_contract_retains_parent_provenance_and_derives_exact_region() {
    let view = contract(PermissionKindV1::ExclusiveWrite, 3, 4).unwrap();
    assert_eq!(view.allocation().provenance().get(), 7);
    assert_eq!(view.parent_region().byte_length(), 64);
    assert_eq!(view.region().byte_offset(), 12);
    assert_eq!(view.region().byte_length(), 16);
    assert_eq!(view.parent_element_count(), 16);
    assert_eq!(view.start_element(), 3);
    assert_eq!(view.element_count(), 4);
    assert_eq!(view.element_size(), 4);
    assert_eq!(view.element_alignment(), 4);
    assert_eq!(view.permission(), PermissionKindV1::ExclusiveWrite);
}

#[test]
fn element_regions_are_exact_and_bounded() {
    let view = contract(PermissionKindV1::SharedRead, 3, 4).unwrap();
    for index in 0..4 {
        let element = view.element_region(index).unwrap();
        assert_eq!(element.provenance(), view.region().provenance());
        assert_eq!(element.address_space(), view.region().address_space());
        assert_eq!(element.byte_offset(), 12 + index * 4);
        assert_eq!(element.byte_length(), 4);
        assert!(view.contains_element_index(index));
    }
    assert!(!view.contains_element_index(4));
    assert_eq!(
        view.element_region(4),
        Err(StaticViewContractErrorV1::ElementIndexOutsideView { index: 4, count: 4 })
    );
}

#[test]
fn contract_is_pure_specification_data() {
    fn is_specification<T: SpecificationFactV1>() {}
    is_specification::<StaticViewContractV1>();
    let shared = contract(PermissionKindV1::SharedRead, 0, 1).unwrap();
    let exclusive = contract(PermissionKindV1::ExclusiveWrite, 0, 1).unwrap();
    assert_ne!(shared, exclusive);
}

#[test]
fn zero_and_out_of_range_extents_fail_closed() {
    assert_eq!(
        contract(PermissionKindV1::SharedRead, 0, 0),
        Err(StaticViewContractErrorV1::EmptyView)
    );
    assert_eq!(
        contract(PermissionKindV1::SharedRead, 13, 4),
        Err(StaticViewContractErrorV1::ElementRangeOutsideParent {
            start: 13,
            count: 4,
            parent_count: 16,
        })
    );
    assert_eq!(
        contract(PermissionKindV1::SharedRead, u64::MAX, 2),
        Err(StaticViewContractErrorV1::ElementRangeOverflow)
    );
}

#[test]
fn layout_and_extent_mutations_are_rejected() {
    let allocation = allocation(7, 1, 64);
    let parent = ByteRegionV1::for_allocation(allocation, 0, 64).unwrap();
    let make = |parent_count, size, alignment| {
        StaticViewContractV1::new(
            allocation,
            parent,
            parent_count,
            0,
            1,
            size,
            alignment,
            PermissionKindV1::SharedRead,
        )
    };

    assert_eq!(
        make(16, 0, 1),
        Err(StaticViewContractErrorV1::ZeroSizedElement)
    );
    assert_eq!(
        make(16, 4, 3),
        Err(StaticViewContractErrorV1::InvalidElementAlignment { alignment: 3 })
    );
    assert_eq!(
        make(16, 6, 4),
        Err(StaticViewContractErrorV1::ElementLayoutMismatch {
            element_size: 6,
            alignment: 4,
        })
    );
    assert_eq!(
        make(15, 4, 4),
        Err(StaticViewContractErrorV1::ParentExtentMismatch {
            expected: 60,
            actual: 64,
        })
    );
    assert_eq!(
        make(MAX_STATIC_VIEW_ELEMENTS_V1 + 1, 4, 4),
        Err(StaticViewContractErrorV1::ElementCountBoundExceeded {
            actual: MAX_STATIC_VIEW_ELEMENTS_V1 + 1,
            maximum: MAX_STATIC_VIEW_ELEMENTS_V1,
        })
    );
}

#[test]
fn provenance_address_space_and_parent_region_mutations_are_rejected() {
    let first = allocation(7, 1, 64);
    let second_provenance = allocation(8, 1, 64);
    let second_space = allocation(7, 2, 64);
    let parent = ByteRegionV1::for_allocation(first, 0, 64).unwrap();

    for mutated in [second_provenance, second_space] {
        assert_eq!(
            StaticViewContractV1::new(
                mutated,
                parent,
                16,
                0,
                4,
                4,
                4,
                PermissionKindV1::SharedRead,
            ),
            Err(StaticViewContractErrorV1::ParentRegionOutsideAllocation)
        );
    }

    let short_parent = ByteRegionV1::for_allocation(first, 4, 60).unwrap();
    assert_eq!(
        StaticViewContractV1::new(
            first,
            short_parent,
            16,
            0,
            4,
            4,
            4,
            PermissionKindV1::SharedRead,
        ),
        Err(StaticViewContractErrorV1::ParentExtentMismatch {
            expected: 64,
            actual: 60,
        })
    );
}

#[test]
fn misaligned_and_overflowing_parent_layouts_are_rejected() {
    let misaligned_allocation = AllocationSpecV1::new(
        AllocationProvenanceIdV1::new(1).unwrap(),
        AddressSpaceIdV1::new(1).unwrap(),
        0x1_0002,
        64,
        0x2_0000,
    )
    .unwrap();
    let parent = ByteRegionV1::for_allocation(misaligned_allocation, 0, 64).unwrap();
    assert_eq!(
        StaticViewContractV1::new(
            misaligned_allocation,
            parent,
            16,
            0,
            4,
            4,
            4,
            PermissionKindV1::SharedRead,
        ),
        Err(StaticViewContractErrorV1::MisalignedParentRegion {
            address: 0x1_0002,
            alignment: 4,
        })
    );

    let allocation = allocation(7, 1, 64);
    let parent = ByteRegionV1::for_allocation(allocation, 0, 64).unwrap();
    assert_eq!(
        StaticViewContractV1::new(
            allocation,
            parent,
            MAX_STATIC_VIEW_ELEMENTS_V1,
            0,
            1,
            u64::MAX,
            1,
            PermissionKindV1::SharedRead,
        ),
        Err(StaticViewContractErrorV1::ParentExtentOverflow)
    );
}
