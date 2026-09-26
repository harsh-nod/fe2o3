use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionRankedAccessSourceV1 as Access, ProductionRankedExecutableEffectOriginV1 as Origin,
    ProductionRankedExecutableEffectSourceV1 as Effect,
    ProductionRankedOutputExtentSourceV1 as Extent, ProductionRankedSourceRowsV1,
    decode_production_ranked_source_rows_v1 as decode,
    encode_production_ranked_source_rows_v1 as encode,
};
use fe2o3_pliron::ProductionRankedValueV1 as Value;

#[test]
fn moved_source_rows_keep_allocation_order_repeated_reads_and_extent_proposals() {
    let access = [
        Access::new(1, Some(4), 0, 3, 8),
        Access::new(1, Some(4), 1, 3, 9),
        Access::new(2, Some(5), 0, 3, 10).with_output_extent(Extent::new(
            7,
            Value::Argument(1),
            Value::Argument(2),
            Value::Argument(3),
        )),
    ];
    let effects = [Effect::new(
        1,
        0,
        3,
        11,
        Origin::GeneratedFromSemanticTerminator,
        [8; 32],
    )];
    budgeted(|b| {
        let (bytes, byte_storage) = encode(&access, &effects, b).unwrap();
        b.reserve_storage(byte_storage.retained_storage()).unwrap();
        let (rows, storage) = decode(&bytes, b).unwrap();
        b.reserve_storage(storage.retained_storage()).unwrap();
        let access_ptr = rows.access_sources().as_ptr();
        let effect_ptr = rows.executable_effect_sources().as_ptr();
        let (moved_access, moved_effects) = rows.into_parts();
        assert_eq!(moved_access.as_ptr(), access_ptr);
        assert_eq!(moved_effects.as_ptr(), effect_ptr);
        assert_eq!(moved_access, access);
        assert_eq!(moved_effects, effects);
        assert_eq!(
            storage.retained_storage(),
            size_of::<ProductionRankedSourceRowsV1>()
                + moved_access.capacity() * size_of::<Access>()
                + moved_effects.capacity() * size_of::<Effect>()
        );
        let (roundtrip, roundtrip_storage) = encode(&moved_access, &moved_effects, b).unwrap();
        b.reserve_storage(roundtrip_storage.retained_storage())
            .unwrap();
        assert_eq!(roundtrip, bytes);
        drop(roundtrip);
        b.release_storage(roundtrip_storage.retained_storage())
            .unwrap();
        drop(moved_access);
        drop(moved_effects);
        b.release_storage(storage.retained_storage()).unwrap();
        drop(bytes);
        b.release_storage(byte_storage.retained_storage()).unwrap();
        assert_eq!(b.storage(), FLOOR);
    });
}
