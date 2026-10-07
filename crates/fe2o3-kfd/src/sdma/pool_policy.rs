//! Device cached-free policy. This scans ownership; it is not a second N2 ledger.

use arrayvec::ArrayVec;
use fe2o3_runtime_model::{
    QueueKeyV1, R66DeviceDomainV1, R71_MAX_DEVICE_POOL_BUFFERS_V1, R71DevicePoolDispositionV1,
    R71DevicePoolEntryV1, r71_device_pool_limits_valid_v1, r71_device_pool_recycle_v1,
    r71_device_pool_usage_v1,
};

use super::{Gfx942SdmaBufferStorageV1, Gfx942SdmaBufferV1};
use crate::MemorySessionError;
use crate::shared_memory::{
    Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryMappedV1,
    MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1, MAX_SHARED_GTT_ALLOCATIONS_V1,
    SharedGttMemorySessionV1,
};

const MAX_MIXED_POOL_BUFFERS_V1: usize =
    MAX_SHARED_GTT_ALLOCATIONS_V1 + MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1;

/// Immutable device cached-free ceilings, independent of resident-backing caps.
/// Either zero limit disables device caching. Host-visible entries are excluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942DevicePoolLimitsV1 {
    max_cached_backing_bytes: u64,
    max_cached_buffers: usize,
}

impl Gfx942DevicePoolLimitsV1 {
    /// Accepts at most 192 GiB and 128 device buffers; zero limits are valid.
    pub const fn new(max_cached_backing_bytes: u64, max_cached_buffers: usize) -> Option<Self> {
        if r71_device_pool_limits_valid_v1(max_cached_backing_bytes, max_cached_buffers) {
            Some(Self {
                max_cached_backing_bytes,
                max_cached_buffers,
            })
        } else {
            None
        }
    }

    pub const fn max_cached_backing_bytes(self) -> u64 {
        self.max_cached_backing_bytes
    }

    pub const fn max_cached_buffers(self) -> usize {
        self.max_cached_buffers
    }
}

/// Inert observation of cached device buffers, not all resident device memory.
/// Checked-out buffers and ambiguous disposal remain outside this cache roster.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942DevicePoolUsageV1 {
    pub limits: Gfx942DevicePoolLimitsV1,
    pub cached_backing_bytes: u64,
    pub cached_buffers: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DevicePoolDispositionV1 {
    Cache,
    Dispose,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DevicePoolPolicyErrorV1 {
    InvalidRoster,
    InvalidCandidate,
}

pub(crate) fn device_pool_usage_v1(
    memory: &SharedGttMemorySessionV1,
    owner: QueueKeyV1,
    limits: Gfx942DevicePoolLimitsV1,
    cached: &[Gfx942SdmaBufferV1],
) -> Result<Gfx942DevicePoolUsageV1, DevicePoolPolicyErrorV1> {
    memory
        .validate_device_pool_domain_v1(owner.vm)
        .map_err(|_| DevicePoolPolicyErrorV1::InvalidRoster)?;
    device_pool_usage_with_v1(owner, limits, cached, &mut |lease| {
        memory.device_pool_backing_bytes_v1(lease)
    })
}

pub(crate) fn device_pool_recycle_decision_v1(
    memory: &SharedGttMemorySessionV1,
    owner: QueueKeyV1,
    limits: Gfx942DevicePoolLimitsV1,
    cached: &[Gfx942SdmaBufferV1],
    candidate: &Gfx942SdmaBufferV1,
) -> Result<DevicePoolDispositionV1, DevicePoolPolicyErrorV1> {
    memory
        .validate_device_pool_domain_v1(owner.vm)
        .map_err(|_| DevicePoolPolicyErrorV1::InvalidRoster)?;
    device_pool_recycle_decision_with_v1(owner, limits, cached, candidate, &mut |lease| {
        memory.device_pool_backing_bytes_v1(lease)
    })
}

fn domain_v1(owner: QueueKeyV1) -> R66DeviceDomainV1 {
    R66DeviceDomainV1 {
        physical_device: owner.vm.device.physical.0,
        device_generation: owner.vm.device.generation.0,
        vm_id: owner.vm.id.0,
    }
}

fn buffer_shape_valid_v1(owner: QueueKeyV1, buffer: &Gfx942SdmaBufferV1) -> bool {
    buffer.belongs_to(owner)
        && buffer.pool_generation() != 0
        && buffer.requested_bytes() != 0
        && buffer.requested_bytes() <= buffer.physical_bytes()
}

fn project_device_v1(
    owner: QueueKeyV1,
    buffer: &Gfx942SdmaBufferV1,
    backing_bytes: &mut impl FnMut(
        &Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<u64, MemorySessionError>,
) -> Option<R71DevicePoolEntryV1> {
    if !buffer_shape_valid_v1(owner, buffer) {
        return None;
    }
    let Gfx942SdmaBufferStorageV1::Device(lease) = &buffer.storage else {
        return None;
    };
    let bytes = backing_bytes(lease).ok()?;
    if bytes != lease.layout().backing_bytes() {
        return None;
    }
    Some(R71DevicePoolEntryV1 {
        storage: lease.storage_identity().coexistence_facts_v1()?,
        pool_generation: buffer.pool_generation(),
        backing_bytes: bytes,
    })
}

fn project_roster_v1(
    owner: QueueKeyV1,
    cached: &[Gfx942SdmaBufferV1],
    backing_bytes: &mut impl FnMut(
        &Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<u64, MemorySessionError>,
) -> Result<ArrayVec<R71DevicePoolEntryV1, R71_MAX_DEVICE_POOL_BUFFERS_V1>, DevicePoolPolicyErrorV1>
{
    if cached.len() > MAX_MIXED_POOL_BUFFERS_V1 {
        return Err(DevicePoolPolicyErrorV1::InvalidRoster);
    }
    let mut entries = ArrayVec::new();
    let mut host_buffers = 0usize;
    for buffer in cached {
        if !buffer_shape_valid_v1(owner, buffer) {
            return Err(DevicePoolPolicyErrorV1::InvalidRoster);
        }
        if matches!(&buffer.storage, Gfx942SdmaBufferStorageV1::Device(_)) {
            let entry = project_device_v1(owner, buffer, backing_bytes)
                .ok_or(DevicePoolPolicyErrorV1::InvalidRoster)?;
            entries
                .try_push(entry)
                .map_err(|_| DevicePoolPolicyErrorV1::InvalidRoster)?;
        } else {
            host_buffers += 1;
            if host_buffers > MAX_SHARED_GTT_ALLOCATIONS_V1 {
                return Err(DevicePoolPolicyErrorV1::InvalidRoster);
            }
        }
    }
    Ok(entries)
}

// The production wrappers supply the exact native record/lease validator. This
// private boundary also lets fake-engine tests exercise the identical roster scan.
pub(crate) fn device_pool_usage_with_v1(
    owner: QueueKeyV1,
    limits: Gfx942DevicePoolLimitsV1,
    cached: &[Gfx942SdmaBufferV1],
    backing_bytes: &mut impl FnMut(
        &Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<u64, MemorySessionError>,
) -> Result<Gfx942DevicePoolUsageV1, DevicePoolPolicyErrorV1> {
    let entries = project_roster_v1(owner, cached, backing_bytes)?;
    let usage = r71_device_pool_usage_v1(
        domain_v1(owner),
        &entries,
        limits.max_cached_backing_bytes,
        limits.max_cached_buffers,
    )
    .ok_or(DevicePoolPolicyErrorV1::InvalidRoster)?;
    Ok(Gfx942DevicePoolUsageV1 {
        limits,
        cached_backing_bytes: usage.cached_backing_bytes,
        cached_buffers: usage.cached_buffers,
    })
}

pub(crate) fn device_pool_recycle_decision_with_v1(
    owner: QueueKeyV1,
    limits: Gfx942DevicePoolLimitsV1,
    cached: &[Gfx942SdmaBufferV1],
    candidate: &Gfx942SdmaBufferV1,
    backing_bytes: &mut impl FnMut(
        &Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<u64, MemorySessionError>,
) -> Result<DevicePoolDispositionV1, DevicePoolPolicyErrorV1> {
    let entries = project_roster_v1(owner, cached, backing_bytes)?;
    // Classify a corrupt existing roster before considering the new candidate.
    r71_device_pool_usage_v1(
        domain_v1(owner),
        &entries,
        limits.max_cached_backing_bytes,
        limits.max_cached_buffers,
    )
    .ok_or(DevicePoolPolicyErrorV1::InvalidRoster)?;
    let candidate = project_device_v1(owner, candidate, backing_bytes)
        .ok_or(DevicePoolPolicyErrorV1::InvalidCandidate)?;
    match r71_device_pool_recycle_v1(
        domain_v1(owner),
        &entries,
        candidate,
        limits.max_cached_backing_bytes,
        limits.max_cached_buffers,
    )
    .ok_or(DevicePoolPolicyErrorV1::InvalidCandidate)?
    {
        R71DevicePoolDispositionV1::Cache => Ok(DevicePoolDispositionV1::Cache),
        R71DevicePoolDispositionV1::Dispose => Ok(DevicePoolDispositionV1::Dispose),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_runtime_model::{
        DeviceGenerationV1, DeviceKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1,
        VmIdV1, VmKeyV1,
    };

    fn owner() -> QueueKeyV1 {
        QueueKeyV1 {
            vm: VmKeyV1 {
                device: DeviceKeyV1 {
                    physical: PhysicalDeviceIdV1(7),
                    generation: DeviceGenerationV1(1),
                },
                id: VmIdV1(1),
            },
            id: QueueInstanceIdV1(1),
            generation: QueueGenerationV1(1),
        }
    }

    // Synthetic layout projections test only the policy adapter. Exact native
    // records/charges and disposal are covered by shared_memory fake-engine tests.
    fn layout_bytes(
        lease: &Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<u64, MemorySessionError> {
        Ok(lease.layout().backing_bytes())
    }

    #[test]
    fn device_pool_limit_bounds_and_zero_are_explicit() {
        let zero = Gfx942DevicePoolLimitsV1::new(0, 0).unwrap();
        assert_eq!(zero.max_cached_backing_bytes(), 0);
        assert_eq!(zero.max_cached_buffers(), 0);
        assert!(Gfx942DevicePoolLimitsV1::new(192 * 1024 * 1024 * 1024, 128).is_some());
        assert!(Gfx942DevicePoolLimitsV1::new(192 * 1024 * 1024 * 1024 + 1, 128).is_none());
        assert!(Gfx942DevicePoolLimitsV1::new(1, 129).is_none());
    }

    #[test]
    fn device_pool_scan_excludes_host_and_keeps_logical_bytes_separate() {
        let owner = owner();
        let (mut device, host) = super::super::persistent_sdma_buffers_for_test(owner, 1);
        device.set_logical_bytes(1);
        let usage = device_pool_usage_with_v1(
            owner,
            Gfx942DevicePoolLimitsV1::new(4096, 1).unwrap(),
            &[host, device],
            &mut layout_bytes,
        )
        .unwrap();
        assert_eq!(usage.cached_backing_bytes, 4096);
        assert_eq!(usage.cached_buffers, 1);
    }

    #[test]
    fn device_pool_capacity_miss_disposes_and_invalid_candidate_rejects() {
        let owner = owner();
        let (device, host) = super::super::persistent_sdma_buffers_for_test(owner, 1);
        let disabled = Gfx942DevicePoolLimitsV1::new(0, 0).unwrap();
        assert_eq!(
            device_pool_recycle_decision_with_v1(owner, disabled, &[], &device, &mut layout_bytes),
            Ok(DevicePoolDispositionV1::Dispose)
        );
        assert_eq!(
            device_pool_recycle_decision_with_v1(owner, disabled, &[], &host, &mut layout_bytes),
            Err(DevicePoolPolicyErrorV1::InvalidCandidate)
        );
        assert_eq!(
            device_pool_recycle_decision_with_v1(owner, disabled, &[], &device, &mut |_| Ok(1)),
            Err(DevicePoolPolicyErrorV1::InvalidCandidate)
        );
    }

    #[test]
    fn device_pool_complete_scan_rejects_foreign_alias_and_bad_last_member() {
        let owner = owner();
        let limits = Gfx942DevicePoolLimitsV1::new(128 * 4096, 128).unwrap();
        let mut cached: Vec<_> = (1..=128)
            .map(|id| super::super::persistent_sdma_buffers_for_test(owner, id).0)
            .collect();
        let mut calls = 0;
        assert_eq!(
            device_pool_usage_with_v1(owner, limits, &cached, &mut |lease| {
                calls += 1;
                layout_bytes(lease)
            })
            .unwrap()
            .cached_buffers,
            128
        );
        assert_eq!(calls, 128);
        cached[127].pool_generation = 0;
        assert_eq!(
            device_pool_usage_with_v1(owner, limits, &cached, &mut layout_bytes),
            Err(DevicePoolPolicyErrorV1::InvalidRoster)
        );
        cached[127] = super::super::persistent_sdma_buffers_for_test(owner, 1).0;
        assert_eq!(
            device_pool_usage_with_v1(owner, limits, &cached, &mut layout_bytes),
            Err(DevicePoolPolicyErrorV1::InvalidRoster)
        );
        cached[127] = super::super::persistent_sdma_buffers_for_test(owner, 128).0;
        cached[127].owner.generation = QueueGenerationV1(2);
        assert_eq!(
            device_pool_usage_with_v1(owner, limits, &cached, &mut layout_bytes),
            Err(DevicePoolPolicyErrorV1::InvalidRoster)
        );
    }

    #[test]
    fn device_pool_roster_bounds_and_overlimit_are_not_disposal() {
        let owner = owner();
        let limits = Gfx942DevicePoolLimitsV1::new(128 * 4096, 128).unwrap();
        let cached: Vec<_> = (1..=129)
            .map(|id| super::super::persistent_sdma_buffers_for_test(owner, id).0)
            .collect();
        assert_eq!(
            device_pool_usage_with_v1(owner, limits, &cached, &mut layout_bytes),
            Err(DevicePoolPolicyErrorV1::InvalidRoster)
        );
        let (candidate, _) = super::super::persistent_sdma_buffers_for_test(owner, 130);
        assert_eq!(
            device_pool_recycle_decision_with_v1(
                owner,
                Gfx942DevicePoolLimitsV1::new(0, 0).unwrap(),
                &cached[..1],
                &candidate,
                &mut layout_bytes
            ),
            Err(DevicePoolPolicyErrorV1::InvalidRoster)
        );
        let hosts: Vec<_> = (1..=257)
            .map(|id| super::super::persistent_sdma_buffers_for_test(owner, id).1)
            .collect();
        assert_eq!(
            device_pool_usage_with_v1(owner, limits, &hosts, &mut layout_bytes),
            Err(DevicePoolPolicyErrorV1::InvalidRoster)
        );
    }
}
