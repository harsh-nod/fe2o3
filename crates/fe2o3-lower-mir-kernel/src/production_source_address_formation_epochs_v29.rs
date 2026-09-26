// This query reconciles C2 visits of one original AddressOf recipe. It does not
// merge pointer births, expire/revive aliases, or establish memory currentness.
fn source_reference_address_epoch_matches_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    place: &SemanticPlaceV1,
    source: &SourceReferenceAccessRecordV29,
    origin_index: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let result = (|| {
        budget.source_reference_owner_v29(plan)?;
        budget.source_reference_charge_v29(plan, 24)?;
        let origin = plan
            .raw_origins
            .get(origin_index)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if source.key.site != site
            || source.key.source != place as *const SemanticPlaceV1 as usize
            || source.key.access != SourceReferenceAccessV29::Address
            || source.instance != site.instance
            || source.local != place.local()
            || source.source_local != place.local()
            || source.generation == origin.generation
            || source.ty != place.ty()
            || !source.projections.is_empty()
            || source.loan.is_some()
            || !source.traversed.is_empty()
            || source.shared_path
            || !place.projections().is_empty()
            || origin.site != site
            || origin.source != place as *const SemanticPlaceV1 as usize
            || origin.instance != source.instance
            || origin.local != source.local
            || origin.ty != source.ty
            || origin.parent.is_some()
            || origin.count != 0
            || origin.formation != SourceReferenceRawFormationV29::AddressOf
        {
            return Ok(false);
        }
        let function = plan
            .instances
            .instance(site.instance)
            .ok_or(ArgumentResourceV1::Accounting)?
            .declaration();
        let Some(statement) = site.statement else {
            return Ok(false);
        };
        let Some(SemanticStatementKindV1::Assign(assignment)) = function
            .blocks()
            .get(site.block.index() as usize)
            .and_then(|block| block.statements().get(statement))
            .map(|statement| statement.kind())
        else {
            return Ok(false);
        };
        let SemanticRvalueKindV1::AddressOf {
            place: original,
            mutability,
        } = assignment.value().kind()
        else {
            return Ok(false);
        };
        if !std::ptr::eq(original, place)
            || assignment.value().result_type() != origin.pointer_type
            || (*mutability == SemanticMutabilityV1::Mutable) != origin.mutable
            || function
                .locals()
                .get(source.local.index() as usize)
                .map(|local| local.ty())
                != Some(source.ty)
            || !matches!(
                plan.instances
                    .owner()
                    .source_semantic()
                    .types()
                    .get(source.ty.index() as usize)
                    .map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
            )
        {
            return Ok(false);
        }
        charge_execution_cfg_lookup_v29(plan.raw_origin_sites.len(), budget)?;
        if plan.raw_origin_sites.get(&(
            site.instance.index(),
            site.block.index(),
            statement,
            origin.generation,
        )) != Some(&origin_index)
        {
            return Ok(false);
        }
        let Some((_, current_cell, current)) = plan.physical_object_generation(
            source.instance,
            source.local,
            source.generation,
            budget,
        )?
        else {
            return Ok(false);
        };
        let Some((_, captured_cell, captured)) = plan.physical_object_generation(
            origin.instance,
            origin.local,
            origin.generation,
            budget,
        )?
        else {
            return Ok(false);
        };
        if current_cell != captured_cell || current != captured || current.ty != source.ty {
            return Ok(false);
        }
        let Some(current) = source_reference_original_epoch_members_v29(
            plan,
            source.instance,
            source.local,
            source.generation,
            budget,
        )?
        else {
            return Ok(false);
        };
        let captured = source_reference_original_epoch_members_v29(
            plan,
            origin.instance,
            origin.local,
            origin.generation,
            budget,
        )?;
        if let Some(captured) = captured {
            source_reference_epoch_subset_v29(current, captured, budget)
        } else {
            // An absent joined label is accepted only as an independently
            // retained original activation, never as an arbitrary integer.
            charge_execution_cfg_lookup_v29(plan.storage_activation_sites.len(), budget)?;
            let key = (
                origin.instance.index(),
                origin.local.index(),
                origin.generation,
            );
            let Some(&index) = plan.storage_activation_sites.get(&key) else {
                return Ok(false);
            };
            let activation = plan
                .storage_activations
                .get(index)
                .ok_or(ArgumentResourceV1::Accounting)?;
            budget.source_reference_charge_v29(plan, 7)?;
            if (activation.instance, activation.local, activation.generation)
                != (origin.instance, origin.local, origin.generation)
            {
                return Ok(false);
            }
            match activation.origin {
                SourceReferenceActivationOriginV29::Entry if origin.generation == 0 => {}
                SourceReferenceActivationOriginV29::StorageLive(site)
                    if site.instance == origin.instance =>
                {
                    if !matches!(function.blocks().get(site.block.index() as usize)
                        .and_then(|block| site.statement.and_then(|statement| block.statements().get(statement)))
                        .map(|statement| statement.kind()),
                        Some(SemanticStatementKindV1::StorageLive(local)) if *local == origin.local)
                    {
                        return Ok(false);
                    }
                }
                _ => return Ok(false),
            }
            source_reference_epoch_subset_v29(
                current,
                std::slice::from_ref(&origin.generation),
                budget,
            )
        }
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_reference_original_epoch_members_v29<'plan>(
    plan: &'plan SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<&'plan [u32]>, ProductionSemanticKirErrorV1> {
    charge_execution_cfg_lookup_v29(plan.epoch_objects.len(), budget)?;
    let Some(indices) = plan.epoch_objects.get(&(instance.index(), local.index())) else {
        return Ok(None);
    };
    budget.source_reference_charge_v29(plan, call_splice_search_work_v1(indices.len()))?;
    let found = indices.binary_search_by_key(&generation, |&index| {
        plan.epoch_sets
            .get(index)
            .map_or(u32::MAX, |row| row.generation)
    });
    let Ok(index) = found else {
        return Ok(None);
    };
    budget.source_reference_charge_v29(plan, 6)?;
    let row = plan
        .epoch_sets
        .get(indices[index])
        .ok_or(ArgumentResourceV1::Accounting)?;
    if row.instance != instance
        || row.local != local
        || row.generation != generation
        || row.count < 2
    {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    Ok(Some(
        plan.epoch_members
            .get(row.first..argument_sum_v1(&[row.first, row.count])?)
            .ok_or(ArgumentResourceV1::Accounting)?,
    ))
}

fn source_reference_epoch_subset_v29(
    current: &[u32],
    captured: &[u32],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    if current.len() < 2 || captured.is_empty() {
        return Ok(false);
    }
    for atoms in [current, captured] {
        let mut previous = None;
        for &atom in atoms {
            budget.charge_work(1)?;
            if atom == u32::MAX || previous.is_some_and(|previous| previous >= atom) {
                return Ok(false);
            }
            previous = Some(atom);
        }
    }
    let mut first = 0;
    for &wanted in captured {
        while let Some(&found) = current.get(first) {
            budget.charge_work(1)?;
            first += 1;
            if found > wanted {
                return Ok(false);
            }
            if found == wanted {
                break;
            }
        }
        if first == 0 || current.get(first - 1) != Some(&wanted) {
            return Ok(false);
        }
    }
    Ok(true)
}
