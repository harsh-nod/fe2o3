use vstd::prelude::*;
verus! {
pub enum AccessV1 { Read, Write, ReadWrite }
pub open spec fn mutated_readwrite_authorization_v1() -> AccessV1 { AccessV1::Write }
pub proof fn mutated_authorization_is_derived_v1()
    ensures mutated_readwrite_authorization_v1() == AccessV1::ReadWrite, {}
}
