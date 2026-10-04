use vstd::prelude::*;

verus! {

pub struct MutatedMappingV1 {
    pub allocation_generation: nat,
    pub mapped_devices: nat,
    pub live_publications: nat,
}

pub open spec fn retains_allocation_v1(mapping: MutatedMappingV1) -> bool {
    mapping.mapped_devices > 0 || mapping.live_publications > 0
}

pub open spec fn mutated_free_ignores_partial_mapping_v1(mapping: MutatedMappingV1) -> bool {
    mapping.live_publications == 0
}

pub proof fn mutated_free_while_partial_is_safe_v1(mapping: MutatedMappingV1)
    requires
        mapping.allocation_generation > 0,
        mapping.mapped_devices > 0,
        mapping.live_publications == 0,
    ensures
        mutated_free_ignores_partial_mapping_v1(mapping)
            ==> !retains_allocation_v1(mapping),
{
}

} // verus!
