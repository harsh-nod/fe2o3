use vstd::prelude::*;

verus! {

pub open spec fn max_async_waiters_v1() -> nat { 65_536 }

pub proof fn mutated_capacity_above_bound_is_admitted_v1()
    ensures max_async_waiters_v1() > 65_536,
{
}

}
