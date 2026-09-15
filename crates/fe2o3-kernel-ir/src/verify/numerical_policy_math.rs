use super::*;
use crate::{ExecutionCapabilityOperationV1 as E, NumericalPolicyMathOperationV1 as M};

pub(super) fn operand_contract(math: M) -> Vec<ExecutionOperandContractV1> {
    let binding = math.binding();
    let capability = |source, role| ExecutionOperandContractV1::Capability {
        source: execution_source(source),
        role,
    };
    match math {
        M::MathDerive { .. } => vec![ExecutionOperandContractV1::KernelContext],
        M::Bind { .. } => vec![
            capability(
                binding.math,
                ExecutionRoleContractV1::NumericalPolicyMathSource(binding),
            ),
            capability(
                binding.capability,
                ExecutionRoleContractV1::NumericalPolicy {
                    policy: binding.policy,
                    mode: binding.mode,
                },
            ),
        ],
        M::F32 { function, .. } => {
            let mut operands = vec![capability(
                binding.bound,
                ExecutionRoleContractV1::NumericalPolicyMathBound(binding),
            )];
            operands.extend(std::iter::repeat_n(
                ExecutionOperandContractV1::Scalar(ScalarType::F32),
                function.arity(),
            ));
            operands
        }
    }
}

pub(super) fn result_contract(math: M) -> Vec<ExecutionResultContractV1> {
    let binding = math.binding();
    vec![match math {
        M::MathDerive { .. } => ExecutionResultContractV1::Capability {
            source: binding.math,
            role: ExecutionRoleContractV1::NumericalPolicyMathSource(binding),
        },
        M::Bind { .. } => ExecutionResultContractV1::Capability {
            source: binding.bound,
            role: ExecutionRoleContractV1::NumericalPolicyMathBound(binding),
        },
        M::F32 { .. } => ExecutionResultContractV1::Scalar(ScalarType::F32),
    }]
}

impl FunctionVerifier<'_, '_> {
    pub(super) fn valid_numerical_policy_math_custody(
        &self,
        contract: &ExecutionCapabilityOpV1,
        location: &DiagnosticLocation,
    ) -> bool {
        let Some((block, operation)) = location.block.zip(location.operation) else {
            return false;
        };
        self.policy_math_chain(contract, block, operation, 0)
    }

    // Only three direct issuer edges are admitted. Source moves and shared
    // reborrows must already preserve these ValueIds under checked MIR custody.
    // This check does not discharge the retained LIFETIME_VALIDITY obligation.
    fn policy_math_chain(
        &self,
        contract: &ExecutionCapabilityOpV1,
        block: BlockId,
        index: usize,
        depth: u8,
    ) -> bool {
        if depth > 2 || !contract.is_complete() || contract.provenance.root != self.function.id {
            return false;
        }
        let E::NumericalPolicyMath(math) = contract.operation else {
            return false;
        };
        match math {
            M::MathDerive { .. } => {
                let [context] = contract.operands.as_slice() else {
                    return false;
                };
                self.policy_math_root(*context, contract, block, index)
            }
            M::Bind { binding } => {
                let [math_value, policy_value] = contract.operands.as_slice() else {
                    return false;
                };
                let Some((math, math_block, math_index)) =
                    self.policy_math_issuer(*math_value, block, index)
                else {
                    return false;
                };
                let Some((policy, policy_block, policy_index)) =
                    self.policy_math_issuer(*policy_value, block, index)
                else {
                    return false;
                };
                matches!(math.operation, E::NumericalPolicyMath(M::MathDerive { binding: issued, .. }) if issued == binding)
                    && matches!(policy.operation, E::NumericalPolicyIssue { capability, policy, mode, .. }
                        if capability == binding.capability && policy == binding.policy && mode == binding.mode)
                    && same_custody(math, contract)
                    && same_custody(policy, contract)
                    && math.operands.len() == 1
                    && policy.operands == math.operands
                    && policy.is_complete()
                    && self.policy_math_root(policy.operands[0], policy, policy_block, policy_index)
                    && self.policy_math_chain(math, math_block, math_index, depth + 1)
            }
            M::F32 { binding, .. } => {
                let Some(receiver) = contract.operands.first() else {
                    return false;
                };
                let Some((constructor, constructor_block, constructor_index)) =
                    self.policy_math_issuer(*receiver, block, index)
                else {
                    return false;
                };
                matches!(constructor.operation, E::NumericalPolicyMath(M::Bind { binding: issued }) if issued == binding)
                    && same_custody(constructor, contract)
                    && self.policy_math_chain(
                        constructor,
                        constructor_block,
                        constructor_index,
                        depth + 1,
                    )
            }
        }
    }

    fn policy_math_issuer(
        &self,
        value: ValueId,
        block: BlockId,
        index: usize,
    ) -> Option<(&ExecutionCapabilityOpV1, BlockId, usize)> {
        let definition = self.definitions.get(&value)?;
        let DefSite::Operation(issuer_block, issuer_index) = definition.site else {
            return None;
        };
        if !self.site_dominates(definition.site, block, Some(index)) {
            return None;
        }
        let operation = self.defining_operation(value)?;
        if operation.results.len() != 1 || operation.results[0].id != value {
            return None;
        }
        let OperationKind::ExecutionCapability(contract) = &operation.kind else {
            return None;
        };
        Some((contract, issuer_block, issuer_index))
    }

    fn policy_math_root(
        &self,
        value: ValueId,
        contract: &ExecutionCapabilityOpV1,
        block: BlockId,
        index: usize,
    ) -> bool {
        let Some(definition) = self.definitions.get(&value) else {
            return false;
        };
        let Some(operation) = self.defining_operation(value) else {
            return false;
        };
        self.function.role == FunctionRole::KernelEntry
            && matches!(operation.kind, OperationKind::KernelContextIssue(_))
            && operation.results.len() == 1
            && operation.results[0].id == value
            && self.site_dominates(definition.site, block, Some(index))
            && execution_operand_type_matches(
                Some(&definition.ty),
                ExecutionOperandContractV1::KernelContext,
                contract,
            )
    }
}

fn same_custody(issuer: &ExecutionCapabilityOpV1, consumer: &ExecutionCapabilityOpV1) -> bool {
    issuer.provenance == consumer.provenance
        && issuer.workgroup_brand.is_none()
        && consumer.workgroup_brand.is_none()
        && issuer.epoch_before.is_none()
        && consumer.epoch_before.is_none()
        && issuer.epoch_after.is_none()
        && consumer.epoch_after.is_none()
}
