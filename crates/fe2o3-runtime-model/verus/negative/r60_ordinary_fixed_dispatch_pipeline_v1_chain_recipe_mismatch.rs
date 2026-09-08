// Expected-negative R60 mutation: a physical chain omits exact recipe/storage equality.
use vstd::prelude::*;

verus! {

pub open spec fn mutated_physical_chain_capable_v1(
    predecessor_recipe: nat,
    successor_recipe: nat,
) -> bool {
    predecessor_recipe > 0 && successor_recipe > 0
}

pub proof fn mutated_recipe_storage_mismatch_is_rejected_v1()
    ensures !mutated_physical_chain_capable_v1(1, 2),
{
}

}
