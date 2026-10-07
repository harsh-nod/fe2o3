//! Ordinary coherent GTT cost projection, not native extraction or authority.
//!
//! The adapter establishes the exact ordinary non-userptr profile and native
//! layout. This projection checks its bounded page-span relationship and charges
//! one physical host backing extent, not its CPU/GPU views twice or VA residency.
//! Native ownership, disposal, mutex/arena behavior and aggregate ceilings are
//! outside the corresponding property-specific Verus proof.

use crate::{R67ResourceKindV1, R67ResourceVectorV1};

pub const R72_HOST_VISIBLE_PAGE_BYTES_V1: u64 = 4096;
pub const R72_MAX_HOST_VISIBLE_ALLOCATION_BYTES_V1: u64 = 2 * 1024 * 1024 * 1024;

/// Accepts exactly a positive request's bounded page-rounded ordinary span.
/// The profile's equal GPU VA extent is checked but is not charged separately.
pub const fn r72_host_visible_backing_charge_v1(
    requested_bytes: u64,
    cpu_mapping_bytes: u64,
    gpu_va_bytes: u64,
) -> Option<R67ResourceVectorV1> {
    if requested_bytes == 0
        || cpu_mapping_bytes > R72_MAX_HOST_VISIBLE_ALLOCATION_BYTES_V1
        || cpu_mapping_bytes < requested_bytes
        || cpu_mapping_bytes - requested_bytes >= R72_HOST_VISIBLE_PAGE_BYTES_V1
        || !cpu_mapping_bytes.is_multiple_of(R72_HOST_VISIBLE_PAGE_BYTES_V1)
        || gpu_va_bytes != cpu_mapping_bytes
    {
        return None;
    }
    Some(
        R67ResourceVectorV1::ZERO
            .with(
                R67ResourceKindV1::ResidentHostAllocationBytes,
                cpu_mapping_bytes,
            )
            .with(R67ResourceKindV1::AllocationRecords, 1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{r67_resource_release_v1, r67_resource_reserve_v1};

    #[test]
    fn r72_charge_is_padded_host_backing_and_one_record_only() {
        for (requested, backing) in [
            (1, 4096),
            (4095, 4096),
            (4096, 4096),
            (4097, 8192),
            (
                R72_MAX_HOST_VISIBLE_ALLOCATION_BYTES_V1 - 1,
                R72_MAX_HOST_VISIBLE_ALLOCATION_BYTES_V1,
            ),
            (
                R72_MAX_HOST_VISIBLE_ALLOCATION_BYTES_V1,
                R72_MAX_HOST_VISIBLE_ALLOCATION_BYTES_V1,
            ),
        ] {
            let charge = r72_host_visible_backing_charge_v1(requested, backing, backing).unwrap();
            for (index, value) in charge.counts().iter().copied().enumerate() {
                assert_eq!(
                    value,
                    match index {
                        i if i == R67ResourceKindV1::ResidentHostAllocationBytes as usize =>
                            backing,
                        i if i == R67ResourceKindV1::AllocationRecords as usize => 1,
                        _ => 0,
                    }
                );
            }
        }
    }

    #[test]
    fn r72_invalid_page_extents_never_issue_a_charge() {
        let max = R72_MAX_HOST_VISIBLE_ALLOCATION_BYTES_V1;
        for (requested, cpu, gpu) in [
            (0, 0, 0),
            (0, 4096, 4096),
            (1, 0, 0),
            (4097, 4096, 4096),
            (1, 8192, 8192),
            (4096, 8192, 8192),
            (4097, 4097, 4097),
            (1, 4096, 8192),
            (1, 4096, 0),
            (max + 1, max + 4096, max + 4096),
            (u64::MAX, u64::MAX, u64::MAX),
            (u64::MAX, 4096, 4096),
        ] {
            assert_eq!(
                r72_host_visible_backing_charge_v1(requested, cpu, gpu),
                None
            );
        }
    }

    #[test]
    fn r72_page_rounding_projection_agrees_for_every_small_boundary() {
        for requested in 1u64..=3 * 4096 + 1 {
            let expected = requested.div_ceil(4096) * 4096;
            assert!(r72_host_visible_backing_charge_v1(requested, expected, expected).is_some());
            for candidate in [expected - 1, expected + 1, expected + 4096] {
                assert_eq!(
                    r72_host_visible_backing_charge_v1(requested, candidate, candidate),
                    None
                );
            }
        }
    }

    #[test]
    fn r72_projected_vector_composes_without_device_or_va_double_charge() {
        let charge = r72_host_visible_backing_charge_v1(4100, 8192, 8192).unwrap();
        let used =
            R67ResourceVectorV1::ZERO.with(R67ResourceKindV1::ResidentDeviceAllocationBytes, 17);
        let capacity = used
            .with(R67ResourceKindV1::ResidentHostAllocationBytes, 8192)
            .with(R67ResourceKindV1::AllocationRecords, 1);
        let next = r67_resource_reserve_v1(used, charge, capacity).unwrap();
        assert_eq!(
            next.get(R67ResourceKindV1::ResidentDeviceAllocationBytes),
            17
        );
        assert_eq!(r67_resource_release_v1(next, charge), Some(used));
        for insufficient in [
            capacity.with(R67ResourceKindV1::ResidentHostAllocationBytes, 8191),
            capacity.with(R67ResourceKindV1::AllocationRecords, 0),
        ] {
            assert_eq!(r67_resource_reserve_v1(used, charge, insufficient), None);
        }
    }
}
