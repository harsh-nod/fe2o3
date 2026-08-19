use vstd::prelude::*;

#[path = "../general_gemm_schedule_model_v1.rs"]
mod model;

verus! {

// The symbolic schedule contains no artifact or ISA model. Claiming that it
// alone completes machine refinement must remain unprovable.
pub proof fn mutated_symbolic_proof_claims_machine_refinement_v1()
    ensures model::machine_refinement_complete_v1(),
{
}

fn main() {}

}
