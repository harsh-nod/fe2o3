use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum CopyPhaseV1 {
    Reserved,
    Published,
}

pub open spec fn mutated_publish_xgmi_copy_v1(
    source_mapping_active: bool,
    destination_mapping_active: bool,
) -> CopyPhaseV1 {
    if source_mapping_active || destination_mapping_active {
        CopyPhaseV1::Published
    } else {
        CopyPhaseV1::Reserved
    }
}

pub proof fn mutated_xgmi_copy_requires_both_active_mappings_v1()
    ensures mutated_publish_xgmi_copy_v1(false, true) == CopyPhaseV1::Reserved,
{
}

} // verus!
