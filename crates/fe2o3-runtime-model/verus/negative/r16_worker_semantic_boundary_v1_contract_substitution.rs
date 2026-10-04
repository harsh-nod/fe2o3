use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum ContractV1 { Atomic, Collective }

pub struct WorkerStateV1 {
    pub pending_contract: Option<ContractV1>,
}

pub open spec fn mutated_accept_v1(_contract: ContractV1) -> WorkerStateV1 {
    WorkerStateV1 { pending_contract: Some(ContractV1::Collective) }
}

pub proof fn mutated_custody_preserves_exact_contract_v1()
    ensures mutated_accept_v1(ContractV1::Atomic).pending_contract
        == Some(ContractV1::Atomic),
{
}

}
