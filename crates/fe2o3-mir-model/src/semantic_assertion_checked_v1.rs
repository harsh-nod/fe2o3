// Relocated semantic rules; metering does not grant assertion admission.
impl<M: SemanticAssertionMeterV1> SemanticAssertionRecipeQueriesV1<'_, '_, M> {
    fn proves_literal_shift_assert_v1(
        &mut self,
        condition: &SemanticOperandV1,
        expected: bool,
        message: &SemanticAssertMessageV1,
        block: usize,
    ) -> Result<bool, ErrorFor<M>> {
        if !expected {
            return Ok(false);
        }
        let SemanticAssertMessageV1::Overflow {
            operation,
            left: message_left,
            right: message_right,
        } = message
        else {
            return Ok(false);
        };
        if !matches!(
            operation,
            SemanticBinaryOpV1::ShiftLeft | SemanticBinaryOpV1::ShiftRight
        ) {
            return Ok(false);
        }
        let Some(bit_width) = self
            .core
            .types
            .get(message_left.ty().index() as usize)
            .and_then(|ty| match ty.shape() {
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { bits, .. }) => {
                    Some(*bits)
                }
                _ => None,
            })
        else {
            return Ok(false);
        };
        let Some(message_shift_bits) = self
            .core
            .types
            .get(message_right.ty().index() as usize)
            .and_then(|ty| match ty.shape() {
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: true,
                    bits,
                }) => Some(*bits),
                _ => None,
            })
        else {
            return Ok(false);
        };
        let SemanticOperandV1::Constant(message_shift) = message_right else {
            return Ok(false);
        };
        let SemanticConstantValueV1::Scalar(message_shift) = message_shift.value() else {
            return Ok(false);
        };
        if message_shift.bits() >= u128::from(bit_width) {
            return Ok(false);
        }

        let Some(condition_local) = simple_operand_local(condition) else {
            return Ok(false);
        };
        let Some(statement_count) = self
            .core
            .function
            .blocks()
            .get(block)
            .map(|block| block.statements().len())
        else {
            return Ok(false);
        };
        let Some(condition_site) = self.exact_reaching_assignment_v1(
            condition_local.index() as usize,
            ScalarAssignmentSiteV1 {
                block,
                statement: statement_count,
            },
        )?
        else {
            return Ok(false);
        };
        if condition_site.block != block {
            return Ok(false);
        }
        let condition_statement =
            &self.core.function.blocks()[block].statements()[condition_site.statement];
        let SemanticStatementKindV1::Assign(condition_assignment) = condition_statement.kind()
        else {
            return Ok(false);
        };
        if !condition_assignment.destination().projections().is_empty()
            || condition_assignment.destination().local() != condition_local
        {
            return Ok(false);
        }
        let SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left: compared_shift,
            right: compared_width,
        } = condition_assignment.value().kind()
        else {
            return Ok(false);
        };
        let Some(cast_local) = simple_operand_local(compared_shift) else {
            return Ok(false);
        };
        let SemanticOperandV1::Constant(compared_width) = compared_width else {
            return Ok(false);
        };
        let SemanticConstantValueV1::Scalar(compared_width_value) = compared_width.value() else {
            return Ok(false);
        };
        let exact_rustc_shift_check_type = compared_shift.ty() == compared_width.ty()
            && matches!(
                self.core.types
                    .get(compared_shift.ty().index() as usize)
                    .map(|ty| ty.shape()),
                Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits,
                })) if *bits == message_shift_bits
            );
        if compared_width_value.bits() != u128::from(bit_width) || !exact_rustc_shift_check_type {
            return Ok(false);
        }

        let Some(cast_site) =
            self.exact_reaching_assignment_v1(cast_local.index() as usize, condition_site)?
        else {
            return Ok(false);
        };
        if cast_site.block != block || cast_site.statement >= condition_site.statement {
            return Ok(false);
        }
        let cast_statement = &self.core.function.blocks()[block].statements()[cast_site.statement];
        let SemanticStatementKindV1::Assign(cast_assignment) = cast_statement.kind() else {
            return Ok(false);
        };
        if !cast_assignment.destination().projections().is_empty()
            || cast_assignment.destination().local() != cast_local
            || cast_assignment.value().result_type() != compared_shift.ty()
        {
            return Ok(false);
        }
        Ok(matches!(
            cast_assignment.value().kind(),
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand,
            } if self.same_operand(operand, message_right)?
        ))
    }

    fn proves_checked_overflow_assert_v1(
        &mut self,
        condition: &SemanticOperandV1,
        expected: bool,
        message: &SemanticAssertMessageV1,
        block: usize,
    ) -> Result<bool, ErrorFor<M>> {
        if expected {
            return Ok(false);
        }
        let SemanticAssertMessageV1::Overflow {
            operation,
            left: message_left,
            right: message_right,
        } = message
        else {
            return Ok(false);
        };
        let Some(result_local) = tuple_field_operand_local_v1(condition, 1) else {
            return Ok(false);
        };
        let local = result_local.index() as usize;
        if self.core.definition_counts.get(local).copied() != Some(1)
            || self.core.address_escaped.get(local).copied() != Some(false)
        {
            return Ok(false);
        }
        let Some(site) = self.core.assignments.get(local).copied().flatten() else {
            return Ok(false);
        };
        if !self.assignment_dominates_use(
            site,
            block,
            self.core.function.blocks()[block].statements().len(),
        )? {
            return Ok(false);
        }
        let SemanticStatementKindV1::Assign(assignment) =
            self.core.function.blocks()[site.block].statements()[site.statement].kind()
        else {
            return Ok(false);
        };
        let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() else {
            return Ok(false);
        };
        let checked_operation = match checked.operation() {
            SemanticCheckedBinaryOpV1::Add => SemanticBinaryOpV1::Add,
            SemanticCheckedBinaryOpV1::Subtract => SemanticBinaryOpV1::Subtract,
            SemanticCheckedBinaryOpV1::Multiply => SemanticBinaryOpV1::Multiply,
        };
        if *operation != checked_operation
            || !self.same_operand(message_left, checked.left())?
            || !self.same_operand(message_right, checked.right())?
        {
            return Ok(false);
        }
        if checked.operation() == SemanticCheckedBinaryOpV1::Subtract {
            if self.proves_scaled_quotient_remainder_nonnegative_v1(
                checked.left(),
                checked.right(),
                site,
                block,
            )? {
                return Ok(true);
            }
            if self.proves_authenticated_unsigned_lower_bound_subtraction_v1(checked, site)? {
                return Ok(true);
            }
        }
        if checked.operation() == SemanticCheckedBinaryOpV1::Multiply
            && self.proves_strictly_bounded_unsigned_product_v1(checked, site)?
        {
            return Ok(true);
        }
        if checked.operation() == SemanticCheckedBinaryOpV1::Add
            && self.proves_strictly_bounded_unsigned_flat_index_v1(checked, site)?
        {
            return Ok(true);
        }
        let left = self.range_at_operand(checked.left(), site.block, site.statement)?;
        let right = self.range_at_operand(checked.right(), site.block, site.statement)?;
        let maximum = self.scalar_unsigned_maximum(checked.left().ty());
        Ok(Self::range_of_binary(checked_operation, left, right, maximum)?.is_some())
    }

    pub fn authenticated_checked_binary_value_v1(
        &mut self,
        operand: &SemanticOperandV1,
        use_block: usize,
        use_statement: usize,
    ) -> Result<Option<AuthenticatedCheckedBinaryValueV1>, ErrorFor<M>> {
        let Some(result_local) = tuple_field_operand_local_v1(operand, 0) else {
            return Ok(None);
        };
        let local = result_local.index() as usize;
        let value =
            self.authenticated_checked_binary_local_value_v1(local, use_block, use_statement)?;
        Ok(value.filter(|value| {
            value.checked.left().ty() == operand.ty() && value.checked.right().ty() == operand.ty()
        }))
    }

    fn authenticated_checked_binary_local_value_v1(
        &mut self,
        local: usize,
        use_block: usize,
        use_statement: usize,
    ) -> Result<Option<AuthenticatedCheckedBinaryValueV1>, ErrorFor<M>> {
        if self.core.definition_counts.get(local).copied() != Some(1)
            || self.core.address_escaped.get(local).copied() != Some(false)
        {
            return Ok(None);
        }
        let Some(definition) = self.core.assignments.get(local).copied().flatten() else {
            return Ok(None);
        };
        if !self.assignment_dominates_use(definition, use_block, use_statement)? {
            return Ok(None);
        }
        let SemanticStatementKindV1::Assign(assignment) =
            self.core.function.blocks()[definition.block].statements()[definition.statement].kind()
        else {
            return Ok(None);
        };
        let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() else {
            return Ok(None);
        };
        let checked_operation = match checked.operation() {
            SemanticCheckedBinaryOpV1::Add => SemanticBinaryOpV1::Add,
            SemanticCheckedBinaryOpV1::Subtract => SemanticBinaryOpV1::Subtract,
            SemanticCheckedBinaryOpV1::Multiply => SemanticBinaryOpV1::Multiply,
        };
        let assertion_count = self
            .core
            .checked_assertion_blocks
            .get(local)
            .map(Vec::len)
            .ok_or(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                "a checked arithmetic result is outside the assertion table",
            )))?;
        for assertion_index in 0..assertion_count {
            self.charge(1)?;
            let assertion_block = self.core.checked_assertion_blocks[local][assertion_index];
            let SemanticTerminatorKindV1::Assert {
                condition,
                expected,
                message,
                target,
                unwind,
            } = self.core.function.blocks()[assertion_block]
                .terminator()
                .kind()
            else {
                unreachable!("indexed checked arithmetic assertion changed kind")
            };
            let exact_message = matches!(
                message,
                SemanticAssertMessageV1::Overflow { operation, left, right }
                    if *operation == checked_operation
                        && self.same_operand(left, checked.left())?
                        && self.same_operand(right, checked.right())?
            );
            if tuple_field_operand_local_v1(condition, 1).map(|result| result.index() as usize)
                != Some(local)
                || *expected
                || !exact_message
                || target.role() != SemanticEdgeRoleV1::AssertSuccess
                || !matches!(unwind, SemanticUnwindActionV1::Unreachable)
            {
                continue;
            }
            if !self.assignment_dominates_use(
                definition,
                assertion_block,
                self.core.function.blocks()[assertion_block]
                    .statements()
                    .len(),
            )? {
                continue;
            }
            let success = target.target().index() as usize;
            let assertion_dominates =
                assertion_block != use_block && self.block_dominates(assertion_block, use_block)?;
            let success_dominates =
                success == use_block || self.block_dominates(success, use_block)?;
            if assertion_dominates && success_dominates {
                return Ok(Some(AuthenticatedCheckedBinaryValueV1 {
                    local,
                    definition,
                    assertion_block,
                    checked: self.paid().clone_checked(checked)?,
                }));
            }
        }
        Ok(None)
    }

    fn authenticated_checked_binary_source_v1(
        &mut self,
        operand: &SemanticOperandV1,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<Option<AuthenticatedCheckedBinaryValueV1>, ErrorFor<M>> {
        let mut operand = self.paid().clone_operand(operand)?;
        let mut use_site = use_site;
        let mut visited = HashSet::new();
        loop {
            self.charge(1)?;
            if tuple_field_operand_local_v1(&operand, 0).is_some()
                && let Some(value) = self.authenticated_checked_binary_value_v1(
                    &operand,
                    use_site.block,
                    use_site.statement,
                )?
            {
                return Ok(Some(value));
            }
            let Some(place) = raw_operand_place(&operand) else {
                return Ok(None);
            };
            if !place.projections().is_empty() {
                return Ok(None);
            }
            let local = place.local().index() as usize;
            if !self.paid().set_insert(&mut visited, local)? {
                return Ok(None);
            }
            let Some(site) = self.exact_reaching_assignment_v1(local, use_site)? else {
                return Ok(None);
            };
            let SemanticStatementKindV1::Assign(assignment) =
                self.core.function.blocks()[site.block].statements()[site.statement].kind()
            else {
                return Ok(None);
            };
            let next = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(next) => next,
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Integer,
                    operand: next,
                } if self
                    .unsigned_integer_bits(assignment.value().result_type())
                    .zip(self.unsigned_integer_bits(next.ty()))
                    .is_some_and(|(destination, source)| destination >= source) =>
                {
                    next
                }
                _ => return Ok(None),
            };
            operand = self.paid().clone_operand(next)?;
            use_site = site;
        }
    }

    fn exact_binary_source_v1(
        &mut self,
        operand: &SemanticOperandV1,
        use_site: ScalarAssignmentSiteV1,
        expected_operation: SemanticBinaryOpV1,
    ) -> Result<Option<(ScalarAssignmentSiteV1, SemanticOperandV1, SemanticOperandV1)>, ErrorFor<M>>
    {
        let mut operand = self.paid().clone_operand(operand)?;
        let mut use_site = use_site;
        let mut visited = HashSet::new();
        loop {
            self.charge(1)?;
            let Some(place) = raw_operand_place(&operand) else {
                return Ok(None);
            };
            let local = place.local().index() as usize;
            if !self.paid().set_insert(&mut visited, local)? {
                return Ok(None);
            }
            let Some(site) = self.exact_reaching_assignment_v1(local, use_site)? else {
                return Ok(None);
            };
            let SemanticStatementKindV1::Assign(assignment) =
                self.core.function.blocks()[site.block].statements()[site.statement].kind()
            else {
                return Ok(None);
            };
            if place.projections().is_empty() {
                match assignment.value().kind() {
                    SemanticRvalueKindV1::Binary {
                        operation,
                        left,
                        right,
                    } if *operation == expected_operation => {
                        return Ok(Some((
                            site,
                            self.paid().clone_operand(left)?,
                            self.paid().clone_operand(right)?,
                        )));
                    }
                    SemanticRvalueKindV1::Use(next) if next.ty() == place.ty() => {
                        operand = self.paid().clone_operand(next)?;
                        use_site = site;
                        continue;
                    }
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Integer,
                        operand: next,
                    } if self
                        .unsigned_integer_bits(assignment.value().result_type())
                        .zip(self.unsigned_integer_bits(next.ty()))
                        .is_some_and(|(destination, source)| destination >= source) =>
                    {
                        operand = self.paid().clone_operand(next)?;
                        use_site = site;
                        continue;
                    }
                    _ => return Ok(None),
                }
            }
            let projected = match place.projections() {
                [downcast, field] => {
                    let (
                        SemanticProjectionKindV1::Downcast(projected_variant),
                        SemanticProjectionKindV1::Field(projected_field),
                        SemanticRvalueKindV1::Aggregate(aggregate),
                    ) = (downcast.kind(), field.kind(), assignment.value().kind())
                    else {
                        return Ok(None);
                    };
                    let SemanticAggregateKindV1::EnumVariant(actual_variant) = aggregate.kind()
                    else {
                        return Ok(None);
                    };
                    if projected_variant != *actual_variant {
                        return Ok(None);
                    }
                    aggregate.operands().get(projected_field as usize)
                }
                [field] => {
                    let (
                        SemanticProjectionKindV1::Field(projected_field),
                        SemanticRvalueKindV1::Aggregate(aggregate),
                    ) = (field.kind(), assignment.value().kind())
                    else {
                        return Ok(None);
                    };
                    if !matches!(
                        aggregate.kind(),
                        SemanticAggregateKindV1::Array
                            | SemanticAggregateKindV1::Tuple
                            | SemanticAggregateKindV1::Aggregate
                    ) {
                        return Ok(None);
                    }
                    aggregate.operands().get(projected_field as usize)
                }
                _ => None,
            };
            let Some(projected) = projected.filter(|projected| projected.ty() == place.ty()) else {
                return Ok(None);
            };
            operand = self.paid().clone_operand(projected)?;
            use_site = site;
        }
    }

    pub fn authenticated_scaled_quotient_remainder_v1(
        &mut self,
        left: &SemanticOperandV1,
        right: &SemanticOperandV1,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<Option<AuthenticatedScaledQuotientRemainderV1>, ErrorFor<M>> {
        let Some(sum) = self.authenticated_checked_binary_source_v1(left, use_site)? else {
            return Ok(None);
        };
        let Some(head_extent) = self.authenticated_checked_binary_source_v1(right, use_site)?
        else {
            return Ok(None);
        };
        if sum.checked.operation() != SemanticCheckedBinaryOpV1::Add
            || head_extent.checked.operation() != SemanticCheckedBinaryOpV1::Multiply
        {
            return Ok(None);
        }

        for (scaled_operand, offset) in [
            (sum.checked.left(), sum.checked.right()),
            (sum.checked.right(), sum.checked.left()),
        ] {
            let Some(scaled) =
                self.authenticated_checked_binary_source_v1(scaled_operand, sum.definition)?
            else {
                continue;
            };
            if scaled.checked.operation() != SemanticCheckedBinaryOpV1::Multiply {
                continue;
            }
            for (quotient_operand, extent) in [
                (head_extent.checked.left(), head_extent.checked.right()),
                (head_extent.checked.right(), head_extent.checked.left()),
            ] {
                let Some((quotient_site, quotient_numerator, quotient_divisor)) = self
                    .exact_binary_source_v1(
                        quotient_operand,
                        head_extent.definition,
                        SemanticBinaryOpV1::Divide,
                    )?
                else {
                    continue;
                };
                for (scaled_numerator, scale) in [
                    (scaled.checked.left(), scaled.checked.right()),
                    (scaled.checked.right(), scaled.checked.left()),
                ] {
                    if !self.same_operand(scaled_numerator, &quotient_numerator)? {
                        continue;
                    }
                    let Some((divisor_site, divisor_extent, divisor_scale)) = self
                        .exact_binary_source_v1(
                            &quotient_divisor,
                            quotient_site,
                            SemanticBinaryOpV1::Divide,
                        )?
                    else {
                        continue;
                    };
                    if !self.same_operand(&divisor_extent, extent)?
                        || !self.same_operand(&divisor_scale, scale)?
                        || !self.operand_has_globally_stable_value_at_v1(
                            &quotient_numerator,
                            quotient_site,
                        )?
                        || !self.operand_has_globally_stable_value_at_v1(extent, divisor_site)?
                        || !self.operand_has_globally_stable_value_at_v1(scale, divisor_site)?
                        || !self.operand_has_globally_stable_value_at_v1(
                            &quotient_divisor,
                            quotient_site,
                        )?
                    {
                        continue;
                    }
                    return Ok(Some(AuthenticatedScaledQuotientRemainderV1 {
                        divisor: quotient_divisor,
                        divisor_use: quotient_site,
                        extent: self.paid().clone_operand(extent)?,
                        extent_use: head_extent.definition,
                        scale: self.paid().clone_operand(scale)?,
                        scale_use: scaled.definition,
                        offset: self.paid().clone_operand(offset)?,
                        offset_use: sum.definition,
                    }));
                }
            }
        }
        Ok(None)
    }

    pub fn scaled_quotient_remainder_upper_range_v1(
        &mut self,
        left: &SemanticOperandV1,
        right: &SemanticOperandV1,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<Option<UnsignedRangeProofV1>, ErrorFor<M>> {
        let Some(remainder) =
            self.authenticated_scaled_quotient_remainder_v1(left, right, use_site)?
        else {
            return Ok(None);
        };
        let Some(scale) = self.range_at_operand(
            &remainder.scale,
            remainder.scale_use.block,
            remainder.scale_use.statement,
        )?
        else {
            return Ok(None);
        };
        let Some(offset) = self.range_at_operand(
            &remainder.offset,
            remainder.offset_use.block,
            remainder.offset_use.statement,
        )?
        else {
            return Ok(None);
        };
        let Some(divisor) = self.range_at_operand(
            &remainder.divisor,
            remainder.divisor_use.block,
            remainder.divisor_use.statement,
        )?
        else {
            return Ok(None);
        };
        let Some(extent) = self.range_at_operand(
            &remainder.extent,
            remainder.extent_use.block,
            remainder.extent_use.statement,
        )?
        else {
            return Ok(None);
        };
        if scale.minimum == 0
            || scale.minimum != scale.maximum
            || divisor.minimum == 0
            || offset.maximum >= scale.minimum
        {
            return Ok(None);
        }
        // For d=floor(extent/scale), q=floor(x/d), and offset<scale, the
        // authenticated subtraction result is at most (d-1)*scale+offset.
        Ok(extent
            .maximum
            .checked_sub(1)
            .map(|maximum| UnsignedRangeProofV1 {
                minimum: 0,
                maximum,
            }))
    }

    pub fn proves_scaled_quotient_remainder_nonnegative_v1(
        &mut self,
        left: &SemanticOperandV1,
        right: &SemanticOperandV1,
        use_site: ScalarAssignmentSiteV1,
        use_block: usize,
    ) -> Result<bool, ErrorFor<M>> {
        let Some(remainder) =
            self.authenticated_scaled_quotient_remainder_v1(left, right, use_site)?
        else {
            return Ok(false);
        };
        if self
            .range_at_operand(
                &remainder.divisor,
                remainder.divisor_use.block,
                remainder.divisor_use.statement,
            )?
            .is_none_or(|range| range.minimum == 0)
            || self
                .range_at_operand(
                    &remainder.scale,
                    remainder.scale_use.block,
                    remainder.scale_use.statement,
                )?
                .is_none_or(|range| range.minimum == 0)
        {
            return Ok(false);
        }

        for (switch_block, block) in self.core.function.blocks().iter().enumerate() {
            self.charge(1)?;
            let SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } = block.terminator().kind()
            else {
                continue;
            };
            let [zero_target] = targets.values() else {
                continue;
            };
            if zero_target.value() != 0
                || zero_target.edge().role() != SemanticEdgeRoleV1::SwitchValue
                || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
            {
                continue;
            }
            let false_target = zero_target.edge().target().index() as usize;
            let true_target = targets.otherwise().target().index() as usize;
            if false_target == true_target {
                continue;
            }
            let Some(condition_local) = simple_operand_local(discriminant) else {
                continue;
            };
            let condition_use = ScalarAssignmentSiteV1 {
                block: switch_block,
                statement: block.statements().len(),
            };
            let Some(condition_site) =
                self.exact_reaching_assignment_v1(condition_local.index() as usize, condition_use)?
            else {
                continue;
            };
            let SemanticStatementKindV1::Assign(condition) = self.core.function.blocks()
                [condition_site.block]
                .statements()[condition_site.statement]
                .kind()
            else {
                continue;
            };
            let SemanticRvalueKindV1::Binary {
                operation,
                left: condition_left,
                right: condition_right,
            } = condition.value().kind()
            else {
                continue;
            };
            let (tested_remainder, success_target) = match operation {
                SemanticBinaryOpV1::Equal
                    if self.operand_is_exact_unsigned_zero(condition_right) =>
                {
                    (condition_left, true_target)
                }
                SemanticBinaryOpV1::Equal
                    if self.operand_is_exact_unsigned_zero(condition_left) =>
                {
                    (condition_right, true_target)
                }
                SemanticBinaryOpV1::NotEqual
                    if self.operand_is_exact_unsigned_zero(condition_right) =>
                {
                    (condition_left, false_target)
                }
                SemanticBinaryOpV1::NotEqual
                    if self.operand_is_exact_unsigned_zero(condition_left) =>
                {
                    (condition_right, false_target)
                }
                _ => continue,
            };
            let Some((remainder_site, guarded_extent, guarded_scale)) = self
                .exact_binary_source_v1(
                    tested_remainder,
                    condition_site,
                    SemanticBinaryOpV1::Remainder,
                )?
            else {
                continue;
            };
            if !self.same_exact_unsigned_value_v1(
                &guarded_extent,
                remainder_site,
                &remainder.extent,
                remainder.extent_use,
            )? || !self.same_exact_unsigned_value_v1(
                &guarded_scale,
                remainder_site,
                &remainder.scale,
                remainder.scale_use,
            )? {
                continue;
            }
            // The exact remainder-zero edge makes extent=d*scale, so
            // q*d<=x implies q*extent<=x*scale<=x*scale+offset.
            let mut success_edge = HashSet::new();
            self.paid().set_reserve(&mut success_edge, 1)?;
            self.paid()
                .set_insert(&mut success_edge, (switch_block, success_target))?;
            if self.edge_set_dominates(&success_edge, use_block)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn proves_authenticated_unsigned_lower_bound_subtraction_v1(
        &mut self,
        checked: &SemanticCheckedBinaryRvalueV1,
        subtraction_site: ScalarAssignmentSiteV1,
    ) -> Result<bool, ErrorFor<M>> {
        if checked.operation() != SemanticCheckedBinaryOpV1::Subtract
            || checked.left().ty() != checked.right().ty()
            || self.unsigned_integer_bits(checked.left().ty()).is_none()
        {
            return Ok(false);
        }

        let block_count = self.core.function.blocks().len();
        let can_reach_use = self.blocks_reaching(subtraction_site.block)?;
        let mut stability_visited = Vec::new();
        self.paid().grow(&mut stability_visited, block_count)?;
        self.paid().charge(block_count)?;
        stability_visited.resize(block_count, 0_usize);
        let mut stability_pending = self.paid().deque(block_count)?;
        let mut stability_generation = 0_usize;

        for (switch_block, block) in self.core.function.blocks().iter().enumerate() {
            self.charge(1)?;
            let SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } = block.terminator().kind()
            else {
                continue;
            };
            let [zero_target] = targets.values() else {
                continue;
            };
            if zero_target.value() != 0
                || zero_target.edge().role() != SemanticEdgeRoleV1::SwitchValue
                || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
                || zero_target.edge().target() == targets.otherwise().target()
            {
                continue;
            }
            let switch_use = ScalarAssignmentSiteV1 {
                block: switch_block,
                statement: block.statements().len(),
            };
            let Some(condition_local) = simple_operand_local(discriminant) else {
                continue;
            };
            let Some(condition_site) =
                self.exact_reaching_assignment_v1(condition_local.index() as usize, switch_use)?
            else {
                continue;
            };
            if condition_site.block != switch_block {
                continue;
            }
            let SemanticStatementKindV1::Assign(condition) =
                block.statements()[condition_site.statement].kind()
            else {
                continue;
            };
            let SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            } = condition.value().kind()
            else {
                continue;
            };
            let false_target = zero_target.edge().target().index() as usize;
            let true_target = targets.otherwise().target().index() as usize;
            let (guarded_left, guarded_right, success_target) = match operation {
                SemanticBinaryOpV1::GreaterOrEqual => (left, right, true_target),
                SemanticBinaryOpV1::LessThan => (left, right, false_target),
                SemanticBinaryOpV1::LessOrEqual => (right, left, true_target),
                SemanticBinaryOpV1::GreaterThan => (right, left, false_target),
                _ => continue,
            };
            if guarded_left.ty() != checked.left().ty()
                || guarded_right.ty() != checked.right().ty()
                || !self.same_edge_stable_unsigned_subtraction_value_v1(
                    guarded_left,
                    condition_site,
                    checked.left(),
                    success_target,
                    subtraction_site,
                    &can_reach_use,
                    &mut stability_visited,
                    &mut stability_pending,
                    &mut stability_generation,
                )?
                || !self.same_edge_stable_unsigned_subtraction_value_v1(
                    guarded_right,
                    condition_site,
                    checked.right(),
                    success_target,
                    subtraction_site,
                    &can_reach_use,
                    &mut stability_visited,
                    &mut stability_pending,
                    &mut stability_generation,
                )?
            {
                continue;
            }
            if self.comparison_edge_authenticates_each_dynamic_use_v1(
                switch_block,
                success_target,
                subtraction_site.block,
            )? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[allow(clippy::too_many_arguments)]
    fn same_edge_stable_unsigned_subtraction_value_v1(
        &mut self,
        compared: &SemanticOperandV1,
        comparison_site: ScalarAssignmentSiteV1,
        expected: &SemanticOperandV1,
        edge_target: usize,
        use_site: ScalarAssignmentSiteV1,
        can_reach_use: &[bool],
        visited: &mut [usize],
        pending: &mut VecDeque<usize>,
        generation: &mut usize,
    ) -> Result<bool, ErrorFor<M>> {
        if compared.ty() != expected.ty() || self.unsigned_integer_bits(compared.ty()).is_none() {
            return Ok(false);
        }
        if let (Some(compared), Some(expected)) = (
            simple_operand_local(compared),
            simple_operand_local(expected),
        ) && compared == expected
        {
            let local = compared.index() as usize;
            let Some(comparison_block) = self.core.function.blocks().get(comparison_site.block)
            else {
                return Ok(false);
            };
            if comparison_site.statement >= comparison_block.statements().len()
                || self.block_defines_local_in_statement_range_v1(
                    comparison_site.block,
                    local,
                    comparison_site.statement,
                    comparison_site.statement + 1,
                )?
            {
                return Ok(false);
            }
            return self.local_stable_after_comparison_and_edge_to_use_v1(
                local,
                comparison_site,
                edge_target,
                use_site,
                can_reach_use,
                visited,
                pending,
                generation,
            );
        }
        self.same_edge_stable_unsigned_value_v1(
            compared,
            comparison_site,
            expected,
            use_site,
            edge_target,
            use_site,
            can_reach_use,
            visited,
            pending,
            generation,
        )
    }
}
