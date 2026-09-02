use vstd::prelude::*;

verus! {

/// Hostile mutation: one retained row is not a two-guard roster.
pub proof fn single_guard_is_not_multiple(rows: Seq<int>)
    requires rows.len() == 1,
    ensures rows.len() >= 2,
{
}

}
