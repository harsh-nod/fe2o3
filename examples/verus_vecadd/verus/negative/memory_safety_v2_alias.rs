use vstd::prelude::*;

#[path = "../memory_safety_v2.rs"]
mod memory_safety_v2;
use memory_safety_v2::*;

verus! {

proof fn mutated_overlapping_exclusive_loans_are_compatible(
    allocation_id: nat,
    generation: nat,
)
    ensures
        loans_compatible(
            Loan {
                allocation_id,
                generation,
                range: ByteRange { start: 0, len: 8 },
                kind: LoanKind::Exclusive,
                borrow_epoch: 1,
                alive_from: 0,
                alive_through: 1,
            },
            Loan {
                allocation_id,
                generation,
                range: ByteRange { start: 4, len: 8 },
                kind: LoanKind::Exclusive,
                borrow_epoch: 2,
                alive_from: 0,
                alive_through: 1,
            },
        ),
{
}

} // verus!
