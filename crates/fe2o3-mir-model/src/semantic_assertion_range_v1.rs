// Relocated semantic rules; metering does not grant assertion admission.
impl<M: SemanticAssertionMeterV1> SemanticAssertionRecipeQueriesV1<'_, '_, M> {
    pub fn range_at_operand(
        &mut self,
        operand: &SemanticOperandV1,
        block: usize,
        statement: usize,
    ) -> Result<Option<UnsignedRangeProofV1>, ErrorFor<M>> {
        if block >= self.core.function.blocks().len()
            || statement > self.core.function.blocks()[block].statements().len()
        {
            return Err(analysis_error::<M>(SemanticAssertionErrorV1::Incomplete(
                "a compiler-derived unsigned cast has a stale semantic use site",
            )));
        }
        self.evaluate_assertion_range_operand_v1(operand, block, statement)
    }

    pub fn scalar_unsigned_maximum(&self, ty: SemanticTypeIdV1) -> Option<u128> {
        self.core.scalar_unsigned_maximum(ty)
    }

    pub fn unsigned_integer_bits(&self, ty: SemanticTypeIdV1) -> Option<u16> {
        self.core.unsigned_integer_bits(ty)
    }

    pub fn literal_unsigned_subtraction_upper_range_v1(
        &self,
        checked: &SemanticCheckedBinaryRvalueV1,
    ) -> Option<UnsignedRangeProofV1> {
        self.core
            .literal_unsigned_subtraction_upper_range_v1(checked)
    }

    fn assertion_range_operand_task_v1(
        &mut self,
        operand: &SemanticOperandV1,
    ) -> MR<AssertionRangeOperandTaskV1, M> {
        Ok(match operand {
            SemanticOperandV1::Constant(constant) => AssertionRangeOperandTaskV1::Constant {
                ty: constant.ty(),
                bits: match constant.value() {
                    SemanticConstantValueV1::Scalar(value) => Some(value.bits()),
                    _ => None,
                },
            },
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                if tuple_field_operand_local_v1(operand, 0).is_some() {
                    AssertionRangeOperandTaskV1::CheckedResult {
                        local: place.local().index() as usize,
                    }
                } else if place.projections().is_empty() {
                    AssertionRangeOperandTaskV1::Local(place.local().index() as usize)
                } else {
                    AssertionRangeOperandTaskV1::ProjectedPlace(self.paid().clone_place(place)?)
                }
            }
        })
    }

    fn assertion_range_expression_task_v1(
        &mut self,
        value: &crate::semantic_mir_v1::SemanticRvalueV1,
    ) -> MR<AssertionRangeExpressionTaskV1, M> {
        let destination_maximum = match value.kind() {
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::BitAnd,
                left,
                right,
            } if left.ty() != value.result_type()
                || right.ty() != value.result_type()
                || !matches!(
                    self.unsigned_integer_bits(value.result_type()),
                    Some(8 | 16 | 32 | 64 | 128)
                ) =>
            {
                None
            }
            _ => self.scalar_unsigned_maximum(value.result_type()),
        };
        Ok(match value.kind() {
            SemanticRvalueKindV1::Use(operand) => AssertionRangeExpressionTaskV1::Operand(
                self.assertion_range_operand_task_v1(operand)?,
            ),
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand,
            } if self
                .unsigned_integer_bits(operand.ty())
                .zip(self.unsigned_integer_bits(value.result_type()))
                .is_some_and(|(source, destination)| destination >= source) =>
            {
                AssertionRangeExpressionTaskV1::Operand(
                    self.assertion_range_operand_task_v1(operand)?,
                )
            }
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            } => AssertionRangeExpressionTaskV1::Binary {
                operation: *operation,
                destination_maximum,
                left: self.assertion_range_operand_task_v1(left)?,
                right: self.assertion_range_operand_task_v1(right)?,
                left_source: self.paid().clone_operand(left)?,
                right_source: self.paid().clone_operand(right)?,
            },
            _ => AssertionRangeExpressionTaskV1::Unsupported,
        })
    }

    fn schedule_assertion_range_operand_v1(
        &mut self,
        frames: &mut Vec<AssertionRangeFrameV1>,
        task: AssertionRangeOperandTaskV1,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<(), ErrorFor<M>> {
        self.paid()
            .push(frames, AssertionRangeFrameV1::Operand { task, use_site })
    }

    fn schedule_assertion_range_expression_v1(
        &mut self,
        frames: &mut Vec<AssertionRangeFrameV1>,
        values: &mut Vec<Option<UnsignedRangeProofV1>>,
        task: AssertionRangeExpressionTaskV1,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<(), ErrorFor<M>> {
        match task {
            AssertionRangeExpressionTaskV1::Operand(operand) => {
                self.schedule_assertion_range_operand_v1(frames, operand, use_site)
            }
            AssertionRangeExpressionTaskV1::Binary {
                operation,
                destination_maximum,
                left,
                right,
                left_source,
                right_source,
            } => {
                self.paid().push(
                    frames,
                    AssertionRangeFrameV1::FinishBinary {
                        operation,
                        destination_maximum,
                        left_source,
                        right_source,
                        use_site,
                    },
                )?;
                self.schedule_assertion_range_operand_v1(frames, right, use_site)?;
                self.schedule_assertion_range_operand_v1(frames, left, use_site)
            }
            AssertionRangeExpressionTaskV1::Unsupported => self.paid().push(values, None),
        }
    }

    fn schedule_assertion_local_narrowing_v1(
        &mut self,
        frames: &mut Vec<AssertionRangeFrameV1>,
        local: usize,
        use_block: usize,
        maximum: u128,
        result: Option<UnsignedRangeProofV1>,
    ) -> Result<(), ErrorFor<M>> {
        // A live safe-Rust unsigned scalar always inhabits its declared bit
        // width even when exact single-definition reconstruction is
        // unavailable (for example, a loop-carried induction). This fallback
        // is intentionally no stronger than the type invariant.
        let result = result.or(Some(UnsignedRangeProofV1 {
            minimum: 0,
            maximum,
        }));
        let zero_is_excluded = self.zero_excluding_edge_dominates(local, use_block)?;
        let result = match (result, zero_is_excluded) {
            (Some(mut range), true) => {
                range.minimum = range.minimum.max(1);
                Some(range)
            }
            (None, true) => Some(UnsignedRangeProofV1 {
                minimum: 1,
                maximum,
            }),
            (result, false) => result,
        };
        let block_count = self.core.function.blocks().len();
        let can_reach_use = self.blocks_reaching(use_block)?;
        let mut stability_visited = Vec::new();
        self.paid().grow(&mut stability_visited, block_count)?;
        self.paid().charge(block_count)?;
        stability_visited.resize(block_count, 0_usize);
        let stability_pending = self.paid().deque(block_count)?;
        self.paid().push(
            frames,
            AssertionRangeFrameV1::ContinueStrictUpperBound(AssertionStrictUpperBoundStateV1 {
                local,
                use_block,
                next_switch_block: 0,
                range: result,
                can_reach_use,
                stability_visited,
                stability_pending,
                stability_generation: 0,
                proven_upper_bound: None,
            }),
        )
    }

    fn evaluate_assertion_range_operand_v1(
        &mut self,
        operand: &SemanticOperandV1,
        use_block: usize,
        use_statement: usize,
    ) -> Result<Option<UnsignedRangeProofV1>, ErrorFor<M>> {
        let mut frames = Vec::new();
        let mut values = Vec::new();
        let mut visiting = HashSet::new();
        let task = self.assertion_range_operand_task_v1(operand)?;
        self.schedule_assertion_range_operand_v1(
            &mut frames,
            task,
            ScalarAssignmentSiteV1 {
                block: use_block,
                statement: use_statement,
            },
        )?;

        while let Some(frame) = frames.pop() {
            match frame {
                AssertionRangeFrameV1::Operand { task, use_site } => {
                    self.charge(1)?;
                    match task {
                        AssertionRangeOperandTaskV1::Constant { ty, bits } => {
                            let Some(maximum) = self.scalar_unsigned_maximum(ty) else {
                                self.paid().push(&mut values, None)?;
                                continue;
                            };
                            let Some(bits) = bits else {
                                self.paid().push(&mut values, None)?;
                                continue;
                            };
                            let value = (bits <= maximum)
                                .then_some(UnsignedRangeProofV1::exact(bits))
                                .ok_or(analysis_error::<M>(
                                    SemanticAssertionErrorV1::Unsupported(
                                        "an unsigned scalar constant exceeds its semantic type",
                                    ),
                                ))?;
                            self.paid().push(&mut values, Some(value))?;
                        }
                        AssertionRangeOperandTaskV1::ProjectedPlace(place) => {
                            self.charge(1)?;
                            let local = place.local().index() as usize;
                            if self.core.address_escaped.get(local).copied() != Some(false)
                                || self.paid().set_contains(&visiting, &local)?
                            {
                                self.paid().push(&mut values, None)?;
                                continue;
                            }
                            let Some(site) = self.exact_reaching_assignment_v1(local, use_site)?
                            else {
                                self.paid().push(&mut values, None)?;
                                continue;
                            };
                            let SemanticStatementKindV1::Assign(assignment) =
                                self.core.function.blocks()[site.block].statements()
                                    [site.statement]
                                    .kind()
                            else {
                                self.paid().push(&mut values, None)?;
                                continue;
                            };
                            let projected = match place.projections() {
                                [downcast, field] => {
                                    let (
                                        SemanticProjectionKindV1::Downcast(projected_variant),
                                        SemanticProjectionKindV1::Field(projected_field),
                                        SemanticRvalueKindV1::Aggregate(aggregate),
                                    ) = (downcast.kind(), field.kind(), assignment.value().kind())
                                    else {
                                        self.paid().push(&mut values, None)?;
                                        continue;
                                    };
                                    let SemanticAggregateKindV1::EnumVariant(actual_variant) =
                                        aggregate.kind()
                                    else {
                                        self.paid().push(&mut values, None)?;
                                        continue;
                                    };
                                    if projected_variant != *actual_variant {
                                        self.paid().push(&mut values, None)?;
                                        continue;
                                    }
                                    aggregate.operands().get(projected_field as usize)
                                }
                                [field] => {
                                    let (
                                        SemanticProjectionKindV1::Field(projected_field),
                                        SemanticRvalueKindV1::Aggregate(aggregate),
                                    ) = (field.kind(), assignment.value().kind())
                                    else {
                                        self.paid().push(&mut values, None)?;
                                        continue;
                                    };
                                    if !matches!(
                                        aggregate.kind(),
                                        SemanticAggregateKindV1::Array
                                            | SemanticAggregateKindV1::Tuple
                                            | SemanticAggregateKindV1::Aggregate
                                    ) {
                                        self.paid().push(&mut values, None)?;
                                        continue;
                                    }
                                    aggregate.operands().get(projected_field as usize)
                                }
                                _ => None,
                            };
                            let Some(projected) =
                                projected.filter(|value| value.ty() == place.ty())
                            else {
                                self.paid().push(&mut values, None)?;
                                continue;
                            };
                            self.paid().set_reserve(&mut visiting, 1)?;
                            self.paid().set_insert(&mut visiting, local)?;
                            self.paid().push(
                                &mut frames,
                                AssertionRangeFrameV1::FinishProjectedPlace { local },
                            )?;
                            let task = self.assertion_range_operand_task_v1(projected)?;

                            self.schedule_assertion_range_operand_v1(&mut frames, task, site)?;
                        }
                        AssertionRangeOperandTaskV1::Local(local) => {
                            self.charge(1)?;
                            let (ty, is_argument) = self.core.function
                                .locals()
                                .get(local)
                                .map(|declaration| {
                                    (
                                        declaration.ty(),
                                        matches!(
                                            declaration.role(),
                                            SemanticLocalRoleV1::Argument(_)
                                        ),
                                    )
                                })
                                .ok_or(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                                    "an assertion proof local is outside the semantic local table",
                                )))?;
                            let Some(maximum) = self.scalar_unsigned_maximum(ty) else {
                                self.paid().push(&mut values, None)?;
                                continue;
                            };
                            if self.core.address_escaped.get(local).copied() != Some(false)
                                || self.paid().set_contains(&visiting, &local)?
                            {
                                self.paid().push(&mut values, None)?;
                                continue;
                            }
                            self.paid().set_reserve(&mut visiting, 1)?;
                            self.paid().set_insert(&mut visiting, local)?;
                            match self.core.definition_counts.get(local).copied() {
                                Some(0) if is_argument => {
                                    let minimum = if self
                                        .zero_excluding_edge_dominates(local, use_site.block)?
                                    {
                                        1
                                    } else {
                                        0
                                    };
                                    self.schedule_assertion_local_narrowing_v1(
                                        &mut frames,
                                        local,
                                        use_site.block,
                                        maximum,
                                        Some(UnsignedRangeProofV1 { minimum, maximum }),
                                    )?;
                                }
                                Some(1) => {
                                    let Some(site) =
                                        self.core.assignments.get(local).copied().flatten()
                                    else {
                                        self.schedule_assertion_local_narrowing_v1(
                                            &mut frames,
                                            local,
                                            use_site.block,
                                            maximum,
                                            None,
                                        )?;
                                        continue;
                                    };
                                    if !self.assignment_dominates_use(
                                        site,
                                        use_site.block,
                                        use_site.statement,
                                    )? {
                                        self.schedule_assertion_local_narrowing_v1(
                                            &mut frames,
                                            local,
                                            use_site.block,
                                            maximum,
                                            None,
                                        )?;
                                        continue;
                                    }
                                    let SemanticStatementKindV1::Assign(assignment) =
                                        self.core.function.blocks()[site.block].statements()
                                            [site.statement]
                                            .kind()
                                    else {
                                        return Err(analysis_error::<M>(
                                            SemanticAssertionErrorV1::Unsupported(
                                                "an indexed assertion proof assignment changed semantic kind",
                                            ),
                                        ));
                                    };
                                    let expression = self
                                        .assertion_range_expression_task_v1(assignment.value())?;
                                    self.paid().push(
                                        &mut frames,
                                        AssertionRangeFrameV1::FinishLocalDefinition {
                                            local,
                                            use_block: use_site.block,
                                            maximum,
                                        },
                                    )?;
                                    self.schedule_assertion_range_expression_v1(
                                        &mut frames,
                                        &mut values,
                                        expression,
                                        site,
                                    )?;
                                }
                                Some(_) | None => {
                                    self.schedule_assertion_local_narrowing_v1(
                                        &mut frames,
                                        local,
                                        use_site.block,
                                        maximum,
                                        None,
                                    )?;
                                }
                            }
                        }
                        AssertionRangeOperandTaskV1::CheckedResult { local } => {
                            self.charge(1)?;
                            if self.core.definition_counts.get(local).copied() != Some(1)
                                || self.core.address_escaped.get(local).copied() != Some(false)
                                || self.paid().set_contains(&visiting, &local)?
                            {
                                self.paid().push(&mut values, None)?;
                                continue;
                            }
                            self.paid().set_reserve(&mut visiting, 1)?;
                            self.paid().set_insert(&mut visiting, local)?;
                            let Some(site) = self.core.assignments.get(local).copied().flatten()
                            else {
                                self.paid().set_remove(&mut visiting, &local)?;
                                self.paid().push(&mut values, None)?;
                                continue;
                            };
                            if !self.assignment_dominates_use(
                                site,
                                use_site.block,
                                use_site.statement,
                            )? {
                                self.paid().set_remove(&mut visiting, &local)?;
                                self.paid().push(&mut values, None)?;
                                continue;
                            }
                            let SemanticStatementKindV1::Assign(assignment) =
                                self.core.function.blocks()[site.block].statements()
                                    [site.statement]
                                    .kind()
                            else {
                                self.paid().set_remove(&mut visiting, &local)?;
                                self.paid().push(&mut values, None)?;
                                continue;
                            };
                            let SemanticRvalueKindV1::CheckedBinary(checked) =
                                assignment.value().kind()
                            else {
                                self.paid().set_remove(&mut visiting, &local)?;
                                self.paid().push(&mut values, None)?;
                                continue;
                            };
                            let operation = match checked.operation() {
                                SemanticCheckedBinaryOpV1::Add => SemanticBinaryOpV1::Add,
                                SemanticCheckedBinaryOpV1::Subtract => SemanticBinaryOpV1::Subtract,
                                SemanticCheckedBinaryOpV1::Multiply => SemanticBinaryOpV1::Multiply,
                            };
                            let expression = AssertionRangeExpressionTaskV1::Binary {
                                operation,
                                destination_maximum: self
                                    .scalar_unsigned_maximum(checked.left().ty()),
                                left: self.assertion_range_operand_task_v1(checked.left())?,
                                right: self.assertion_range_operand_task_v1(checked.right())?,
                                left_source: self.paid().clone_operand(checked.left())?,
                                right_source: self.paid().clone_operand(checked.right())?,
                            };
                            let authenticated = self
                                .authenticated_checked_binary_local_value_v1(
                                    local,
                                    use_site.block,
                                    use_site.statement,
                                )?
                                .is_some();
                            let relational_range = match checked.operation() {
                                SemanticCheckedBinaryOpV1::Subtract if authenticated => {
                                    self.scaled_quotient_remainder_upper_range_v1(
                                        checked.left(),
                                        checked.right(),
                                        site,
                                    )?
                                    .or_else(|| {
                                        // On the exact checked-success edge, unsigned K-rhs
                                        // lies in [0,K], even when independent ranges overlap.
                                        self.literal_unsigned_subtraction_upper_range_v1(checked)
                                    })
                                }
                                SemanticCheckedBinaryOpV1::Multiply if authenticated => self
                                    .bounded_quotient_product_upper_range_v1(
                                        checked.left(),
                                        checked.right(),
                                        site,
                                    )?,
                                _ => None,
                            };
                            self.paid().push(
                                &mut frames,
                                AssertionRangeFrameV1::FinishCheckedResult {
                                    local,
                                    relational_range,
                                },
                            )?;
                            self.schedule_assertion_range_expression_v1(
                                &mut frames,
                                &mut values,
                                expression,
                                site,
                            )?;
                        }
                    }
                }
                AssertionRangeFrameV1::FinishBinary {
                    operation,
                    destination_maximum,
                    left_source,
                    right_source,
                    use_site,
                } => {
                    let right = pop_assertion_range_value_v1::<M>(&mut values)?;
                    let left = pop_assertion_range_value_v1::<M>(&mut values)?;
                    let mut result =
                        Self::range_of_binary(operation, left, right, destination_maximum)?;
                    if operation == SemanticBinaryOpV1::Divide
                        && let Some(bound) = self.quotient_strict_product_upper_bound_v1(
                            &left_source,
                            &right_source,
                            use_site,
                        )?
                    {
                        let upper_bound = bound.maximum;
                        result = Some(match result {
                            Some(mut range) => {
                                range.maximum = range.maximum.min(upper_bound);
                                range
                            }
                            None => UnsignedRangeProofV1 {
                                minimum: 0,
                                maximum: upper_bound,
                            },
                        });
                    }
                    self.paid().push(&mut values, result)?;
                }
                AssertionRangeFrameV1::FinishCheckedResult {
                    local,
                    relational_range,
                } => {
                    let result = pop_assertion_range_value_v1::<M>(&mut values)?;
                    self.paid().set_remove(&mut visiting, &local)?;
                    let result = match (result, relational_range) {
                        (Some(left), Some(right)) => {
                            let minimum = left.minimum.max(right.minimum);
                            let maximum = left.maximum.min(right.maximum);
                            (minimum <= maximum)
                                .then_some(UnsignedRangeProofV1 { minimum, maximum })
                        }
                        (Some(range), None) | (None, Some(range)) => Some(range),
                        (None, None) => None,
                    };
                    self.paid().push(&mut values, result)?;
                }
                AssertionRangeFrameV1::FinishProjectedPlace { local } => {
                    self.paid().set_remove(&mut visiting, &local)?;
                }
                AssertionRangeFrameV1::FinishLocalDefinition {
                    local,
                    use_block,
                    maximum,
                } => {
                    let result = pop_assertion_range_value_v1::<M>(&mut values)?;
                    self.schedule_assertion_local_narrowing_v1(
                        &mut frames,
                        local,
                        use_block,
                        maximum,
                        result,
                    )?;
                }
                AssertionRangeFrameV1::ContinueStrictUpperBound(mut state) => {
                    let mut scheduled = None;
                    while state.next_switch_block < self.core.function.blocks().len() {
                        let switch_block = state.next_switch_block;
                        state.next_switch_block += 1;
                        self.charge(1)?;
                        let Some((condition_local, false_target, true_target, statement_count)) =
                            (|| {
                                let block = &self.core.function.blocks()[switch_block];
                                let SemanticTerminatorKindV1::SwitchInt {
                                    discriminant,
                                    targets,
                                } = block.terminator().kind()
                                else {
                                    return None;
                                };
                                if targets.values().len() != 1 || targets.values()[0].value() != 0 {
                                    return None;
                                }
                                let false_target =
                                    targets.values()[0].edge().target().index() as usize;
                                let true_target = targets.otherwise().target().index() as usize;
                                if false_target == true_target {
                                    return None;
                                }
                                Some((
                                    simple_operand_local(discriminant)?.index() as usize,
                                    false_target,
                                    true_target,
                                    block.statements().len(),
                                ))
                            })()
                        else {
                            continue;
                        };
                        if self.core.definition_counts.get(condition_local).copied() != Some(1)
                            || self.core.address_escaped.get(condition_local).copied()
                                != Some(false)
                        {
                            continue;
                        }
                        let Some(site) = self
                            .core
                            .assignments
                            .get(condition_local)
                            .copied()
                            .flatten()
                        else {
                            continue;
                        };
                        if site.block != switch_block
                            || !self.assignment_dominates_use(
                                site,
                                switch_block,
                                statement_count,
                            )?
                        {
                            continue;
                        }
                        let Some((left_local, right, success_target)) = (|| {
                            let SemanticStatementKindV1::Assign(assignment) =
                                self.core.function.blocks()[site.block].statements()
                                    [site.statement]
                                    .kind()
                            else {
                                return None;
                            };
                            let SemanticRvalueKindV1::Binary {
                                operation,
                                left,
                                right,
                            } = assignment.value().kind()
                            else {
                                return None;
                            };
                            let success_target = match operation {
                                SemanticBinaryOpV1::LessThan => true_target,
                                SemanticBinaryOpV1::GreaterOrEqual => false_target,
                                _ => return None,
                            };
                            Some((
                                simple_operand_local(left)?.index() as usize,
                                right,
                                success_target,
                            ))
                        })(
                        ) else {
                            continue;
                        };
                        let mut capture = site;
                        if !self.local_is_value_preserving_alias_of(
                            left_local,
                            state.local,
                            site.block,
                            site.statement,
                            Some(&mut capture),
                        )? {
                            continue;
                        };
                        if capture.block != switch_block || capture.statement > statement_count {
                            continue;
                        }
                        // Earlier definitions precede the compared value. Any write from
                        // its exact alias capture through the guard still invalidates it.
                        self.charge(statement_count - capture.statement)?;
                        if self.block_defines_local_in_statement_range_v1(
                            switch_block,
                            state.local,
                            capture.statement,
                            statement_count,
                        )? {
                            continue;
                        }
                        let right = self.assertion_range_operand_task_v1(right)?;
                        scheduled = Some((right, site, switch_block, success_target));
                        break;
                    }
                    if let Some((task, site, switch_block, success_target)) = scheduled {
                        self.paid().push(
                            &mut frames,
                            AssertionRangeFrameV1::ApplyStrictUpperBoundCandidate {
                                state,
                                switch_block,
                                success_target,
                            },
                        )?;
                        self.schedule_assertion_range_operand_v1(&mut frames, task, site)?;
                    } else {
                        if let Some(upper_bound) = state.proven_upper_bound {
                            state.range = Some(match state.range {
                                Some(mut range) => {
                                    range.maximum = range.maximum.min(upper_bound);
                                    range
                                }
                                None => UnsignedRangeProofV1 {
                                    minimum: 0,
                                    maximum: upper_bound,
                                },
                            });
                            if state
                                .range
                                .is_some_and(|range| range.minimum > range.maximum)
                            {
                                state.range = None;
                            }
                        }
                        self.paid().set_remove(&mut visiting, &state.local)?;
                        self.paid().push(&mut values, state.range)?;
                    }
                }
                AssertionRangeFrameV1::ApplyStrictUpperBoundCandidate {
                    mut state,
                    switch_block,
                    success_target,
                } => {
                    let bound = pop_assertion_range_value_v1::<M>(&mut values)?;
                    if let Some(candidate) = bound.and_then(|bound| bound.maximum.checked_sub(1)) {
                        state.stability_generation = state
                            .stability_generation
                            .checked_add(1)
                            .ok_or(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                                "assertion proof upper-bound stability generation overflowed",
                            )))?;
                        let mut edge = HashSet::new();
                        self.paid().set_reserve(&mut edge, 1)?;
                        self.paid()
                            .set_insert(&mut edge, (switch_block, success_target))?;
                        if self.edge_set_dominates(&edge, state.use_block)?
                            && self.local_is_stable_from_revalidating_edge_to_use(
                                state.local,
                                switch_block,
                                success_target,
                                state.use_block,
                                &state.can_reach_use,
                                &mut state.stability_visited,
                                &mut state.stability_pending,
                                state.stability_generation,
                            )?
                        {
                            state.proven_upper_bound = Some(
                                state
                                    .proven_upper_bound
                                    .map_or(candidate, |current| current.min(candidate)),
                            );
                        }
                    }
                    self.paid().push(
                        &mut frames,
                        AssertionRangeFrameV1::ContinueStrictUpperBound(state),
                    )?;
                }
            }
        }

        if !visiting.is_empty() || values.len() != 1 {
            return Err(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
                "assertion range evaluator did not finish with one exact result",
            )));
        }
        pop_assertion_range_value_v1::<M>(&mut values)
    }

    pub fn range_of_binary(
        operation: SemanticBinaryOpV1,
        left: Option<UnsignedRangeProofV1>,
        right: Option<UnsignedRangeProofV1>,
        destination_maximum: Option<u128>,
    ) -> Result<Option<UnsignedRangeProofV1>, ErrorFor<M>> {
        let (Some(left), Some(right)) = (left, right) else {
            return Ok(None);
        };
        let range = match operation {
            SemanticBinaryOpV1::BitAnd => {
                let Some(maximum) = destination_maximum else {
                    return Ok(None);
                };
                if ![
                    u128::from(u8::MAX),
                    u128::from(u16::MAX),
                    u128::from(u32::MAX),
                    u128::from(u64::MAX),
                    u128::MAX,
                ]
                .contains(&maximum)
                    || left.minimum > left.maximum
                    || right.minimum > right.maximum
                    || left.maximum > maximum
                    || right.maximum > maximum
                {
                    return Ok(None);
                }
                // Unsigned bitwise intersection cannot exceed either operand.
                Some(UnsignedRangeProofV1 {
                    minimum: 0,
                    maximum: left.maximum.min(right.maximum),
                })
            }
            SemanticBinaryOpV1::Add => {
                let (Some(minimum), Some(maximum), Some(destination_maximum)) = (
                    left.minimum.checked_add(right.minimum),
                    left.maximum.checked_add(right.maximum),
                    destination_maximum,
                ) else {
                    return Ok(None);
                };
                (maximum <= destination_maximum)
                    .then_some(UnsignedRangeProofV1 { minimum, maximum })
            }
            SemanticBinaryOpV1::Multiply => {
                let (Some(minimum), Some(maximum), Some(destination_maximum)) = (
                    left.minimum.checked_mul(right.minimum),
                    left.maximum.checked_mul(right.maximum),
                    destination_maximum,
                ) else {
                    return Ok(None);
                };
                (maximum <= destination_maximum)
                    .then_some(UnsignedRangeProofV1 { minimum, maximum })
            }
            SemanticBinaryOpV1::Subtract if left.minimum >= right.maximum => {
                Some(UnsignedRangeProofV1 {
                    minimum: left.minimum - right.maximum,
                    maximum: left.maximum - right.minimum,
                })
            }
            SemanticBinaryOpV1::Divide if right.minimum != 0 => Some(UnsignedRangeProofV1 {
                minimum: left.minimum / right.maximum,
                maximum: left.maximum / right.minimum,
            }),
            SemanticBinaryOpV1::Remainder if right.minimum != 0 => Some(UnsignedRangeProofV1 {
                minimum: 0,
                maximum: left.maximum.min(right.maximum - 1),
            }),
            SemanticBinaryOpV1::Equal => Some(
                if left.maximum < right.minimum || right.maximum < left.minimum {
                    UnsignedRangeProofV1::exact(0)
                } else if left.minimum == left.maximum && left == right {
                    UnsignedRangeProofV1::exact(1)
                } else {
                    UnsignedRangeProofV1 {
                        minimum: 0,
                        maximum: 1,
                    }
                },
            ),
            SemanticBinaryOpV1::NotEqual => Some(
                if left.maximum < right.minimum || right.maximum < left.minimum {
                    UnsignedRangeProofV1::exact(1)
                } else if left.minimum == left.maximum && left == right {
                    UnsignedRangeProofV1::exact(0)
                } else {
                    UnsignedRangeProofV1 {
                        minimum: 0,
                        maximum: 1,
                    }
                },
            ),
            SemanticBinaryOpV1::LessThan => Some(if left.maximum < right.minimum {
                UnsignedRangeProofV1::exact(1)
            } else if left.minimum >= right.maximum {
                UnsignedRangeProofV1::exact(0)
            } else {
                UnsignedRangeProofV1 {
                    minimum: 0,
                    maximum: 1,
                }
            }),
            SemanticBinaryOpV1::LessOrEqual => Some(if left.maximum <= right.minimum {
                UnsignedRangeProofV1::exact(1)
            } else if left.minimum > right.maximum {
                UnsignedRangeProofV1::exact(0)
            } else {
                UnsignedRangeProofV1 {
                    minimum: 0,
                    maximum: 1,
                }
            }),
            SemanticBinaryOpV1::GreaterThan => Some(if left.minimum > right.maximum {
                UnsignedRangeProofV1::exact(1)
            } else if left.maximum <= right.minimum {
                UnsignedRangeProofV1::exact(0)
            } else {
                UnsignedRangeProofV1 {
                    minimum: 0,
                    maximum: 1,
                }
            }),
            SemanticBinaryOpV1::GreaterOrEqual => Some(if left.minimum >= right.maximum {
                UnsignedRangeProofV1::exact(1)
            } else if left.maximum < right.minimum {
                UnsignedRangeProofV1::exact(0)
            } else {
                UnsignedRangeProofV1 {
                    minimum: 0,
                    maximum: 1,
                }
            }),
            _ => None,
        };
        Ok(range)
    }
}
