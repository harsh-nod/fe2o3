use vstd::prelude::*;

verus! {

pub open spec fn path_uses_edge(path: Seq<int>, edge: int) -> bool {
    exists|step: int| 0 <= step < path.len() && #[trigger] path[step] == edge
}

// Hostile mutation: a bypass edge is incorrectly treated as if it crossed
// the removed true edge. Verus must reject the postcondition.
pub proof fn bypass_path_does_not_establish_edge_dominance(
    bypass_edge: int,
    removed_edge: int,
)
    requires bypass_edge != removed_edge,
    ensures path_uses_edge(seq![bypass_edge], removed_edge),
{
}

}
