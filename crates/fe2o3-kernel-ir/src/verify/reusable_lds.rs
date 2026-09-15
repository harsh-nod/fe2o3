use super::*;
use crate::{ExecutionCapabilityOperationV1 as E, ExecutionCapabilityRoleV1 as R};

impl FunctionVerifier<'_, '_> {
    pub(super) fn valid_reusable_lds_receiver(
        &mut self,
        contract: &ExecutionCapabilityOpV1,
    ) -> bool {
        let E::ReusableLdsConversion(conversion) = contract.operation else {
            return false;
        };
        let [input] = contract.operands.as_slice() else {
            return false;
        };
        let Some(producer) = self.defining_operation(*input) else {
            return false;
        };
        let [result] = producer.results.as_slice() else {
            return false;
        };
        let OperationKind::ExecutionCapability(allocation) = &producer.kind else {
            return false;
        };
        let (E::LdsAllocateBorrowed { lds, element, layout, elements, .. } | E::LdsAllocate {
            lds,
            element,
            layout,
            elements,
            ..
        }) = allocation.operation
        else {
            return false;
        };
        if result.id != *input
            || lds != conversion.input
            || element != conversion.element
            || layout != conversion.layout
            || elements != conversion.elements
            || allocation.provenance != contract.provenance
            || allocation.workgroup_brand != contract.workgroup_brand
            || allocation.epoch_before != contract.epoch_before
            || allocation.epoch_after.is_some()
            || contract.epoch_after.is_some()
            || contract.source.operation != conversion.defined_function
            || contract.source.occurrence.is_none()
        {
            return false;
        }
        // Ordinary KIR dominance checking still applies. Requiring exactly one
        // occurrence of the allocated handle also rejects copies, phi aliases,
        // abandoned old handles and multiple consuming conversions. This first
        // slice accepts the direct allocation SSA edge only.
        let mut uses = 0usize;
        let mut remaining = self.reusable_lds_work_remaining;
        // Set the retained budget to zero up front: any early failure cannot
        // refund work and allow later conversions to repeat the expensive scan.
        self.reusable_lds_work_remaining = 0;
        let Some(body) = &self.function.body else {
            return false;
        };
        for block in &body.blocks {
            let Some(next) = remaining.checked_sub(1) else {
                return false;
            };
            remaining = next;
            for operation in &block.operations {
                let Some(next) = remaining.checked_sub(1) else {
                    return false;
                };
                remaining = next;
                for operand in operation.kind.operands() {
                    let Some(next) = remaining.checked_sub(1) else {
                        return false;
                    };
                    remaining = next;
                    if operand == *input {
                        uses += 1;
                        if uses != 1 {
                            return false;
                        }
                    }
                }
            }
            if let Some(terminator) = &block.terminator {
                for operand in terminator.operands() {
                    let Some(next) = remaining.checked_sub(1) else {
                        return false;
                    };
                    remaining = next;
                    if operand == *input {
                        return false;
                    }
                }
            }
        }
        self.reusable_lds_work_remaining = remaining;
        uses == 1
    }
}

pub(super) fn operand_contract(
    value: crate::ReusableLdsConversionV1,
) -> Vec<ExecutionOperandContractV1> {
    vec![ExecutionOperandContractV1::Capability {
        source: execution_source(value.input),
        role: ExecutionRoleContractV1::Lds {
            element: value.element,
            layout: value.layout,
            elements: value.elements,
            state: crate::ExecutionLdsStateV1::Uninitialized,
        },
    }]
}
pub(super) fn result_contract(
    value: crate::ReusableLdsConversionV1,
) -> Vec<ExecutionResultContractV1> {
    vec![ExecutionResultContractV1::Capability {
        source: value.output,
        role: ExecutionRoleContractV1::ReusableLds {
            element: value.element,
            layout: value.layout,
            elements: value.elements,
        },
    }]
}
pub(super) fn matches_role(expected: &ExecutionRoleContractV1, actual: &R) -> bool {
    matches!((expected, actual), (ExecutionRoleContractV1::ReusableLds { element: a, layout: b, elements: c },
        R::ReusableLds { element: x, layout: y, elements: z }) if a == x && b == y && c == z)
}
