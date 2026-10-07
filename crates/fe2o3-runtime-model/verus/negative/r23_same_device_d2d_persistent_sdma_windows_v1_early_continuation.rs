use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)]
pub enum D2dPhaseV1 { Published, FullyRetired }

pub struct D2dOutcomeV1 {
    pub phase: D2dPhaseV1,
    pub retired_ticket_count: nat,
    pub total_ticket_count: nat,
    pub continuation_visible: bool,
}

// Mutation: one of two tickets retires and exposes continuation while the
// aggregate remains Published.
pub open spec fn mutated_partial_retirement_v1() -> D2dOutcomeV1 {
    D2dOutcomeV1 {
        phase: D2dPhaseV1::Published,
        retired_ticket_count: 1,
        total_ticket_count: 2,
        continuation_visible: true,
    }
}

pub proof fn mutated_d2d_continuation_may_precede_full_retirement_v1()
    ensures {
        let out = mutated_partial_retirement_v1();
        out.continuation_visible ==> (out.phase == D2dPhaseV1::FullyRetired
            && out.retired_ticket_count == out.total_ticket_count)
    }, {}
}
