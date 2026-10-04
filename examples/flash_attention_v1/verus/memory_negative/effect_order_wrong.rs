use vstd::prelude::*;
verus! {
pub open spec fn read_phase_v1() -> nat { 1 }
pub open spec fn write_phase_v1() -> nat { 2 }
pub proof fn mutated_write_precedes_read_v1()
    ensures write_phase_v1() < read_phase_v1(),
{
}
}
