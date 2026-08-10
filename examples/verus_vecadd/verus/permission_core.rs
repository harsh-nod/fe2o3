use vstd::prelude::*;

verus! {

/// Symbolic allocation metadata supplied by the source-model environment.
pub struct Allocation {
    pub id: nat,
    pub address_space: nat,
    pub base_address: nat,
    pub byte_length: nat,
    pub address_space_size: nat,
}

/// A half-open byte range retaining its allocation provenance.
pub struct ByteRegion {
    pub allocation_id: nat,
    pub address_space: nat,
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

pub struct RegionCapability {
    pub permission: RegionPermission,
    pub initialized: bool,
}

pub open spec fn allocation_is_representable(allocation: Allocation) -> bool {
    allocation.base_address + allocation.byte_length <= allocation.address_space_size
}

pub open spec fn region_is_in_bounds(allocation: Allocation, region: ByteRegion) -> bool {
    allocation.id == region.allocation_id
        && allocation.address_space == region.address_space
        && region.byte_length > 0
        && region.byte_offset + region.byte_length <= allocation.byte_length
        && allocation.base_address + region.byte_offset + region.byte_length
            <= allocation.address_space_size
}

pub open spec fn regions_overlap(left: ByteRegion, right: ByteRegion) -> bool {
    left.allocation_id == right.allocation_id
        && left.address_space == right.address_space
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

pub open spec fn shared_read(region: ByteRegion) -> RegionPermission {
    RegionPermission { kind: PermissionKind::SharedRead, region }
}

pub open spec fn exclusive_write(region: ByteRegion) -> RegionPermission {
    RegionPermission { kind: PermissionKind::ExclusiveWrite, region }
}

pub open spec fn initialized_read_capability(region: ByteRegion) -> RegionCapability {
    RegionCapability {
        permission: shared_read(region),
        initialized: true,
    }
}

pub open spec fn capability_can_read(capability: RegionCapability) -> bool {
    capability.permission.kind == PermissionKind::SharedRead && capability.initialized
}

pub open spec fn permission_can_write(permission: RegionPermission) -> bool {
    permission.kind == PermissionKind::ExclusiveWrite
}

pub open spec fn output_index(thread: nat) -> nat {
    thread
}

pub open spec fn element_region(
    allocation: Allocation,
    index: nat,
    element_size: nat,
) -> ByteRegion {
    ByteRegion {
        allocation_id: allocation.id,
        address_space: allocation.address_space,
        byte_offset: index * element_size,
        byte_length: element_size,
    }
}

pub open spec fn element_byte_address(
    allocation: Allocation,
    index: nat,
    element_size: nat,
) -> nat {
    allocation.base_address + index * element_size
}

pub open spec fn element_byte_end(
    allocation: Allocation,
    index: nat,
    element_size: nat,
) -> nat {
    element_byte_address(allocation, index, element_size) + element_size
}

pub proof fn element_region_is_in_bounds_and_address_representable(
    allocation: Allocation,
    element_count: nat,
    index: nat,
    element_size: nat,
)
    requires
        allocation_is_representable(allocation),
        allocation.byte_length == element_count * element_size,
        element_size > 0,
        index < element_count,
    ensures
        region_is_in_bounds(allocation, element_region(allocation, index, element_size)),
        element_byte_address(allocation, index, element_size)
            < element_byte_end(allocation, index, element_size),
        element_byte_end(allocation, index, element_size) <= allocation.address_space_size,
{
    assert(index + 1 <= element_count);
    assert((index + 1) * element_size <= element_count * element_size) by (nonlinear_arith)
        requires
            index + 1 <= element_count,
            element_size > 0,
    ;
    assert(index * element_size + element_size == (index + 1) * element_size)
        by (nonlinear_arith);
}

/// Unequal identity indices yield disjoint output byte ranges.
pub proof fn distinct_threads_have_disjoint_output_regions(
    output_allocation: Allocation,
    left: nat,
    right: nat,
    thread_count: nat,
    element_size: nat,
)
    requires
        left < thread_count,
        right < thread_count,
        left != right,
        element_size > 0,
    ensures
        !regions_overlap(
            element_region(output_allocation, output_index(left), element_size),
            element_region(output_allocation, output_index(right), element_size),
        ),
        permissions_are_compatible(
            exclusive_write(element_region(
                output_allocation,
                output_index(left),
                element_size,
            )),
            exclusive_write(element_region(
                output_allocation,
                output_index(right),
                element_size,
            )),
        ),
{
    if left < right {
        assert((left + 1) * element_size <= right * element_size) by (nonlinear_arith)
            requires
                left < right,
                element_size > 0,
        ;
        assert(left * element_size + element_size == (left + 1) * element_size)
            by (nonlinear_arith);
    } else {
        assert(right < left);
        assert((right + 1) * element_size <= left * element_size) by (nonlinear_arith)
            requires
                right < left,
                element_size > 0,
        ;
        assert(right * element_size + element_size == (right + 1) * element_size)
            by (nonlinear_arith);
    }
}

} // verus!
