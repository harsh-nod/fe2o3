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
        write: 7,
        last_read: 6,
    }
}

pub open spec fn mutated_replay_reservation_v1(
    before: RingStateV1,
    observed_read: nat,
) -> RingStateV1 {
    RingStateV1 {
        capacity: before.capacity,
        write: before.write,
        last_read: observed_read,
    }
}

pub proof fn mutated_replay_advances_write_once_v1()
    ensures
        mutated_replay_reservation_v1(before_v1(), 6).write
            == before_v1().write + 1,
{
}

} // verus!
