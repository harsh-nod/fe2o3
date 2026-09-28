//! Differential descriptor geometry checks, not native execution evidence.

use super::*;

fn records() -> AllocationTableV1 {
    let mut records = AllocationTableV1::default();
    for id in 1..=17 {
        records.insert(
            id,
            AllocationRecordV1 {
                device: 7,
                kind: if id.is_multiple_of(2) {
                    RuntimeMemoryKindV1::DeviceLocal
                } else {
                    RuntimeMemoryKindV1::HostVisible
                },
                alignment: [1, 8, 4096][id as usize % 3],
                bytes: Arc::from(vec![0_u8; 8192]),
                content_sha256: None,
                last_full_host_write: None,
                native_dirty: Vec::new(),
                sdma_storage: KfdRuntimeSdmaStorageV1::Synthetic,
                sdma_backed: false,
                sdma_initialized: false,
                sdma_shadow_dirty: false,
                persistent_storage_restore: None,
                scripted_three_binding_replay: false,
            },
        );
    }
    records
}

fn bindings(ids: &[u64]) -> Vec<BackendBindingV1> {
    ids.iter()
        .enumerate()
        .map(|(i, id)| BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: *id,
                access: [
                    RuntimeAccessV1::Read,
                    RuntimeAccessV1::Write,
                    RuntimeAccessV1::ReadWrite,
                ][i % 3],
                byte_offset: (i * 31 % 6000) as u64,
                byte_len: (1 + i * 17 % 1000) as u64,
            },
            kernarg_byte_offset: (i * 8) as u32,
        })
        .collect()
}

// Independent prior algorithm: first-unique prefix scans followed by a full
// binding scan per allocation. Do not reuse the production slot reducer here.
fn prior_projection(
    bindings: &[BackendBindingV1],
    records: &AllocationTableV1,
) -> Option<Vec<ResidentDataDescriptorV1>> {
    let mut descriptors = Vec::new();
    for (i, binding) in bindings.iter().enumerate() {
        let allocation = binding.region.allocation;
        if bindings[..i]
            .iter()
            .any(|prior| prior.region.allocation == allocation)
        {
            continue;
        }
        let record = records.get(&allocation)?;
        if !record.alignment.is_power_of_two() {
            return None;
        }
        let mut start = u64::MAX;
        let mut end = 0;
        for alias in bindings
            .iter()
            .filter(|alias| alias.region.allocation == allocation)
        {
            let limit = alias
                .region
                .byte_offset
                .checked_add(alias.region.byte_len)?;
            if alias.region.byte_len == 0 || limit > record.bytes.len() as u64 {
                return None;
            }
            start = start.min(alias.region.byte_offset & !(record.alignment - 1));
            end = end.max(limit);
        }
        descriptors.push(ResidentDataDescriptorV1 {
            allocation,
            kind: record.kind,
            alignment: record.alignment,
            allocation_offset: start,
            byte_len: end - start,
            host_content_sha256: None,
            device_may_have_modified: false,
        });
    }
    Some(descriptors)
}

fn compare(
    bindings: &[BackendBindingV1],
    descriptors: &[ResidentDataDescriptorV1],
    records: &AllocationTableV1,
) -> bool {
    let expected = bindings.len() <= fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1
        && descriptors.len() <= GFX942_MAX_FIXED_DISPATCH_DATA_V1
        && prior_projection(bindings, records).is_some_and(|prior| {
            prior.len() == descriptors.len()
                && prior.iter().zip(descriptors).all(|(expected, actual)| {
                    // Content hashing is not part of either geometry guard.
                    *expected
                        == ResidentDataDescriptorV1 {
                            host_content_sha256: None,
                            ..*actual
                        }
                })
        });
    let (actual, allocations) = super::super::drain_capture::tests::counted(|| {
        materialized_descriptor_projection_intact_v1(bindings, descriptors, records)
    });
    assert_eq!(allocations, 0);
    assert_eq!(
        actual, expected,
        "bindings={bindings:?} descriptors={descriptors:?}"
    );
    actual
}

#[test]
fn materialized_descriptor_projection_matches_prior_alias_scan() {
    let records = records();
    for len in 0..=6 {
        for mut code in 0..3_usize.pow(len) {
            let ids: Vec<_> = (0..len)
                .map(|_| {
                    let id = 1 + (code % 3) as u64;
                    code /= 3;
                    id
                })
                .collect();
            let bindings = bindings(&ids);
            let descriptors = prior_projection(&bindings, &records).unwrap();
            assert!(compare(&bindings, &descriptors, &records));
            let mut hashed = descriptors.clone();
            for descriptor in &mut hashed {
                descriptor.host_content_sha256 = Some([0xa5; 32]);
            }
            assert!(compare(&bindings, &hashed, &records));
            for i in 0..descriptors.len() {
                for corruption in 0..8 {
                    let mut bad = descriptors.clone();
                    match corruption {
                        0 => bad[i].allocation = 99,
                        1 => {
                            bad[i].kind = match bad[i].kind {
                                RuntimeMemoryKindV1::HostVisible => {
                                    RuntimeMemoryKindV1::DeviceLocal
                                }
                                RuntimeMemoryKindV1::DeviceLocal => {
                                    RuntimeMemoryKindV1::HostVisible
                                }
                            }
                        }
                        2 => bad[i].alignment *= 2,
                        3 => bad[i].allocation_offset += 1,
                        4 => bad[i].byte_len += 1,
                        5 => bad[i].device_may_have_modified = true,
                        6 => {
                            bad.remove(i);
                        }
                        7 => bad.push(bad[i]),
                        _ => unreachable!(),
                    }
                    assert!(!compare(&bindings, &bad, &records));
                }
            }
            if descriptors.len() > 1 {
                let mut bad = descriptors.clone();
                bad.swap(0, 1);
                assert!(!compare(&bindings, &bad, &records));
                bad[1] = bad[0];
                assert!(!compare(&bindings, &bad, &records));
            }
        }
    }
}

#[test]
fn materialized_descriptor_projection_checks_limits_and_bad_ranges() {
    let mut records = records();
    let ids: Vec<_> = (0..128).map(|i| 1 + (i % 16) as u64).collect();
    let bindings = bindings(&ids);
    let descriptors = prior_projection(&bindings, &records).unwrap();
    assert!(compare(&bindings, &descriptors, &records));
    let mut too_many = bindings.clone();
    too_many.push(bindings[0]);
    assert!(!compare(&too_many, &descriptors, &records));
    let mut too_many_data = bindings.clone();
    too_many_data[127].region.allocation = 17;
    let too_many_descriptors = prior_projection(&too_many_data, &records).unwrap();
    assert_eq!(too_many_descriptors.len(), 17);
    assert!(!compare(&too_many_data, &too_many_descriptors, &records));
    for (offset, len) in [(0, 0), (8191, 2), (8192, 1), (u64::MAX, 1), (1, u64::MAX)] {
        let mut bad = bindings.clone();
        bad[127].region.byte_offset = offset;
        bad[127].region.byte_len = len;
        assert!(!compare(&bad, &descriptors, &records));
    }
    for alignment in [0, 3, u64::MAX] {
        records.get_mut(&1).unwrap().alignment = alignment;
        assert!(!compare(&bindings, &descriptors, &records));
    }
    records.get_mut(&1).unwrap().alignment = 8;
    records.remove(&1).unwrap();
    assert!(!compare(&bindings, &descriptors, &records));
}

#[test]
fn materialized_descriptor_projection_checks_isolated_alignment_edges() {
    let mut records = records();
    for alignment in [1, 8, 4096] {
        records.get_mut(&1).unwrap().alignment = alignment;
        for offset in [0, 1, 7, 8, 9, 4095, 4096, 4097, 8191] {
            let mut edge = bindings(&[1]);
            edge[0].region.byte_offset = offset;
            edge[0].region.byte_len = 1;
            let mut projection = prior_projection(&edge, &records).unwrap();
            let start = offset / alignment * alignment;
            assert_eq!(projection[0].allocation_offset, start);
            assert_eq!(projection[0].byte_len, offset + 1 - start);
            for digest in [None, Some([0; 32]), Some([0xff; 32])] {
                projection[0].host_content_sha256 = digest;
                assert!(compare(&edge, &projection, &records));
            }
        }
    }
}
