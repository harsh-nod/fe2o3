// Expected-negative R57 mutation: completion omits queue-generation equality.
use vstd::prelude::*;
verus! {
pub struct CompletionV1 {
    pub queue_generation: nat,
    pub dispatch: nat,
    pub signal: nat,
}
pub open spec fn mutated_exact_completion_v1(
    expected_generation: nat, expected_dispatch: nat, expected_signal: nat,
    observed: CompletionV1,
) -> bool {
    observed.dispatch == expected_dispatch && observed.signal == expected_signal
}
pub proof fn mutated_completion_generation_is_rejected_v1(
    expected_generation: nat, observed_generation: nat, dispatch: nat, signal: nat,
)
    requires observed_generation != expected_generation,
    ensures !mutated_exact_completion_v1(
        expected_generation, dispatch, signal,
        CompletionV1 { queue_generation: observed_generation, dispatch, signal },
    ),
{}
}
fn main() {}
