use vstd::prelude::*;
verus! {
pub open spec fn permutation_phase_v1() -> nat { 8 }
pub open spec fn output_commit_phase_v1() -> nat { 9 }
pub proof fn mutated_output_commit_precedes_permutation_v1()
    ensures output_commit_phase_v1() < permutation_phase_v1(),
{
}
}
