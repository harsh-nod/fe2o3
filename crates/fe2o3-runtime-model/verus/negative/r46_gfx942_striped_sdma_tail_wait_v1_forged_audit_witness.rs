// Expected-negative R46 mutation: retirement witness minting ignores audit truth.
use vstd::prelude::*;
verus! {
pub struct WitnessV1 { pub observation_count: nat }
pub open spec fn mutated_mint_witness_v1(
    request_count: nat,
    exact_roster: bool,
    all_ready: bool,
    preflight: bool,
) -> Option<WitnessV1> {
    Some(WitnessV1 { observation_count: request_count })
}
pub proof fn mutated_forged_audit_witness_is_rejected_v1(request_count: nat)
    requires request_count > 0,
    ensures mutated_mint_witness_v1(request_count, false, false, false).is_none(),
{}
}
