// Expected-negative R51 mutation: one target routes to two source arenas.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_source_arenas_v1() -> Seq<nat> { seq![7nat, 9nat] }
pub proof fn mutated_target_has_one_source_arena_v1()
    ensures forall|i: int| 0 <= i < mutated_source_arenas_v1().len()
        ==> #[trigger] mutated_source_arenas_v1()[i] == mutated_source_arenas_v1()[0],
{}
}
