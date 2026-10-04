use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)]
pub enum AccessV1 { Read, Write }
pub open spec fn mutated_source_xgmi_route_metadata_valid_v1(
    source: nat,
    route_source: nat,
    engine: nat,
    access: AccessV1,
) -> bool {
    source == route_source && engine <= 16
}
pub proof fn mutated_xgmi_route_metadata_roster_and_access_are_exact_v1()
    ensures !mutated_source_xgmi_route_metadata_valid_v1(1, 1, 16, AccessV1::Write),
{}
}
