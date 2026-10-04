include!("wave_lds.rs");

verus! {

pub open spec fn gfx942_wave_extent_is_valid(extent: nat) -> bool {
    extent == 64
}

pub open spec fn gfx942_workgroup_extent_is_valid(extent: nat) -> bool {
    extent == 256
}

pub open spec fn gfx942_u32_modulus() -> nat {
    4_294_967_296
}

pub open spec fn gfx942_lane_is_logically_active(
    active_flags: Seq<nat>,
    lane: nat,
) -> bool {
    lane < 64 && lane < active_flags.len() && active_flags[lane as int] != 0
}

pub open spec fn gfx942_masked_u32_contribution(value: nat, active_flag: nat) -> nat {
    if active_flag == 0 { 0 } else { value }
}

pub open spec fn gfx942_masked_u32_prefix(
    values: Seq<nat>,
    active_flags: Seq<nat>,
    end: nat,
) -> nat
    decreases end,
{
    if end == 0 {
        0
    } else if end <= values.len() && end <= active_flags.len() {
        (gfx942_masked_u32_prefix(values, active_flags, (end - 1) as nat)
            + gfx942_masked_u32_contribution(
                values[(end - 1) as int],
                active_flags[(end - 1) as int],
            )) % gfx942_u32_modulus()
    } else {
        0
    }
}

pub open spec fn gfx942_masked_wave_reduction(
    values: Seq<nat>,
    active_flags: Seq<nat>,
) -> nat {
    gfx942_masked_u32_prefix(values, active_flags, 64)
}

/// Logical inactivity changes a lane's contribution, not its obligation to
/// execute the convergent wave operation with all 64 physical lanes.
pub open spec fn gfx942_wave64_contract(
    values: Seq<nat>,
    active_flags: Seq<nat>,
    physically_participating: Seq<bool>,
) -> bool {
    values.len() == 64
        && active_flags.len() == 64
        && physically_participating.len() == 64
        && forall |lane: nat| lane < 64 ==> values[lane as int] < gfx942_u32_modulus()
        && forall |lane: nat| lane < 64 ==>
            active_flags[lane as int] < gfx942_u32_modulus()
        && forall |lane: nat| lane < 64 ==> physically_participating[lane as int]
}

pub proof fn gfx942_zero_flag_contributes_zero_but_lane_participates(
    values: Seq<nat>,
    active_flags: Seq<nat>,
    physically_participating: Seq<bool>,
    lane: nat,
)
    requires
        gfx942_wave64_contract(values, active_flags, physically_participating),
        lane < 64,
        active_flags[lane as int] == 0,
    ensures
        !gfx942_lane_is_logically_active(active_flags, lane),
        gfx942_masked_u32_contribution(
            values[lane as int],
            active_flags[lane as int],
        ) == 0,
        physically_participating[lane as int],
{
}

pub proof fn gfx942_every_nonzero_flag_contributes_its_value(
    values: Seq<nat>,
    active_flags: Seq<nat>,
    physically_participating: Seq<bool>,
    lane: nat,
)
    requires
        gfx942_wave64_contract(values, active_flags, physically_participating),
        lane < 64,
        active_flags[lane as int] != 0,
    ensures
        gfx942_lane_is_logically_active(active_flags, lane),
        gfx942_masked_u32_contribution(
            values[lane as int],
            active_flags[lane as int],
        ) == values[lane as int],
        physically_participating[lane as int],
{
}

pub proof fn gfx942_inactive_values_do_not_change_prefix(
    left: Seq<nat>,
    right: Seq<nat>,
    active_flags: Seq<nat>,
    end: nat,
)
    requires
        left.len() == active_flags.len(),
        right.len() == active_flags.len(),
        end <= active_flags.len(),
        forall |lane: nat| lane < end && active_flags[lane as int] != 0
            ==> left[lane as int] == right[lane as int],
    ensures
        gfx942_masked_u32_prefix(left, active_flags, end)
            == gfx942_masked_u32_prefix(right, active_flags, end),
    decreases end,
{
    if end > 0 {
        gfx942_inactive_values_do_not_change_prefix(
            left,
            right,
            active_flags,
            (end - 1) as nat,
        );
        if active_flags[(end - 1) as int] != 0 {
            assert(left[(end - 1) as int] == right[(end - 1) as int]);
        }
    }
}

pub proof fn gfx942_inactive_values_do_not_change_wave_reduction(
    left: Seq<nat>,
    right: Seq<nat>,
    active_flags: Seq<nat>,
    physically_participating: Seq<bool>,
)
    requires
        gfx942_wave64_contract(left, active_flags, physically_participating),
        gfx942_wave64_contract(right, active_flags, physically_participating),
        forall |lane: nat| lane < 64 && active_flags[lane as int] != 0
            ==> left[lane as int] == right[lane as int],
    ensures
        gfx942_masked_wave_reduction(left, active_flags)
            == gfx942_masked_wave_reduction(right, active_flags),
{
    gfx942_inactive_values_do_not_change_prefix(left, right, active_flags, 64);
}

pub open spec fn gfx942_static_lds_u32x256_is_exact(state: WorkgroupLdsState) -> bool {
    lds_state_is_well_formed(state)
        && gfx942_workgroup_extent_is_valid(state.space.extent)
        && state.allocation.address_space == 3
        && state.allocation.base_address % 4 == 0
        && state.allocation.byte_length == 1_024
        && state.element_size == 4
}

pub open spec fn gfx942_reduction_offset(offset: nat) -> bool {
    offset == 128 || offset == 64 || offset == 32 || offset == 16
        || offset == 8 || offset == 4 || offset == 2 || offset == 1
}

pub proof fn gfx942_reduction_partner_is_in_lds(lane: nat, offset: nat)
    requires
        lane < offset,
        gfx942_reduction_offset(offset),
    ensures
        lane + offset < 256,
{
    if offset == 128 {
    } else if offset == 64 {
    } else if offset == 32 {
    } else if offset == 16 {
    } else if offset == 8 {
    } else if offset == 4 {
    } else if offset == 2 {
    } else {
        assert(offset == 1);
    }
}

pub proof fn gfx942_distinct_threads_own_disjoint_lds_slots(
    state: WorkgroupLdsState,
    left: BrandedThreadIndex1d,
    right: BrandedThreadIndex1d,
)
    requires
        gfx942_static_lds_u32x256_is_exact(state),
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
    distinct_threads_have_disjoint_lds_writes(state, left, right);
}

pub open spec fn gfx942_barrier_round_is_complete(arrived: Seq<bool>) -> bool {
    arrived.len() == 256
        && forall |lane: nat| lane < 256 ==> arrived[lane as int]
}

/// The concrete tree has one initialization barrier, two barriers for each of
/// eight reduction offsets, and one final barrier: 18 uniform rounds.
pub open spec fn gfx942_barrier_trace_is_uniform(trace: Seq<Seq<bool>>) -> bool {
    trace.len() == 18
        && forall |round: nat| round < 18 ==>
            gfx942_barrier_round_is_complete(trace[round as int])
}

pub proof fn gfx942_every_recorded_barrier_has_full_participation(
    trace: Seq<Seq<bool>>,
    round: nat,
    lane: nat,
)
    requires
        gfx942_barrier_trace_is_uniform(trace),
        round < 18,
        lane < 256,
    ensures
        trace[round as int].len() == 256,
        trace[round as int][lane as int],
{
}

pub proof fn gfx942_barrier_enables_legal_stage_partner_read(
    pre: WorkgroupLdsState,
    post: WorkgroupLdsState,
    arrived: Seq<bool>,
    reader: BrandedThreadIndex1d,
    offset: nat,
)
    requires
        gfx942_static_lds_u32x256_is_exact(pre),
        convergent_barrier_transfer(pre, post, arrived),
        thread_belongs_to_space(pre.space, reader),
        reader.linear < offset,
        gfx942_reduction_offset(offset),
    ensures
        lds_shared_read_is_legal(
            post,
            BrandedThreadIndex1d {
                brand: reader.brand,
                linear: reader.linear + offset,
            },
        ),
{
    gfx942_reduction_partner_is_in_lds(reader.linear, offset);
    let source = BrandedThreadIndex1d {
        brand: reader.brand,
        linear: reader.linear + offset,
    };
    assert(pre.space.extent == 256);
    assert(source.brand == pre.space.brand);
    assert(thread_belongs_to_space(pre.space, source));
    convergent_barrier_enables_shared_lds_read(pre, post, arrived, reader, source);
}

// Deliberate refinement boundary: these theorems specify the authenticated
// Rust primitive. Translation to six ds_bpermute operations, addrspace(3)
// storage, and machine barriers is checked by compiler/LLVM/assembly tests; it
// is not represented here as a proved compiler-correctness theorem.

} // verus!
