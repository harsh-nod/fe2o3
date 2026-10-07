use vstd::prelude::*;
verus! {
pub enum RoleV1 { Read, Write }
pub open spec fn mutated_destination_role_v1() -> RoleV1 { RoleV1::Read }
pub proof fn mutated_source_and_destination_lease_roles_are_exact_v1()
    ensures mutated_destination_role_v1() == RoleV1::Write, {}
}
