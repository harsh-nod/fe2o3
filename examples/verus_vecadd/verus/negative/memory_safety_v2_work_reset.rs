use vstd::prelude::*;

verus! {

pub proof fn mutated_separate_phase_budgets_imply_cumulative_admission(
    decode_work: nat,
    later_work: nat,
    max_work: nat,
)
    requires
        decode_work <= max_work,
        later_work <= max_work,
    ensures
        decode_work + later_work <= max_work,
{
}

} // verus!
