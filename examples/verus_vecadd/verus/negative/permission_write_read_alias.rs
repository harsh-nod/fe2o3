use vstd::prelude::*;

verus! {

pub struct ByteRegion {
    pub allocation_id: nat,
    pub byte_offset: nat,
    pub byte_length: nat,
}

pub enum PermissionKind {
    SharedRead,
    ExclusiveWrite,
}

pub struct RegionPermission {
    pub kind: PermissionKind,
    pub region: ByteRegion,
}

pub open spec fn regions_overlap(left: ByteRegion, right: ByteRegion) -> bool {
    left.allocation_id == right.allocation_id
        && left.byte_offset < right.byte_offset + right.byte_length
        && right.byte_offset < left.byte_offset + left.byte_length
}

pub open spec fn permissions_are_compatible(
    left: RegionPermission,
    right: RegionPermission,
) -> bool {
    !regions_overlap(left.region, right.region)
        || (left.kind == PermissionKind::SharedRead
            && right.kind == PermissionKind::SharedRead)
}

/// Expected failure marker: mutated_write_read_alias_is_compatible.
pub proof fn mutated_write_read_alias_is_compatible(region: ByteRegion)
    requires
        region.byte_length > 0,
    ensures
        permissions_are_compatible(
            RegionPermission { kind: PermissionKind::ExclusiveWrite, region },
            RegionPermission { kind: PermissionKind::SharedRead, region },
        ),
{
}

} // verus!
