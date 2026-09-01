use vstd::prelude::*;

verus! {

/// Hostile mutation: the valid certificate for `2 * x + 1`, `0 <= x < 8`
/// has maximum 15 and extent 16. Tightening the extent to 15 must not prove
/// the strict upper bound.
pub proof fn tightened_extent_is_not_sound(x: int)
    requires
        0 <= x < 8,
    ensures
        2 * x + 1 < 15,
{
}

}
