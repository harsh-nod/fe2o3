fn execution_assert_operand_v29(
    message: &SemanticAssertMessageV1,
    index: u32,
) -> Option<&SemanticOperandV1> {
    let (first, second) = match message {
        SemanticAssertMessageV1::BoundsCheck { length, index } => (length, Some(index)),
        SemanticAssertMessageV1::Overflow { left, right, .. } => (left, Some(right)),
        SemanticAssertMessageV1::DivisionByZero(operand)
        | SemanticAssertMessageV1::RemainderByZero(operand) => (operand, None),
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment,
            found_alignment,
        } => (required_alignment, Some(found_alignment)),
        SemanticAssertMessageV1::NullPointerDereference
        | SemanticAssertMessageV1::ResumedAfterReturn
        | SemanticAssertMessageV1::ResumedAfterPanic => return None,
    };
    match index {
        0 => Some(first),
        1 => second,
        _ => None,
    }
}

impl SemanticFunctionLoweringV1<'_> {
    fn lower_rvalue_operand_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        index: u32,
        operand: &SemanticOperandV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.lower_source_operand_v29(
            block,
            statement,
            Some(ExecutionOperandV29::RvalueOperand(index)),
            operand,
            operations,
        )
    }

    fn consume_execution_assert_operand_v29(
        &mut self,
        block: SemanticBlockIdV1,
        role: ExecutionOperandV29,
        operand: &SemanticOperandV1,
        operations: &mut Vec<Operation>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
            return Ok(());
        };
        if !self.execution_cfg_local_v29(place.local().index() as usize) {
            return Ok(());
        }
        self.with_emission_budget_v1(|this, budget| {
            this.execution.as_ref().unwrap().check_ledger(budget)?;
            budget.charge_work(argument_sum_v1(&[place.projections().len(), 4])?)?;
            if place.projections().len() > MAX_SSA_VALUE_COMPONENTS_V1 {
                return Err(execution_availability_error_v29());
            }
            Ok(())
        })?;
        // A folded condition or discarded panic payload still has source move
        // effects. Only logical scalar fields can be read without emitting a
        // memory access or granting a nominal capability another consumer.
        if place
            .projections()
            .iter()
            .any(|projection| !matches!(projection.kind(), SemanticProjectionKindV1::Field(_)))
            || !matches!(
                self.types
                    .get(place.ty().index() as usize)
                    .map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
            )
        {
            return Err(execution_availability_error_v29());
        }
        let value = self.lower_source_operand_v29(block, None, Some(role), operand, operations)?;
        if !matches!(
            value,
            SemanticValueBindingV1::Value {
                ty: Type::Scalar(_),
                ..
            }
        ) {
            return Err(execution_availability_error_v29());
        }
        Ok(())
    }

    fn consume_execution_assert_message_v29(
        &mut self,
        block: SemanticBlockIdV1,
        message: &SemanticAssertMessageV1,
        operations: &mut Vec<Operation>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        for index in 0..2 {
            if let Some(operand) = execution_assert_operand_v29(message, index) {
                self.consume_execution_assert_operand_v29(
                    block,
                    ExecutionOperandV29::AssertMessage(index),
                    operand,
                    operations,
                )?;
            }
        }
        Ok(())
    }
}
