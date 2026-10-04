include!("../gfx942_wave_lds_v1.rs");

verus! {

/// Expected failure marker: mutated_gfx942_wave63_is_wave64.
pub proof fn mutated_gfx942_wave63_is_wave64()
    ensures
        gfx942_wave_extent_is_valid(63),
{
}

} // verus!
