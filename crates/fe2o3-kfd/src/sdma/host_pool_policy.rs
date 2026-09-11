//! Ordinary coherent host cache policy, not a second backing-credit ledger.

use arrayvec::ArrayVec;
use fe2o3_runtime_model::QueueKeyV1;

use super::{Gfx942SdmaBufferStorageV1, Gfx942SdmaBufferV1, MappedHostBufferV1};
use crate::MemorySessionError;
use crate::shared_memory::{
    MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1, MAX_SHARED_GTT_ALLOCATIONS_V1,
    MAX_SHARED_GTT_GPU_VA_BYTES_V1, SharedGttAllocationIdentityV1, SharedGttMemorySessionV1,
};

/// Immutable cached-free ordinary coherent GTT limits. Either zero disables
/// caching, not allocation. Checked-out backing and nonordinary profiles are excluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942HostPoolLimitsV1 {
    max_cached_backing_bytes: u64,
    max_cached_buffers: usize,
}

impl Gfx942HostPoolLimitsV1 {
    /// Accepts at most 8 GiB and 256 buffers. Zero limits are valid.
    pub const fn new(max_cached_backing_bytes: u64, max_cached_buffers: usize) -> Option<Self> {
        if max_cached_backing_bytes <= MAX_SHARED_GTT_GPU_VA_BYTES_V1
            && max_cached_buffers <= MAX_SHARED_GTT_ALLOCATIONS_V1
        {
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

/// Inert padded cache occupancy, not total backing, currentness or disposal evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942HostPoolUsageV1 {
    pub limits: Gfx942HostPoolLimitsV1,
    pub cached_backing_bytes: u64,
    pub cached_buffers: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostPoolDispositionV1 {
    Cache,
    Dispose,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostPoolPolicyErrorV1 {
    InvalidRoster,
    InvalidCandidate,
}

pub(crate) fn host_pool_usage_v1(
    memory: &SharedGttMemorySessionV1,
    owner: QueueKeyV1,
    limits: Gfx942HostPoolLimitsV1,
    cached: &[Gfx942SdmaBufferV1],
) -> Result<Gfx942HostPoolUsageV1, HostPoolPolicyErrorV1> {
    memory
        .validate_host_pool_domain_v1(owner.vm)
        .map_err(|_| HostPoolPolicyErrorV1::InvalidRoster)?;
    host_pool_usage_with_v1(owner, limits, cached, &mut |token| {
        memory.host_pool_backing_bytes_v1(token)
    })
}

pub(crate) fn host_pool_recycle_decision_v1(
    memory: &SharedGttMemorySessionV1,
    owner: QueueKeyV1,
    limits: Gfx942HostPoolLimitsV1,
    cached: &[Gfx942SdmaBufferV1],
    candidate: &Gfx942SdmaBufferV1,
) -> Result<HostPoolDispositionV1, HostPoolPolicyErrorV1> {
    memory
        .validate_host_pool_domain_v1(owner.vm)
        .map_err(|_| HostPoolPolicyErrorV1::InvalidRoster)?;
    host_pool_recycle_decision_with_v1(owner, limits, cached, candidate, &mut |token| {
        memory.host_pool_backing_bytes_v1(token)
    })
}

fn shape_valid(owner: QueueKeyV1, buffer: &Gfx942SdmaBufferV1) -> bool {
    buffer.belongs_to(owner)
        && buffer.pool_generation() != 0
        && buffer.requested_bytes() != 0
        && buffer.requested_bytes() <= buffer.physical_bytes()
}

fn project(
    owner: QueueKeyV1,
    buffer: &Gfx942SdmaBufferV1,
    backing_bytes: &mut impl FnMut(&MappedHostBufferV1) -> Result<u64, MemorySessionError>,
) -> Option<(SharedGttAllocationIdentityV1, u64)> {
    if !shape_valid(owner, buffer) {
        return None;
    }
    let Gfx942SdmaBufferStorageV1::Host(token) = &buffer.storage else {
        return None;
    };
    let bytes = backing_bytes(token).ok()?;
    if bytes == 0 || bytes != u64::try_from(token.layout().cpu_mapping_bytes()).ok()? {
        return None;
    }
    Some((token.storage_identity(), bytes))
}

type Roster = ArrayVec<SharedGttAllocationIdentityV1, MAX_SHARED_GTT_ALLOCATIONS_V1>;

fn scan(
    owner: QueueKeyV1,
    limits: Gfx942HostPoolLimitsV1,
    cached: &[Gfx942SdmaBufferV1],
    backing_bytes: &mut impl FnMut(&MappedHostBufferV1) -> Result<u64, MemorySessionError>,
) -> Result<(Roster, Gfx942HostPoolUsageV1), HostPoolPolicyErrorV1> {
    use HostPoolPolicyErrorV1::InvalidRoster;
    if cached.len() > MAX_SHARED_GTT_ALLOCATIONS_V1 + MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1
    {
        return Err(InvalidRoster);
    }
    let mut identities = Roster::new();
    let mut usage = Gfx942HostPoolUsageV1 {
        limits,
        cached_backing_bytes: 0,
        cached_buffers: 0,
    };
    let mut devices = 0usize;
    for buffer in cached {
        if !shape_valid(owner, buffer) {
            return Err(InvalidRoster);
        }
        if matches!(&buffer.storage, Gfx942SdmaBufferStorageV1::Device(_)) {
            devices += 1;
            if devices > MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1 {
                return Err(InvalidRoster);
            }
            continue;
        }
        let (identity, bytes) = project(owner, buffer, backing_bytes).ok_or(InvalidRoster)?;
        if identities
            .iter()
            .any(|other| identity.same_retained_allocation_v1(*other))
        {
            return Err(InvalidRoster);
        }
        identities.try_push(identity).map_err(|_| InvalidRoster)?;
        usage.cached_backing_bytes = usage
            .cached_backing_bytes
            .checked_add(bytes)
            .ok_or(InvalidRoster)?;
        usage.cached_buffers = identities.len();
        if usage.cached_backing_bytes > limits.max_cached_backing_bytes
            || usage.cached_buffers > limits.max_cached_buffers
        {
            return Err(InvalidRoster);
        }
    }
    Ok((identities, usage))
}

// Tests supply the same exact record validator on a fake native engine.
pub(crate) fn host_pool_usage_with_v1(
    owner: QueueKeyV1,
    limits: Gfx942HostPoolLimitsV1,
    cached: &[Gfx942SdmaBufferV1],
    backing_bytes: &mut impl FnMut(&MappedHostBufferV1) -> Result<u64, MemorySessionError>,
) -> Result<Gfx942HostPoolUsageV1, HostPoolPolicyErrorV1> {
    scan(owner, limits, cached, backing_bytes).map(|(_, usage)| usage)
}

pub(crate) fn host_pool_recycle_decision_with_v1(
    owner: QueueKeyV1,
    limits: Gfx942HostPoolLimitsV1,
    cached: &[Gfx942SdmaBufferV1],
    candidate: &Gfx942SdmaBufferV1,
    backing_bytes: &mut impl FnMut(&MappedHostBufferV1) -> Result<u64, MemorySessionError>,
) -> Result<HostPoolDispositionV1, HostPoolPolicyErrorV1> {
    use HostPoolPolicyErrorV1::InvalidCandidate;
    let (identities, usage) = scan(owner, limits, cached, backing_bytes)?;
    let (identity, bytes) = project(owner, candidate, backing_bytes).ok_or(InvalidCandidate)?;
    if identities
        .iter()
        .any(|other| identity.same_retained_allocation_v1(*other))
    {
        return Err(InvalidCandidate);
    }
    let next_bytes = usage
        .cached_backing_bytes
        .checked_add(bytes)
        .ok_or(InvalidCandidate)?;
    Ok(
        if next_bytes <= limits.max_cached_backing_bytes
            && usage.cached_buffers < limits.max_cached_buffers
        {
            HostPoolDispositionV1::Cache
        } else {
            HostPoolDispositionV1::Dispose
        },
    )
}
