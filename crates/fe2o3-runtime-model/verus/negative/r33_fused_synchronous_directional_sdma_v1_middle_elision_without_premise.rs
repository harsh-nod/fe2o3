// Expected-negative R33 mutation: the old wait open is false while the fused
// machine elides it without the required aligned/sticky-currentness premise.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum OutcomeV1 { Timeout, Terminal }
#[derive(PartialEq, Eq)] pub enum CustodyV1 { PendingPublished, TerminalPublished }
pub struct StateV1 { pub outcome: OutcomeV1, pub custody: CustodyV1 }

pub open spec fn former_without_alignment_v1() -> StateV1 {
    StateV1 { outcome: OutcomeV1::Terminal, custody: CustodyV1::TerminalPublished }
}

pub open spec fn mutated_fused_without_alignment_v1() -> StateV1 {
    StateV1 { outcome: OutcomeV1::Timeout, custody: CustodyV1::PendingPublished }
}

pub proof fn mutated_middle_elision_preserves_external_semantics_v1()
    ensures former_without_alignment_v1().outcome == mutated_fused_without_alignment_v1().outcome,
        former_without_alignment_v1().custody == mutated_fused_without_alignment_v1().custody,
{}
}
