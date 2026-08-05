use vstd::prelude::*;

verus! {

/// Symbolic allocation metadata supplied by the launch environment.
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

pub open spec fn allocation_is_representable(allocation: Allocation) -> bool {
    allocation.base_address + allocation.byte_length <= allocation.address_space_size
}

pub open spec fn region_is_in_bounds(
    allocation: Allocation,
    region: ByteRegion,
) -> bool {
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

/// Target-neutral model of the identity write mapping used by the Rust example.
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

pub open spec fn vecadd_value(a: Seq<int>, b: Seq<int>, thread: nat) -> int
    recommends
        thread < a.len(),
        thread < b.len(),
{
    a[thread as int] + b[thread as int]
}

pub open spec fn vecadd_write(
    old_output: Seq<int>,
    a: Seq<int>,
    b: Seq<int>,
    thread: nat,
) -> Seq<int>
    recommends
        thread < old_output.len(),
        thread < a.len(),
        thread < b.len(),
{
    old_output.update(output_index(thread) as int, vecadd_value(a, b, thread))
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

pub proof fn per_thread_vecadd_has_valid_region_permissions(
    a: Seq<int>,
    b: Seq<int>,
    output: Seq<int>,
    a_allocation: Allocation,
    b_allocation: Allocation,
    output_allocation: Allocation,
    thread: nat,
    element_size: nat,
)
    requires
        a.len() == b.len(),
        a.len() == output.len(),
        thread < output.len(),
        element_size > 0,
        allocation_is_representable(a_allocation),
        allocation_is_representable(b_allocation),
        allocation_is_representable(output_allocation),
        a_allocation.byte_length == a.len() * element_size,
        b_allocation.byte_length == b.len() * element_size,
        output_allocation.byte_length == output.len() * element_size,
        output_allocation.id != a_allocation.id,
        output_allocation.id != b_allocation.id,
    ensures
        region_is_in_bounds(
            a_allocation,
            element_region(a_allocation, thread, element_size),
        ),
        region_is_in_bounds(
            b_allocation,
            element_region(b_allocation, thread, element_size),
        ),
        region_is_in_bounds(
            output_allocation,
            element_region(output_allocation, output_index(thread), element_size),
        ),
        permissions_are_compatible(
            shared_read(element_region(a_allocation, thread, element_size)),
            shared_read(element_region(b_allocation, thread, element_size)),
        ),
        permissions_are_compatible(
            shared_read(element_region(a_allocation, thread, element_size)),
            exclusive_write(element_region(output_allocation, output_index(thread), element_size)),
        ),
        permissions_are_compatible(
            shared_read(element_region(b_allocation, thread, element_size)),
            exclusive_write(element_region(output_allocation, output_index(thread), element_size)),
        ),
        vecadd_value(a, b, thread) == a[thread as int] + b[thread as int],
{
    element_region_is_in_bounds_and_address_representable(
        a_allocation,
        a.len(),
        thread,
        element_size,
    );
    element_region_is_in_bounds_and_address_representable(
        b_allocation,
        b.len(),
        thread,
        element_size,
    );
    element_region_is_in_bounds_and_address_representable(
        output_allocation,
        output.len(),
        thread,
        element_size,
    );
}

/// Shared reads are compatible even when both inputs name the same bytes.
pub proof fn shared_input_reads_may_alias(region: ByteRegion)
    ensures
        regions_overlap(region, region) ==> permissions_are_compatible(
            shared_read(region),
            shared_read(region),
        ),
{
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

pub proof fn vecadd_changes_only_the_owned_output(
    old_output: Seq<int>,
    a: Seq<int>,
    b: Seq<int>,
    output_allocation: Allocation,
    thread: nat,
    other: nat,
    element_size: nat,
)
    requires
        old_output.len() == a.len(),
        old_output.len() == b.len(),
        thread < old_output.len(),
        other < old_output.len(),
        other != output_index(thread),
        element_size > 0,
    ensures
        vecadd_write(old_output, a, b, thread)[other as int] == old_output[other as int],
        !regions_overlap(
            element_region(output_allocation, output_index(thread), element_size),
            element_region(output_allocation, other, element_size),
        ),
{
    distinct_threads_have_disjoint_output_regions(
        output_allocation,
        thread,
        other,
        old_output.len(),
        element_size,
    );
}

/// A write also frames every region from another symbolic allocation.
pub proof fn output_write_frames_other_allocations(
    output_allocation: Allocation,
    framed_allocation: Allocation,
    output_index: nat,
    framed_index: nat,
    element_size: nat,
)
    requires
        output_allocation.id != framed_allocation.id,
        element_size > 0,
    ensures
        !regions_overlap(
            element_region(output_allocation, output_index, element_size),
            element_region(framed_allocation, framed_index, element_size),
        ),
        permissions_are_compatible(
            exclusive_write(element_region(output_allocation, output_index, element_size)),
            shared_read(element_region(framed_allocation, framed_index, element_size)),
        ),
{
}

/// Trusted hardware/backend boundary. The backend must refine this contract,
/// and launch composition must separately guarantee distinct IDs for distinct
/// active threads.
#[verifier::external_body]
pub fn hardware_thread_id(thread_count: usize) -> (thread: usize)
    requires
        thread_count > 0,
    ensures
        thread < thread_count,
{
    unimplemented!()
}

} // verus!
