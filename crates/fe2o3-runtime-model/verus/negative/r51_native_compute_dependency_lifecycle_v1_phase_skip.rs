// Expected-negative R51 mutation: Prepared skips NativePublished.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum PhaseV1 { Prepared, NativePublished, Published }
pub open spec fn mutated_next_v1() -> PhaseV1 { PhaseV1::Published }
pub proof fn mutated_prepared_next_is_native_published_v1()
    ensures mutated_next_v1() == PhaseV1::NativePublished,
{}
}
