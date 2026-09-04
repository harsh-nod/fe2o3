use vstd::prelude::*;
verus! {
pub enum RoleV1 { Source, Destination }
pub open spec fn mutated_d2h_role_v1() -> RoleV1 { RoleV1::Destination }
pub proof fn mutated_d2h_is_exact_read_source_v1()
    ensures mutated_d2h_role_v1() == RoleV1::Source,
{}
}
