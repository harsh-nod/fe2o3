// Expected-negative R60 mutation: preparation accepts substituted storage.
use vstd::prelude::*;
verus! {
pub struct EntryV1 { pub recipe: nat, pub prepared_recipe: nat }
pub open spec fn mutated_prepare_v1(entry: EntryV1, observed: nat) -> EntryV1 {
    EntryV1 { prepared_recipe: observed, ..entry }
}
pub proof fn mutated_recipe_substitution_is_no_effect_v1(entry: EntryV1, substituted: nat)
    requires substituted != entry.recipe,
    ensures mutated_prepare_v1(entry, substituted) == entry,
{}
}
