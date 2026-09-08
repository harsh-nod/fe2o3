// Expected-negative R57 mutation: completion omits queue equality.
use vstd::prelude::*;
verus! {
pub struct CompletionV1 { pub queue: nat, pub dispatch: nat, pub signal: nat }
pub open spec fn mutated_exact_completion_v1(
    expected_queue: nat, expected_dispatch: nat, expected_signal: nat,
    observed: CompletionV1,
) -> bool {
    observed.dispatch == expected_dispatch && observed.signal == expected_signal
}
pub proof fn mutated_completion_queue_is_rejected_v1(
    expected_queue: nat, observed_queue: nat, dispatch: nat, signal: nat,
)
    requires observed_queue != expected_queue,
    ensures !mutated_exact_completion_v1(
        expected_queue, dispatch, signal,
        CompletionV1 { queue: observed_queue, dispatch, signal },
    ),
{}
}
fn main() {}
