use fe2o3_device::{DisjointSlice, StaticIndex, StaticViewError};

#[test]
fn checked_tile_preserves_the_exact_parent_region_and_mutates_only_its_extent() {
    let mut storage = [10_u32, 20, 30, 40, 50, 60];
    // SAFETY: this test owns the complete array and does not create aliases
    // while the device representation is live.
    let mut parent =
        unsafe { DisjointSlice::<u32>::from_raw_parts(storage.as_mut_ptr(), storage.len()) };

    {
        let mut tile = parent.checked_static_tile_mut::<3>(2).unwrap();
        let witness = tile.region_witness();
        assert_eq!(witness.start_element(), 2);
        assert_eq!(witness.parent_region_len(), 6);
        assert_eq!(witness.tile_len(), 3);
        assert_eq!(*tile.at_const(StaticIndex::<3, 0>::CHECKED), 30);

        *tile.at_const_mut(StaticIndex::<3, 2>::CHECKED) = 55;
        tile.as_mut_array()[1] = 44;
        assert_eq!(tile.as_array(), &[30, 44, 55]);
    }

    assert_eq!(storage, [10, 20, 30, 44, 55, 60]);
}

#[test]
fn checked_tile_rejects_empty_overflowing_out_of_range_and_zst_extents() {
    let mut storage = [1_u32, 2, 3, 4];
    // SAFETY: this test exclusively owns `storage`.
    let mut parent =
        unsafe { DisjointSlice::<u32>::from_raw_parts(storage.as_mut_ptr(), storage.len()) };

    assert_eq!(
        parent.checked_static_tile_mut::<0>(0).unwrap_err(),
        StaticViewError::EmptyView
    );
    assert_eq!(
        parent.checked_static_tile_mut::<3>(2).unwrap_err(),
        StaticViewError::ElementRangeOutsideParent {
            start: 2,
            count: 3,
            parent_count: 4,
        }
    );
    assert_eq!(
        parent.checked_static_tile_mut::<2>(usize::MAX).unwrap_err(),
        StaticViewError::ElementRangeOverflow
    );

    let mut units = [(); 1];
    // SAFETY: the representation is valid, but checked static tiles reject a
    // zero-sized element address model.
    let mut zero_sized =
        unsafe { DisjointSlice::<()>::from_raw_parts(units.as_mut_ptr(), units.len()) };
    assert_eq!(
        zero_sized.checked_static_tile_mut::<1>(0).unwrap_err(),
        StaticViewError::ZeroSizedElement
    );
}
