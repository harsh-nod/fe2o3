// Expected-negative R37 mutation: timeout restores the active owner but omits
// its exact published-index membership.
use vstd::prelude::*;

verus! {
pub struct StateV1 {
    pub active: bool,
    pub published_index: bool,
    pub storage_token: nat,
    pub retain_count: nat,
}

pub open spec fn initial_v1() -> StateV1 {
    StateV1 { active: true, published_index: true, storage_token: 7, retain_count: 2 }
}

pub open spec fn mutated_timeout_v1() -> StateV1 {
    StateV1 { active: true, published_index: false, ..initial_v1() }
}

pub proof fn mutated_timeout_restores_exact_operational_custody_v1()
    ensures
        mutated_timeout_v1().active == initial_v1().active,
        mutated_timeout_v1().published_index == initial_v1().published_index,
        mutated_timeout_v1().storage_token == initial_v1().storage_token,
        mutated_timeout_v1().retain_count == initial_v1().retain_count,
{}
}
