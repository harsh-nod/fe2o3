use vstd::prelude::*;

verus! {

pub struct RingStateV1 {
    pub capacity: nat,
    pub write: nat,
    pub observed_read: nat,
}

#[derive(PartialEq, Eq)]
pub enum OutcomeV1 {
    Accepted { packet_id: nat },
    RejectedFull,
}

pub open spec fn full_state_v1() -> RingStateV1 {
    RingStateV1 {
        capacity: 4,
        write: 4,
        observed_read: 0,
    }
}

pub open spec fn mutated_full_reservation_v1(state: RingStateV1) -> OutcomeV1 {
    OutcomeV1::Accepted {
        packet_id: state.write,
    }
}

pub proof fn mutated_full_overwrite_is_rejected_v1()
    ensures
        full_state_v1().write - full_state_v1().observed_read
            == full_state_v1().capacity,
        mutated_full_reservation_v1(full_state_v1()) == OutcomeV1::RejectedFull,
{
}

} // verus!
