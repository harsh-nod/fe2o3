use super::*;
use crate::{ExecutionCapabilityOperationV1 as E, SubgroupPartitionOperationV1 as P};

impl FunctionVerifier<'_, '_> {
    pub(super) fn valid_subgroup_partition_receiver(
        &self,
        contract: &ExecutionCapabilityOpV1,
    ) -> bool {
        let E::SubgroupPartition(partition) = contract.operation else {
            return false;
        };
        match partition {
            P::Derive { width, .. } => {
                let [subgroup, workgroup] = contract.operands.as_slice() else {
                    return false;
                };
                let Some(issuer) = self.partition_value_issuer(*subgroup) else {
                    return false;
                };
                matches!(issuer.operation, E::SubgroupDerive { width: issued, .. } | E::SubgroupDeriveBorrowed { width: issued, .. } if issued == width)
                    && (!matches!(issuer.operation, E::SubgroupDeriveBorrowed { .. }) || self.valid_borrowed_subgroup_receiver(issuer))
                    && issuer.operands.as_slice() == [*workgroup]
                    && same_partition_epoch(issuer, contract)
            }
            P::ReduceSumF32 {
                width,
                partition_width,
                ..
            }
            | P::ReduceMaxF32 {
                width,
                partition_width,
                ..
            }
            | P::BroadcastF32 {
                width,
                partition_width,
                ..
            } => {
                let Some(receiver) = contract.operands.first() else {
                    return false;
                };
                let Some(issuer) = self.partition_value_issuer(*receiver) else {
                    return false;
                };
                if !matches!(issuer.operation, E::SubgroupPartition(P::Derive {
                    width: issued_width, partition_width: issued_partition, ..
                }) if issued_width == width && issued_partition == partition_width)
                    || !same_partition_epoch(issuer, contract)
                    || !self.valid_subgroup_partition_receiver(issuer)
                {
                    return false;
                }
                // Reuse only a bound present in the value itself, never insert a mask.
                match partition {
                    P::BroadcastF32 { .. } => contract
                        .operands
                        .get(2)
                        .is_some_and(|lane| self.bounded_u32_source_lane(*lane, partition_width)),
                    _ => true,
                }
            }
        }
    }

    fn partition_value_issuer(&self, value: ValueId) -> Option<&ExecutionCapabilityOpV1> {
        let operation = self.defining_operation(value)?;
        if operation.results.len() != 1 || operation.results[0].id != value {
            return None;
        }
        match &operation.kind {
            OperationKind::ExecutionCapability(contract) => Some(contract),
            _ => None,
        }
    }
}

fn same_partition_epoch(a: &ExecutionCapabilityOpV1, b: &ExecutionCapabilityOpV1) -> bool {
    a.provenance == b.provenance
        && a.workgroup_brand == b.workgroup_brand
        && a.epoch_before == b.epoch_before
        && a.epoch_after.is_none()
        && b.epoch_after.is_none()
}
