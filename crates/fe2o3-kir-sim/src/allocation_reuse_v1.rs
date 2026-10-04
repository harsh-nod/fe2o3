//! Exact-shape, two-cell backing-buffer reuse. This is a CPU simulator policy,
//! not a physical GPU allocator, pointer address, or portable slot identity.
use super::*;
use crate::{
    SimulationAllocationDescriptorV1, SimulationAllocationObservationUnavailableV1,
    SimulationAllocationScopeV1, SimulationAllocationStorageIdentityV1,
    SimulationAllocationTransitionKindV1, SimulationAllocationTransitionV1,
    SimulationAllocationWatermarkV1,
};

pub const MAX_ALLOCATION_REUSE_CACHED_PAYLOAD_BYTES_V1: usize = 256 * 1024 * 1024;

/// Explicitly enables exact Private/Workgroup buffer reuse for one execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationAllocationReuseV1 {
    max_cached_payload_bytes: usize,
}

impl SimulationAllocationReuseV1 {
    /// Zero permits identities but retains no nonempty backing allocations.
    pub fn exact_private_and_workgroup(
        max_cached_payload_bytes: usize,
    ) -> Result<Self, SimulationAllocationReuseErrorV1> {
        if max_cached_payload_bytes > MAX_ALLOCATION_REUSE_CACHED_PAYLOAD_BYTES_V1 {
            return Err(SimulationAllocationReuseErrorV1 {
                actual: max_cached_payload_bytes,
            });
        }
        Ok(Self {
            max_cached_payload_bytes,
        })
    }

    pub const fn max_cached_payload_bytes(self) -> usize {
        self.max_cached_payload_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationAllocationReuseErrorV1 {
    pub actual: usize,
}

impl fmt::Display for SimulationAllocationReuseErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "allocation reuse cache is {} bytes, maximum {}",
            self.actual, MAX_ALLOCATION_REUSE_CACHED_PAYLOAD_BYTES_V1
        )
    }
}
impl Error for SimulationAllocationReuseErrorV1 {}

pub(super) struct AllocationReusePool {
    private: Option<Allocation>,
    workgroup: Option<Allocation>,
    next_storage_slot: u64,
    cached_payload_bytes: usize,
    policy: SimulationAllocationReuseV1,
    sequence: u64,
    unavailable: Option<SimulationAllocationObservationUnavailableV1>,
}

impl AllocationReusePool {
    pub(super) fn new(policy: SimulationAllocationReuseV1) -> Self {
        Self {
            private: None,
            workgroup: None,
            next_storage_slot: 1,
            cached_payload_bytes: 0,
            policy,
            sequence: 0,
            unavailable: None,
        }
    }

    fn cell(&self, address_space: AddressSpace) -> Option<&Option<Allocation>> {
        match address_space {
            AddressSpace::Private => Some(&self.private),
            AddressSpace::Workgroup => Some(&self.workgroup),
            _ => None,
        }
    }

    fn cell_mut(&mut self, address_space: AddressSpace) -> Option<&mut Option<Allocation>> {
        match address_space {
            AddressSpace::Private => Some(&mut self.private),
            AddressSpace::Workgroup => Some(&mut self.workgroup),
            _ => None,
        }
    }

    fn transition(
        &mut self,
        descriptor: SimulationAllocationDescriptorV1,
        kind: SimulationAllocationTransitionKindV1,
    ) -> Option<SimulationAllocationTransitionV1> {
        if self.unavailable.is_some() {
            return None;
        }
        let Some(sequence) = self.sequence.checked_add(1) else {
            self.unavailable = Some(SimulationAllocationObservationUnavailableV1::SequenceOverflow);
            return None;
        };
        self.sequence = sequence;
        Some(SimulationAllocationTransitionV1::new(
            sequence, descriptor, kind,
        ))
    }

    pub(super) fn watermark(&self) -> SimulationAllocationWatermarkV1 {
        match self.unavailable {
            Some(reason) => SimulationAllocationWatermarkV1::Unavailable { reason },
            None => SimulationAllocationWatermarkV1::Available {
                through_sequence: self.sequence,
            },
        }
    }
}

impl Allocation {
    fn retained_payload_capacity_bytes(&self) -> Option<usize> {
        self.bytes
            .capacity()
            .checked_mul(size_of::<u8>())?
            .checked_add(self.initialized.capacity().checked_mul(size_of::<bool>())?)?
            .checked_add(
                self.workgroup_published
                    .capacity()
                    .checked_mul(size_of::<bool>())?,
            )?
            .checked_add(
                self.workgroup_writer
                    .capacity()
                    .checked_mul(size_of::<u64>())?,
            )
    }

    fn reusable_for(&self, shape: (AddressSpace, AccessMode, u32), bytes: usize) -> bool {
        self.address_space == shape.0
            && self.access == shape.1
            && self.alignment == shape.2
            && self.bytes.len() == bytes
            && self.initialized.len() == bytes
            && self.bytes.capacity() >= bytes
            && self.initialized.capacity() >= bytes
            && if shape.0 == AddressSpace::Workgroup {
                self.workgroup_published.len() == bytes
                    && self.workgroup_writer.len() == bytes
                    && self.workgroup_published.capacity() >= bytes
                    && self.workgroup_writer.capacity() >= bytes
            } else {
                self.workgroup_published.is_empty() && self.workgroup_writer.is_empty()
            }
    }
}

pub(super) struct AllocationCommit {
    pub(super) id: u64,
    pub(super) transition: Option<SimulationAllocationTransitionV1>,
}

pub(super) struct ReleaseCommit {
    pub(super) removed: bool,
    pub(super) transition: Option<SimulationAllocationTransitionV1>,
}

/// Immutable facts supplied by the actual allocation callsite.
pub(super) struct AllocationCreationV1 {
    pub(super) scope: SimulationAllocationScopeV1,
    pub(super) site: Option<SimulationEventSiteV1>,
}

impl AllocationCreationV1 {
    pub(super) fn dispatch() -> Self {
        Self {
            scope: SimulationAllocationScopeV1::Dispatch,
            site: None,
        }
    }
}

enum StoragePlan {
    Disabled,
    Fresh {
        identity: SimulationAllocationStorageIdentityV1,
        next_storage_slot: u64,
        retire_cached_bytes: Option<usize>,
    },
    Reuse {
        identity: SimulationAllocationStorageIdentityV1,
        previous_allocation: u64,
        remaining_cached_bytes: usize,
    },
}

impl Memory {
    fn plan_storage(
        &self,
        shape: (AddressSpace, AccessMode, u32),
        bytes: usize,
        id: u64,
    ) -> Result<StoragePlan, SimulationExecutionErrorKindV1> {
        let Some(pool) = &self.reuse else {
            return Ok(StoragePlan::Disabled);
        };
        let mut retire_cached_bytes = None;
        if let Some(cached) = pool.cell(shape.0).and_then(Option::as_ref) {
            let descriptor = cached.observation_descriptor.first().copied().ok_or(
                SimulationExecutionErrorKindV1::InternalInvariant("cached allocation descriptor"),
            )?;
            let identity = descriptor.identity();
            if cached.reusable_for(shape, bytes) {
                let capacity = cached.retained_payload_capacity_bytes().ok_or(
                    SimulationExecutionErrorKindV1::InternalInvariant("cached payload capacity"),
                )?;
                let remaining = pool.cached_payload_bytes.checked_sub(capacity).ok_or(
                    SimulationExecutionErrorKindV1::InternalInvariant("cached payload accounting"),
                )?;
                if let Some(generation) = identity.generation().checked_add(1) {
                    return Ok(StoragePlan::Reuse {
                        identity: SimulationAllocationStorageIdentityV1::new(
                            id,
                            identity.storage_slot(),
                            generation,
                        ),
                        previous_allocation: identity.allocation(),
                        remaining_cached_bytes: remaining,
                    });
                }
                // Exhausted generations are retired only once fresh creation is prepared.
                retire_cached_bytes = Some(remaining);
            }
        }
        let next_storage_slot = pool.next_storage_slot.checked_add(1).ok_or(
            SimulationExecutionErrorKindV1::InternalInvariant("storage slot identity exhausted"),
        )?;
        if pool.next_storage_slot == 0 {
            return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                "zero storage slot",
            ));
        }
        Ok(StoragePlan::Fresh {
            identity: SimulationAllocationStorageIdentityV1::new(id, pool.next_storage_slot, 1),
            next_storage_slot,
            retire_cached_bytes,
        })
    }

    pub(super) fn allocate(
        &mut self,
        shape: (AddressSpace, AccessMode, u32),
        contents: (Vec<u8>, Vec<bool>),
        creation: AllocationCreationV1,
        limits: SimulationLimitsV1,
    ) -> Result<AllocationCommit, SimulationExecutionErrorKindV1> {
        self.allocate_with_descriptor_reservation(shape, contents, creation, limits, |descriptor| {
            descriptor
                .try_reserve_exact(1)
                .map_err(|_| SimulationExecutionErrorKindV1::AllocationFailure)
        })
    }

    // A private reservation seam permits a deterministic pre-commit failure
    // test; production always uses the fallible exact reservation above.
    fn allocate_with_descriptor_reservation(
        &mut self,
        shape: (AddressSpace, AccessMode, u32),
        contents: (Vec<u8>, Vec<bool>),
        creation: AllocationCreationV1,
        limits: SimulationLimitsV1,
        reserve_descriptor: impl FnOnce(
            &mut Vec<SimulationAllocationDescriptorV1>,
        ) -> Result<(), SimulationExecutionErrorKindV1>,
    ) -> Result<AllocationCommit, SimulationExecutionErrorKindV1> {
        let (bytes, initialized) = contents;
        if bytes.len() != initialized.len() {
            return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                "allocation initialization length",
            ));
        }
        self.validate_allocation(bytes.len(), limits)?;
        let byte_len = u64::try_from(bytes.len()).map_err(|_| {
            SimulationExecutionErrorKindV1::InternalInvariant("allocation descriptor length")
        })?;
        let next_allocation = self.next_allocation.checked_add(1).ok_or(
            SimulationExecutionErrorKindV1::AllocationLimit {
                limit: limits.max_allocations,
            },
        )?;
        if self.next_allocation == 0 {
            return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                "zero allocation ID",
            ));
        }
        let allocations_created = self.allocations_created.checked_add(1).ok_or(
            SimulationExecutionErrorKindV1::AllocationLimit {
                limit: limits.max_allocations,
            },
        )?;
        let live_bytes = self.live_bytes.checked_add(bytes.len()).ok_or(
            SimulationExecutionErrorKindV1::TotalBytesLimit {
                actual: usize::MAX,
                limit: limits.max_total_bytes,
            },
        )?;
        let id = self.next_allocation;
        let storage = self.plan_storage(shape, bytes.len(), id)?;
        if self.allocations.contains_key(&id) {
            return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                "fresh allocation ID",
            ));
        }
        if self.allocations.len() == self.allocations.capacity() {
            self.allocations
                .try_reserve(1)
                .map_err(|_| SimulationExecutionErrorKindV1::AllocationFailure)?;
        }

        // Decide reuse before constructing workgroup metadata: a hit moves all four
        // old vectors, never an identity onto fresh backing storage.
        let fresh = if matches!(storage, StoragePlan::Reuse { .. }) {
            None
        } else {
            let (published, writer) = if shape.0 == AddressSpace::Workgroup {
                (
                    try_filled(bytes.len(), false)?,
                    try_filled(bytes.len(), 0_u64)?,
                )
            } else {
                (Vec::new(), Vec::new())
            };
            Some((published, writer))
        };
        // Reserve observation metadata before changing a cache cell, slot counter,
        // or semantic allocation counter. The default profile allocates none.
        // A reuse hit moves this one-element vector with the actual backing.
        let mut fresh_descriptor = Vec::new();
        if let StoragePlan::Fresh { identity, .. } = &storage {
            reserve_descriptor(&mut fresh_descriptor)?;
            fresh_descriptor.push(SimulationAllocationDescriptorV1::new(
                *identity,
                (shape.0, shape.1, shape.2, byte_len),
                creation.scope,
                creation.site,
            ));
        }
        let (allocation, previous_allocation) = match storage {
            StoragePlan::Reuse {
                identity,
                previous_allocation,
                remaining_cached_bytes,
            } => {
                let pool = self.reuse.as_mut().ok_or(
                    SimulationExecutionErrorKindV1::InternalInvariant("reuse pool disappeared"),
                )?;
                let cached = pool.cell_mut(shape.0).and_then(Option::take).ok_or(
                    SimulationExecutionErrorKindV1::InternalInvariant("reuse cell disappeared"),
                )?;
                let mut allocation = cached;
                allocation.bytes.copy_from_slice(&bytes);
                allocation.initialized.copy_from_slice(&initialized);
                allocation.workgroup_published.fill(false);
                allocation.workgroup_writer.fill(0);
                // plan_storage already checked this cached descriptor before the
                // allocation was removed; replacing it needs no allocation.
                allocation.observation_descriptor[0] = SimulationAllocationDescriptorV1::new(
                    identity,
                    (shape.0, shape.1, shape.2, byte_len),
                    creation.scope,
                    creation.site,
                );
                pool.cached_payload_bytes = remaining_cached_bytes;
                (allocation, Some(previous_allocation))
            }
            StoragePlan::Fresh {
                identity: _,
                next_storage_slot,
                retire_cached_bytes,
            } => {
                let (published, writer) = fresh.ok_or(
                    SimulationExecutionErrorKindV1::InternalInvariant("fresh allocation metadata"),
                )?;
                let pool = self.reuse.as_mut().ok_or(
                    SimulationExecutionErrorKindV1::InternalInvariant("fresh pool disappeared"),
                )?;
                if let Some(remaining) = retire_cached_bytes {
                    // The exact cell was checked before mutation; taking is allocation-free.
                    if let Some(cell) = pool.cell_mut(shape.0) {
                        *cell = None;
                    }
                    pool.cached_payload_bytes = remaining;
                }
                pool.next_storage_slot = next_storage_slot;
                (
                    Allocation {
                        address_space: shape.0,
                        access: shape.1,
                        alignment: shape.2,
                        bytes,
                        initialized,
                        workgroup_published: published,
                        workgroup_writer: writer,
                        observation_descriptor: fresh_descriptor,
                    },
                    None,
                )
            }
            StoragePlan::Disabled => {
                let (published, writer) = fresh.ok_or(
                    SimulationExecutionErrorKindV1::InternalInvariant("legacy allocation metadata"),
                )?;
                (
                    Allocation {
                        address_space: shape.0,
                        access: shape.1,
                        alignment: shape.2,
                        bytes,
                        initialized,
                        workgroup_published: published,
                        workgroup_writer: writer,
                        observation_descriptor: Vec::new(),
                    },
                    None,
                )
            }
        };
        let descriptor = allocation.observation_descriptor.first().copied();
        self.allocations.insert(id, allocation);
        self.next_allocation = next_allocation;
        self.allocations_created = allocations_created;
        self.live_bytes = live_bytes;
        let transition = self.reuse.as_mut().and_then(|pool| {
            descriptor.and_then(|descriptor| {
                let kind = if matches!(creation.scope, SimulationAllocationScopeV1::Dispatch) {
                    SimulationAllocationTransitionKindV1::Preexisting
                } else {
                    SimulationAllocationTransitionKindV1::Create {
                        previous_allocation,
                    }
                };
                pool.transition(descriptor, kind)
            })
        });
        Ok(AllocationCommit { id, transition })
    }

    pub(super) fn release_one(
        &mut self,
        id: u64,
    ) -> Result<ReleaseCommit, SimulationExecutionErrorKindV1> {
        let Some(allocation) = self.allocations.get(&id) else {
            return Ok(ReleaseCommit {
                removed: false,
                transition: None,
            });
        };
        // Validate all arithmetic while the active semantic allocation is intact.
        let live_bytes = self.live_bytes.checked_sub(allocation.bytes.len()).ok_or(
            SimulationExecutionErrorKindV1::InternalInvariant(
                "released allocation live-byte accounting",
            ),
        )?;
        let descriptor = allocation.observation_descriptor.first().copied();
        let cache_bytes = if let Some(pool) = &self.reuse {
            let payload = allocation.retained_payload_capacity_bytes().ok_or(
                SimulationExecutionErrorKindV1::InternalInvariant("released payload capacity"),
            )?;
            if let Some(cell) = pool.cell(allocation.address_space) {
                let old_bytes = match cell {
                    Some(old) => old.retained_payload_capacity_bytes().ok_or(
                        SimulationExecutionErrorKindV1::InternalInvariant(
                            "retired payload capacity",
                        ),
                    )?,
                    None => 0,
                };
                let new_bytes = pool
                    .cached_payload_bytes
                    .checked_sub(old_bytes)
                    .and_then(|bytes| bytes.checked_add(payload))
                    .ok_or(SimulationExecutionErrorKindV1::InternalInvariant(
                        "release cache accounting",
                    ))?;
                (new_bytes <= pool.policy.max_cached_payload_bytes()).then_some(new_bytes)
            } else {
                None
            }
        } else {
            None
        };
        let allocation = self.allocations.remove(&id).ok_or(
            SimulationExecutionErrorKindV1::InternalInvariant("released allocation disappeared"),
        )?;
        self.live_bytes = live_bytes;
        let transition = if let Some(pool) = &mut self.reuse {
            if let Some(cache_bytes) = cache_bytes {
                if let Some(cell) = pool.cell_mut(allocation.address_space) {
                    *cell = Some(allocation);
                }
                pool.cached_payload_bytes = cache_bytes;
            }
            match descriptor {
                Some(descriptor) => {
                    pool.transition(descriptor, SimulationAllocationTransitionKindV1::Release)
                }
                None => {
                    pool.unavailable =
                        Some(SimulationAllocationObservationUnavailableV1::IdentityInvariant);
                    None
                }
            }
        } else {
            None
        };
        Ok(ReleaseCommit {
            removed: true,
            transition,
        })
    }
}

impl<S: SimulationEventSinkV1> Engine<'_, S> {
    pub(super) fn deliver_allocation_lifecycle_v1(
        &mut self,
        transition: Option<SimulationAllocationTransitionV1>,
    ) {
        if !self.allocation_lifecycle_requested || self.allocation_lifecycle_stopped {
            return;
        }
        if let Some(transition) = transition {
            match self.debug_sink.allocation_lifecycle_v1(transition) {
                SimulationDebugSinkControlV1::Continue => {}
                SimulationDebugSinkControlV1::Stop | SimulationDebugSinkControlV1::DropAndStop => {
                    self.allocation_lifecycle_stopped = true;
                }
            }
        }
    }

    pub(super) fn allocation_lifecycle_watermark_v1(&self) -> SimulationAllocationWatermarkV1 {
        if !self.allocation_lifecycle_requested {
            SimulationAllocationWatermarkV1::NotRequested
        } else if self.reuse_disabled() {
            SimulationAllocationWatermarkV1::PolicyDisabled
        } else if self.allocation_lifecycle_stopped {
            SimulationAllocationWatermarkV1::Unavailable {
                reason: SimulationAllocationObservationUnavailableV1::ObservationStopped,
            }
        } else {
            self.memory.reuse.as_ref().map_or(
                SimulationAllocationWatermarkV1::Unavailable {
                    reason: SimulationAllocationObservationUnavailableV1::IdentityInvariant,
                },
                AllocationReusePool::watermark,
            )
        }
    }

    fn reuse_disabled(&self) -> bool {
        self.memory.reuse.is_none()
    }

    pub(super) fn allocation_creation_v1(
        &self,
        site: CompactSite,
        address_space: AddressSpace,
    ) -> Result<AllocationCreationV1, SimulationExecutionErrorV1> {
        let invocation = self.invocation.ok_or_else(|| {
            self.at(
                site,
                SimulationExecutionErrorKindV1::InternalInvariant("allocation creation invocation"),
            )
        })?;
        let scope = if address_space == AddressSpace::Workgroup {
            SimulationAllocationScopeV1::Workgroup {
                coordinate: invocation.workgroup,
                size: invocation.workgroup_size,
                count: invocation.workgroup_count,
                launch: invocation.launch_extent,
            }
        } else {
            SimulationAllocationScopeV1::Invocation(invocation)
        };
        Ok(AllocationCreationV1 {
            scope,
            site: Some(self.materialize_event_site(site)),
        })
    }
}

#[cfg(test)]
#[path = "allocation_reuse_v1_tests.rs"]
mod tests;
