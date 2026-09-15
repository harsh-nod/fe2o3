use super::*;
use crate::{Operation, OperationKind, Type, ValueDef};

impl ReusablePhaseOpV1 {
    /// Builds only a locally consistent operation from exact current KIR
    /// bindings. It grants no source authority. The production caller must
    /// consume its private checked source/SSA plan, then verify the whole graph.
    /// No caller-supplied result type or pre-existing result ID is accepted.
    pub fn checked_operation(self, inputs: &[(ValueId, &Type)], first_result: ValueId)
        -> Option<(Operation, ValueId)> {
        if inputs.len() != self.operands.len()
            || inputs.len() > crate::MAX_EXECUTION_CAPABILITY_OPERANDS_V1
            || inputs.iter().zip(&self.operands).any(|((id, _), expected)| id != expected || id.0 >= first_result.0) {
            return None;
        }
        let (_, result_count) = self.operation.arity();
        if result_count > crate::MAX_EXECUTION_CAPABILITY_RESULTS_V1 { return None; }
        let next = first_result.0.checked_add(u32::try_from(result_count).ok()?)?;
        let mut input_types = Vec::new();
        input_types.try_reserve_exact(inputs.len()).ok()?;
        input_types.extend(inputs.iter().map(|(_, ty)| *ty));
        let output = self.checked_result_types(&input_types)?;
        let mut results = Vec::new();
        results.try_reserve_exact(output.len()).ok()?;
        for (slot, ty) in output.into_iter().enumerate() {
            results.push(ValueDef::new(ValueId(first_result.0.checked_add(slot as u32)?), ty));
        }
        Some((Operation::new(results, OperationKind::ReusablePhase(self)), ValueId(next)))
    }
}
