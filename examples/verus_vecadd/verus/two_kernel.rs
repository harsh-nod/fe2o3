use vstd::prelude::*;

include!("../src/two_kernel_bodies.rs");

#[path = "vecadd.rs"]
pub mod permission_model;

verus! {

pub struct ModelDisjointSlice {
    pub values: Vec<i64>,
}

impl ModelDisjointSlice {
    pub fn get_mut(&mut self, index: usize) -> (element: Option<&mut i64>)
        ensures
            match element {
                Some(element) => {
                    &&& index < old(self).values@.len()
                    &&& *element == old(self).values@[index as int]
                    &&& final(self).values@ == old(self).values@.update(
                        index as int,
                        *final(element),
                    )
                }
                None => {
                    &&& index >= old(self).values@.len()
                    &&& final(self).values@ == old(self).values@
                }
            },
    {
        if index < self.values.len() {
            Some(&mut self.values[index])
        } else {
            None
        }
    }
}

/// Exact, bounded mathematical abstraction of the `f32` multiply call site.
/// Relating this operation to IEEE-754 multiplication is a later refinement
/// obligation; the shared control and memory-access body is unchanged.
pub open spec fn alpha_math(scale: i16, value: i16) -> int {
    scale as int * value as int
}

pub fn exact_alpha(scale: i16, value: i16) -> (result: i64)
    ensures
        result as int == alpha_math(scale, value),
{
    assert(i16::MIN as int <= scale as int <= i16::MAX as int);
    assert(i16::MIN as int <= value as int <= i16::MAX as int);
    assert(i64::MIN as int <= scale as int * value as int <= i64::MAX as int)
        by (nonlinear_arith);
    scale as i64 * value as i64
}

/// Exact, bounded mathematical abstraction of `(a + b) + bias`.
pub open spec fn zeta_math(a: i16, b: i16, bias: i32) -> int {
    a as int + b as int + bias as int
}

pub fn exact_zeta(a: i16, b: i16, bias: i32) -> (result: i64)
    ensures
        result as int == zeta_math(a, b, bias),
{
    assert(i16::MIN as int <= a as int <= i16::MAX as int);
    assert(i16::MIN as int <= b as int <= i16::MAX as int);
    assert(i32::MIN as int <= bias as int <= i32::MAX as int);
    assert(i64::MIN as int <= a as int + b as int + bias as int <= i64::MAX as int)
        by (linear_arith);
    a as i64 + b as i64 + bias as i64
}

pub struct AlphaEvidence {
    pub input_allocation: permission_model::Allocation,
    pub output_allocation: permission_model::Allocation,
    pub input_capability: permission_model::RegionCapability,
    pub output_capability: permission_model::RegionCapability,
}

pub struct ZetaEvidence {
    pub a_allocation: permission_model::Allocation,
    pub b_allocation: permission_model::Allocation,
    pub output_allocation: permission_model::Allocation,
    pub a_capability: permission_model::RegionCapability,
    pub b_capability: permission_model::RegionCapability,
    pub output_capability: permission_model::RegionCapability,
}

pub open spec fn models_f32_slice(
    allocation: permission_model::Allocation,
    length: nat,
) -> bool {
    permission_model::allocation_is_representable(allocation)
        && allocation.byte_length == length * 4
        && allocation.address_space_size <= usize::MAX as nat
}

pub open spec fn read_capability_at(
    allocation: permission_model::Allocation,
    index: nat,
) -> permission_model::RegionCapability {
    permission_model::initialized_read_capability(
        permission_model::element_region(allocation, index, 4),
    )
}

pub open spec fn output_capability_at(
    allocation: permission_model::Allocation,
    index: nat,
    initialized: bool,
) -> permission_model::RegionCapability {
    permission_model::RegionCapability {
        permission: permission_model::exclusive_write(
            permission_model::element_region(allocation, index, 4),
        ),
        initialized,
    }
}

pub open spec fn capability_after_write(
    capability: permission_model::RegionCapability,
) -> permission_model::RegionCapability {
    permission_model::RegionCapability {
        permission: capability.permission,
        initialized: true,
    }
}

pub open spec fn alpha_evidence_is_valid(
    evidence: AlphaEvidence,
    length: nat,
    thread: nat,
) -> bool {
    models_f32_slice(evidence.input_allocation, length)
        && models_f32_slice(evidence.output_allocation, length)
        && evidence.input_allocation.id != evidence.output_allocation.id
        && evidence.input_capability == read_capability_at(evidence.input_allocation, thread)
        && evidence.output_capability.permission
            == output_capability_at(evidence.output_allocation, thread, false).permission
}

pub open spec fn zeta_evidence_is_valid(
    evidence: ZetaEvidence,
    length: nat,
    thread: nat,
) -> bool {
    models_f32_slice(evidence.a_allocation, length)
        && models_f32_slice(evidence.b_allocation, length)
        && models_f32_slice(evidence.output_allocation, length)
        && evidence.output_allocation.id != evidence.a_allocation.id
        && evidence.output_allocation.id != evidence.b_allocation.id
        && evidence.a_capability == read_capability_at(evidence.a_allocation, thread)
        && evidence.b_capability == read_capability_at(evidence.b_allocation, thread)
        && evidence.output_capability.permission
            == output_capability_at(evidence.output_allocation, thread, false).permission
}

/// Shared proof for every initialized `f32` input element in this slice.
pub proof fn initialized_read_is_bounded(
    allocation: permission_model::Allocation,
    capability: permission_model::RegionCapability,
    length: nat,
    index: nat,
)
    requires
        models_f32_slice(allocation, length),
        index < length,
        capability == read_capability_at(allocation, index),
    ensures
        permission_model::region_is_in_bounds(allocation, capability.permission.region),
        permission_model::capability_can_read(capability),
        permission_model::element_byte_end(allocation, index, 4) <= usize::MAX as nat,
{
    permission_model::element_region_is_in_bounds_and_address_representable(
        allocation,
        length,
        index,
        4,
    );
}

/// Shared proof for the exclusive element selected by either kernel.
pub proof fn exclusive_output_is_bounded_and_initialized_by_write(
    allocation: permission_model::Allocation,
    capability: permission_model::RegionCapability,
    length: nat,
    index: nat,
)
    requires
        models_f32_slice(allocation, length),
        index < length,
        capability.permission == output_capability_at(allocation, index, false).permission,
    ensures
        permission_model::region_is_in_bounds(allocation, capability.permission.region),
        permission_model::permission_can_write(capability.permission),
        capability_after_write(capability).initialized,
        permission_model::element_byte_end(allocation, index, 4) <= usize::MAX as nat,
{
    permission_model::element_region_is_in_bounds_and_address_representable(
        allocation,
        length,
        index,
        4,
    );
}

pub proof fn alpha_permissions_are_valid(
    evidence: AlphaEvidence,
    length: nat,
    thread: nat,
)
    requires
        alpha_evidence_is_valid(evidence, length, thread),
        thread < length,
    ensures
        permission_model::region_is_in_bounds(
            evidence.input_allocation,
            evidence.input_capability.permission.region,
        ),
        permission_model::region_is_in_bounds(
            evidence.output_allocation,
            evidence.output_capability.permission.region,
        ),
        permission_model::capability_can_read(evidence.input_capability),
        permission_model::permission_can_write(evidence.output_capability.permission),
        capability_after_write(evidence.output_capability).initialized,
        permission_model::permissions_are_compatible(
            evidence.input_capability.permission,
            evidence.output_capability.permission,
        ),
        permission_model::element_byte_end(evidence.input_allocation, thread, 4)
            <= usize::MAX as nat,
        permission_model::element_byte_end(evidence.output_allocation, thread, 4)
            <= usize::MAX as nat,
{
    initialized_read_is_bounded(
        evidence.input_allocation,
        evidence.input_capability,
        length,
        thread,
    );
    exclusive_output_is_bounded_and_initialized_by_write(
        evidence.output_allocation,
        evidence.output_capability,
        length,
        thread,
    );
    assert(!permission_model::regions_overlap(
        evidence.input_capability.permission.region,
        evidence.output_capability.permission.region,
    ));
}

pub proof fn zeta_permissions_are_valid(
    evidence: ZetaEvidence,
    length: nat,
    thread: nat,
)
    requires
        zeta_evidence_is_valid(evidence, length, thread),
        thread < length,
    ensures
        permission_model::region_is_in_bounds(
            evidence.a_allocation,
            evidence.a_capability.permission.region,
        ),
        permission_model::region_is_in_bounds(
            evidence.b_allocation,
            evidence.b_capability.permission.region,
        ),
        permission_model::region_is_in_bounds(
            evidence.output_allocation,
            evidence.output_capability.permission.region,
        ),
        permission_model::capability_can_read(evidence.a_capability),
        permission_model::capability_can_read(evidence.b_capability),
        permission_model::permission_can_write(evidence.output_capability.permission),
        capability_after_write(evidence.output_capability).initialized,
        permission_model::permissions_are_compatible(
            evidence.a_capability.permission,
            evidence.b_capability.permission,
        ),
        permission_model::permissions_are_compatible(
            evidence.a_capability.permission,
            evidence.output_capability.permission,
        ),
        permission_model::permissions_are_compatible(
            evidence.b_capability.permission,
            evidence.output_capability.permission,
        ),
        permission_model::element_byte_end(evidence.a_allocation, thread, 4)
            <= usize::MAX as nat,
        permission_model::element_byte_end(evidence.b_allocation, thread, 4)
            <= usize::MAX as nat,
        permission_model::element_byte_end(evidence.output_allocation, thread, 4)
            <= usize::MAX as nat,
{
    initialized_read_is_bounded(
        evidence.a_allocation,
        evidence.a_capability,
        length,
        thread,
    );
    initialized_read_is_bounded(
        evidence.b_allocation,
        evidence.b_capability,
        length,
        thread,
    );
    exclusive_output_is_bounded_and_initialized_by_write(
        evidence.output_allocation,
        evidence.output_capability,
        length,
        thread,
    );
    assert(!permission_model::regions_overlap(
        evidence.a_capability.permission.region,
        evidence.output_capability.permission.region,
    ));
    assert(!permission_model::regions_overlap(
        evidence.b_capability.permission.region,
        evidence.output_capability.permission.region,
    ));
}

/// Same-source model for `alpha(scale, input, output)`. A rounded launch tail
/// exits through `get_mut` before indexing `input`.
pub fn verified_alpha_thread(
    thread: usize,
    scale: i16,
    input: &[i16],
    mut output: ModelDisjointSlice,
    Ghost(evidence): Ghost<AlphaEvidence>,
) -> (result: ModelDisjointSlice)
    requires
        input@.len() == output.values@.len(),
        thread < output.values@.len() ==>
            alpha_evidence_is_valid(evidence, output.values@.len(), thread as nat),
    ensures
        result.values@.len() == output.values@.len(),
        thread >= output.values@.len() ==> result.values@ == output.values@,
        thread < output.values@.len() ==>
            result.values@ == output.values@.update(
                thread as int,
                alpha_math(scale, input@[thread as int]) as i64,
            ),
        thread < output.values@.len() ==>
            permission_model::capability_can_read(evidence.input_capability),
        thread < output.values@.len() ==>
            permission_model::permission_can_write(evidence.output_capability.permission),
        thread < output.values@.len() ==>
            capability_after_write(evidence.output_capability).initialized,
        thread < output.values@.len() ==>
            permission_model::permissions_are_compatible(
                evidence.input_capability.permission,
                evidence.output_capability.permission,
            ),
        thread < output.values@.len() ==>
            permission_model::element_byte_end(evidence.input_allocation, thread as nat, 4)
                <= usize::MAX as nat,
        thread < output.values@.len() ==>
            permission_model::element_byte_end(evidence.output_allocation, thread as nat, 4)
                <= usize::MAX as nat,
{
    proof {
        if thread < output.values@.len() {
            alpha_permissions_are_valid(
                evidence,
                output.values@.len(),
                thread as nat,
            );
        }
    }
    alpha_kernel_body!(thread, exact_alpha, scale, input, output);
    output
}

/// Same-source model for `zeta(a, b, bias, output)`. Shared inputs may alias;
/// the exclusive output allocation must be separate from both.
pub fn verified_zeta_thread(
    thread: usize,
    a: &[i16],
    b: &[i16],
    bias: i32,
    mut output: ModelDisjointSlice,
    Ghost(evidence): Ghost<ZetaEvidence>,
) -> (result: ModelDisjointSlice)
    requires
        a@.len() == output.values@.len(),
        b@.len() == output.values@.len(),
        thread < output.values@.len() ==>
            zeta_evidence_is_valid(evidence, output.values@.len(), thread as nat),
    ensures
        result.values@.len() == output.values@.len(),
        thread >= output.values@.len() ==> result.values@ == output.values@,
        thread < output.values@.len() ==>
            result.values@ == output.values@.update(
                thread as int,
                zeta_math(a@[thread as int], b@[thread as int], bias) as i64,
            ),
        thread < output.values@.len() ==>
            permission_model::capability_can_read(evidence.a_capability),
        thread < output.values@.len() ==>
            permission_model::capability_can_read(evidence.b_capability),
        thread < output.values@.len() ==>
            permission_model::permission_can_write(evidence.output_capability.permission),
        thread < output.values@.len() ==>
            capability_after_write(evidence.output_capability).initialized,
        thread < output.values@.len() ==>
            permission_model::permissions_are_compatible(
                evidence.a_capability.permission,
                evidence.output_capability.permission,
            ),
        thread < output.values@.len() ==>
            permission_model::permissions_are_compatible(
                evidence.b_capability.permission,
                evidence.output_capability.permission,
            ),
        thread < output.values@.len() ==>
            permission_model::element_byte_end(evidence.a_allocation, thread as nat, 4)
                <= usize::MAX as nat,
        thread < output.values@.len() ==>
            permission_model::element_byte_end(evidence.b_allocation, thread as nat, 4)
                <= usize::MAX as nat,
        thread < output.values@.len() ==>
            permission_model::element_byte_end(evidence.output_allocation, thread as nat, 4)
                <= usize::MAX as nat,
{
    proof {
        if thread < output.values@.len() {
            zeta_permissions_are_valid(
                evidence,
                output.values@.len(),
                thread as nat,
            );
        }
    }
    zeta_kernel_body!(thread, exact_zeta, a, b, bias, output);
    output
}

/// Identity ownership is injective, so distinct active threads in either
/// kernel own disjoint four-byte output elements and cannot race.
pub proof fn two_kernel_identity_ownership_is_race_free(
    output_allocation: permission_model::Allocation,
    left: nat,
    right: nat,
    length: nat,
)
    requires
        left < length,
        right < length,
        left != right,
    ensures
        permission_model::output_index(left) != permission_model::output_index(right),
        !permission_model::regions_overlap(
            permission_model::element_region(output_allocation, left, 4),
            permission_model::element_region(output_allocation, right, 4),
        ),
        permission_model::permissions_are_compatible(
            permission_model::exclusive_write(
                permission_model::element_region(output_allocation, left, 4),
            ),
            permission_model::exclusive_write(
                permission_model::element_region(output_allocation, right, 4),
            ),
        ),
{
    permission_model::distinct_threads_have_disjoint_output_regions(
        output_allocation,
        left,
        right,
        length,
        4,
    );
}

} // verus!
