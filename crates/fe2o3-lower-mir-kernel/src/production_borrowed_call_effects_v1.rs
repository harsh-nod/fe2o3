// Included by the slice query so pre-admission consumers use the same checks.
include!("production_borrowed_call_effects_v1_source.rs");
include!("production_borrowed_call_effects_v1_index.rs");

fn checked_borrowed_slice_call_input_v1(
    subject: CanonicalCallSubjectV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    site: ProductionSliceAccessSiteV1,
    call_path: &[usize],
    facts: &SliceFacts<'_>,
    budget: &mut SliceBudget<'_>,
) -> SliceResult<(SemanticFunctionIdV1, SliceDefinition)> {
    use fe2o3_kernel_analysis::{
        CanonicalKirCallEffectDecisionV1 as Decision, CanonicalKirCallEffectsV1,
    };

    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    budget.charge_work(2)?;
    if call_path.is_empty() {
        return Ok((site.function, facts.input));
    }
    // A complete ordinary-call path cannot revisit a function. Check this bound
    // before scanning caller-supplied locators or allocating the call index.
    if call_path.len() >= inventory.functions().len() {
        return Err(mismatch());
    }
    with_canonical_call_scratch_v1(budget, |budget| {
        let (groups, calls) = build_canonical_call_index_v1(subject, inventory, budget)?;
        let mut input = facts.input;
        let mut semantic_function = site.function;
        for &call_index in call_path.iter().rev() {
            budget.charge_work(8)?;
            let SliceDefinition::FunctionArgument { function, argument } = input else {
                return Err(mismatch());
            };
            let call = inventory.calls().get(call_index).ok_or_else(mismatch)?;
            if call.target != Some(function) {
                return Err(mismatch());
            }
            let ty = inventory
                .functions()
                .get(function.0 as usize)
                .filter(|row| row.coordinate == function)
                .and_then(|row| row.function.signature.parameters.get(argument as usize))
                .ok_or_else(mismatch)?;
            if !matches!(ty, Type::Slice(slice)
                if slice.address_space == AddressSpace::Global
                    && slice.access == AccessMode::ReadOnly
                    && slice.element.as_ref() == facts.loaded_type)
            {
                return Err(
                    site.unsupported("helper slice access requires an exact shared global carrier")
                );
            }
            let binding = assert_origin_find_v1(&calls, budget, |row, budget| {
                budget.charge_work(1)?;
                Ok(row.key().cmp(&(site.root.index(), call_index)))
            })
            .map_err(call_index_error_v1)?
            .and_then(|index| calls.get(index))
            .ok_or_else(mismatch)?;
            let callee = groups.get(binding.callee).ok_or_else(mismatch)?;
            if callee.function.canonical.coordinate != function
                || callee.function.source.semantic_function() != semantic_function
                || callee.function.source.role() != SemanticKirFunctionRoleV1::InternalHelper
            {
                return Err(mismatch());
            }
            let (caller, formal, actual, source_argument, source_local) =
                with_checked_call_site_v1(
                    subject.semantic_ssa,
                    &subject.correspondence.call_result_components,
                    binding.site,
                    callee.parameters(),
                    budget,
                    |view| {
                        if !std::ptr::eq(view.operation(), call.operation) {
                            return Err(mismatch());
                        }
                        let physical = view.physical(argument as usize)?.ok_or_else(mismatch)?;
                        if physical.parameter().ty() != ty {
                            return Err(mismatch());
                        }
                        let mut source = None;
                        view.visit_arguments(|node| {
                        if matches!(node.parameter().coverage(), ProductionArgumentCoverageV1::Parameter(parameter)
                            if parameter.slot() == argument as usize)
                        {
                            let SemanticOperandV1::Copy(place) = node.operand() else {
                                return Err(mismatch());
                            };
                            if !place.projections().is_empty()
                                || !node.parameter().source_path().is_empty()
                                || source.replace((node.parameter().source_argument(), place.local())).is_some()
                            {
                                return Err(mismatch());
                            }
                        }
                        Ok(())
                    })?;
                        let (source_argument, source_local) = source.ok_or_else(mismatch)?;
                        Ok((
                            view.caller().semantic_function(),
                            physical.parameter().value(),
                            physical.caller_value(),
                            source_argument,
                            source_local,
                        ))
                    },
                )?;
            let formal = inventory
                .definition_for_value(function, formal, budget)
                .map_err(slice_inventory_error)?
                .ok_or_else(mismatch)?;
            let caller_function = call.coordinate.block.function;
            let actual_type = inventory
                .definition_for_value(caller_function, actual, budget)
                .map_err(slice_inventory_error)?
                .ok_or_else(mismatch)?;
            if formal.coordinate != input || actual_type.ty != ty {
                return Err(mismatch());
            }
            input = super::value_origin_v1::with_whole_value_origins_v1(
                inventory,
                subject.executable,
                caller_function,
                budget,
                |origins, budget| origins.resolve(actual, budget),
            )
            .map_err(slice_inventory_error)?
            .map_err(slice_inventory_error)?
            .filter(|origin| {
                matches!(origin,
                SliceDefinition::FunctionArgument { function, .. } if *function == caller_function)
            })
            .ok_or_else(|| {
                site.unsupported("helper slice actual is not exact whole-entry transport")
            })?;
            checked_borrowed_slice_source_input_v1(
                subject,
                site.root,
                caller,
                input,
                source_local,
                fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Terminator {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(
                        binding.site.anchor.semantic_block.index(),
                    ),
                },
                fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::CallArgument(source_argument),
                budget,
            )?;
            semantic_function = caller;
        }

        let SliceDefinition::FunctionArgument { function, .. } = input else {
            return Err(mismatch());
        };
        budget.charge_work(groups.len())?;
        let mut roots = groups.iter().filter(|group| {
            group.function.source.correspondence_owner() == site.root
                && group.function.source.semantic_function() == semantic_function
                && group.function.source.role() == SemanticKirFunctionRoleV1::KernelEntry
                && group.function.canonical.coordinate == function
        });
        if roots.next().is_none() || roots.next().is_some() {
            return Err(mismatch());
        }
        // Completeness is required, but a real helper data read stays nonempty.
        // No read-only/purity decision is inferred for any other occurrence.
        let (effects, storage) = CanonicalKirCallEffectsV1::derive(inventory, budget)
            .map_err(borrowed_slice_effect_error_v1)?;
        budget.reserve_storage(storage.retained_storage())?;
        if effects
            .decision(function, budget)
            .map_err(borrowed_slice_effect_error_v1)?
            != Decision::CompleteNonempty
        {
            return Err(mismatch());
        }
        Ok((semantic_function, input))
    })
}

fn borrowed_slice_effect_error_v1(
    error: fe2o3_kernel_analysis::CanonicalKirCallEffectErrorV1,
) -> ProductionSemanticKirErrorV1 {
    match error {
        fe2o3_kernel_analysis::CanonicalKirCallEffectErrorV1::Resource(error) => error.into(),
        _ => ProductionSemanticKirErrorV1::CorrespondenceMismatch,
    }
}
