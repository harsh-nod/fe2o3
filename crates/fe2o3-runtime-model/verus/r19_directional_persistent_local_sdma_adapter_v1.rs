// Bounded R19 directional-pair model. This is a versioned successor, not a
// Rust/KFD/native refinement and proves no hardware liveness or performance.
use vstd::prelude::*;

verus! {

pub open spec fn max_allocation_bytes_v1() -> nat { 256 * 1024 * 1024 }
pub open spec fn max_copy_bytes_v1() -> nat { 0x003f_ffe0 }
pub open spec fn page_bytes_v1() -> nat { 4096 }
pub open spec fn queue_slots_v1() -> nat { 64 }
pub open spec fn native_queue_limit_v1() -> nat { 1024 }
pub open spec fn max_u32_v1() -> nat { 0xffff_ffff }
pub open spec fn min_i32_v1() -> int { -0x8000_0000 }
pub open spec fn max_i32_v1() -> int { 0x7fff_ffff }

#[derive(PartialEq, Eq)] pub struct DeviceV1 { pub physical: nat, pub generation: nat }
#[derive(PartialEq, Eq)] pub struct VmV1 { pub device: DeviceV1, pub id: nat }
#[derive(PartialEq, Eq)] pub struct QueueV1 { pub vm: VmV1, pub id: nat, pub generation: nat }
#[derive(PartialEq, Eq)] pub struct AllocationV1 {
    pub owner: nat, pub vm: VmV1, pub allocation_id: nat,
    pub allocation_generation: nat, pub mapping_id: nat,
    pub logical_bytes: nat, pub physical_bytes: nat, pub pool_generation: nat,
}
#[derive(PartialEq, Eq)] pub struct ChildV1 { pub native_queue_id: nat, pub engine: nat }
#[derive(PartialEq, Eq)] pub struct PairV1 {
    pub parent: QueueV1, pub occurrence: nat, pub attachment_generation: nat,
    pub d2h: ChildV1, pub h2d: ChildV1,
}
#[derive(PartialEq, Eq)] pub enum DirectionV1 { DeviceToHost, HostToDevice }
#[derive(PartialEq, Eq)] pub enum AccessV1 { Read, Write }
#[derive(PartialEq, Eq)] pub enum EndpointV1 { Source, Destination }
#[derive(PartialEq, Eq)] pub struct HostV1 {
    pub session: nat, pub identity: nat, pub generation: nat,
    pub byte_len: nat, pub coherent: bool,
}
#[derive(PartialEq, Eq)] pub struct RangeV1 { pub offset: nat, pub byte_len: nat }
#[derive(PartialEq, Eq)] pub struct TicketV1 {
    pub owner: QueueV1, pub native_queue_id: nat, pub slot: nat, pub generation: nat,
}
#[derive(PartialEq, Eq)] pub struct BindingV1 {
    pub allocation: AllocationV1, pub pair: PairV1, pub adapter_incarnation: nat,
    pub host: HostV1, pub device_range: RangeV1, pub host_range: RangeV1,
    pub direction: DirectionV1, pub access: AccessV1, pub endpoint: EndpointV1,
    pub use_slot: nat, pub use_generation: nat, pub ticket: TicketV1,
}

// Whole allocation and pair values bind VM/allocation/mapping/pool and
// parent/occurrence/attachment/children before retirement is attempted.
#[derive(PartialEq, Eq)] pub struct FrontierV1 {
    pub allocation: AllocationV1, pub pair: PairV1, pub adapter_incarnation: nat,
    pub use_slot: nat, pub use_generation: nat, pub frontier_generation: nat,
    pub direction: DirectionV1,
}
#[derive(PartialEq, Eq)] pub enum TerminalStatusV1 { Succeeded, Failed(int) }
#[derive(PartialEq, Eq)] pub enum PhaseV1 {
    Prepared, Published, TimedOut, Completed, Restored, FrontierPending, Idle, Quarantined,
}
#[derive(PartialEq, Eq)] pub enum LocationV1 {
    PreparedRequest, NativeChild, CompletionBatch, PersistentOwner, Quarantine,
}
#[derive(PartialEq, Eq)] pub struct StateV1 {
    pub binding: BindingV1, pub phase: PhaseV1, pub location: LocationV1,
    pub live_ticket: Option<TicketV1>, pub terminal_status: Option<TerminalStatusV1>,
    pub frontier: Option<FrontierV1>, pub current: bool, pub authority_count: nat,
    pub occupied_slots: nat, pub next_use_generation: nat, pub retired_uses: nat,
    pub next_ticket_slot: nat, pub next_ticket_generation: nat,
}

pub open spec fn child_v1(pair: PairV1, direction: DirectionV1) -> ChildV1 {
    match direction { DirectionV1::DeviceToHost => pair.d2h, DirectionV1::HostToDevice => pair.h2d }
}
pub open spec fn access_v1(direction: DirectionV1) -> AccessV1 {
    match direction { DirectionV1::DeviceToHost => AccessV1::Read, DirectionV1::HostToDevice => AccessV1::Write }
}
pub open spec fn endpoint_v1(direction: DirectionV1) -> EndpointV1 {
    match direction { DirectionV1::DeviceToHost => EndpointV1::Source, DirectionV1::HostToDevice => EndpointV1::Destination }
}
pub open spec fn valid_allocation_v1(a: AllocationV1) -> bool {
    &&& a.owner > 0 &&& a.vm.device.physical > 0 &&& a.vm.device.generation > 0
    &&& a.vm.id > 0 &&& a.allocation_id > 0 &&& a.allocation_generation > 0
    &&& a.mapping_id > 0 &&& 0 < a.logical_bytes <= a.physical_bytes
    &&& a.physical_bytes <= max_allocation_bytes_v1()
    &&& a.physical_bytes % page_bytes_v1() == 0 &&& a.pool_generation > 0
}
pub open spec fn valid_pair_v1(p: PairV1, a: AllocationV1) -> bool {
    &&& p.parent.vm == a.vm &&& p.parent.id > 0 &&& p.parent.generation > 0
    &&& p.occurrence > 0 &&& p.attachment_generation > 0
    &&& p.d2h.native_queue_id < native_queue_limit_v1()
    &&& p.h2d.native_queue_id < native_queue_limit_v1()
    &&& p.d2h.native_queue_id != p.h2d.native_queue_id
    &&& p.d2h.engine == 0 &&& p.h2d.engine == 1
}
pub open spec fn valid_range_v1(r: RangeV1, extent: nat) -> bool {
    r.byte_len > 0 && r.offset + r.byte_len <= extent
}
pub open spec fn valid_direction_v1(b: BindingV1) -> bool {
    &&& b.access == access_v1(b.direction) &&& b.endpoint == endpoint_v1(b.direction)
    &&& match b.direction {
        DirectionV1::DeviceToHost => child_v1(b.pair, b.direction).engine == 0,
        DirectionV1::HostToDevice => child_v1(b.pair, b.direction).engine == 1,
    }
}
pub open spec fn exact_ticket_v1(b: BindingV1) -> bool {
    &&& b.ticket.owner == b.pair.parent
    &&& b.ticket.native_queue_id == child_v1(b.pair, b.direction).native_queue_id
    &&& b.ticket.native_queue_id < native_queue_limit_v1()
    &&& b.ticket.slot < queue_slots_v1()
    &&& 0 < b.ticket.generation <= max_u32_v1()
}
pub open spec fn valid_binding_v1(b: BindingV1) -> bool {
    &&& valid_allocation_v1(b.allocation) &&& valid_pair_v1(b.pair, b.allocation)
    &&& b.adapter_incarnation > 0 &&& b.host.session > 0 &&& b.host.identity > 0
    &&& b.host.generation > 0 &&& b.host.byte_len > 0 &&& b.host.coherent
    &&& b.use_slot < queue_slots_v1() &&& b.use_generation > 0
    &&& valid_direction_v1(b) &&& b.device_range.byte_len == b.host_range.byte_len
    &&& b.device_range.byte_len <= max_copy_bytes_v1()
    &&& valid_range_v1(b.device_range, b.allocation.logical_bytes)
    &&& valid_range_v1(b.host_range, b.host.byte_len) &&& exact_ticket_v1(b)
}
pub open spec fn exact_frontier_v1(b: BindingV1, generation: nat) -> FrontierV1 {
    FrontierV1 { allocation: b.allocation, pair: b.pair, adapter_incarnation: b.adapter_incarnation,
        use_slot: b.use_slot, use_generation: b.use_generation,
        frontier_generation: generation, direction: b.direction }
}
pub open spec fn valid_terminal_status_v1(s: TerminalStatusV1) -> bool {
    match s {
        TerminalStatusV1::Succeeded => true,
        TerminalStatusV1::Failed(c) => min_i32_v1() <= c <= max_i32_v1(),
    }
}
pub open spec fn valid_state_v1(s: StateV1) -> bool {
    &&& valid_binding_v1(s.binding) &&& s.authority_count == 1 &&& s.occupied_slots <= 1
    &&& s.next_use_generation > s.binding.use_generation
    &&& s.next_ticket_slot < queue_slots_v1()
    &&& 0 < s.next_ticket_generation <= max_u32_v1()
    &&& match s.terminal_status { Some(status) => valid_terminal_status_v1(status), None => true }
    &&& match s.phase {
        PhaseV1::Prepared => s.location == LocationV1::PreparedRequest && s.live_ticket.is_none()
            && s.terminal_status.is_none() && s.frontier.is_none() && s.current && s.occupied_slots == 1,
        PhaseV1::Published | PhaseV1::TimedOut => s.location == LocationV1::NativeChild
            && s.live_ticket == Some(s.binding.ticket) && s.terminal_status.is_none()
            && s.frontier.is_none() && s.current && s.occupied_slots == 1,
        PhaseV1::Completed => s.location == LocationV1::CompletionBatch
            && s.live_ticket == Some(s.binding.ticket) && s.terminal_status.is_some()
            && s.frontier.is_none() && s.current && s.occupied_slots == 1,
        PhaseV1::Restored => s.location == LocationV1::PersistentOwner
            && s.live_ticket == Some(s.binding.ticket) && s.terminal_status.is_some()
            && s.frontier.is_none() && s.current && s.occupied_slots == 1,
        PhaseV1::FrontierPending => s.location == LocationV1::PersistentOwner
            && s.live_ticket.is_none() && s.terminal_status.is_some()
            && s.frontier == Some(exact_frontier_v1(s.binding, s.retired_uses + 1))
            && s.current && s.occupied_slots == 1,
        PhaseV1::Idle => s.location == LocationV1::PersistentOwner && s.live_ticket.is_none()
            && s.terminal_status.is_none() && s.frontier.is_none() && s.current && s.occupied_slots == 0,
        PhaseV1::Quarantined => s.location == LocationV1::Quarantine
            && (s.live_ticket.is_none() || s.live_ticket == Some(s.binding.ticket))
            && s.terminal_status.is_none() && s.frontier.is_none()
            && !s.current && s.occupied_slots == 1,
    }
}
pub open spec fn reusable_owner_v1(s: StateV1) -> bool {
    valid_state_v1(s) && s.phase == PhaseV1::Idle && s.current
        && s.occupied_slots == 0 && s.frontier.is_none()
}
pub open spec fn same_persistent_identity_v1(a: BindingV1, b: BindingV1) -> bool {
    a.allocation == b.allocation && a.pair == b.pair
        && a.adapter_incarnation == b.adapter_incarnation
}
pub open spec fn can_prepare_v1(prior: StateV1, next: BindingV1) -> bool {
    &&& reusable_owner_v1(prior) &&& valid_binding_v1(next)
    &&& same_persistent_identity_v1(prior.binding, next)
    &&& prior.next_ticket_generation < max_u32_v1()
    &&& next.use_slot == 0 &&& next.use_generation == prior.next_use_generation
    &&& next.ticket.slot == prior.next_ticket_slot
    &&& next.ticket.generation == prior.next_ticket_generation
}
pub open spec fn prepare_from_state_v1(prior: StateV1, next: BindingV1) -> StateV1 {
    if can_prepare_v1(prior, next) {
        StateV1 { binding: next, phase: PhaseV1::Prepared, location: LocationV1::PreparedRequest,
            live_ticket: None, terminal_status: None, frontier: None, current: true,
            authority_count: 1, occupied_slots: 1,
            next_use_generation: prior.next_use_generation + 1, retired_uses: prior.retired_uses,
            next_ticket_slot: if prior.next_ticket_slot + 1 < queue_slots_v1() {
                prior.next_ticket_slot + 1
            } else { 0 },
            next_ticket_generation: prior.next_ticket_generation + 1 }
    } else { prior }
}
pub open spec fn confirm_v1(s: StateV1, ticket: TicketV1) -> StateV1 {
    if valid_state_v1(s) && s.phase == PhaseV1::Prepared && ticket == s.binding.ticket {
        StateV1 { phase: PhaseV1::Published, location: LocationV1::NativeChild,
            live_ticket: Some(ticket), ..s }
    } else { s }
}
pub open spec fn recover_v1(s: StateV1) -> StateV1 {
    if valid_state_v1(s) && s.phase == PhaseV1::Prepared {
        StateV1 { phase: PhaseV1::Idle, location: LocationV1::PersistentOwner,
            occupied_slots: 0, ..s }
    } else { s }
}
pub open spec fn quarantine_preparation_v1(s: StateV1) -> StateV1 {
    if valid_state_v1(s) && s.phase == PhaseV1::Prepared {
        StateV1 { phase: PhaseV1::Quarantined, location: LocationV1::Quarantine,
            live_ticket: None, current: false, ..s }
    } else { s }
}
pub open spec fn quarantine_retained_v1(s: StateV1) -> StateV1 {
    if valid_state_v1(s) && s.phase == PhaseV1::Prepared {
        StateV1 { phase: PhaseV1::Quarantined, location: LocationV1::Quarantine,
            live_ticket: Some(s.binding.ticket), current: false, ..s }
    } else { s }
}
pub open spec fn pending_v1(s: StateV1, ticket: TicketV1) -> StateV1 { s }
pub open spec fn timeout_v1(s: StateV1, ticket: TicketV1) -> StateV1 {
    if valid_state_v1(s) && s.phase == PhaseV1::Published && ticket == s.binding.ticket {
        StateV1 { phase: PhaseV1::TimedOut, ..s }
    } else { s }
}
pub open spec fn complete_v1(s: StateV1, ticket: TicketV1, status: TerminalStatusV1) -> StateV1 {
    if valid_state_v1(s) && (s.phase == PhaseV1::Published || s.phase == PhaseV1::TimedOut)
        && ticket == s.binding.ticket && valid_terminal_status_v1(status) {
        StateV1 { phase: PhaseV1::Completed, location: LocationV1::CompletionBatch,
            terminal_status: Some(status), ..s }
    } else { s }
}
pub open spec fn completion_ambiguous_v1(s: StateV1) -> StateV1 {
    if valid_state_v1(s) && (s.phase == PhaseV1::Published || s.phase == PhaseV1::TimedOut) {
        StateV1 { phase: PhaseV1::Quarantined, location: LocationV1::Quarantine,
            current: false, ..s }
    } else { s }
}
pub open spec fn restore_v1(s: StateV1, status: TerminalStatusV1, child_current: bool) -> StateV1 {
    if valid_state_v1(s) && s.phase == PhaseV1::Completed && s.terminal_status == Some(status) {
        if child_current {
            StateV1 { phase: PhaseV1::Restored, location: LocationV1::PersistentOwner, ..s }
        } else {
            StateV1 { phase: PhaseV1::Quarantined, location: LocationV1::Quarantine,
                terminal_status: None, current: false, ..s }
        }
    } else { s }
}
pub open spec fn settle_v1(s: StateV1, status: TerminalStatusV1) -> StateV1 {
    if valid_state_v1(s) && s.phase == PhaseV1::Restored && s.terminal_status == Some(status) {
        StateV1 { phase: PhaseV1::FrontierPending, live_ticket: None,
            frontier: Some(exact_frontier_v1(s.binding, s.retired_uses + 1)), ..s }
    } else { s }
}
pub open spec fn retire_v1(s: StateV1, observed: FrontierV1) -> StateV1 {
    if valid_state_v1(s) && s.phase == PhaseV1::FrontierPending && s.frontier == Some(observed) {
        StateV1 { phase: PhaseV1::Idle, frontier: None, terminal_status: None,
            occupied_slots: 0, retired_uses: s.retired_uses + 1, ..s }
    } else { s }
}
pub open spec fn can_release_v1(s: StateV1) -> bool { reusable_owner_v1(s) }
pub open spec fn can_rebind_v1(s: StateV1, pair: PairV1) -> bool {
    &&& reusable_owner_v1(s)
    &&& valid_pair_v1(pair, s.binding.allocation)
    &&& pair.attachment_generation == s.binding.pair.attachment_generation + 1
}
pub open spec fn rebound_binding_v1(s: StateV1, pair: PairV1) -> BindingV1 {
    BindingV1 { pair, ticket: TicketV1 { owner: pair.parent,
        native_queue_id: child_v1(pair, s.binding.direction).native_queue_id,
        ..s.binding.ticket }, ..s.binding }
}
pub open spec fn rebind_v1(s: StateV1, pair: PairV1) -> StateV1 {
    if can_rebind_v1(s, pair) { StateV1 { binding: rebound_binding_v1(s, pair), ..s } }
    else { s }
}
pub open spec fn can_demote_v1(s: StateV1) -> bool { reusable_owner_v1(s) }
pub open spec fn repromoted_binding_v1(s: StateV1) -> BindingV1 {
    BindingV1 { allocation: AllocationV1 {
            pool_generation: s.binding.allocation.pool_generation + 1, ..s.binding.allocation },
        adapter_incarnation: s.binding.adapter_incarnation + 1, ..s.binding }
}
pub open spec fn demote_repromote_v1(s: StateV1) -> StateV1 {
    if can_demote_v1(s) { StateV1 { binding: repromoted_binding_v1(s), ..s } } else { s }
}
pub open spec fn next_binding_v1(s: StateV1, direction: DirectionV1) -> BindingV1 {
    // This bounded model uses one abstract pair-global rotating coordinate.
    // Concrete child queues plan independently; no refinement is claimed.
    BindingV1 { direction, access: access_v1(direction), endpoint: endpoint_v1(direction),
        use_slot: 0, use_generation: s.next_use_generation,
        ticket: TicketV1 { owner: s.binding.pair.parent,
            native_queue_id: child_v1(s.binding.pair, direction).native_queue_id,
            slot: s.next_ticket_slot, generation: s.next_ticket_generation }, ..s.binding }
}
pub open spec fn execute_and_retire_one_v1(s: StateV1, direction: DirectionV1) -> StateV1 {
    let b = next_binding_v1(s, direction);
    let prepared = prepare_from_state_v1(s, b);
    let published = confirm_v1(prepared, b.ticket);
    let completed = complete_v1(published, b.ticket, TerminalStatusV1::Succeeded);
    let restored = restore_v1(completed, TerminalStatusV1::Succeeded, true);
    let pending = settle_v1(restored, TerminalStatusV1::Succeeded);
    retire_v1(pending, exact_frontier_v1(b, s.retired_uses + 1))
}
pub open spec fn direction_for_step_v1(steps: nat, alternating: bool) -> DirectionV1 {
    if alternating {
        if steps % 2 == 0 { DirectionV1::HostToDevice } else { DirectionV1::DeviceToHost }
    } else {
        DirectionV1::HostToDevice
    }
}
pub open spec fn run_retired_steps_v1(s: StateV1, steps: nat, alternating: bool) -> StateV1
    decreases steps,
{
    if steps == 0 { s } else {
        let next = execute_and_retire_one_v1(s, direction_for_step_v1(steps, alternating));
        run_retired_steps_v1(next, (steps - 1) as nat, alternating)
    }
}

pub open spec fn sample_allocation_v1() -> AllocationV1 {
    AllocationV1 { owner: 1, vm: VmV1 { device: DeviceV1 { physical: 2, generation: 3 }, id: 4 },
        allocation_id: 5, allocation_generation: 6, mapping_id: 7,
        logical_bytes: 65537, physical_bytes: 69632, pool_generation: 8 }
}
pub open spec fn sample_pair_v1() -> PairV1 {
    PairV1 { parent: QueueV1 { vm: sample_allocation_v1().vm, id: 9, generation: 10 },
        occurrence: 11, attachment_generation: 12,
        d2h: ChildV1 { native_queue_id: 0, engine: 0 },
        h2d: ChildV1 { native_queue_id: 7, engine: 1 } }
}
pub open spec fn sample_binding_v1(direction: DirectionV1) -> BindingV1 {
    BindingV1 { allocation: sample_allocation_v1(), pair: sample_pair_v1(), adapter_incarnation: 13,
        host: HostV1 { session: 14, identity: 15, generation: 16, byte_len: 8192, coherent: true },
        device_range: RangeV1 { offset: 64, byte_len: 256 },
        host_range: RangeV1 { offset: 128, byte_len: 256 },
        direction, access: access_v1(direction), endpoint: endpoint_v1(direction),
        use_slot: 0, use_generation: 17,
        ticket: TicketV1 { owner: sample_pair_v1().parent,
            native_queue_id: child_v1(sample_pair_v1(), direction).native_queue_id,
            slot: 0, generation: 17 } }
}
pub open spec fn sample_idle_state_v1(direction: DirectionV1) -> StateV1 {
    StateV1 { binding: sample_binding_v1(direction), phase: PhaseV1::Idle,
        location: LocationV1::PersistentOwner, live_ticket: None, terminal_status: None,
        frontier: None, current: true, authority_count: 1, occupied_slots: 0,
        next_use_generation: 18, retired_uses: 0,
        next_ticket_slot: 18, next_ticket_generation: 19 }
}

pub proof fn exact_profile_bounds_are_fixed_v1()
    ensures max_allocation_bytes_v1() == 268435456, max_copy_bytes_v1() == 4194272,
        page_bytes_v1() == 4096, queue_slots_v1() == 64,
        native_queue_limit_v1() == 1024, max_u32_v1() == 4294967295,
        min_i32_v1() == -2147483648, max_i32_v1() == 2147483647,
{}
pub proof fn exact_pair_is_inhabited_v1()
    ensures valid_pair_v1(sample_pair_v1(), sample_allocation_v1()), {}
pub proof fn child_queue_ids_must_be_distinct_v1()
    ensures !valid_pair_v1(PairV1 { h2d: ChildV1 { native_queue_id: 0, engine: 1 },
        ..sample_pair_v1() }, sample_allocation_v1()), {}
pub proof fn child_engines_are_exact_v1()
    ensures !valid_pair_v1(PairV1 { d2h: ChildV1 { native_queue_id: 0, engine: 1 },
        ..sample_pair_v1() }, sample_allocation_v1()), {}
pub proof fn logical_physical_and_page_bounds_are_exact_v1()
    ensures valid_allocation_v1(sample_allocation_v1()),
        !valid_allocation_v1(AllocationV1 { logical_bytes: 70000, ..sample_allocation_v1() }),
        !valid_allocation_v1(AllocationV1 { physical_bytes: 65537, ..sample_allocation_v1() }), {}
pub proof fn d2h_role_is_exact_v1()
    ensures valid_binding_v1(sample_binding_v1(DirectionV1::DeviceToHost)), {}
pub proof fn h2d_role_is_exact_v1()
    ensures valid_binding_v1(sample_binding_v1(DirectionV1::HostToDevice)), {}
pub proof fn ticket_binds_selected_child_v1()
    ensures exact_ticket_v1(sample_binding_v1(DirectionV1::DeviceToHost)),
        exact_ticket_v1(sample_binding_v1(DirectionV1::HostToDevice)), {}
pub proof fn range_is_bounded_by_logical_not_padding_v1()
    ensures !valid_binding_v1(BindingV1 { device_range: RangeV1 { offset: 65536, byte_len: 2 },
        host_range: RangeV1 { offset: 0, byte_len: 2 },
        ..sample_binding_v1(DirectionV1::DeviceToHost) }), {}
pub proof fn sample_idle_owner_is_reusable_v1()
    ensures reusable_owner_v1(sample_idle_state_v1(DirectionV1::DeviceToHost)),
        reusable_owner_v1(sample_idle_state_v1(DirectionV1::HostToDevice)), {}

pub proof fn next_binding_is_admitted_v1(s: StateV1, direction: DirectionV1)
    requires reusable_owner_v1(s), s.next_ticket_generation < max_u32_v1(),
    ensures valid_binding_v1(next_binding_v1(s, direction)),
        same_persistent_identity_v1(s.binding, next_binding_v1(s, direction)),
        can_prepare_v1(s, next_binding_v1(s, direction)),
{ match direction { DirectionV1::DeviceToHost => {}, DirectionV1::HostToDevice => {} } }

pub proof fn prepare_consumes_the_single_slot_v1(s: StateV1, direction: DirectionV1)
    requires reusable_owner_v1(s), s.next_ticket_generation < max_u32_v1(),
    ensures {
        let prepared = prepare_from_state_v1(s, next_binding_v1(s, direction));
        &&& valid_state_v1(prepared) &&& prepared.phase == PhaseV1::Prepared
        &&& prepared.occupied_slots == 1 &&& prepared.frontier.is_none()
        &&& prepared.next_use_generation == s.next_use_generation + 1
    },
{ next_binding_is_admitted_v1(s, direction); }

pub proof fn active_or_frontier_state_cannot_prepare_v1(s: StateV1, next: BindingV1)
    requires valid_state_v1(s), s.phase != PhaseV1::Idle,
    ensures !can_prepare_v1(s, next), {}

pub proof fn confirmed_publication_selects_exact_child_v1() {
    let idle = sample_idle_state_v1(DirectionV1::DeviceToHost);
    let b = next_binding_v1(idle, DirectionV1::HostToDevice);
    let next = confirm_v1(prepare_from_state_v1(idle, b), b.ticket);
    assert(valid_state_v1(next)); assert(next.location == LocationV1::NativeChild);
    assert(next.live_ticket == Some(b.ticket));
}
pub proof fn stale_publication_ticket_is_atomic_v1(s: StateV1, stale: TicketV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Prepared, stale != s.binding.ticket,
    ensures confirm_v1(s, stale) == s, {}
pub proof fn recoverable_failure_restores_reusable_owner_v1() {
    let idle = sample_idle_state_v1(DirectionV1::DeviceToHost);
    let prepared = prepare_from_state_v1(idle, next_binding_v1(idle, DirectionV1::DeviceToHost));
    let next = recover_v1(prepared); assert(valid_state_v1(next)); assert(reusable_owner_v1(next));
}
pub proof fn retained_publication_quarantines_with_ticket_v1() {
    let idle = sample_idle_state_v1(DirectionV1::DeviceToHost);
    let prepared = prepare_from_state_v1(idle, next_binding_v1(idle, DirectionV1::DeviceToHost));
    let next = quarantine_retained_v1(prepared); assert(valid_state_v1(next));
    assert(next.live_ticket == Some(prepared.binding.ticket)); assert(!can_release_v1(next));
}
pub proof fn preparation_ambiguity_quarantines_without_ticket_v1() {
    let idle = sample_idle_state_v1(DirectionV1::DeviceToHost);
    let prepared = prepare_from_state_v1(idle, next_binding_v1(idle, DirectionV1::DeviceToHost));
    let next = quarantine_preparation_v1(prepared); assert(valid_state_v1(next));
    assert(next.live_ticket.is_none());
}
pub proof fn pending_is_nonblocking_and_preserves_custody_v1(s: StateV1, ticket: TicketV1)
    ensures pending_v1(s, ticket) == s, {}
pub proof fn timeout_preserves_exact_ticket_v1() {
    let idle = sample_idle_state_v1(DirectionV1::HostToDevice);
    let b = next_binding_v1(idle, DirectionV1::HostToDevice);
    let published = confirm_v1(prepare_from_state_v1(idle, b), b.ticket);
    let timed = timeout_v1(published, b.ticket); assert(valid_state_v1(timed));
    assert(timed.live_ticket == published.live_ticket);
}
pub proof fn exact_completion_binds_succeeded_status_v1() {
    let idle = sample_idle_state_v1(DirectionV1::HostToDevice); let b = next_binding_v1(idle, DirectionV1::HostToDevice);
    let published = confirm_v1(prepare_from_state_v1(idle, b), b.ticket);
    let done = complete_v1(published, b.ticket, TerminalStatusV1::Succeeded);
    assert(valid_state_v1(done)); assert(done.terminal_status == Some(TerminalStatusV1::Succeeded));
}
pub proof fn exact_completion_binds_failed_status_v1() {
    let idle = sample_idle_state_v1(DirectionV1::HostToDevice); let b = next_binding_v1(idle, DirectionV1::HostToDevice);
    let published = confirm_v1(prepare_from_state_v1(idle, b), b.ticket);
    let status = TerminalStatusV1::Failed(-17); let done = complete_v1(published, b.ticket, status);
    assert(valid_state_v1(done)); assert(done.terminal_status == Some(status));
}
pub proof fn stale_completion_ticket_is_atomic_v1(s: StateV1, stale: TicketV1, status: TerminalStatusV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Published || s.phase == PhaseV1::TimedOut,
        stale != s.binding.ticket,
    ensures complete_v1(s, stale, status) == s, {}
pub proof fn completion_ambiguity_enters_quarantine_and_blocks_release_v1() {
    let idle = sample_idle_state_v1(DirectionV1::DeviceToHost); let b = next_binding_v1(idle, DirectionV1::DeviceToHost);
    let published = confirm_v1(prepare_from_state_v1(idle, b), b.ticket);
    let next = completion_ambiguous_v1(published); assert(valid_state_v1(next));
    assert(next.live_ticket == Some(b.ticket)); assert(!can_release_v1(next));
}
pub proof fn restore_status_mismatch_is_atomic_v1(s: StateV1, stale: TerminalStatusV1)
    requires valid_state_v1(s), s.phase == PhaseV1::Completed, s.terminal_status != Some(stale),
    ensures restore_v1(s, stale, true) == s, {}
pub proof fn restore_currentness_ambiguity_retains_ticket_v1() {
    let idle = sample_idle_state_v1(DirectionV1::DeviceToHost); let b = next_binding_v1(idle, DirectionV1::DeviceToHost);
    let published = confirm_v1(prepare_from_state_v1(idle, b), b.ticket);
    let done = complete_v1(published, b.ticket, TerminalStatusV1::Succeeded);
    let next = restore_v1(done, TerminalStatusV1::Succeeded, false);
    assert(valid_state_v1(next)); assert(next.phase == PhaseV1::Quarantined);
    assert(next.live_ticket == Some(b.ticket)); assert(next.terminal_status.is_none());
    assert(!can_release_v1(next));
}
pub proof fn restore_and_settle_create_exact_frontier_v1() {
    let idle = sample_idle_state_v1(DirectionV1::HostToDevice); let b = next_binding_v1(idle, DirectionV1::HostToDevice);
    let published = confirm_v1(prepare_from_state_v1(idle, b), b.ticket);
    let done = complete_v1(published, b.ticket, TerminalStatusV1::Succeeded);
    let restored = restore_v1(done, TerminalStatusV1::Succeeded, true);
    let next = settle_v1(restored, TerminalStatusV1::Succeeded); assert(valid_state_v1(next));
    assert(next.frontier == Some(exact_frontier_v1(b, 1))); assert(!can_release_v1(next));
}
pub proof fn stale_frontier_retirement_is_atomic_v1(s: StateV1, stale: FrontierV1)
    requires valid_state_v1(s), s.phase == PhaseV1::FrontierPending, s.frontier != Some(stale),
    ensures retire_v1(s, stale) == s,
        !can_prepare_v1(s, next_binding_v1(s, DirectionV1::DeviceToHost)), {}
pub proof fn exact_retirement_opens_arbitrary_prepare_gate_v1() {
    let idle = sample_idle_state_v1(DirectionV1::DeviceToHost); let b = next_binding_v1(idle, DirectionV1::HostToDevice);
    let published = confirm_v1(prepare_from_state_v1(idle, b), b.ticket);
    let done = complete_v1(published, b.ticket, TerminalStatusV1::Succeeded);
    let restored = restore_v1(done, TerminalStatusV1::Succeeded, true);
    let pending = settle_v1(restored, TerminalStatusV1::Succeeded);
    assert(!can_prepare_v1(pending, next_binding_v1(pending, DirectionV1::HostToDevice)));
    let retired = retire_v1(pending, exact_frontier_v1(b, 1)); assert(valid_state_v1(retired));
    next_binding_is_admitted_v1(retired, DirectionV1::DeviceToHost);
    next_binding_is_admitted_v1(retired, DirectionV1::HostToDevice);
}
pub proof fn active_and_frontier_states_block_release_v1(s: StateV1)
    requires valid_state_v1(s), s.phase != PhaseV1::Idle,
    ensures !can_release_v1(s), {}

pub proof fn frontier_rejects_cross_allocation_mapping_v1() {
    let b = sample_binding_v1(DirectionV1::DeviceToHost);
    let a = AllocationV1 { mapping_id: b.allocation.mapping_id + 1, ..b.allocation };
    let foreign = BindingV1 { allocation: a, ..b };
    assert(exact_frontier_v1(b, 1) != exact_frontier_v1(foreign, 1));
}
pub proof fn frontier_rejects_cross_parent_pair_v1() {
    let b = sample_binding_v1(DirectionV1::DeviceToHost);
    let parent = QueueV1 { id: b.pair.parent.id + 1, ..b.pair.parent };
    let pair = PairV1 { parent, ..b.pair }; let foreign = BindingV1 { pair, ..b };
    assert(exact_frontier_v1(b, 1) != exact_frontier_v1(foreign, 1));
}
pub proof fn frontier_rejects_cross_incarnation_v1() {
    let b = sample_binding_v1(DirectionV1::DeviceToHost);
    let foreign = BindingV1 { adapter_incarnation: b.adapter_incarnation + 1, ..b };
    assert(exact_frontier_v1(b, 1) != exact_frontier_v1(foreign, 1));
}
pub proof fn rebind_requires_idle_and_advances_attachment_v1(s: StateV1, pair: PairV1)
    requires reusable_owner_v1(s), valid_pair_v1(pair, s.binding.allocation),
        pair.attachment_generation == s.binding.pair.attachment_generation + 1,
    ensures {
        let next = rebind_v1(s, pair);
        &&& valid_state_v1(next) &&& reusable_owner_v1(next)
        &&& next.binding.pair == pair
        &&& next.binding.pair.attachment_generation
            == s.binding.pair.attachment_generation + 1
    },
{}
pub proof fn active_or_frontier_state_blocks_rebind_v1(s: StateV1, pair: PairV1)
    requires valid_state_v1(s), s.phase != PhaseV1::Idle,
    ensures !can_rebind_v1(s, pair), rebind_v1(s, pair) == s, {}
pub proof fn demotion_repromotion_requires_idle_and_advances_identity_v1(s: StateV1)
    requires reusable_owner_v1(s),
    ensures {
        let next = demote_repromote_v1(s);
        &&& valid_state_v1(next) &&& reusable_owner_v1(next)
        &&& next.binding.allocation.pool_generation
            == s.binding.allocation.pool_generation + 1
        &&& next.binding.adapter_incarnation == s.binding.adapter_incarnation + 1
    },
{}
pub proof fn active_or_frontier_state_blocks_demotion_v1(s: StateV1)
    requires valid_state_v1(s), s.phase != PhaseV1::Idle,
    ensures !can_demote_v1(s), demote_repromote_v1(s) == s, {}
pub open spec fn same_binding_except_pool_generation_v1(prior: BindingV1, next: BindingV1) -> bool {
    next == BindingV1 { allocation: AllocationV1 {
        pool_generation: next.allocation.pool_generation, ..prior.allocation }, ..prior }
}
pub proof fn old_pool_frontier_is_rejected_after_repromotion_v1() {
    let prior = sample_binding_v1(DirectionV1::HostToDevice);
    let allocation = AllocationV1 { pool_generation: prior.allocation.pool_generation + 1, ..prior.allocation };
    let next = BindingV1 { allocation, ..prior };
    assert(next.direction == prior.direction); assert(next.pair == prior.pair);
    assert(next.adapter_incarnation == prior.adapter_incarnation);
    assert(same_binding_except_pool_generation_v1(prior, next));
    assert(prior.allocation.pool_generation != next.allocation.pool_generation);
    assert(exact_frontier_v1(prior, 1) != exact_frontier_v1(next, 1));
}

pub proof fn one_successful_use_retires_before_reuse_v1(s: StateV1, direction: DirectionV1)
    requires reusable_owner_v1(s), s.next_ticket_generation < max_u32_v1(),
    ensures {
        let retired = execute_and_retire_one_v1(s, direction);
        &&& reusable_owner_v1(retired) &&& retired.occupied_slots == 0
        &&& retired.frontier.is_none()
        &&& retired.next_use_generation == s.next_use_generation + 1
        &&& retired.retired_uses == s.retired_uses + 1
        &&& retired.next_ticket_generation == s.next_ticket_generation + 1
        &&& same_persistent_identity_v1(s.binding, retired.binding)
    },
{
    next_binding_is_admitted_v1(s, direction); let b = next_binding_v1(s, direction);
    let prepared = prepare_from_state_v1(s, b); assert(valid_state_v1(prepared));
    let published = confirm_v1(prepared, b.ticket); assert(valid_state_v1(published));
    let done = complete_v1(published, b.ticket, TerminalStatusV1::Succeeded); assert(valid_state_v1(done));
    let restored = restore_v1(done, TerminalStatusV1::Succeeded, true); assert(valid_state_v1(restored));
    let pending = settle_v1(restored, TerminalStatusV1::Succeeded); assert(valid_state_v1(pending));
    let retired = retire_v1(pending, exact_frontier_v1(b, s.retired_uses + 1));
    assert(valid_state_v1(retired));
}
pub proof fn retired_steps_preserve_reusable_owner_v1(s: StateV1, steps: nat, alternating: bool)
    requires reusable_owner_v1(s), s.next_ticket_generation + steps <= max_u32_v1(),
    ensures {
        let retired = run_retired_steps_v1(s, steps, alternating);
        &&& reusable_owner_v1(retired) &&& retired.occupied_slots == 0
        &&& retired.frontier.is_none()
        &&& retired.next_use_generation == s.next_use_generation + steps
        &&& retired.retired_uses == s.retired_uses + steps
        &&& retired.next_ticket_generation == s.next_ticket_generation + steps
        &&& same_persistent_identity_v1(s.binding, retired.binding)
    },
    decreases steps,
{
    if steps > 0 {
        let direction = direction_for_step_v1(steps, alternating);
        one_successful_use_retires_before_reuse_v1(s, direction);
        let next = execute_and_retire_one_v1(s, direction);
        assert((steps - 1) as nat + 1 == steps);
        assert(next.next_use_generation + (steps - 1) as nat == s.next_use_generation + steps);
        assert(next.next_ticket_generation + (steps - 1) as nat == s.next_ticket_generation + steps);
        retired_steps_preserve_reusable_owner_v1(next, (steps - 1) as nat, alternating);
        assert(same_persistent_identity_v1(s.binding,
            run_retired_steps_v1(next, (steps - 1) as nat, alternating).binding));
    }
}
pub proof fn repeated_h2d_after_h2d_is_derived_v1() {
    let initial = sample_idle_state_v1(DirectionV1::HostToDevice);
    one_successful_use_retires_before_reuse_v1(initial, DirectionV1::HostToDevice);
    let first = execute_and_retire_one_v1(initial, DirectionV1::HostToDevice);
    next_binding_is_admitted_v1(first, DirectionV1::HostToDevice);
}
pub proof fn repeated_d2h_after_d2h_is_derived_v1() {
    let initial = sample_idle_state_v1(DirectionV1::DeviceToHost);
    one_successful_use_retires_before_reuse_v1(initial, DirectionV1::DeviceToHost);
    let first = execute_and_retire_one_v1(initial, DirectionV1::DeviceToHost);
    next_binding_is_admitted_v1(first, DirectionV1::DeviceToHost);
}
pub proof fn alternating_direction_after_retirement_is_derived_v1() {
    let initial = sample_idle_state_v1(DirectionV1::DeviceToHost);
    one_successful_use_retires_before_reuse_v1(initial, DirectionV1::DeviceToHost);
    let first = execute_and_retire_one_v1(initial, DirectionV1::DeviceToHost);
    next_binding_is_admitted_v1(first, DirectionV1::HostToDevice);
}
pub proof fn one_hundred_thirty_mixed_retired_uses_reuse_one_slot_v1() {
    let initial = sample_idle_state_v1(DirectionV1::DeviceToHost);
    retired_steps_preserve_reusable_owner_v1(initial, 130, true);
    let retired = run_retired_steps_v1(initial, 130, true);
    assert(retired.occupied_slots == 0); assert(retired.frontier.is_none());
    assert(retired.retired_uses == 130); assert(retired.next_use_generation == 148);
}
pub proof fn seventy_repeated_h2d_retired_uses_reuse_one_slot_v1() {
    let initial = sample_idle_state_v1(DirectionV1::HostToDevice);
    retired_steps_preserve_reusable_owner_v1(initial, 70, false);
    let retired = run_retired_steps_v1(initial, 70, false);
    assert(retired.occupied_slots == 0); assert(retired.frontier.is_none());
    assert(retired.retired_uses == 70); assert(retired.next_use_generation == 88);
}

}
