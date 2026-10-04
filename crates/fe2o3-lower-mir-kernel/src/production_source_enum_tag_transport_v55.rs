// The complete source parent is retained, but no payload may be reconstructed
// from this tag or from the compiler-private spill addresses.
#[derive(Clone, Debug)]
struct SemanticSourceEnumTagBindingV55 {
    owner: usize,
    source: [u8; 32],
    ssa: fe2o3_pliron::ProductionSemanticSsaIdentityV1,
    root: ProductionCallInstanceIdV1,
    node: usize,
    tag: ValueDef,
}

fn source_enum_tag_error_v55() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29(
        "source enum tag transport differs from its complete original alternatives",
    )
}

fn source_enum_tag_type_v55(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    source_reference_owned_prepay_v29::<Type>(plan, budget)?;
    budget.source_reference_charge_v29(plan, 6)?;
    let row = plan.nodes.get(node).ok_or_else(source_enum_tag_error_v55)?;
    let SourceReferenceNodeKindV29::Enum { first, count } = row.kind else {
        return Err(source_enum_tag_error_v55());
    };
    let types = plan.instances.owner().source_semantic().types();
    let SemanticTypeShapeV1::Enum {
        discriminant,
        variants,
    } = types
        .get(row.ty.index() as usize)
        .ok_or_else(source_enum_tag_error_v55)?
        .shape()
    else {
        return Err(source_enum_tag_error_v55());
    };
    if count == 0
        || row.inactive.is_some()
        || execution_cfg_nominal_count_v29(types, row.ty, budget)? != 0
    {
        return Err(source_enum_tag_error_v55());
    }
    let mut previous = None;
    for offset in 0..count {
        budget.source_reference_charge_v29(plan, 6)?;
        let member = *plan
            .enum_members
            .get(argument_sum_v1(&[first, offset])?)
            .ok_or_else(source_enum_tag_error_v55)?;
        if previous.is_some_and(|previous| previous >= member) {
            return Err(source_enum_tag_error_v55());
        }
        previous = Some(member);
        let alternative = plan
            .enum_alternatives
            .get(member)
            .ok_or_else(source_enum_tag_error_v55)?;
        let variant = variants
            .get(alternative.variant as usize)
            .ok_or_else(source_enum_tag_error_v55)?;
        if alternative.ty != row.ty
            || alternative.opaque.is_some()
            || variant.is_uninhabited()
            || alternative.count != variant.fields().fields().len()
        {
            return Err(source_enum_tag_error_v55());
        }
        for (field, &ty) in variant.fields().fields().iter().enumerate() {
            budget.source_reference_charge_v29(plan, 3)?;
            let child = *plan
                .children
                .get(argument_sum_v1(&[alternative.first, field])?)
                .ok_or_else(source_enum_tag_error_v55)?;
            if child >= node || plan.nodes.get(child).map(|child| child.ty) != Some(ty) {
                return Err(source_enum_tag_error_v55());
            }
        }
    }
    lower_scalar_type(types, *discriminant)
}

fn validate_source_enum_tag_v55(
    plan: &SourceReferencePlanV29<'_, '_>,
    binding: &SemanticSourceEnumTagBindingV55,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    budget.source_reference_charge_v29(plan, 6)?;
    if binding.owner != plan as *const SourceReferencePlanV29<'_, '_> as usize
        || binding.source != plan.source
        || binding.ssa != plan.ssa
        || binding.root != plan.root
        || !invocation_equal_types_v1(
            &binding.tag.ty,
            &source_enum_tag_type_v55(plan, binding.node, budget)?,
            budget,
        )?
    {
        return Err(source_enum_tag_error_v55());
    }
    Ok(())
}

fn rebuild_source_enum_tag_v55(
    references: &SourceReferenceEmissionV29<'_, '_>,
    node: usize,
    values: &[ValueDef],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
    references.check(budget)?;
    let plan = references.plan;
    source_reference_owned_prepay_v29::<SemanticSourceEnumTagBindingV55>(plan, budget)?;
    let [value] = values else {
        return Err(source_enum_tag_error_v55());
    };
    let ty = source_enum_tag_type_v55(plan, node, budget)?;
    if !invocation_equal_types_v1(&value.ty, &ty, budget)? {
        return Err(source_enum_tag_error_v55());
    }
    Ok(SemanticValueBindingV1::SourceEnumTag(
        SemanticSourceEnumTagBindingV55 {
            owner: plan as *const SourceReferencePlanV29<'_, '_> as usize,
            source: plan.source,
            ssa: plan.ssa,
            root: plan.root,
            node,
            tag: ValueDef::new(value.id, ty),
        },
    ))
}

fn source_reference_cfg_enum_tag_types_v55(
    cursor: &ExecutionAvailabilityV29<'_>,
    local: u32,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
    cursor.check_ledger(budget)?;
    let references = cursor.references.ok_or_else(source_enum_tag_error_v55)?;
    references.check(budget)?;
    let plan = references.plan;
    if cursor.cfg.source_enum_locals.get(local as usize) != Some(&true) {
        return Err(source_enum_tag_error_v55());
    }
    source_reference_owned_prepay_v29::<Option<Type>>(plan, budget)?;
    let mut selected = None;
    for entry in &cursor.cfg.entries {
        budget.source_reference_charge_v29(plan, 2)?;
        if entry.local != local {
            continue;
        }
        let ty = source_enum_tag_type_v55(
            plan,
            entry.reference.ok_or_else(source_enum_tag_error_v55)?,
            budget,
        )?;
        if let Some(previous) = &selected {
            if !invocation_equal_types_v1(previous, &ty, budget)? {
                return Err(source_enum_tag_error_v55());
            }
        } else {
            selected = Some(ty);
        }
    }
    let mut result = source_reference_owned_vec_v29(plan, 1, budget)?;
    result.push(selected.ok_or_else(source_enum_tag_error_v55)?);
    Ok(result)
}

fn source_enum_parent_subset_v55(
    plan: &SourceReferencePlanV29<'_, '_>,
    source: usize,
    destination: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    source_enum_tag_type_v55(plan, source, budget)?;
    source_enum_tag_type_v55(plan, destination, budget)?;
    let a = &plan.nodes[source];
    let b = &plan.nodes[destination];
    if a.ty != b.ty {
        return Ok(false);
    }
    let SourceReferenceNodeKindV29::Enum {
        first: a,
        count: na,
    } = a.kind
    else {
        return Err(source_enum_tag_error_v55());
    };
    let SourceReferenceNodeKindV29::Enum {
        first: b,
        count: nb,
    } = b.kind
    else {
        return Err(source_enum_tag_error_v55());
    };
    let mut next = 0;
    for offset in 0..na {
        let member = plan.enum_members[argument_sum_v1(&[a, offset])?];
        while next < nb {
            budget.source_reference_charge_v29(plan, 2)?;
            if plan.enum_members[argument_sum_v1(&[b, next])?] >= member {
                break;
            }
            next += 1;
        }
        budget.source_reference_charge_v29(plan, 2)?;
        if next == nb || plan.enum_members[argument_sum_v1(&[b, next])?] != member {
            return Ok(false);
        }
    }
    Ok(true)
}

fn merge_source_enum_tag_v55(
    references: &SourceReferenceEmissionV29<'_, '_>,
    node: usize,
    held: &SemanticValueBindingV1,
    archived: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    references.check(budget)?;
    merge_source_enum_tag_plan_v59(references.plan, node, held, archived, budget)
}

fn merge_source_enum_tag_plan_v59(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    held: &SemanticValueBindingV1,
    archived: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    let tag = source_enum_tag_type_v55(plan, node, budget)?;
    if let (SemanticValueBindingV1::SourceEnumTag(a), SemanticValueBindingV1::SourceEnumTag(b)) =
        (held, archived)
    {
        validate_source_enum_tag_v55(plan, a, budget)?;
        validate_source_enum_tag_v55(plan, b, budget)?;
        budget.source_reference_charge_v29(plan, 4)?;
        if a.node != b.node
            || a.tag.id != b.tag.id
            || !invocation_equal_types_v1(&a.tag.ty, &b.tag.ty, budget)?
            || !source_enum_parent_subset_v55(plan, a.node, node, budget)?
        {
            return Err(source_enum_tag_error_v55());
        }
        return Ok(());
    }
    let (
        SemanticValueBindingV1::Enum {
            discriminant: a,
            discriminant_ty: at,
            semantic_type: ay,
            variant: Some(av),
            payloads: ap,
        },
        SemanticValueBindingV1::Enum {
            discriminant: b,
            discriminant_ty: bt,
            semantic_type: by,
            variant: Some(bv),
            payloads: bp,
        },
    ) = (held, archived)
    else {
        return Err(source_enum_tag_error_v55());
    };
    budget.source_reference_charge_v29(plan, 9)?;
    if a != b
        || ay != by
        || *ay != plan.nodes[node].ty
        || av != bv
        || ap.len() != 1
        || bp.len() != 1
        || !invocation_equal_types_v1(at, &tag, budget)?
        || !invocation_equal_types_v1(bt, &tag, budget)?
    {
        return Err(source_enum_tag_error_v55());
    }
    let left = ap.get(av).ok_or_else(source_enum_tag_error_v55)?;
    let right = bp.get(bv).ok_or_else(source_enum_tag_error_v55)?;
    let SourceReferenceNodeKindV29::Enum { first, count } = plan.nodes[node].kind else {
        return Err(source_enum_tag_error_v55());
    };
    // One candidate must match every field. A source branch is not authorized
    // by independently selecting different parent members for sibling fields.
    for offset in 0..count {
        budget.source_reference_charge_v29(plan, 4)?;
        let member = plan.enum_members[argument_sum_v1(&[first, offset])?];
        let alternative = &plan.enum_alternatives[member];
        if alternative.variant != *av
            || alternative.count != left.len()
            || alternative.count != right.len()
        {
            continue;
        }
        let mut matched = true;
        for field in 0..alternative.count {
            budget.source_reference_charge_v29(plan, 2)?;
            let child = plan.children[argument_sum_v1(&[alternative.first, field])?];
            let mut no_leaf_storage: [Option<ExecutionCfgLeafV29>; 0] = [];
            let mut no_leaves = no_leaf_storage.iter_mut();
            match source_reference_merge_node_plan_v59(
                plan,
                child,
                &left[field],
                &right[field],
                &mut no_leaves,
                &mut 0,
                budget,
            ) {
                Ok(()) => {}
                Err(error @ ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_)) => {
                    return Err(error);
                }
                Err(_) => {
                    matched = false;
                    break;
                }
            }
        }
        if matched {
            return Ok(());
        }
    }
    Err(source_enum_tag_error_v55())
}

fn source_enum_transport_tag_v55(
    references: &SourceReferenceEmissionV29<'_, '_>,
    node: usize,
    binding: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<ValueDef, ProductionSemanticKirErrorV1> {
    references.check(budget)?;
    source_reference_owned_prepay_v29::<ValueDef>(references.plan, budget)?;
    merge_source_enum_tag_v55(references, node, binding, binding, budget)?;
    let (id, ty) = match binding {
        SemanticValueBindingV1::SourceEnumTag(binding) => (binding.tag.id, &binding.tag.ty),
        SemanticValueBindingV1::Enum {
            discriminant,
            discriminant_ty,
            ..
        } => (*discriminant, discriminant_ty),
        _ => return Err(source_enum_tag_error_v55()),
    };
    Ok(ValueDef::new(id, execution_cfg_clone_type_v29(ty, budget)?))
}

fn with_source_enum_transport_tag_v55<R>(
    references: &SourceReferenceEmissionV29<'_, '_>,
    node: usize,
    binding: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
    consume: impl FnOnce(
        &[ValueDef],
        &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    let floor = budget.storage();
    let built = catch_unwind(AssertUnwindSafe(|| {
        source_enum_transport_tag_v55(references, node, binding, budget)
    }));
    let storage = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let result = match built {
        Ok(Ok(tag)) => catch_unwind(AssertUnwindSafe(|| {
            consume(std::slice::from_ref(&tag), budget)
        })),
        Ok(Err(error)) => Ok(Err(error)),
        Err(payload) => Err(payload),
    };
    let released = budget.release_storage(storage);
    match result {
        Ok(result) => {
            released?;
            result
        }
        Err(payload) => {
            let _ = released;
            resume_unwind(payload)
        }
    }
}

include!("production_source_enum_spill_transport_v55.rs");
