impl DeterministicScalarProjectorV1<'_> {
    fn guarded_divisor_is_total_v1(
        &mut self,
        site: ScalarAssignmentSiteV1,
        result_type: SemanticTypeIdV1,
        operation: SemanticBinaryOpV1,
        left: &SemanticOperandV1,
        right: &SemanticOperandV1,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        if !matches!(
            operation,
            SemanticBinaryOpV1::Divide | SemanticBinaryOpV1::Remainder
        ) || left.ty() != result_type
            || right.ty() != result_type
            || !self
                .types
                .get(result_type.index() as usize)
                .is_some_and(|ty| {
                    matches!(
                        ty.shape(),
                        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                            signed: false,
                            bits: 8 | 16 | 32 | 64
                        })
                    )
                })
        {
            return Ok(false);
        }
        let Some(statement) = self
            .function
            .blocks()
            .get(site.block)
            .and_then(|block| block.statements().get(site.statement))
        else {
            return Ok(false);
        };
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return Ok(false);
        };
        if !assignment.destination().projections().is_empty()
            || assignment.destination().ty() != result_type
            || assignment.value().result_type() != result_type
            || self
                .function
                .locals()
                .get(assignment.destination().local().index() as usize)
                .is_none_or(|local| local.ty() != result_type)
        {
            return Ok(false);
        }
        let SemanticRvalueKindV1::Binary {
            operation: actual_operation,
            left: actual_left,
            right: actual_right,
        } = assignment.value().kind()
        else {
            return Ok(false);
        };
        if *actual_operation != operation {
            return Ok(false);
        }
        let mut comparison_work = 8_usize;
        for operand in [left, right, actual_left, actual_right] {
            let entries = match operand {
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                    place.projections().len()
                }
                SemanticOperandV1::Constant(constant)
                    if matches!(constant.value(), SemanticConstantValueV1::Scalar(_)) =>
                {
                    0
                }
                SemanticOperandV1::Constant(_) => return Ok(false),
            };
            comparison_work = comparison_work.checked_add(entries).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "guarded divisor operand comparison work overflowed",
                ),
            )?;
        }
        if self.guarded_divisor_proofs.is_none() {
            self.guarded_divisor_proofs =
                Some(SemanticAssertProofsV1::new(self.types, self.function)?);
        }
        let proofs = self.guarded_divisor_proofs.as_mut().ok_or(
            ProductionRankedProjectionErrorV1::Incomplete(
                "guarded divisor proof engine was not initialized",
            ),
        )?;
        proofs.charge(comparison_work)?;
        if actual_left != left || actual_right != right {
            return Ok(false);
        }
        Ok(proofs
            .range_at_operand(actual_right, site.block, site.statement)?
            .is_some_and(|range| range.minimum > 0))
    }
}
