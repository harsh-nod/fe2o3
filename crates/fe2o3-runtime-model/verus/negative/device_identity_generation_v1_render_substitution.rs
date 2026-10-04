use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct PciAddressV1 {
    pub domain: nat,
    pub bus: nat,
    pub device: nat,
    pub function: nat,
}

pub open spec fn topology_matches_render_v1(
    topology: PciAddressV1,
    render: PciAddressV1,
) -> bool {
    topology == render
}

pub proof fn mutated_render_substitution_correlates_v1()
    ensures
        topology_matches_render_v1(
            PciAddressV1 { domain: 0, bus: 4, device: 1, function: 0 },
            PciAddressV1 { domain: 0, bus: 5, device: 1, function: 0 },
        ),
{
}

} // verus!
