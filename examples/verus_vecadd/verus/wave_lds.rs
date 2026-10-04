include!("vecadd.rs");

verus! {

/// The active mask has one entry for every lane in the launch-branded wave.
/// No theorem fixes the wave extent to 32 or 64 lanes.
pub open spec fn active_wave_is_well_formed(
    wave: IndexSpace1d,
    values: Seq<int>,
    active: Seq<bool>,
) -> bool {
    wave.extent > 0
        && values.len() == wave.extent
        && active.len() == wave.extent
}

/// Mathematical inclusive-scan prefix over active lanes. Inactive lanes add
/// zero, while still occupying a lane in the wave's branded index space.
pub open spec fn active_prefix_sum(
    values: Seq<int>,
    active: Seq<bool>,
    end: nat,
) -> int
    decreases end,
{
    if end == 0 {
        0
    } else if end <= values.len() && end <= active.len() {
        active_prefix_sum(values, active, (end - 1) as nat)
            + if active[(end - 1) as int] { values[(end - 1) as int] } else { 0 }
    } else {
        0
    }
}

pub open spec fn active_scan_value(
    values: Seq<int>,
    active: Seq<bool>,
    lane: nat,
) -> int {
    active_prefix_sum(values, active, lane + 1)
}

pub open spec fn active_wave_reduction(
    wave: IndexSpace1d,
    values: Seq<int>,
    active: Seq<bool>,
) -> int {
    active_prefix_sum(values, active, wave.extent)
}

pub proof fn active_scan_step_is_exact(
    wave: IndexSpace1d,
    values: Seq<int>,
    active: Seq<bool>,
    lane: BrandedThreadIndex1d,
)
    requires
        active_wave_is_well_formed(wave, values, active),
        thread_belongs_to_space(wave, lane),
    ensures
        active_scan_value(values, active, lane.linear)
            == active_prefix_sum(values, active, lane.linear)
                + if active[lane.linear as int] {
                    values[lane.linear as int]
                } else {
                    0
                },
{
    assert(lane.linear + 1 <= wave.extent);
    assert(lane.linear + 1 <= values.len());
    assert(lane.linear + 1 <= active.len());
}

pub proof fn inactive_lane_does_not_contribute(
    wave: IndexSpace1d,
    values: Seq<int>,
    active: Seq<bool>,
    lane: BrandedThreadIndex1d,
)
    requires
        active_wave_is_well_formed(wave, values, active),
        thread_belongs_to_space(wave, lane),
        !active[lane.linear as int],
    ensures
        active_scan_value(values, active, lane.linear)
            == active_prefix_sum(values, active, lane.linear),
{
    active_scan_step_is_exact(wave, values, active, lane);
}

/// The reduction depends only on values belonging to active lanes. This is
/// recursive over the runtime wave extent rather than a wave32-specific tree.
pub proof fn active_values_determine_prefix(
    left: Seq<int>,
    right: Seq<int>,
    active: Seq<bool>,
    end: nat,
)
    requires
        left.len() == active.len(),
        right.len() == active.len(),
        end <= active.len(),
        forall |lane: nat| lane < end && active[lane as int]
            ==> left[lane as int] == right[lane as int],
    ensures
        active_prefix_sum(left, active, end) == active_prefix_sum(right, active, end),
    decreases end,
{
    if end > 0 {
        active_values_determine_prefix(left, right, active, (end - 1) as nat);
        assert(end - 1 < end);
        if active[(end - 1) as int] {
            assert(left[(end - 1) as int] == right[(end - 1) as int]);
        }
    }
}

pub proof fn active_values_determine_reduction(
    wave: IndexSpace1d,
    left: Seq<int>,
    right: Seq<int>,
    active: Seq<bool>,
)
    requires
        active_wave_is_well_formed(wave, left, active),
        active_wave_is_well_formed(wave, right, active),
        forall |lane: nat| lane < wave.extent && active[lane as int]
            ==> left[lane as int] == right[lane as int],
    ensures
        active_wave_reduction(wave, left, active)
            == active_wave_reduction(wave, right, active),
{
    active_values_determine_prefix(left, right, active, wave.extent);
}

/// Distinct active lane witnesses receive disjoint scan-output regions through
/// the same branded identity-index permission model used by vecadd.
pub proof fn distinct_active_lanes_have_disjoint_scan_outputs(
    wave: IndexSpace1d,
    active: Seq<bool>,
    left: BrandedThreadIndex1d,
    right: BrandedThreadIndex1d,
    output_allocation: Allocation,
    element_size: nat,
)
    requires
        active.len() == wave.extent,
        thread_belongs_to_space(wave, left),
        thread_belongs_to_space(wave, right),
        active[left.linear as int],
        active[right.linear as int],
        left.linear != right.linear,
        element_size > 0,
    ensures
        !regions_overlap(
            element_region(output_allocation, left.linear, element_size),
            element_region(output_allocation, right.linear, element_size),
        ),
        permissions_are_compatible(
            exclusive_write(element_region(output_allocation, left.linear, element_size)),
            exclusive_write(element_region(output_allocation, right.linear, element_size)),
        ),
{
    distinct_branded_threads_have_disjoint_output_regions(
        wave,
        left,
        right,
        output_allocation,
        element_size,
    );
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum WorkgroupPhase {
    Initializing,
    ReadableAfterBarrier,
}

/// Ghost state for one LDS array. `initialized` records ownership-completing
/// writes; it contains no executable storage and grants no runtime authority.
pub struct WorkgroupLdsState {
    pub space: IndexSpace1d,
    pub allocation: Allocation,
    pub element_size: nat,
    pub initialized: Seq<bool>,
    pub phase: WorkgroupPhase,
}

pub open spec fn lds_state_is_well_formed(state: WorkgroupLdsState) -> bool {
    state.space.extent > 0
        && state.element_size > 0
        && state.initialized.len() == state.space.extent
        && allocation_is_representable(state.allocation)
        && state.allocation.byte_length == state.space.extent * state.element_size
}

pub open spec fn every_lds_slot_is_initialized(state: WorkgroupLdsState) -> bool {
    forall |lane: nat| lane < state.space.extent ==> state.initialized[lane as int]
}

pub open spec fn every_lane_arrived(space: IndexSpace1d, arrived: Seq<bool>) -> bool {
    arrived.len() == space.extent
        && forall |lane: nat| lane < space.extent ==> arrived[lane as int]
}

pub open spec fn lds_write_permission(
    state: WorkgroupLdsState,
    thread: BrandedThreadIndex1d,
) -> RegionPermission {
    exclusive_write(element_region(
        state.allocation,
        thread.linear,
        state.element_size,
    ))
}

pub open spec fn initialized_lds_read_capability(
    state: WorkgroupLdsState,
    source: BrandedThreadIndex1d,
) -> RegionCapability {
    initialized_read_capability(element_region(
        state.allocation,
        source.linear,
        state.element_size,
    ))
}

pub open spec fn record_owned_lds_write(
    state: WorkgroupLdsState,
    thread: BrandedThreadIndex1d,
) -> Seq<bool> {
    state.initialized.update(thread.linear as int, true)
}

/// A convergent barrier releases only after every branded lane has arrived and
/// every lane-owned LDS slot has been initialized. The transfer preserves the
/// allocation and changes exclusive pre-barrier ownership into legal shared
/// post-barrier reads.
pub open spec fn convergent_barrier_transfer(
    pre: WorkgroupLdsState,
    post: WorkgroupLdsState,
    arrived: Seq<bool>,
) -> bool {
    lds_state_is_well_formed(pre)
        && pre.phase == WorkgroupPhase::Initializing
        && every_lds_slot_is_initialized(pre)
        && every_lane_arrived(pre.space, arrived)
        && post.space == pre.space
        && post.allocation == pre.allocation
        && post.element_size == pre.element_size
        && post.initialized == pre.initialized
        && post.phase == WorkgroupPhase::ReadableAfterBarrier
}

pub open spec fn lds_shared_read_is_legal(
    state: WorkgroupLdsState,
    source: BrandedThreadIndex1d,
) -> bool {
    state.phase == WorkgroupPhase::ReadableAfterBarrier
        && thread_belongs_to_space(state.space, source)
        && state.initialized[source.linear as int]
        && region_is_in_bounds(
            state.allocation,
            initialized_lds_read_capability(state, source).permission.region,
        )
        && capability_can_read(initialized_lds_read_capability(state, source))
}

/// Before the barrier, each thread can initialize only its branded identity
/// slot. The resulting update preserves every other initialization bit.
pub proof fn owned_lds_write_is_in_bounds_and_framed(
    state: WorkgroupLdsState,
    thread: BrandedThreadIndex1d,
)
    requires
        lds_state_is_well_formed(state),
        state.phase == WorkgroupPhase::Initializing,
        thread_belongs_to_space(state.space, thread),
        !state.initialized[thread.linear as int],
    ensures
        region_is_in_bounds(state.allocation, lds_write_permission(state, thread).region),
        permission_can_write(lds_write_permission(state, thread)),
        record_owned_lds_write(state, thread).len() == state.initialized.len(),
        record_owned_lds_write(state, thread)[thread.linear as int],
        forall |other: nat| other < state.space.extent && other != thread.linear
            ==> record_owned_lds_write(state, thread)[other as int]
                == state.initialized[other as int],
{
    element_region_is_in_bounds_and_address_representable(
        state.allocation,
        state.space.extent,
        thread.linear,
        state.element_size,
    );
}

/// Identity ownership makes all distinct pre-barrier LDS writes race-free.
pub proof fn distinct_threads_have_disjoint_lds_writes(
    state: WorkgroupLdsState,
    left: BrandedThreadIndex1d,
    right: BrandedThreadIndex1d,
)
    requires
        lds_state_is_well_formed(state),
        state.phase == WorkgroupPhase::Initializing,
        thread_belongs_to_space(state.space, left),
        thread_belongs_to_space(state.space, right),
        left.linear != right.linear,
    ensures
        !regions_overlap(
            lds_write_permission(state, left).region,
            lds_write_permission(state, right).region,
        ),
        permissions_are_compatible(
            lds_write_permission(state, left),
            lds_write_permission(state, right),
        ),
{
    distinct_branded_threads_have_disjoint_output_regions(
        state.space,
        left,
        right,
        state.allocation,
        state.element_size,
    );
}

/// Any participating thread may read any initialized LDS slot after the
/// convergent barrier. Shared reads remain compatible even at one source slot.
pub proof fn convergent_barrier_enables_shared_lds_read(
    pre: WorkgroupLdsState,
    post: WorkgroupLdsState,
    arrived: Seq<bool>,
    reader: BrandedThreadIndex1d,
    source: BrandedThreadIndex1d,
)
    requires
        convergent_barrier_transfer(pre, post, arrived),
        thread_belongs_to_space(pre.space, reader),
        thread_belongs_to_space(pre.space, source),
    ensures
        lds_shared_read_is_legal(post, source),
        region_is_in_bounds(
            post.allocation,
            initialized_lds_read_capability(post, source).permission.region,
        ),
        capability_can_read(initialized_lds_read_capability(post, source)),
        permissions_are_compatible(
            initialized_lds_read_capability(post, source).permission,
            initialized_lds_read_capability(post, source).permission,
        ),
{
    assert(post.space == pre.space);
    assert(post.initialized == pre.initialized);
    assert(pre.initialized[source.linear as int]);
    element_region_is_in_bounds_and_address_representable(
        post.allocation,
        post.space.extent,
        source.linear,
        post.element_size,
    );
    shared_input_reads_may_alias(
        initialized_lds_read_capability(post, source).permission.region,
    );
}

} // verus!
