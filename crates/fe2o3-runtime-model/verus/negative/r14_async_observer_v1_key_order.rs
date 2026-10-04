use vstd::prelude::*;

verus! {

pub open spec fn mutated_event_key_less_v1(
    first_context: nat,
    first_event: nat,
    second_context: nat,
    second_event: nat,
) -> bool {
    first_event < second_event
}

pub proof fn mutated_event_key_order_is_lexicographic_v1()
    ensures mutated_event_key_less_v1(1, 9, 2, 1),
{
}

}
