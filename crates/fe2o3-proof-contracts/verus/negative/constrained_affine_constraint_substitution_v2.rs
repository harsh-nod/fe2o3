use vstd::prelude::*;

verus! {

/// Hostile mutation: substituting `x <= 8` for the certified `x <= 7`
/// cannot retain the old strict extent-eight conclusion.
pub proof fn substituted_constraint_is_not_sound(x: int)
    requires 0 <= x <= 8,
    ensures x < 8,
{
}

}
