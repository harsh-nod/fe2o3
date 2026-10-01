fn source_scalar_move_path_v45(
    types: &[SemanticTypeDeclV1],
    place: &SemanticPlaceV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_sum_v1(&[2, place.projections().len()])?)?;
    Ok(!place.projections().is_empty()
        && matches!(
            types
                .get(place.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
        )
        && place.projections().iter().all(|projection| {
            matches!(
                projection.kind(),
                SemanticProjectionKindV1::Field(_) | SemanticProjectionKindV1::ConstantIndex { .. }
            )
        }))
}

fn checked_scoped_scalar_move_v45<'a>(
    function: &'a SemanticFunctionDeclV1,
    occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    row: &ScopedMemoryAnchorV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<&'a SemanticPlaceV1, ProductionSemanticKirErrorV1> {
    budget.charge_work(12)?;
    if !occurrences
        .owner()
        .source_semantic()
        .functions()
        .get(occurrences.function().index() as usize)
        .is_some_and(|original| std::ptr::eq(original, function))
    {
        return Err(scoped_memory_error_v29());
    }
    let ScopedMemoryAnchorKindV29::ScalarMove { event, local } = row.kind else {
        return Err(scoped_memory_error_v29());
    };
    let frame = row.source.ok_or_else(scoped_memory_error_v29)?;
    let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
        return Err(scoped_memory_error_v29());
    };
    if matches!(role, ExecutionOperandV29::AssertMessage(_)) {
        return Err(scoped_memory_error_v29());
    }
    let Some(SemanticOperandV1::Move(place)) =
        scoped_source_operand_v29(function, frame.site, role)
    else {
        return Err(scoped_memory_error_v29());
    };
    let original = occurrences
        .events()
        .get(event)
        .ok_or_else(scoped_memory_error_v29)?;
    if original.site() != frame.site
        || original.operand() != role
        || original.role() != ExecutionEventV29::BaseUse
        || !original.is_reachable()
        || original.event()
            != fe2o3_mir_model::SsaEventV1::Use(fe2o3_mir_model::SsaVariableIdV1::new(local))
        || place.local().index() != local
        || !source_scalar_move_path_v45(
            occurrences.owner().source_semantic().types(),
            place,
            budget,
        )?
    {
        return Err(scoped_memory_error_v29());
    }
    Ok(place)
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn has_retained_scalar_move_v45(
        &mut self,
        place: &SemanticPlaceV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        if self.scoped_memory.is_none()
            || self.retained_local_slots.is_empty()
            || place.projections().is_empty()
        {
            return Ok(false);
        }
        self.with_emission_budget_v1(|this, budget| {
            if !source_scalar_move_path_v45(this.types, place, budget)? {
                return Ok(false);
            }
            charge_execution_cfg_lookup_v29(this.retained_local_slots.len(), budget)?;
            let first = ScopedAllocationIdentityV29::OriginalObject {
                local: place.local().index(),
                generation: 0,
            };
            let last = ScopedAllocationIdentityV29::OriginalObject {
                local: place.local().index(),
                generation: u32::MAX,
            };
            Ok(this
                .retained_local_slots
                .range(first..=last)
                .next()
                .is_some())
        })
    }

    fn record_scoped_scalar_move_v45(
        &mut self,
        site: ExecutionSiteV29,
        role: Option<ExecutionOperandV29>,
        operand: &SemanticOperandV1,
        position: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let SemanticOperandV1::Move(place) = operand else {
            return Ok(());
        };
        if !self.has_retained_scalar_move_v45(place)? {
            return Ok(());
        }
        self.with_emission_budget_v1(|this, budget| {
            source_reference_emission_prepay_v29::<(
                ScopedMemoryAnchorV29,
                ScopedMemoryFrameV29,
                usize,
                usize,
            )>(budget)?;
            budget.charge_work(5)?;
            let role = role.ok_or_else(scoped_memory_error_v29)?;
            let cursor = this
                .execution
                .as_mut()
                .ok_or_else(scoped_memory_error_v29)?;
            cursor.check_ledger(budget)?;
            let source_block = match site {
                ExecutionSiteV29::Statement { block, .. }
                | ExecutionSiteV29::Terminator { block } => block,
            };
            if cursor.block != Some(source_block) {
                return Err(scoped_memory_error_v29());
            }
            if !scoped_source_operand_v29(this.function, site, role)
                .is_some_and(|source| std::ptr::eq(source, operand))
            {
                return Err(scoped_memory_error_v29());
            }
            let key = unit_local_source_key_v1(site, role, Some(ExecutionEventV29::BaseUse));
            budget.charge_work(argument_product_v1(
                8,
                scoped_initialization_search_work_v29(cursor.index.len()),
            )?)?;
            let index = cursor
                .index
                .binary_search_by_key(&key, |row| row.key)
                .map_err(|_| scoped_memory_error_v29())?;
            let event = cursor.index[index].index;
            let recorder = this
                .scoped_memory
                .as_mut()
                .ok_or_else(scoped_memory_error_v29)?;
            let row = ScopedMemoryAnchorV29 {
                block: recorder.block.ok_or_else(scoped_memory_error_v29)?,
                position,
                source: Some(ScopedMemoryFrameV29::operand(site, Some(role))),
                kind: ScopedMemoryAnchorKindV29::ScalarMove {
                    event,
                    local: place.local().index(),
                },
            };
            if !std::ptr::eq(
                checked_scoped_scalar_move_v45(this.function, &cursor.occurrences, &row, budget)?,
                place,
            ) {
                return Err(scoped_memory_error_v29());
            }
            if cursor.claimed[event] {
                let Some(SsaResolvedEventV1::Use { value, .. }) =
                    cursor.occurrences.events()[event].resolved()
                else {
                    return Err(scoped_memory_error_v29());
                };
                cursor.check_claimed_original_operand_v46(site, role, place, value, budget)?;
            } else {
                // Retained storage has an original use, not an SSA value.
                // Claim only that exact use in source order; byte history
                // separately checks its current generation and initialized range.
                let original = &cursor.occurrences.events()[event];
                if original.is_promoted() || original.resolved().is_some() {
                    return Err(scoped_memory_error_v29());
                }
                cursor.claim_events(&[event], budget)?;
            }
            emission_push_v1(&mut recorder.anchors.rows, row, budget)
        })
    }
}
