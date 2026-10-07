// Expected-negative R39 mutation: Ready substitutes terminal custody and
// eagerly publishes a continuation instead of carrying the R37 snapshot.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum CustodyV1 { Active, Terminal }
pub struct StateV1 { pub custody: CustodyV1, pub continuation_publications: nat }

pub open spec fn mutated_ready_v1() -> StateV1 {
    StateV1 { custody: CustodyV1::Terminal, continuation_publications: 1 }
}

pub proof fn mutated_ready_retains_custody_and_continuation_v1()
    ensures
        mutated_ready_v1().custody == CustodyV1::Active,
        mutated_ready_v1().continuation_publications == 0,
{}
}
