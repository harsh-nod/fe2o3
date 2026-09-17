// Statement recipes join source operands to their exact native operation spans.
impl<'a, 'r> SourceLocalCursorV1<'a, 'r> {
    fn bind_destination(
        &mut self,
        block: SemanticBlockIdV1,
        statement: u32,
        place: &SemanticPlaceV1,
        input: Option<usize>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        use fe2o3_pliron::{
            ProductionSemanticSsaEventRoleV1 as E, ProductionSemanticSsaOccurrenceSiteV1 as S,
            ProductionSemanticSsaOperandRoleV1 as R,
        };
        let site = S::Statement {
            block: fe2o3_mir_model::SsaBlockIdV1::new(block.index()),
            statement,
        };
        if !place.projections().is_empty() {
            self.source_use(site, R::Destination, E::BaseUse, place.local(), budget)?;
            return Ok(input);
        }
        let variable = fe2o3_mir_model::SsaVariableIdV1::new(place.local().index());
        let (_, resolved) = self.claim_source_event(
            site,
            R::Destination,
            E::DestinationDefine,
            fe2o3_mir_model::SsaEventV1::Define(variable),
            budget,
        )?;
        budget.charge_work(4)?;
        let source_ssa = match resolved {
            Some(fe2o3_mir_model::SsaResolvedEventV1::Define {
                variable: actual,
                value,
            }) if actual == variable => Some(value),
            None => None,
            _ => return Err(unit_local_mismatch_v1()),
        };
        let value = if let Some(input) = input {
            let row = *self.value(input).ok_or_else(unit_local_mismatch_v1)?;
            if row.source_type != place.ty() {
                return Err(unit_local_mismatch_v1());
            }
            Some(self.rows.append_value(
                UnitLocalValueRowV1 {
                    key: self.input.key,
                    origin: UnitLocalValueOriginV1::Assignment {
                        block,
                        statement,
                        local: place.local(),
                    },
                    role: Some(R::Destination),
                    source_type: place.ty(),
                    source_ssa,
                    native: row.native,
                    recipe: UnitLocalValueRecipeV1::Copy { input },
                    known_bits: row.known_bits,
                },
                budget,
            )?)
        } else {
            if source_ssa.is_some() {
                return Err(unit_local_mismatch_v1());
            }
            None
        };
        self.locals
            .get_mut(place.local().index() as usize)
            .ok_or_else(unit_local_mismatch_v1)?
            .value = value;
        Ok(value)
    }

    fn check_rvalue(
        &mut self,
        block: SemanticBlockIdV1,
        statement: u32,
        local: SemanticLocalIdV1,
        value: &fe2o3_mir_model::semantic_mir_v1::SemanticRvalueV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        use fe2o3_pliron::{
            ProductionSemanticSsaEventRoleV1 as E, ProductionSemanticSsaOccurrenceSiteV1 as S,
            ProductionSemanticSsaOperandRoleV1 as R,
        };
        let site = S::Statement {
            block: fe2o3_mir_model::SsaBlockIdV1::new(block.index()),
            statement,
        };
        let origin = UnitLocalValueOriginV1::Assignment {
            block,
            statement,
            local,
        };
        budget.charge_work(4)?;
        let (native, recipe, known_bits) = match value.kind() {
            SemanticRvalueKindV1::Use(operand) => {
                let input = self.resolve_operand(
                    site,
                    R::RvalueOperand(0),
                    operand,
                    UnitLocalOperandUseV1::Next,
                    budget,
                )?;
                let row = self.value(input).ok_or_else(unit_local_mismatch_v1)?;
                if row.source_type != value.result_type() {
                    return Err(unit_local_mismatch_v1());
                }
                return Ok(input);
            }
            SemanticRvalueKindV1::Load(load) => {
                if load.volatility() != SemanticVolatilityV1::NonVolatile
                    || load.atomic().is_some()
                    || load.source().ty() != value.result_type()
                {
                    return Err(self.error("local helper volatile or atomic Load is unsupported"));
                }
                let (origin, source_ssa) = self.source_use(
                    site,
                    R::RvaluePlace,
                    E::BaseUse,
                    load.source().local(),
                    budget,
                )?;
                return self.read_memory_place(
                    site,
                    R::RvaluePlace,
                    load.source(),
                    origin,
                    source_ssa,
                    UnitLocalOperandUseV1::Next,
                    budget,
                );
            }
            SemanticRvalueKindV1::Cast { kind, operand } => {
                if *kind != SemanticCastKindV1::Integer {
                    return Err(self.error("local helper cast kind is unsupported"));
                }
                let input = self.resolve_operand(
                    site,
                    R::RvalueOperand(0),
                    operand,
                    UnitLocalOperandUseV1::Next,
                    budget,
                )?;
                let row = *self.value(input).ok_or_else(unit_local_mismatch_v1)?;
                let to = self.source_scalar(value.result_type(), budget)?;
                let UnitLocalNativeValueV1::Scalar {
                    value: mut current,
                    scalar: from,
                    mut definition,
                } = row.native
                else {
                    return Err(unit_local_mismatch_v1());
                };
                let path = lower_cast_path(*kind, &Type::Scalar(from), &Type::Scalar(to))
                    .ok_or_else(unit_local_mismatch_v1)?;
                budget.charge_work(6)?;
                for (cast, target) in path.into_iter().flatten() {
                    let (op, at) = self.take_native_operation(budget)?;
                    budget.charge_work(4)?;
                    if !matches!(&op.kind, OperationKind::Cast { kind, value, to } if *kind == cast && *value == current && *to == Type::Scalar(target))
                        || op.results.len() != 1
                        || op.results[0].ty != Type::Scalar(target)
                    {
                        return Err(unit_local_mismatch_v1());
                    }
                    current = op.results[0].id;
                    definition = Some(at);
                }
                let width = to.bit_width().ok_or_else(unit_local_mismatch_v1)?;
                let mask = if width == 64 {
                    u64::MAX
                } else {
                    (1_u64 << width) - 1
                };
                (
                    UnitLocalNativeValueV1::Scalar {
                        value: current,
                        scalar: to,
                        definition,
                    },
                    UnitLocalValueRecipeV1::Cast { input, kind: *kind },
                    row.known_bits.map(|bits| bits & mask),
                )
            }
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            } => {
                let predicate = lower_compare(*operation)
                    .ok_or_else(|| self.error("local helper arithmetic recipe is unsupported"))?;
                if self.source_scalar(value.result_type(), budget)? != ScalarType::Bool
                    || left.ty() != right.ty()
                {
                    return Err(unit_local_mismatch_v1());
                }
                let left = self.resolve_operand(
                    site,
                    R::RvalueOperand(0),
                    left,
                    UnitLocalOperandUseV1::Next,
                    budget,
                )?;
                let right = self.resolve_operand(
                    site,
                    R::RvalueOperand(1),
                    right,
                    UnitLocalOperandUseV1::Next,
                    budget,
                )?;
                let lhs = *self.value(left).ok_or_else(unit_local_mismatch_v1)?;
                let rhs = *self.value(right).ok_or_else(unit_local_mismatch_v1)?;
                let (
                    UnitLocalNativeValueV1::Scalar {
                        value: a,
                        scalar: at,
                        ..
                    },
                    UnitLocalNativeValueV1::Scalar {
                        value: b,
                        scalar: bt,
                        ..
                    },
                ) = (lhs.native, rhs.native)
                else {
                    return Err(unit_local_mismatch_v1());
                };
                let (op, at_op) = self.take_native_operation(budget)?;
                budget.charge_work(8)?;
                if at != bt
                    || !matches!(op.kind, OperationKind::Compare { predicate: p, lhs, rhs } if p == predicate && lhs == a && rhs == b)
                    || op.results.len() != 1
                    || op.results[0].ty != Type::Scalar(ScalarType::Bool)
                {
                    return Err(unit_local_mismatch_v1());
                }
                let known = lhs.known_bits.zip(rhs.known_bits).map(|(a, b)| {
                    u64::from(match predicate {
                        ComparePredicate::Equal => a == b,
                        ComparePredicate::NotEqual => a != b,
                        ComparePredicate::LessThan => a < b,
                        ComparePredicate::LessThanOrEqual => a <= b,
                        ComparePredicate::GreaterThan => a > b,
                        ComparePredicate::GreaterThanOrEqual => a >= b,
                    })
                });
                (
                    UnitLocalNativeValueV1::Scalar {
                        value: op.results[0].id,
                        scalar: ScalarType::Bool,
                        definition: Some(at_op),
                    },
                    UnitLocalValueRecipeV1::Compare {
                        left,
                        right,
                        predicate,
                    },
                    known,
                )
            }
            SemanticRvalueKindV1::Length(place) => {
                if !place.projections().is_empty()
                    || self.source_scalar(value.result_type(), budget)? != ScalarType::U64
                {
                    return Err(self.error("local helper Length requires an exact local array"));
                }
                self.source_use(site, R::RvaluePlace, E::BaseUse, place.local(), budget)?;
                let allocation = self
                    .locals
                    .get(place.local().index() as usize)
                    .and_then(|s| s.allocation)
                    .ok_or_else(unit_local_mismatch_v1)?;
                let UnitLocalMemoryRowV1::Allocation {
                    count,
                    array_slot: Some(_),
                    ..
                } = self.rows.memory[allocation]
                else {
                    return Err(unit_local_mismatch_v1());
                };
                let (op, at) = self.take_native_operation(budget)?;
                budget.charge_work(5)?;
                if !matches!(op.kind, OperationKind::Constant(Constant::U64(n)) if n == count)
                    || op.results.len() != 1
                    || op.results[0].ty != Type::Scalar(ScalarType::U64)
                {
                    return Err(unit_local_mismatch_v1());
                }
                (
                    UnitLocalNativeValueV1::Scalar {
                        value: op.results[0].id,
                        scalar: ScalarType::U64,
                        definition: Some(at),
                    },
                    UnitLocalValueRecipeV1::ArrayLength {
                        local: place.local(),
                        allocation,
                    },
                    Some(count),
                )
            }
            SemanticRvalueKindV1::Aggregate(aggregate)
                if *aggregate.kind() == SemanticAggregateKindV1::Tuple
                    && aggregate.operands().is_empty()
                    && value.result_type() == self.input.unit_type =>
            {
                (
                    UnitLocalNativeValueV1::IgnoredUnit,
                    UnitLocalValueRecipeV1::Unit,
                    None,
                )
            }
            _ => return Err(self.error("local helper source value recipe is unsupported")),
        };
        self.rows.append_value(
            UnitLocalValueRowV1 {
                key: self.input.key,
                origin,
                role: None,
                source_type: value.result_type(),
                source_ssa: None,
                native,
                recipe,
                known_bits,
            },
            budget,
        )
    }

    fn check_statement(
        &mut self,
        block: SemanticBlockIdV1,
        statement: u32,
        source: &fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        use fe2o3_pliron::{
            ProductionSemanticSsaEventRoleV1 as E, ProductionSemanticSsaOccurrenceSiteV1 as S,
            ProductionSemanticSsaOperandRoleV1 as R,
        };
        let site = S::Statement {
            block: fe2o3_mir_model::SsaBlockIdV1::new(block.index()),
            statement,
        };
        budget.charge_work(5)?;
        match source.kind() {
            SemanticStatementKindV1::Assign(assignment) => {
                let destination = assignment.destination();
                if let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind()
                    && *aggregate.kind() == SemanticAggregateKindV1::Array
                {
                    if !destination.projections().is_empty()
                        || assignment.value().result_type() != destination.ty()
                    {
                        return Err(unit_local_mismatch_v1());
                    }
                    let start = self.rows.values.len();
                    for (component, operand) in aggregate.operands().iter().enumerate() {
                        budget.charge_work(2)?;
                        if !matches!(operand, SemanticOperandV1::Constant(_)) {
                            return Err(
                                self.error("local helper initializer requires literal components")
                            );
                        }
                        let row = self.resolve_operand(
                            site,
                            R::RvalueOperand(
                                u32::try_from(component)
                                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                            ),
                            operand,
                            UnitLocalOperandUseV1::Next,
                            budget,
                        )?;
                        if row != start + component
                            || !matches!(
                                self.rows.values[row].recipe,
                                UnitLocalValueRecipeV1::Literal { .. }
                            )
                        {
                            return Err(unit_local_mismatch_v1());
                        }
                    }
                    self.bind_destination(block, statement, destination, None, budget)?;
                    for component in 0..aggregate.operands().len() {
                        self.write_memory_place(
                            site,
                            R::Destination,
                            destination,
                            Some(
                                u32::try_from(component)
                                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                            ),
                            start + component,
                            budget,
                        )?;
                    }
                } else {
                    let value = self.check_rvalue(
                        block,
                        statement,
                        destination.local(),
                        assignment.value(),
                        budget,
                    )?;
                    let value = self
                        .bind_destination(block, statement, destination, Some(value), budget)?
                        .ok_or_else(unit_local_mismatch_v1)?;
                    if self
                        .locals
                        .get(destination.local().index() as usize)
                        .ok_or_else(unit_local_mismatch_v1)?
                        .allocation
                        .is_some()
                    {
                        self.write_memory_place(
                            site,
                            R::Destination,
                            destination,
                            None,
                            value,
                            budget,
                        )?;
                    } else if !destination.projections().is_empty() {
                        return Err(unit_local_mismatch_v1());
                    }
                }
            }
            SemanticStatementKindV1::Store(store) => {
                if store.volatility() != SemanticVolatilityV1::NonVolatile
                    || store.atomic().is_some()
                {
                    return Err(self.error("local helper volatile or atomic Store is unsupported"));
                }
                let value = self.resolve_operand(
                    site,
                    R::StoreValue,
                    store.value(),
                    UnitLocalOperandUseV1::Next,
                    budget,
                )?;
                self.source_use(
                    site,
                    R::StoreDestination,
                    E::BaseUse,
                    store.destination().local(),
                    budget,
                )?;
                self.write_memory_place(
                    site,
                    R::StoreDestination,
                    store.destination(),
                    None,
                    value,
                    budget,
                )?;
            }
            SemanticStatementKindV1::Deinitialize(place) => {
                if !place.projections().is_empty() {
                    return Err(self.error("local helper partial deinitialization is unsupported"));
                }
                self.source_use(site, R::StatementPlace, E::BaseUse, place.local(), budget)?;
                self.invalidate_source(
                    site,
                    R::StatementPlace,
                    place.local(),
                    UnitLocalInvalidateV1::Deinitialize,
                    budget,
                )?;
            }
            SemanticStatementKindV1::StorageLive(local) => self.invalidate_source(
                site,
                R::StorageLive,
                *local,
                UnitLocalInvalidateV1::StorageLive,
                budget,
            )?,
            SemanticStatementKindV1::StorageDead(local) => self.invalidate_source(
                site,
                R::StorageDead,
                *local,
                UnitLocalInvalidateV1::StorageDead,
                budget,
            )?,
            SemanticStatementKindV1::Nop => {}
            _ => return Err(self.error("local helper source statement is unsupported")),
        }
        Ok(())
    }
}
