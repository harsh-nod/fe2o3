use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct ProjectionV1 {
    pub kfd_schema: nat,
    pub drm_schema: nat,
}

pub proof fn mutated_projection_drops_drm_schema_v1(kfd: nat, drm: nat)
    requires
        drm > 0,
    ensures
        (ProjectionV1 { kfd_schema: kfd, drm_schema: 0 })
            == (ProjectionV1 { kfd_schema: kfd, drm_schema: drm }),
{
}

} // verus!
