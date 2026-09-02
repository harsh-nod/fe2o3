use vstd::prelude::*;

verus! {

pub open spec fn path_uses_edge(path: Seq<int>, edge: int) -> bool {
    exists|step: int| 0 <= step < path.len() && #[trigger] path[step] == edge
}

/// Hostile mutation: a bypass path cannot establish the omitted true edge.
pub proof fn bypass_does_not_authenticate_guard(bypass: int, true_edge: int)
    requires bypass != true_edge,
    ensures path_uses_edge(seq![bypass], true_edge),
{
}

}
