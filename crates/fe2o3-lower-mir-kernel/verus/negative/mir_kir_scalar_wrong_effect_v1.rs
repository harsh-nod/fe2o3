use vstd::prelude::*;

verus! {
spec fn mir_effects(left: int, right: int, destination: int) -> Seq<int> {
    seq![left, right, destination, (left + right) % 4294967296]
}
spec fn mutated_kir_effects(left: int, right: int, destination: int) -> Seq<int> {
    seq![left, right, destination + 1, (left + right) % 4294967296]
}

proof fn hostile_effect_mutation_is_not_a_refinement(
    left: int,
    right: int,
    destination: int,
)
    ensures mir_effects(left, right, destination) == mutated_kir_effects(left, right, destination),
{
}
}

fn main() {}
