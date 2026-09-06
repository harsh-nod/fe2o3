// Expected-negative R39 mutation: the elapsed-floor test uses <= and keeps
// spinning at the exact boundary instead of resuming the default yield stage.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum ActionV1 { Spin, Yield }
pub open spec fn mutated_boundary_action_v1() -> ActionV1 { ActionV1::Spin }

pub proof fn mutated_exact_boundary_resumes_default_yield_v1()
    ensures mutated_boundary_action_v1() == ActionV1::Yield,
{}
}
