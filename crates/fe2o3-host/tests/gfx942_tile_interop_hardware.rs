use fe2o3_core::GpuContext;
use fe2o3_host::{
    GFX942_XOR4_BF16_TILE_ELEMENTS_V1, Gfx942TileInteropErrorV1, Gfx942Xor4Bf16TileAllocationV1,
};

#[test]
#[ignore = "requires a gfx942:xnack- HIP device"]
fn xor4_tile_round_trips_and_rejects_stream_substitution() {
    let context = GpuContext::new(0).unwrap();
    let stream = context.create_stream().unwrap();
    let other_stream = context.create_stream().unwrap();
    let mut logical = [[0_u16; 16]; 16];
    for (row, values) in logical.iter_mut().enumerate() {
        for (column, value) in values.iter_mut().enumerate() {
            *value = ((row as u16) << 8) | column as u16;
        }
    }

    let mut tile = Gfx942Xor4Bf16TileAllocationV1::from_logical_bits(&stream, &logical).unwrap();
    assert_eq!(tile.len(), GFX942_XOR4_BF16_TILE_ELEMENTS_V1);
    assert_eq!(tile.stream_identity(), stream.identity());
    assert_eq!(tile.target().processor(), "gfx942");
    assert_eq!(tile.to_logical_bits(&stream).unwrap(), logical);
    assert!(matches!(
        tile.to_logical_bits(&other_stream),
        Err(Gfx942TileInteropErrorV1::StreamSubstitution { .. })
    ));

    let allocation_identity = tile.allocation_identity();
    let mut lease = tile.lease(&stream).unwrap();
    assert_eq!(lease.allocation_identity(), allocation_identity);
    assert_eq!(lease.stream_identity(), stream.identity());
    assert_eq!(lease.len(), GFX942_XOR4_BF16_TILE_ELEMENTS_V1);
    assert_eq!(lease.physical_index(3, 5), Some(57));
    assert_eq!(lease.lane_fragment_indices(0), Some([0, 1, 2, 3]));
    unsafe {
        lease
            .run_scoped_unchecked(
                |pointer, length| {
                    assert!(!pointer.as_raw().is_null());
                    assert_eq!(length, GFX942_XOR4_BF16_TILE_ELEMENTS_V1);
                    Ok(())
                },
                |operation| operation.is_complete(),
            )
            .unwrap()
            .unwrap();
    }
    drop(lease);
    assert_eq!(tile.to_logical_bits(&stream).unwrap(), logical);
}
