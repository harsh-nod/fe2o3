// Relocated semantic rules; metering does not grant assertion admission.
impl<M: SemanticAssertionMeterV1> SemanticAssertionRecipeQueriesV1<'_, '_, M> {
    fn comparison_edge_authenticates_each_dynamic_use_v1(
        &mut self,
        comparison_block: usize,
        success_target: usize,
        use_block: usize,
    ) -> Result<bool, ErrorFor<M>> {
        let block_count = self.core.function.blocks().len();
        if comparison_block >= block_count
            || success_target >= block_count
            || use_block >= block_count
        {
            return Ok(false);
        }
        let mut success_edge = HashSet::new();
        self.paid().set_reserve(&mut success_edge, 1)?;
        self.paid()
            .set_insert(&mut success_edge, (comparison_block, success_target))?;
        if !self.edge_set_dominates(&success_edge, use_block)? {
            return Ok(false);
        }

        let mut visited = Vec::new();
        self.paid().grow(&mut visited, block_count)?;
        self.paid().charge(block_count)?;
        visited.resize(block_count, false);
        let mut pending = self.paid().deque(block_count)?;
        pending.push_back(use_block);
        visited[use_block] = true;
        while let Some(block) = pending.pop_front() {
            self.charge(1)?;
            self.charge(self.core.graph.successors[block].len())?;
            for &successor in &self.core.graph.successors[block] {
                if block == comparison_block && successor == success_target {
                    continue;
                }
                if successor == use_block {
                    return Ok(false);
                }
                if !visited[successor] {
                    visited[successor] = true;
                    pending.push_back(successor);
                }
            }
        }
        Ok(true)
    }

    pub fn same_exact_unsigned_value_v1(
        &mut self,
        left: &SemanticOperandV1,
        left_use: ScalarAssignmentSiteV1,
        right: &SemanticOperandV1,
        right_use: ScalarAssignmentSiteV1,
    ) -> Result<bool, ErrorFor<M>> {
        let Some((left, left_use)) = self.exact_unsigned_value_origin_v1(left, left_use)? else {
            return Ok(false);
        };
        let Some((right, right_use)) = self.exact_unsigned_value_origin_v1(right, right_use)?
        else {
            return Ok(false);
        };
        match (&left, &right) {
            (SemanticOperandV1::Constant(left), SemanticOperandV1::Constant(right)) => {
                let (
                    SemanticConstantValueV1::Scalar(left_value),
                    SemanticConstantValueV1::Scalar(right_value),
                ) = (left.value(), right.value())
                else {
                    return Ok(false);
                };
                Ok(self
                    .scalar_unsigned_maximum(left.ty())
                    .is_some_and(|maximum| left_value.bits() <= maximum)
                    && self
                        .scalar_unsigned_maximum(right.ty())
                        .is_some_and(|maximum| right_value.bits() <= maximum)
                    && left_value.bits() == right_value.bits())
            }
            _ => {
                let (Some(left_local), Some(right_local)) =
                    (simple_operand_local(&left), simple_operand_local(&right))
                else {
                    return Ok(false);
                };
                let local = left_local.index() as usize;
                Ok(left_local == right_local
                    && self.local_has_globally_stable_value_at(local, left_use)?
                    && self.local_has_globally_stable_value_at(local, right_use)?)
            }
        }
    }

    fn exact_unsigned_value_origin_v1(
        &mut self,
        operand: &SemanticOperandV1,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<Option<(SemanticOperandV1, ScalarAssignmentSiteV1)>, ErrorFor<M>> {
        let mut operand = self.paid().clone_operand(operand)?;
        let mut use_site = use_site;
        let mut visited = HashSet::new();
        loop {
            self.charge(1)?;
            match &operand {
                SemanticOperandV1::Constant(constant)
                    if self.scalar_unsigned_maximum(constant.ty()).is_some() =>
                {
                    return Ok(Some((operand, use_site)));
                }
                SemanticOperandV1::Constant(_) => return Ok(None),
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
                    if place.projections().is_empty()
                        && self.scalar_unsigned_maximum(place.ty()).is_some() =>
                {
                    let local = place.local().index() as usize;
                    if !self.paid().set_insert(&mut visited, local)?
                        || self.core.address_escaped.get(local).copied() != Some(false)
                    {
                        return Ok(None);
                    }
                    let Some(site) = self.exact_reaching_assignment_v1(local, use_site)? else {
                        return Ok(self
                            .local_has_globally_stable_value_at(local, use_site)?
                            .then_some((operand, use_site)));
                    };
                    let SemanticStatementKindV1::Assign(assignment) =
                        self.core.function.blocks()[site.block].statements()[site.statement].kind()
                    else {
                        return Ok(None);
                    };
                    let next = match assignment.value().kind() {
                        SemanticRvalueKindV1::Use(next) if next.ty() == place.ty() => next,
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
                        _ => return Ok(Some((operand, use_site))),
                    };
                    operand = self.paid().clone_operand(next)?;
                    use_site = site;
                }
                _ => return Ok(None),
            }
        }
    }

    fn bounded_quotient_product_upper_range_v1(
        &mut self,
        left: &SemanticOperandV1,
        right: &SemanticOperandV1,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<Option<UnsignedRangeProofV1>, ErrorFor<M>> {
        for (quotient, extent) in [(left, right), (right, left)] {
            let Some((quotient_site, numerator, divisor)) =
                self.exact_binary_source_v1(quotient, use_site, SemanticBinaryOpV1::Divide)?
            else {
                continue;
            };
            let Some(bound) =
                self.quotient_strict_product_upper_bound_v1(&numerator, &divisor, quotient_site)?
            else {
                continue;
            };
            if self
                .range_at_operand(extent, use_site.block, use_site.statement)?
                .is_none_or(|range| range.minimum == 0)
            {
                continue;
            }
            let Some(total_maximum) = self.explicit_checked_product_maximum_v1(
                &bound.factor,
                bound.factor_use,
                extent,
                use_site,
                use_site,
                None,
            )?
            else {
                continue;
            };
            // q<factor and extent>0 make q*extent strictly smaller than the
            // authenticated total product factor*extent.
            let Some(maximum) = total_maximum.checked_sub(1) else {
                continue;
            };
            return Ok(Some(UnsignedRangeProofV1 {
                minimum: 0,
                maximum,
            }));
        }
        Ok(None)
    }

    pub fn explicit_checked_product_maximum_v1(
        &mut self,
        left: &SemanticOperandV1,
        left_use: ScalarAssignmentSiteV1,
        right: &SemanticOperandV1,
        right_use: ScalarAssignmentSiteV1,
        use_site: ScalarAssignmentSiteV1,
        required_type: Option<SemanticTypeIdV1>,
    ) -> Result<Option<u128>, ErrorFor<M>> {
        if required_type.is_some_and(|required| left.ty() != required || right.ty() != required) {
            return Ok(None);
        }
        let assignment_count = self.core.assignments.len();
        for local in 0..assignment_count {
            self.charge(1)?;
            if self.core.definition_counts.get(local).copied() != Some(1)
                || self.core.address_escaped.get(local).copied() != Some(false)
            {
                continue;
            }
            let Some(definition) = self.core.assignments[local] else {
                continue;
            };
            let SemanticStatementKindV1::Assign(assignment) =
                self.core.function.blocks()[definition.block].statements()[definition.statement]
                    .kind()
            else {
                continue;
            };
            let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() else {
                continue;
            };
            if checked.operation() != SemanticCheckedBinaryOpV1::Multiply {
                continue;
            }
            if required_type.is_some_and(|required| {
                checked.left().ty() != required || checked.right().ty() != required
            }) {
                continue;
            }
            let operands_match =
                self.same_checked_product_operand_v1(checked.left(), definition, left, left_use)?
                    && self.same_checked_product_operand_v1(
                        checked.right(),
                        definition,
                        right,
                        right_use,
                    )?
                    || self.same_checked_product_operand_v1(
                        checked.left(),
                        definition,
                        right,
                        right_use,
                    )? && self.same_checked_product_operand_v1(
                        checked.right(),
                        definition,
                        left,
                        left_use,
                    )?;
            if !operands_match {
                continue;
            }
            let Some(maximum) = self.scalar_unsigned_maximum(checked.left().ty()) else {
                continue;
            };
            if checked.right().ty() != checked.left().ty() {
                continue;
            }
            if self
                .authenticated_checked_binary_local_value_v1(
                    local,
                    use_site.block,
                    use_site.statement,
                )?
                .is_some()
            {
                return Ok(Some(maximum));
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
                    || zero_target.edge().target() == targets.otherwise().target()
                {
                    continue;
                }
                let switch_use = ScalarAssignmentSiteV1 {
                    block: switch_block,
                    statement: block.statements().len(),
                };
                if self.exact_checked_overflow_flag_source_local_v1(discriminant, switch_use)?
                    != Some(local)
                    || !self.assignment_dominates_use(
                        definition,
                        switch_block,
                        block.statements().len(),
                    )?
                {
                    continue;
                }
                let success_target = zero_target.edge().target().index() as usize;
                let mut success_edge = HashSet::new();
                self.paid().set_reserve(&mut success_edge, 1)?;
                self.paid()
                    .set_insert(&mut success_edge, (switch_block, success_target))?;
                if self.edge_set_dominates(&success_edge, use_site.block)? {
                    return Ok(Some(maximum));
                }
            }
        }
        Ok(None)
    }

    pub fn same_checked_product_operand_v1(
        &mut self,
        left: &SemanticOperandV1,
        left_use: ScalarAssignmentSiteV1,
        right: &SemanticOperandV1,
        right_use: ScalarAssignmentSiteV1,
    ) -> Result<bool, ErrorFor<M>> {
        if self.same_exact_unsigned_value_v1(left, left_use, right, right_use)? {
            return Ok(true);
        }
        if left.ty() != right.ty() || self.unsigned_integer_bits(left.ty()).is_none() {
            return Ok(false);
        }
        let (Some(left_local), Some(right_local)) =
            (simple_operand_local(left), simple_operand_local(right))
        else {
            return Ok(false);
        };
        let local = left_local.index() as usize;
        if left_local != right_local || self.core.address_escaped.get(local).copied() != Some(false)
        {
            return Ok(false);
        }
        Ok(
            self.local_is_stable_from_operand_use_site_v1(local, left_use, right_use)?
                || self.local_is_stable_from_operand_use_site_v1(local, right_use, left_use)?,
        )
    }

    fn local_is_stable_from_operand_use_site_v1(
        &mut self,
        local: usize,
        capture_site: ScalarAssignmentSiteV1,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<bool, ErrorFor<M>> {
        let Some(block) = self.core.function.blocks().get(capture_site.block) else {
            return Ok(false);
        };
        if capture_site.statement > block.statements().len() {
            return Ok(false);
        }
        let capture_redefines_local = if capture_site.statement < block.statements().len() {
            self.block_defines_local_in_statement_range_v1(
                capture_site.block,
                local,
                capture_site.statement,
                capture_site.statement + 1,
            )?
        } else {
            self.block_terminator_defines_local_v1(capture_site.block, local)
        };
        if capture_redefines_local {
            return Ok(false);
        }
        self.local_is_stable_between_sites_v1(local, capture_site, use_site)
    }

    pub fn proves_strictly_bounded_unsigned_product_v1(
        &mut self,
        checked: &SemanticCheckedBinaryRvalueV1,
        product_site: ScalarAssignmentSiteV1,
    ) -> Result<bool, ErrorFor<M>> {
        if checked.operation() != SemanticCheckedBinaryOpV1::Multiply
            || checked.left().ty() != checked.right().ty()
            || self.unsigned_integer_bits(checked.left().ty()).is_none()
        {
            return Ok(false);
        }

        for (bounded, factor) in [
            (checked.left(), checked.right()),
            (checked.right(), checked.left()),
        ] {
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
                let Some(condition_site) = self
                    .exact_reaching_assignment_v1(condition_local.index() as usize, switch_use)?
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
                    left,
                    right,
                } = condition.value().kind()
                else {
                    continue;
                };
                let false_target = zero_target.edge().target().index() as usize;
                let true_target = targets.otherwise().target().index() as usize;
                let (tested, upper, success_target) = match operation {
                    SemanticBinaryOpV1::LessThan => (left, right, true_target),
                    SemanticBinaryOpV1::GreaterOrEqual => (left, right, false_target),
                    SemanticBinaryOpV1::GreaterThan => (right, left, true_target),
                    SemanticBinaryOpV1::LessOrEqual => (right, left, false_target),
                    _ => continue,
                };
                if tested.ty() != upper.ty()
                    || tested.ty() != bounded.ty()
                    || !self.same_exact_unsigned_value_v1(
                        tested,
                        condition_site,
                        bounded,
                        product_site,
                    )?
                {
                    continue;
                }
                let mut success_edge = HashSet::new();
                self.paid().set_reserve(&mut success_edge, 1)?;
                self.paid()
                    .set_insert(&mut success_edge, (switch_block, success_target))?;
                if !self.edge_set_dominates(&success_edge, product_site.block)? {
                    continue;
                }
                // Unsigned multiplication is monotone: on the exact strict
                // edge, bounded < upper, and a successful checked upper*factor
                // therefore proves bounded*factor fits the same scalar width.
                if self
                    .explicit_checked_product_maximum_v1(
                        upper,
                        condition_site,
                        factor,
                        product_site,
                        product_site,
                        Some(checked.left().ty()),
                    )?
                    .is_some()
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub fn proves_strictly_bounded_unsigned_flat_index_v1(
        &mut self,
        checked: &SemanticCheckedBinaryRvalueV1,
        sum_site: ScalarAssignmentSiteV1,
    ) -> Result<bool, ErrorFor<M>> {
        if checked.operation() != SemanticCheckedBinaryOpV1::Add
            || checked.left().ty() != checked.right().ty()
            || self.unsigned_integer_bits(checked.left().ty()).is_none()
        {
            return Ok(false);
        }

        for (product_operand, offset) in [
            (checked.left(), checked.right()),
            (checked.right(), checked.left()),
        ] {
            let Some(product) =
                self.authenticated_checked_binary_source_v1(product_operand, sum_site)?
            else {
                continue;
            };
            if product.checked.operation() != SemanticCheckedBinaryOpV1::Multiply
                || product.checked.left().ty() != checked.left().ty()
                || product.checked.right().ty() != checked.left().ty()
            {
                continue;
            }

            for (index, extent) in [
                (product.checked.left(), product.checked.right()),
                (product.checked.right(), product.checked.left()),
            ] {
                let index_bounds = self.authenticated_strict_unsigned_bounds_v1(
                    index,
                    product.definition,
                    sum_site,
                )?;
                let offset_bounds =
                    self.authenticated_strict_unsigned_bounds_v1(offset, sum_site, sum_site)?;
                for (outer, outer_use) in &index_bounds {
                    for (offset_extent, offset_extent_use) in &offset_bounds {
                        if !self.same_exact_unsigned_value_v1(
                            extent,
                            product.definition,
                            offset_extent,
                            *offset_extent_use,
                        )? {
                            continue;
                        }
                        if self
                            .explicit_checked_product_maximum_v1(
                                outer,
                                *outer_use,
                                extent,
                                product.definition,
                                sum_site,
                                Some(checked.left().ty()),
                            )?
                            .is_some()
                        {
                            // On the two exact strict-success edges, index<outer and
                            // offset<extent. A successful checked outer*extent therefore
                            // bounds index*extent+offset by outer*extent-1.
                            return Ok(true);
                        }
                    }
                }
            }
        }

        if self.proves_strictly_bounded_unsigned_nested_flat_index_v1(checked, sum_site)? {
            return Ok(true);
        }
        Ok(false)
    }

    pub fn proves_strictly_bounded_unsigned_nested_flat_index_v1(
        &mut self,
        checked: &SemanticCheckedBinaryRvalueV1,
        sum_site: ScalarAssignmentSiteV1,
    ) -> Result<bool, ErrorFor<M>> {
        const MAX_NESTED_FLAT_INDEX_OFFSETS_V1: usize = 8;

        if checked.operation() != SemanticCheckedBinaryOpV1::Add
            || checked.left().ty() != checked.right().ty()
            || self.unsigned_integer_bits(checked.left().ty()).is_none()
        {
            return Ok(false);
        }
        for (base_operand, tail_offset) in [
            (checked.left(), checked.right()),
            (checked.right(), checked.left()),
        ] {
            let mut offsets = Vec::new();
            self.paid()
                .grow(&mut offsets, MAX_NESTED_FLAT_INDEX_OFFSETS_V1)?;
            offsets.push((self.paid().clone_operand(tail_offset)?, sum_site));
            if self.proves_strictly_bounded_unsigned_flat_index_path_v1(
                base_operand,
                sum_site,
                sum_site,
                checked.left().ty(),
                &mut offsets,
                MAX_NESTED_FLAT_INDEX_OFFSETS_V1 - 1,
            )? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn proves_strictly_bounded_unsigned_flat_index_path_v1(
        &mut self,
        operand: &SemanticOperandV1,
        operand_use: ScalarAssignmentSiteV1,
        sum_site: ScalarAssignmentSiteV1,
        scalar_type: SemanticTypeIdV1,
        offsets: &mut Vec<(SemanticOperandV1, ScalarAssignmentSiteV1)>,
        remaining_offsets: usize,
    ) -> Result<bool, ErrorFor<M>> {
        let Some(value) = self.authenticated_checked_binary_source_v1(operand, operand_use)? else {
            return Ok(false);
        };
        if value.checked.left().ty() != scalar_type || value.checked.right().ty() != scalar_type {
            return Ok(false);
        }

        if value.checked.operation() == SemanticCheckedBinaryOpV1::Multiply {
            let mut combined_offset_maximum = Some(0_u128);
            for (offset, offset_use) in offsets.iter() {
                combined_offset_maximum = combined_offset_maximum
                    .zip(self.range_at_operand(offset, offset_use.block, offset_use.statement)?)
                    .and_then(|(combined, range)| combined.checked_add(range.maximum));
            }

            for (index, extent) in [
                (value.checked.left(), value.checked.right()),
                (value.checked.right(), value.checked.left()),
            ] {
                let numeric_offset_bound = self
                    .range_at_operand(extent, value.definition.block, value.definition.statement)?
                    .zip(combined_offset_maximum)
                    .is_some_and(|(extent_range, offset_maximum)| {
                        offset_maximum < extent_range.minimum
                    });
                if !numeric_offset_bound
                    && !self.proves_nested_offset_sum_below_product_extent_v1(
                        offsets,
                        extent,
                        value.definition,
                        sum_site,
                        scalar_type,
                    )?
                {
                    continue;
                }
                let index_bounds = self.authenticated_strict_unsigned_bounds_v1(
                    index,
                    value.definition,
                    sum_site,
                )?;
                for (outer, outer_use) in &index_bounds {
                    if self
                        .explicit_checked_product_maximum_v1(
                            outer,
                            *outer_use,
                            extent,
                            value.definition,
                            sum_site,
                            Some(scalar_type),
                        )?
                        .is_some()
                    {
                        // Every peeled add has an authenticated success edge.
                        // The strict index edge and the summed unsigned offset
                        // maxima therefore keep the result below the successful
                        // checked outer*extent witness.
                        return Ok(true);
                    }
                }
            }
            return Ok(false);
        }

        if value.checked.operation() != SemanticCheckedBinaryOpV1::Add || remaining_offsets == 0 {
            return Ok(false);
        }
        for (next, offset) in [
            (value.checked.left(), value.checked.right()),
            (value.checked.right(), value.checked.left()),
        ] {
            self.paid().grow(offsets, 1)?;
            offsets.push((self.paid().clone_operand(offset)?, value.definition));
            let proved = self.proves_strictly_bounded_unsigned_flat_index_path_v1(
                next,
                value.definition,
                sum_site,
                scalar_type,
                offsets,
                remaining_offsets - 1,
            )?;
            offsets.pop();
            if proved {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn proves_nested_offset_sum_below_product_extent_v1(
        &mut self,
        offsets: &[(SemanticOperandV1, ScalarAssignmentSiteV1)],
        extent: &SemanticOperandV1,
        extent_use: ScalarAssignmentSiteV1,
        sum_site: ScalarAssignmentSiteV1,
        scalar_type: SemanticTypeIdV1,
    ) -> Result<bool, ErrorFor<M>> {
        let Some(extent_product) =
            self.authenticated_checked_binary_source_v1(extent, extent_use)?
        else {
            return Ok(false);
        };
        if extent_product.checked.operation() != SemanticCheckedBinaryOpV1::Multiply
            || extent_product.checked.left().ty() != scalar_type
            || extent_product.checked.right().ty() != scalar_type
        {
            return Ok(false);
        }

        for (scaled_offset_index, (scaled_offset, scaled_offset_use)) in offsets.iter().enumerate()
        {
            let Some(offset_product) =
                self.authenticated_checked_binary_source_v1(scaled_offset, *scaled_offset_use)?
            else {
                continue;
            };
            if offset_product.checked.operation() != SemanticCheckedBinaryOpV1::Multiply
                || offset_product.checked.left().ty() != scalar_type
                || offset_product.checked.right().ty() != scalar_type
            {
                continue;
            }

            let mut residual_maximum = Some(0_u128);
            for (offset_index, (offset, offset_use)) in offsets.iter().enumerate() {
                if offset_index == scaled_offset_index {
                    continue;
                }
                residual_maximum = residual_maximum
                    .zip(self.range_at_operand(offset, offset_use.block, offset_use.statement)?)
                    .and_then(|(combined, range)| combined.checked_add(range.maximum));
            }
            let Some(residual_maximum) = residual_maximum else {
                continue;
            };

            for (inner_index, offset_stride) in [
                (
                    offset_product.checked.left(),
                    offset_product.checked.right(),
                ),
                (
                    offset_product.checked.right(),
                    offset_product.checked.left(),
                ),
            ] {
                let Some(stride_range) = self.range_at_operand(
                    offset_stride,
                    offset_product.definition.block,
                    offset_product.definition.statement,
                )?
                else {
                    continue;
                };
                if residual_maximum >= stride_range.minimum {
                    continue;
                }
                let inner_bounds = self.authenticated_strict_unsigned_bounds_v1(
                    inner_index,
                    offset_product.definition,
                    sum_site,
                )?;
                for (inner_upper, extent_stride) in [
                    (
                        extent_product.checked.left(),
                        extent_product.checked.right(),
                    ),
                    (
                        extent_product.checked.right(),
                        extent_product.checked.left(),
                    ),
                ] {
                    if !self.same_checked_product_operand_v1(
                        offset_stride,
                        offset_product.definition,
                        extent_stride,
                        extent_product.definition,
                    )? {
                        continue;
                    }
                    for (bound, bound_use) in &inner_bounds {
                        if self.same_checked_product_operand_v1(
                            bound,
                            *bound_use,
                            inner_upper,
                            extent_product.definition,
                        )? {
                            // inner<inner_upper and residual<stride establish
                            // inner*stride+residual < inner_upper*stride.
                            return Ok(true);
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    fn authenticated_strict_unsigned_bounds_v1(
        &mut self,
        tested_value: &SemanticOperandV1,
        tested_use: ScalarAssignmentSiteV1,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<Vec<(SemanticOperandV1, ScalarAssignmentSiteV1)>, ErrorFor<M>> {
        if self.unsigned_integer_bits(tested_value.ty()).is_none() {
            return Ok(Vec::new());
        }
        let can_reach_use = self.blocks_reaching(use_site.block)?;
        let block_count = self.core.function.blocks().len();
        let mut visited = Vec::new();
        self.paid().grow(&mut visited, block_count)?;
        self.paid().charge(block_count)?;
        visited.resize(block_count, 0_usize);
        let mut pending = self.paid().deque(block_count)?;
        let mut generation = 0_usize;
        let mut bounds = Vec::new();
        self.paid().grow(&mut bounds, block_count)?;

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
            let (tested, upper, success_target) = match operation {
                SemanticBinaryOpV1::LessThan => (left, right, true_target),
                SemanticBinaryOpV1::GreaterOrEqual => (left, right, false_target),
                SemanticBinaryOpV1::GreaterThan => (right, left, true_target),
                SemanticBinaryOpV1::LessOrEqual => (right, left, false_target),
                _ => continue,
            };
            if tested.ty() != tested_value.ty()
                || upper.ty() != tested_value.ty()
                || !self.same_edge_stable_unsigned_value_v1(
                    tested,
                    condition_site,
                    tested_value,
                    tested_use,
                    success_target,
                    use_site,
                    &can_reach_use,
                    &mut visited,
                    &mut pending,
                    &mut generation,
                )?
                || !self.unsigned_operand_stable_from_edge_to_use_v1(
                    upper,
                    condition_site,
                    success_target,
                    use_site,
                    &can_reach_use,
                    &mut visited,
                    &mut pending,
                    &mut generation,
                )?
            {
                continue;
            }
            let mut success_edge = HashSet::new();
            self.paid().set_reserve(&mut success_edge, 1)?;
            self.paid()
                .set_insert(&mut success_edge, (switch_block, success_target))?;
            if self.edge_set_dominates(&success_edge, tested_use.block)?
                && self.edge_set_dominates(&success_edge, use_site.block)?
            {
                bounds.push((self.paid().clone_operand(upper)?, condition_site));
            }
        }
        Ok(bounds)
    }
}
