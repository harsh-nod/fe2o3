use vstd::prelude::*;

verus! {

pub open spec fn prefix_v1(values: Seq<int>, end: nat) -> int
    recommends end <= values.len(),
    decreases end,
{
    if end == 0 {
        0
    } else {
        prefix_v1(values, (end - 1) as nat) + values[(end - 1) as int]
    }
}

pub open spec fn mutated_inclusive_v1(values: Seq<int>, lane: nat) -> int {
    prefix_v1(values, lane)
}

pub proof fn mutated_inclusive_obeys_recurrence_v1(values: Seq<int>, lane: nat)
    requires
        values.len() == 64,
        lane < 64,
        values[lane as int] == 1,
    ensures mutated_inclusive_v1(values, lane)
        == prefix_v1(values, lane) + values[lane as int],
{
}

}
