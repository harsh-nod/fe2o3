use super::*;

impl FunctionVerifier<'_, '_> {
    pub(super) fn valid_borrowed_subgroup_receiver(
        &self,
        contract: &ExecutionCapabilityOpV1,
    ) -> bool {
        let crate::ExecutionCapabilityOperationV1::SubgroupDeriveBorrowed { workgroup, .. } =
            contract.operation
        else {
            return false;
        };
        let [owner] = contract.operands.as_slice() else {
            return false;
        };
        let Some(definition) = self.defining_operation(*owner) else {
            return false;
        };
        if definition.results.len() != 1 || definition.results[0].id != *owner {
            return false;
        }
        let OperationKind::ExecutionCapability(issuer) = &definition.kind else {
            return false;
        };
        matches!(issuer.operation, crate::ExecutionCapabilityOperationV1::WorkgroupDerive { workgroup: issued, .. } if issued == workgroup)
            && issuer.is_complete()
            && issuer.provenance == contract.provenance
            && issuer.workgroup_brand == contract.workgroup_brand
            && issuer.epoch_before == contract.epoch_before
            && issuer.epoch_after.is_none()
            && same_execution_root_custody(issuer.source, contract.source)
    }
}

pub(super) fn same_execution_root_custody(
    a: crate::ExecutionCapabilitySourceV1,
    b: crate::ExecutionCapabilitySourceV1,
) -> bool {
    match (a.occurrence, b.occurrence) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.root_source_identity() == b.root_source_identity()
                && a.expansion_identity() == b.expansion_identity()
                && a.expanded_root_identity() == b.expanded_root_identity()
        }
        _ => false,
    }
}
