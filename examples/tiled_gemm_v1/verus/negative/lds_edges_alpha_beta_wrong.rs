use vstd::prelude::*;

#[path = "../lds_tiled_edges_alpha_beta.rs"]
mod model;

verus! {

/// Mutation: beta is added directly instead of scaling Cinput.
pub open spec fn mutated_alpha_beta_v1(
    product: real,
    c_input: real,
    alpha: real,
    beta: real,
) -> real {
    alpha * product + beta
}

pub proof fn mutated_wrong_alpha_beta_matches_exact_contract_v1()
    ensures
        mutated_alpha_beta_v1(2real, 3real, 5real, 7real)
            == model::edges_exact_alpha_beta_v1(2real, 3real, 5real, 7real),
{
}

} // verus!
