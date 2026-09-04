use vstd::prelude::*;

verus! {

pub open spec fn mutated_register_count_v1(waiter_count: nat, duplicate: bool) -> nat {
    waiter_count + 1
}

pub proof fn mutated_duplicate_registration_is_atomic_v1(waiter_count: nat)
    ensures mutated_register_count_v1(waiter_count, true) == waiter_count,
{
}

}
