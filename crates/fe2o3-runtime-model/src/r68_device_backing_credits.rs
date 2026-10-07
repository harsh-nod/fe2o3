//! Exact two-dimensional projection of an already established native backing extent.
//!
//! This does not establish layout extraction, native domains, physical residency,
//! ownership or disposal. Those remain the native adapter's reviewed obligations.

use crate::{R67ResourceKindV1, R67ResourceVectorV1};

pub const R68_MAX_DEVICE_BACKING_BYTES_V1: u64 = 192 * 1024 * 1024 * 1024;

/// Produces one N2 allocation's charge, not requested logical bytes or GTT cost.
pub const fn r68_device_backing_charge_v1(backing_bytes: u64) -> Option<R67ResourceVectorV1> {
    if backing_bytes == 0 || backing_bytes > R68_MAX_DEVICE_BACKING_BYTES_V1 {
        return None;
    }
    Some(
        R67ResourceVectorV1::ZERO
            .with(
                R67ResourceKindV1::ResidentDeviceAllocationBytes,
                backing_bytes,
            )
            .with(R67ResourceKindV1::AllocationRecords, 1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{r67_resource_release_v1, r67_resource_reserve_v1};

    #[test]
    fn projection_rejects_zero_and_every_out_of_profile_boundary() {
        for bytes in [0, R68_MAX_DEVICE_BACKING_BYTES_V1 + 1, u64::MAX] {
            assert_eq!(r68_device_backing_charge_v1(bytes), None);
        }
    }

    #[test]
    fn projection_preserves_backing_bytes_one_record_and_no_other_dimension() {
        for bytes in [1, 4096, 8192, R68_MAX_DEVICE_BACKING_BYTES_V1] {
            let charge = r68_device_backing_charge_v1(bytes).unwrap();
            for (index, value) in charge.counts().iter().copied().enumerate() {
                assert_eq!(
                    value,
                    match index {
                        value
                            if value
                                == R67ResourceKindV1::ResidentDeviceAllocationBytes as usize =>
                            bytes,
                        value if value == R67ResourceKindV1::AllocationRecords as usize => 1,
                        _ => 0,
                    }
                );
            }
        }
    }

    #[test]
    fn projected_charge_composes_with_shared_reserve_and_release() {
        let charge = r68_device_backing_charge_v1(8192).unwrap();
        let used = R67ResourceVectorV1::ZERO.with(R67ResourceKindV1::LogicalPayloadBytes, 17);
        let capacity = used
            .with(R67ResourceKindV1::ResidentDeviceAllocationBytes, 8192)
            .with(R67ResourceKindV1::AllocationRecords, 1);
        let next = r67_resource_reserve_v1(used, charge, capacity).unwrap();
        assert_eq!(r67_resource_release_v1(next, charge), Some(used));
        for insufficient in [
            capacity.with(R67ResourceKindV1::ResidentDeviceAllocationBytes, 8191),
            capacity.with(R67ResourceKindV1::AllocationRecords, 0),
        ] {
            assert_eq!(r67_resource_reserve_v1(used, charge, insufficient), None);
        }
    }
}
