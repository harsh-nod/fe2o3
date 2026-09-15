//! Borrowed subgroup keeps its source reference distinct from its SSA owner.

use super::*;

pub(super) fn contract_revision(operation: &ExecutionCapabilityOperationV1) -> u8 {
    match operation {
        ExecutionCapabilityOperationV1::ReusableLdsConversion(_) => 6,
        ExecutionCapabilityOperationV1::LdsAllocateBorrowed { .. }
        | ExecutionCapabilityOperationV1::SubgroupDeriveBorrowed { .. } => 3,
        ExecutionCapabilityOperationV1::NumericalPolicyMath(_) => 4,
        ExecutionCapabilityOperationV1::NumericalPolicyIssue { .. }
        | ExecutionCapabilityOperationV1::SubgroupPartition(_) => 2,
        _ => 1,
    }
}

pub(super) fn type_revision(role: &ExecutionCapabilityRoleV1) -> u8 {
    match role {
        ExecutionCapabilityRoleV1::ReusableLds { .. } => 6,
        ExecutionCapabilityRoleV1::BorrowedSubgroup { .. } => 3,
        ExecutionCapabilityRoleV1::NumericalPolicyMathSource(_)
        | ExecutionCapabilityRoleV1::NumericalPolicyMathBound(_) => 4,
        ExecutionCapabilityRoleV1::NumericalPolicy { .. }
        | ExecutionCapabilityRoleV1::SubgroupPartition { .. } => 2,
        _ => 1,
    }
}

pub(super) const fn obligations() -> u32 {
    ExecutionSafetyObligationsV1::TARGET_SUPPORT
        | ExecutionSafetyObligationsV1::DYNAMIC_WORKGROUP_IDENTITY
        | ExecutionSafetyObligationsV1::LIFETIME_VALIDITY
}

pub(super) fn valid_pair(
    reference: ExecutionTypeIdentityV1,
    workgroup: ExecutionTypeIdentityV1,
    width: u32,
) -> bool {
    reference.is_complete() && workgroup.is_complete() && reference != workgroup && width == 64
}

pub(super) fn encode_role(
    writer: &mut ContractWriter,
    reference: ExecutionTypeIdentityV1,
    workgroup: ExecutionTypeIdentityV1,
    width: u32,
) {
    writer.u8(15);
    writer.identity(reference);
    writer.identity(workgroup);
    writer.u32(width);
}

pub(super) fn decode_role(reader: &mut ContractReader<'_>) -> Option<ExecutionCapabilityRoleV1> {
    Some(ExecutionCapabilityRoleV1::BorrowedSubgroup {
        workgroup_reference: reader.identity()?,
        workgroup: reader.identity()?,
        width: reader.u32()?,
    })
}

#[cfg(test)]
mod tests;
