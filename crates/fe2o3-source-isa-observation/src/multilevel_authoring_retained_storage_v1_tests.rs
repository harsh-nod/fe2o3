//! Synthetic fixture accounting only, not measured source-workflow performance.
use super::*;
use fe2o3_kernel_ir::{
    LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error,
    LogicalStorageLimitsV1 as Limits,
};
use std::mem::size_of;

fn measure(snapshot: &AuthoringSnapshotV1) -> AuthoringSnapshotRetainedStorageV1 {
    snapshot
        .retained_logical_storage_v1(Limits {
            max_bytes: None,
            max_items: 10_000,
        })
        .unwrap()
}

#[test]
fn complete_snapshot_components_include_one_header_and_every_index() {
    let snapshot = snapshot(true);
    let measured = measure(&snapshot);
    assert_eq!(measured.inline_bytes, size_of::<AuthoringSnapshotV1>());
    assert_eq!(
        measured.operation_index_owned_bytes,
        snapshot.operations.capacity() * size_of::<AuthoringOperationCoordinateV1>()
    );
    assert_eq!(
        measured.definition_index_owned_bytes,
        snapshot.definitions.capacity() * size_of::<BTreeMap<ValueId, Definition>>()
            + snapshot
                .definitions
                .iter()
                .map(|definitions| definitions.len()
                    * (size_of::<ValueId>() + size_of::<Definition>()))
                .sum::<usize>()
    );
    assert_eq!(
        measured.source_site_index_owned_bytes,
        snapshot.source_sites.len()
            * (size_of::<AuthoringOperationCoordinateV1>() + size_of::<usize>())
    );
    assert_eq!(
        measured.source_file_index_owned_bytes,
        snapshot.source_files.len() * (size_of::<[u8; 32]>() + size_of::<usize>())
    );
    for (expected, owner) in [
        (measured.bundle_owned_bytes, 0),
        (measured.module_owned_bytes, 1),
        (measured.source_map_owned_bytes, 2),
    ] {
        let mut c = Counter::new(Limits {
            max_bytes: None,
            max_items: 10_000,
        });
        match owner {
            0 => snapshot.bundle.charge_retained_heap_v6(&mut c).unwrap(),
            1 => snapshot.module.charge_retained_heap_v11(&mut c).unwrap(),
            _ => snapshot.source_map.charge_retained_heap_v2(&mut c).unwrap(),
        }
        assert_eq!(expected, c.bytes());
    }
    assert_eq!(
        measured.total_bytes,
        [
            measured.inline_bytes,
            measured.bundle_owned_bytes,
            measured.module_owned_bytes,
            measured.source_map_owned_bytes,
            measured.operation_index_owned_bytes,
            measured.definition_index_owned_bytes,
            measured.source_site_index_owned_bytes,
            measured.source_file_index_owned_bytes,
        ]
        .into_iter()
        .sum::<usize>()
    );
}

#[test]
fn incoming_bundle_spare_capacity_is_retained_not_section_bytes_twice() {
    let bytes = fixture_bundle(
        fixture_module(false, ScalarType::U32, "v_add_u32", None, false),
        "gfx942:xnack-",
        false,
    )
    .into_canonical_bytes();
    let mut bytes = bytes;
    bytes.reserve(4096);
    let capacity = bytes.capacity();
    assert!(capacity > bytes.len());
    let bundle = VerifiedSimulationBundleV6::from_canonical_bytes(bytes).unwrap();
    let snapshot = AuthoringSnapshotV1::from_bundle_v6(bundle).unwrap();
    assert_eq!(measure(&snapshot).bundle_owned_bytes, capacity);
}

#[test]
fn empty_spare_index_slots_and_each_map_entry_are_visible() {
    let mut snapshot = snapshot(false);
    let before = measure(&snapshot);
    let old_operations_capacity = snapshot.operations.capacity();
    let old_definitions_capacity = snapshot.definitions.capacity();
    snapshot.operations.reserve(31);
    snapshot.definitions.reserve(37);
    let reserved = measure(&snapshot);
    assert_eq!(
        reserved.total_bytes - before.total_bytes,
        (snapshot.operations.capacity() - old_operations_capacity)
            * size_of::<AuthoringOperationCoordinateV1>()
            + (snapshot.definitions.capacity() - old_definitions_capacity)
                * size_of::<BTreeMap<ValueId, Definition>>()
    );
    // Test-only private mutations; no public authoring mutation API is added.
    snapshot.definitions[0].insert(ValueId(12345), Definition::Parameter(0));
    snapshot.source_sites.insert(
        AuthoringOperationCoordinateV1 {
            function: 99,
            block: 99,
            operation: 99,
        },
        1,
    );
    snapshot.source_files.insert([99; 32], 1);
    let inserted = measure(&snapshot);
    assert_eq!(
        inserted.total_bytes - reserved.total_bytes,
        size_of::<ValueId>()
            + size_of::<Definition>()
            + size_of::<AuthoringOperationCoordinateV1>()
            + size_of::<usize>()
            + size_of::<[u8; 32]>()
            + size_of::<usize>()
    );
}

#[test]
fn explicit_limits_refuse_without_changing_output_or_authority() {
    let snapshot = snapshot(true);
    let selected = selector(&snapshot, 2);
    let before = serde_json::to_vec(&(
        snapshot.summary(),
        snapshot
            .operation_page(&selected.bundle_identity, 0, 64)
            .unwrap(),
        snapshot.select_region(&selected).unwrap(),
        snapshot
            .materialize_typed_rust(&selected, "candidate")
            .unwrap(),
    ))
    .unwrap();
    let measured = measure(&snapshot);
    let exact = Limits {
        max_bytes: Some(measured.total_bytes),
        max_items: measured.visited_items,
    };
    assert_eq!(
        snapshot.retained_logical_storage_v1(exact).unwrap(),
        measured
    );
    assert_eq!(
        snapshot.retained_logical_storage_v1(Limits {
            max_bytes: Some(measured.total_bytes - 1),
            ..exact
        }),
        Err(Error::ByteLimit)
    );
    assert_eq!(
        snapshot.retained_logical_storage_v1(Limits {
            max_bytes: None,
            max_items: measured.visited_items - 1,
        }),
        Err(Error::ItemLimit)
    );
    assert_eq!(
        snapshot.retained_logical_storage_v1(Limits {
            max_bytes: Some(0),
            max_items: 10_000,
        }),
        Err(Error::ByteLimit)
    );
    assert_eq!(
        snapshot.retained_logical_storage_v1(Limits {
            max_bytes: None,
            max_items: 0,
        }),
        Err(Error::ItemLimit)
    );
    assert_eq!(measure(&snapshot), measured);
    let after = serde_json::to_vec(&(
        snapshot.summary(),
        snapshot
            .operation_page(&selected.bundle_identity, 0, 64)
            .unwrap(),
        snapshot.select_region(&selected).unwrap(),
        snapshot
            .materialize_typed_rust(&selected, "candidate")
            .unwrap(),
    ))
    .unwrap();
    assert_eq!(before, after);
    assert_eq!(snapshot.summary().authority, AUTHORITY);
}
