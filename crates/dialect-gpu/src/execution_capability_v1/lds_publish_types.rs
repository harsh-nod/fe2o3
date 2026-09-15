use super::*;
use fe2o3_kernel_ir::{
    ExecutionCapabilityOperationV1, ExecutionCapabilityRoleV1, ExecutionLdsStateV1,
    ExecutionMemoryAccessV1, ExecutionMemoryAddressSpaceV1, ExecutionMemoryInitializationV1,
    ExecutionTypeIdentityV1,
};

// Publish is an existing two-result operation: the transition workgroup and
// published LDS have different source types. Neither is the other's authority.
pub(super) fn valid(
    context: &Context,
    raw: &Operation,
    contract: &KirExecutionCapabilityOp,
) -> bool {
    if raw.get_num_operands() != 2 || raw.get_num_results() != 2 {
        return false;
    }
    let matches =
        |value: Value, source: ExecutionTypeIdentityV1, role: ExecutionCapabilityRoleV1, epoch| {
            let ty = value.get_type(context);
            let ty = ty.deref(context);
            let Some(capability) = ty
                .downcast_ref::<ExecutionCapabilityType>()
                .and_then(ExecutionCapabilityType::capability)
            else {
                return false;
            };
            capability.source_type == source
                && capability.role == role
                && capability.provenance == contract.provenance
                && capability.workgroup_brand == contract.workgroup_brand
                && capability.epoch == epoch
        };
    let (input_workgroup, input_storage, output_storage, transition, before_role, after_role) =
        match contract.operation {
            ExecutionCapabilityOperationV1::LdsPublish {
                input_workgroup,
                input_lds,
                output_lds,
                transition,
                element,
                layout,
                elements,
            } => {
                let lds = |state| ExecutionCapabilityRoleV1::Lds {
                    element,
                    layout,
                    elements,
                    state,
                };
                (
                    input_workgroup,
                    input_lds,
                    output_lds,
                    transition,
                    lds(ExecutionLdsStateV1::InvocationInitialized),
                    lds(ExecutionLdsStateV1::Published),
                )
            }
            ExecutionCapabilityOperationV1::WorkgroupMemoryPublish {
                input_workgroup,
                input_view,
                output_view,
                transition,
                element,
                layout,
            } => {
                let input_type = raw.get_operand(1).get_type(context);
                let input_type = input_type.deref(context);
                let Some(input) = input_type
                    .downcast_ref::<ExecutionCapabilityType>()
                    .and_then(ExecutionCapabilityType::capability)
                else {
                    return false;
                };
                let ExecutionCapabilityRoleV1::MemoryView {
                    extent,
                    index_space: Some(index_space),
                    ..
                } = input.role
                else {
                    return false;
                };
                let view =
                    |access, initialization, index_space| ExecutionCapabilityRoleV1::MemoryView {
                        element,
                        layout,
                        space: ExecutionMemoryAddressSpaceV1::Workgroup,
                        access,
                        extent,
                        initialization,
                        index_space,
                        atomic_scope: None,
                    };
                (
                    input_workgroup,
                    input_view,
                    output_view,
                    transition,
                    view(
                        ExecutionMemoryAccessV1::DisjointWrite,
                        ExecutionMemoryInitializationV1::Uninitialized,
                        Some(index_space),
                    ),
                    view(
                        ExecutionMemoryAccessV1::ReadOnly,
                        ExecutionMemoryInitializationV1::Published,
                        None,
                    ),
                )
            }
            _ => return false,
        };
    matches(
        raw.get_operand(0),
        input_workgroup,
        ExecutionCapabilityRoleV1::Workgroup,
        contract.epoch_before,
    ) && matches(
        raw.get_operand(1),
        input_storage,
        before_role,
        contract.epoch_before,
    ) && matches(
        raw.get_result(0),
        transition,
        ExecutionCapabilityRoleV1::Workgroup,
        contract.epoch_after,
    ) && matches(
        raw.get_result(1),
        output_storage,
        after_role,
        contract.epoch_after,
    )
}
