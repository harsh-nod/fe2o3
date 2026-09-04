//! Bounded model of one persistent native allocation and its use leases.
//!
//! Admission consumes caller-constructible R2 memory records. Individual XGMI
//! route-metadata classifications validate an R9 route token, but do not bind
//! that route to this registry mapping or grant mapping/publication authority.
//! The model performs no allocation, mapping, queue operation, thread lookup,
//! or native access. In particular, it does not refine Rust auto-traits, OS
//! thread affinity, KFD, SDMA, VM currentness, or GPU completion.
//!
//! The registry represents exactly one allocation of at most 256 MiB mapped to
//! exactly two devices. It deliberately does not model a two-registry atomic
//! XGMI join or a 1 GiB aggregate allocation boundary.

// Transition failures deliberately return move-only custody without boxing.
#![allow(clippy::result_large_err)]

use alloc::{rc::Rc, vec::Vec};
use core::marker::PhantomData;

use crate::*;

pub const R17_PERSISTENT_NATIVE_ALLOCATION_SCHEMA_VERSION_V1: u16 = 1;
pub const R17_PERSISTENT_NATIVE_ALLOCATION_BYTES_V1: u64 = 256 * 1024 * 1024;
pub const R17_PERSISTENT_NATIVE_DEVICE_COUNT_V1: usize = 2;
pub const MAX_R17_PERSISTENT_USE_LEASES_V1: usize = 64;
pub const MAX_R17_PERSISTENT_DEPENDENCIES_V1: usize = 256;
pub const R17_GFX942_LOCAL_SDMA_ENGINE_COUNT_V1: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct R17PersistentAllocationOwnerIdV1(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// Non-authoritative observable coordinates; not sufficient for a transition.
/// Equal values may recur in separately reconstructed registries.
pub struct R17PersistentUseLeaseKeyV1 {
    pub owner: R17PersistentAllocationOwnerIdV1,
    pub slot: u8,
    pub generation: u64,
}

/// Allocation-relative half-open byte range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R17PersistentUseRangeV1 {
    pub byte_offset: u64,
    pub byte_len: u64,
}

impl R17PersistentUseRangeV1 {
    pub const fn checked_end(self) -> Option<u64> {
        self.byte_offset.checked_add(self.byte_len)
    }

    pub const fn overlaps(self, other: Self) -> bool {
        match (self.checked_end(), other.checked_end()) {
            (Some(self_end), Some(other_end)) => {
                self.byte_offset < other_end && other.byte_offset < self_end
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R17PersistentAccessModeV1 {
    Read,
    Write,
}

/// Classification for one modeled use of the persistent allocation.
///
/// `XgmiRouteMetadata` validates directional route metadata only. It is not
/// evidence that the route names this R2 mapping or that any packet published.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R17PersistentUseClassV1 {
    Compute {
        device: DeviceKeyV1,
        queue: QueueKeyV1,
    },
    LocalSdma {
        device: DeviceKeyV1,
        queue: QueueKeyV1,
        engine_id: u32,
    },
    XgmiRouteMetadata {
        source_device: DeviceKeyV1,
        destination_device: DeviceKeyV1,
        engine_id: u32,
        route: ModelNativeXgmiRouteV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R17PersistentUseDescriptorV1 {
    pub class: R17PersistentUseClassV1,
    pub access: R17PersistentAccessModeV1,
    pub range: R17PersistentUseRangeV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R17PersistentUseBindingV1 {
    pub allocation: MemoryAllocationKeyV1,
    pub mapping: MemoryMappingKeyV1,
    pub lease: R17PersistentUseLeaseKeyV1,
    pub descriptor: R17PersistentUseDescriptorV1,
}

/// Cloneable dependency witness for one exact registry incarnation and use.
/// It is intentionally not `Copy`; numeric keys and receipts are
/// non-authoritative observations and may coincide across reconstructed
/// registries. Only witnesses and state-changing tokens carry the private
/// incarnation checked by transitions.
#[derive(Clone, Debug)]
pub struct R17PersistentUseDependencyV1 {
    binding: R17PersistentUseBindingV1,
    registry_incarnation: Rc<()>,
}

impl R17PersistentUseDependencyV1 {
    pub const fn binding(&self) -> R17PersistentUseBindingV1 {
        self.binding
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R17PersistentTerminalStatusV1 {
    Succeeded,
    Failed { code: i32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R17PersistentQuarantineReasonV1 {
    NativeResultIndeterminate,
    CompletionObservationUnavailable,
    DeviceCurrentnessLost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R17PersistentUsePhaseV1 {
    Reserved,
    Published,
    TimedOut,
    Terminal,
    CancelledBeforePublication,
    Quarantined,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R17PersistentUseObservationV1 {
    Pending,
    TimedOut,
    Terminal(R17PersistentTerminalStatusV1),
    Indeterminate(R17PersistentQuarantineReasonV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R17PersistentAllocationErrorV1 {
    InvalidOwner,
    InvalidAllocation,
    InvalidMapping,
    InvalidDeviceSet,
    InvalidRange,
    InvalidClassBinding,
    CapacityExceeded,
    DuplicateDependency,
    UnknownDependency,
    DependencyNotReady,
    ConflictingUse,
    WrongOwner,
    StaleLease,
    IllegalState,
    NotCurrent,
    DependentRetained,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R17PersistentUseRecordObservationV1 {
    pub binding: R17PersistentUseBindingV1,
    pub phase: R17PersistentUsePhaseV1,
    pub dependency_count: usize,
    pub terminal_status: Option<R17PersistentTerminalStatusV1>,
    pub quarantine_reason: Option<R17PersistentQuarantineReasonV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R17PersistentRegistrySnapshotV1 {
    pub current: bool,
    pub lease_count: usize,
    pub reserved_count: usize,
    pub published_count: usize,
    pub timed_out_count: usize,
    pub terminal_count: usize,
    pub quarantined_count: usize,
    pub released_count: usize,
    pub next_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R17PersistentCurrentnessLossV1 {
    pub cancelled_reservations: usize,
    pub quarantined_uses: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R17PersistentReleaseOutcomeV1 {
    CancelledBeforePublication,
    ReleasedAfterTerminal(R17PersistentTerminalStatusV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Non-authoritative record of a model transition.
pub struct R17PersistentUseReleaseReceiptV1 {
    binding: R17PersistentUseBindingV1,
    outcome: R17PersistentReleaseOutcomeV1,
}

impl R17PersistentUseReleaseReceiptV1 {
    pub const fn binding(self) -> R17PersistentUseBindingV1 {
        self.binding
    }

    pub const fn outcome(self) -> R17PersistentReleaseOutcomeV1 {
        self.outcome
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Non-authoritative record of modeled owner release.
pub struct R17PersistentAllocationReleaseReceiptV1 {
    pub owner: R17PersistentAllocationOwnerIdV1,
    pub allocation: MemoryAllocationKeyV1,
    pub mapping: MemoryMappingKeyV1,
    pub completed_lease_count: usize,
}

#[derive(Debug)]
pub struct R17PersistentLeaseTransitionFailureV1<T> {
    error: R17PersistentAllocationErrorV1,
    retained: T,
}

impl<T> R17PersistentLeaseTransitionFailureV1<T> {
    pub const fn error(&self) -> R17PersistentAllocationErrorV1 {
        self.error
    }

    pub fn into_parts(self) -> (R17PersistentAllocationErrorV1, T) {
        (self.error, self.retained)
    }
}

struct R17PersistentUseRecordV1 {
    binding: R17PersistentUseBindingV1,
    dependencies: Vec<R17PersistentUseLeaseKeyV1>,
    phase: R17PersistentUsePhaseV1,
    terminal_status: Option<R17PersistentTerminalStatusV1>,
    quarantine_reason: Option<R17PersistentQuarantineReasonV1>,
}

/// Sole model owner of one persistent allocation registry.
///
/// This type is intentionally neither `Clone` nor `Send`. The `Rc` marker is a
/// Rust type-system check only; Verus does not prove Rust auto-traits or that a
/// production registry remains on its creating OS thread.
///
/// ```compile_fail
/// use fe2o3_runtime_model::R17PersistentNativeAllocationRegistryV1;
/// fn cannot_clone(registry: R17PersistentNativeAllocationRegistryV1) {
///     let _duplicate = registry.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_runtime_model::R17PersistentNativeAllocationRegistryV1;
/// fn requires_send<T: Send>() {}
/// fn registry_is_thread_affine() {
///     requires_send::<R17PersistentNativeAllocationRegistryV1>();
/// }
/// ```
pub struct R17PersistentNativeAllocationRegistryV1 {
    owner: R17PersistentAllocationOwnerIdV1,
    allocation: MemoryAllocationKeyV1,
    mapping: MemoryMappingKeyV1,
    devices: [DeviceKeyV1; R17_PERSISTENT_NATIVE_DEVICE_COUNT_V1],
    byte_len: u64,
    current: bool,
    next_generation: u64,
    completed_lease_count: usize,
    records: [Option<R17PersistentUseRecordV1>; MAX_R17_PERSISTENT_USE_LEASES_V1],
    registry_incarnation: Rc<()>,
    thread_affine: PhantomData<Rc<()>>,
}

impl R17PersistentNativeAllocationRegistryV1 {
    pub fn new_model_only(
        owner: R17PersistentAllocationOwnerIdV1,
        allocation: MemoryAllocationRecordV1,
        mapping: MemoryMappingRecordV1,
        devices: [DeviceKeyV1; R17_PERSISTENT_NATIVE_DEVICE_COUNT_V1],
    ) -> Result<Self, R17PersistentAllocationErrorV1> {
        if owner.0 == 0 {
            return Err(R17PersistentAllocationErrorV1::InvalidOwner);
        }
        if allocation.state != MemoryAllocationStateV1::Live
            || allocation.spec.byte_len == 0
            || allocation.spec.byte_len > R17_PERSISTENT_NATIVE_ALLOCATION_BYTES_V1
            || allocation.spec.kind != MemoryKindV1::DeviceLocal
            || allocation.spec.coherence != MemoryCoherenceV1::ExplicitVisibility
            || !allocation
                .spec
                .byte_len
                .is_multiple_of(MEMORY_PAGE_BYTES_V1)
            || allocation.spec.alignment < MEMORY_PAGE_BYTES_V1
            || !allocation.spec.alignment.is_power_of_two()
            || allocation.key.vm.id.0 == 0
            || allocation.key.id.0 == 0
            || allocation.key.generation.0 == 0
            || allocation.reservation.vm != allocation.key.vm
            || allocation.reservation.id.0 == 0
            || allocation.handle.0 == 0
        {
            return Err(R17PersistentAllocationErrorV1::InvalidAllocation);
        }
        if devices[0] >= devices[1]
            || devices[0].physical == devices[1].physical
            || devices.iter().any(|device| device.generation.0 == 0)
            || !devices.contains(&allocation.key.vm.device)
        {
            return Err(R17PersistentAllocationErrorV1::InvalidDeviceSet);
        }
        if mapping.key.allocation != allocation.key
            || mapping.state != MemoryMappingStateV1::Mapped
            || mapping.key.id.0 == 0
            || mapping.access != MemoryAccessV1::ReadWrite
            || mapping.mapped_start != 0
            || mapping.mapped_end != R17_PERSISTENT_NATIVE_DEVICE_COUNT_V1
            || mapping.target_devices.as_slice() != devices
        {
            return Err(R17PersistentAllocationErrorV1::InvalidMapping);
        }
        let registry = Self {
            owner,
            allocation: allocation.key,
            mapping: mapping.key,
            devices,
            byte_len: allocation.spec.byte_len,
            current: true,
            next_generation: 1,
            completed_lease_count: 0,
            records: [const { None }; MAX_R17_PERSISTENT_USE_LEASES_V1],
            registry_incarnation: Rc::new(()),
            thread_affine: PhantomData,
        };
        registry.validate_global_invariants()?;
        Ok(registry)
    }

    pub const fn owner(&self) -> R17PersistentAllocationOwnerIdV1 {
        self.owner
    }

    pub const fn allocation(&self) -> MemoryAllocationKeyV1 {
        self.allocation
    }

    pub const fn mapping(&self) -> MemoryMappingKeyV1 {
        self.mapping
    }

    pub const fn devices(&self) -> [DeviceKeyV1; R17_PERSISTENT_NATIVE_DEVICE_COUNT_V1] {
        self.devices
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    pub const fn is_current(&self) -> bool {
        self.current
    }

    pub fn snapshot(&self) -> R17PersistentRegistrySnapshotV1 {
        let mut snapshot = R17PersistentRegistrySnapshotV1 {
            current: self.current,
            lease_count: self.records.iter().flatten().count(),
            reserved_count: 0,
            published_count: 0,
            timed_out_count: 0,
            terminal_count: 0,
            quarantined_count: 0,
            released_count: self.completed_lease_count,
            next_generation: self.next_generation,
        };
        for record in self.records.iter().flatten() {
            match record.phase {
                R17PersistentUsePhaseV1::Reserved => snapshot.reserved_count += 1,
                R17PersistentUsePhaseV1::Published => snapshot.published_count += 1,
                R17PersistentUsePhaseV1::TimedOut => snapshot.timed_out_count += 1,
                R17PersistentUsePhaseV1::Terminal => snapshot.terminal_count += 1,
                R17PersistentUsePhaseV1::Quarantined => snapshot.quarantined_count += 1,
                R17PersistentUsePhaseV1::CancelledBeforePublication
                | R17PersistentUsePhaseV1::Released => snapshot.released_count += 1,
            }
        }
        snapshot
    }

    pub fn record(
        &self,
        key: R17PersistentUseLeaseKeyV1,
    ) -> Option<R17PersistentUseRecordObservationV1> {
        self.record_index(key)
            .and_then(|index| self.records[index].as_ref())
            .map(|record| R17PersistentUseRecordObservationV1 {
                binding: record.binding,
                phase: record.phase,
                dependency_count: record.dependencies.len(),
                terminal_status: record.terminal_status,
                quarantine_reason: record.quarantine_reason,
            })
    }

    pub fn reserve_model_only(
        &mut self,
        descriptor: R17PersistentUseDescriptorV1,
        dependencies: Vec<R17PersistentUseDependencyV1>,
    ) -> Result<R17ReservedPersistentUseLeaseV1, R17PersistentAllocationErrorV1> {
        self.require_current()?;
        let Some(slot) = self.records.iter().position(Option::is_none) else {
            return Err(R17PersistentAllocationErrorV1::CapacityExceeded);
        };
        self.validate_descriptor(descriptor)?;
        if dependencies.len() > MAX_R17_PERSISTENT_DEPENDENCIES_V1 {
            return Err(R17PersistentAllocationErrorV1::CapacityExceeded);
        }
        for (index, dependency) in dependencies.iter().enumerate() {
            if !Rc::ptr_eq(&dependency.registry_incarnation, &self.registry_incarnation)
                || dependency.binding.allocation != self.allocation
                || dependency.binding.mapping != self.mapping
                || dependency.binding.lease.owner != self.owner
            {
                return Err(R17PersistentAllocationErrorV1::WrongOwner);
            }
            if dependencies[..index]
                .iter()
                .any(|prior| prior.binding.lease == dependency.binding.lease)
            {
                return Err(R17PersistentAllocationErrorV1::DuplicateDependency);
            }
            if self.record_index(dependency.binding.lease).is_none() {
                return Err(R17PersistentAllocationErrorV1::UnknownDependency);
            }
        }
        let next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(R17PersistentAllocationErrorV1::CapacityExceeded)?;
        let lease = R17PersistentUseLeaseKeyV1 {
            owner: self.owner,
            slot: u8::try_from(slot)
                .map_err(|_| R17PersistentAllocationErrorV1::CapacityExceeded)?,
            generation: self.next_generation,
        };
        let binding = R17PersistentUseBindingV1 {
            allocation: self.allocation,
            mapping: self.mapping,
            lease,
            descriptor,
        };
        self.records[slot] = Some(R17PersistentUseRecordV1 {
            binding,
            dependencies: dependencies
                .into_iter()
                .map(|dependency| dependency.binding.lease)
                .collect(),
            phase: R17PersistentUsePhaseV1::Reserved,
            terminal_status: None,
            quarantine_reason: None,
        });
        self.next_generation = next_generation;
        Ok(R17ReservedPersistentUseLeaseV1 {
            binding,
            registry_incarnation: Rc::clone(&self.registry_incarnation),
        })
    }

    pub fn lose_currentness_model_only(
        &mut self,
        reason: R17PersistentQuarantineReasonV1,
    ) -> Result<R17PersistentCurrentnessLossV1, R17PersistentAllocationErrorV1> {
        self.require_current()?;
        let result = self.lose_currentness_inner(reason);
        self.validate_global_invariants()?;
        Ok(result)
    }

    pub fn release_allocation_model_only(
        self,
    ) -> Result<R17PersistentAllocationReleaseReceiptV1, R17PersistentOwnerReleaseFailureV1> {
        let error = if !self.current {
            Some(R17PersistentAllocationErrorV1::Quarantined)
        } else if self.records.iter().any(Option::is_some) {
            Some(R17PersistentAllocationErrorV1::IllegalState)
        } else {
            self.validate_global_invariants().err()
        };
        if let Some(error) = error {
            return Err(R17PersistentOwnerReleaseFailureV1 {
                error,
                retained: self,
            });
        }
        Ok(R17PersistentAllocationReleaseReceiptV1 {
            owner: self.owner,
            allocation: self.allocation,
            mapping: self.mapping,
            completed_lease_count: self.completed_lease_count,
        })
    }

    pub fn validate_global_invariants(&self) -> Result<(), R17PersistentAllocationErrorV1> {
        if self.owner.0 == 0 || self.next_generation == 0 || self.byte_len == 0 {
            return Err(R17PersistentAllocationErrorV1::InvalidOwner);
        }
        if self.mapping.allocation != self.allocation {
            return Err(R17PersistentAllocationErrorV1::InvalidMapping);
        }
        if self.devices[0] == self.devices[1]
            || self.devices[0].physical == self.devices[1].physical
            || !self.devices.contains(&self.allocation.vm.device)
        {
            return Err(R17PersistentAllocationErrorV1::InvalidDeviceSet);
        }
        for (index, record) in self
            .records
            .iter()
            .enumerate()
            .filter_map(|(index, record)| record.as_ref().map(|record| (index, record)))
        {
            if record.binding.allocation != self.allocation
                || record.binding.mapping != self.mapping
                || record.binding.lease.owner != self.owner
                || usize::from(record.binding.lease.slot) != index
                || record.binding.lease.generation == 0
                || record.binding.lease.generation >= self.next_generation
            {
                return Err(R17PersistentAllocationErrorV1::StaleLease);
            }
            self.validate_descriptor(record.binding.descriptor)?;
            if record.dependencies.len() > MAX_R17_PERSISTENT_DEPENDENCIES_V1 {
                return Err(R17PersistentAllocationErrorV1::CapacityExceeded);
            }
            for (dependency_index, dependency) in record.dependencies.iter().enumerate() {
                if dependency.owner != self.owner
                    || record.dependencies[..dependency_index].contains(dependency)
                    || dependency.generation >= record.binding.lease.generation
                {
                    return Err(R17PersistentAllocationErrorV1::UnknownDependency);
                }
            }
            let terminal_ok = match record.phase {
                R17PersistentUsePhaseV1::Terminal | R17PersistentUsePhaseV1::Released => {
                    record.terminal_status.is_some() && record.quarantine_reason.is_none()
                }
                R17PersistentUsePhaseV1::Quarantined => record.quarantine_reason.is_some(),
                R17PersistentUsePhaseV1::Reserved
                | R17PersistentUsePhaseV1::Published
                | R17PersistentUsePhaseV1::TimedOut
                | R17PersistentUsePhaseV1::CancelledBeforePublication => {
                    record.terminal_status.is_none() && record.quarantine_reason.is_none()
                }
            };
            if !terminal_ok {
                return Err(R17PersistentAllocationErrorV1::IllegalState);
            }
            if !self.current
                && matches!(
                    record.phase,
                    R17PersistentUsePhaseV1::Reserved
                        | R17PersistentUsePhaseV1::Published
                        | R17PersistentUsePhaseV1::TimedOut
                        | R17PersistentUsePhaseV1::Terminal
                )
            {
                return Err(R17PersistentAllocationErrorV1::NotCurrent);
            }
        }
        for (index, left) in self.records.iter().enumerate() {
            let Some(left) = left.as_ref() else {
                continue;
            };
            if !retains_exclusive_use_v1(left.phase) {
                continue;
            }
            for right in self.records[index + 1..].iter().flatten() {
                if retained_records_conflict_v1(left, right) {
                    return Err(R17PersistentAllocationErrorV1::ConflictingUse);
                }
            }
        }
        Ok(())
    }

    fn validate_descriptor(
        &self,
        descriptor: R17PersistentUseDescriptorV1,
    ) -> Result<(), R17PersistentAllocationErrorV1> {
        let Some(end) = descriptor.range.checked_end() else {
            return Err(R17PersistentAllocationErrorV1::InvalidRange);
        };
        if descriptor.range.byte_len == 0 || end > self.byte_len {
            return Err(R17PersistentAllocationErrorV1::InvalidRange);
        }
        let valid = match descriptor.class {
            R17PersistentUseClassV1::Compute { device, queue } => {
                device == self.allocation.vm.device
                    && queue.vm == self.allocation.vm
                    && queue.id.0 != 0
                    && queue.generation.0 != 0
            }
            R17PersistentUseClassV1::LocalSdma {
                device,
                queue,
                engine_id,
            } => {
                device == self.allocation.vm.device
                    && queue.vm == self.allocation.vm
                    && queue.id.0 != 0
                    && queue.generation.0 != 0
                    && engine_id < R17_GFX942_LOCAL_SDMA_ENGINE_COUNT_V1
            }
            R17PersistentUseClassV1::XgmiRouteMetadata {
                source_device,
                destination_device,
                engine_id,
                route,
            } => {
                let observation = route.observation();
                route.authority_domain() == AuthorityDomainV1::ModelOnly
                    && route.source_device() == source_device
                    && route.destination_device() == destination_device
                    && source_device != destination_device
                    && self.devices.contains(&source_device)
                    && self.devices.contains(&destination_device)
                    && ((self.allocation.vm.device == source_device
                        && descriptor.access == R17PersistentAccessModeV1::Read)
                        || (self.allocation.vm.device == destination_device
                            && descriptor.access == R17PersistentAccessModeV1::Write))
                    && engine_id == observation.selected_sdma_engine_id
                    && (GFX942_FIRST_XGMI_SDMA_ENGINE_ID_V1..GFX942_SDMA_ENGINE_ID_LIMIT_V1)
                        .contains(&engine_id)
            }
        };
        if !valid {
            return Err(R17PersistentAllocationErrorV1::InvalidClassBinding);
        }
        Ok(())
    }

    fn require_current(&self) -> Result<(), R17PersistentAllocationErrorV1> {
        if self.current {
            Ok(())
        } else {
            Err(R17PersistentAllocationErrorV1::NotCurrent)
        }
    }

    fn record_index(&self, key: R17PersistentUseLeaseKeyV1) -> Option<usize> {
        if key.owner != self.owner {
            return None;
        }
        let index = usize::from(key.slot);
        self.records
            .get(index)
            .and_then(Option::as_ref)
            .filter(|record| record.binding.lease == key)
            .map(|_| index)
    }

    fn require_binding(
        &self,
        binding: R17PersistentUseBindingV1,
        registry_incarnation: &Rc<()>,
    ) -> Result<usize, R17PersistentAllocationErrorV1> {
        if !Rc::ptr_eq(registry_incarnation, &self.registry_incarnation)
            || binding.lease.owner != self.owner
            || binding.allocation != self.allocation
            || binding.mapping != self.mapping
        {
            return Err(R17PersistentAllocationErrorV1::WrongOwner);
        }
        let index = self
            .record_index(binding.lease)
            .ok_or(R17PersistentAllocationErrorV1::StaleLease)?;
        if self.records[index]
            .as_ref()
            .is_none_or(|record| record.binding != binding)
        {
            return Err(R17PersistentAllocationErrorV1::StaleLease);
        }
        Ok(index)
    }

    fn publish(
        &mut self,
        binding: R17PersistentUseBindingV1,
        registry_incarnation: &Rc<()>,
    ) -> Result<(), R17PersistentAllocationErrorV1> {
        self.require_current()?;
        let index = self.require_binding(binding, registry_incarnation)?;
        if self.records[index]
            .as_ref()
            .expect("validated record")
            .phase
            != R17PersistentUsePhaseV1::Reserved
        {
            return Err(R17PersistentAllocationErrorV1::IllegalState);
        }
        if !self.records[index]
            .as_ref()
            .expect("validated record")
            .dependencies
            .iter()
            .all(|dependency| {
                self.record_index(*dependency)
                    .is_some_and(|dependency_index| {
                        self.records[dependency_index]
                            .as_ref()
                            .expect("located record")
                            .terminal_status
                            == Some(R17PersistentTerminalStatusV1::Succeeded)
                    })
            })
        {
            return Err(R17PersistentAllocationErrorV1::DependencyNotReady);
        }
        if self
            .records
            .iter()
            .enumerate()
            .any(|(other_index, record)| {
                other_index != index
                    && record.as_ref().is_some_and(|record| {
                        retains_exclusive_use_v1(record.phase)
                            && !self.records[index]
                                .as_ref()
                                .expect("validated record")
                                .dependencies
                                .contains(&record.binding.lease)
                            && descriptors_conflict_v1(
                                binding.descriptor,
                                record.binding.descriptor,
                            )
                    })
            })
        {
            return Err(R17PersistentAllocationErrorV1::ConflictingUse);
        }
        self.records[index]
            .as_mut()
            .expect("validated record")
            .phase = R17PersistentUsePhaseV1::Published;
        Ok(())
    }

    fn cancel_reserved(
        &mut self,
        binding: R17PersistentUseBindingV1,
        registry_incarnation: &Rc<()>,
    ) -> Result<(), R17PersistentAllocationErrorV1> {
        self.require_current()?;
        let index = self.require_binding(binding, registry_incarnation)?;
        if self.records[index]
            .as_ref()
            .expect("validated record")
            .phase
            != R17PersistentUsePhaseV1::Reserved
        {
            return Err(R17PersistentAllocationErrorV1::IllegalState);
        }
        self.records[index] = None;
        self.completed_lease_count += 1;
        Ok(())
    }

    fn observe_use(
        &mut self,
        binding: R17PersistentUseBindingV1,
        registry_incarnation: &Rc<()>,
        timed_out: bool,
        observation: R17PersistentUseObservationV1,
    ) -> Result<R17ObservedUseStateV1, R17PersistentAllocationErrorV1> {
        self.require_current()?;
        let index = self.require_binding(binding, registry_incarnation)?;
        let expected = if timed_out {
            R17PersistentUsePhaseV1::TimedOut
        } else {
            R17PersistentUsePhaseV1::Published
        };
        if self.records[index]
            .as_ref()
            .expect("validated record")
            .phase
            != expected
        {
            return Err(R17PersistentAllocationErrorV1::IllegalState);
        }
        match observation {
            R17PersistentUseObservationV1::Pending if !timed_out => {
                Ok(R17ObservedUseStateV1::Published)
            }
            R17PersistentUseObservationV1::Pending | R17PersistentUseObservationV1::TimedOut => {
                self.records[index]
                    .as_mut()
                    .expect("validated record")
                    .phase = R17PersistentUsePhaseV1::TimedOut;
                Ok(R17ObservedUseStateV1::TimedOut)
            }
            R17PersistentUseObservationV1::Terminal(status) => {
                let record = self.records[index].as_mut().expect("validated record");
                record.phase = R17PersistentUsePhaseV1::Terminal;
                record.terminal_status = Some(status);
                Ok(R17ObservedUseStateV1::Terminal(status))
            }
            R17PersistentUseObservationV1::Indeterminate(reason) => {
                self.lose_currentness_inner(reason);
                Ok(R17ObservedUseStateV1::Quarantined(reason))
            }
        }
    }

    fn release_terminal(
        &mut self,
        binding: R17PersistentUseBindingV1,
        registry_incarnation: &Rc<()>,
        status: R17PersistentTerminalStatusV1,
    ) -> Result<(), R17PersistentAllocationErrorV1> {
        self.require_current()?;
        let index = self.require_binding(binding, registry_incarnation)?;
        let record = self.records[index].as_ref().expect("validated record");
        if record.phase != R17PersistentUsePhaseV1::Terminal
            || record.terminal_status != Some(status)
        {
            return Err(R17PersistentAllocationErrorV1::IllegalState);
        }
        if self.records.iter().flatten().any(|candidate| {
            candidate.phase == R17PersistentUsePhaseV1::Reserved
                && candidate.dependencies.contains(&binding.lease)
        }) {
            return Err(R17PersistentAllocationErrorV1::DependentRetained);
        }
        self.records[index] = None;
        self.completed_lease_count += 1;
        Ok(())
    }

    fn reconcile_cancelled(
        &self,
        binding: R17PersistentUseBindingV1,
        registry_incarnation: &Rc<()>,
    ) -> Result<(), R17PersistentAllocationErrorV1> {
        let index = self.require_binding(binding, registry_incarnation)?;
        if self.current
            || self.records[index]
                .as_ref()
                .expect("validated record")
                .phase
                != R17PersistentUsePhaseV1::CancelledBeforePublication
        {
            return Err(R17PersistentAllocationErrorV1::IllegalState);
        }
        Ok(())
    }

    fn reconcile_quarantined(
        &self,
        binding: R17PersistentUseBindingV1,
        registry_incarnation: &Rc<()>,
    ) -> Result<R17PersistentQuarantineReasonV1, R17PersistentAllocationErrorV1> {
        let index = self.require_binding(binding, registry_incarnation)?;
        if self.current
            || self.records[index]
                .as_ref()
                .expect("validated record")
                .phase
                != R17PersistentUsePhaseV1::Quarantined
        {
            return Err(R17PersistentAllocationErrorV1::IllegalState);
        }
        self.records[index]
            .as_ref()
            .expect("validated record")
            .quarantine_reason
            .ok_or(R17PersistentAllocationErrorV1::IllegalState)
    }

    fn lose_currentness_inner(
        &mut self,
        reason: R17PersistentQuarantineReasonV1,
    ) -> R17PersistentCurrentnessLossV1 {
        self.current = false;
        let mut cancelled_reservations = 0;
        let mut quarantined_uses = 0;
        for record in self.records.iter_mut().flatten() {
            match record.phase {
                R17PersistentUsePhaseV1::Reserved => {
                    record.phase = R17PersistentUsePhaseV1::CancelledBeforePublication;
                    cancelled_reservations += 1;
                }
                R17PersistentUsePhaseV1::Published
                | R17PersistentUsePhaseV1::TimedOut
                | R17PersistentUsePhaseV1::Terminal => {
                    record.phase = R17PersistentUsePhaseV1::Quarantined;
                    record.quarantine_reason = Some(reason);
                    quarantined_uses += 1;
                }
                R17PersistentUsePhaseV1::CancelledBeforePublication
                | R17PersistentUsePhaseV1::Quarantined
                | R17PersistentUsePhaseV1::Released => {}
            }
        }
        R17PersistentCurrentnessLossV1 {
            cancelled_reservations,
            quarantined_uses,
        }
    }
}

fn retains_exclusive_use_v1(phase: R17PersistentUsePhaseV1) -> bool {
    matches!(
        phase,
        R17PersistentUsePhaseV1::Published
            | R17PersistentUsePhaseV1::TimedOut
            | R17PersistentUsePhaseV1::Terminal
    )
}

fn descriptors_conflict_v1(
    left: R17PersistentUseDescriptorV1,
    right: R17PersistentUseDescriptorV1,
) -> bool {
    left.range.overlaps(right.range)
        && (left.access == R17PersistentAccessModeV1::Write
            || right.access == R17PersistentAccessModeV1::Write)
}

fn retained_records_conflict_v1(
    left: &R17PersistentUseRecordV1,
    right: &R17PersistentUseRecordV1,
) -> bool {
    retains_exclusive_use_v1(left.phase)
        && retains_exclusive_use_v1(right.phase)
        && descriptors_conflict_v1(left.binding.descriptor, right.binding.descriptor)
        && !successful_dependency_orders_v1(left, right)
        && !successful_dependency_orders_v1(right, left)
}

fn successful_dependency_orders_v1(
    predecessor: &R17PersistentUseRecordV1,
    successor: &R17PersistentUseRecordV1,
) -> bool {
    predecessor.phase == R17PersistentUsePhaseV1::Terminal
        && predecessor.terminal_status == Some(R17PersistentTerminalStatusV1::Succeeded)
        && successor.dependencies.contains(&predecessor.binding.lease)
}

enum R17ObservedUseStateV1 {
    Published,
    TimedOut,
    Terminal(R17PersistentTerminalStatusV1),
    Quarantined(R17PersistentQuarantineReasonV1),
}

/// Move-only pre-publication use custody.
///
/// ```compile_fail
/// use fe2o3_runtime_model::R17ReservedPersistentUseLeaseV1;
/// fn cannot_clone(lease: R17ReservedPersistentUseLeaseV1) {
///     let _duplicate = lease.clone();
/// }
/// ```
#[derive(Debug)]
#[must_use = "reserved persistent use must be published or cancelled"]
pub struct R17ReservedPersistentUseLeaseV1 {
    binding: R17PersistentUseBindingV1,
    registry_incarnation: Rc<()>,
}

impl R17ReservedPersistentUseLeaseV1 {
    pub const fn binding(&self) -> R17PersistentUseBindingV1 {
        self.binding
    }

    pub fn dependency_model_only(&self) -> R17PersistentUseDependencyV1 {
        R17PersistentUseDependencyV1 {
            binding: self.binding,
            registry_incarnation: Rc::clone(&self.registry_incarnation),
        }
    }

    pub fn publish_model_only(
        self,
        registry: &mut R17PersistentNativeAllocationRegistryV1,
    ) -> Result<R17PublishedPersistentUseLeaseV1, R17PersistentLeaseTransitionFailureV1<Self>> {
        match registry.publish(self.binding, &self.registry_incarnation) {
            Ok(()) => Ok(R17PublishedPersistentUseLeaseV1 {
                binding: self.binding,
                registry_incarnation: self.registry_incarnation,
            }),
            Err(error) => Err(R17PersistentLeaseTransitionFailureV1 {
                error,
                retained: self,
            }),
        }
    }

    pub fn cancel_before_publication_model_only(
        self,
        registry: &mut R17PersistentNativeAllocationRegistryV1,
    ) -> Result<R17PersistentUseReleaseReceiptV1, R17PersistentLeaseTransitionFailureV1<Self>> {
        match registry.cancel_reserved(self.binding, &self.registry_incarnation) {
            Ok(()) => Ok(R17PersistentUseReleaseReceiptV1 {
                binding: self.binding,
                outcome: R17PersistentReleaseOutcomeV1::CancelledBeforePublication,
            }),
            Err(error) => Err(R17PersistentLeaseTransitionFailureV1 {
                error,
                retained: self,
            }),
        }
    }

    pub fn reconcile_after_currentness_loss_model_only(
        self,
        registry: &R17PersistentNativeAllocationRegistryV1,
    ) -> Result<R17PersistentUseReleaseReceiptV1, R17PersistentLeaseTransitionFailureV1<Self>> {
        match registry.reconcile_cancelled(self.binding, &self.registry_incarnation) {
            Ok(()) => Ok(R17PersistentUseReleaseReceiptV1 {
                binding: self.binding,
                outcome: R17PersistentReleaseOutcomeV1::CancelledBeforePublication,
            }),
            Err(error) => Err(R17PersistentLeaseTransitionFailureV1 {
                error,
                retained: self,
            }),
        }
    }
}

#[derive(Debug)]
#[must_use = "published persistent use retains allocation custody"]
pub struct R17PublishedPersistentUseLeaseV1 {
    binding: R17PersistentUseBindingV1,
    registry_incarnation: Rc<()>,
}

impl R17PublishedPersistentUseLeaseV1 {
    pub const fn binding(&self) -> R17PersistentUseBindingV1 {
        self.binding
    }

    pub fn dependency_model_only(&self) -> R17PersistentUseDependencyV1 {
        R17PersistentUseDependencyV1 {
            binding: self.binding,
            registry_incarnation: Rc::clone(&self.registry_incarnation),
        }
    }

    pub fn observe_model_only(
        self,
        registry: &mut R17PersistentNativeAllocationRegistryV1,
        observation: R17PersistentUseObservationV1,
    ) -> Result<R17PersistentUsePollV1, R17PersistentLeaseTransitionFailureV1<Self>> {
        match registry.observe_use(self.binding, &self.registry_incarnation, false, observation) {
            Ok(R17ObservedUseStateV1::Published) => Ok(R17PersistentUsePollV1::Published(self)),
            Ok(R17ObservedUseStateV1::TimedOut) => Ok(R17PersistentUsePollV1::TimedOut(
                R17TimedOutPersistentUseLeaseV1 {
                    binding: self.binding,
                    registry_incarnation: self.registry_incarnation,
                },
            )),
            Ok(R17ObservedUseStateV1::Terminal(status)) => Ok(R17PersistentUsePollV1::Terminal(
                R17TerminalPersistentUseLeaseV1 {
                    binding: self.binding,
                    status,
                    registry_incarnation: self.registry_incarnation,
                },
            )),
            Ok(R17ObservedUseStateV1::Quarantined(reason)) => Ok(
                R17PersistentUsePollV1::Quarantined(R17QuarantinedPersistentUseLeaseV1 {
                    binding: self.binding,
                    reason,
                    _registry_incarnation: self.registry_incarnation,
                }),
            ),
            Err(error) => Err(R17PersistentLeaseTransitionFailureV1 {
                error,
                retained: self,
            }),
        }
    }

    pub fn reconcile_after_currentness_loss_model_only(
        self,
        registry: &R17PersistentNativeAllocationRegistryV1,
    ) -> Result<R17QuarantinedPersistentUseLeaseV1, R17PersistentLeaseTransitionFailureV1<Self>>
    {
        match registry.reconcile_quarantined(self.binding, &self.registry_incarnation) {
            Ok(reason) => Ok(R17QuarantinedPersistentUseLeaseV1 {
                binding: self.binding,
                reason,
                _registry_incarnation: self.registry_incarnation,
            }),
            Err(error) => Err(R17PersistentLeaseTransitionFailureV1 {
                error,
                retained: self,
            }),
        }
    }
}

#[derive(Debug)]
#[must_use = "timed-out persistent use remains published custody"]
pub struct R17TimedOutPersistentUseLeaseV1 {
    binding: R17PersistentUseBindingV1,
    registry_incarnation: Rc<()>,
}

impl R17TimedOutPersistentUseLeaseV1 {
    pub const fn binding(&self) -> R17PersistentUseBindingV1 {
        self.binding
    }

    pub fn dependency_model_only(&self) -> R17PersistentUseDependencyV1 {
        R17PersistentUseDependencyV1 {
            binding: self.binding,
            registry_incarnation: Rc::clone(&self.registry_incarnation),
        }
    }

    pub fn observe_model_only(
        self,
        registry: &mut R17PersistentNativeAllocationRegistryV1,
        observation: R17PersistentUseObservationV1,
    ) -> Result<R17TimedOutUsePollV1, R17PersistentLeaseTransitionFailureV1<Self>> {
        match registry.observe_use(self.binding, &self.registry_incarnation, true, observation) {
            Ok(R17ObservedUseStateV1::Published | R17ObservedUseStateV1::TimedOut) => {
                Ok(R17TimedOutUsePollV1::TimedOut(self))
            }
            Ok(R17ObservedUseStateV1::Terminal(status)) => Ok(R17TimedOutUsePollV1::Terminal(
                R17TerminalPersistentUseLeaseV1 {
                    binding: self.binding,
                    status,
                    registry_incarnation: self.registry_incarnation,
                },
            )),
            Ok(R17ObservedUseStateV1::Quarantined(reason)) => Ok(
                R17TimedOutUsePollV1::Quarantined(R17QuarantinedPersistentUseLeaseV1 {
                    binding: self.binding,
                    reason,
                    _registry_incarnation: self.registry_incarnation,
                }),
            ),
            Err(error) => Err(R17PersistentLeaseTransitionFailureV1 {
                error,
                retained: self,
            }),
        }
    }

    pub fn reconcile_after_currentness_loss_model_only(
        self,
        registry: &R17PersistentNativeAllocationRegistryV1,
    ) -> Result<R17QuarantinedPersistentUseLeaseV1, R17PersistentLeaseTransitionFailureV1<Self>>
    {
        match registry.reconcile_quarantined(self.binding, &self.registry_incarnation) {
            Ok(reason) => Ok(R17QuarantinedPersistentUseLeaseV1 {
                binding: self.binding,
                reason,
                _registry_incarnation: self.registry_incarnation,
            }),
            Err(error) => Err(R17PersistentLeaseTransitionFailureV1 {
                error,
                retained: self,
            }),
        }
    }
}

#[derive(Debug)]
#[must_use = "terminal persistent use must be explicitly released"]
pub struct R17TerminalPersistentUseLeaseV1 {
    binding: R17PersistentUseBindingV1,
    status: R17PersistentTerminalStatusV1,
    registry_incarnation: Rc<()>,
}

impl R17TerminalPersistentUseLeaseV1 {
    pub const fn binding(&self) -> R17PersistentUseBindingV1 {
        self.binding
    }

    pub const fn status(&self) -> R17PersistentTerminalStatusV1 {
        self.status
    }

    pub fn dependency_model_only(&self) -> R17PersistentUseDependencyV1 {
        R17PersistentUseDependencyV1 {
            binding: self.binding,
            registry_incarnation: Rc::clone(&self.registry_incarnation),
        }
    }

    pub fn release_model_only(
        self,
        registry: &mut R17PersistentNativeAllocationRegistryV1,
    ) -> Result<R17PersistentUseReleaseReceiptV1, R17PersistentLeaseTransitionFailureV1<Self>> {
        match registry.release_terminal(self.binding, &self.registry_incarnation, self.status) {
            Ok(()) => Ok(R17PersistentUseReleaseReceiptV1 {
                binding: self.binding,
                outcome: R17PersistentReleaseOutcomeV1::ReleasedAfterTerminal(self.status),
            }),
            Err(error) => Err(R17PersistentLeaseTransitionFailureV1 {
                error,
                retained: self,
            }),
        }
    }

    pub fn reconcile_after_currentness_loss_model_only(
        self,
        registry: &R17PersistentNativeAllocationRegistryV1,
    ) -> Result<R17QuarantinedPersistentUseLeaseV1, R17PersistentLeaseTransitionFailureV1<Self>>
    {
        match registry.reconcile_quarantined(self.binding, &self.registry_incarnation) {
            Ok(reason) => Ok(R17QuarantinedPersistentUseLeaseV1 {
                binding: self.binding,
                reason,
                _registry_incarnation: self.registry_incarnation,
            }),
            Err(error) => Err(R17PersistentLeaseTransitionFailureV1 {
                error,
                retained: self,
            }),
        }
    }
}

#[derive(Debug)]
#[must_use = "quarantined persistent use is process-teardown custody"]
pub struct R17QuarantinedPersistentUseLeaseV1 {
    binding: R17PersistentUseBindingV1,
    reason: R17PersistentQuarantineReasonV1,
    _registry_incarnation: Rc<()>,
}

impl R17QuarantinedPersistentUseLeaseV1 {
    pub const fn binding(&self) -> R17PersistentUseBindingV1 {
        self.binding
    }

    pub const fn reason(&self) -> R17PersistentQuarantineReasonV1 {
        self.reason
    }
}

#[derive(Debug)]
pub enum R17PersistentUsePollV1 {
    Published(R17PublishedPersistentUseLeaseV1),
    TimedOut(R17TimedOutPersistentUseLeaseV1),
    Terminal(R17TerminalPersistentUseLeaseV1),
    Quarantined(R17QuarantinedPersistentUseLeaseV1),
}

#[derive(Debug)]
pub enum R17TimedOutUsePollV1 {
    TimedOut(R17TimedOutPersistentUseLeaseV1),
    Terminal(R17TerminalPersistentUseLeaseV1),
    Quarantined(R17QuarantinedPersistentUseLeaseV1),
}

/// Release failure returns the sole owner unchanged.
pub struct R17PersistentOwnerReleaseFailureV1 {
    error: R17PersistentAllocationErrorV1,
    retained: R17PersistentNativeAllocationRegistryV1,
}

impl R17PersistentOwnerReleaseFailureV1 {
    pub const fn error(&self) -> R17PersistentAllocationErrorV1 {
        self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        R17PersistentAllocationErrorV1,
        R17PersistentNativeAllocationRegistryV1,
    ) {
        (self.error, self.retained)
    }
}
