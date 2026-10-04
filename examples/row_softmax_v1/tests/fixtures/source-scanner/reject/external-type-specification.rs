use vstd::prelude::*;

verus! {
#[verifier::external_type_specification]
struct RejectedType(u64);
} // verus!
