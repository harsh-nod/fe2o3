//! Executable transition model for a persistent bidirectional local-SDMA pair.
//!
//! R19 is a versioned successor to, not a refinement of, R18. One private R17
//! local-allocation registry is composed directly with an exact pair of child
//! queues: engine zero for device-to-host and engine one for host-to-device.
//! Transfers are single-flight; either direction may follow exact frontier
//! retirement, including same-direction chunk continuation. No transition
//! performs a native operation or establishes a
//! correspondence with the independent Verus model or concrete KFD code.

#![allow(clippy::result_large_err)]

use alloc::rc::Rc;
use core::marker::PhantomData;

use crate::*;

pub const R19_DIRECTIONAL_PERSISTENT_LOCAL_SDMA_SCHEMA_VERSION_V1: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R19DirectionalChildQueueV1 {
    pub native_queue_id: u32,
    pub engine_id: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R19DirectionalQueuePairV1 {
    pub parent_queue: QueueKeyV1,
    pub pair_occurrence: u64,
    pub device_to_host: R19DirectionalChildQueueV1,
    pub host_to_device: R19DirectionalChildQueueV1,
}

impl R19DirectionalQueuePairV1 {
    pub const fn child(self, direction: R18LocalSdmaDirectionV1) -> R19DirectionalChildQueueV1 {
        match direction {
            R18LocalSdmaDirectionV1::DeviceToHost => self.device_to_host,
            R18LocalSdmaDirectionV1::HostToDevice => self.host_to_device,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R19DirectionalAdmissionV1 {
    pub allocation: R18LocalPersistentAllocationAdmissionV1,
    pub pair: R19DirectionalQueuePairV1,
    pub pool_generation: u64,
    pub logical_byte_len: u64,
    pub physical_byte_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R19DirectionalTransferBindingV1 {
    pub allocation: R18NativeAllocationKeyV1,
    pub pair: R19DirectionalQueuePairV1,
    pub attachment_generation: u64,
    pub pool_generation: u64,
    pub logical_byte_len: u64,
    pub physical_byte_len: u64,
    pub direction: R18LocalSdmaDirectionV1,
    pub persistent_access: R17PersistentAccessModeV1,
    pub persistent_endpoint: R18PersistentEndpointV1,
    pub device_range: R18ByteRangeV1,
    pub host: R18HostBufferKeyV1,
    pub host_range: R18ByteRangeV1,
    pub persistent_use: R17PersistentUseBindingV1,
    pub ticket: R18PlannedSdmaTicketV1,
}

impl R19DirectionalTransferBindingV1 {
    pub const fn child(self) -> R19DirectionalChildQueueV1 {
        self.pair.child(self.direction)
    }

    pub const fn persistent_descriptor(self) -> R17PersistentUseDescriptorV1 {
        R17PersistentUseDescriptorV1 {
            class: R17PersistentUseClassV1::LocalSdma {
                device: self.allocation.allocation.vm.device,
                queue: self.pair.parent_queue,
                engine_id: self.child().engine_id,
            },
            access: self.persistent_access,
            range: R17PersistentUseRangeV1 {
                byte_offset: self.device_range.byte_offset,
                byte_len: self.device_range.byte_len,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R19DirectionalPhaseV1 {
    Prepared,
    Published,
    TimedOut,
    Completed,
    Restored,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R19DirectionalLocationV1 {
    PersistentAllocation,
    PreparedRequest,
    NativeChildQueue,
    CompletionBatch,
    Quarantine,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R19DirectionalQuarantineReasonV1 {
    PreparationCurrentnessAmbiguous,
    PublicationIndeterminate(R18PrepublicationFailurePointV1),
    PublicationCurrentnessAmbiguous,
    CompletionCurrentnessAmbiguous,
    RestoreCurrentnessAmbiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R19DirectionalPublicationObservationV1 {
    pub binding: R19DirectionalTransferBindingV1,
    pub resolution: R18PublicationResolutionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R19DirectionalCompletionObservationV1 {
    pub binding: R19DirectionalTransferBindingV1,
    pub resolution: R18CompletionResolutionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R19DirectionalRestoreObservationV1 {
    pub binding: R19DirectionalTransferBindingV1,
    pub status: R18SdmaTerminalStatusV1,
    pub child_current: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R19DirectionalSettlementObservationV1 {
    pub binding: R19DirectionalTransferBindingV1,
    pub status: R18SdmaTerminalStatusV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R19SettledTransferKeyV1 {
    pub allocation: R18NativeAllocationKeyV1,
    pub pair: R19DirectionalQueuePairV1,
    pub attachment_generation: u64,
    pub pool_generation: u64,
    pub logical_byte_len: u64,
    pub physical_byte_len: u64,
    pub direction: R18LocalSdmaDirectionV1,
    pub persistent_frontier: R17SettledFrontierKeyV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R19DirectionalSnapshotV1 {
    pub allocation: R18NativeAllocationKeyV1,
    pub pair: R19DirectionalQueuePairV1,
    pub attachment_generation: u64,
    pub pool_generation: u64,
    pub logical_byte_len: u64,
    pub physical_byte_len: u64,
    pub current: bool,
    pub phase: Option<R19DirectionalPhaseV1>,
    pub location: R19DirectionalLocationV1,
    pub live_ticket: Option<R18PlannedSdmaTicketV1>,
    pub pending_frontier: Option<R19SettledTransferKeyV1>,
    pub settled_transfer_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R19DirectionalErrorV1 {
    InvalidAllocation,
    InvalidPair,
    InvalidPoolGeneration,
    InvalidHostBuffer,
    InvalidRange,
    WrongDirection,
    WrongAdapter,
    StaleBinding,
    ObservationMismatch,
    IllegalFailureClassification,
    IllegalState,
    NotCurrent,
    Busy,
    Quarantined,
    CapacityExceeded,
    InvariantViolation,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum R19InjectedLowerFailurePointV1 {
    Publish,
    Cancel,
    Quarantine,
    Observe,
    RestoreCurrentness,
    Settle,
    Retire,
}

struct R19ActiveTransferV1 {
    binding: R19DirectionalTransferBindingV1,
    phase: R19DirectionalPhaseV1,
    location: R19DirectionalLocationV1,
    live_ticket: Option<R18PlannedSdmaTicketV1>,
    terminal_status: Option<R18SdmaTerminalStatusV1>,
    quarantine_reason: Option<R19DirectionalQuarantineReasonV1>,
    lease: Option<R19UnderlyingLeaseV1>,
}

struct R19PendingFrontierV1 {
    key: R19SettledTransferKeyV1,
    frontier: R17SettledPersistentFrontierV1,
}

#[derive(Debug)]
enum R19UnderlyingLeaseV1 {
    Reserved(R17ReservedPersistentUseLeaseV1),
    Published(R17PublishedPersistentUseLeaseV1),
    TimedOut(R17TimedOutPersistentUseLeaseV1),
    Terminal(R17TerminalPersistentUseLeaseV1),
    Quarantined(R17QuarantinedPersistentUseLeaseV1),
}

impl R19UnderlyingLeaseV1 {
    const fn binding(&self) -> R17PersistentUseBindingV1 {
        match self {
            Self::Reserved(lease) => lease.binding(),
            Self::Published(lease) => lease.binding(),
            Self::TimedOut(lease) => lease.binding(),
            Self::Terminal(lease) => lease.binding(),
            Self::Quarantined(lease) => lease.binding(),
        }
    }
}

/// Sole owner of one directional pair and its directly composed R17 registry.
///
/// ```compile_fail
/// use fe2o3_runtime_model::R19DirectionalPersistentLocalSdmaAdapterV1;
/// fn cannot_clone(adapter: R19DirectionalPersistentLocalSdmaAdapterV1) {
///     let _copy = adapter.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_runtime_model::R19DirectionalPersistentLocalSdmaAdapterV1;
/// fn requires_send<T: Send>() {}
/// fn adapter_is_thread_affine() {
///     requires_send::<R19DirectionalPersistentLocalSdmaAdapterV1>();
/// }
/// ```
pub struct R19DirectionalPersistentLocalSdmaAdapterV1 {
    registry: R17PersistentNativeAllocationRegistryV1,
    admission: R18LocalPersistentAllocationAdmissionV1,
    allocation: R18NativeAllocationKeyV1,
    pair: R19DirectionalQueuePairV1,
    attachment_generation: u64,
    pool_generation: u64,
    logical_byte_len: u64,
    physical_byte_len: u64,
    current: bool,
    active: Option<R19ActiveTransferV1>,
    pending_frontier: Option<R19PendingFrontierV1>,
    settled_transfer_count: usize,
    incarnation: Rc<()>,
    thread_affine: PhantomData<Rc<()>>,
    #[cfg(test)]
    injected_lower_failure: Option<R19InjectedLowerFailurePointV1>,
}

impl R19DirectionalPersistentLocalSdmaAdapterV1 {
    pub fn new_model_only(
        admission: R19DirectionalAdmissionV1,
    ) -> Result<Self, R19DirectionalAdmissionFailureV1> {
        let allocation = R18NativeAllocationKeyV1 {
            owner: admission.allocation.owner,
            allocation: admission.allocation.allocation.key,
            mapping: admission.allocation.mapping.key,
        };
        if admission.pool_generation == 0 {
            return Err(R19DirectionalAdmissionFailureV1 {
                error: R19DirectionalErrorV1::InvalidPoolGeneration,
                retained: admission,
            });
        }
        if admission.logical_byte_len == 0
            || admission.logical_byte_len > admission.physical_byte_len
            || admission.physical_byte_len != admission.allocation.allocation.spec.byte_len
            || admission.physical_byte_len > R17_PERSISTENT_NATIVE_ALLOCATION_BYTES_V1
            || !admission
                .physical_byte_len
                .is_multiple_of(MEMORY_PAGE_BYTES_V1)
        {
            return Err(R19DirectionalAdmissionFailureV1 {
                error: R19DirectionalErrorV1::InvalidAllocation,
                retained: admission,
            });
        }
        if !valid_pair_v1(admission.pair, allocation) {
            return Err(R19DirectionalAdmissionFailureV1 {
                error: R19DirectionalErrorV1::InvalidPair,
                retained: admission,
            });
        }
        let registry = match R17PersistentNativeAllocationRegistryV1::new_local_model_only(
            admission.allocation.owner,
            admission.allocation.allocation,
            admission.allocation.mapping.clone(),
            admission.allocation.device,
        ) {
            Ok(registry) => registry,
            Err(_) => {
                return Err(R19DirectionalAdmissionFailureV1 {
                    error: R19DirectionalErrorV1::InvalidAllocation,
                    retained: admission,
                });
            }
        };
        let adapter = Self {
            registry,
            admission: admission.allocation,
            allocation,
            pair: admission.pair,
            attachment_generation: 1,
            pool_generation: admission.pool_generation,
            logical_byte_len: admission.logical_byte_len,
            physical_byte_len: admission.physical_byte_len,
            current: true,
            active: None,
            pending_frontier: None,
            settled_transfer_count: 0,
            incarnation: Rc::new(()),
            thread_affine: PhantomData,
            #[cfg(test)]
            injected_lower_failure: None,
        };
        if let Err(error) = adapter.validate_global_invariants() {
            return Err(R19DirectionalAdmissionFailureV1 {
                error,
                retained: R19DirectionalAdmissionV1 {
                    allocation: adapter.admission,
                    pair: adapter.pair,
                    pool_generation: adapter.pool_generation,
                    logical_byte_len: adapter.logical_byte_len,
                    physical_byte_len: adapter.physical_byte_len,
                },
            });
        }
        Ok(adapter)
    }

    pub const fn allocation(&self) -> R18NativeAllocationKeyV1 {
        self.allocation
    }

    pub const fn pair(&self) -> R19DirectionalQueuePairV1 {
        self.pair
    }

    pub fn snapshot(&self) -> R19DirectionalSnapshotV1 {
        R19DirectionalSnapshotV1 {
            allocation: self.allocation,
            pair: self.pair,
            attachment_generation: self.attachment_generation,
            pool_generation: self.pool_generation,
            logical_byte_len: self.logical_byte_len,
            physical_byte_len: self.physical_byte_len,
            current: self.current,
            phase: self.active.as_ref().map(|active| active.phase),
            location: self
                .active
                .as_ref()
                .map_or(R19DirectionalLocationV1::PersistentAllocation, |active| {
                    active.location
                }),
            live_ticket: self.active.as_ref().and_then(|active| active.live_ticket),
            pending_frontier: self.pending_frontier.as_ref().map(|frontier| frontier.key),
            settled_transfer_count: self.settled_transfer_count,
        }
    }

    pub fn active_persistent_use_record(&self) -> Option<R17PersistentUseRecordObservationV1> {
        let lease = self
            .active
            .as_ref()
            .map(|active| active.binding.persistent_use.lease)
            .or_else(|| {
                self.pending_frontier
                    .as_ref()
                    .map(|frontier| frontier.key.persistent_frontier.through_use)
            })?;
        self.registry.record(lease)
    }

    pub fn prepare_model_only(
        &mut self,
        direction: R18LocalSdmaDirectionV1,
        device_range: R18ByteRangeV1,
        host: R18HostBufferKeyV1,
        host_range: R18ByteRangeV1,
        ticket: R18PlannedSdmaTicketV1,
    ) -> Result<R19DirectionalTransferLeaseV1, R19DirectionalErrorV1> {
        if !self.current {
            return Err(R19DirectionalErrorV1::NotCurrent);
        }
        if self.active.is_some() || self.pending_frontier.is_some() {
            return Err(R19DirectionalErrorV1::Busy);
        }
        validate_host_and_ranges_v1(host, host_range, device_range, self.logical_byte_len)?;
        let child = self.pair.child(direction);
        if ticket.owner != self.pair.parent_queue
            || ticket.queue_id != child.native_queue_id
            || ticket.slot >= R18_SDMA_RING_SLOT_COUNT_V1
            || ticket.generation == 0
        {
            return Err(R19DirectionalErrorV1::StaleBinding);
        }
        let descriptor = R17PersistentUseDescriptorV1 {
            class: R17PersistentUseClassV1::LocalSdma {
                device: self.allocation.allocation.vm.device,
                queue: self.pair.parent_queue,
                engine_id: child.engine_id,
            },
            access: direction.persistent_access(),
            range: R17PersistentUseRangeV1 {
                byte_offset: device_range.byte_offset,
                byte_len: device_range.byte_len,
            },
        };
        let lease = self
            .registry
            .reserve_model_only(descriptor, alloc::vec![])
            .map_err(|_| R19DirectionalErrorV1::InvariantViolation)?;
        let binding = R19DirectionalTransferBindingV1 {
            allocation: self.allocation,
            pair: self.pair,
            attachment_generation: self.attachment_generation,
            pool_generation: self.pool_generation,
            logical_byte_len: self.logical_byte_len,
            physical_byte_len: self.physical_byte_len,
            direction,
            persistent_access: direction.persistent_access(),
            persistent_endpoint: direction.persistent_endpoint(),
            device_range,
            host,
            host_range,
            persistent_use: lease.binding(),
            ticket,
        };
        self.active = Some(R19ActiveTransferV1 {
            binding,
            phase: R19DirectionalPhaseV1::Prepared,
            location: R19DirectionalLocationV1::PreparedRequest,
            live_ticket: None,
            terminal_status: None,
            quarantine_reason: None,
            lease: Some(R19UnderlyingLeaseV1::Reserved(lease)),
        });
        self.validate_global_invariants()?;
        Ok(R19DirectionalTransferLeaseV1 {
            binding,
            state: R19TransferLeaseStateV1::Prepared,
            incarnation: Rc::clone(&self.incarnation),
        })
    }

    pub fn quarantine_preparation_currentness_model_only(
        &mut self,
        lease: R19DirectionalTransferLeaseV1,
    ) -> Result<
        R19DirectionalQuarantinedLeaseV1,
        R19DirectionalTransitionFailureV1<R19DirectionalTransferLeaseV1>,
    > {
        let binding = lease.binding;
        let transition = (|| {
            self.require_lease(&lease, &[R19TransferLeaseStateV1::Prepared])?;
            let reserved = self.take_reserved_lease()?;
            #[cfg(test)]
            if self.take_injected_lower_failure(R19InjectedLowerFailurePointV1::Quarantine) {
                self.restore_lease(R19UnderlyingLeaseV1::Reserved(reserved));
                return Err(R19DirectionalErrorV1::InvariantViolation);
            }
            let quarantined = match reserved
                .quarantine_indeterminate_prepublication_model_only(&mut self.registry)
            {
                Ok(quarantined) => quarantined,
                Err(failure) => {
                    self.restore_lease(R19UnderlyingLeaseV1::Reserved(failure.into_parts().1));
                    return Err(R19DirectionalErrorV1::InvariantViolation);
                }
            };
            self.restore_lease(R19UnderlyingLeaseV1::Quarantined(quarantined));
            self.current = false;
            self.set_quarantined(
                R19DirectionalQuarantineReasonV1::PreparationCurrentnessAmbiguous,
                None,
            );
            self.validate_global_invariants()?;
            Ok(R19DirectionalQuarantinedLeaseV1 {
                binding,
                reason: R19DirectionalQuarantineReasonV1::PreparationCurrentnessAmbiguous,
                live_ticket: None,
                _incarnation: Rc::clone(&self.incarnation),
            })
        })();
        transition.map_err(|error| R19DirectionalTransitionFailureV1 {
            error,
            retained: lease,
        })
    }

    pub fn rebind_pair_model_only(
        &mut self,
        pair: R19DirectionalQueuePairV1,
    ) -> Result<R19DirectionalRebindReceiptV1, R19DirectionalErrorV1> {
        if !self.current {
            return Err(R19DirectionalErrorV1::NotCurrent);
        }
        if self.active.is_some() || self.pending_frontier.is_some() {
            return Err(R19DirectionalErrorV1::Busy);
        }
        if !valid_pair_v1(pair, self.allocation) {
            return Err(R19DirectionalErrorV1::InvalidPair);
        }
        let attachment_generation = self
            .attachment_generation
            .checked_add(1)
            .ok_or(R19DirectionalErrorV1::CapacityExceeded)?;
        let previous = self.pair;
        self.pair = pair;
        self.attachment_generation = attachment_generation;
        self.validate_global_invariants()?;
        Ok(R19DirectionalRebindReceiptV1 {
            previous,
            current: pair,
            attachment_generation,
        })
    }

    pub fn demote_model_only(
        self,
    ) -> Result<R19DemotedDirectionalAllocationV1, R19DirectionalAdapterFailureV1> {
        let error = self.release_gate_error();
        if let Some(error) = error {
            return Err(R19DirectionalAdapterFailureV1 {
                error,
                retained: self,
            });
        }
        let next_pool_generation = match self.pool_generation.checked_add(1) {
            Some(generation) => generation,
            None => {
                return Err(R19DirectionalAdapterFailureV1 {
                    error: R19DirectionalErrorV1::CapacityExceeded,
                    retained: self,
                });
            }
        };
        let completed_lease_count = match self.registry.release_allocation_model_only() {
            Ok(receipt) => receipt.completed_lease_count,
            Err(failure) => {
                return Err(R19DirectionalAdapterFailureV1 {
                    error: R19DirectionalErrorV1::InvariantViolation,
                    retained: Self {
                        registry: failure.into_parts().1,
                        ..self
                    },
                });
            }
        };
        Ok(R19DemotedDirectionalAllocationV1 {
            allocation: self.admission,
            prior_pair: self.pair,
            prior_attachment_generation: self.attachment_generation,
            pool_generation: next_pool_generation,
            logical_byte_len: self.logical_byte_len,
            physical_byte_len: self.physical_byte_len,
            completed_lease_count,
            settled_transfer_count: self.settled_transfer_count,
            prior_incarnation: Rc::clone(&self.incarnation),
        })
    }

    pub fn release_model_only(
        self,
    ) -> Result<R19DirectionalReleaseReceiptV1, R19DirectionalAdapterFailureV1> {
        if let Some(error) = self.release_gate_error() {
            return Err(R19DirectionalAdapterFailureV1 {
                error,
                retained: self,
            });
        }
        let receipt = match self.registry.release_allocation_model_only() {
            Ok(receipt) => receipt,
            Err(failure) => {
                return Err(R19DirectionalAdapterFailureV1 {
                    error: R19DirectionalErrorV1::InvariantViolation,
                    retained: Self {
                        registry: failure.into_parts().1,
                        ..self
                    },
                });
            }
        };
        Ok(R19DirectionalReleaseReceiptV1 {
            allocation: self.allocation,
            pair: self.pair,
            attachment_generation: self.attachment_generation,
            pool_generation: self.pool_generation,
            logical_byte_len: self.logical_byte_len,
            physical_byte_len: self.physical_byte_len,
            completed_lease_count: receipt.completed_lease_count,
            settled_transfer_count: self.settled_transfer_count,
        })
    }

    pub fn validate_global_invariants(&self) -> Result<(), R19DirectionalErrorV1> {
        self.registry
            .validate_global_invariants()
            .map_err(|_| R19DirectionalErrorV1::InvariantViolation)?;
        if self.allocation.owner != self.registry.owner()
            || self.allocation.allocation != self.registry.allocation()
            || self.allocation.mapping != self.registry.mapping()
            || self.pool_generation == 0
            || self.logical_byte_len == 0
            || self.logical_byte_len > self.physical_byte_len
            || self.physical_byte_len != self.registry.byte_len()
            || self.physical_byte_len > R17_PERSISTENT_NATIVE_ALLOCATION_BYTES_V1
            || self.attachment_generation == 0
            || self.current != self.registry.is_current()
            || !valid_pair_v1(self.pair, self.allocation)
            || (self.active.is_some() && self.pending_frontier.is_some())
        {
            return Err(R19DirectionalErrorV1::InvariantViolation);
        }
        if let Some(active) = &self.active {
            self.validate_binding(active.binding)?;
            let expected_location = match active.phase {
                R19DirectionalPhaseV1::Prepared => R19DirectionalLocationV1::PreparedRequest,
                R19DirectionalPhaseV1::Published | R19DirectionalPhaseV1::TimedOut => {
                    R19DirectionalLocationV1::NativeChildQueue
                }
                R19DirectionalPhaseV1::Completed => R19DirectionalLocationV1::CompletionBatch,
                R19DirectionalPhaseV1::Restored => R19DirectionalLocationV1::PersistentAllocation,
                R19DirectionalPhaseV1::Quarantined => R19DirectionalLocationV1::Quarantine,
            };
            let expected_r17 = match active.phase {
                R19DirectionalPhaseV1::Prepared => R17PersistentUsePhaseV1::Reserved,
                R19DirectionalPhaseV1::Published => R17PersistentUsePhaseV1::Published,
                R19DirectionalPhaseV1::TimedOut => R17PersistentUsePhaseV1::TimedOut,
                R19DirectionalPhaseV1::Completed | R19DirectionalPhaseV1::Restored => {
                    R17PersistentUsePhaseV1::Terminal
                }
                R19DirectionalPhaseV1::Quarantined => R17PersistentUsePhaseV1::Quarantined,
            };
            let Some(lease) = active.lease.as_ref() else {
                return Err(R19DirectionalErrorV1::InvariantViolation);
            };
            let actual_r17 = match lease {
                R19UnderlyingLeaseV1::Reserved(_) => R17PersistentUsePhaseV1::Reserved,
                R19UnderlyingLeaseV1::Published(_) => R17PersistentUsePhaseV1::Published,
                R19UnderlyingLeaseV1::TimedOut(_) => R17PersistentUsePhaseV1::TimedOut,
                R19UnderlyingLeaseV1::Terminal(_) => R17PersistentUsePhaseV1::Terminal,
                R19UnderlyingLeaseV1::Quarantined(_) => R17PersistentUsePhaseV1::Quarantined,
            };
            let ticket_valid = match active.phase {
                R19DirectionalPhaseV1::Prepared => active.live_ticket.is_none(),
                R19DirectionalPhaseV1::Published
                | R19DirectionalPhaseV1::TimedOut
                | R19DirectionalPhaseV1::Completed
                | R19DirectionalPhaseV1::Restored => {
                    active.live_ticket == Some(active.binding.ticket)
                }
                R19DirectionalPhaseV1::Quarantined => active
                    .live_ticket
                    .is_none_or(|ticket| ticket == active.binding.ticket),
            };
            if active.location != expected_location
                || actual_r17 != expected_r17
                || lease.binding() != active.binding.persistent_use
                || !ticket_valid
                || (active.phase == R19DirectionalPhaseV1::Quarantined)
                    != active.quarantine_reason.is_some()
                || matches!(
                    active.phase,
                    R19DirectionalPhaseV1::Completed | R19DirectionalPhaseV1::Restored
                ) != active.terminal_status.is_some()
            {
                return Err(R19DirectionalErrorV1::InvariantViolation);
            }
            let record = self
                .registry
                .record(active.binding.persistent_use.lease)
                .ok_or(R19DirectionalErrorV1::InvariantViolation)?;
            if record.binding != active.binding.persistent_use || record.phase != expected_r17 {
                return Err(R19DirectionalErrorV1::InvariantViolation);
            }
        }
        if let Some(pending) = &self.pending_frontier {
            let snapshot = self.registry.snapshot();
            if pending.frontier.key() != pending.key.persistent_frontier
                || pending.key.allocation != self.allocation
                || pending.key.pair != self.pair
                || pending.key.attachment_generation != self.attachment_generation
                || pending.key.pool_generation != self.pool_generation
                || pending.key.logical_byte_len != self.logical_byte_len
                || pending.key.physical_byte_len != self.physical_byte_len
                || snapshot.lease_count != 1
                || snapshot.settled_count != 1
                || snapshot.frontier_use != Some(pending.key.persistent_frontier.through_use)
            {
                return Err(R19DirectionalErrorV1::InvariantViolation);
            }
        }
        let expected_count = usize::from(self.active.is_some() || self.pending_frontier.is_some());
        if self.registry.snapshot().lease_count != expected_count {
            return Err(R19DirectionalErrorV1::InvariantViolation);
        }
        Ok(())
    }

    fn validate_binding(
        &self,
        binding: R19DirectionalTransferBindingV1,
    ) -> Result<(), R19DirectionalErrorV1> {
        let child = binding.child();
        if binding.allocation != self.allocation
            || binding.pair != self.pair
            || binding.attachment_generation != self.attachment_generation
            || binding.pool_generation != self.pool_generation
            || binding.logical_byte_len != self.logical_byte_len
            || binding.physical_byte_len != self.physical_byte_len
            || binding.ticket.owner != binding.pair.parent_queue
            || binding.ticket.queue_id != child.native_queue_id
            || binding.ticket.slot >= R18_SDMA_RING_SLOT_COUNT_V1
            || binding.ticket.generation == 0
            || binding.persistent_use.allocation != binding.allocation.allocation
            || binding.persistent_use.mapping != binding.allocation.mapping
            || binding.persistent_use.lease.owner != binding.allocation.owner
            || binding.persistent_use.descriptor != binding.persistent_descriptor()
        {
            return Err(R19DirectionalErrorV1::StaleBinding);
        }
        if binding.persistent_access != binding.direction.persistent_access()
            || binding.persistent_endpoint != binding.direction.persistent_endpoint()
            || child.engine_id != binding.direction.required_engine()
        {
            return Err(R19DirectionalErrorV1::WrongDirection);
        }
        validate_host_and_ranges_v1(
            binding.host,
            binding.host_range,
            binding.device_range,
            self.logical_byte_len,
        )
    }

    fn require_lease(
        &self,
        lease: &R19DirectionalTransferLeaseV1,
        states: &[R19TransferLeaseStateV1],
    ) -> Result<(), R19DirectionalErrorV1> {
        if !Rc::ptr_eq(&lease.incarnation, &self.incarnation) {
            return Err(R19DirectionalErrorV1::WrongAdapter);
        }
        let active = self
            .active
            .as_ref()
            .ok_or(R19DirectionalErrorV1::StaleBinding)?;
        if active.binding != lease.binding || !states.contains(&lease.state) {
            return Err(R19DirectionalErrorV1::StaleBinding);
        }
        Ok(())
    }

    fn take_lease(&mut self) -> Result<R19UnderlyingLeaseV1, R19DirectionalErrorV1> {
        self.active
            .as_mut()
            .and_then(|active| active.lease.take())
            .ok_or(R19DirectionalErrorV1::InvariantViolation)
    }

    fn restore_lease(&mut self, lease: R19UnderlyingLeaseV1) {
        self.active.as_mut().expect("active transfer").lease = Some(lease);
    }

    fn take_reserved_lease(
        &mut self,
    ) -> Result<R17ReservedPersistentUseLeaseV1, R19DirectionalErrorV1> {
        match self.take_lease()? {
            R19UnderlyingLeaseV1::Reserved(lease) => Ok(lease),
            lease => {
                self.restore_lease(lease);
                Err(R19DirectionalErrorV1::InvariantViolation)
            }
        }
    }

    fn take_terminal_lease(
        &mut self,
    ) -> Result<R17TerminalPersistentUseLeaseV1, R19DirectionalErrorV1> {
        match self.take_lease()? {
            R19UnderlyingLeaseV1::Terminal(lease) => Ok(lease),
            lease => {
                self.restore_lease(lease);
                Err(R19DirectionalErrorV1::InvariantViolation)
            }
        }
    }

    fn set_state(
        &mut self,
        phase: R19DirectionalPhaseV1,
        location: R19DirectionalLocationV1,
        live_ticket: Option<R18PlannedSdmaTicketV1>,
        terminal_status: Option<R18SdmaTerminalStatusV1>,
    ) {
        let active = self.active.as_mut().expect("active transfer");
        active.phase = phase;
        active.location = location;
        active.live_ticket = live_ticket;
        active.terminal_status = terminal_status;
        active.quarantine_reason = None;
    }

    fn set_quarantined(
        &mut self,
        reason: R19DirectionalQuarantineReasonV1,
        live_ticket: Option<R18PlannedSdmaTicketV1>,
    ) {
        let active = self.active.as_mut().expect("active transfer");
        active.phase = R19DirectionalPhaseV1::Quarantined;
        active.location = R19DirectionalLocationV1::Quarantine;
        active.live_ticket = live_ticket;
        active.terminal_status = None;
        active.quarantine_reason = Some(reason);
    }

    fn release_gate_error(&self) -> Option<R19DirectionalErrorV1> {
        if !self.current {
            Some(R19DirectionalErrorV1::Quarantined)
        } else if self.active.is_some() || self.pending_frontier.is_some() {
            Some(R19DirectionalErrorV1::Busy)
        } else {
            self.validate_global_invariants().err()
        }
    }

    #[cfg(test)]
    pub(crate) fn inject_lower_failure_once(&mut self, point: R19InjectedLowerFailurePointV1) {
        self.injected_lower_failure = Some(point);
    }

    #[cfg(test)]
    fn take_injected_lower_failure(&mut self, point: R19InjectedLowerFailurePointV1) -> bool {
        if self.injected_lower_failure == Some(point) {
            self.injected_lower_failure = None;
            true
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub struct R19DirectionalTransferLeaseV1 {
    binding: R19DirectionalTransferBindingV1,
    state: R19TransferLeaseStateV1,
    incarnation: Rc<()>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum R19TransferLeaseStateV1 {
    Prepared,
    Published,
    TimedOut,
    Completed,
    Restored,
}

impl R19DirectionalTransferLeaseV1 {
    pub const fn binding(&self) -> R19DirectionalTransferBindingV1 {
        self.binding
    }

    pub fn resolve_publication_model_only(
        mut self,
        adapter: &mut R19DirectionalPersistentLocalSdmaAdapterV1,
        observation: R19DirectionalPublicationObservationV1,
    ) -> Result<R19DirectionalPublicationOutcomeV1, R19DirectionalTransitionFailureV1<Self>> {
        let transition = (|| {
            adapter.require_lease(&self, &[R19TransferLeaseStateV1::Prepared])?;
            if observation.binding != self.binding {
                return Err(R19DirectionalErrorV1::ObservationMismatch);
            }
            match observation.resolution {
                R18PublicationResolutionV1::Confirmed => {
                    let reserved = adapter.take_reserved_lease()?;
                    #[cfg(test)]
                    if adapter.take_injected_lower_failure(R19InjectedLowerFailurePointV1::Publish)
                    {
                        adapter.restore_lease(R19UnderlyingLeaseV1::Reserved(reserved));
                        return Err(R19DirectionalErrorV1::InvariantViolation);
                    }
                    let published = match reserved.publish_model_only(&mut adapter.registry) {
                        Ok(published) => published,
                        Err(failure) => {
                            adapter.restore_lease(R19UnderlyingLeaseV1::Reserved(
                                failure.into_parts().1,
                            ));
                            return Err(R19DirectionalErrorV1::InvariantViolation);
                        }
                    };
                    adapter.restore_lease(R19UnderlyingLeaseV1::Published(published));
                    adapter.set_state(
                        R19DirectionalPhaseV1::Published,
                        R19DirectionalLocationV1::NativeChildQueue,
                        Some(self.binding.ticket),
                        None,
                    );
                    self.state = R19TransferLeaseStateV1::Published;
                    adapter.validate_global_invariants()?;
                    Ok(R19DirectionalPublicationOutcomeV1::Published(
                        R19DirectionalTransferLeaseV1 {
                            binding: self.binding,
                            state: R19TransferLeaseStateV1::Published,
                            incarnation: Rc::clone(&self.incarnation),
                        },
                    ))
                }
                R18PublicationResolutionV1::RecoverableFailure { point } => {
                    if point != R18PrepublicationFailurePointV1::BeforeQueueCustody {
                        return Err(R19DirectionalErrorV1::IllegalFailureClassification);
                    }
                    let reserved = adapter.take_reserved_lease()?;
                    #[cfg(test)]
                    if adapter.take_injected_lower_failure(R19InjectedLowerFailurePointV1::Cancel) {
                        adapter.restore_lease(R19UnderlyingLeaseV1::Reserved(reserved));
                        return Err(R19DirectionalErrorV1::InvariantViolation);
                    }
                    if let Err(failure) =
                        reserved.cancel_before_publication_model_only(&mut adapter.registry)
                    {
                        adapter
                            .restore_lease(R19UnderlyingLeaseV1::Reserved(failure.into_parts().1));
                        return Err(R19DirectionalErrorV1::InvariantViolation);
                    }
                    adapter.active = None;
                    adapter.validate_global_invariants()?;
                    Ok(R19DirectionalPublicationOutcomeV1::Recovered(
                        R19DirectionalRecoveryReceiptV1 {
                            binding: self.binding,
                            point,
                        },
                    ))
                }
                R18PublicationResolutionV1::IndeterminateRetention { point } => {
                    if point == R18PrepublicationFailurePointV1::BeforeQueueCustody {
                        return Err(R19DirectionalErrorV1::IllegalFailureClassification);
                    }
                    let reserved = adapter.take_reserved_lease()?;
                    #[cfg(test)]
                    if adapter
                        .take_injected_lower_failure(R19InjectedLowerFailurePointV1::Quarantine)
                    {
                        adapter.restore_lease(R19UnderlyingLeaseV1::Reserved(reserved));
                        return Err(R19DirectionalErrorV1::InvariantViolation);
                    }
                    let quarantined = match reserved
                        .quarantine_indeterminate_prepublication_model_only(&mut adapter.registry)
                    {
                        Ok(quarantined) => quarantined,
                        Err(failure) => {
                            adapter.restore_lease(R19UnderlyingLeaseV1::Reserved(
                                failure.into_parts().1,
                            ));
                            return Err(R19DirectionalErrorV1::InvariantViolation);
                        }
                    };
                    adapter.restore_lease(R19UnderlyingLeaseV1::Quarantined(quarantined));
                    adapter.current = false;
                    let reason = R19DirectionalQuarantineReasonV1::PublicationIndeterminate(point);
                    adapter.set_quarantined(reason, Some(self.binding.ticket));
                    adapter.validate_global_invariants()?;
                    Ok(R19DirectionalPublicationOutcomeV1::Quarantined(
                        R19DirectionalQuarantinedLeaseV1 {
                            binding: self.binding,
                            reason,
                            live_ticket: Some(self.binding.ticket),
                            _incarnation: Rc::clone(&self.incarnation),
                        },
                    ))
                }
                R18PublicationResolutionV1::CurrentnessAmbiguous => {
                    let reserved = adapter.take_reserved_lease()?;
                    #[cfg(test)]
                    if adapter
                        .take_injected_lower_failure(R19InjectedLowerFailurePointV1::Quarantine)
                    {
                        adapter.restore_lease(R19UnderlyingLeaseV1::Reserved(reserved));
                        return Err(R19DirectionalErrorV1::InvariantViolation);
                    }
                    let quarantined = match reserved
                        .quarantine_indeterminate_prepublication_model_only(&mut adapter.registry)
                    {
                        Ok(quarantined) => quarantined,
                        Err(failure) => {
                            adapter.restore_lease(R19UnderlyingLeaseV1::Reserved(
                                failure.into_parts().1,
                            ));
                            return Err(R19DirectionalErrorV1::InvariantViolation);
                        }
                    };
                    adapter.restore_lease(R19UnderlyingLeaseV1::Quarantined(quarantined));
                    adapter.current = false;
                    let reason = R19DirectionalQuarantineReasonV1::PublicationCurrentnessAmbiguous;
                    adapter.set_quarantined(reason, Some(self.binding.ticket));
                    adapter.validate_global_invariants()?;
                    Ok(R19DirectionalPublicationOutcomeV1::Quarantined(
                        R19DirectionalQuarantinedLeaseV1 {
                            binding: self.binding,
                            reason,
                            live_ticket: Some(self.binding.ticket),
                            _incarnation: Rc::clone(&self.incarnation),
                        },
                    ))
                }
            }
        })();
        transition.map_err(|error| R19DirectionalTransitionFailureV1 {
            error,
            retained: self,
        })
    }

    pub fn observe_model_only(
        mut self,
        adapter: &mut R19DirectionalPersistentLocalSdmaAdapterV1,
        observation: R19DirectionalCompletionObservationV1,
    ) -> Result<R19DirectionalPollV1, R19DirectionalTransitionFailureV1<Self>> {
        let transition = (|| {
            adapter.require_lease(
                &self,
                &[
                    R19TransferLeaseStateV1::Published,
                    R19TransferLeaseStateV1::TimedOut,
                ],
            )?;
            if observation.binding != self.binding {
                return Err(R19DirectionalErrorV1::ObservationMismatch);
            }
            let already_timed_out = self.state == R19TransferLeaseStateV1::TimedOut;
            let r17_observation = match observation.resolution {
                R18CompletionResolutionV1::Pending => R17PersistentUseObservationV1::Pending,
                R18CompletionResolutionV1::TimedOut => R17PersistentUseObservationV1::TimedOut,
                R18CompletionResolutionV1::Terminal(status) => {
                    R17PersistentUseObservationV1::Terminal(r17_status_v1(status))
                }
                R18CompletionResolutionV1::CurrentnessAmbiguous => {
                    R17PersistentUseObservationV1::Indeterminate(
                        R17PersistentQuarantineReasonV1::CompletionObservationUnavailable,
                    )
                }
            };
            let current = adapter.take_lease()?;
            #[cfg(test)]
            if adapter.take_injected_lower_failure(R19InjectedLowerFailurePointV1::Observe) {
                adapter.restore_lease(current);
                return Err(R19DirectionalErrorV1::InvariantViolation);
            }
            let next = match (current, already_timed_out) {
                (R19UnderlyingLeaseV1::Published(lease), false) => {
                    match lease.observe_model_only(&mut adapter.registry, r17_observation) {
                        Ok(R17PersistentUsePollV1::Published(lease)) => {
                            R19UnderlyingLeaseV1::Published(lease)
                        }
                        Ok(R17PersistentUsePollV1::TimedOut(lease)) => {
                            R19UnderlyingLeaseV1::TimedOut(lease)
                        }
                        Ok(R17PersistentUsePollV1::Terminal(lease)) => {
                            R19UnderlyingLeaseV1::Terminal(lease)
                        }
                        Ok(R17PersistentUsePollV1::Quarantined(lease)) => {
                            R19UnderlyingLeaseV1::Quarantined(lease)
                        }
                        Err(failure) => {
                            adapter.restore_lease(R19UnderlyingLeaseV1::Published(
                                failure.into_parts().1,
                            ));
                            return Err(R19DirectionalErrorV1::InvariantViolation);
                        }
                    }
                }
                (R19UnderlyingLeaseV1::TimedOut(lease), true) => {
                    match lease.observe_model_only(&mut adapter.registry, r17_observation) {
                        Ok(R17TimedOutUsePollV1::TimedOut(lease)) => {
                            R19UnderlyingLeaseV1::TimedOut(lease)
                        }
                        Ok(R17TimedOutUsePollV1::Terminal(lease)) => {
                            R19UnderlyingLeaseV1::Terminal(lease)
                        }
                        Ok(R17TimedOutUsePollV1::Quarantined(lease)) => {
                            R19UnderlyingLeaseV1::Quarantined(lease)
                        }
                        Err(failure) => {
                            adapter.restore_lease(R19UnderlyingLeaseV1::TimedOut(
                                failure.into_parts().1,
                            ));
                            return Err(R19DirectionalErrorV1::InvariantViolation);
                        }
                    }
                }
                (lease, _) => {
                    adapter.restore_lease(lease);
                    return Err(R19DirectionalErrorV1::InvariantViolation);
                }
            };
            adapter.restore_lease(next);
            let result = match observation.resolution {
                R18CompletionResolutionV1::Pending if !already_timed_out => {
                    adapter.set_state(
                        R19DirectionalPhaseV1::Published,
                        R19DirectionalLocationV1::NativeChildQueue,
                        Some(self.binding.ticket),
                        None,
                    );
                    R19DirectionalPollV1::Pending(R19DirectionalTransferLeaseV1 {
                        binding: self.binding,
                        state: R19TransferLeaseStateV1::Published,
                        incarnation: Rc::clone(&self.incarnation),
                    })
                }
                R18CompletionResolutionV1::Pending | R18CompletionResolutionV1::TimedOut => {
                    adapter.set_state(
                        R19DirectionalPhaseV1::TimedOut,
                        R19DirectionalLocationV1::NativeChildQueue,
                        Some(self.binding.ticket),
                        None,
                    );
                    self.state = R19TransferLeaseStateV1::TimedOut;
                    R19DirectionalPollV1::TimedOut(R19DirectionalTransferLeaseV1 {
                        binding: self.binding,
                        state: R19TransferLeaseStateV1::TimedOut,
                        incarnation: Rc::clone(&self.incarnation),
                    })
                }
                R18CompletionResolutionV1::Terminal(status) => {
                    adapter.set_state(
                        R19DirectionalPhaseV1::Completed,
                        R19DirectionalLocationV1::CompletionBatch,
                        Some(self.binding.ticket),
                        Some(status),
                    );
                    self.state = R19TransferLeaseStateV1::Completed;
                    R19DirectionalPollV1::Completed(R19DirectionalTransferLeaseV1 {
                        binding: self.binding,
                        state: R19TransferLeaseStateV1::Completed,
                        incarnation: Rc::clone(&self.incarnation),
                    })
                }
                R18CompletionResolutionV1::CurrentnessAmbiguous => {
                    adapter.current = false;
                    let reason = R19DirectionalQuarantineReasonV1::CompletionCurrentnessAmbiguous;
                    adapter.set_quarantined(reason, Some(self.binding.ticket));
                    R19DirectionalPollV1::Quarantined(R19DirectionalQuarantinedLeaseV1 {
                        binding: self.binding,
                        reason,
                        live_ticket: Some(self.binding.ticket),
                        _incarnation: Rc::clone(&self.incarnation),
                    })
                }
            };
            adapter.validate_global_invariants()?;
            Ok(result)
        })();
        transition.map_err(|error| R19DirectionalTransitionFailureV1 {
            error,
            retained: self,
        })
    }

    pub fn restore_model_only(
        mut self,
        adapter: &mut R19DirectionalPersistentLocalSdmaAdapterV1,
        observation: R19DirectionalRestoreObservationV1,
    ) -> Result<R19DirectionalRestoreOutcomeV1, R19DirectionalTransitionFailureV1<Self>> {
        let transition = (|| {
            adapter.require_lease(&self, &[R19TransferLeaseStateV1::Completed])?;
            let status = adapter
                .active
                .as_ref()
                .and_then(|active| active.terminal_status)
                .ok_or(R19DirectionalErrorV1::IllegalState)?;
            if observation.binding != self.binding || observation.status != status {
                return Err(R19DirectionalErrorV1::ObservationMismatch);
            }
            if observation.child_current {
                adapter.set_state(
                    R19DirectionalPhaseV1::Restored,
                    R19DirectionalLocationV1::PersistentAllocation,
                    Some(self.binding.ticket),
                    Some(status),
                );
                self.state = R19TransferLeaseStateV1::Restored;
                adapter.validate_global_invariants()?;
                Ok(R19DirectionalRestoreOutcomeV1::Restored(
                    R19DirectionalTransferLeaseV1 {
                        binding: self.binding,
                        state: R19TransferLeaseStateV1::Restored,
                        incarnation: Rc::clone(&self.incarnation),
                    },
                ))
            } else {
                let terminal = adapter.take_terminal_lease()?;
                #[cfg(test)]
                if adapter
                    .take_injected_lower_failure(R19InjectedLowerFailurePointV1::RestoreCurrentness)
                {
                    adapter.restore_lease(R19UnderlyingLeaseV1::Terminal(terminal));
                    return Err(R19DirectionalErrorV1::InvariantViolation);
                }
                if adapter
                    .registry
                    .lose_currentness_model_only(
                        R17PersistentQuarantineReasonV1::DeviceCurrentnessLost,
                    )
                    .is_err()
                {
                    adapter.restore_lease(R19UnderlyingLeaseV1::Terminal(terminal));
                    return Err(R19DirectionalErrorV1::InvariantViolation);
                }
                let quarantined = match terminal
                    .reconcile_after_currentness_loss_model_only(&adapter.registry)
                {
                    Ok(quarantined) => quarantined,
                    Err(failure) => {
                        adapter
                            .restore_lease(R19UnderlyingLeaseV1::Terminal(failure.into_parts().1));
                        return Err(R19DirectionalErrorV1::InvariantViolation);
                    }
                };
                adapter.restore_lease(R19UnderlyingLeaseV1::Quarantined(quarantined));
                adapter.current = false;
                let reason = R19DirectionalQuarantineReasonV1::RestoreCurrentnessAmbiguous;
                adapter.set_quarantined(reason, Some(self.binding.ticket));
                adapter.validate_global_invariants()?;
                Ok(R19DirectionalRestoreOutcomeV1::Quarantined(
                    R19DirectionalQuarantinedLeaseV1 {
                        binding: self.binding,
                        reason,
                        live_ticket: Some(self.binding.ticket),
                        _incarnation: Rc::clone(&self.incarnation),
                    },
                ))
            }
        })();
        transition.map_err(|error| R19DirectionalTransitionFailureV1 {
            error,
            retained: self,
        })
    }

    pub fn settle_model_only(
        self,
        adapter: &mut R19DirectionalPersistentLocalSdmaAdapterV1,
        observation: R19DirectionalSettlementObservationV1,
    ) -> Result<R19DirectionalSettledFrontierV1, R19DirectionalTransitionFailureV1<Self>> {
        let transition = (|| {
            adapter.require_lease(&self, &[R19TransferLeaseStateV1::Restored])?;
            let status = adapter
                .active
                .as_ref()
                .and_then(|active| active.terminal_status)
                .ok_or(R19DirectionalErrorV1::IllegalState)?;
            if observation.binding != self.binding || observation.status != status {
                return Err(R19DirectionalErrorV1::ObservationMismatch);
            }
            let next_count = adapter
                .settled_transfer_count
                .checked_add(1)
                .ok_or(R19DirectionalErrorV1::CapacityExceeded)?;
            let terminal = adapter.take_terminal_lease()?;
            #[cfg(test)]
            if adapter.take_injected_lower_failure(R19InjectedLowerFailurePointV1::Settle) {
                adapter.restore_lease(R19UnderlyingLeaseV1::Terminal(terminal));
                return Err(R19DirectionalErrorV1::InvariantViolation);
            }
            let frontier = match terminal.settle_for_frontier_model_only(&mut adapter.registry) {
                Ok(frontier) => frontier,
                Err(failure) => {
                    adapter.restore_lease(R19UnderlyingLeaseV1::Terminal(failure.into_parts().1));
                    return Err(R19DirectionalErrorV1::InvariantViolation);
                }
            };
            let key = R19SettledTransferKeyV1 {
                allocation: adapter.allocation,
                pair: adapter.pair,
                attachment_generation: adapter.attachment_generation,
                pool_generation: adapter.pool_generation,
                logical_byte_len: adapter.logical_byte_len,
                physical_byte_len: adapter.physical_byte_len,
                direction: self.binding.direction,
                persistent_frontier: frontier.key(),
            };
            adapter.active = None;
            adapter.pending_frontier = Some(R19PendingFrontierV1 { key, frontier });
            adapter.settled_transfer_count = next_count;
            adapter.validate_global_invariants()?;
            Ok(R19DirectionalSettledFrontierV1 {
                key,
                incarnation: Rc::clone(&self.incarnation),
            })
        })();
        transition.map_err(|error| R19DirectionalTransitionFailureV1 {
            error,
            retained: self,
        })
    }
}

#[derive(Debug)]
pub enum R19DirectionalPublicationOutcomeV1 {
    Recovered(R19DirectionalRecoveryReceiptV1),
    Published(R19DirectionalTransferLeaseV1),
    Quarantined(R19DirectionalQuarantinedLeaseV1),
}

#[derive(Debug)]
pub enum R19DirectionalPollV1 {
    Pending(R19DirectionalTransferLeaseV1),
    TimedOut(R19DirectionalTransferLeaseV1),
    Completed(R19DirectionalTransferLeaseV1),
    Quarantined(R19DirectionalQuarantinedLeaseV1),
}

#[derive(Debug)]
pub enum R19DirectionalRestoreOutcomeV1 {
    Restored(R19DirectionalTransferLeaseV1),
    Quarantined(R19DirectionalQuarantinedLeaseV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R19DirectionalRecoveryReceiptV1 {
    pub binding: R19DirectionalTransferBindingV1,
    pub point: R18PrepublicationFailurePointV1,
}

#[derive(Debug)]
#[must_use = "settled frontier must be retired before any next transfer can start"]
pub struct R19DirectionalSettledFrontierV1 {
    key: R19SettledTransferKeyV1,
    incarnation: Rc<()>,
}

impl R19DirectionalSettledFrontierV1 {
    pub const fn key(&self) -> R19SettledTransferKeyV1 {
        self.key
    }

    pub fn retire_model_only(
        self,
        adapter: &mut R19DirectionalPersistentLocalSdmaAdapterV1,
        observed: R19SettledTransferKeyV1,
    ) -> Result<R19DirectionalFrontierRetirementReceiptV1, R19DirectionalTransitionFailureV1<Self>>
    {
        let transition = (|| {
            if !Rc::ptr_eq(&self.incarnation, &adapter.incarnation) {
                return Err(R19DirectionalErrorV1::WrongAdapter);
            }
            if observed != self.key {
                return Err(R19DirectionalErrorV1::ObservationMismatch);
            }
            let pending = adapter
                .pending_frontier
                .take()
                .ok_or(R19DirectionalErrorV1::StaleBinding)?;
            if pending.key != self.key {
                adapter.pending_frontier = Some(pending);
                return Err(R19DirectionalErrorV1::StaleBinding);
            }
            #[cfg(test)]
            if adapter.take_injected_lower_failure(R19InjectedLowerFailurePointV1::Retire) {
                adapter.pending_frontier = Some(pending);
                return Err(R19DirectionalErrorV1::InvariantViolation);
            }
            let receipt = match pending.frontier.retire_model_only(&mut adapter.registry) {
                Ok(receipt) => receipt,
                Err(failure) => {
                    adapter.pending_frontier = Some(R19PendingFrontierV1 {
                        key: pending.key,
                        frontier: failure.into_parts().1,
                    });
                    return Err(R19DirectionalErrorV1::InvariantViolation);
                }
            };
            adapter.validate_global_invariants()?;
            Ok(R19DirectionalFrontierRetirementReceiptV1 {
                frontier: self.key,
                retired_use_count: receipt.retired_use_count,
            })
        })();
        transition.map_err(|error| R19DirectionalTransitionFailureV1 {
            error,
            retained: self,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R19DirectionalFrontierRetirementReceiptV1 {
    pub frontier: R19SettledTransferKeyV1,
    pub retired_use_count: usize,
}

#[derive(Debug)]
#[must_use = "quarantine is permanent model custody until process teardown"]
pub struct R19DirectionalQuarantinedLeaseV1 {
    binding: R19DirectionalTransferBindingV1,
    reason: R19DirectionalQuarantineReasonV1,
    live_ticket: Option<R18PlannedSdmaTicketV1>,
    _incarnation: Rc<()>,
}

impl R19DirectionalQuarantinedLeaseV1 {
    pub const fn binding(&self) -> R19DirectionalTransferBindingV1 {
        self.binding
    }

    pub const fn reason(&self) -> R19DirectionalQuarantineReasonV1 {
        self.reason
    }

    pub const fn live_ticket(&self) -> Option<R18PlannedSdmaTicketV1> {
        self.live_ticket
    }
}

#[derive(Debug)]
#[must_use = "demoted allocation custody must be re-promoted or explicitly retained"]
pub struct R19DemotedDirectionalAllocationV1 {
    allocation: R18LocalPersistentAllocationAdmissionV1,
    prior_pair: R19DirectionalQueuePairV1,
    prior_attachment_generation: u64,
    pool_generation: u64,
    logical_byte_len: u64,
    physical_byte_len: u64,
    completed_lease_count: usize,
    settled_transfer_count: usize,
    prior_incarnation: Rc<()>,
}

impl R19DemotedDirectionalAllocationV1 {
    pub const fn pool_generation(&self) -> u64 {
        self.pool_generation
    }

    pub fn promote_model_only(
        self,
        pair: R19DirectionalQueuePairV1,
    ) -> Result<R19DirectionalPersistentLocalSdmaAdapterV1, R19DirectionalAdmissionFailureV1> {
        let _old_incarnation_remains_private = self.prior_incarnation;
        let _history = (
            self.prior_pair,
            self.prior_attachment_generation,
            self.completed_lease_count,
            self.settled_transfer_count,
        );
        R19DirectionalPersistentLocalSdmaAdapterV1::new_model_only(R19DirectionalAdmissionV1 {
            allocation: self.allocation,
            pair,
            pool_generation: self.pool_generation,
            logical_byte_len: self.logical_byte_len,
            physical_byte_len: self.physical_byte_len,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R19DirectionalRebindReceiptV1 {
    pub previous: R19DirectionalQueuePairV1,
    pub current: R19DirectionalQueuePairV1,
    pub attachment_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R19DirectionalReleaseReceiptV1 {
    pub allocation: R18NativeAllocationKeyV1,
    pub pair: R19DirectionalQueuePairV1,
    pub attachment_generation: u64,
    pub pool_generation: u64,
    pub logical_byte_len: u64,
    pub physical_byte_len: u64,
    pub completed_lease_count: usize,
    pub settled_transfer_count: usize,
}

#[derive(Debug)]
pub struct R19DirectionalTransitionFailureV1<T> {
    error: R19DirectionalErrorV1,
    retained: T,
}

impl<T> R19DirectionalTransitionFailureV1<T> {
    pub const fn error(&self) -> R19DirectionalErrorV1 {
        self.error
    }

    pub fn into_parts(self) -> (R19DirectionalErrorV1, T) {
        (self.error, self.retained)
    }
}

#[derive(Debug)]
pub struct R19DirectionalAdmissionFailureV1 {
    error: R19DirectionalErrorV1,
    retained: R19DirectionalAdmissionV1,
}

impl R19DirectionalAdmissionFailureV1 {
    pub const fn error(&self) -> R19DirectionalErrorV1 {
        self.error
    }

    pub fn into_parts(self) -> (R19DirectionalErrorV1, R19DirectionalAdmissionV1) {
        (self.error, self.retained)
    }
}

pub struct R19DirectionalAdapterFailureV1 {
    error: R19DirectionalErrorV1,
    retained: R19DirectionalPersistentLocalSdmaAdapterV1,
}

impl R19DirectionalAdapterFailureV1 {
    pub const fn error(&self) -> R19DirectionalErrorV1 {
        self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        R19DirectionalErrorV1,
        R19DirectionalPersistentLocalSdmaAdapterV1,
    ) {
        (self.error, self.retained)
    }
}

fn valid_pair_v1(pair: R19DirectionalQueuePairV1, allocation: R18NativeAllocationKeyV1) -> bool {
    pair.parent_queue.vm == allocation.allocation.vm
        && pair.parent_queue.id.0 != 0
        && pair.parent_queue.generation.0 != 0
        && pair.pair_occurrence != 0
        && pair.device_to_host.engine_id == R18_LOCAL_SDMA_DEVICE_TO_HOST_ENGINE_V1
        && pair.host_to_device.engine_id == R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1
        && pair.device_to_host.native_queue_id < R18_KFD_PROCESS_QUEUE_ID_LIMIT_V1
        && pair.host_to_device.native_queue_id < R18_KFD_PROCESS_QUEUE_ID_LIMIT_V1
        && pair.device_to_host.native_queue_id != pair.host_to_device.native_queue_id
}

fn validate_host_and_ranges_v1(
    host: R18HostBufferKeyV1,
    host_range: R18ByteRangeV1,
    device_range: R18ByteRangeV1,
    device_bytes: u64,
) -> Result<(), R19DirectionalErrorV1> {
    if host.session_id == 0
        || host.id == 0
        || host.generation == 0
        || host.byte_len == 0
        || host.coherence != MemoryCoherenceV1::HostCoherent
    {
        return Err(R19DirectionalErrorV1::InvalidHostBuffer);
    }
    let Some(device_end) = device_range.checked_end() else {
        return Err(R19DirectionalErrorV1::InvalidRange);
    };
    let Some(host_end) = host_range.checked_end() else {
        return Err(R19DirectionalErrorV1::InvalidRange);
    };
    if device_range.byte_len == 0
        || device_range.byte_len != host_range.byte_len
        || device_range.byte_len > R18_SDMA_MAX_LINEAR_COPY_BYTES_V1
        || device_end > device_bytes
        || host_end > host.byte_len
    {
        return Err(R19DirectionalErrorV1::InvalidRange);
    }
    Ok(())
}

const fn r17_status_v1(status: R18SdmaTerminalStatusV1) -> R17PersistentTerminalStatusV1 {
    match status {
        R18SdmaTerminalStatusV1::Succeeded => R17PersistentTerminalStatusV1::Succeeded,
        R18SdmaTerminalStatusV1::Failed { code } => R17PersistentTerminalStatusV1::Failed { code },
    }
}
