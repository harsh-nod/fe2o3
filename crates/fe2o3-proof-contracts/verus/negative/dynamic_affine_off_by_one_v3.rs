use vstd::prelude::*;

verus! {

/// Hostile mutation: nonnegative `extent - index` proves only `index <= extent`.
pub proof fn missing_strict_slack_offset_is_unsound(index: int, extent: int)
    requires 0 <= extent - index,
    ensures index < extent,
{
}

}
