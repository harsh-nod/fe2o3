use super::*;
use crate::ExecutionCapabilityOperationV1 as E;

impl FunctionVerifier<'_, '_> {
    pub(super) fn valid_borrowed_lds_receiver(
        &self,
        contract: &ExecutionCapabilityOpV1,
        location: &DiagnosticLocation,
    ) -> bool {
        let Some((block, index)) = location.block.zip(location.operation) else {
            return false;
        };
        let E::LdsAllocateBorrowed { workgroup, .. } = contract.operation else {
            return false;
        };
        let [owner] = contract.operands.as_slice() else {
            return false;
        };
        let Some(definition) = self.definitions.get(owner) else {
            return false;
        };
        let DefSite::Operation(issuer_block, issuer_index) = definition.site else {
            return false;
        };
        if !self.site_dominates(definition.site, block, Some(index)) {
            return false;
        }
        let Some(operation) = self.defining_operation(*owner) else {
            return false;
        };
        let [result] = operation.results.as_slice() else {
            return false;
        };
        let OperationKind::ExecutionCapability(issuer) = &operation.kind else {
            return false;
        };
        let [context] = issuer.operands.as_slice() else {
            return false;
        };
        let Some(context_definition) = self.definitions.get(context) else {
            return false;
        };
        let Some(context_issue) = self.defining_operation(*context) else {
            return false;
        };
        // Direct typed SSA edges only. Live source moves, borrows and call-return
        // transfers must already preserve these identities under the SSA owner.
        // Required lifetime/alias obligations are retained, not discharged here.
        result.id == *owner
            && matches!(issuer.operation, E::WorkgroupDerive { workgroup: actual, .. } if actual == workgroup)
            && issuer.is_complete()
            && issuer.provenance == contract.provenance
            && issuer.workgroup_brand == contract.workgroup_brand
            && issuer.epoch_before == contract.epoch_before
            && issuer.epoch_after.is_none()
            && borrowed_subgroup::same_execution_root_custody(issuer.source, contract.source)
            && self.function.role == FunctionRole::KernelEntry
            && matches!(context_issue.kind, OperationKind::KernelContextIssue(_))
            && context_issue.results.len() == 1
            && context_issue.results[0].id == *context
            && self.site_dominates(context_definition.site, issuer_block, Some(issuer_index))
            && execution_operand_type_matches(
                Some(&context_definition.ty),
                ExecutionOperandContractV1::KernelContext,
                issuer,
            )
    }
}
