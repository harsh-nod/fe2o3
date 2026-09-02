use vstd::prelude::*;

verus! {

/// Hostile mutation: `x <= 7` proves `x < 8`, not the tightened `x < 7`.
pub proof fn tightened_extent_is_not_sound(x: int)
    requires 0 <= x <= 7,
    ensures x < 7,
{
}

}
