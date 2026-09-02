use vstd::prelude::*;

verus! {

pub open spec fn mutated_loaded_instruction_receipt_v1(receipt: nat) -> nat {
    receipt + 1
}

pub proof fn mutated_instruction_class_receipt_is_exact_v1(receipt: nat)
    requires receipt > 0,
    ensures mutated_loaded_instruction_receipt_v1(receipt) == receipt,
{
}

} // verus!
