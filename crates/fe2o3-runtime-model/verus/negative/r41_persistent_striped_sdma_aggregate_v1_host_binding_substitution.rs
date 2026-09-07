// Expected-negative R41 mutation: completion substitutes host generation and coherent kind.
use vstd::prelude::*;
verus! {
pub open spec fn captured_host_generation_v1() -> nat { 17 }
pub open spec fn mutated_host_generation_v1() -> nat { 18 }
pub open spec fn mutated_host_is_coherent_v1() -> bool { false }
pub proof fn mutated_host_binding_is_exact_v1()
    ensures
        mutated_host_generation_v1() == captured_host_generation_v1(),
        mutated_host_is_coherent_v1(),
{}
}
