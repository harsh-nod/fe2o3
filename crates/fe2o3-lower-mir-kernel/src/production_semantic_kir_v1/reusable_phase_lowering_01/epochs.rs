//! Dynamic epoch labels retain the checked Begin occurrence and source marker.
//! This transport changes no source type, source call, operand, or proof gate.
use super::*;
use fe2o3_kernel_ir::{PhaseKeyV1, PhaseTerminalCallOccurrenceV1};
use sha2::{Digest as _, Sha256};

pub(super) fn scoped_epoch(key: PhaseKeyV1, source_epoch: [u8; 32]) -> PhaseResult<[u8; 32]> {
    if key.bytes() == [0; 32] || source_epoch == [0; 32] {
        return Err(rejected(
            "phase epoch lacks its Begin occurrence or source marker",
        ));
    }
    let mut digest = Sha256::new();
    digest.update(b"fe2o3.kir.reusable-phase.source-epoch.v1\0");
    digest.update(key.bytes());
    digest.update(source_epoch);
    let result = digest.finalize().into();
    if result == [0; 32] {
        return Err(rejected("phase epoch has an empty derived identity"));
    }
    Ok(result)
}

#[derive(Clone, Copy)]
pub(in super::super) struct FinishEpoch {
    terminal: PhaseTerminalCallOccurrenceV1,
    contract: SemanticExecutionCapabilityContractV1,
    before: [u8; 32],
    after: [u8; 32],
}

impl FinishEpoch {
    // Only Descriptions calls this with its source-checked Finish and Begin.
    pub(super) fn new(
        key: PhaseKeyV1,
        terminal: PhaseTerminalCallOccurrenceV1,
        contract: SemanticExecutionCapabilityContractV1,
    ) -> PhaseResult<Self> {
        if !terminal.is_complete()
            || terminal.source.operation != *contract.source_identity().as_bytes()
            || !matches!(
                contract.operation(),
                SemanticExecutionCapabilityOperationV1::WorkgroupBarrier { .. }
            )
        {
            return Err(rejected(
                "phase epoch transport lost its original Finish barrier",
            ));
        }
        let before = contract
            .epoch_before()
            .ok_or_else(|| rejected("phase Finish has no original input epoch"))?;
        let after = contract
            .epoch_after()
            .ok_or_else(|| rejected("phase Finish has no original advanced epoch"))?;
        if before == after {
            return Err(rejected("phase Finish does not advance its original epoch"));
        }
        let before = scoped_epoch(key, *before.as_bytes())?;
        let after = scoped_epoch(key, *after.as_bytes())?;
        if before == after {
            return Err(rejected(
                "phase epoch transport collapsed an original transition",
            ));
        }
        Ok(Self {
            terminal,
            contract,
            before,
            after,
        })
    }

    pub(super) fn block(self) -> u32 {
        self.terminal.source.occurrence.unwrap().expanded_block()
    }

    pub(in super::super) fn lower(
        self,
        source: ExecutionCapabilitySourceV1,
        contract: SemanticExecutionCapabilityContractV1,
        normal_target: u32,
        operation: &ExecutionCapabilityOperationV1,
        provenance: &ExecutionCapabilityProvenanceV1,
        operand_types: &[Type],
    ) -> PhaseResult<(Option<[u8; 32]>, Option<[u8; 32]>)> {
        if source != self.terminal.source
            || contract != self.contract
            || normal_target != self.terminal.expanded_normal_target
        {
            return Err(rejected(
                "phase Finish epoch substituted its exact original source occurrence",
            ));
        }
        let ExecutionCapabilityOperationV1::WorkgroupBarrier {
            input_workgroup, ..
        } = operation
        else {
            return Err(rejected(
                "phase Finish epoch was consumed by another operation",
            ));
        };
        let [Type::ExecutionCapability(input)] = operand_types else {
            return Err(rejected(
                "phase Finish epoch has no exact live Workgroup operand",
            ));
        };
        if !input.is_complete()
            || input.source_type != *input_workgroup
            || input.role != ExecutionCapabilityRoleV1::Workgroup
            || input.provenance != *provenance
            || input.workgroup_brand != contract.workgroup_brand().map(|id| *id.as_bytes())
            || input.epoch != Some(self.before)
        {
            return Err(rejected(
                "phase Finish epoch does not match its original live SSA producer",
            ));
        }
        Ok((Some(self.before), Some(self.after)))
    }
}

#[cfg(test)]
#[path = "epoch_tests.rs"]
mod tests;
