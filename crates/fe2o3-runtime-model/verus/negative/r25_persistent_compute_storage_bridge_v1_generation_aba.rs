use vstd::prelude::*;
verus! {
pub open spec fn retired_generation_v1() -> nat { 12 }
pub open spec fn mutated_next_generation_v1() -> nat { 12 }
pub proof fn mutated_next_generation_rejects_aba_v1()
    ensures mutated_next_generation_v1() > retired_generation_v1(), {}
}
