use vstd::prelude::*;
verus! {
spec fn source_effects(left: int, right: int, destination: int) -> Seq<int> {
    seq![left, right, destination, (left + right) % 4294967296]
}
spec fn mutated_effects(left: int, right: int, destination: int) -> Seq<int> {
    seq![left, right, destination + 1, (left + right) % 4294967296]
}
proof fn wrong_effect(left: int, right: int, destination: int)
    ensures source_effects(left, right, destination) == mutated_effects(left, right, destination),
{}
}
fn main() {}
