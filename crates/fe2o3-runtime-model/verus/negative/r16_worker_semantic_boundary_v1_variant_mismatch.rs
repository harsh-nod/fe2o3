use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum SemanticKindV1 { Atomic, Collective }

pub open spec fn mutated_semantic_request_valid_v1(
    opcode: SemanticKindV1,
    variant: SemanticKindV1,
) -> bool {
    opcode == SemanticKindV1::Atomic
}

pub proof fn mutated_variant_mismatch_is_rejected_v1()
    ensures !mutated_semantic_request_valid_v1(
        SemanticKindV1::Atomic,
        SemanticKindV1::Collective,
    ),
{
}

}
