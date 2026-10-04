use vstd::prelude::*;

verus! {

pub open spec fn mutated_atomic_scope_corresponds_v1(declared: nat, observed: nat) -> bool {
    declared != observed
}

pub proof fn mutated_substituted_atomic_scope_never_corresponds_v1(
    declared: nat,
    observed: nat,
)
    requires declared != observed,
    ensures !mutated_atomic_scope_corresponds_v1(declared, observed),
{
}

} // verus!
