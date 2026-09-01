use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct DigestV1 {
    pub word0: nat,
    pub word1: nat,
    pub word2: nat,
    pub word3: nat,
}

#[derive(PartialEq, Eq)]
pub enum EffectV1 {
    SharedRead,
    ExclusiveWrite,
}

#[derive(PartialEq, Eq)]
pub enum ArgumentOwnershipV1 {
    SharedBorrow,
    UniqueBorrow,
}

#[derive(PartialEq, Eq)]
pub enum RuntimeOwnerV1 {
    Caller,
    PreparedInvocation,
}

#[derive(PartialEq, Eq)]
pub enum RuntimePhaseV1 {
    Loaded,
    Prepared,
}

/// Canonical input admitted by the bounded production-shaped vecadd profile.
pub struct CanonicalVecaddRuntimeInputV1 {
    pub kernel: DigestV1,
    pub generated_host_contract: DigestV1,
    pub rust_layout_contract: DigestV1,
    pub rust_effect_contract: DigestV1,
    pub explicit_bytes: nat,
    pub implicit_offset: nat,
    pub implicit_bytes: nat,
    pub physical_bytes: nat,
    pub descriptor_alignment: nat,
    pub runtime_alignment: nat,
    pub component0_offset: nat,
    pub component1_offset: nat,
    pub component2_offset: nat,
    pub component3_offset: nat,
    pub component4_offset: nat,
    pub component5_offset: nat,
    pub left_effect: EffectV1,
    pub right_effect: EffectV1,
    pub output_effect: EffectV1,
    pub left_ownership: ArgumentOwnershipV1,
    pub right_ownership: ArgumentOwnershipV1,
    pub output_ownership: ArgumentOwnershipV1,
    pub grid_x: nat,
    pub grid_y: nat,
    pub grid_z: nat,
    pub workgroup_x: nat,
    pub workgroup_y: nat,
    pub workgroup_z: nat,
    pub dynamic_group_bytes: nat,
    pub source_static_group_bytes: nat,
    pub physical_static_group_bytes: nat,
    pub source_required_workgroup_x: nat,
    pub source_required_workgroup_y: nat,
    pub source_required_workgroup_z: nat,
    pub physical_required_workgroup_x: nat,
    pub physical_required_workgroup_y: nat,
    pub physical_required_workgroup_z: nat,
    pub source_max_grid_x: nat,
    pub physical_max_grid_x: nat,
    pub source_max_flat_workgroup: nat,
    pub physical_max_flat_workgroup: nat,
    pub physical_private_segment_bytes: nat,
    pub left_resource: nat,
    pub right_resource: nat,
    pub output_resource: nat,
    pub left_context: nat,
    pub right_context: nat,
    pub output_context: nat,
    pub left_base: nat,
    pub right_base: nat,
    pub output_base: nat,
    pub left_address: nat,
    pub right_address: nat,
    pub output_address: nat,
    pub left_elements: nat,
    pub right_elements: nat,
    pub output_elements: nat,
    pub left_bytes: nat,
    pub right_bytes: nat,
    pub output_bytes: nat,
    pub left_offset: nat,
    pub right_offset: nat,
    pub output_offset: nat,
    pub left_owner: RuntimeOwnerV1,
    pub right_owner: RuntimeOwnerV1,
    pub output_owner: RuntimeOwnerV1,
    pub source_phase: RuntimePhaseV1,
}

pub struct PreparedVecaddRuntimeV1 {
    pub kernel: DigestV1,
    pub generated_host_contract: DigestV1,
    pub rust_layout_contract: DigestV1,
    pub rust_effect_contract: DigestV1,
    pub explicit_bytes: nat,
    pub implicit_offset: nat,
    pub implicit_bytes: nat,
    pub physical_bytes: nat,
    pub descriptor_alignment: nat,
    pub runtime_alignment: nat,
    pub component0_offset: nat,
    pub component1_offset: nat,
    pub component2_offset: nat,
    pub component3_offset: nat,
    pub component4_offset: nat,
    pub component5_offset: nat,
    pub left_effect: EffectV1,
    pub right_effect: EffectV1,
    pub output_effect: EffectV1,
    pub left_ownership: ArgumentOwnershipV1,
    pub right_ownership: ArgumentOwnershipV1,
    pub output_ownership: ArgumentOwnershipV1,
    pub grid_x: nat,
    pub grid_y: nat,
    pub grid_z: nat,
    pub workgroup_x: nat,
    pub workgroup_y: nat,
    pub workgroup_z: nat,
    pub left_resource: nat,
    pub right_resource: nat,
    pub output_resource: nat,
    pub left_context: nat,
    pub right_context: nat,
    pub output_context: nat,
    pub left_base: nat,
    pub right_base: nat,
    pub output_base: nat,
    pub left_address: nat,
    pub right_address: nat,
    pub output_address: nat,
    pub left_elements: nat,
    pub right_elements: nat,
    pub output_elements: nat,
    pub left_bytes: nat,
    pub right_bytes: nat,
    pub output_bytes: nat,
    pub left_offset: nat,
    pub right_offset: nat,
    pub output_offset: nat,
    pub left_owner: RuntimeOwnerV1,
    pub right_owner: RuntimeOwnerV1,
    pub output_owner: RuntimeOwnerV1,
    pub phase: RuntimePhaseV1,
}

pub open spec fn canonical_vecadd_runtime_input_v1(
    input: CanonicalVecaddRuntimeInputV1,
) -> bool {
    &&& input.explicit_bytes == 48
    &&& input.implicit_offset == 48
    &&& input.implicit_bytes == 256
    &&& input.physical_bytes == 304
    &&& input.descriptor_alignment == 8
    &&& input.runtime_alignment == 16
    &&& input.component0_offset == 0
    &&& input.component1_offset == 8
    &&& input.component2_offset == 16
    &&& input.component3_offset == 24
    &&& input.component4_offset == 32
    &&& input.component5_offset == 40
    &&& input.left_effect == EffectV1::SharedRead
    &&& input.right_effect == EffectV1::SharedRead
    &&& input.output_effect == EffectV1::ExclusiveWrite
    &&& input.left_ownership == ArgumentOwnershipV1::SharedBorrow
    &&& input.right_ownership == ArgumentOwnershipV1::SharedBorrow
    &&& input.output_ownership == ArgumentOwnershipV1::UniqueBorrow
    &&& input.output_elements > 0
    &&& input.output_elements <= input.left_elements
    &&& input.output_elements <= input.right_elements
    &&& input.left_bytes == input.left_elements * 4
    &&& input.right_bytes == input.right_elements * 4
    &&& input.output_bytes == input.output_elements * 4
    &&& input.workgroup_x == 256
    &&& input.workgroup_y == 1
    &&& input.workgroup_z == 1
    &&& input.grid_x == (input.output_elements + 255) / 256
    &&& input.grid_y == 1
    &&& input.grid_z == 1
    &&& input.source_required_workgroup_x == 256
    &&& input.source_required_workgroup_y == 1
    &&& input.source_required_workgroup_z == 1
    &&& input.physical_required_workgroup_x == 256
    &&& input.physical_required_workgroup_y == 1
    &&& input.physical_required_workgroup_z == 1
    &&& input.grid_x <= input.source_max_grid_x
    &&& input.grid_x <= input.physical_max_grid_x
    &&& input.grid_x * input.workgroup_x >= input.output_elements
    &&& input.source_max_flat_workgroup >= 256
    &&& input.physical_max_flat_workgroup >= 256
    &&& input.physical_private_segment_bytes == 0
    &&& input.dynamic_group_bytes == 0
    &&& input.source_static_group_bytes == 0
    &&& input.physical_static_group_bytes == 0
    &&& input.left_resource > 0
    &&& input.right_resource > 0
    &&& input.output_resource > 0
    &&& input.left_address == input.left_base + input.left_offset
    &&& input.right_address == input.right_base + input.right_offset
    &&& input.output_address == input.output_base + input.output_offset
    &&& (input.left_context != input.output_context
        || input.left_resource != input.output_resource
        || input.left_offset + input.left_bytes <= input.output_offset
        || input.output_offset + input.output_bytes <= input.left_offset)
    &&& (input.right_context != input.output_context
        || input.right_resource != input.output_resource
        || input.right_offset + input.right_bytes <= input.output_offset
        || input.output_offset + input.output_bytes <= input.right_offset)
    &&& input.left_owner == RuntimeOwnerV1::Caller
    &&& input.right_owner == RuntimeOwnerV1::Caller
    &&& input.output_owner == RuntimeOwnerV1::Caller
    &&& input.source_phase == RuntimePhaseV1::Loaded
}

pub open spec fn prepare_vecadd_runtime_v1(
    input: CanonicalVecaddRuntimeInputV1,
) -> PreparedVecaddRuntimeV1 {
    PreparedVecaddRuntimeV1 {
        kernel: input.kernel,
        generated_host_contract: input.generated_host_contract,
        rust_layout_contract: input.rust_layout_contract,
        rust_effect_contract: input.rust_effect_contract,
        explicit_bytes: input.explicit_bytes,
        implicit_offset: input.implicit_offset,
        implicit_bytes: input.implicit_bytes,
        physical_bytes: input.physical_bytes,
        descriptor_alignment: input.descriptor_alignment,
        runtime_alignment: input.runtime_alignment,
        component0_offset: input.component0_offset,
        component1_offset: input.component1_offset,
        component2_offset: input.component2_offset,
        component3_offset: input.component3_offset,
        component4_offset: input.component4_offset,
        component5_offset: input.component5_offset,
        left_effect: input.left_effect,
        right_effect: input.right_effect,
        output_effect: input.output_effect,
        left_ownership: input.left_ownership,
        right_ownership: input.right_ownership,
        output_ownership: input.output_ownership,
        grid_x: input.grid_x,
        grid_y: input.grid_y,
        grid_z: input.grid_z,
        workgroup_x: input.workgroup_x,
        workgroup_y: input.workgroup_y,
        workgroup_z: input.workgroup_z,
        left_resource: input.left_resource,
        right_resource: input.right_resource,
        output_resource: input.output_resource,
        left_context: input.left_context,
        right_context: input.right_context,
        output_context: input.output_context,
        left_base: input.left_base,
        right_base: input.right_base,
        output_base: input.output_base,
        left_address: input.left_address,
        right_address: input.right_address,
        output_address: input.output_address,
        left_elements: input.left_elements,
        right_elements: input.right_elements,
        output_elements: input.output_elements,
        left_bytes: input.left_bytes,
        right_bytes: input.right_bytes,
        output_bytes: input.output_bytes,
        left_offset: input.left_offset,
        right_offset: input.right_offset,
        output_offset: input.output_offset,
        left_owner: RuntimeOwnerV1::PreparedInvocation,
        right_owner: RuntimeOwnerV1::PreparedInvocation,
        output_owner: RuntimeOwnerV1::PreparedInvocation,
        phase: RuntimePhaseV1::Prepared,
    }
}

/// Exact refinement postcondition. It intentionally stops before publication
/// or execution and therefore contains no LLVM, driver, firmware, or device
/// execution proposition.
pub open spec fn vecadd_runtime_preparation_refinement_v1(
    input: CanonicalVecaddRuntimeInputV1,
    prepared: PreparedVecaddRuntimeV1,
) -> bool {
    &&& prepared.kernel == input.kernel
    &&& prepared.generated_host_contract == input.generated_host_contract
    &&& prepared.rust_layout_contract == input.rust_layout_contract
    &&& prepared.rust_effect_contract == input.rust_effect_contract
    &&& prepared.explicit_bytes == 48
    &&& prepared.implicit_offset == 48
    &&& prepared.implicit_bytes == 256
    &&& prepared.physical_bytes == 304
    &&& prepared.descriptor_alignment == 8
    &&& prepared.runtime_alignment == 16
    &&& prepared.component0_offset == 0
    &&& prepared.component1_offset == 8
    &&& prepared.component2_offset == 16
    &&& prepared.component3_offset == 24
    &&& prepared.component4_offset == 32
    &&& prepared.component5_offset == 40
    &&& prepared.left_effect == input.left_effect
    &&& prepared.right_effect == input.right_effect
    &&& prepared.output_effect == input.output_effect
    &&& prepared.left_ownership == input.left_ownership
    &&& prepared.right_ownership == input.right_ownership
    &&& prepared.output_ownership == input.output_ownership
    &&& prepared.grid_x == input.grid_x
    &&& prepared.grid_y == input.grid_y
    &&& prepared.grid_z == input.grid_z
    &&& prepared.workgroup_x == input.workgroup_x
    &&& prepared.workgroup_y == input.workgroup_y
    &&& prepared.workgroup_z == input.workgroup_z
    &&& prepared.grid_x * prepared.workgroup_x >= prepared.output_elements
    &&& prepared.left_resource == input.left_resource
    &&& prepared.right_resource == input.right_resource
    &&& prepared.output_resource == input.output_resource
    &&& prepared.left_context == input.left_context
    &&& prepared.right_context == input.right_context
    &&& prepared.output_context == input.output_context
    &&& prepared.left_base == input.left_base
    &&& prepared.right_base == input.right_base
    &&& prepared.output_base == input.output_base
    &&& prepared.left_address == input.left_address
    &&& prepared.right_address == input.right_address
    &&& prepared.output_address == input.output_address
    &&& prepared.left_elements == input.left_elements
    &&& prepared.right_elements == input.right_elements
    &&& prepared.output_elements == input.output_elements
    &&& prepared.left_bytes == input.left_bytes
    &&& prepared.right_bytes == input.right_bytes
    &&& prepared.output_bytes == input.output_bytes
    &&& prepared.left_offset == input.left_offset
    &&& prepared.right_offset == input.right_offset
    &&& prepared.output_offset == input.output_offset
    &&& prepared.output_elements <= prepared.left_elements
    &&& prepared.output_elements <= prepared.right_elements
    &&& prepared.left_owner == RuntimeOwnerV1::PreparedInvocation
    &&& prepared.right_owner == RuntimeOwnerV1::PreparedInvocation
    &&& prepared.output_owner == RuntimeOwnerV1::PreparedInvocation
    &&& prepared.phase == RuntimePhaseV1::Prepared
}

/// The first formal ABI/runtime refinement theorem used by production host
/// admission. The theorem is exclusively about the pure preparation relation.
pub proof fn vecadd_runtime_preparation_refines_v1(
    input: CanonicalVecaddRuntimeInputV1,
)
    requires
        canonical_vecadd_runtime_input_v1(input),
    ensures
        vecadd_runtime_preparation_refinement_v1(input, prepare_vecadd_runtime_v1(input)),
{
}

} // verus!
