use vstd::prelude::*;

verus! {

pub struct RingStateV1 {
    pub capacity: nat,
    pub write: nat,
    pub last_read: nat,
}

pub open spec fn before_v1() -> RingStateV1 {
    RingStateV1 {
        capacity: 4,
        write: 6,
        last_read: 5,
    }
}

pub open spec fn mutated_accept_regressed_read_v1(
    before: RingStateV1,
    observed_read: nat,
) -> RingStateV1 {
    RingStateV1 {
        capacity: before.capacity,
        write: before.write + 1,
        last_read: observed_read,
    }
}

pub proof fn mutated_read_regression_is_nondecreasing_v1()
    ensures
        before_v1().last_read
            <= mutated_accept_regressed_read_v1(before_v1(), 4).last_read,
{
}

} // verus!
