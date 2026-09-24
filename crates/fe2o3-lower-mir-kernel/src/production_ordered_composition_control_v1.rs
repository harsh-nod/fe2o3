//! Closed source-observed Option discriminant control for the safety projection.
//! The canonical CFG remains unchanged. All three actual switch edges participate
//! in acyclicity accounting; the empty unreachable default is not deleted.
use super::*;

impl Dependencies<'_, '_, '_> {
    pub(super) fn boolean_discriminant(
        &mut self,
        function: u32,
        operation: &fe2o3_kernel_ir::Operation,
    ) -> Result<bool, E> {
        let [result] = operation.results.as_slice() else {
            return Ok(false);
        };
        let Op::Cast {
            kind: CastKind::ZeroExtend,
            value: input,
            to: Type::Scalar(ScalarType::I64),
        } = &operation.kind
        else {
            return Ok(false);
        };
        if function != self.owner.root_function_ordinal()
            || result.ty != Type::Scalar(ScalarType::I64)
        {
            return Ok(false);
        }
        let body = self.body(function)?;
        // This is deliberately not admission of general signed constants,
        // arithmetic, parameters or block parameters.
        let mut selected = false;
        let mut bool_input = false;
        for block in &body.blocks {
            self.budget.charge_work(1)?;
            selected |= matches!(
                &block.terminator,
                Some(End::Switch { selector, .. }) if *selector == result.id
            );
            for candidate in &block.operations {
                self.budget.charge_work(1)?;
                if let [value] = candidate.results.as_slice() {
                    bool_input |= value.id == *input && value.ty == Type::BOOL;
                }
            }
        }
        Ok(selected && bool_input)
    }

    pub(super) fn root_successors(
        &mut self,
        block: &fe2o3_kernel_ir::BasicBlock,
    ) -> Result<[Option<BlockId>; 3], E> {
        self.budget.charge_work(1)?;
        match block.terminator.as_ref() {
            Some(End::Branch { target, .. }) => Ok([Some(*target), None, None]),
            Some(End::ConditionalBranch {
                then_target,
                else_target,
                ..
            }) => Ok([Some(*then_target), Some(*else_target), None]),
            Some(End::Return { values }) if values.is_empty() => Ok([None; 3]),
            Some(End::Unreachable)
                if block.parameters.is_empty() && block.operations.is_empty() =>
            {
                Ok([None; 3])
            }
            Some(End::Switch {
                selector,
                cases,
                default_target,
                default_arguments,
            }) => {
                if cases.len() != 2
                    || cases[0].value != 0
                    || cases[1].value != 1
                    || cases[0].target == cases[1].target
                    || cases
                        .iter()
                        .any(|case| case.target == *default_target || !case.arguments.is_empty())
                    || !default_arguments.is_empty()
                {
                    return Err(refusal("unsupported root switch in safety projection"));
                }
                let function = self.owner.root_function_ordinal();
                let body = self.body(function)?;
                let mut default_valid = false;
                let mut selector_valid = false;
                for candidate in &body.blocks {
                    self.budget.charge_work(1)?;
                    if candidate.id == *default_target {
                        default_valid = candidate.parameters.is_empty()
                            && candidate.operations.is_empty()
                            && matches!(candidate.terminator, Some(End::Unreachable));
                    }
                    for operation in &candidate.operations {
                        self.budget.charge_work(1)?;
                        if operation.results.iter().any(|value| value.id == *selector) {
                            selector_valid = self.boolean_discriminant(function, operation)?;
                        }
                    }
                }
                if !default_valid || !selector_valid {
                    return Err(refusal("unsupported root switch in safety projection"));
                }
                Ok([
                    Some(cases[0].target),
                    Some(cases[1].target),
                    Some(*default_target),
                ])
            }
            _ => Err(refusal("unsupported root termination in safety projection")),
        }
    }
}
