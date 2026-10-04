// Abstract summary of one local persistent device allocation composed with one
// ordinary host buffer and one targeted gfx942 SDMA queue. This file models a
// single in-flight adapter use because the sole native lease changes abstract
// custody location. It does not refine the independent R17 Rust/Verus models,
// production Rust ownership, KFD, mapped writes, a doorbell, or GPU completion.

use vstd::prelude::*;

verus! {

pub open spec fn max_allocation_bytes_v1() -> nat { 256 * 1024 * 1024 }
pub open spec fn max_copy_bytes_v1() -> nat { 0x003f_ffe0 }
pub open spec fn queue_slot_count_v1() -> nat { 64 }
pub open spec fn native_queue_id_limit_v1() -> nat { 1024 }
pub open spec fn max_u64_v1() -> nat { 0xffff_ffff_ffff_ffff }

#[derive(PartialEq, Eq)]
pub struct DeviceKeyV1 {
    pub physical: nat,
    pub generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct VmKeyV1 {
    pub device: DeviceKeyV1,
    pub id: nat,
}

#[derive(PartialEq, Eq)]
pub struct NativeAllocationKeyV1 {
    pub vm: VmKeyV1,
    pub allocation_id: nat,
    pub allocation_generation: nat,
    pub mapping_id: nat,
    pub byte_len: nat,
    pub local_mapping: bool,
}

#[derive(PartialEq, Eq)]
pub struct HostBufferKeyV1 {
    pub session_id: nat,
    pub allocation_id: nat,
    pub generation: nat,
    pub byte_len: nat,
    pub host_visible_coherent: bool,
}

#[derive(PartialEq, Eq)]
pub struct LogicalQueueKeyV1 {
    pub vm: VmKeyV1,
    pub queue_id: nat,
    pub generation: nat,
}

// `native_queue_id` is the KFD process queue slot. It is distinct from the
// logical parent QueueKey occurrence, and native slot zero is valid.
#[derive(PartialEq, Eq)]
pub struct SdmaQueueOccurrenceV1 {
    pub logical: LogicalQueueKeyV1,
    pub native_queue_id: nat,
    pub occurrence: nat,
    pub engine_index: nat,
}

#[derive(PartialEq, Eq)]
pub struct RangeV1 {
    pub offset: nat,
    pub byte_len: nat,
}

#[derive(PartialEq, Eq)]
pub enum DirectionV1 { DeviceToHost, HostToDevice }

#[derive(PartialEq, Eq)]
pub enum PersistentOperationV1 { LocalSdmaSource, LocalSdmaDestination }

#[derive(PartialEq, Eq)]
pub enum AccessV1 { Read, Write }

#[derive(PartialEq, Eq)]
pub struct SdmaTicketV1 {
    pub logical_queue: LogicalQueueKeyV1,
    pub native_queue_id: nat,
    pub slot: nat,
    pub generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct AdapterBindingV1 {
    pub persistent_owner_id: nat,
    pub persistent_use_slot: nat,
    pub persistent_use_generation: nat,
    pub native: NativeAllocationKeyV1,
    pub host: HostBufferKeyV1,
    pub queue: SdmaQueueOccurrenceV1,
    pub attachment_generation: nat,
    pub direction: DirectionV1,
    pub operation: PersistentOperationV1,
    pub access: AccessV1,
    pub persistent_range: RangeV1,
    pub host_range: RangeV1,
    pub copy_bytes: nat,
    pub planned_ticket: SdmaTicketV1,
}

#[derive(PartialEq, Eq)]
pub struct SettledFrontierV1 {
    pub native: NativeAllocationKeyV1,
    pub persistent_owner_id: nat,
    pub persistent_use_slot: nat,
    pub persistent_use_generation: nat,
    pub generation: nat,
}

pub open spec fn exact_settled_frontier_v1(
    binding: AdapterBindingV1,
    generation: nat,
) -> SettledFrontierV1 {
    SettledFrontierV1 {
        native: binding.native,
        persistent_owner_id: binding.persistent_owner_id,
        persistent_use_slot: binding.persistent_use_slot,
        persistent_use_generation: binding.persistent_use_generation,
        generation,
    }
}

pub open spec fn valid_device_v1(device: DeviceKeyV1) -> bool {
    device.physical > 0 && device.generation > 0
}

pub open spec fn valid_range_v1(range: RangeV1, extent: nat) -> bool {
    range.byte_len > 0
        && range.offset + range.byte_len <= max_u64_v1()
        && range.offset + range.byte_len <= extent
}

pub open spec fn exact_ticket_for_queue_v1(
    ticket: SdmaTicketV1,
    queue: SdmaQueueOccurrenceV1,
) -> bool {
    &&& ticket.logical_queue == queue.logical
    &&& ticket.native_queue_id == queue.native_queue_id
    &&& ticket.slot < queue_slot_count_v1()
    &&& ticket.generation > 0
}

pub open spec fn exact_direction_v1(binding: AdapterBindingV1) -> bool {
    match binding.direction {
        DirectionV1::DeviceToHost => {
            &&& binding.queue.engine_index == 0
            &&& binding.operation == PersistentOperationV1::LocalSdmaSource
            &&& binding.access == AccessV1::Read
        },
        DirectionV1::HostToDevice => {
            &&& binding.queue.engine_index == 1
            &&& binding.operation == PersistentOperationV1::LocalSdmaDestination
            &&& binding.access == AccessV1::Write
        },
    }
}

pub open spec fn admitted_binding_v1(binding: AdapterBindingV1) -> bool {
    &&& binding.persistent_owner_id > 0
    &&& binding.persistent_use_slot < 64
    &&& binding.persistent_use_generation > 0
    &&& valid_device_v1(binding.native.vm.device)
    &&& binding.native.vm.id > 0
    &&& binding.native.allocation_id > 0
    &&& binding.native.allocation_generation > 0
    &&& binding.native.mapping_id > 0
    &&& 0 < binding.native.byte_len <= max_allocation_bytes_v1()
    &&& binding.native.local_mapping
    &&& binding.host.session_id > 0
    &&& binding.host.allocation_id > 0
    &&& binding.host.generation > 0
    &&& binding.host.byte_len > 0
    &&& binding.host.host_visible_coherent
    &&& binding.queue.logical.vm == binding.native.vm
    &&& binding.queue.logical.queue_id > 0
    &&& binding.queue.logical.generation > 0
    &&& binding.queue.native_queue_id < native_queue_id_limit_v1()
    &&& binding.queue.occurrence > 0
    &&& binding.attachment_generation > 0
    &&& exact_direction_v1(binding)
    &&& 0 < binding.copy_bytes <= max_copy_bytes_v1()
    &&& binding.persistent_range.byte_len == binding.copy_bytes
    &&& binding.host_range.byte_len == binding.copy_bytes
    &&& valid_range_v1(binding.persistent_range, binding.native.byte_len)
    &&& valid_range_v1(binding.host_range, binding.host.byte_len)
    &&& exact_ticket_for_queue_v1(binding.planned_ticket, binding.queue)
}

#[derive(PartialEq, Eq)]
pub enum PersistentLeasePhaseV1 { Prepared, Published, Completed, Settled, Cancelled, Quarantined }

#[derive(PartialEq, Eq)]
pub enum NativeLocationV1 {
    PreparedRequest,
    QueueRecord,
    PersistentOwner,
    QuarantineCustody,
    Released,
}

#[derive(PartialEq, Eq)]
pub enum AdapterPhaseV1 {
    AdapterPrepared,
    PrepublicationRestored,
    Published,
    TimedOut,
    CompletionRestored,
    SettledFrontierPending,
    Settled,
    Cancelled,
    Quarantined,
    Released,
}

#[derive(PartialEq, Eq)]
pub struct AdapterStateV1 {
    pub binding: AdapterBindingV1,
    pub phase: AdapterPhaseV1,
    pub lease_phase: PersistentLeasePhaseV1,
    pub native_location: NativeLocationV1,
    pub live_ticket: Option<SdmaTicketV1>,
    pub settled_frontier: Option<SettledFrontierV1>,
    pub frontier_generation: nat,
    pub authority_count: nat,
    pub owner_quarantined: bool,
}

pub open spec fn valid_state_v1(state: AdapterStateV1) -> bool {
    &&& admitted_binding_v1(state.binding)
    &&& match state.phase {
        AdapterPhaseV1::AdapterPrepared => {
            &&& state.lease_phase == PersistentLeasePhaseV1::Prepared
            &&& state.native_location == NativeLocationV1::PreparedRequest
            &&& state.live_ticket.is_none()
            &&& state.settled_frontier.is_none()
            &&& state.authority_count == 1
            &&& !state.owner_quarantined
        },
        AdapterPhaseV1::PrepublicationRestored => {
            &&& state.lease_phase == PersistentLeasePhaseV1::Prepared
            &&& state.native_location == NativeLocationV1::PersistentOwner
            &&& state.live_ticket.is_none()
            &&& state.settled_frontier.is_none()
            &&& state.authority_count == 1
            &&& !state.owner_quarantined
        },
        AdapterPhaseV1::Published => {
            &&& state.lease_phase == PersistentLeasePhaseV1::Published
            &&& state.native_location == NativeLocationV1::QueueRecord
            &&& state.live_ticket == Some(state.binding.planned_ticket)
            &&& state.settled_frontier.is_none()
            &&& state.authority_count == 1
            &&& !state.owner_quarantined
        },
        AdapterPhaseV1::TimedOut => {
            &&& state.lease_phase == PersistentLeasePhaseV1::Published
            &&& state.native_location == NativeLocationV1::QueueRecord
            &&& state.live_ticket == Some(state.binding.planned_ticket)
            &&& state.settled_frontier.is_none()
            &&& state.authority_count == 1
            &&& !state.owner_quarantined
        },
        AdapterPhaseV1::CompletionRestored => {
            &&& state.lease_phase == PersistentLeasePhaseV1::Completed
            &&& state.native_location == NativeLocationV1::PersistentOwner
            &&& state.live_ticket.is_none()
            &&& state.settled_frontier.is_none()
            &&& state.authority_count == 1
            &&& !state.owner_quarantined
        },
        AdapterPhaseV1::SettledFrontierPending => {
            &&& state.lease_phase == PersistentLeasePhaseV1::Settled
            &&& state.native_location == NativeLocationV1::PersistentOwner
            &&& state.live_ticket.is_none()
            &&& state.frontier_generation > 0
            &&& state.settled_frontier
                == Some(exact_settled_frontier_v1(state.binding, state.frontier_generation))
            &&& state.authority_count == 1
            &&& !state.owner_quarantined
        },
        AdapterPhaseV1::Settled => {
            &&& state.lease_phase == PersistentLeasePhaseV1::Settled
            &&& state.native_location == NativeLocationV1::PersistentOwner
            &&& state.live_ticket.is_none()
            &&& state.settled_frontier.is_none()
            &&& state.authority_count == 1
            &&& !state.owner_quarantined
        },
        AdapterPhaseV1::Cancelled => {
            &&& state.lease_phase == PersistentLeasePhaseV1::Cancelled
            &&& state.native_location == NativeLocationV1::PersistentOwner
            &&& state.live_ticket.is_none()
            &&& state.settled_frontier.is_none()
            &&& state.authority_count == 1
            &&& !state.owner_quarantined
        },
        AdapterPhaseV1::Quarantined => {
            &&& state.lease_phase == PersistentLeasePhaseV1::Quarantined
            &&& state.native_location == NativeLocationV1::QuarantineCustody
            &&& (state.live_ticket.is_none()
                || state.live_ticket == Some(state.binding.planned_ticket))
            &&& state.settled_frontier.is_none()
            &&& state.authority_count == 1
            &&& state.owner_quarantined
        },
        AdapterPhaseV1::Released => {
            &&& (state.lease_phase == PersistentLeasePhaseV1::Settled
                || state.lease_phase == PersistentLeasePhaseV1::Cancelled)
            &&& state.native_location == NativeLocationV1::Released
            &&& state.live_ticket.is_none()
            &&& state.settled_frontier.is_none()
            &&& state.authority_count == 0
            &&& !state.owner_quarantined
        },
    }
}

// Currentness may fail before any queue record retains a planned ticket. The
// persistent allocation is still permanently quarantined.
pub open spec fn quarantine_preparation_currentness_v1(
    state: AdapterStateV1,
) -> AdapterStateV1 {
    if valid_state_v1(state) && state.phase == AdapterPhaseV1::AdapterPrepared {
        AdapterStateV1 {
            phase: AdapterPhaseV1::Quarantined,
            lease_phase: PersistentLeasePhaseV1::Quarantined,
            native_location: NativeLocationV1::QuarantineCustody,
            live_ticket: None,
            owner_quarantined: true,
            ..state
        }
    } else {
        state
    }
}

pub open spec fn restore_prepublication_v1(state: AdapterStateV1) -> AdapterStateV1 {
    if valid_state_v1(state) && state.phase == AdapterPhaseV1::AdapterPrepared {
        AdapterStateV1 {
            phase: AdapterPhaseV1::PrepublicationRestored,
            native_location: NativeLocationV1::PersistentOwner,
            ..state
        }
    } else {
        state
    }
}

pub open spec fn confirm_publication_v1(
    state: AdapterStateV1,
    observed_ticket: SdmaTicketV1,
) -> AdapterStateV1 {
    if valid_state_v1(state)
        && state.phase == AdapterPhaseV1::AdapterPrepared
        && observed_ticket == state.binding.planned_ticket
    {
        AdapterStateV1 {
            phase: AdapterPhaseV1::Published,
            lease_phase: PersistentLeasePhaseV1::Published,
            native_location: NativeLocationV1::QueueRecord,
            live_ticket: Some(observed_ticket),
            ..state
        }
    } else {
        state
    }
}

// SDMA's retained branch means mapped state may have changed after buffers
// entered the queue record. It is not proof that the write pointer published.
pub open spec fn quarantine_indeterminate_publication_v1(
    state: AdapterStateV1,
    retained_ticket: SdmaTicketV1,
) -> AdapterStateV1 {
    if valid_state_v1(state)
        && state.phase == AdapterPhaseV1::AdapterPrepared
        && retained_ticket == state.binding.planned_ticket
    {
        AdapterStateV1 {
            phase: AdapterPhaseV1::Quarantined,
            lease_phase: PersistentLeasePhaseV1::Quarantined,
            native_location: NativeLocationV1::QuarantineCustody,
            live_ticket: Some(retained_ticket),
            owner_quarantined: true,
            ..state
        }
    } else {
        state
    }
}

pub open spec fn observe_pending_v1(
    state: AdapterStateV1,
    observed_ticket: SdmaTicketV1,
) -> AdapterStateV1 {
    state
}

pub open spec fn observe_timeout_v1(
    state: AdapterStateV1,
    observed_ticket: SdmaTicketV1,
) -> AdapterStateV1 {
    if valid_state_v1(state)
        && (state.phase == AdapterPhaseV1::Published || state.phase == AdapterPhaseV1::TimedOut)
        && state.live_ticket == Some(observed_ticket)
    {
        AdapterStateV1 { phase: AdapterPhaseV1::TimedOut, ..state }
    } else {
        state
    }
}

pub open spec fn observe_completion_v1(
    state: AdapterStateV1,
    observed_ticket: SdmaTicketV1,
    returned_native: NativeAllocationKeyV1,
    returned_host: HostBufferKeyV1,
    returned_persistent_range: RangeV1,
    returned_host_range: RangeV1,
    returned_copy_bytes: nat,
    pre_current: bool,
    post_current: bool,
) -> AdapterStateV1 {
    if valid_state_v1(state)
        && (state.phase == AdapterPhaseV1::Published || state.phase == AdapterPhaseV1::TimedOut)
        && state.live_ticket == Some(observed_ticket)
        && returned_native == state.binding.native
        && returned_host == state.binding.host
        && returned_persistent_range == state.binding.persistent_range
        && returned_host_range == state.binding.host_range
        && returned_copy_bytes == state.binding.copy_bytes
    {
        if pre_current && post_current {
            AdapterStateV1 {
                phase: AdapterPhaseV1::CompletionRestored,
                lease_phase: PersistentLeasePhaseV1::Completed,
                native_location: NativeLocationV1::PersistentOwner,
                live_ticket: None,
                ..state
            }
        } else {
            AdapterStateV1 {
                phase: AdapterPhaseV1::Quarantined,
                lease_phase: PersistentLeasePhaseV1::Quarantined,
                native_location: NativeLocationV1::QuarantineCustody,
                live_ticket: Some(state.binding.planned_ticket),
                owner_quarantined: true,
                ..state
            }
        }
    } else {
        state
    }
}

pub open spec fn settle_completion_v1(state: AdapterStateV1) -> AdapterStateV1 {
    if valid_state_v1(state) && state.phase == AdapterPhaseV1::CompletionRestored {
        let generation = state.frontier_generation + 1;
        AdapterStateV1 {
            phase: AdapterPhaseV1::SettledFrontierPending,
            lease_phase: PersistentLeasePhaseV1::Settled,
            settled_frontier: Some(exact_settled_frontier_v1(state.binding, generation)),
            frontier_generation: generation,
            ..state
        }
    } else {
        state
    }
}

pub open spec fn retire_settled_frontier_v1(
    state: AdapterStateV1,
    observed_frontier: SettledFrontierV1,
) -> AdapterStateV1 {
    if valid_state_v1(state)
        && state.phase == AdapterPhaseV1::SettledFrontierPending
        && state.settled_frontier == Some(observed_frontier)
    {
        AdapterStateV1 {
            phase: AdapterPhaseV1::Settled,
            settled_frontier: None,
            ..state
        }
    } else {
        state
    }
}

pub open spec fn cancel_restored_v1(state: AdapterStateV1) -> AdapterStateV1 {
    if valid_state_v1(state) && state.phase == AdapterPhaseV1::PrepublicationRestored {
        AdapterStateV1 {
            phase: AdapterPhaseV1::Cancelled,
            lease_phase: PersistentLeasePhaseV1::Cancelled,
            ..state
        }
    } else {
        state
    }
}

pub open spec fn can_release_v1(state: AdapterStateV1, other_uses_outstanding: bool) -> bool {
    &&& !other_uses_outstanding
    &&& !state.owner_quarantined
    &&& state.native_location == NativeLocationV1::PersistentOwner
    &&& (state.phase == AdapterPhaseV1::Settled || state.phase == AdapterPhaseV1::Cancelled)
}

pub open spec fn release_v1(
    state: AdapterStateV1,
    other_uses_outstanding: bool,
) -> AdapterStateV1 {
    if valid_state_v1(state) && can_release_v1(state, other_uses_outstanding) {
        AdapterStateV1 {
            phase: AdapterPhaseV1::Released,
            native_location: NativeLocationV1::Released,
            authority_count: 0,
            ..state
        }
    } else {
        state
    }
}

#[derive(PartialEq, Eq)]
pub struct SequentialReuseSummaryV1 {
    pub occupied_slots: nat,
    pub retired_uses: nat,
}

// Each abstract step reserves one free slot, settles it, then retires its exact
// frontier before the next use. This isolates the capacity-reuse obligation
// from the queue publication state machine above.
pub open spec fn sequential_reuse_summary_v1(steps: nat) -> SequentialReuseSummaryV1
    decreases steps,
{
    if steps == 0 {
        SequentialReuseSummaryV1 { occupied_slots: 0, retired_uses: 0 }
    } else {
        let previous = sequential_reuse_summary_v1((steps - 1) as nat);
        SequentialReuseSummaryV1 {
            occupied_slots: 0,
            retired_uses: previous.retired_uses + 1,
        }
    }
}

pub open spec fn sample_device_v1() -> DeviceKeyV1 {
    DeviceKeyV1 { physical: 1, generation: 2 }
}

pub open spec fn sample_binding_v1(direction: DirectionV1) -> AdapterBindingV1 {
    let vm = VmKeyV1 { device: sample_device_v1(), id: 3 };
    let logical = LogicalQueueKeyV1 { vm, queue_id: 10, generation: 11 };
    let queue = SdmaQueueOccurrenceV1 {
        logical,
        native_queue_id: 0,
        occurrence: 17,
        engine_index: if direction == DirectionV1::DeviceToHost { 0 } else { 1 },
    };
    AdapterBindingV1 {
        persistent_owner_id: 12,
        persistent_use_slot: 13,
        persistent_use_generation: 14,
        native: NativeAllocationKeyV1 {
            vm,
            allocation_id: 4,
            allocation_generation: 5,
            mapping_id: 6,
            byte_len: 65536,
            local_mapping: true,
        },
        host: HostBufferKeyV1 {
            session_id: 7,
            allocation_id: 8,
            generation: 9,
            byte_len: 65536,
            host_visible_coherent: true,
        },
        queue,
        attachment_generation: 18,
        direction,
        operation: if direction == DirectionV1::DeviceToHost {
            PersistentOperationV1::LocalSdmaSource
        } else {
            PersistentOperationV1::LocalSdmaDestination
        },
        access: if direction == DirectionV1::DeviceToHost { AccessV1::Read } else { AccessV1::Write },
        persistent_range: RangeV1 { offset: 4096, byte_len: 4096 },
        host_range: RangeV1 { offset: 8192, byte_len: 4096 },
        copy_bytes: 4096,
        planned_ticket: SdmaTicketV1 {
            logical_queue: logical,
            native_queue_id: 0,
            slot: 15,
            generation: 16,
        },
    }
}

pub open spec fn sample_prepared_v1(direction: DirectionV1) -> AdapterStateV1 {
    AdapterStateV1 {
        binding: sample_binding_v1(direction),
        phase: AdapterPhaseV1::AdapterPrepared,
        lease_phase: PersistentLeasePhaseV1::Prepared,
        native_location: NativeLocationV1::PreparedRequest,
        live_ticket: None,
        settled_frontier: None,
        frontier_generation: 0,
        authority_count: 1,
        owner_quarantined: false,
    }
}

pub proof fn exact_profile_bounds_are_fixed_v1()
    ensures
        max_allocation_bytes_v1() == 268435456,
        max_copy_bytes_v1() == 4194272,
        queue_slot_count_v1() == 64,
        native_queue_id_limit_v1() == 1024,
{}

pub proof fn exact_d2h_binding_is_inhabited_v1()
    ensures admitted_binding_v1(sample_binding_v1(DirectionV1::DeviceToHost)),
{}

pub proof fn exact_h2d_binding_is_inhabited_v1()
    ensures admitted_binding_v1(sample_binding_v1(DirectionV1::HostToDevice)),
{}

pub proof fn native_queue_zero_is_valid_and_upper_bound_is_rejected_v1()
    ensures
        admitted_binding_v1(sample_binding_v1(DirectionV1::DeviceToHost)),
        !admitted_binding_v1(AdapterBindingV1 {
            queue: SdmaQueueOccurrenceV1 {
                native_queue_id: 1024,
                ..sample_binding_v1(DirectionV1::DeviceToHost).queue
            },
            ..sample_binding_v1(DirectionV1::DeviceToHost)
        }),
{}

pub proof fn logical_queue_occurrence_substitution_is_rejected_v1()
    ensures !admitted_binding_v1(AdapterBindingV1 {
        queue: SdmaQueueOccurrenceV1 {
            logical: LogicalQueueKeyV1 {
                generation: 99,
                ..sample_binding_v1(DirectionV1::DeviceToHost).queue.logical
            },
            ..sample_binding_v1(DirectionV1::DeviceToHost).queue
        },
        ..sample_binding_v1(DirectionV1::DeviceToHost)
    }),
{}

pub proof fn allocation_vm_substitution_is_rejected_v1()
    ensures !admitted_binding_v1(AdapterBindingV1 {
        native: NativeAllocationKeyV1 {
            vm: VmKeyV1 { id: 99, ..sample_binding_v1(DirectionV1::DeviceToHost).native.vm },
            ..sample_binding_v1(DirectionV1::DeviceToHost).native
        },
        ..sample_binding_v1(DirectionV1::DeviceToHost)
    }),
{}

pub proof fn host_identity_is_nonzero_and_range_is_exact_v1()
    ensures
        !admitted_binding_v1(AdapterBindingV1 {
            host: HostBufferKeyV1 {
                generation: 0,
                ..sample_binding_v1(DirectionV1::DeviceToHost).host
            },
            ..sample_binding_v1(DirectionV1::DeviceToHost)
        }),
        !admitted_binding_v1(AdapterBindingV1 {
            host_range: RangeV1 { offset: 8192, byte_len: 2048 },
            ..sample_binding_v1(DirectionV1::DeviceToHost)
        }),
        !admitted_binding_v1(AdapterBindingV1 {
            native: NativeAllocationKeyV1 {
                local_mapping: false,
                ..sample_binding_v1(DirectionV1::DeviceToHost).native
            },
            ..sample_binding_v1(DirectionV1::DeviceToHost)
        }),
        !admitted_binding_v1(AdapterBindingV1 {
            host: HostBufferKeyV1 {
                host_visible_coherent: false,
                ..sample_binding_v1(DirectionV1::DeviceToHost).host
            },
            ..sample_binding_v1(DirectionV1::DeviceToHost)
        }),
{}

pub proof fn direction_engine_operation_and_access_are_exact_v1()
    ensures
        !admitted_binding_v1(AdapterBindingV1 {
            queue: SdmaQueueOccurrenceV1 {
                engine_index: 1,
                ..sample_binding_v1(DirectionV1::DeviceToHost).queue
            },
            ..sample_binding_v1(DirectionV1::DeviceToHost)
        }),
        !admitted_binding_v1(AdapterBindingV1 {
            operation: PersistentOperationV1::LocalSdmaSource,
            ..sample_binding_v1(DirectionV1::HostToDevice)
        }),
{}

pub proof fn persistent_range_overflow_and_extent_are_rejected_v1()
    ensures
        !admitted_binding_v1(AdapterBindingV1 {
            persistent_range: RangeV1 { offset: 65535, byte_len: 4096 },
            ..sample_binding_v1(DirectionV1::DeviceToHost)
        }),
        !valid_range_v1(RangeV1 { offset: max_u64_v1(), byte_len: 2 }, max_u64_v1()),
{}

pub proof fn ticket_binds_both_queue_identities_and_bounds_slot_generation_v1()
    ensures {
        let binding = sample_binding_v1(DirectionV1::DeviceToHost);
        &&& exact_ticket_for_queue_v1(binding.planned_ticket, binding.queue)
        &&& binding.planned_ticket.slot < queue_slot_count_v1()
        &&& binding.planned_ticket.generation > 0
        &&& !exact_ticket_for_queue_v1(SdmaTicketV1 {
            native_queue_id: binding.planned_ticket.native_queue_id + 1,
            ..binding.planned_ticket
        }, binding.queue)
    },
{}

pub proof fn prepared_state_has_one_authority_v1()
    ensures {
        let state = sample_prepared_v1(DirectionV1::DeviceToHost);
        &&& valid_state_v1(state)
        &&& state.authority_count == 1
        &&& state.native_location == NativeLocationV1::PreparedRequest
    },
{}

pub proof fn preparation_currentness_loss_quarantines_without_ticket_v1(
    state: AdapterStateV1,
)
    requires valid_state_v1(state), state.phase == AdapterPhaseV1::AdapterPrepared,
    ensures {
        let quarantined = quarantine_preparation_currentness_v1(state);
        &&& valid_state_v1(quarantined)
        &&& quarantined.binding == state.binding
        &&& quarantined.phase == AdapterPhaseV1::Quarantined
        &&& quarantined.lease_phase == PersistentLeasePhaseV1::Quarantined
        &&& quarantined.native_location == NativeLocationV1::QuarantineCustody
        &&& quarantined.live_ticket.is_none()
        &&& quarantined.authority_count == 1
        &&& quarantined.owner_quarantined
        &&& !can_release_v1(quarantined, false)
        &&& release_v1(quarantined, false) == quarantined
    },
{}

pub proof fn recoverable_failure_restores_exact_prepublication_custody_v1(
    state: AdapterStateV1,
)
    requires valid_state_v1(state), state.phase == AdapterPhaseV1::AdapterPrepared,
    ensures {
        let restored = restore_prepublication_v1(state);
        &&& restored.binding == state.binding
        &&& restored.phase == AdapterPhaseV1::PrepublicationRestored
        &&& restored.lease_phase == PersistentLeasePhaseV1::Prepared
        &&& restored.native_location == NativeLocationV1::PersistentOwner
        &&& restored.live_ticket.is_none()
        &&& restored.authority_count == 1
        &&& valid_state_v1(restored)
    },
{}

pub proof fn confirmed_publication_retains_exact_ticket_and_single_custody_v1(
    state: AdapterStateV1,
)
    requires valid_state_v1(state), state.phase == AdapterPhaseV1::AdapterPrepared,
    ensures {
        let published = confirm_publication_v1(state, state.binding.planned_ticket);
        &&& published.binding == state.binding
        &&& published.phase == AdapterPhaseV1::Published
        &&& published.lease_phase == PersistentLeasePhaseV1::Published
        &&& published.native_location == NativeLocationV1::QueueRecord
        &&& published.live_ticket == Some(state.binding.planned_ticket)
        &&& published.authority_count == 1
        &&& valid_state_v1(published)
    },
{}

pub proof fn confirmed_publication_retains_child_occurrence_and_attachment_generation_v1(
    state: AdapterStateV1,
)
    requires valid_state_v1(state), state.phase == AdapterPhaseV1::AdapterPrepared,
    ensures {
        let published = confirm_publication_v1(state, state.binding.planned_ticket);
        &&& published.binding.queue.logical == state.binding.queue.logical
        &&& published.binding.queue.native_queue_id == state.binding.queue.native_queue_id
        &&& published.binding.queue.occurrence == state.binding.queue.occurrence
        &&& published.binding.attachment_generation == state.binding.attachment_generation
    },
{}

pub proof fn stale_confirmation_ticket_is_rejected_atomically_v1(
    state: AdapterStateV1,
    stale: SdmaTicketV1,
)
    requires
        valid_state_v1(state),
        state.phase == AdapterPhaseV1::AdapterPrepared,
        stale != state.binding.planned_ticket,
    ensures confirm_publication_v1(state, stale) == state,
{}

pub proof fn retained_indeterminate_moves_prepared_directly_to_quarantine_v1(
    state: AdapterStateV1,
)
    requires valid_state_v1(state), state.phase == AdapterPhaseV1::AdapterPrepared,
    ensures {
        let quarantined = quarantine_indeterminate_publication_v1(
            state,
            state.binding.planned_ticket,
        );
        &&& quarantined.binding == state.binding
        &&& quarantined.phase == AdapterPhaseV1::Quarantined
        &&& quarantined.lease_phase == PersistentLeasePhaseV1::Quarantined
        &&& quarantined.native_location == NativeLocationV1::QuarantineCustody
        &&& quarantined.live_ticket == Some(state.binding.planned_ticket)
        &&& quarantined.authority_count == 1
        &&& quarantined.owner_quarantined
        &&& valid_state_v1(quarantined)
    },
{}

pub proof fn pending_observation_preserves_exact_custody_v1(
    state: AdapterStateV1,
    observed_ticket: SdmaTicketV1,
)
    ensures observe_pending_v1(state, observed_ticket) == state,
{}

pub proof fn timeout_retains_exact_published_ticket_and_location_v1(
    state: AdapterStateV1,
)
    requires valid_state_v1(state), state.phase == AdapterPhaseV1::Published,
    ensures {
        let timed_out = observe_timeout_v1(state, state.binding.planned_ticket);
        &&& timed_out.binding == state.binding
        &&& timed_out.phase == AdapterPhaseV1::TimedOut
        &&& timed_out.lease_phase == PersistentLeasePhaseV1::Published
        &&& timed_out.native_location == NativeLocationV1::QueueRecord
        &&& timed_out.live_ticket == state.live_ticket
        &&& timed_out.authority_count == 1
        &&& valid_state_v1(timed_out)
    },
{}

pub proof fn stale_timeout_ticket_is_rejected_atomically_v1(
    state: AdapterStateV1,
    stale: SdmaTicketV1,
)
    requires
        valid_state_v1(state),
        state.phase == AdapterPhaseV1::Published,
        stale != state.binding.planned_ticket,
    ensures observe_timeout_v1(state, stale) == state,
{}

pub proof fn exact_completion_restores_same_native_host_ranges_and_owner_v1(
    state: AdapterStateV1,
)
    requires
        valid_state_v1(state),
        state.phase == AdapterPhaseV1::Published || state.phase == AdapterPhaseV1::TimedOut,
    ensures {
        let completed = observe_completion_v1(
            state,
            state.binding.planned_ticket,
            state.binding.native,
            state.binding.host,
            state.binding.persistent_range,
            state.binding.host_range,
            state.binding.copy_bytes,
            true,
            true,
        );
        &&& completed.binding == state.binding
        &&& completed.phase == AdapterPhaseV1::CompletionRestored
        &&& completed.lease_phase == PersistentLeasePhaseV1::Completed
        &&& completed.native_location == NativeLocationV1::PersistentOwner
        &&& completed.live_ticket.is_none()
        &&& completed.authority_count == 1
        &&& valid_state_v1(completed)
    },
{}

pub proof fn stale_or_foreign_completion_is_rejected_atomically_v1(
    state: AdapterStateV1,
    stale: SdmaTicketV1,
)
    requires
        valid_state_v1(state),
        state.phase == AdapterPhaseV1::Published || state.phase == AdapterPhaseV1::TimedOut,
        stale != state.binding.planned_ticket,
    ensures observe_completion_v1(
        state,
        stale,
        state.binding.native,
        state.binding.host,
        state.binding.persistent_range,
        state.binding.host_range,
        state.binding.copy_bytes,
        true,
        true,
    ) == state,
{}

pub proof fn substituted_completion_resources_or_ranges_are_rejected_v1(
    state: AdapterStateV1,
)
    requires
        valid_state_v1(state),
        state.phase == AdapterPhaseV1::Published || state.phase == AdapterPhaseV1::TimedOut,
    ensures
        observe_completion_v1(
            state,
            state.binding.planned_ticket,
            NativeAllocationKeyV1 {
                allocation_generation: state.binding.native.allocation_generation + 1,
                ..state.binding.native
            },
            state.binding.host,
            state.binding.persistent_range,
            state.binding.host_range,
            state.binding.copy_bytes,
            true,
            true,
        ) == state,
        observe_completion_v1(
            state,
            state.binding.planned_ticket,
            state.binding.native,
            state.binding.host,
            RangeV1 {
                offset: state.binding.persistent_range.offset + 1,
                ..state.binding.persistent_range
            },
            state.binding.host_range,
            state.binding.copy_bytes,
            true,
            true,
        ) == state,
{}

pub proof fn incomplete_completion_currentness_permanently_quarantines_v1(
    state: AdapterStateV1,
)
    requires valid_state_v1(state), state.phase == AdapterPhaseV1::Published,
    ensures {
        let quarantined = observe_completion_v1(
            state,
            state.binding.planned_ticket,
            state.binding.native,
            state.binding.host,
            state.binding.persistent_range,
            state.binding.host_range,
            state.binding.copy_bytes,
            true,
            false,
        );
        &&& valid_state_v1(quarantined)
        &&& quarantined.binding == state.binding
        &&& quarantined.phase == AdapterPhaseV1::Quarantined
        &&& quarantined.lease_phase == PersistentLeasePhaseV1::Quarantined
        &&& quarantined.native_location == NativeLocationV1::QuarantineCustody
        &&& quarantined.live_ticket == Some(state.binding.planned_ticket)
        &&& quarantined.authority_count == 1
        &&& quarantined.owner_quarantined
        &&& !can_release_v1(quarantined, false)
        &&& release_v1(quarantined, false) == quarantined
    },
{}

pub proof fn completion_settlement_and_restored_cancellation_are_exact_v1()
    ensures {
        let prepared = sample_prepared_v1(DirectionV1::HostToDevice);
        let published = confirm_publication_v1(prepared, prepared.binding.planned_ticket);
        let completed = observe_completion_v1(
            published,
            published.binding.planned_ticket,
            published.binding.native,
            published.binding.host,
            published.binding.persistent_range,
            published.binding.host_range,
            published.binding.copy_bytes,
            true,
            true,
        );
        let frontier_pending = settle_completion_v1(completed);
        let settled = retire_settled_frontier_v1(
            frontier_pending,
            exact_settled_frontier_v1(
                frontier_pending.binding,
                frontier_pending.frontier_generation,
            ),
        );
        let restored = restore_prepublication_v1(prepared);
        let cancelled = cancel_restored_v1(restored);
        &&& frontier_pending.phase == AdapterPhaseV1::SettledFrontierPending
        &&& frontier_pending.lease_phase == PersistentLeasePhaseV1::Settled
        &&& valid_state_v1(frontier_pending)
        &&& settled.phase == AdapterPhaseV1::Settled
        &&& settled.lease_phase == PersistentLeasePhaseV1::Settled
        &&& settled.settled_frontier.is_none()
        &&& valid_state_v1(settled)
        &&& cancelled.phase == AdapterPhaseV1::Cancelled
        &&& cancelled.lease_phase == PersistentLeasePhaseV1::Cancelled
        &&& valid_state_v1(cancelled)
    },
{}

pub proof fn settlement_creates_exact_frontier_and_blocks_release_v1(
    state: AdapterStateV1,
)
    requires
        valid_state_v1(state),
        state.phase == AdapterPhaseV1::CompletionRestored,
    ensures {
        let pending = settle_completion_v1(state);
        &&& valid_state_v1(pending)
        &&& pending.phase == AdapterPhaseV1::SettledFrontierPending
        &&& pending.frontier_generation == state.frontier_generation + 1
        &&& pending.settled_frontier
            == Some(exact_settled_frontier_v1(
                state.binding,
                state.frontier_generation + 1,
            ))
        &&& !can_release_v1(pending, false)
        &&& release_v1(pending, false) == pending
    },
{}

pub proof fn stale_or_substituted_frontier_is_rejected_atomically_v1(
    state: AdapterStateV1,
    stale: SettledFrontierV1,
)
    requires
        valid_state_v1(state),
        state.phase == AdapterPhaseV1::SettledFrontierPending,
        state.settled_frontier != Some(stale),
    ensures retire_settled_frontier_v1(state, stale) == state,
{}

pub proof fn exact_frontier_retirement_reuses_slot_v1(steps: nat)
    ensures {
        let summary = sequential_reuse_summary_v1(steps);
        &&& summary.occupied_slots == 0
        &&& summary.retired_uses == steps
    },
    decreases steps,
{
    if steps > 0 {
        exact_frontier_retirement_reuses_slot_v1((steps - 1) as nat);
    }
}

pub proof fn sixty_five_sequential_uses_do_not_exhaust_slots_v1()
    ensures {
        let summary = sequential_reuse_summary_v1(65);
        &&& summary.occupied_slots == 0
        &&& summary.occupied_slots < queue_slot_count_v1()
        &&& summary.retired_uses == 65
    },
{
    exact_frontier_retirement_reuses_slot_v1(65);
}

pub proof fn settled_or_cancelled_quiescent_owner_can_release_v1(
    state: AdapterStateV1,
)
    requires
        valid_state_v1(state),
        state.phase == AdapterPhaseV1::Settled || state.phase == AdapterPhaseV1::Cancelled,
    ensures {
        let released = release_v1(state, false);
        &&& released.phase == AdapterPhaseV1::Released
        &&& released.native_location == NativeLocationV1::Released
        &&& released.authority_count == 0
        &&& valid_state_v1(released)
    },
{}

pub proof fn outstanding_peer_use_blocks_otherwise_releasable_owner_v1(
    state: AdapterStateV1,
)
    requires
        valid_state_v1(state),
        state.phase == AdapterPhaseV1::Settled || state.phase == AdapterPhaseV1::Cancelled,
    ensures release_v1(state, true) == state,
{}

pub proof fn prepared_published_timeout_completed_and_quarantine_block_early_release_v1(
    state: AdapterStateV1,
)
    requires
        valid_state_v1(state),
        state.phase == AdapterPhaseV1::AdapterPrepared
            || state.phase == AdapterPhaseV1::PrepublicationRestored
            || state.phase == AdapterPhaseV1::Published
            || state.phase == AdapterPhaseV1::TimedOut
            || state.phase == AdapterPhaseV1::CompletionRestored
            || state.phase == AdapterPhaseV1::SettledFrontierPending
            || state.phase == AdapterPhaseV1::Quarantined,
    ensures
        !can_release_v1(state, false),
        release_v1(state, false) == state,
{}

pub proof fn permanent_quarantine_is_absorbing_v1(
    state: AdapterStateV1,
)
    requires valid_state_v1(state), state.phase == AdapterPhaseV1::Quarantined,
    ensures
        quarantine_preparation_currentness_v1(state) == state,
        restore_prepublication_v1(state) == state,
        confirm_publication_v1(state, state.binding.planned_ticket) == state,
        quarantine_indeterminate_publication_v1(state, state.binding.planned_ticket) == state,
        observe_pending_v1(state, state.binding.planned_ticket) == state,
        observe_timeout_v1(state, state.binding.planned_ticket) == state,
        observe_completion_v1(
            state,
            state.binding.planned_ticket,
            state.binding.native,
            state.binding.host,
            state.binding.persistent_range,
            state.binding.host_range,
            state.binding.copy_bytes,
            true,
            true,
        ) == state,
        settle_completion_v1(state) == state,
        retire_settled_frontier_v1(
            state,
            exact_settled_frontier_v1(state.binding, state.frontier_generation),
        ) == state,
        cancel_restored_v1(state) == state,
        release_v1(state, false) == state,
{}

}
