// Expected-negative R45 mutation: source signal coordinates may drift.
use vstd::prelude::*;
verus! {
pub open spec fn source_signal_generation_v1() -> nat { 59 }
pub open spec fn observed_signal_generation_v1() -> nat { 60 }
pub proof fn mutated_source_signal_identity_is_exact_v1()
    ensures source_signal_generation_v1() == observed_signal_generation_v1(),
{}
}
