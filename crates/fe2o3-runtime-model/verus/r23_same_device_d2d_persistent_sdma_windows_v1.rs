// Independent bounded R23 same-device D2D persistent-SDMA window model. This
// is not a refinement of executable Rust, R17-R22, runtime/KFD, hardware,
// liveness, HIP/HSA behavior, or performance.
use vstd::prelude::*;

verus! {

pub open spec fn max_transfer_bytes_v1() -> nat { 256 * 1024 * 1024 }
pub open spec fn max_packet_bytes_v1() -> nat { 0x003f_ffe0 }
pub open spec fn ring_slots_v1() -> nat { 64 }
pub open spec fn max_window_packets_v1() -> nat { (ring_slots_v1() - 1) as nat }
pub open spec fn max_window_bytes_v1() -> nat {
    max_packet_bytes_v1() * max_window_packets_v1()
}
pub open spec fn max_u32_v1() -> nat { 0xffff_ffff }
pub open spec fn max_u64_v1() -> nat { 0xffff_ffff_ffff_ffff }
pub open spec fn native_h2d_engine_v1() -> nat { 0 }

#[derive(PartialEq, Eq)] pub struct DeviceV1 {
    pub physical: nat, pub generation: nat,
}
#[derive(PartialEq, Eq)] pub struct VmV1 {
    pub device: DeviceV1, pub identity: nat,
}
#[derive(PartialEq, Eq)] pub struct AllocationV1 {
    pub owner: nat, pub identity: nat, pub generation: nat, pub mapping: nat,
    pub backing: nat, pub vm: VmV1, pub attachment_generation: nat,
    pub pool_generation: nat, pub logical_bytes: nat, pub physical_bytes: nat,
    pub gpu_va_base: nat, pub incarnation: nat,
}
#[derive(PartialEq, Eq)] pub struct QueueV1 {
    pub vm: VmV1, pub logical_identity: nat, pub generation: nat,
    pub native_identity: nat, pub occurrence: nat, pub engine: nat,
}
#[derive(PartialEq, Eq)] pub struct RangeV1 {
    pub offset: nat, pub bytes: nat,
}
#[derive(PartialEq, Eq)] pub enum LeaseRoleV1 { SourceRead, DestinationWrite }
#[derive(PartialEq, Eq)] pub struct LeaseV1 {
    pub allocation: AllocationV1, pub role: LeaseRoleV1, pub range: RangeV1,
    pub generation: nat,
}
#[derive(PartialEq, Eq)] pub struct LeasePairV1 {
    pub source_read: LeaseV1, pub destination_write: LeaseV1,
}
#[derive(PartialEq, Eq)] pub struct TicketV1 {
    pub queue: QueueV1, pub slot: nat, pub generation: nat,
}
pub struct WindowV1 {
    pub transfer_id: nat, pub ordinal: nat, pub transfer_offset: nat,
    pub bytes: nat, pub packet_count: nat, pub final_packet_bytes: nat,
    pub first_slot: nat, pub prior_slot_generations: Seq<nat>,
    pub source: AllocationV1, pub destination: AllocationV1,
    pub source_range: RangeV1, pub destination_range: RangeV1,
    pub queue: QueueV1, pub leases: LeasePairV1,
}
pub struct AggregateCompletionV1 {
    pub window: WindowV1, pub tickets: Seq<TicketV1>,
    pub completion_values: Seq<nat>, pub aggregate_bytes: nat,
}
pub struct FrontierV1 {
    pub completion: AggregateCompletionV1,
}
#[derive(PartialEq, Eq)] pub enum PhaseV1 {
    DevicePairReady, Ready, Prepared, Published, TimedOut, FrontierPending,
    Completed, Quarantined,
}
#[derive(PartialEq, Eq)] pub enum CustodyV1 {
    DevicePair, ReadyPair, PreparedPair, PublishedPair, FrontierPair,
    QuarantinedPair,
}
pub struct StateV1 {
    pub phase: PhaseV1, pub custody: CustodyV1,
    pub source: AllocationV1, pub destination: AllocationV1, pub queue: QueueV1,
    pub transfer_id: nat, pub source_range: RangeV1, pub destination_range: RangeV1,
    pub total_bytes: nat, pub completed_bytes: nat, pub window_ordinal: nat,
    pub window: Option<WindowV1>, pub frontier: Option<FrontierV1>,
    pub observed_completed_packets: nat, pub next_ring_slot: nat,
    pub slot_generations: Seq<nat>, pub source_next_use_generation: nat,
    pub destination_next_use_generation: nat, pub published_windows: nat,
    pub published_packets: nat, pub write_pointer_publications: nat,
    pub doorbell_publications: nat, pub retired_windows: nat,
    pub destination_dirty_through: nat,
    pub destination_possibly_mutated_through: nat,
    pub source_authority_count: nat, pub destination_authority_count: nat,
    pub source_read_lease_count: nat, pub destination_write_lease_count: nat,
    pub target_retained: bool, pub current: bool,
    pub result_succeeded: bool, pub result_code: int,
}

pub open spec fn valid_allocation_v1(allocation: AllocationV1) -> bool {
    &&& allocation.owner > 0
    &&& allocation.identity > 0
    &&& allocation.generation > 0
    &&& allocation.mapping > 0
    &&& allocation.backing > 0
    &&& allocation.vm.device.generation > 0
    &&& allocation.vm.identity > 0
    &&& allocation.attachment_generation > 0
    &&& allocation.pool_generation > 0
    &&& allocation.incarnation > 0
    &&& 0 < allocation.logical_bytes <= allocation.physical_bytes
    &&& allocation.physical_bytes <= max_transfer_bytes_v1()
    &&& allocation.gpu_va_base > 0
    &&& allocation.gpu_va_base + allocation.physical_bytes <= max_u64_v1()
}
pub open spec fn mapped_extents_overlap_v1(left: AllocationV1, right: AllocationV1)
    -> bool
{
    left.gpu_va_base < right.gpu_va_base + right.physical_bytes
        && right.gpu_va_base < left.gpu_va_base + left.physical_bytes
}
pub open spec fn valid_allocation_pair_v1(source: AllocationV1,
    destination: AllocationV1) -> bool
{
    &&& valid_allocation_v1(source)
    &&& valid_allocation_v1(destination)
    &&& source.vm == destination.vm
    &&& source.owner != destination.owner
    &&& source.identity != destination.identity
    &&& source.mapping != destination.mapping
    &&& source.backing != destination.backing
    &&& source.incarnation != destination.incarnation
    &&& !mapped_extents_overlap_v1(source, destination)
}
pub open spec fn valid_queue_v1(queue: QueueV1, vm: VmV1) -> bool {
    queue.vm == vm && queue.logical_identity > 0 && queue.generation > 0
        && queue.native_identity < 1024 && queue.occurrence > 0
        && queue.engine == native_h2d_engine_v1()
}
pub open spec fn valid_range_v1(allocation: AllocationV1, range: RangeV1) -> bool {
    range.bytes > 0 && range.offset + range.bytes <= allocation.logical_bytes
        && range.offset + range.bytes <= allocation.physical_bytes
        && range.offset + range.bytes <= max_u64_v1()
}
pub open spec fn ranges_are_nonaliased_v1(source: AllocationV1, source_range: RangeV1,
    destination: AllocationV1, destination_range: RangeV1) -> bool
{
    source.backing != destination.backing
        && (source.gpu_va_base + source_range.offset + source_range.bytes
                <= destination.gpu_va_base + destination_range.offset
            || destination.gpu_va_base + destination_range.offset
                    + destination_range.bytes
                <= source.gpu_va_base + source_range.offset)
}
pub open spec fn packet_count_v1(bytes: nat) -> nat {
    if bytes == 0 { 0 }
    else { ((bytes - 1) as nat) / max_packet_bytes_v1() + 1 }
}
pub open spec fn window_bytes_v1(remaining: nat) -> nat {
    if remaining <= max_window_bytes_v1() { remaining } else { max_window_bytes_v1() }
}
pub open spec fn final_packet_bytes_v1(bytes: nat) -> nat {
    if bytes == 0 { 0 }
    else {
        (bytes - ((packet_count_v1(bytes) - 1) as nat) * max_packet_bytes_v1()) as nat
    }
}
pub open spec fn packet_transfer_offset_v1(window: WindowV1, index: nat) -> nat {
    index * max_packet_bytes_v1()
}
pub open spec fn packet_bytes_v1(window: WindowV1, index: nat) -> nat {
    if index + 1 == window.packet_count {
        window.final_packet_bytes
    } else {
        max_packet_bytes_v1()
    }
}
pub open spec fn packet_source_range_v1(window: WindowV1, index: nat) -> RangeV1 {
    RangeV1 { offset: window.source_range.offset + packet_transfer_offset_v1(window, index),
        bytes: packet_bytes_v1(window, index) }
}
pub open spec fn packet_destination_range_v1(window: WindowV1, index: nat) -> RangeV1 {
    RangeV1 { offset: window.destination_range.offset
            + packet_transfer_offset_v1(window, index),
        bytes: packet_bytes_v1(window, index) }
}
pub open spec fn valid_slot_generations_v1(generations: Seq<nat>) -> bool {
    generations.len() == ring_slots_v1()
        && forall|slot: int| 0 <= slot < generations.len()
            ==> generations[slot] <= max_u32_v1()
}
pub open spec fn slot_distance_v1(first_slot: nat, slot: nat) -> nat {
    if first_slot <= slot { (slot - first_slot) as nat }
    else { (ring_slots_v1() - first_slot + slot) as nat }
}
pub open spec fn window_selects_slot_v1(window: WindowV1, slot: nat) -> bool {
    slot < ring_slots_v1()
        && slot_distance_v1(window.first_slot, slot) < window.packet_count
}
pub open spec fn ticket_at_v1(window: WindowV1, index: nat) -> TicketV1 {
    let slot = (window.first_slot + index) % ring_slots_v1();
    TicketV1 { queue: window.queue, slot,
        generation: window.prior_slot_generations[slot as int] + 1 }
}
pub open spec fn committed_slot_generations_v1(window: WindowV1) -> Seq<nat> {
    Seq::new(ring_slots_v1(), |slot: int|
        if window_selects_slot_v1(window, slot as nat) {
            window.prior_slot_generations[slot] + 1
        } else { window.prior_slot_generations[slot] })
}
pub open spec fn exact_source_lease_v1(window: WindowV1) -> LeaseV1 {
    LeaseV1 { allocation: window.source, role: LeaseRoleV1::SourceRead,
        range: window.source_range, generation: window.leases.source_read.generation }
}
pub open spec fn exact_destination_lease_v1(window: WindowV1) -> LeaseV1 {
    LeaseV1 { allocation: window.destination, role: LeaseRoleV1::DestinationWrite,
        range: window.destination_range,
        generation: window.leases.destination_write.generation }
}
pub open spec fn valid_window_v1(window: WindowV1) -> bool {
    &&& window.transfer_id > 0
    &&& 0 < window.bytes <= max_window_bytes_v1()
    &&& window.packet_count == packet_count_v1(window.bytes)
    &&& 0 < window.packet_count <= max_window_packets_v1()
    &&& window.final_packet_bytes == final_packet_bytes_v1(window.bytes)
    &&& 0 < window.final_packet_bytes <= max_packet_bytes_v1()
    &&& window.bytes == (window.packet_count - 1) * max_packet_bytes_v1()
        + window.final_packet_bytes
    &&& window.first_slot < ring_slots_v1()
    &&& valid_slot_generations_v1(window.prior_slot_generations)
    &&& forall|slot: int| 0 <= slot < window.prior_slot_generations.len()
        && window_selects_slot_v1(window, slot as nat) ==>
        #[trigger] window.prior_slot_generations[slot] < max_u32_v1()
    &&& valid_allocation_pair_v1(window.source, window.destination)
    &&& valid_queue_v1(window.queue, window.source.vm)
    &&& valid_range_v1(window.source, window.source_range)
    &&& valid_range_v1(window.destination, window.destination_range)
    &&& window.source_range.bytes == window.bytes
    &&& window.destination_range.bytes == window.bytes
    &&& ranges_are_nonaliased_v1(window.source, window.source_range,
        window.destination, window.destination_range)
    &&& window.leases.source_read == exact_source_lease_v1(window)
    &&& window.leases.destination_write == exact_destination_lease_v1(window)
    &&& window.leases.source_read.generation > 0
    &&& window.leases.destination_write.generation > 0
}
pub open spec fn exact_completion_for_window_v1(window: WindowV1)
    -> AggregateCompletionV1
{
    AggregateCompletionV1 {
        window,
        tickets: Seq::new(window.packet_count as nat,
            |index: int| ticket_at_v1(window, index as nat)),
        completion_values: Seq::new(window.packet_count as nat,
            |index: int| ticket_at_v1(window, index as nat).generation),
        aggregate_bytes: window.bytes,
    }
}
pub open spec fn authenticated_completion_v1(window: WindowV1,
    completion: AggregateCompletionV1) -> bool
{
    &&& completion.window == window
    &&& completion.aggregate_bytes == window.bytes
    &&& completion.tickets.len() == window.packet_count
    &&& completion.completion_values.len() == window.packet_count
    &&& forall|index: int| 0 <= index < window.packet_count ==>
        #[trigger] completion.tickets[index] == ticket_at_v1(window, index as nat)
    &&& forall|index: int| 0 <= index < window.packet_count ==>
        #[trigger] completion.completion_values[index]
            == ticket_at_v1(window, index as nat).generation
}
pub open spec fn planned_window_v1(state: StateV1) -> WindowV1 {
    let bytes = window_bytes_v1((state.total_bytes - state.completed_bytes) as nat);
    WindowV1 {
        transfer_id: state.transfer_id, ordinal: state.window_ordinal,
        transfer_offset: state.completed_bytes, bytes,
        packet_count: packet_count_v1(bytes),
        final_packet_bytes: final_packet_bytes_v1(bytes),
        first_slot: state.next_ring_slot,
        prior_slot_generations: state.slot_generations,
        source: state.source, destination: state.destination,
        source_range: RangeV1 { offset: state.source_range.offset + state.completed_bytes,
            bytes },
        destination_range: RangeV1 {
            offset: state.destination_range.offset + state.completed_bytes, bytes },
        queue: state.queue,
        leases: LeasePairV1 {
            source_read: LeaseV1 { allocation: state.source,
                role: LeaseRoleV1::SourceRead,
                range: RangeV1 { offset: state.source_range.offset + state.completed_bytes,
                    bytes }, generation: state.source_next_use_generation },
            destination_write: LeaseV1 { allocation: state.destination,
                role: LeaseRoleV1::DestinationWrite,
                range: RangeV1 {
                    offset: state.destination_range.offset + state.completed_bytes, bytes },
                generation: state.destination_next_use_generation },
        },
    }
}
pub open spec fn exact_frontier_v1(state: StateV1) -> FrontierV1 {
    FrontierV1 { completion: exact_completion_for_window_v1(state.window.unwrap()) }
}
pub open spec fn base_valid_v1(state: StateV1) -> bool {
    &&& valid_allocation_pair_v1(state.source, state.destination)
    &&& valid_queue_v1(state.queue, state.source.vm)
    &&& state.transfer_id > 0
    &&& state.total_bytes > 0
    &&& state.source_range.bytes == state.total_bytes
    &&& state.destination_range.bytes == state.total_bytes
    &&& valid_range_v1(state.source, state.source_range)
    &&& valid_range_v1(state.destination, state.destination_range)
    &&& ranges_are_nonaliased_v1(state.source, state.source_range,
        state.destination, state.destination_range)
    &&& state.completed_bytes <= state.total_bytes
    &&& state.destination_dirty_through == state.completed_bytes
    &&& state.completed_bytes <= state.destination_possibly_mutated_through
        <= state.total_bytes
    &&& state.next_ring_slot < ring_slots_v1()
    &&& valid_slot_generations_v1(state.slot_generations)
    &&& state.source_next_use_generation > 0
    &&& state.destination_next_use_generation > 0
    &&& state.write_pointer_publications == state.published_windows
    &&& state.doorbell_publications == state.published_windows
    &&& state.retired_windows <= state.published_windows
    &&& state.source_authority_count == 1
    &&& state.destination_authority_count == 1
    &&& state.source_read_lease_count == state.destination_write_lease_count
}
pub open spec fn window_identity_matches_state_v1(state: StateV1, window: WindowV1) -> bool {
    &&& window.transfer_id == state.transfer_id
    &&& window.ordinal == state.window_ordinal
    &&& window.transfer_offset == state.completed_bytes
    &&& window.transfer_offset + window.bytes <= state.total_bytes
    &&& window.first_slot == state.next_ring_slot
    &&& window.source == state.source
    &&& window.destination == state.destination
    &&& window.queue == state.queue
}
pub open spec fn planned_window_matches_state_v1(state: StateV1, window: WindowV1) -> bool {
    &&& window_identity_matches_state_v1(state, window)
    &&& window.leases.source_read.generation == state.source_next_use_generation
    &&& window.leases.destination_write.generation
        == state.destination_next_use_generation
}
pub open spec fn reserved_window_matches_state_v1(state: StateV1, window: WindowV1) -> bool {
    &&& window.leases.source_read.generation + 1 == state.source_next_use_generation
    &&& window.leases.destination_write.generation + 1
        == state.destination_next_use_generation
}
pub open spec fn valid_state_v1(state: StateV1) -> bool {
    &&& base_valid_v1(state)
    &&& match state.phase {
        PhaseV1::DevicePairReady => state.custody == CustodyV1::DevicePair
            && state.window.is_none() && state.frontier.is_none()
            && state.source_read_lease_count == 0 && !state.target_retained
            && state.current,
        PhaseV1::Ready => state.custody == CustodyV1::ReadyPair
            && state.completed_bytes < state.total_bytes && state.window.is_none()
            && state.frontier.is_none() && state.source_read_lease_count == 0
            && state.target_retained && state.current && !state.result_succeeded
            && state.result_code == 0,
        PhaseV1::Prepared => state.custody == CustodyV1::PreparedPair
            && state.window.is_some() && valid_window_v1(state.window.unwrap())
            && window_identity_matches_state_v1(state, state.window.unwrap())
            && reserved_window_matches_state_v1(state, state.window.unwrap())
            && state.window.unwrap().prior_slot_generations == state.slot_generations
            && state.frontier.is_none() && state.observed_completed_packets == 0
            && state.source_read_lease_count == 1 && state.target_retained
            && state.current && !state.result_succeeded && state.result_code == 0,
        PhaseV1::Published | PhaseV1::TimedOut =>
            state.custody == CustodyV1::PublishedPair
            && state.window.is_some() && valid_window_v1(state.window.unwrap())
            && state.window.unwrap().transfer_id == state.transfer_id
            && state.window.unwrap().ordinal == state.window_ordinal
            && state.window.unwrap().transfer_offset == state.completed_bytes
            && state.window.unwrap().source == state.source
            && state.window.unwrap().destination == state.destination
            && state.window.unwrap().queue == state.queue
            && reserved_window_matches_state_v1(state, state.window.unwrap())
            && state.slot_generations
                == committed_slot_generations_v1(state.window.unwrap())
            && state.frontier.is_none()
            && state.observed_completed_packets < state.window.unwrap().packet_count
            && state.destination_possibly_mutated_through
                == state.window.unwrap().transfer_offset + state.window.unwrap().bytes
            && state.source_read_lease_count == 1 && state.target_retained
            && state.current && !state.result_succeeded && state.result_code == 0,
        PhaseV1::FrontierPending => state.custody == CustodyV1::FrontierPair
            && state.window.is_some() && valid_window_v1(state.window.unwrap())
            && reserved_window_matches_state_v1(state, state.window.unwrap())
            && state.window.unwrap().transfer_id == state.transfer_id
            && state.window.unwrap().ordinal == state.window_ordinal
            && state.window.unwrap().transfer_offset == state.completed_bytes
            && state.window.unwrap().source == state.source
            && state.window.unwrap().destination == state.destination
            && state.window.unwrap().queue == state.queue
            && state.frontier.is_some()
            && authenticated_completion_v1(state.window.unwrap(),
                state.frontier.unwrap().completion)
            && state.observed_completed_packets == state.window.unwrap().packet_count
            && state.destination_possibly_mutated_through
                == state.window.unwrap().transfer_offset + state.window.unwrap().bytes
            && state.source_read_lease_count == 1 && state.target_retained
            && state.current && !state.result_succeeded && state.result_code == 0,
        PhaseV1::Completed => state.custody == CustodyV1::DevicePair
            && state.window.is_none() && state.frontier.is_none()
            && state.source_read_lease_count == 0 && state.target_retained
            && state.current
            && ((state.result_succeeded && state.result_code == 0
                    && state.completed_bytes == state.total_bytes)
                || (!state.result_succeeded && state.result_code < 0)),
        PhaseV1::Quarantined => state.custody == CustodyV1::QuarantinedPair
            && state.target_retained && !state.current
            && state.source_read_lease_count <= 1,
    }
}

pub open spec fn prepare_window_v1(state: StateV1) -> StateV1 {
    let window = planned_window_v1(state);
    if state.phase == PhaseV1::Ready && valid_state_v1(state)
        && valid_window_v1(window) && planned_window_matches_state_v1(state, window) {
        StateV1 { phase: PhaseV1::Prepared, custody: CustodyV1::PreparedPair,
            window: Some(window), observed_completed_packets: 0,
            source_read_lease_count: 1, destination_write_lease_count: 1,
            source_next_use_generation: state.source_next_use_generation + 1,
            destination_next_use_generation: state.destination_next_use_generation + 1,
            ..state }
    } else { state }
}
pub open spec fn destination_reservation_failure_v1(state: StateV1) -> StateV1 {
    if state.phase == PhaseV1::Ready && valid_state_v1(state) {
        StateV1 { source_next_use_generation: state.source_next_use_generation + 1, ..state }
    } else { state }
}
pub open spec fn retryable_prepublication_v1(state: StateV1) -> StateV1 {
    if state.phase == PhaseV1::Prepared && valid_state_v1(state) {
        StateV1 { phase: PhaseV1::Ready, custody: CustodyV1::ReadyPair,
            window: None, source_read_lease_count: 0,
            destination_write_lease_count: 0, ..state }
    } else { state }
}
pub open spec fn publish_window_v1(state: StateV1) -> StateV1 {
    if state.phase == PhaseV1::Prepared && valid_state_v1(state) {
        let window = state.window.unwrap();
        StateV1 { phase: PhaseV1::Published, custody: CustodyV1::PublishedPair,
            next_ring_slot: (state.next_ring_slot + window.packet_count) % ring_slots_v1(),
            slot_generations: committed_slot_generations_v1(window),
            published_windows: state.published_windows + 1,
            published_packets: state.published_packets + window.packet_count,
            write_pointer_publications: state.write_pointer_publications + 1,
            doorbell_publications: state.doorbell_publications + 1,
            destination_possibly_mutated_through:
                window.transfer_offset + window.bytes, ..state }
    } else { state }
}
pub open spec fn poll_pending_v1(state: StateV1) -> StateV1 { state }
pub open spec fn poll_timeout_v1(state: StateV1) -> StateV1 {
    if (state.phase == PhaseV1::Published || state.phase == PhaseV1::TimedOut)
        && valid_state_v1(state) {
        StateV1 { phase: PhaseV1::TimedOut, ..state }
    } else { state }
}
pub open spec fn observe_incomplete_v1(state: StateV1, completed_packets: nat)
    -> StateV1
{
    if (state.phase == PhaseV1::Published || state.phase == PhaseV1::TimedOut)
        && valid_state_v1(state)
        && state.observed_completed_packets <= completed_packets
        && 0 < completed_packets < state.window.unwrap().packet_count {
        StateV1 { observed_completed_packets: completed_packets, ..state }
    } else { state }
}
pub open spec fn quarantine_v1(state: StateV1) -> StateV1 {
    if state.phase != PhaseV1::Quarantined && valid_state_v1(state) {
        StateV1 { phase: PhaseV1::Quarantined, custody: CustodyV1::QuarantinedPair,
            target_retained: true, current: false, ..state }
    } else { state }
}
pub open spec fn complete_window_v1(state: StateV1,
    completion: AggregateCompletionV1) -> StateV1
{
    if (state.phase == PhaseV1::Published || state.phase == PhaseV1::TimedOut)
        && valid_state_v1(state) {
        if authenticated_completion_v1(state.window.unwrap(), completion) {
            StateV1 { phase: PhaseV1::FrontierPending,
                custody: CustodyV1::FrontierPair,
                frontier: Some(FrontierV1 { completion }),
                observed_completed_packets: state.window.unwrap().packet_count, ..state }
        } else { quarantine_v1(state) }
    } else { state }
}
pub open spec fn retire_window_v1(state: StateV1, frontier_matches: bool) -> StateV1 {
    if state.phase == PhaseV1::FrontierPending && valid_state_v1(state) {
        if !frontier_matches { quarantine_v1(state) }
        else {
            let window = state.window.unwrap();
            let through = state.completed_bytes + window.bytes;
            StateV1 {
                phase: if through == state.total_bytes { PhaseV1::Completed }
                    else { PhaseV1::Ready },
                custody: if through == state.total_bytes { CustodyV1::DevicePair }
                    else { CustodyV1::ReadyPair },
                completed_bytes: through, window_ordinal: state.window_ordinal + 1,
                window: None, frontier: None, observed_completed_packets: 0,
                source_read_lease_count: 0, destination_write_lease_count: 0,
                retired_windows: state.retired_windows + 1,
                destination_dirty_through: through,
                result_succeeded: through == state.total_bytes, result_code: 0, ..state
            }
        }
    } else { state }
}
pub open spec fn cancel_v1(state: StateV1) -> StateV1 {
    if state.phase == PhaseV1::Ready && state.completed_bytes == 0 {
        StateV1 { phase: PhaseV1::DevicePairReady, custody: CustodyV1::DevicePair,
            target_retained: false, ..state }
    } else { state }
}
pub open spec fn release_terminal_v1(state: StateV1, transfer_id: nat) -> StateV1 {
    if state.phase == PhaseV1::Completed && state.transfer_id == transfer_id
        && state.target_retained {
        StateV1 { phase: PhaseV1::DevicePairReady, custody: CustodyV1::DevicePair,
            target_retained: false, result_succeeded: false, result_code: 0, ..state }
    } else { state }
}

pub open spec fn sample_device_v1() -> DeviceV1 {
    DeviceV1 { physical: 1, generation: 2 }
}
pub open spec fn sample_vm_v1() -> VmV1 {
    VmV1 { device: sample_device_v1(), identity: 3 }
}
pub open spec fn sample_source_v1() -> AllocationV1 {
    AllocationV1 { owner: 4, identity: 5, generation: 6, mapping: 7,
        backing: 8, vm: sample_vm_v1(), attachment_generation: 9,
        pool_generation: 10, logical_bytes: max_transfer_bytes_v1(),
        physical_bytes: max_transfer_bytes_v1(), gpu_va_base: 4096,
        incarnation: 11 }
}
pub open spec fn sample_destination_v1() -> AllocationV1 {
    AllocationV1 { owner: 12, identity: 13, generation: 14, mapping: 15,
        backing: 16, vm: sample_vm_v1(), attachment_generation: 17,
        pool_generation: 18, logical_bytes: max_transfer_bytes_v1(),
        physical_bytes: max_transfer_bytes_v1(),
        gpu_va_base: max_transfer_bytes_v1() + 8192, incarnation: 19 }
}
pub open spec fn sample_queue_v1() -> QueueV1 {
    QueueV1 { vm: sample_vm_v1(), logical_identity: 20, generation: 21,
        native_identity: 4, occurrence: 22, engine: 0 }
}
pub open spec fn sample_ready_v1() -> StateV1 {
    StateV1 { phase: PhaseV1::Ready, custody: CustodyV1::ReadyPair,
        source: sample_source_v1(), destination: sample_destination_v1(),
        queue: sample_queue_v1(), transfer_id: 23,
        source_range: RangeV1 { offset: 0, bytes: max_transfer_bytes_v1() },
        destination_range: RangeV1 { offset: 0, bytes: max_transfer_bytes_v1() },
        total_bytes: max_transfer_bytes_v1(), completed_bytes: 0,
        window_ordinal: 0, window: None, frontier: None,
        observed_completed_packets: 0, next_ring_slot: 0,
        slot_generations: Seq::new(ring_slots_v1(), |_slot: int| 0),
        source_next_use_generation: 24, destination_next_use_generation: 25,
        published_windows: 0, published_packets: 0,
        write_pointer_publications: 0, doorbell_publications: 0,
        retired_windows: 0, destination_dirty_through: 0,
        destination_possibly_mutated_through: 0,
        source_authority_count: 1, destination_authority_count: 1,
        source_read_lease_count: 0, destination_write_lease_count: 0,
        target_retained: true, current: true,
        result_succeeded: false, result_code: 0 }
}
pub open spec fn sample_wrap_generations_v1() -> Seq<nat> {
    Seq::new(ring_slots_v1(), |slot: int| if slot == 0 { 1 } else { 0 })
}
pub open spec fn sample_wrap_window_v1() -> WindowV1 {
    let base = planned_window_v1(sample_ready_v1());
    let bytes = max_packet_bytes_v1() + 1;
    let source_range = RangeV1 { offset: 0, bytes };
    let destination_range = RangeV1 { offset: 0, bytes };
    WindowV1 { ordinal: 1, transfer_offset: max_window_bytes_v1(), bytes,
        packet_count: 2, final_packet_bytes: 1, first_slot: 63,
        prior_slot_generations: sample_wrap_generations_v1(), source_range,
        destination_range, leases: LeasePairV1 {
            source_read: LeaseV1 { range: source_range, generation: 26,
                ..base.leases.source_read },
            destination_write: LeaseV1 { range: destination_range, generation: 27,
                ..base.leases.destination_write },
        }, ..base }
}

pub proof fn fixed_d2d_window_bounds_v1()
    ensures max_transfer_bytes_v1() == 268435456,
        max_packet_bytes_v1() == 4194272, ring_slots_v1() == 64,
        max_window_packets_v1() == 63, max_window_bytes_v1() == 264239136,
        max_u32_v1() == 4294967295, {}
pub proof fn sample_same_device_pair_is_valid_v1()
    ensures valid_allocation_pair_v1(sample_source_v1(), sample_destination_v1()),
        valid_queue_v1(sample_queue_v1(), sample_vm_v1()),
        valid_state_v1(sample_ready_v1()), {}
pub proof fn allocation_alias_is_rejected_v1(source: AllocationV1)
    requires valid_allocation_v1(source),
    ensures !valid_allocation_pair_v1(source, source), {}
pub proof fn cross_vm_pair_is_rejected_v1(source: AllocationV1,
    destination: AllocationV1)
    requires valid_allocation_v1(source), valid_allocation_v1(destination),
        source.vm != destination.vm,
    ensures !valid_allocation_pair_v1(source, destination), {}
pub proof fn mapped_overlap_is_rejected_v1(source: AllocationV1,
    destination: AllocationV1)
    requires mapped_extents_overlap_v1(source, destination),
    ensures !valid_allocation_pair_v1(source, destination), {}
pub proof fn endpoint_range_overflow_is_rejected_v1(allocation: AllocationV1)
    requires valid_allocation_v1(allocation),
    ensures !valid_range_v1(allocation,
        RangeV1 { offset: allocation.logical_bytes, bytes: 1 }), {}
pub proof fn planned_leases_bind_exact_roles_and_ranges_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Ready,
    ensures {
        let window = planned_window_v1(state);
        &&& window.leases.source_read.role == LeaseRoleV1::SourceRead
        &&& window.leases.destination_write.role == LeaseRoleV1::DestinationWrite
        &&& window.leases.source_read.allocation == state.source
        &&& window.leases.destination_write.allocation == state.destination
        &&& window.leases.source_read.range == window.source_range
        &&& window.leases.destination_write.range == window.destination_range
    }, {}
pub proof fn planned_window_packet_count_is_bounded_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Ready,
        valid_window_v1(planned_window_v1(state)),
    ensures 0 < planned_window_v1(state).packet_count <= max_window_packets_v1(), {}
pub proof fn planned_window_has_exact_packet_roster_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Ready,
        valid_window_v1(planned_window_v1(state)),
    ensures planned_window_v1(state).packet_count
        == packet_count_v1(planned_window_v1(state).bytes), {}
pub proof fn packets_cover_exact_source_and_destination_v1(window: WindowV1)
    requires valid_window_v1(window),
    ensures window.bytes == (window.packet_count - 1) * max_packet_bytes_v1()
            + window.final_packet_bytes,
        window.source_range.bytes == window.bytes,
        window.destination_range.bytes == window.bytes, {}
pub proof fn source_packets_are_contiguous_v1(window: WindowV1, index: nat)
    requires valid_window_v1(window), index + 1 < window.packet_count,
    ensures packet_source_range_v1(window, index).offset
            + packet_source_range_v1(window, index).bytes
        == packet_source_range_v1(window, index + 1).offset, {}
pub proof fn destination_packets_are_contiguous_v1(window: WindowV1, index: nat)
    requires valid_window_v1(window), index + 1 < window.packet_count,
    ensures packet_destination_range_v1(window, index).offset
            + packet_destination_range_v1(window, index).bytes
        == packet_destination_range_v1(window, index + 1).offset, {}
pub proof fn packet_source_destination_lengths_match_v1(window: WindowV1, index: nat)
    requires valid_window_v1(window), index < window.packet_count,
    ensures packet_source_range_v1(window, index).bytes
        == packet_destination_range_v1(window, index).bytes, {}
pub proof fn tickets_bind_exact_queue_and_slot_generation_v1(window: WindowV1,
    index: nat)
    requires valid_window_v1(window), index < window.packet_count,
    ensures ticket_at_v1(window, index).queue == window.queue,
        ticket_at_v1(window, index).slot < ring_slots_v1(),
        ticket_at_v1(window, index).generation
            == window.prior_slot_generations[ticket_at_v1(window, index).slot as int] + 1,
        0 < ticket_at_v1(window, index).generation <= max_u32_v1(),
{
    let slot = (window.first_slot + index) % ring_slots_v1();
    assert(slot < ring_slots_v1());
    assert(window_selects_slot_v1(window, slot));
}
pub proof fn planned_window_slots_are_unique_v1(window: WindowV1, i: nat, j: nat)
    requires valid_window_v1(window), i < window.packet_count,
        j < window.packet_count, i != j,
    ensures ticket_at_v1(window, i).slot != ticket_at_v1(window, j).slot,
{
    assert(i < ring_slots_v1());
    assert(j < ring_slots_v1());
    assert((window.first_slot + i) % ring_slots_v1()
        != (window.first_slot + j) % ring_slots_v1()) by (nonlinear_arith)
        requires window.first_slot < ring_slots_v1(), i < ring_slots_v1(),
            j < ring_slots_v1(), i != j;
}
pub proof fn wrapped_window_uses_independent_slot_generations_v1()
    ensures {
        let window = sample_wrap_window_v1();
        let committed = committed_slot_generations_v1(window);
        &&& valid_window_v1(window)
        &&& ticket_at_v1(window, 0).slot == 63
        &&& ticket_at_v1(window, 0).generation == 1
        &&& ticket_at_v1(window, 1).slot == 0
        &&& ticket_at_v1(window, 1).generation == 2
        &&& committed[63] == 1 && committed[0] == 2 && committed[1] == 0
    }, {}
pub proof fn preparation_has_no_publication_or_dirty_effect_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Ready,
    ensures {
        let prepared = prepare_window_v1(state);
        &&& prepared.published_windows == state.published_windows
        &&& prepared.write_pointer_publications == state.write_pointer_publications
        &&& prepared.doorbell_publications == state.doorbell_publications
        &&& prepared.slot_generations == state.slot_generations
        &&& prepared.destination_dirty_through == state.destination_dirty_through
    }, {}
pub proof fn paired_reservation_consumes_both_use_generations_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Ready,
        valid_window_v1(planned_window_v1(state)),
        planned_window_matches_state_v1(state, planned_window_v1(state)),
    ensures prepare_window_v1(state).source_next_use_generation
            == state.source_next_use_generation + 1,
        prepare_window_v1(state).destination_next_use_generation
            == state.destination_next_use_generation + 1, {}
pub proof fn destination_reservation_failure_consumes_only_source_generation_v1(
    state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Ready,
    ensures destination_reservation_failure_v1(state).phase == PhaseV1::Ready,
        destination_reservation_failure_v1(state).custody == CustodyV1::ReadyPair,
        destination_reservation_failure_v1(state).source_next_use_generation
            == state.source_next_use_generation + 1,
        destination_reservation_failure_v1(state).destination_next_use_generation
            == state.destination_next_use_generation, {}
pub proof fn clean_retry_restores_exact_pair_with_consumed_generations_v1(
    state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Prepared,
    ensures {
        let restored = retryable_prepublication_v1(state);
        &&& restored.phase == PhaseV1::Ready
        &&& restored.custody == CustodyV1::ReadyPair
        &&& restored.source_authority_count == 1
        &&& restored.destination_authority_count == 1
        &&& restored.source_read_lease_count == 0
        &&& restored.destination_write_lease_count == 0
        &&& restored.slot_generations == state.slot_generations
        &&& restored.source_next_use_generation == state.source_next_use_generation
        &&& restored.destination_next_use_generation
            == state.destination_next_use_generation
    }, {}
pub proof fn confirmed_publication_preserves_validity_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Prepared,
    ensures valid_state_v1(publish_window_v1(state)),
{
    let window = state.window.unwrap();
    let committed = committed_slot_generations_v1(window);
    assert(committed.len() == ring_slots_v1());
    assert forall|slot: int| 0 <= slot < committed.len() implies
        #[trigger] committed[slot] <= max_u32_v1() by {
        assert(0 <= slot < window.prior_slot_generations.len());
        if window_selects_slot_v1(window, slot as nat) {
            assert(window.prior_slot_generations[slot] < max_u32_v1());
        } else {
            assert(window.prior_slot_generations[slot] <= max_u32_v1());
        }
    }
    assert(valid_slot_generations_v1(committed));
}
pub proof fn publication_commits_exact_packet_count_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Prepared,
    ensures publish_window_v1(state).published_packets
        == state.published_packets + state.window.unwrap().packet_count, {}
pub proof fn publication_has_one_write_pointer_update_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Prepared,
    ensures publish_window_v1(state).write_pointer_publications
        == state.write_pointer_publications + 1, {}
pub proof fn publication_has_one_doorbell_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Prepared,
    ensures publish_window_v1(state).doorbell_publications
        == state.doorbell_publications + 1, {}
pub proof fn publication_retains_both_typed_leases_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Prepared,
    ensures publish_window_v1(state).custody == CustodyV1::PublishedPair,
        publish_window_v1(state).source_read_lease_count == 1,
        publish_window_v1(state).destination_write_lease_count == 1, {}
pub proof fn publication_marks_possible_but_not_authenticated_dirty_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Prepared,
    ensures publish_window_v1(state).destination_dirty_through
            == state.destination_dirty_through,
        publish_window_v1(state).destination_possibly_mutated_through
            == state.window.unwrap().transfer_offset + state.window.unwrap().bytes, {}
pub proof fn pending_poll_is_observation_only_v1(state: StateV1)
    ensures poll_pending_v1(state) == state, {}
pub proof fn timeout_retains_exact_pair_and_leases_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Published,
    ensures poll_timeout_v1(state).phase == PhaseV1::TimedOut,
        poll_timeout_v1(state).custody == CustodyV1::PublishedPair,
        poll_timeout_v1(state).source_authority_count == 1,
        poll_timeout_v1(state).destination_authority_count == 1,
        poll_timeout_v1(state).source_read_lease_count == 1,
        poll_timeout_v1(state).destination_write_lease_count == 1, {}
pub proof fn timed_out_window_accepts_exact_repoll_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::TimedOut,
    ensures complete_window_v1(state,
            exact_completion_for_window_v1(state.window.unwrap())).phase
        == PhaseV1::FrontierPending, {}
pub proof fn incomplete_aggregate_has_no_prefix_retirement_v1(state: StateV1,
    completed_packets: nat)
    requires valid_state_v1(state), state.phase == PhaseV1::Published,
        state.observed_completed_packets <= completed_packets,
        0 < completed_packets < state.window.unwrap().packet_count,
    ensures {
        let partial = observe_incomplete_v1(state, completed_packets);
        &&& partial.phase == PhaseV1::Published
        &&& partial.custody == CustodyV1::PublishedPair
        &&& partial.frontier.is_none()
        &&& partial.retired_windows == state.retired_windows
        &&& partial.completed_bytes == state.completed_bytes
        &&& partial.destination_dirty_through == state.destination_dirty_through
    }, {}
pub proof fn exact_completion_metadata_authenticates_whole_roster_v1(window: WindowV1)
    requires valid_window_v1(window),
    ensures authenticated_completion_v1(window, exact_completion_for_window_v1(window)), {}
pub proof fn unauthenticated_completion_quarantines_without_dirty_progress_v1(
    state: StateV1, completion: AggregateCompletionV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Published,
        !authenticated_completion_v1(state.window.unwrap(), completion),
    ensures complete_window_v1(state, completion).phase
            == PhaseV1::Quarantined,
        complete_window_v1(state, completion)
                .destination_dirty_through == state.destination_dirty_through, {}
pub proof fn authenticated_completion_creates_exact_full_frontier_v1(state: StateV1)
    requires valid_state_v1(state),
        state.phase == PhaseV1::Published || state.phase == PhaseV1::TimedOut,
    ensures {
        let completion = exact_completion_for_window_v1(state.window.unwrap());
        let completed = complete_window_v1(state, completion);
        &&& completed.phase == PhaseV1::FrontierPending
        &&& completed.frontier == Some(FrontierV1 { completion })
        &&& completed.observed_completed_packets == state.window.unwrap().packet_count
        &&& completed.destination_dirty_through == state.destination_dirty_through
    }, {}
pub proof fn stale_frontier_quarantines_exact_pair_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::FrontierPending,
    ensures retire_window_v1(state, false).phase == PhaseV1::Quarantined,
        retire_window_v1(state, false).source_authority_count == 1,
        retire_window_v1(state, false).destination_authority_count == 1,
        retire_window_v1(state, false).source_read_lease_count == 1,
        retire_window_v1(state, false).destination_write_lease_count == 1, {}
pub proof fn exact_successful_retirement_advances_full_window_dirty_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::FrontierPending,
    ensures retire_window_v1(state, true).completed_bytes
            == state.completed_bytes + state.window.unwrap().bytes,
        retire_window_v1(state, true).destination_dirty_through
            == state.completed_bytes + state.window.unwrap().bytes,
        retire_window_v1(state, true).retired_windows == state.retired_windows + 1, {}
pub proof fn native_execution_failure_quarantines_without_dirty_progress_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Published,
    ensures quarantine_v1(state).phase == PhaseV1::Quarantined,
        quarantine_v1(state).destination_dirty_through == state.destination_dirty_through,
        quarantine_v1(state).source_read_lease_count == 1,
        quarantine_v1(state).destination_write_lease_count == 1, {}
pub proof fn continuation_waits_for_full_frontier_retirement_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::FrontierPending,
        state.completed_bytes + state.window.unwrap().bytes < state.total_bytes,
    ensures retire_window_v1(state, true).phase == PhaseV1::Ready,
        retire_window_v1(state, true).window.is_none(),
        retire_window_v1(state, true).source_read_lease_count == 0,
        retire_window_v1(state, true).destination_write_lease_count == 0, {}
pub proof fn quarantine_is_absorbing_and_retains_pair_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Quarantined,
    ensures quarantine_v1(state) == state,
        release_terminal_v1(state, state.transfer_id) == state,
        state.source_authority_count == 1, state.destination_authority_count == 1, {}
pub proof fn quarantine_entry_preserves_validity_and_pair_v1(state: StateV1)
    requires valid_state_v1(state), state.phase != PhaseV1::Quarantined,
    ensures valid_state_v1(quarantine_v1(state)),
        quarantine_v1(state).phase == PhaseV1::Quarantined,
        quarantine_v1(state).custody == CustodyV1::QuarantinedPair,
        quarantine_v1(state).target_retained,
        !quarantine_v1(state).current,
        quarantine_v1(state).source_authority_count == 1,
        quarantine_v1(state).destination_authority_count == 1, {}
pub proof fn cancellation_only_before_progress_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Ready,
    ensures cancel_v1(state).phase == if state.completed_bytes == 0 {
            PhaseV1::DevicePairReady } else { PhaseV1::Ready }, {}
pub proof fn published_window_cannot_cancel_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Published,
    ensures cancel_v1(state) == state, {}
pub proof fn terminal_release_restores_two_owner_pair_v1(state: StateV1)
    requires valid_state_v1(state), state.phase == PhaseV1::Completed,
    ensures release_terminal_v1(state, state.transfer_id).phase
            == PhaseV1::DevicePairReady,
        release_terminal_v1(state, state.transfer_id).custody == CustodyV1::DevicePair,
        release_terminal_v1(state, state.transfer_id).source_authority_count == 1,
        release_terminal_v1(state, state.transfer_id).destination_authority_count == 1, {}
pub proof fn full_256_mib_uses_two_windows_and_65_packets_v1()
    ensures packet_count_v1(max_transfer_bytes_v1()) == 65,
        packet_count_v1(max_window_bytes_v1()) == 63,
        packet_count_v1((max_transfer_bytes_v1() - max_window_bytes_v1()) as nat) == 2, {}
pub proof fn full_256_mib_final_packet_is_2048_bytes_v1()
    ensures final_packet_bytes_v1((max_transfer_bytes_v1()
        - max_window_bytes_v1()) as nat) == 2048, {}

fn main() {}

}
