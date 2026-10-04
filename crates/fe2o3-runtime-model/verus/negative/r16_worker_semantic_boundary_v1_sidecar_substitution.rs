use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum ContractV1 { Atomic }

pub struct PublicationV1 {
    pub runtime_profile: nat,
    pub contract: Option<ContractV1>,
}

pub struct RecordV1 {
    pub runtime_profile: nat,
    pub contract: Option<ContractV1>,
}

pub open spec fn mutated_join_v1(publication: PublicationV1, record: RecordV1) -> bool {
    record.runtime_profile == publication.runtime_profile
}

pub proof fn mutated_sidecar_contract_substitution_is_rejected_v1()
    ensures !mutated_join_v1(
        PublicationV1 { runtime_profile: 7, contract: Some(ContractV1::Atomic) },
        RecordV1 { runtime_profile: 7, contract: None },
    ),
{
}

}
