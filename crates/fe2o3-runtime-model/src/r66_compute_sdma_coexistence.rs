//! Bounded storage noninterference checks, not native execution authority.
//!
//! The Verus companion verifies scalar checks and the bounded scan, with reviewed
//! correspondence to this Rust implementation. Native adapters must extract the
//! complete retained compute and copy rosters, validate owner/session/currentness
//! and host endpoints, and exclude unsupported queue modes. Allocation identity
//! extraction, physical nonaliasing, publication and GPU execution are not proved
//! here. Every model value remains publicly constructible and non-authoritative.

pub const R66_MAX_COMPUTE_BINDINGS_V1: usize = 16;
pub const R66_MAX_COPY_DEVICE_ENDPOINTS_V1: usize = 258;
pub const R66_DIRECTIONAL_RING_SLOTS_V1: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R66DeviceDomainV1 {
    pub physical_device: u64,
    pub device_generation: u64,
    pub vm_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R66DeviceStorageV1 {
    pub allocation_id: u64,
    pub generation: u64,
    pub physical_device: u64,
    pub device_generation: u64,
    pub vm_id: u64,
}

pub const fn r66_device_storage_in_domain_v1(
    domain: R66DeviceDomainV1,
    storage: R66DeviceStorageV1,
) -> bool {
    storage.allocation_id != 0
        && storage.generation != 0
        && storage.physical_device == domain.physical_device
        && storage.device_generation == domain.device_generation
        && storage.vm_id == domain.vm_id
}

/// Checks whole native allocations, deliberately ignoring subrange disjointness.
/// A changed generation cannot turn the same allocation ID into disjoint storage.
/// Duplicate copy endpoints are allowed; no copy/compute alias is allowed.
pub fn r66_device_storage_rosters_disjoint_v1(
    domain: R66DeviceDomainV1,
    compute: &[R66DeviceStorageV1],
    copy_devices: &[R66DeviceStorageV1],
) -> bool {
    if compute.is_empty()
        || compute.len() > R66_MAX_COMPUTE_BINDINGS_V1
        || copy_devices.len() > R66_MAX_COPY_DEVICE_ENDPOINTS_V1
    {
        return false;
    }
    let mut i = 0;
    while i < compute.len() {
        if !r66_device_storage_in_domain_v1(domain, compute[i]) {
            return false;
        }
        let mut j = 0;
        while j < i {
            if compute[i].allocation_id == compute[j].allocation_id {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    let mut k = 0;
    while k < copy_devices.len() {
        if !r66_device_storage_in_domain_v1(domain, copy_devices[k]) {
            return false;
        }
        let mut j = 0;
        while j < compute.len() {
            if copy_devices[k].allocation_id == compute[j].allocation_id {
                return false;
            }
            j += 1;
        }
        k += 1;
    }
    true
}

/// Checks one retained slot against its actual cyclic window and slot generation.
/// The adapter must additionally establish that the anchor record exists, the
/// roster is complete, and no ordinary/XGMI record occupies the same slot.
pub const fn r66_window_slot_matches_v1(
    slot: usize,
    anchor: usize,
    packet_count: usize,
    generation: u32,
    expected_generation: u32,
    completion_value: u32,
) -> bool {
    slot < R66_DIRECTIONAL_RING_SLOTS_V1
        && anchor < R66_DIRECTIONAL_RING_SLOTS_V1
        && packet_count != 0
        && packet_count < R66_DIRECTIONAL_RING_SLOTS_V1
        && generation != 0
        && generation == expected_generation
        && completion_value == generation
        && (slot + R66_DIRECTIONAL_RING_SLOTS_V1 - anchor) % R66_DIRECTIONAL_RING_SLOTS_V1
            < packet_count
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOMAIN: R66DeviceDomainV1 = R66DeviceDomainV1 {
        physical_device: 7,
        device_generation: 3,
        vm_id: 9,
    };

    fn storage(allocation_id: u64) -> R66DeviceStorageV1 {
        R66DeviceStorageV1 {
            allocation_id,
            generation: 1,
            physical_device: DOMAIN.physical_device,
            device_generation: DOMAIN.device_generation,
            vm_id: DOMAIN.vm_id,
        }
    }

    #[test]
    fn r66_storage_scan_checks_every_compute_and_copy_endpoint() {
        let compute: [R66DeviceStorageV1; 16] = core::array::from_fn(|i| storage(i as u64 + 1));
        let copy: [R66DeviceStorageV1; 258] = core::array::from_fn(|i| storage(i as u64 + 100));
        assert!(r66_device_storage_rosters_disjoint_v1(
            DOMAIN, &compute, &copy
        ));
        for ci in 0..compute.len() {
            for si in 0..copy.len() {
                let mut aliased = copy;
                aliased[si].allocation_id = compute[ci].allocation_id;
                // Stale pool/storage generations are not evidence of disjointness.
                aliased[si].generation = u64::MAX;
                assert!(!r66_device_storage_rosters_disjoint_v1(
                    DOMAIN, &compute, &aliased
                ));
            }
        }
    }

    #[test]
    fn r66_storage_scan_rejects_each_invalid_identity_and_domain() {
        let good = storage(1);
        let invalid = [
            R66DeviceStorageV1 {
                allocation_id: 0,
                ..good
            },
            R66DeviceStorageV1 {
                generation: 0,
                ..good
            },
            R66DeviceStorageV1 {
                physical_device: 8,
                ..good
            },
            R66DeviceStorageV1 {
                device_generation: 4,
                ..good
            },
            R66DeviceStorageV1 { vm_id: 10, ..good },
        ];
        for invalid_storage in invalid {
            assert!(!r66_device_storage_rosters_disjoint_v1(
                DOMAIN,
                &[invalid_storage],
                &[storage(2)]
            ));
            assert!(!r66_device_storage_rosters_disjoint_v1(
                DOMAIN,
                &[storage(2)],
                &[invalid_storage]
            ));
        }
        assert!(!r66_device_storage_rosters_disjoint_v1(
            DOMAIN,
            &[
                good,
                R66DeviceStorageV1 {
                    generation: 2,
                    ..good
                }
            ],
            &[]
        ));
    }

    #[test]
    fn r66_storage_scan_bounds_empty_and_duplicate_copy_cases() {
        assert!(!r66_device_storage_rosters_disjoint_v1(DOMAIN, &[], &[]));
        let compute: [R66DeviceStorageV1; 17] = core::array::from_fn(|i| storage(i as u64 + 1));
        assert!(!r66_device_storage_rosters_disjoint_v1(
            DOMAIN,
            &compute,
            &[]
        ));
        assert!(!r66_device_storage_rosters_disjoint_v1(
            DOMAIN,
            &[storage(1)],
            &[storage(2); 259]
        ));
        assert!(r66_device_storage_rosters_disjoint_v1(
            DOMAIN,
            &[storage(1)],
            &[]
        ));
        assert!(r66_device_storage_rosters_disjoint_v1(
            DOMAIN,
            &[storage(1)],
            &[storage(2); 258]
        ));
    }

    #[test]
    fn r66_window_scan_exhausts_cyclic_slots_and_bounds() {
        for anchor in 0..64 {
            for count in 1..64 {
                for slot in 0..64 {
                    let expected = (0..count).any(|offset| (anchor + offset) % 64 == slot);
                    assert_eq!(
                        r66_window_slot_matches_v1(slot, anchor, count, 5, 5, 5),
                        expected
                    );
                }
            }
        }
        for (slot, anchor, count) in [
            (64, 0, 1),
            (0, 64, 1),
            (0, 0, 0),
            (0, 0, 64),
            (usize::MAX, 0, 1),
            (0, usize::MAX, 1),
        ] {
            assert!(!r66_window_slot_matches_v1(slot, anchor, count, 5, 5, 5));
        }
        for (generation, expected, completion) in [(0, 0, 0), (5, 6, 5), (5, 5, 6)] {
            assert!(!r66_window_slot_matches_v1(
                0, 0, 1, generation, expected, completion
            ));
        }
    }
}
