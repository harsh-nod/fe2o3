use vstd::prelude::*;

verus! {

/// Hostile mutation: two equal SSA identities are not distinct runtime symbols.
pub proof fn duplicate_runtime_identity_is_not_distinct(left: int, right: int)
    requires left == right,
    ensures left != right,
{
}

}
