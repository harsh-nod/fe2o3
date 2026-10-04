use vstd::prelude::*;

verus! {

pub open spec fn mutated_causal_key_v1(query: nat, key: nat) -> bool {
    key <= query + 1
}

pub proof fn mutated_future_key_is_excluded_v1()
    ensures !mutated_causal_key_v1(0, 1),
{
}

}
