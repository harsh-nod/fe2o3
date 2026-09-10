//! Device-only cached-free policy over a complete, bounded native projection.
//!
//! This is not a residency ledger or disposal authority. The native adapter must
//! establish exact live session/record/lease/charge correspondence and extract
//! padded backing extents. The Verus companion checks the policy scans and
//! arithmetic with reviewed Rust correspondence, not that native extraction,
//! cache mutation, N2 retention, physical disposal or GPU execution.

use crate::{R66DeviceDomainV1, R66DeviceStorageV1, r66_device_storage_in_domain_v1};

pub const R71_MAX_DEVICE_POOL_BACKING_BYTES_V1: u64 = 192 * 1024 * 1024 * 1024;
pub const R71_MAX_DEVICE_POOL_BUFFERS_V1: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R71DevicePoolEntryV1 {
    pub storage: R66DeviceStorageV1,
    pub pool_generation: u64,
    pub backing_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R71DevicePoolUsageV1 {
    pub cached_backing_bytes: u64,
    pub cached_buffers: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R71DevicePoolDispositionV1 {
    Cache,
    Dispose,
}

pub const fn r71_device_pool_limits_valid_v1(bytes: u64, buffers: usize) -> bool {
    bytes <= R71_MAX_DEVICE_POOL_BACKING_BYTES_V1 && buffers <= R71_MAX_DEVICE_POOL_BUFFERS_V1
}

pub const fn r71_device_pool_entry_valid_v1(
    domain: R66DeviceDomainV1,
    entry: R71DevicePoolEntryV1,
) -> bool {
    domain.device_generation != 0
        && domain.vm_id != 0
        && r66_device_storage_in_domain_v1(domain, entry.storage)
        && entry.pool_generation != 0
        && entry.backing_bytes != 0
        && entry.backing_bytes <= R71_MAX_DEVICE_POOL_BACKING_BYTES_V1
}

/// Rejects an invalid/over-limit existing roster instead of hiding it as a miss.
/// Allocation IDs must be distinct even when their native generations differ.
pub fn r71_device_pool_usage_v1(
    domain: R66DeviceDomainV1,
    entries: &[R71DevicePoolEntryV1],
    max_bytes: u64,
    max_buffers: usize,
) -> Option<R71DevicePoolUsageV1> {
    if !r71_device_pool_limits_valid_v1(max_bytes, max_buffers)
        || domain.device_generation == 0
        || domain.vm_id == 0
        || entries.len() > max_buffers
    {
        return None;
    }
    let mut bytes = 0u64;
    let mut i = 0;
    while i < entries.len() {
        if !r71_device_pool_entry_valid_v1(domain, entries[i]) {
            return None;
        }
        let mut j = 0;
        while j < i {
            if entries[i].storage.allocation_id == entries[j].storage.allocation_id {
                return None;
            }
            j += 1;
        }
        bytes = bytes.checked_add(entries[i].backing_bytes)?;
        if bytes > max_bytes {
            return None;
        }
        i += 1;
    }
    Some(R71DevicePoolUsageV1 {
        cached_backing_bytes: bytes,
        cached_buffers: entries.len(),
    })
}

/// A capacity miss chooses disposal; neither disposition releases a native debit.
pub fn r71_device_pool_recycle_v1(
    domain: R66DeviceDomainV1,
    entries: &[R71DevicePoolEntryV1],
    candidate: R71DevicePoolEntryV1,
    max_bytes: u64,
    max_buffers: usize,
) -> Option<R71DevicePoolDispositionV1> {
    let usage = r71_device_pool_usage_v1(domain, entries, max_bytes, max_buffers)?;
    if !r71_device_pool_entry_valid_v1(domain, candidate) {
        return None;
    }
    let mut i = 0;
    while i < entries.len() {
        if entries[i].storage.allocation_id == candidate.storage.allocation_id {
            return None;
        }
        i += 1;
    }
    let next_bytes = usage
        .cached_backing_bytes
        .checked_add(candidate.backing_bytes)?;
    let next_buffers = usage.cached_buffers.checked_add(1)?;
    Some(if next_bytes <= max_bytes && next_buffers <= max_buffers {
        R71DevicePoolDispositionV1::Cache
    } else {
        R71DevicePoolDispositionV1::Dispose
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOMAIN: R66DeviceDomainV1 = R66DeviceDomainV1 {
        physical_device: 7,
        device_generation: 3,
        vm_id: 9,
    };

    fn entry(id: u64, bytes: u64) -> R71DevicePoolEntryV1 {
        R71DevicePoolEntryV1 {
            storage: R66DeviceStorageV1 {
                allocation_id: id,
                generation: 1,
                physical_device: DOMAIN.physical_device,
                device_generation: DOMAIN.device_generation,
                vm_id: DOMAIN.vm_id,
            },
            pool_generation: 1,
            backing_bytes: bytes,
        }
    }

    #[test]
    fn r71_exact_padded_bytes_and_record_boundary() {
        let cached = [entry(1, 4096), entry(2, 8192)];
        assert_eq!(
            r71_device_pool_usage_v1(DOMAIN, &cached, 12288, 2),
            Some(R71DevicePoolUsageV1 {
                cached_backing_bytes: 12288,
                cached_buffers: 2
            })
        );
        assert_eq!(r71_device_pool_usage_v1(DOMAIN, &cached, 12287, 2), None);
        assert_eq!(r71_device_pool_usage_v1(DOMAIN, &cached, 12288, 1), None);
        for (bytes, count, expected) in [
            (16384, 3, R71DevicePoolDispositionV1::Cache),
            (16383, 3, R71DevicePoolDispositionV1::Dispose),
            (16384, 2, R71DevicePoolDispositionV1::Dispose),
        ] {
            assert_eq!(
                r71_device_pool_recycle_v1(DOMAIN, &cached, entry(3, 4096), bytes, count),
                Some(expected)
            );
        }
    }

    #[test]
    fn r71_zero_limits_disable_only_caching() {
        for (bytes, count) in [(0, 0), (0, 128), (8192, 0)] {
            assert_eq!(
                r71_device_pool_usage_v1(DOMAIN, &[], bytes, count),
                Some(R71DevicePoolUsageV1 {
                    cached_backing_bytes: 0,
                    cached_buffers: 0
                })
            );
            assert_eq!(
                r71_device_pool_recycle_v1(DOMAIN, &[], entry(1, 4096), bytes, count),
                Some(R71DevicePoolDispositionV1::Dispose)
            );
            assert_eq!(
                r71_device_pool_usage_v1(DOMAIN, &[entry(1, 4096)], bytes, count),
                None
            );
        }
    }

    #[test]
    fn r71_complete_roster_rejects_every_stale_alias() {
        let entries: [R71DevicePoolEntryV1; 128] =
            core::array::from_fn(|i| entry(i as u64 + 1, 4096));
        assert!(r71_device_pool_usage_v1(DOMAIN, &entries, 128 * 4096, 128).is_some());
        for i in 0..128 {
            let mut invalid = entries;
            invalid[i].pool_generation = 0;
            assert_eq!(
                r71_device_pool_usage_v1(DOMAIN, &invalid, 128 * 4096, 128),
                None
            );
            for j in 0..i {
                let mut aliased = entries;
                aliased[i].storage.allocation_id = aliased[j].storage.allocation_id;
                aliased[i].storage.generation = u64::MAX;
                assert_eq!(
                    r71_device_pool_usage_v1(DOMAIN, &aliased, 128 * 4096, 128),
                    None
                );
            }
            let mut candidate = entries[i];
            candidate.storage.generation = u64::MAX;
            assert_eq!(
                r71_device_pool_recycle_v1(DOMAIN, &entries, candidate, 129 * 4096, 128),
                None
            );
        }
    }

    #[test]
    fn r71_invalid_candidate_never_becomes_capacity_disposal() {
        let valid = entry(2, 4096);
        let mut invalid = [valid; 8];
        invalid[0].storage.allocation_id = 0;
        invalid[1].storage.generation = 0;
        invalid[2].storage.physical_device += 1;
        invalid[3].storage.device_generation += 1;
        invalid[4].storage.vm_id += 1;
        invalid[5].pool_generation = 0;
        invalid[6].backing_bytes = 0;
        invalid[7].backing_bytes = R71_MAX_DEVICE_POOL_BACKING_BYTES_V1 + 1;
        for candidate in invalid {
            assert_eq!(
                r71_device_pool_recycle_v1(DOMAIN, &[], candidate, 0, 0),
                None
            );
            assert_eq!(
                r71_device_pool_usage_v1(DOMAIN, &[candidate], 8192, 1),
                None
            );
        }
    }

    #[test]
    fn r71_bounds_reject_even_empty_roster_and_huge_inputs() {
        assert_eq!(r71_device_pool_usage_v1(DOMAIN, &[], u64::MAX, 128), None);
        assert_eq!(r71_device_pool_usage_v1(DOMAIN, &[], 0, usize::MAX), None);
        assert_eq!(
            r71_device_pool_usage_v1(R66DeviceDomainV1 { vm_id: 0, ..DOMAIN }, &[], 0, 0),
            None
        );
        assert_eq!(
            r71_device_pool_usage_v1(
                R66DeviceDomainV1 {
                    device_generation: 0,
                    ..DOMAIN
                },
                &[],
                0,
                0
            ),
            None
        );
        let entries = [entry(1, R71_MAX_DEVICE_POOL_BACKING_BYTES_V1), entry(2, 1)];
        assert_eq!(
            r71_device_pool_usage_v1(DOMAIN, &entries, R71_MAX_DEVICE_POOL_BACKING_BYTES_V1, 128),
            None
        );
        assert_eq!(
            r71_device_pool_recycle_v1(
                DOMAIN,
                &entries[..1],
                entries[1],
                R71_MAX_DEVICE_POOL_BACKING_BYTES_V1,
                128
            ),
            Some(R71DevicePoolDispositionV1::Dispose)
        );
    }

    #[test]
    fn r71_small_complete_decision_matrix() {
        for bytes in 0..=8 {
            for count in 0..=3 {
                for used in 0..=4 {
                    let cached = [entry(1, used)];
                    let roster = if used == 0 { &cached[..0] } else { &cached[..] };
                    for incoming in 1..=4 {
                        let decision = r71_device_pool_recycle_v1(
                            DOMAIN,
                            roster,
                            entry(2, incoming),
                            bytes,
                            count,
                        );
                        let expected = if used > bytes || roster.len() > count {
                            None
                        } else if used + incoming <= bytes && roster.len() < count {
                            Some(R71DevicePoolDispositionV1::Cache)
                        } else {
                            Some(R71DevicePoolDispositionV1::Dispose)
                        };
                        assert_eq!(decision, expected);
                    }
                }
            }
        }
    }
}
