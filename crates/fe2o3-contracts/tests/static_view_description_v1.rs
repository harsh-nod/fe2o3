use fe2o3_contracts::{
    AddressSpaceIdV1, AllocationProvenanceIdV1, AllocationSpecV1, ByteRegionV1,
    MAX_STATIC_VIEW_ELEMENTS_V1, StaticViewAccessDescriptionV1, StaticViewDescriptionErrorV1,
    StaticViewDescriptionV1,
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

fn description(
    access: StaticViewAccessDescriptionV1,
    start: u64,
    count: u64,
) -> Result<StaticViewDescriptionV1, StaticViewDescriptionErrorV1> {
    let allocation = allocation(7, 1, 64);
    let parent = ByteRegionV1::for_allocation(allocation, 0, 64).unwrap();
    StaticViewDescriptionV1::describe(allocation, parent, 16, start, count, 4, 4, access)
}

#[test]
fn coherent_description_retains_only_caller_supplied_symbolic_data() {
    let view = description(StaticViewAccessDescriptionV1::ExclusiveWrite, 3, 4).unwrap();
    assert_eq!(view.described_allocation().provenance().get(), 7);
    assert_eq!(view.described_parent_region().byte_length(), 64);
    assert_eq!(view.described_region().byte_offset(), 12);
    assert_eq!(view.described_region().byte_length(), 16);
    assert_eq!(view.parent_element_count(), 16);
    assert_eq!(view.start_element(), 3);
    assert_eq!(view.element_count(), 4);
    assert_eq!(view.element_size(), 4);
    assert_eq!(view.element_alignment(), 4);
    assert_eq!(
        view.access_description(),
        StaticViewAccessDescriptionV1::ExclusiveWrite
    );
}

#[test]
fn coherent_forged_provenance_and_exclusivity_claims_remain_descriptions() {
    let first = allocation(7, 1, 64);
    let forged = allocation(0xfeed, 9, 64);
    let first_parent = ByteRegionV1::for_allocation(first, 0, 64).unwrap();
    let forged_parent = ByteRegionV1::for_allocation(forged, 0, 64).unwrap();

    let first = StaticViewDescriptionV1::describe(
        first,
        first_parent,
        16,
        0,
        4,
        4,
        4,
        StaticViewAccessDescriptionV1::ExclusiveWrite,
    )
    .unwrap();
    let forged = StaticViewDescriptionV1::describe(
        forged,
        forged_parent,
        16,
        0,
        4,
        4,
        4,
        StaticViewAccessDescriptionV1::ExclusiveWrite,
    )
    .unwrap();

    assert_ne!(first, forged);
    assert_eq!(forged.described_allocation().provenance().get(), 0xfeed);
    assert_eq!(forged.described_allocation().address_space().get(), 9);
    assert_eq!(
        forged.access_description(),
        StaticViewAccessDescriptionV1::ExclusiveWrite
    );
    // Both values are coherent because coherence authenticates nothing. The
    // contracts crate intentionally exposes no conversion to runtime authority.
}

#[test]
fn described_element_regions_are_exact_and_bounded() {
    let view = description(StaticViewAccessDescriptionV1::SharedRead, 3, 4).unwrap();
    for index in 0..4 {
        let element = view.described_element_region(index).unwrap();
        assert_eq!(element.provenance(), view.described_region().provenance());
        assert_eq!(
            element.address_space(),
            view.described_region().address_space()
        );
        assert_eq!(element.byte_offset(), 12 + index * 4);
        assert_eq!(element.byte_length(), 4);
        assert!(view.contains_element_index(index));
    }
    assert!(!view.contains_element_index(4));
    assert_eq!(
        view.described_element_region(4),
        Err(StaticViewDescriptionErrorV1::ElementIndexOutsideView { index: 4, count: 4 })
    );
}

#[test]
fn access_descriptions_are_data_not_permissions() {
    let shared = description(StaticViewAccessDescriptionV1::SharedRead, 0, 1).unwrap();
    let exclusive = description(StaticViewAccessDescriptionV1::ExclusiveWrite, 0, 1).unwrap();
    assert_ne!(shared, exclusive);
}

#[test]
fn zero_and_out_of_range_extents_fail_coherence_checks() {
    assert_eq!(
        description(StaticViewAccessDescriptionV1::SharedRead, 0, 0),
        Err(StaticViewDescriptionErrorV1::EmptyView)
    );
    assert_eq!(
        description(StaticViewAccessDescriptionV1::SharedRead, 13, 4),
        Err(StaticViewDescriptionErrorV1::ElementRangeOutsideParent {
            start: 13,
            count: 4,
            parent_count: 16,
        })
    );
    assert_eq!(
        description(StaticViewAccessDescriptionV1::SharedRead, u64::MAX, 2),
        Err(StaticViewDescriptionErrorV1::ElementRangeOverflow)
    );
}

#[test]
fn layout_and_extent_mutations_are_rejected() {
    let allocation = allocation(7, 1, 64);
    let parent = ByteRegionV1::for_allocation(allocation, 0, 64).unwrap();
    let make = |parent_count, size, alignment| {
        StaticViewDescriptionV1::describe(
            allocation,
            parent,
            parent_count,
            0,
            1,
            size,
            alignment,
            StaticViewAccessDescriptionV1::SharedRead,
        )
    };

    assert_eq!(
        make(16, 0, 1),
        Err(StaticViewDescriptionErrorV1::ZeroSizedElement)
    );
    assert_eq!(
        make(16, 4, 3),
        Err(StaticViewDescriptionErrorV1::InvalidElementAlignment { alignment: 3 })
    );
    assert_eq!(
        make(16, 6, 4),
        Err(StaticViewDescriptionErrorV1::ElementLayoutMismatch {
            element_size: 6,
            alignment: 4,
        })
    );
    assert_eq!(
        make(15, 4, 4),
        Err(StaticViewDescriptionErrorV1::ParentExtentMismatch {
            expected: 60,
            actual: 64,
        })
    );
    assert_eq!(
        make(MAX_STATIC_VIEW_ELEMENTS_V1 + 1, 4, 4),
        Err(StaticViewDescriptionErrorV1::ElementCountBoundExceeded {
            actual: MAX_STATIC_VIEW_ELEMENTS_V1 + 1,
            maximum: MAX_STATIC_VIEW_ELEMENTS_V1,
        })
    );
}

#[test]
fn mismatched_symbolic_records_are_rejected_without_authenticating_matches() {
    let first = allocation(7, 1, 64);
    let other_provenance = allocation(8, 1, 64);
    let other_space = allocation(7, 2, 64);
    let parent = ByteRegionV1::for_allocation(first, 0, 64).unwrap();

    for mutated in [other_provenance, other_space] {
        assert_eq!(
            StaticViewDescriptionV1::describe(
                mutated,
                parent,
                16,
                0,
                4,
                4,
                4,
                StaticViewAccessDescriptionV1::SharedRead,
            ),
            Err(StaticViewDescriptionErrorV1::ParentRegionOutsideDescribedAllocation)
        );
    }

    let short_parent = ByteRegionV1::for_allocation(first, 4, 60).unwrap();
    assert_eq!(
        StaticViewDescriptionV1::describe(
            first,
            short_parent,
            16,
            0,
            4,
            4,
            4,
            StaticViewAccessDescriptionV1::SharedRead,
        ),
        Err(StaticViewDescriptionErrorV1::ParentExtentMismatch {
            expected: 64,
            actual: 60,
        })
    );
}

#[test]
fn misaligned_and_overflowing_descriptions_are_rejected() {
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
        StaticViewDescriptionV1::describe(
            misaligned_allocation,
            parent,
            16,
            0,
            4,
            4,
            4,
            StaticViewAccessDescriptionV1::SharedRead,
        ),
        Err(StaticViewDescriptionErrorV1::MisalignedParentRegion {
            address: 0x1_0002,
            alignment: 4,
        })
    );

    let allocation = allocation(7, 1, 64);
    let parent = ByteRegionV1::for_allocation(allocation, 0, 64).unwrap();
    assert_eq!(
        StaticViewDescriptionV1::describe(
            allocation,
            parent,
            MAX_STATIC_VIEW_ELEMENTS_V1,
            0,
            1,
            u64::MAX,
            1,
            StaticViewAccessDescriptionV1::SharedRead,
        ),
        Err(StaticViewDescriptionErrorV1::ParentExtentOverflow)
    );
}
