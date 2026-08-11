use vstd::prelude::*;

verus! {

pub struct TargetLayout {
    pub architecture: nat,
    pub xnack_disabled: bool,
    pub little_endian: bool,
    pub flat_pointer_bits: nat,
    pub flat_pointer_alignment: nat,
    pub global_pointer_bits: nat,
    pub global_pointer_alignment: nat,
    pub workgroup_pointer_bits: nat,
    pub workgroup_pointer_alignment: nat,
    pub constant_pointer_bits: nat,
    pub constant_pointer_alignment: nat,
    pub private_pointer_bits: nat,
    pub private_pointer_alignment: nat,
}

pub struct Allocation {
    pub id: nat,
    pub generation: nat,
    pub base: nat,
    pub byte_len: nat,
    pub alive_from: nat,
    pub alive_through: nat,
}

pub struct Provenance {
    pub allocation_id: nat,
    pub generation: nat,
}

pub struct ByteRange {
    pub start: nat,
    pub len: nat,
}

pub struct PhysicalRange {
    pub address_space: nat,
    pub start: nat,
    pub len: nat,
}

pub struct ValidityRange {
    pub start: nat,
    pub end_inclusive: nat,
}

pub enum LoanKind {
    Shared,
    Exclusive,
}

pub struct Loan {
    pub allocation_id: nat,
    pub generation: nat,
    pub range: ByteRange,
    pub kind: LoanKind,
    pub borrow_epoch: nat,
    pub alive_from: nat,
    pub alive_through: nat,
}

pub open spec fn gfx942_xnack_minus(target: TargetLayout) -> bool {
    target.architecture == 942
        && target.xnack_disabled
        && target.little_endian
        && target.flat_pointer_bits == 64
        && target.flat_pointer_alignment == 64
        && target.global_pointer_bits == 64
        && target.global_pointer_alignment == 64
        && target.workgroup_pointer_bits == 32
        && target.workgroup_pointer_alignment == 32
        && target.constant_pointer_bits == 64
        && target.constant_pointer_alignment == 64
        && target.private_pointer_bits == 32
        && target.private_pointer_alignment == 32
}

pub open spec fn known_address_space(address_space: nat) -> bool {
    address_space == 0 || address_space == 1 || address_space == 3
        || address_space == 4 || address_space == 5
}

pub open spec fn pointer_bits(target: TargetLayout, address_space: nat) -> nat {
    if address_space == 0 {
        target.flat_pointer_bits
    } else if address_space == 1 {
        target.global_pointer_bits
    } else if address_space == 3 {
        target.workgroup_pointer_bits
    } else if address_space == 4 {
        target.constant_pointer_bits
    } else {
        target.private_pointer_bits
    }
}

pub open spec fn pointer_max(target: TargetLayout, address_space: nat) -> nat {
    if pointer_bits(target, address_space) == 32 {
        4_294_967_295
    } else {
        18_446_744_073_709_551_615
    }
}

pub open spec fn pointer_value_representable(
    target: TargetLayout,
    address_space: nat,
    address: nat,
) -> bool {
    gfx942_xnack_minus(target)
        && known_address_space(address_space)
        && address <= pointer_max(target, address_space)
}

pub open spec fn allocation_range_representable(
    target: TargetLayout,
    address_space: nat,
    base: nat,
    len: nat,
) -> bool {
    pointer_value_representable(target, address_space, base)
        && base + len <= pointer_max(target, address_space) + 1
}

pub open spec fn pointer_range_representable(
    target: TargetLayout,
    address_space: nat,
    allocation_base: nat,
    range: ByteRange,
) -> bool {
    pointer_value_representable(target, address_space, allocation_base + range.start)
        && allocation_base + range.start + range.len
            <= pointer_max(target, address_space) + 1
}

pub open spec fn range_end(range: ByteRange) -> nat {
    range.start + range.len
}

pub open spec fn provenance_matches(allocation: Allocation, provenance: Provenance) -> bool {
    allocation.id == provenance.allocation_id
        && allocation.generation == provenance.generation
}

pub open spec fn allocation_live_at(allocation: Allocation, epoch: nat) -> bool {
    allocation.alive_from <= epoch && epoch <= allocation.alive_through
}

pub open spec fn range_in_bounds(allocation: Allocation, range: ByteRange) -> bool {
    range_end(range) <= allocation.byte_len
}

pub open spec fn range_contains(parent: ByteRange, child: ByteRange) -> bool {
    parent.start <= child.start && range_end(child) <= range_end(parent)
}

pub open spec fn ranges_overlap(left: ByteRange, right: ByteRange) -> bool {
    left.len > 0 && right.len > 0
        && left.start < range_end(right)
        && right.start < range_end(left)
}

pub open spec fn physical_ranges_overlap(left: PhysicalRange, right: PhysicalRange) -> bool {
    left.address_space == right.address_space
        && left.len > 0
        && right.len > 0
        && left.start < right.start + right.len
        && right.start < left.start + left.len
}

pub open spec fn live_storage_disjoint(left: PhysicalRange, right: PhysicalRange) -> bool {
    !physical_ranges_overlap(left, right)
}

pub open spec fn canonical_validity_pair(
    left: ValidityRange,
    right: ValidityRange,
) -> bool {
    left.start <= left.end_inclusive
        && right.start <= right.end_inclusive
        && left.end_inclusive + 1 < right.start
}

pub open spec fn duplicates_named_any(range: ValidityRange, scalar_max: nat) -> bool {
    range.start == 0 && range.end_inclusive == scalar_max
}

pub open spec fn duplicates_named_nonzero(range: ValidityRange, scalar_max: nat) -> bool {
    range.start == 1 && range.end_inclusive == scalar_max
}

pub open spec fn loans_compatible(left: Loan, right: Loan) -> bool {
    left.allocation_id != right.allocation_id
        || left.generation != right.generation
        || !ranges_overlap(left.range, right.range)
        || (left.kind == LoanKind::Shared && right.kind == LoanKind::Shared)
}

pub open spec fn lifetime_contains(
    outer_from: nat,
    outer_through: nat,
    inner_from: nat,
    inner_through: nat,
) -> bool {
    outer_from <= inner_from && inner_through <= outer_through
}

pub open spec fn initialized_covers(initialized: ByteRange, read: ByteRange) -> bool {
    range_contains(initialized, read)
}

pub open spec fn typed_read_obligations(
    allocation: Allocation,
    provenance: Provenance,
    access: ByteRange,
    initialized: ByteRange,
    epoch: nat,
) -> bool {
    provenance_matches(allocation, provenance)
        && allocation_live_at(allocation, epoch)
        && range_in_bounds(allocation, access)
        && initialized_covers(initialized, access)
}

pub proof fn nested_range_stays_in_bounds(
    allocation: Allocation,
    parent: ByteRange,
    child: ByteRange,
)
    requires
        range_in_bounds(allocation, parent),
        range_contains(parent, child),
    ensures
        range_in_bounds(allocation, child),
{
}

pub proof fn stale_generation_cannot_match(
    allocation: Allocation,
    provenance: Provenance,
)
    requires
        allocation.generation != provenance.generation,
    ensures
        !provenance_matches(allocation, provenance),
{
}

pub proof fn nested_loan_lifetime_is_live_with_allocation(
    allocation: Allocation,
    loan: Loan,
    epoch: nat,
)
    requires
        lifetime_contains(
            allocation.alive_from,
            allocation.alive_through,
            loan.alive_from,
            loan.alive_through,
        ),
        loan.alive_from <= epoch,
        epoch <= loan.alive_through,
    ensures
        allocation_live_at(allocation, epoch),
{
}

pub proof fn ordered_exclusive_ranges_are_compatible(
    allocation_id: nat,
    generation: nat,
    left: ByteRange,
    right: ByteRange,
    left_epoch: nat,
    right_epoch: nat,
)
    requires
        range_end(left) <= right.start,
    ensures
        loans_compatible(
            Loan {
                allocation_id,
                generation,
                range: left,
                kind: LoanKind::Exclusive,
                borrow_epoch: left_epoch,
                alive_from: 0,
                alive_through: 0,
            },
            Loan {
                allocation_id,
                generation,
                range: right,
                kind: LoanKind::Exclusive,
                borrow_epoch: right_epoch,
                alive_from: 0,
                alive_through: 0,
            },
        ),
{
}

pub proof fn valid_write_initializes_same_typed_read(
    allocation: Allocation,
    provenance: Provenance,
    written: ByteRange,
    epoch: nat,
)
    requires
        provenance_matches(allocation, provenance),
        allocation_live_at(allocation, epoch),
        range_in_bounds(allocation, written),
    ensures
        typed_read_obligations(allocation, provenance, written, written, epoch),
{
}

pub proof fn same_allocation_element_distance_is_integral(
    left_offset: nat,
    right_offset: nat,
    element_size: nat,
    elements: nat,
)
    requires
        element_size > 0,
        right_offset == left_offset + elements * element_size,
    ensures
        right_offset - left_offset == elements * element_size,
{
}

pub proof fn gfx942_profile_fixes_pointer_widths_and_alignments(target: TargetLayout)
    requires
        gfx942_xnack_minus(target),
    ensures
        pointer_bits(target, 0) == 64,
        pointer_bits(target, 1) == 64,
        pointer_bits(target, 3) == 32,
        pointer_bits(target, 4) == 64,
        pointer_bits(target, 5) == 32,
        target.workgroup_pointer_alignment == 32,
        target.private_pointer_alignment == 32,
{
}

pub proof fn exclusive_bound_is_not_a_materialized_workgroup_pointer(target: TargetLayout)
    requires
        gfx942_xnack_minus(target),
    ensures
        allocation_range_representable(target, 3, 4_294_967_292, 4),
        pointer_range_representable(
            target,
            3,
            4_294_967_292,
            ByteRange { start: 3, len: 1 },
        ),
        !pointer_value_representable(target, 3, 4_294_967_296),
        !pointer_range_representable(
            target,
            3,
            4_294_967_292,
            ByteRange { start: 4, len: 0 },
        ),
{
}

pub proof fn zero_size_storage_never_overlaps(
    address_space: nat,
    left_start: nat,
    right_start: nat,
    right_len: nat,
)
    ensures
        live_storage_disjoint(
            PhysicalRange { address_space, start: left_start, len: 0 },
            PhysicalRange { address_space, start: right_start, len: right_len },
        ),
{
}

pub proof fn admitted_live_storage_makes_copy_ranges_disjoint(
    left: PhysicalRange,
    right: PhysicalRange,
)
    requires
        live_storage_disjoint(left, right),
    ensures
        !physical_ranges_overlap(left, right),
{
}

pub proof fn adjacent_validity_ranges_are_not_canonical(
    left_start: nat,
    boundary: nat,
    right_end: nat,
)
    requires
        left_start <= boundary,
        boundary + 1 <= right_end,
    ensures
        !canonical_validity_pair(
            ValidityRange { start: left_start, end_inclusive: boundary },
            ValidityRange { start: boundary + 1, end_inclusive: right_end },
        ),
{
}

pub proof fn full_domain_and_nonzero_ranges_require_named_encodings(scalar_max: nat)
    requires
        scalar_max >= 1,
    ensures
        duplicates_named_any(ValidityRange { start: 0, end_inclusive: scalar_max }, scalar_max),
        duplicates_named_nonzero(
            ValidityRange { start: 1, end_inclusive: scalar_max },
            scalar_max,
        ),
{
}

} // verus!
