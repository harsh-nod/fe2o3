// Bounded R20 runtime-facade chunking model. This is not a refinement of the
// executable Rust model, KFD code, native execution, hardware, liveness, or performance.
use vstd::prelude::*;

verus! {

pub open spec fn max_transfer_bytes_v1() -> nat { 256 * 1024 * 1024 }
pub open spec fn max_packet_bytes_v1() -> nat { 0x003f_ffe0 }
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
#[derive(PartialEq, Eq)] pub struct DependencyV1 { pub event: nat, pub generation: nat }
#[derive(PartialEq, Eq)] pub enum DependencyStatusV1 {
    Pending, Satisfied, Failed, QuiescentWithoutResult,
}
#[derive(PartialEq, Eq)] pub struct TicketV1 {
    pub parent: nat, pub child: nat, pub slot: nat, pub generation: nat,
}
#[derive(PartialEq, Eq)] pub struct FrontierV1 {
    pub allocation: AllocationV1, pub direction: DirectionV1,
    pub ticket: TicketV1, pub use_generation: nat, pub packet_offset: nat,
    pub packet_bytes: nat,
}
#[derive(PartialEq, Eq)] pub enum PhaseV1 {
    Idle, Ready, Published, FrontierPending, QuiescentWithoutResult,
    Completed, ProcessTeardown,
}
pub struct StateV1 {
    pub phase: PhaseV1, pub allocation: AllocationV1,
    pub transfer_id: nat, pub source: EndpointV1, pub destination: EndpointV1,
    pub direction: DirectionV1, pub total_bytes: nat, pub completed_bytes: nat,
    pub packet_offset: nat, pub packet_bytes: nat,
    pub dependencies: Seq<DependencyV1>, pub dependencies_satisfied: bool,
    pub ticket: Option<TicketV1>, pub frontier: Option<FrontierV1>,
    pub next_ticket_generation: nat, pub next_use_generation: nat,
    pub retired_packets: nat, pub dirty_packets: nat, pub completion_count: nat,
    pub destination_dirty_through: nat, pub authority_count: nat,
    pub target_retained: bool, pub result_succeeded: bool, pub result_code: int,
    pub current: bool,
}

pub open spec fn valid_allocation_v1(a: AllocationV1) -> bool {
    a.owner > 0 && a.identity > 0 && a.generation > 0 && a.pool_generation > 0
        && a.incarnation > 0 && 0 < a.logical_bytes <= max_transfer_bytes_v1()
}
pub open spec fn storage_extent_v1(storage: StorageV1) -> nat {
    match storage { StorageV1::Host(h) => h.byte_len, StorageV1::Device(a) => a.logical_bytes }
}
pub open spec fn valid_storage_v1(storage: StorageV1) -> bool {
    match storage {
        StorageV1::Host(h) => h.session > 0 && h.identity > 0 && h.generation > 0 && h.byte_len > 0,
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
pub open spec fn valid_dependencies_v1(deps: Seq<DependencyV1>) -> bool {
    forall|i: int| 0 <= i < deps.len() ==> deps[i].event > 0 && deps[i].generation > 0
        && forall|j: int| 0 <= j < deps.len() && i != j ==> deps[i] != deps[j]
}
pub open spec fn valid_ticket_v1(ticket: TicketV1, direction: DirectionV1) -> bool {
    ticket.parent == 91
        && ticket.child == match direction { DirectionV1::DeviceToHost => 3nat, DirectionV1::HostToDevice => 4nat }
        && ticket.slot < 64 && 0 < ticket.generation <= max_u32_v1()
}
pub open spec fn exact_frontier_v1(s: StateV1) -> FrontierV1 {
    FrontierV1 { allocation: s.allocation, direction: s.direction,
        ticket: s.ticket.unwrap(), use_generation: (s.next_use_generation - 1) as nat,
        packet_offset: s.packet_offset, packet_bytes: s.packet_bytes }
}
pub open spec fn base_identity_valid_v1(s: StateV1) -> bool {
    &&& valid_allocation_v1(s.allocation)
    &&& s.transfer_id > 0
    &&& resolved_direction_v1(s.source, s.destination) == Some(s.direction)
    &&& bound_allocation_v1(s.source, s.destination) == Some(s.allocation)
    &&& valid_endpoint_range_v1(s.source, s.total_bytes)
    &&& valid_endpoint_range_v1(s.destination, s.total_bytes)
    &&& valid_dependencies_v1(s.dependencies)
    &&& s.completed_bytes <= s.total_bytes
    &&& s.next_use_generation > 0
    &&& 0 < s.next_ticket_generation <= max_u32_v1()
    &&& s.destination_dirty_through == s.completed_bytes
    &&& s.authority_count == 1
}
pub open spec fn valid_state_v1(s: StateV1) -> bool {
    &&& base_identity_valid_v1(s)
    &&& match s.phase {
        PhaseV1::Idle => s.completed_bytes == 0 && s.packet_bytes == 0
            && s.ticket.is_none() && s.frontier.is_none() && !s.target_retained && s.current,
        PhaseV1::Ready => s.completed_bytes < s.total_bytes && s.packet_bytes == 0
            && s.packet_offset == s.completed_bytes && s.ticket.is_none()
            && s.frontier.is_none() && s.target_retained && !s.result_succeeded
            && s.result_code == 0 && s.current,
        PhaseV1::QuiescentWithoutResult => s.completed_bytes < s.total_bytes
            && s.packet_bytes == 0 && s.packet_offset == s.completed_bytes
            && s.ticket.is_none() && s.frontier.is_none() && s.current
            && s.dependencies.len() == 0 && s.target_retained && !s.result_succeeded
            && s.result_code == 0,
        PhaseV1::Published => s.completed_bytes < s.total_bytes
            && s.packet_offset == s.completed_bytes
            && 0 < s.packet_bytes <= max_packet_bytes_v1()
            && s.packet_offset + s.packet_bytes <= s.total_bytes
            && s.ticket.is_some() && valid_ticket_v1(s.ticket.unwrap(), s.direction)
            && s.frontier.is_none() && s.target_retained && !s.result_succeeded
            && s.result_code == 0 && s.current,
        PhaseV1::FrontierPending => s.completed_bytes < s.total_bytes
            && s.packet_offset == s.completed_bytes && s.packet_bytes > 0
            && s.packet_bytes <= max_packet_bytes_v1()
            && s.packet_offset + s.packet_bytes <= s.total_bytes
            && s.ticket.is_some() && s.frontier == Some(exact_frontier_v1(s))
            && s.target_retained && !s.result_succeeded && s.result_code == 0 && s.current,
        PhaseV1::Completed => ((s.result_succeeded && s.result_code == 0
                && s.completed_bytes == s.total_bytes)
                || (!s.result_succeeded && s.result_code < 0
                    && s.completed_bytes < s.total_bytes))
            && s.packet_bytes == 0 && s.ticket.is_none() && s.frontier.is_none()
            && s.completion_count > 0 && s.target_retained && s.current,
        PhaseV1::ProcessTeardown => !s.current && s.authority_count == 1
            && s.target_retained,
    }
}
pub open spec fn supported_request_v1(s: StateV1, source: EndpointV1,
    destination: EndpointV1, bytes: nat, direction: DirectionV1) -> bool
{
    resolved_direction_v1(source, destination) == Some(direction)
        && bound_allocation_v1(source, destination) == Some(s.allocation)
        && valid_endpoint_range_v1(source, bytes)
        && valid_endpoint_range_v1(destination, bytes)
}
pub open spec fn enqueue_v1(s: StateV1, transfer_id: nat, source: EndpointV1,
    destination: EndpointV1, bytes: nat, direction: DirectionV1,
    dependencies: Seq<DependencyV1>) -> StateV1
{
    if s.phase == PhaseV1::Idle && !s.target_retained && transfer_id > 0
        && valid_dependencies_v1(dependencies)
        && supported_request_v1(s, source, destination, bytes, direction) {
        StateV1 { phase: PhaseV1::Ready, transfer_id, source, destination, direction,
            total_bytes: bytes, completed_bytes: 0, packet_offset: 0, packet_bytes: 0,
            dependencies, dependencies_satisfied: dependencies.len() == 0,
            ticket: None, frontier: None, destination_dirty_through: 0,
            target_retained: true, result_succeeded: false, result_code: 0, ..s }
    } else { s }
}
pub open spec fn observe_dependencies_v1(s: StateV1, observed: Seq<DependencyV1>,
    status: DependencyStatusV1) -> StateV1
{
    if s.phase == PhaseV1::Ready && observed == s.dependencies {
        match status {
            DependencyStatusV1::Pending => StateV1 { dependencies_satisfied: false, ..s },
            DependencyStatusV1::Satisfied => StateV1 { dependencies_satisfied: true, ..s },
            DependencyStatusV1::Failed => StateV1 { phase: PhaseV1::Completed,
                completion_count: s.completion_count + 1, packet_bytes: 0,
                ticket: None, frontier: None, dependencies: Seq::empty(),
                dependencies_satisfied: true, result_code: -2, ..s },
            DependencyStatusV1::QuiescentWithoutResult => StateV1 {
                phase: PhaseV1::QuiescentWithoutResult, packet_bytes: 0,
                ticket: None, frontier: None, dependencies: Seq::empty(),
                dependencies_satisfied: true, ..s },
        }
    } else { s }
}
pub open spec fn chunk_bytes_v1(s: StateV1) -> nat {
    if s.total_bytes - s.completed_bytes <= max_packet_bytes_v1() {
        (s.total_bytes - s.completed_bytes) as nat
    } else { max_packet_bytes_v1() }
}
pub open spec fn planned_ticket_v1(s: StateV1) -> TicketV1 {
    TicketV1 { parent: 91,
        child: match s.direction { DirectionV1::DeviceToHost => 3, DirectionV1::HostToDevice => 4 },
        slot: s.next_ticket_generation % 64, generation: s.next_ticket_generation }
}
pub open spec fn publish_v1(s: StateV1) -> StateV1 {
    if s.phase == PhaseV1::Ready && valid_state_v1(s) && s.dependencies_satisfied
        && s.next_ticket_generation < max_u32_v1() {
        StateV1 { phase: PhaseV1::Published, packet_offset: s.completed_bytes,
            packet_bytes: chunk_bytes_v1(s), ticket: Some(planned_ticket_v1(s)),
            next_ticket_generation: s.next_ticket_generation + 1,
            next_use_generation: s.next_use_generation + 1, ..s }
    } else { s }
}
pub open spec fn retryable_restore_v1(s: StateV1) -> StateV1 {
    if s.phase == PhaseV1::Published && valid_state_v1(s) {
        StateV1 { phase: if s.completed_bytes == 0 { PhaseV1::Completed }
            else { PhaseV1::QuiescentWithoutResult }, packet_bytes: 0,
            ticket: None, frontier: None,
            dependencies: Seq::empty(), dependencies_satisfied: true,
            completion_count: if s.completed_bytes == 0 { s.completion_count + 1 }
                else { s.completion_count },
            result_code: if s.completed_bytes == 0 { -1 } else { 0 },
            ..s }
    } else { s }
}
pub open spec fn poll_pending_v1(s: StateV1, ticket: TicketV1) -> StateV1 { s }
pub open spec fn complete_packet_v1(s: StateV1, ticket: TicketV1) -> StateV1 {
    if s.phase == PhaseV1::Published && valid_state_v1(s) && s.ticket == Some(ticket) {
        StateV1 { phase: PhaseV1::FrontierPending,
            frontier: Some(exact_frontier_v1(s)), ..s }
    } else { s }
}
pub open spec fn retire_packet_v1(s: StateV1, frontier: FrontierV1) -> StateV1 {
    if s.phase == PhaseV1::FrontierPending && valid_state_v1(s)
        && s.frontier == Some(frontier) {
        let through = s.completed_bytes + s.packet_bytes;
        StateV1 { phase: if through == s.total_bytes { PhaseV1::Completed }
            else { PhaseV1::Ready }, completed_bytes: through,
            packet_offset: through, packet_bytes: 0, ticket: None, frontier: None,
            retired_packets: s.retired_packets + 1, dirty_packets: s.dirty_packets + 1,
            completion_count: if through == s.total_bytes { s.completion_count + 1 }
                else { s.completion_count }, destination_dirty_through: through,
            result_succeeded: through == s.total_bytes, ..s }
    } else { s }
}
pub open spec fn execute_one_chunk_v1(s: StateV1) -> StateV1 {
    let published = publish_v1(s);
    let pending = complete_packet_v1(published, planned_ticket_v1(s));
    retire_packet_v1(pending, exact_frontier_v1(published))
}
pub open spec fn cancel_v1(s: StateV1) -> StateV1 {
    if s.phase == PhaseV1::Ready && s.completed_bytes == 0 {
        StateV1 { phase: PhaseV1::Idle, packet_bytes: 0, ticket: None,
            frontier: None, dependencies: Seq::empty(), dependencies_satisfied: true,
            target_retained: false, ..s }
    } else { s }
}
pub open spec fn poll_submission_v1(s: StateV1, transfer_id: nat) -> Option<PhaseV1> {
    if (s.phase == PhaseV1::QuiescentWithoutResult || s.phase == PhaseV1::Completed)
        && s.transfer_id == transfer_id {
        Some(s.phase)
    } else { None }
}
pub open spec fn release_terminal_v1(s: StateV1, transfer_id: nat) -> StateV1 {
    if (s.phase == PhaseV1::QuiescentWithoutResult || s.phase == PhaseV1::Completed)
        && s.transfer_id == transfer_id && s.target_retained {
        StateV1 { phase: PhaseV1::Idle, completed_bytes: 0, packet_offset: 0,
            total_bytes: s.allocation.logical_bytes, source: host_endpoint_v1(),
            destination: device_endpoint_v1(), direction: DirectionV1::HostToDevice,
            destination_dirty_through: 0, dependencies_satisfied: true,
            target_retained: false, result_succeeded: false, result_code: 0, ..s }
    } else { s }
}
pub open spec fn opaque_failure_v1(s: StateV1) -> StateV1 {
    if s.phase == PhaseV1::Published {
        StateV1 { phase: PhaseV1::ProcessTeardown, current: false, ..s }
    } else { s }
}
pub open spec fn packets_needed_v1(bytes: nat) -> nat decreases bytes {
    if bytes == 0 { 0 } else if bytes <= max_packet_bytes_v1() { 1 }
    else { 1 + packets_needed_v1((bytes - max_packet_bytes_v1()) as nat) }
}
pub open spec fn run_chunks_v1(s: StateV1) -> StateV1
    decreases (s.total_bytes - s.completed_bytes) as nat,
{
    if s.phase != PhaseV1::Ready || !valid_state_v1(s) || !s.dependencies_satisfied
        || s.next_ticket_generation >= max_u32_v1() { s }
    else {
        let next = execute_one_chunk_v1(s);
        if next.phase == PhaseV1::Completed { next } else { run_chunks_v1(next) }
    }
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
pub open spec fn sample_idle_v1(direction: DirectionV1) -> StateV1 {
    StateV1 { phase: PhaseV1::Idle, allocation: sample_allocation_v1(), transfer_id: 9,
        source: if direction == DirectionV1::HostToDevice { host_endpoint_v1() } else { device_endpoint_v1() },
        destination: if direction == DirectionV1::HostToDevice { device_endpoint_v1() } else { host_endpoint_v1() },
        direction, total_bytes: max_transfer_bytes_v1(), completed_bytes: 0,
        packet_offset: 0, packet_bytes: 0, dependencies: Seq::empty(),
        dependencies_satisfied: true, ticket: None, frontier: None,
        next_ticket_generation: 10, next_use_generation: 11,
        retired_packets: 0, dirty_packets: 0, completion_count: 0,
        destination_dirty_through: 0, authority_count: 1,
        target_retained: false, result_succeeded: false, result_code: 0, current: true }
}
pub open spec fn enqueued_full_v1(direction: DirectionV1) -> StateV1 {
    let idle = sample_idle_v1(direction);
    enqueue_v1(idle, 10, idle.source, idle.destination, max_transfer_bytes_v1(),
        direction, Seq::empty())
}

pub proof fn fixed_packet_and_transfer_bounds_v1()
    ensures max_transfer_bytes_v1() == 268435456,
        max_packet_bytes_v1() == 4194272, max_u32_v1() == 4294967295, {}
pub proof fn sample_idle_states_are_valid_v1()
    ensures valid_state_v1(sample_idle_v1(DirectionV1::HostToDevice)),
        valid_state_v1(sample_idle_v1(DirectionV1::DeviceToHost)), {}
pub proof fn h2d_and_d2h_storage_roles_are_exact_v1()
    ensures resolved_direction_v1(host_endpoint_v1(), device_endpoint_v1())
            == Some(DirectionV1::HostToDevice),
        resolved_direction_v1(device_endpoint_v1(), host_endpoint_v1())
            == Some(DirectionV1::DeviceToHost), {}
pub proof fn h2h_and_d2d_are_unsupported_v1()
    ensures resolved_direction_v1(host_endpoint_v1(), host_endpoint_v1()).is_none(),
        resolved_direction_v1(device_endpoint_v1(), device_endpoint_v1()).is_none(), {}
pub proof fn unsupported_preflight_is_mutation_free_v1(s: StateV1, endpoint: EndpointV1)
    requires s.phase == PhaseV1::Idle,
        resolved_direction_v1(endpoint, endpoint).is_none(),
    ensures enqueue_v1(s, 20, endpoint, endpoint, 4096, DirectionV1::HostToDevice,
        Seq::empty()) == s, {}
pub proof fn dependency_identity_mismatch_is_atomic_v1(s: StateV1,
    observed: Seq<DependencyV1>)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready,
        observed != s.dependencies,
    ensures observe_dependencies_v1(s, observed, DependencyStatusV1::Satisfied) == s, {}
pub proof fn dependency_failure_and_quiescence_settle_retained_target_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready,
    ensures {
        let failed = observe_dependencies_v1(s, s.dependencies, DependencyStatusV1::Failed);
        let quiescent = observe_dependencies_v1(s, s.dependencies,
            DependencyStatusV1::QuiescentWithoutResult);
        &&& valid_state_v1(failed) &&& failed.phase == PhaseV1::Completed
        &&& failed.target_retained &&& failed.ticket.is_none()
        &&& failed.result_code == -2
        &&& valid_state_v1(quiescent)
        &&& quiescent.phase == PhaseV1::QuiescentWithoutResult
        &&& quiescent.target_retained &&& quiescent.ticket.is_none()
    }, {}
pub proof fn unsatisfied_dependencies_block_publish_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready,
        !s.dependencies_satisfied,
    ensures publish_v1(s) == s, {}
pub proof fn publish_binds_direction_storage_offset_and_ticket_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready,
        s.dependencies_satisfied, s.next_ticket_generation < max_u32_v1(),
    ensures {
        let p = publish_v1(s);
        &&& valid_state_v1(p) &&& p.phase == PhaseV1::Published
        &&& p.source == s.source &&& p.destination == s.destination
        &&& p.direction == s.direction &&& p.packet_offset == s.completed_bytes
        &&& p.ticket == Some(planned_ticket_v1(s))
        &&& p.packet_bytes == chunk_bytes_v1(s)
    }, {}
pub proof fn poll_never_publishes_continuation_v1(s: StateV1, ticket: TicketV1)
    ensures poll_pending_v1(s, ticket) == s, {}
pub proof fn stale_completion_ticket_is_atomic_v1(s: StateV1, stale: TicketV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
        s.ticket != Some(stale),
    ensures complete_packet_v1(s, stale) == s, {}
pub proof fn completion_creates_exact_r19_frontier_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
    ensures {
        let pending = complete_packet_v1(s, s.ticket.unwrap());
        &&& valid_state_v1(pending) &&& pending.phase == PhaseV1::FrontierPending
        &&& pending.frontier == Some(exact_frontier_v1(s))
    }, {}
pub proof fn stale_frontier_retirement_is_atomic_v1(s: StateV1, stale: FrontierV1)
    requires valid_state_v1(s), s.phase == PhaseV1::FrontierPending,
        s.frontier != Some(stale),
    ensures retire_packet_v1(s, stale) == s, {}
pub proof fn exact_retirement_advances_dirty_and_progress_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::FrontierPending,
    ensures {
        let retired = retire_packet_v1(s, s.frontier.unwrap());
        &&& valid_state_v1(retired)
        &&& retired.completed_bytes == s.completed_bytes + s.packet_bytes
        &&& retired.destination_dirty_through == retired.completed_bytes
        &&& retired.dirty_packets == s.dirty_packets + 1
        &&& retired.retired_packets == s.retired_packets + 1
        &&& retired.ticket.is_none() &&& retired.frontier.is_none()
    }, {}
pub proof fn one_chunk_composes_actual_prior_states_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready,
        s.dependencies_satisfied, s.next_ticket_generation < max_u32_v1(),
    ensures {
        let next = execute_one_chunk_v1(s);
        &&& valid_state_v1(next)
        &&& next.completed_bytes == s.completed_bytes + chunk_bytes_v1(s)
        &&& next.retired_packets == s.retired_packets + 1
        &&& next.dirty_packets == s.dirty_packets + 1
        &&& next.allocation == s.allocation &&& next.source == s.source
        &&& next.destination == s.destination &&& next.direction == s.direction
        &&& next.ticket.is_none() &&& next.frontier.is_none()
        &&& (next.phase == PhaseV1::Ready || next.phase == PhaseV1::Completed)
    },
{
    publish_binds_direction_storage_offset_and_ticket_v1(s);
    let published = publish_v1(s);
    completion_creates_exact_r19_frontier_v1(published);
    let pending = complete_packet_v1(published, published.ticket.unwrap());
    exact_retirement_advances_dirty_and_progress_v1(pending);
}
pub proof fn retryable_failure_restores_exact_custody_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
    ensures {
        let restored = retryable_restore_v1(s);
        &&& valid_state_v1(restored) &&& restored.source == s.source
        &&& restored.destination == s.destination &&& restored.direction == s.direction
        &&& restored.completed_bytes == s.completed_bytes
        &&& restored.ticket.is_none() &&& restored.frontier.is_none()
        &&& restored.target_retained
        &&& (s.completed_bytes == 0 ==> restored.phase == PhaseV1::Completed)
        &&& (s.completed_bytes > 0 ==> restored.phase == PhaseV1::QuiescentWithoutResult)
    }, {}
pub proof fn partial_retry_is_quiescent_without_result_v1() {
    let ready = enqueued_full_v1(DirectionV1::HostToDevice);
    let first = execute_one_chunk_v1(ready);
    let published = publish_v1(first);
    let restored = retryable_restore_v1(published);
    assert(valid_state_v1(restored));
    assert(restored.completed_bytes == max_packet_bytes_v1());
    assert(restored.phase == PhaseV1::QuiescentWithoutResult);
    assert(restored.ticket.is_none()); assert(restored.frontier.is_none());
    let released = release_terminal_v1(restored, restored.transfer_id);
    assert(valid_state_v1(released)); assert(released.phase == PhaseV1::Idle);
    assert(!released.target_retained);
}
pub proof fn zero_progress_retry_is_conclusive_failed_v1() {
    let ready = enqueued_full_v1(DirectionV1::HostToDevice);
    let published = publish_v1(ready);
    let failed = retryable_restore_v1(published);
    assert(valid_state_v1(failed)); assert(failed.phase == PhaseV1::Completed);
    assert(failed.completed_bytes == 0); assert(failed.target_retained);
    assert(failed.result_code == -1);
    assert(poll_submission_v1(failed, failed.transfer_id) == Some(PhaseV1::Completed));
}
pub proof fn quiescent_marker_is_exact_pollable_and_not_resumable_v1(s: StateV1, foreign: nat)
    requires valid_state_v1(s), s.phase == PhaseV1::QuiescentWithoutResult,
        foreign != s.transfer_id,
    ensures poll_submission_v1(s, s.transfer_id) == Some(PhaseV1::QuiescentWithoutResult),
        poll_submission_v1(s, foreign).is_none(), publish_v1(s) == s,
        cancel_v1(s) == s, {}
pub proof fn quiescent_release_requires_exact_submission_v1(s: StateV1, foreign: nat)
    requires valid_state_v1(s), s.phase == PhaseV1::QuiescentWithoutResult,
        foreign != s.transfer_id,
    ensures release_terminal_v1(s, foreign) == s,
        release_terminal_v1(s, s.transfer_id).phase == PhaseV1::Idle,
        !release_terminal_v1(s, s.transfer_id).target_retained, {}
pub proof fn cancellation_only_before_progress_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready,
    ensures (s.completed_bytes == 0 ==> cancel_v1(s).phase == PhaseV1::Idle),
        (s.completed_bytes > 0 ==> cancel_v1(s) == s), {}
pub proof fn published_cancellation_is_too_late_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
    ensures cancel_v1(s) == s, {}
pub proof fn opaque_failure_retains_single_authority_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
    ensures {
        let terminal = opaque_failure_v1(s);
        &&& valid_state_v1(terminal) &&& terminal.phase == PhaseV1::ProcessTeardown
        &&& terminal.authority_count == 1 &&& !terminal.current
        &&& terminal.ticket == s.ticket &&& terminal.allocation == s.allocation
    }, {}
pub proof fn allocation_pool_aba_changes_frontier_identity_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published,
    ensures {
        let changed = AllocationV1 { pool_generation: s.allocation.pool_generation + 1,
            ..s.allocation };
        &&& changed != s.allocation
        &&& FrontierV1 { allocation: changed, ..exact_frontier_v1(s) }
            != exact_frontier_v1(s)
    }, {}
pub proof fn currentness_loss_cannot_be_ready_v1(s: StateV1)
    requires valid_state_v1(s), !s.current,
    ensures s.phase == PhaseV1::ProcessTeardown, {}

pub proof fn recursive_chunks_preserve_exact_progress_v1(s: StateV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Ready,
        s.dependencies_satisfied,
        s.next_ticket_generation + packets_needed_v1((s.total_bytes - s.completed_bytes) as nat)
            < max_u32_v1(),
    ensures {
        let done = run_chunks_v1(s);
        &&& valid_state_v1(done) &&& done.phase == PhaseV1::Completed
        &&& done.completed_bytes == s.total_bytes
        &&& done.allocation == s.allocation &&& done.source == s.source
        &&& done.destination == s.destination &&& done.direction == s.direction
        &&& done.destination_dirty_through == s.total_bytes
        &&& done.retired_packets == s.retired_packets
            + packets_needed_v1((s.total_bytes - s.completed_bytes) as nat)
        &&& done.dirty_packets == s.dirty_packets
            + packets_needed_v1((s.total_bytes - s.completed_bytes) as nat)
        &&& done.completion_count == s.completion_count + 1
        &&& done.ticket.is_none() &&& done.frontier.is_none()
    },
    decreases (s.total_bytes - s.completed_bytes) as nat,
{
    one_chunk_composes_actual_prior_states_v1(s);
    let next = execute_one_chunk_v1(s);
    if next.phase != PhaseV1::Completed {
        assert(next.phase == PhaseV1::Ready);
        assert(next.total_bytes - next.completed_bytes < s.total_bytes - s.completed_bytes);
        assert(packets_needed_v1((s.total_bytes - s.completed_bytes) as nat)
            == 1 + packets_needed_v1((next.total_bytes - next.completed_bytes) as nat));
        recursive_chunks_preserve_exact_progress_v1(next);
    }
}

fn main() {}
pub proof fn full_256_mib_is_exactly_65_retired_packets_v1() {
    let initial = enqueued_full_v1(DirectionV1::HostToDevice);
    assert(packets_needed_v1(max_transfer_bytes_v1()) == 65) by (compute);
    recursive_chunks_preserve_exact_progress_v1(initial);
    let done = run_chunks_v1(initial);
    assert(done.phase == PhaseV1::Completed);
    assert(done.retired_packets == 65);
    assert(done.dirty_packets == 65);
    assert(done.completed_bytes == 268435456);
}
pub proof fn repeated_and_mixed_directions_are_admitted_after_completion_v1() {
    let h2d = enqueued_full_v1(DirectionV1::HostToDevice);
    assert(packets_needed_v1(max_transfer_bytes_v1()) == 65) by (compute);
    recursive_chunks_preserve_exact_progress_v1(h2d);
    let h2d_done = run_chunks_v1(h2d);
    let reusable = release_terminal_v1(h2d_done, h2d_done.transfer_id);
    assert(valid_state_v1(reusable));
    assert(reusable.phase == PhaseV1::Idle);
    assert(supported_request_v1(reusable, reusable.source, reusable.destination,
        reusable.total_bytes, DirectionV1::HostToDevice));
    assert(supported_request_v1(reusable, reusable.destination, reusable.source,
        reusable.total_bytes, DirectionV1::DeviceToHost));
    assert(valid_dependencies_v1(Seq::<DependencyV1>::empty()));
    let same = enqueue_v1(reusable, 21, reusable.source, reusable.destination,
        reusable.total_bytes, DirectionV1::HostToDevice, Seq::empty());
    let mixed = enqueue_v1(reusable, 22, reusable.destination, reusable.source,
        reusable.total_bytes, DirectionV1::DeviceToHost, Seq::empty());
    assert(valid_state_v1(same)); assert(same.phase == PhaseV1::Ready);
    assert(valid_state_v1(mixed)); assert(mixed.phase == PhaseV1::Ready);
}

}
