include!("../gfx942_wave_lds_v1.rs");

verus! {

/// Expected failure marker: mutated_missing_barrier_lane_is_complete.
pub proof fn mutated_missing_barrier_lane_is_complete(arrived: Seq<bool>, lane: nat)
    requires
        arrived.len() == 256,
        lane < 256,
        !arrived[lane as int],
    ensures
        gfx942_barrier_round_is_complete(arrived),
{
}

} // verus!
