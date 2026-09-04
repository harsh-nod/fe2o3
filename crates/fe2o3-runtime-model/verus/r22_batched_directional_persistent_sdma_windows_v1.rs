// Bounded R22 batched directional persistent-SDMA window model. This is not a
// refinement of executable Rust, R19, KFD, hardware, liveness, or performance.
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

#[derive(PartialEq, Eq)] pub struct AllocationV1 {
    pub owner: nat, pub identity: nat, pub generation: nat,
    pub pool_generation: nat, pub logical_bytes: nat, pub incarnation: nat,
}
#[derive(PartialEq, Eq)] pub struct HostV1 {
    pub session: nat, pub identity: nat, pub generation: nat, pub byte_len: nat,
}
#[derive(PartialEq, Eq)] pub enum StorageV1 { Host(HostV1), Device(AllocationV1) }
#[derive(PartialEq, Eq)] pub struct EndpointV1 { pub storage: StorageV1, pub offset: nat }
#[derive(PartialEq, Eq)] pub enum DirectionV1 { DeviceToHost, HostToDevice }
#[derive(PartialEq, Eq)] pub enum PhaseV1 {
    DeviceReady, Ready, Prepared, Published, FrontierPending, Completed,
    QuiescentWithoutResult, ProcessTeardown,
}
#[derive(PartialEq, Eq)] pub enum CustodyV1 {
    Device, Ready, PreparedWindow, PublishedWindow, FrontierPending, Opaque,
}
#[derive(PartialEq, Eq)] pub enum StatusV1 { Succeeded, Failed }
#[derive(PartialEq, Eq)] pub struct TicketV1 {
    pub parent: nat, pub child: nat, pub slot: nat, pub generation: nat,
}
pub struct WindowV1 {
    pub transfer_id: nat, pub ordinal: nat, pub direction: DirectionV1,
    pub transfer_offset: nat, pub bytes: nat, pub packet_count: nat,
    pub final_packet_bytes: nat, pub first_slot: nat,
    pub prior_slot_generations: Seq<nat>,
    pub parent: nat, pub child: nat, pub lease_generation: nat,
    pub allocation: AllocationV1, pub host: HostV1,
}
pub struct FrontierV1 {
    pub window: WindowV1, pub status: StatusV1,
}
pub struct StateV1 {
    pub phase: PhaseV1, pub custody: CustodyV1,
    pub allocation: AllocationV1, pub host: HostV1,
    pub pair_occurrence: nat, pub attachment_generation: nat,
    pub transfer_id: nat, pub source: EndpointV1, pub destination: EndpointV1,
    pub direction: DirectionV1, pub total_bytes: nat, pub completed_bytes: nat,
    pub window_ordinal: nat, pub window: Option<WindowV1>,
    pub frontier: Option<FrontierV1>, pub observed_completed_packets: nat,
    pub next_ring_slot: nat, pub slot_generations: Seq<nat>,
    pub next_use_generation: nat, pub published_windows: nat,
    pub published_packets: nat, pub write_pointer_publications: nat,
    pub doorbell_publications: nat, pub retired_windows: nat,
    pub destination_dirty_through: nat, pub host_dirty_through: nat,
    pub possibly_mutated_through: nat, pub host_possibly_mutated_through: nat,
    pub authority_count: nat, pub aggregate_lease_count: nat,
    pub target_retained: bool, pub current: bool,
    pub result_succeeded: bool, pub result_code: int,
}

pub open spec fn valid_allocation_v1(a: AllocationV1) -> bool {
    a.owner > 0 && a.identity > 0 && a.generation > 0 && a.pool_generation > 0
        && a.incarnation > 0 && 0 < a.logical_bytes <= max_transfer_bytes_v1()
}
pub open spec fn valid_host_v1(h: HostV1) -> bool {
    h.session > 0 && h.identity > 0 && h.generation > 0 && h.byte_len > 0
}
pub open spec fn storage_extent_v1(storage: StorageV1) -> nat {
    match storage {
        StorageV1::Host(h) => h.byte_len,
        StorageV1::Device(a) => a.logical_bytes,
    }
}
pub open spec fn valid_storage_v1(storage: StorageV1) -> bool {
    match storage {
        StorageV1::Host(h) => valid_host_v1(h),
        StorageV1::Device(a) => valid_allocation_v1(a),
    }
}
pub open spec fn valid_endpoint_range_v1(endpoint: EndpointV1, bytes: nat) -> bool {
    valid_storage_v1(endpoint.storage) && bytes > 0
        && endpoint.offset + bytes <= storage_extent_v1(endpoint.storage)
}
pub open spec fn resolved_direction_v1(source: EndpointV1, destination: EndpointV1)
    -> Option<DirectionV1>
{
    match (source.storage, destination.storage) {
        (StorageV1::Host(_), StorageV1::Device(_)) => Some(DirectionV1::HostToDevice),
        (StorageV1::Device(_), StorageV1::Host(_)) => Some(DirectionV1::DeviceToHost),
        _ => None,
    }
}
pub open spec fn bound_allocation_v1(source: EndpointV1, destination: EndpointV1)
    -> Option<AllocationV1>
{
    match (source.storage, destination.storage) {
        (StorageV1::Host(_), StorageV1::Device(a)) => Some(a),
        (StorageV1::Device(a), StorageV1::Host(_)) => Some(a),
        _ => None,
    }
}
pub open spec fn bound_host_v1(source: EndpointV1, destination: EndpointV1)
    -> Option<HostV1>
{
    match (source.storage, destination.storage) {
        (StorageV1::Host(h), StorageV1::Device(_)) => Some(h),
        (StorageV1::Device(_), StorageV1::Host(h)) => Some(h),
        _ => None,
    }
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
pub open spec fn packet_offset_at_v1(window: WindowV1, index: nat) -> nat {
    window.transfer_offset + index * max_packet_bytes_v1()
}
pub open spec fn packet_bytes_at_v1(window: WindowV1, index: nat) -> nat {
    if index + 1 == window.packet_count {
        window.final_packet_bytes
    } else {
        max_packet_bytes_v1()
    }
}
pub open spec fn valid_slot_generations_v1(generations: Seq<nat>) -> bool {
    generations.len() == ring_slots_v1()
        && forall|slot: int| 0 <= slot < generations.len()
            ==> generations[slot] <= max_u32_v1()
}
pub open spec fn slot_distance_v1(first_slot: nat, slot: nat) -> nat {
    if first_slot <= slot {
        (slot - first_slot) as nat
    } else {
        (ring_slots_v1() - first_slot + slot) as nat
    }
}
pub open spec fn window_selects_slot_v1(window: WindowV1, slot: nat) -> bool {
    slot < ring_slots_v1()
        && slot_distance_v1(window.first_slot, slot) < window.packet_count
}
pub open spec fn ticket_at_v1(window: WindowV1, index: nat) -> TicketV1 {
    let slot = (window.first_slot + index) % ring_slots_v1();
    TicketV1 {
        parent: window.parent,
        child: window.child,
        slot,
        generation: window.prior_slot_generations[slot as int] + 1,
    }
}
pub open spec fn committed_slot_generations_v1(window: WindowV1) -> Seq<nat> {
    Seq::new(ring_slots_v1(), |slot: int|
        if window_selects_slot_v1(window, slot as nat) {
            window.prior_slot_generations[slot] + 1
        } else {
            window.prior_slot_generations[slot]
        })
}
pub open spec fn valid_window_v1(window: WindowV1) -> bool {
    &&& window.transfer_id > 0
    &&& window.ordinal >= 0
    &&& 0 < window.bytes <= max_window_bytes_v1()
    &&& window.packet_count == packet_count_v1(window.bytes)
    &&& 0 < window.packet_count <= max_window_packets_v1()
    &&& window.final_packet_bytes == final_packet_bytes_v1(window.bytes)
    &&& 0 < window.final_packet_bytes <= max_packet_bytes_v1()
    &&& window.bytes == (window.packet_count - 1) * max_packet_bytes_v1()
        + window.final_packet_bytes
    &&& window.first_slot < ring_slots_v1()
    &&& valid_slot_generations_v1(window.prior_slot_generations)
    &&& forall|index: int| 0 <= index < window.packet_count ==>
        #[trigger] window.prior_slot_generations[
            ((window.first_slot + index as nat) % ring_slots_v1()) as int
        ] < max_u32_v1()
    &&& forall|slot: int| 0 <= slot < window.prior_slot_generations.len()
        && window_selects_slot_v1(window, slot as nat) ==>
        #[trigger] window.prior_slot_generations[slot] < max_u32_v1()
    &&& window.parent == 91
    &&& window.child == match window.direction {
        DirectionV1::DeviceToHost => 3nat,
        DirectionV1::HostToDevice => 4nat,
    }
    &&& window.lease_generation > 0
    &&& valid_allocation_v1(window.allocation)
    &&& valid_host_v1(window.host)
}
pub open spec fn planned_window_v1(s: StateV1) -> WindowV1 {
    let bytes = window_bytes_v1((s.total_bytes - s.completed_bytes) as nat);
    WindowV1 {
        transfer_id: s.transfer_id,
        ordinal: s.window_ordinal,
        direction: s.direction,
        transfer_offset: s.completed_bytes,
        bytes,
        packet_count: packet_count_v1(bytes),
        final_packet_bytes: final_packet_bytes_v1(bytes),
        first_slot: s.next_ring_slot,
        prior_slot_generations: s.slot_generations,
        parent: 91,
        child: match s.direction {
            DirectionV1::DeviceToHost => 3,
            DirectionV1::HostToDevice => 4,
        },
        lease_generation: s.next_use_generation,
        allocation: s.allocation,
        host: s.host,
    }
}
pub open spec fn exact_frontier_v1(s: StateV1, status: StatusV1) -> FrontierV1 {
    FrontierV1 { window: s.window.unwrap(), status }
}
pub open spec fn base_valid_v1(s: StateV1) -> bool {
    &&& valid_allocation_v1(s.allocation)
    &&& valid_host_v1(s.host)
    &&& s.pair_occurrence > 0
    &&& s.attachment_generation > 0
    &&& s.transfer_id > 0
    &&& resolved_direction_v1(s.source, s.destination) == Some(s.direction)
    &&& bound_allocation_v1(s.source, s.destination) == Some(s.allocation)
    &&& bound_host_v1(s.source, s.destination) == Some(s.host)
    &&& valid_endpoint_range_v1(s.source, s.total_bytes)
    &&& valid_endpoint_range_v1(s.destination, s.total_bytes)
    &&& s.completed_bytes <= s.total_bytes
    &&& s.destination_dirty_through == s.completed_bytes
    &&& s.host_dirty_through == if s.direction == DirectionV1::DeviceToHost {
        s.completed_bytes
    } else { 0 }
    &&& s.completed_bytes <= s.possibly_mutated_through <= s.total_bytes
    &&& s.host_possibly_mutated_through == if s.direction == DirectionV1::DeviceToHost {
        s.possibly_mutated_through
    } else { 0 }
    &&& s.next_ring_slot < ring_slots_v1()
    &&& valid_slot_generations_v1(s.slot_generations)
    &&& s.next_use_generation > 0
    &&& s.write_pointer_publications == s.published_windows
    &&& s.doorbell_publications == s.published_windows
    &&& s.retired_windows <= s.published_windows
    &&& s.authority_count == 1
}
pub open spec fn valid_state_v1(s: StateV1) -> bool {
    &&& base_valid_v1(s)
    &&& match s.phase {
        PhaseV1::DeviceReady => s.custody == CustodyV1::Device
            && s.window.is_none() && s.frontier.is_none()
            && s.aggregate_lease_count == 0 && !s.target_retained && s.current,
        PhaseV1::Ready => s.custody == CustodyV1::Ready
            && s.completed_bytes < s.total_bytes && s.window.is_none()
            && s.frontier.is_none() && s.aggregate_lease_count == 0
            && s.target_retained && s.current
            && !s.result_succeeded && s.result_code == 0,
        PhaseV1::Prepared => s.custody == CustodyV1::PreparedWindow
            && s.window.is_some() && valid_window_v1(s.window.unwrap())
            && s.window.unwrap().transfer_offset == s.completed_bytes
            && s.window.unwrap().transfer_offset + s.window.unwrap().bytes <= s.total_bytes
            && s.window.unwrap().transfer_id == s.transfer_id
            && s.window.unwrap().ordinal == s.window_ordinal
            && s.window.unwrap().direction == s.direction
            && s.window.unwrap().first_slot == s.next_ring_slot
            && s.window.unwrap().prior_slot_generations == s.slot_generations
            && s.window.unwrap().lease_generation == s.next_use_generation
            && s.window.unwrap().allocation == s.allocation
            && s.window.unwrap().host == s.host
            && s.frontier.is_none() && s.observed_completed_packets == 0
            && s.aggregate_lease_count == 1 && s.target_retained && s.current
            && !s.result_succeeded && s.result_code == 0,
        PhaseV1::Published => s.custody == CustodyV1::PublishedWindow
            && s.window.is_some() && valid_window_v1(s.window.unwrap())
            && s.window.unwrap().transfer_offset == s.completed_bytes
            && s.window.unwrap().transfer_offset + s.window.unwrap().bytes <= s.total_bytes
            && s.window.unwrap().transfer_id == s.transfer_id
            && s.window.unwrap().ordinal == s.window_ordinal
            && s.window.unwrap().direction == s.direction
            && s.window.unwrap().allocation == s.allocation
            && s.window.unwrap().host == s.host
            && s.slot_generations
                == committed_slot_generations_v1(s.window.unwrap())
            && s.frontier.is_none()
            && s.observed_completed_packets < s.window.unwrap().packet_count
            && s.possibly_mutated_through
                == s.window.unwrap().transfer_offset + s.window.unwrap().bytes
            && s.retired_windows < s.published_windows
            && s.aggregate_lease_count == 1 && s.target_retained && s.current
            && !s.result_succeeded && s.result_code == 0,
        PhaseV1::FrontierPending => s.custody == CustodyV1::FrontierPending
            && s.window.is_some() && valid_window_v1(s.window.unwrap())
            && s.window.unwrap().transfer_offset == s.completed_bytes
            && s.window.unwrap().transfer_offset + s.window.unwrap().bytes <= s.total_bytes
            && s.window.unwrap().transfer_id == s.transfer_id
            && s.window.unwrap().ordinal == s.window_ordinal
            && s.window.unwrap().direction == s.direction
            && s.window.unwrap().allocation == s.allocation
            && s.window.unwrap().host == s.host
            && s.slot_generations
                == committed_slot_generations_v1(s.window.unwrap())
            && s.frontier.is_some() && s.frontier.unwrap().window == s.window.unwrap()
            && s.observed_completed_packets == s.window.unwrap().packet_count
            && s.possibly_mutated_through
                == s.window.unwrap().transfer_offset + s.window.unwrap().bytes
            && s.retired_windows < s.published_windows
            && s.aggregate_lease_count == 1 && s.target_retained && s.current
            && !s.result_succeeded && s.result_code == 0,
        PhaseV1::Completed => s.custody == CustodyV1::Device
            && s.window.is_none() && s.frontier.is_none()
            && s.aggregate_lease_count == 0 && s.target_retained && s.current
            && ((s.result_succeeded && s.result_code == 0
                    && s.completed_bytes == s.total_bytes)
                || (!s.result_succeeded && s.result_code < 0)),
        PhaseV1::QuiescentWithoutResult => s.custody == CustodyV1::Device
            && s.window.is_none() && s.frontier.is_none()
            && s.aggregate_lease_count == 0 && s.target_retained && s.current
            && !s.result_succeeded && s.result_code == 0,
        PhaseV1::ProcessTeardown => s.custody == CustodyV1::Opaque
            && s.authority_count == 1 && !s.current && s.target_retained,
    }
}

pub open spec fn prepare_window_v1(s: StateV1) -> StateV1 {
    let window = planned_window_v1(s);
    if s.phase == PhaseV1::Ready && valid_state_v1(s)
        && valid_window_v1(window)
        && window.transfer_offset + window.bytes <= s.total_bytes {
        StateV1 { phase: PhaseV1::Prepared, custody: CustodyV1::PreparedWindow,
            window: Some(window), aggregate_lease_count: 1,
            observed_completed_packets: 0, ..s }
    } else { s }
}
pub open spec fn retryable_prepublication_v1(s: StateV1) -> StateV1 {
    if s.phase == PhaseV1::Prepared && valid_state_v1(s) {
        StateV1 { phase: PhaseV1::Ready, custody: CustodyV1::Ready,
            window: None, aggregate_lease_count: 0, ..s }
    } else { s }
}
pub open spec fn publish_window_v1(s: StateV1) -> StateV1 {
    if s.phase == PhaseV1::Prepared && valid_state_v1(s) {
        let window = s.window.unwrap();
        let through = window.transfer_offset + window.bytes;
        StateV1 { phase: PhaseV1::Published, custody: CustodyV1::PublishedWindow,
            next_ring_slot: (s.next_ring_slot + window.packet_count) % ring_slots_v1(),
            slot_generations: committed_slot_generations_v1(window),
            next_use_generation: s.next_use_generation + 1,
            published_windows: s.published_windows + 1,
            published_packets: s.published_packets + window.packet_count,
            write_pointer_publications: s.write_pointer_publications + 1,
            doorbell_publications: s.doorbell_publications + 1,
            possibly_mutated_through: through,
            host_possibly_mutated_through:
                if s.direction == DirectionV1::DeviceToHost { through } else { 0 },
            ..s }
    } else { s }
}
pub open spec fn opaque_publication_v1(s: StateV1) -> StateV1 {
    if (s.phase == PhaseV1::Prepared || s.phase == PhaseV1::Published)
        && valid_state_v1(s) {
        StateV1 { phase: PhaseV1::ProcessTeardown, custody: CustodyV1::Opaque,
            current: false, ..s }
    } else { s }
}
pub open spec fn poll_pending_v1(s: StateV1) -> StateV1 { s }
pub open spec fn poll_timeout_v1(s: StateV1) -> StateV1 { s }
pub open spec fn observe_partial_v1(s: StateV1, completed_packets: nat) -> StateV1 {
    if s.phase == PhaseV1::Published && valid_state_v1(s)
        && s.observed_completed_packets <= completed_packets
        && 0 < completed_packets < s.window.unwrap().packet_count {
        StateV1 { observed_completed_packets: completed_packets, ..s }
    } else { s }
}
pub open spec fn recover_postpublication_v1(s: StateV1) -> StateV1 {
    if s.phase == PhaseV1::Published && valid_state_v1(s) {
        StateV1 { phase: PhaseV1::QuiescentWithoutResult, custody: CustodyV1::Device,
            window: None, frontier: None, aggregate_lease_count: 0,
            observed_completed_packets: 0, ..s }
    } else { s }
}
pub open spec fn complete_window_v1(s: StateV1, metadata_matches: bool,
    status: StatusV1) -> StateV1
{
    if s.phase == PhaseV1::Published && valid_state_v1(s) {
        if metadata_matches {
            StateV1 { phase: PhaseV1::FrontierPending,
                custody: CustodyV1::FrontierPending,
                frontier: Some(exact_frontier_v1(s, status)),
                observed_completed_packets: s.window.unwrap().packet_count, ..s }
        } else {
            StateV1 { phase: PhaseV1::ProcessTeardown, custody: CustodyV1::Opaque,
                current: false, ..s }
        }
    } else { s }
}
pub open spec fn retire_window_v1(s: StateV1, frontier_matches: bool) -> StateV1 {
    if s.phase == PhaseV1::FrontierPending && valid_state_v1(s) {
        if !frontier_matches {
            StateV1 { phase: PhaseV1::ProcessTeardown, custody: CustodyV1::Opaque,
                current: false, ..s }
        } else {
            let window = s.window.unwrap();
            let status = s.frontier.unwrap().status;
            if status == StatusV1::Succeeded {
                let through = s.completed_bytes + window.bytes;
                StateV1 {
                    phase: if through == s.total_bytes { PhaseV1::Completed }
                        else { PhaseV1::Ready },
                    custody: if through == s.total_bytes { CustodyV1::Device }
                        else { CustodyV1::Ready },
                    completed_bytes: through, window_ordinal: s.window_ordinal + 1,
                    window: None, frontier: None, observed_completed_packets: 0,
                    aggregate_lease_count: 0, retired_windows: s.retired_windows + 1,
                    destination_dirty_through: through,
                    host_dirty_through: if s.direction == DirectionV1::DeviceToHost {
                        through
                    } else { 0 },
                    result_succeeded: through == s.total_bytes, result_code: 0, ..s
                }
            } else {
                StateV1 { phase: PhaseV1::Completed, custody: CustodyV1::Device,
                    window: None, frontier: None, observed_completed_packets: 0,
                    aggregate_lease_count: 0, retired_windows: s.retired_windows + 1,
                    result_succeeded: false, result_code: -1, ..s }
            }
        }
    } else { s }
}
pub open spec fn cancel_v1(s: StateV1) -> StateV1 {
    if s.phase == PhaseV1::Ready && s.completed_bytes == 0 {
        StateV1 { phase: PhaseV1::DeviceReady, custody: CustodyV1::Device,
            target_retained: false, ..s }
    } else { s }
}
pub open spec fn release_terminal_v1(s: StateV1, transfer_id: nat) -> StateV1 {
    if (s.phase == PhaseV1::Completed || s.phase == PhaseV1::QuiescentWithoutResult)
        && s.transfer_id == transfer_id && s.target_retained {
        StateV1 { phase: PhaseV1::DeviceReady, custody: CustodyV1::Device,
            target_retained: false, result_succeeded: false, result_code: 0, ..s }
    } else { s }
}

pub open spec fn sample_allocation_v1() -> AllocationV1 {
    AllocationV1 { owner: 1, identity: 2, generation: 3, pool_generation: 4,
        logical_bytes: max_transfer_bytes_v1(), incarnation: 5 }
}
pub open spec fn sample_host_v1() -> HostV1 {
    HostV1 { session: 6, identity: 7, generation: 8, byte_len: max_transfer_bytes_v1() }
}
pub open spec fn host_endpoint_v1() -> EndpointV1 {
    EndpointV1 { storage: StorageV1::Host(sample_host_v1()), offset: 0 }
}
pub open spec fn device_endpoint_v1() -> EndpointV1 {
    EndpointV1 { storage: StorageV1::Device(sample_allocation_v1()), offset: 0 }
}
pub open spec fn sample_ready_v1(direction: DirectionV1) -> StateV1 {
    StateV1 { phase: PhaseV1::Ready, custody: CustodyV1::Ready,
        allocation: sample_allocation_v1(), host: sample_host_v1(),
        pair_occurrence: 9, attachment_generation: 10, transfer_id: 11,
        source: if direction == DirectionV1::HostToDevice {
            host_endpoint_v1()
        } else { device_endpoint_v1() },
        destination: if direction == DirectionV1::HostToDevice {
            device_endpoint_v1()
        } else { host_endpoint_v1() },
        direction, total_bytes: max_transfer_bytes_v1(), completed_bytes: 0,
        window_ordinal: 0, window: None, frontier: None,
        observed_completed_packets: 0, next_ring_slot: 0,
        slot_generations: Seq::new(ring_slots_v1(), |_slot: int| 0),
        next_use_generation: 13, published_windows: 0,
        published_packets: 0, write_pointer_publications: 0,
        doorbell_publications: 0, retired_windows: 0,
        destination_dirty_through: 0, host_dirty_through: 0,
        possibly_mutated_through: 0, host_possibly_mutated_through: 0,
        authority_count: 1, aggregate_lease_count: 0,
        target_retained: true, current: true,
        result_succeeded: false, result_code: 0 }
}
pub open spec fn sample_wrap_prior_slot_generations_v1() -> Seq<nat> {
    Seq::new(ring_slots_v1(), |slot: int| if slot == 0 { 1 } else { 0 })
}
pub open spec fn sample_wrap_window_v1() -> WindowV1 {
    WindowV1 {
        transfer_id: 11, ordinal: 1, direction: DirectionV1::HostToDevice,
        transfer_offset: max_window_bytes_v1(),
        bytes: max_packet_bytes_v1() + 1, packet_count: 2,
        final_packet_bytes: 1, first_slot: 63,
        prior_slot_generations: sample_wrap_prior_slot_generations_v1(),
        parent: 91, child: 4, lease_generation: 14,
        allocation: sample_allocation_v1(), host: sample_host_v1(),
    }
}

pub proof fn fixed_ring_packet_and_window_bounds_v1()
    ensures max_transfer_bytes_v1() == 268435456,
        max_packet_bytes_v1() == 4194272, ring_slots_v1() == 64,
        max_window_packets_v1() == 63,
        max_window_bytes_v1() == 264239136, max_u32_v1() == 4294967295, {}
pub proof fn sample_directional_states_are_valid_v1()
    ensures valid_state_v1(sample_ready_v1(DirectionV1::HostToDevice)),
        valid_state_v1(sample_ready_v1(DirectionV1::DeviceToHost)), {}
pub proof fn h2d_and_d2h_storage_roles_are_exact_v1()
    ensures resolved_direction_v1(host_endpoint_v1(), device_endpoint_v1())
            == Some(DirectionV1::HostToDevice),
        resolved_direction_v1(device_endpoint_v1(), host_endpoint_v1())
            == Some(DirectionV1::DeviceToHost), {}
pub proof fn h2h_and_d2d_preflight_is_atomic_v1()
    ensures resolved_direction_v1(host_endpoint_v1(), host_endpoint_v1()).is_none(),
        resolved_direction_v1(device_endpoint_v1(), device_endpoint_v1()).is_none(), {}
pub proof fn endpoint_range_rejection_is_atomic_v1(endpoint: EndpointV1)
    requires valid_storage_v1(endpoint.storage),
    ensures !valid_endpoint_range_v1(endpoint, storage_extent_v1(endpoint.storage) + 1), {}
pub proof fn planned_window_count_is_bounded_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready,
        valid_window_v1(planned_window_v1(s)),
    ensures 0 < planned_window_v1(s).packet_count <= max_window_packets_v1(), {}
pub proof fn planned_window_packet_count_matches_roster_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready,
        valid_window_v1(planned_window_v1(s)),
    ensures planned_window_v1(s).packet_count
        == packet_count_v1(planned_window_v1(s).bytes), {}
pub proof fn planned_window_packets_cover_exact_range_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready,
        valid_window_v1(planned_window_v1(s)),
    ensures planned_window_v1(s).bytes
        == (planned_window_v1(s).packet_count - 1) * max_packet_bytes_v1()
            + planned_window_v1(s).final_packet_bytes, {}
pub proof fn planned_window_packets_are_contiguous_v1(window: WindowV1, index: nat)
    requires valid_window_v1(window), index + 1 < window.packet_count,
    ensures packet_offset_at_v1(window, index) + packet_bytes_at_v1(window, index)
        == packet_offset_at_v1(window, index + 1), {}
pub proof fn planned_window_tickets_bind_selected_child_v1(window: WindowV1, index: nat)
    requires valid_window_v1(window), index < window.packet_count,
    ensures ticket_at_v1(window, index).parent == window.parent,
        ticket_at_v1(window, index).child == window.child,
        ticket_at_v1(window, index).generation
            == window.prior_slot_generations[ticket_at_v1(window, index).slot as int] + 1,
        0 < ticket_at_v1(window, index).generation <= max_u32_v1(),
        ticket_at_v1(window, index).slot < ring_slots_v1(),
{
    let slot = (window.first_slot + index) % ring_slots_v1();
    assert(slot < ring_slots_v1());
    assert(0 <= slot < window.prior_slot_generations.len());
    assert(window.prior_slot_generations[slot as int] < max_u32_v1());
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
        &&& committed[63] == 1
        &&& committed[0] == 2
        &&& committed[1] == 0
    },
{}
pub proof fn preparation_has_no_publication_effect_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready,
    ensures {
        let prepared = prepare_window_v1(s);
        &&& prepared.published_windows == s.published_windows
        &&& prepared.published_packets == s.published_packets
        &&& prepared.write_pointer_publications == s.write_pointer_publications
        &&& prepared.doorbell_publications == s.doorbell_publications
        &&& prepared.slot_generations == s.slot_generations
        &&& prepared.next_ring_slot == s.next_ring_slot
    }, {}
pub proof fn confirmed_publication_preserves_validity_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Prepared,
    ensures valid_state_v1(publish_window_v1(s)),
{
    let window = s.window.unwrap();
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
pub proof fn confirmed_publication_commits_exact_packet_count_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Prepared,
    ensures publish_window_v1(s).published_packets
        == s.published_packets + s.window.unwrap().packet_count, {}
pub proof fn confirmed_publication_has_one_write_pointer_update_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Prepared,
    ensures publish_window_v1(s).write_pointer_publications
        == s.write_pointer_publications + 1, {}
pub proof fn confirmed_publication_has_one_doorbell_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Prepared,
    ensures publish_window_v1(s).doorbell_publications
        == s.doorbell_publications + 1, {}
pub proof fn retryable_prepublication_restores_exact_authority_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Prepared,
    ensures {
        let restored = retryable_prepublication_v1(s);
        &&& valid_state_v1(restored) &&& restored.phase == PhaseV1::Ready
        &&& restored.custody == CustodyV1::Ready
        &&& restored.authority_count == 1 &&& restored.aggregate_lease_count == 0
        &&& restored.completed_bytes == s.completed_bytes
        &&& restored.published_windows == s.published_windows
        &&& restored.write_pointer_publications == s.write_pointer_publications
        &&& restored.doorbell_publications == s.doorbell_publications
    }, {}
pub proof fn initial_postpublication_recovery_is_quiescent_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
        s.completed_bytes == 0,
    ensures recover_postpublication_v1(s).phase == PhaseV1::QuiescentWithoutResult,
        recover_postpublication_v1(s).completed_bytes == 0,
        recover_postpublication_v1(s).possibly_mutated_through > 0, {}
pub proof fn prior_window_recovery_is_quiescent_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
        s.completed_bytes > 0,
    ensures recover_postpublication_v1(s).phase == PhaseV1::QuiescentWithoutResult,
        recover_postpublication_v1(s).completed_bytes == s.completed_bytes,
        recover_postpublication_v1(s).destination_dirty_through == s.completed_bytes, {}
pub proof fn prior_d2h_host_mutation_recovery_is_quiescent_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
        s.direction == DirectionV1::DeviceToHost,
    ensures recover_postpublication_v1(s).phase == PhaseV1::QuiescentWithoutResult,
        recover_postpublication_v1(s).host_possibly_mutated_through
            == s.possibly_mutated_through,
        recover_postpublication_v1(s).host_possibly_mutated_through
            >= s.host_dirty_through, {}
pub proof fn retained_publication_enters_teardown_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Prepared,
    ensures valid_state_v1(opaque_publication_v1(s)),
        opaque_publication_v1(s).phase == PhaseV1::ProcessTeardown,
        opaque_publication_v1(s).authority_count == 1, {}
pub proof fn pending_poll_is_observation_only_v1(s: StateV1)
    ensures poll_pending_v1(s) == s, {}
pub proof fn timeout_is_observation_only_v1(s: StateV1)
    ensures poll_timeout_v1(s) == s, {}
pub proof fn incomplete_window_completion_retains_published_custody_v1(
    s: StateV1, completed_packets: nat)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
        s.observed_completed_packets <= completed_packets,
        0 < completed_packets < s.window.unwrap().packet_count,
    ensures valid_state_v1(observe_partial_v1(s, completed_packets)),
        observe_partial_v1(s, completed_packets).phase == PhaseV1::Published,
        observe_partial_v1(s, completed_packets).custody == CustodyV1::PublishedWindow,
        observe_partial_v1(s, completed_packets).completed_bytes == s.completed_bytes,
        observe_partial_v1(s, completed_packets).aggregate_lease_count == 1, {}
pub proof fn completion_metadata_mismatch_enters_teardown_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
    ensures valid_state_v1(complete_window_v1(s, false, StatusV1::Succeeded)),
        complete_window_v1(s, false, StatusV1::Succeeded).phase
            == PhaseV1::ProcessTeardown, {}
pub proof fn exact_window_completion_creates_exact_frontier_v1(
    s: StateV1, status: StatusV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
    ensures valid_state_v1(complete_window_v1(s, true, status)),
        complete_window_v1(s, true, status).phase == PhaseV1::FrontierPending,
        complete_window_v1(s, true, status).frontier
            == Some(exact_frontier_v1(s, status)), {}
pub proof fn frontier_retains_complete_window_roster_v1(s: StateV1, status: StatusV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
    ensures complete_window_v1(s, true, status).frontier.unwrap().window
            == s.window.unwrap(),
        complete_window_v1(s, true, status).observed_completed_packets
            == s.window.unwrap().packet_count, {}
pub proof fn stale_frontier_retirement_enters_teardown_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::FrontierPending,
    ensures valid_state_v1(retire_window_v1(s, false)),
        retire_window_v1(s, false).phase == PhaseV1::ProcessTeardown, {}
pub proof fn exact_retirement_advances_window_progress_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::FrontierPending,
        s.frontier.unwrap().status == StatusV1::Succeeded,
    ensures {
        let retired = retire_window_v1(s, true);
        &&& valid_state_v1(retired)
        &&& retired.completed_bytes == s.completed_bytes + s.window.unwrap().bytes
        &&& retired.retired_windows == s.retired_windows + 1
        &&& retired.window.is_none() &&& retired.frontier.is_none()
        &&& retired.aggregate_lease_count == 0
    }, {}
pub proof fn exact_retirement_updates_directional_dirty_progress_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::FrontierPending,
        s.frontier.unwrap().status == StatusV1::Succeeded,
    ensures {
        let retired = retire_window_v1(s, true);
        &&& retired.destination_dirty_through == retired.completed_bytes
        &&& (s.direction == DirectionV1::DeviceToHost
            ==> retired.host_dirty_through == retired.completed_bytes)
        &&& (s.direction == DirectionV1::HostToDevice
            ==> retired.host_dirty_through == 0)
    }, {}
pub proof fn continuation_waits_for_window_retirement_v1(s: StateV1, status: StatusV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
    ensures prepare_window_v1(s) == s,
        prepare_window_v1(complete_window_v1(s, true, status))
            == complete_window_v1(s, true, status), {}
pub proof fn poll_never_publishes_next_window_v1(s: StateV1)
    ensures poll_pending_v1(s) == s,
        poll_pending_v1(s).published_windows == s.published_windows, {}
pub proof fn cancellation_only_before_progress_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready,
    ensures (s.completed_bytes == 0 ==> cancel_v1(s).phase == PhaseV1::DeviceReady),
        (s.completed_bytes > 0 ==> cancel_v1(s) == s), {}
pub proof fn published_window_cannot_cancel_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
    ensures cancel_v1(s) == s, {}
pub proof fn opaque_failure_retains_single_authority_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
    ensures valid_state_v1(opaque_publication_v1(s)),
        opaque_publication_v1(s).custody == CustodyV1::Opaque,
        opaque_publication_v1(s).authority_count == 1,
        opaque_publication_v1(s).aggregate_lease_count == 1, {}
pub proof fn full_256_mib_uses_two_windows_and_65_packets_v1()
    ensures packet_count_v1(max_transfer_bytes_v1()) == 65,
        packet_count_v1(max_window_bytes_v1()) == 63,
        packet_count_v1((max_transfer_bytes_v1() - max_window_bytes_v1()) as nat) == 2,
        window_bytes_v1(max_transfer_bytes_v1()) == max_window_bytes_v1(),
        window_bytes_v1((max_transfer_bytes_v1() - max_window_bytes_v1()) as nat)
            == (max_transfer_bytes_v1() - max_window_bytes_v1()) as nat,
{}
pub proof fn full_256_mib_last_packet_is_2048_bytes_v1()
    ensures max_transfer_bytes_v1() - max_window_bytes_v1() == 4196320,
        final_packet_bytes_v1((max_transfer_bytes_v1() - max_window_bytes_v1()) as nat)
            == 2048, {}
pub proof fn terminal_release_allows_repeated_or_mixed_direction_v1(
    s: StateV1, next_direction: DirectionV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Completed,
    ensures {
        let released = release_terminal_v1(s, s.transfer_id);
        &&& valid_state_v1(released) &&& released.phase == PhaseV1::DeviceReady
        &&& !released.target_retained
        &&& (next_direction == DirectionV1::HostToDevice
            || next_direction == DirectionV1::DeviceToHost)
    }, {}

fn main() {}
}
