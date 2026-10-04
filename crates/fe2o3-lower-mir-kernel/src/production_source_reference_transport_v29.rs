include!("production_source_reference_owned_types_v29.rs");
include!("production_source_enum_tag_transport_v55.rs");

// Plain source anchors retain the original root's authenticated direct-owner
// representation. In particular, a DisjointSlice is one Slice, not its fields.
fn source_reference_anchor_type_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    anchor: SourceReferenceAnchorV29,
    ty: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<Type>, ProductionSemanticKirErrorV1> {
    let result = (|| {
        budget.source_reference_owner_v29(plan)?;
        budget.source_reference_charge_v29(plan, 8)?;
        let source = plan.instances.owner().source_semantic();
        let root = plan
            .instances
            .instance(plan.root)
            .ok_or_else(execution_cfg_error_v29)?;
        let function = root.declaration();
        let argument = anchor.argument as usize;
        if anchor.ty != ty || function.abi().source_input_types().get(argument) != Some(&ty) {
            return Err(execution_cfg_error_v29());
        }
        if function.abi().source_argument_ownership().get(argument)
            != Some(&SemanticSourceArgumentOwnershipV1::ExclusiveOwner)
        {
            return Ok(None);
        }
        budget.source_reference_charge_v29(plan, 8)?;
        let adjusted = function
            .abi()
            .adjusted_arguments()
            .get(argument)
            .ok_or_else(execution_cfg_error_v29)?;
        if adjusted.role() != SemanticAbiArgumentRoleV1::Source
            || adjusted.ty() != ty
            || adjusted.value().adjusted().is_some()
            || (matches!(adjusted.mode(), SemanticAbiPassModeV1::Pair { .. })
                && adjusted.value().pointee_override().is_some())
        {
            return Err(execution_cfg_error_v29());
        }
        if execution_cfg_nominal_count_v29(source.types(), ty, budget)? != 0 {
            return Ok(None);
        }
        let declaration = source
            .types()
            .get(ty.index() as usize)
            .ok_or_else(execution_cfg_error_v29)?;
        let fields = match declaration.shape() {
            SemanticTypeShapeV1::Aggregate(fields) => fields.fields().len(),
            _ => 0,
        };
        budget.source_reference_charge_v29(
            plan,
            argument_sum_v1(&[
                argument_product_v1(source.callables().len(), 4)?,
                argument_product_v1(fields, 20)?,
                32,
            ])?,
        )?;
        source_reference_owned_prepay_v29::<Option<Type>>(plan, budget)?;
        budget.source_reference_reserve_v29(plan, std::mem::size_of::<Type>())?;
        Ok(authenticated_disjoint_slice_parameter(
            source.types(),
            source.callables(),
            function,
            anchor.argument,
            ty,
        )
        .or_else(|| {
            authenticated_global_mut_pointer_parameter(
                source.types(),
                function,
                anchor.argument,
                ty,
            )
        }))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_reference_type_present_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let mut pending = source_reference_emission_vec_v29(1, budget)?;
    pending.push(ty);
    let mut nodes = 0;
    let mut present = false;
    while let Some(ty) = pending.pop() {
        execution_cfg_charge_node_v29(&mut nodes, budget)?;
        if execution_cfg_nominal_kind_v29(types, ty)?.is_some() {
            continue;
        }
        match types
            .get(ty.index() as usize)
            .ok_or_else(execution_cfg_error_v29)?
            .shape()
        {
            SemanticTypeShapeV1::Pointer(pointer) => {
                present |= (matches!(
                    pointer.kind(),
                    SemanticPointerKindV1::Reference | SemanticPointerKindV1::Raw
                ) && pointer.metadata() == SemanticPointerMetadataV1::None)
                    || (pointer.kind() == SemanticPointerKindV1::Reference
                        && pointer.metadata() == SemanticPointerMetadataV1::SliceLength)
            }
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                for field in fields.fields() {
                    source_reference_emission_push_v29(&mut pending, *field, budget)?;
                }
            }
            SemanticTypeShapeV1::Array { element, .. } => {
                source_reference_emission_push_v29(&mut pending, *element, budget)?
            }
            SemanticTypeShapeV1::Enum { variants, .. } => {
                for variant in variants {
                    budget.charge_work(1)?;
                    if variant.is_uninhabited() {
                        continue;
                    }
                    for field in variant.fields().fields() {
                        source_reference_emission_push_v29(&mut pending, *field, budget)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(present)
}

fn source_reference_node_has_selected_pointer_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let result = (|| {
        budget.source_reference_owner_v29(plan)?;
        let mut pending = source_reference_owned_vec_v29(plan, 1, budget)?;
        pending.push(node);
        let mut nodes = 0;
        let mut present = false;
        while let Some(index) = pending.pop() {
            execution_cfg_charge_node_v29(&mut nodes, budget)?;
            budget.source_reference_charge_v29(plan, 1)?;
            let row = plan
                .nodes
                .get(index)
                .ok_or(ArgumentResourceV1::Accounting)?;
            match row.kind {
                SourceReferenceNodeKindV29::Address(_) => {
                    if budget
                        .source_reference_selected_pointer_type_v29(plan, index, row.ty)?
                        .is_none()
                    {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    present = true;
                }
                SourceReferenceNodeKindV29::Aggregate { first, count } => {
                    for offset in 0..count {
                        budget.source_reference_charge_v29(plan, 1)?;
                        let child = *plan
                            .children
                            .get(argument_sum_v1(&[first, offset])?)
                            .ok_or(ArgumentResourceV1::Accounting)?;
                        if child >= index {
                            return Err(ArgumentResourceV1::Accounting.into());
                        }
                        source_reference_owned_push_v29(plan, &mut pending, child, budget)?;
                    }
                }
                SourceReferenceNodeKindV29::Enum { first, count } => {
                    for offset in 0..count {
                        budget.source_reference_charge_v29(plan, 2)?;
                        let member = *plan
                            .enum_members
                            .get(argument_sum_v1(&[first, offset])?)
                            .ok_or(ArgumentResourceV1::Accounting)?;
                        let alternative = plan
                            .enum_alternatives
                            .get(member)
                            .ok_or(ArgumentResourceV1::Accounting)?;
                        for field in 0..alternative.count {
                            budget.source_reference_charge_v29(plan, 1)?;
                            let child = *plan
                                .children
                                .get(argument_sum_v1(&[alternative.first, field])?)
                                .ok_or(ArgumentResourceV1::Accounting)?;
                            if child >= index {
                                return Err(ArgumentResourceV1::Accounting.into());
                            }
                            source_reference_owned_push_v29(plan, &mut pending, child, budget)?;
                        }
                    }
                }
                SourceReferenceNodeKindV29::EnumView(view) => {
                    budget.source_reference_charge_v29(plan, 1)?;
                    let view = plan
                        .enum_views
                        .get(view)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    if view.source >= index {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    for offset in 0..view.child_count {
                        budget.source_reference_charge_v29(plan, 1)?;
                        let child = *plan
                            .children
                            .get(argument_sum_v1(&[view.children, offset])?)
                            .ok_or(ArgumentResourceV1::Accounting)?;
                        if child >= index {
                            return Err(ArgumentResourceV1::Accounting.into());
                        }
                        source_reference_owned_push_v29(plan, &mut pending, child, budget)?;
                    }
                }
                SourceReferenceNodeKindV29::Absent
                | SourceReferenceNodeKindV29::Plain(_)
                | SourceReferenceNodeKindV29::Discriminant(_)
                | SourceReferenceNodeKindV29::Loan(_) => {}
            }
        }
        Ok(present)
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

#[allow(clippy::too_many_arguments)]
fn source_reference_rebuild_node_v29(
    references: &SourceReferenceEmissionV29<'_, '_>,
    node: usize,
    canonical: bool,
    leaves: &mut std::slice::Iter<'_, Option<ExecutionCfgLeafV29>>,
    values: &mut std::slice::Iter<'_, ValueDef>,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
    let plan = references.plan;
    references.check(budget)?;
    source_reference_owned_prepay_v29::<SemanticValueBindingV1>(plan, budget)?;
    execution_cfg_charge_node_v29(nodes, budget)
        .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
    let row = *plan.nodes.get(node).ok_or_else(execution_cfg_error_v29)?;
    match row.kind {
        SourceReferenceNodeKindV29::Absent => {
            let types = source_reference_inactive_payload_types_v29(plan, node, nodes, budget)?;
            budget.source_reference_reserve_v29(
                plan,
                std::mem::size_of::<SemanticSourceInactiveBindingV29>(),
            )?;
            let mut payload = source_reference_owned_vec_v29(plan, types.len(), budget)?;
            for ty in &types {
                budget.source_reference_charge_v29(plan, 1)?;
                let value = values
                    .next()
                    .ok_or_else(source_reference_inactive_error_v29)?;
                if !invocation_equal_types_v1(&value.ty, ty, budget)
                    .inspect_err(|error| source_reference_record_failure_v29(plan, error))?
                {
                    return Err(source_reference_inactive_error_v29());
                }
                payload.push(ValueDef::new(
                    value.id,
                    execution_cfg_clone_type_v29(ty, budget)
                        .inspect_err(|error| source_reference_record_failure_v29(plan, error))?,
                ));
            }
            Ok(SemanticValueBindingV1::SourceInactive(
                SemanticSourceInactiveBindingV29 {
                    owner: plan as *const SourceReferencePlanV29<'_, '_> as usize,
                    source: plan.source,
                    ssa: plan.ssa,
                    root: plan.root,
                    node,
                    source_type: row.ty,
                    values: payload,
                },
            ))
        }
        SourceReferenceNodeKindV29::Loan(_) | SourceReferenceNodeKindV29::EnumView(_) => {
            let origin = match row.kind {
                SourceReferenceNodeKindV29::Loan(loan) => {
                    SourceReferenceBindingOriginV29::SingleLoan(loan)
                }
                SourceReferenceNodeKindV29::EnumView(_) => {
                    SourceReferenceBindingOriginV29::EnumView(node)
                }
                _ => return Err(source_reference_enum_error_v29()),
            };
            let types = match origin {
                SourceReferenceBindingOriginV29::SingleLoan(loan) => {
                    source_reference_payload_types_v29(plan, loan, budget)?
                }
                SourceReferenceBindingOriginV29::EnumView(_) => {
                    source_reference_binding_origin_types_v29(plan, origin, row.ty, nodes, budget)?
                }
            };
            budget.source_reference_reserve_v29(
                plan,
                std::mem::size_of::<SemanticSourceReferenceBindingV29>(),
            )?;
            let mut payload = source_reference_owned_vec_v29(plan, types.len(), budget)?;
            // This iterator finishes before recursive rebuild or external use.
            // Its vector shell and moved boxes retain their original credits.
            for ty in types {
                budget.source_reference_charge_v29(plan, 1)?;
                let value = values.next().ok_or_else(execution_cfg_error_v29)?;
                if value.ty != ty {
                    return Err(execution_cfg_error_v29());
                }
                payload.push(ValueDef::new(
                    value.id,
                    source_reference_move_type_v29(ty, budget)
                        .inspect_err(|error| source_reference_record_failure_v29(plan, error))?,
                ));
            }
            let binding = SemanticSourceReferenceBindingV29 {
                owner: plan as *const SourceReferencePlanV29<'_, '_> as usize,
                source: plan.source,
                ssa: plan.ssa,
                root: plan.root,
                origin,
                source_type: row.ty,
                values: payload,
            };
            source_reference_validate_binding_v29(plan, &binding, budget)?;
            Ok(SemanticValueBindingV1::SourceReference(binding))
        }
        SourceReferenceNodeKindV29::Address(_) => {
            let ty = budget
                .source_reference_selected_pointer_type_v29(plan, node, row.ty)?
                .ok_or(ArgumentResourceV1::Accounting)?;
            let value = values.next().ok_or_else(execution_cfg_error_v29)?;
            if !invocation_equal_types_v1(&value.ty, &ty, budget)? {
                return Err(execution_cfg_error_v29());
            }
            Ok(SemanticValueBindingV1::Value { id: value.id, ty })
        }
        SourceReferenceNodeKindV29::Plain(_) | SourceReferenceNodeKindV29::Discriminant(_) => {
            if let SourceReferenceNodeKindV29::Plain(Some(anchor)) = row.kind
                && let Some(expected) =
                    source_reference_anchor_type_v29(plan, anchor, row.ty, budget)?
            {
                let value = values.next().ok_or_else(execution_cfg_error_v29)?;
                if !invocation_equal_types_v1(&value.ty, &expected, budget)? {
                    return Err(execution_cfg_error_v29());
                }
                return Ok(SemanticValueBindingV1::Value {
                    id: value.id,
                    ty: expected,
                });
            }
            if row.descriptor.is_some() {
                budget.source_reference_reserve_v29(plan, std::mem::size_of::<Type>())?;
                let expected = source_descriptor_node_type_v29(plan, Some(node), row.ty, budget)?;
                let value = values.next().ok_or_else(execution_cfg_error_v29)?;
                if !invocation_equal_types_v1(&value.ty, &expected, budget)? {
                    return Err(execution_cfg_error_v29());
                }
                return Ok(SemanticValueBindingV1::Value {
                    id: value.id,
                    ty: expected,
                });
            }
            rebuild_source_execution_cfg_binding_v29(
                plan.instances.owner().source_semantic().types(),
                row.ty,
                canonical,
                leaves,
                values,
                nodes,
                budget,
            )
            .inspect_err(|error| source_reference_record_failure_v29(plan, error))
        }
        SourceReferenceNodeKindV29::Enum { .. } => {
            let (tag, alternative) = source_reference_enum_single_v29(plan, node, budget)?;
            let value = values.next().ok_or_else(source_reference_enum_error_v29)?;
            if value.ty != tag {
                return Err(source_reference_enum_error_v29());
            }
            let mut fields = source_reference_owned_vec_v29(plan, alternative.count, budget)?;
            for field in 0..alternative.count {
                budget.source_reference_charge_v29(plan, 1)?;
                let child = plan.children[argument_sum_v1(&[alternative.first, field])?];
                fields.push(source_reference_rebuild_node_v29(
                    references, child, canonical, leaves, values, nodes, budget,
                )?);
            }
            source_reference_owned_prepay_v29::<BTreeMap<u32, Vec<SemanticValueBindingV1>>>(
                plan, budget,
            )?;
            reserve_execution_cfg_map_entry_v29::<u32, Vec<SemanticValueBindingV1>>(0, budget)?;
            let mut payloads = BTreeMap::new();
            payloads.insert(alternative.variant, fields);
            Ok(SemanticValueBindingV1::Enum {
                discriminant: value.id,
                discriminant_ty: tag,
                semantic_type: row.ty,
                // An alternative key describes payload identity, not active-tag proof.
                variant: None,
                payloads,
            })
        }
        SourceReferenceNodeKindV29::Aggregate { first, count } => {
            let mut fields = source_reference_owned_vec_v29(plan, count, budget)?;
            for index in 0..count {
                budget.source_reference_charge_v29(plan, 1)?;
                let child = *plan
                    .children
                    .get(argument_sum_v1(&[first, index])?)
                    .ok_or_else(execution_cfg_error_v29)?;
                if child >= node {
                    return Err(execution_cfg_error_v29());
                }
                fields.push(source_reference_rebuild_node_v29(
                    references, child, canonical, leaves, values, nodes, budget,
                )?);
            }
            Ok(SemanticValueBindingV1::Aggregate(fields))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn source_reference_merge_node_v29(
    references: &SourceReferenceEmissionV29<'_, '_>,
    node: usize,
    held: &SemanticValueBindingV1,
    archived: &SemanticValueBindingV1,
    leaves: &mut std::slice::IterMut<'_, Option<ExecutionCfgLeafV29>>,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let plan = references.plan;
    references.check(budget)?;
    execution_cfg_charge_node_v29(nodes, budget)
        .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
    let row = *plan.nodes.get(node).ok_or_else(execution_cfg_error_v29)?;
    match row.kind {
        SourceReferenceNodeKindV29::Absent => {
            source_reference_merge_inactive_v29(references, node, held, archived, nodes, budget)
        }
        SourceReferenceNodeKindV29::Loan(_) | SourceReferenceNodeKindV29::EnumView(_) => {
            let origin = match row.kind {
                SourceReferenceNodeKindV29::Loan(loan) => {
                    SourceReferenceBindingOriginV29::SingleLoan(loan)
                }
                SourceReferenceNodeKindV29::EnumView(_) => {
                    SourceReferenceBindingOriginV29::EnumView(node)
                }
                _ => return Err(source_reference_enum_error_v29()),
            };
            let (
                SemanticValueBindingV1::SourceReference(held),
                SemanticValueBindingV1::SourceReference(archived),
            ) = (held, archived)
            else {
                return Err(execution_cfg_error_v29());
            };
            source_reference_validate_binding_v29(plan, held, budget)?;
            source_reference_validate_binding_v29(plan, archived, budget)?;
            budget.source_reference_charge_v29(plan, argument_product_v1(held.values.len(), 4)?)?;
            if held.origin != origin
                || archived.origin != origin
                || held.source_type != row.ty
                || archived.source_type != row.ty
                || held.values != archived.values
            {
                return Err(execution_cfg_error_v29());
            }
            Ok(())
        }
        SourceReferenceNodeKindV29::Address(_) => {
            let expected = budget
                .source_reference_selected_pointer_type_v29(plan, node, row.ty)?
                .ok_or(ArgumentResourceV1::Accounting)?;
            let (
                SemanticValueBindingV1::Value {
                    id: held,
                    ty: actual,
                },
                SemanticValueBindingV1::Value {
                    id: archived,
                    ty: archived_type,
                },
            ) = (held, archived)
            else {
                return Err(execution_cfg_error_v29());
            };
            if held != archived
                || !invocation_equal_types_v1(actual, archived_type, budget)?
                || !invocation_equal_types_v1(actual, &expected, budget)?
            {
                return Err(execution_cfg_error_v29());
            }
            Ok(())
        }
        SourceReferenceNodeKindV29::Plain(_) | SourceReferenceNodeKindV29::Discriminant(_) => {
            if let SourceReferenceNodeKindV29::Plain(Some(anchor)) = row.kind
                && let Some(expected) =
                    source_reference_anchor_type_v29(plan, anchor, row.ty, budget)?
            {
                let (
                    SemanticValueBindingV1::Value {
                        id: held,
                        ty: actual,
                    },
                    SemanticValueBindingV1::Value {
                        id: archived,
                        ty: archived_type,
                    },
                ) = (held, archived)
                else {
                    return Err(execution_cfg_error_v29());
                };
                if held != archived
                    || !invocation_equal_types_v1(actual, archived_type, budget)?
                    || !invocation_equal_types_v1(actual, &expected, budget)?
                {
                    return Err(execution_cfg_error_v29());
                }
                return Ok(());
            }
            if row.descriptor.is_some() {
                let (
                    SemanticValueBindingV1::Value {
                        id: held,
                        ty: actual,
                    },
                    SemanticValueBindingV1::Value {
                        id: archived,
                        ty: archived_type,
                    },
                ) = (held, archived)
                else {
                    return Err(execution_cfg_error_v29());
                };
                budget.source_reference_reserve_v29(plan, std::mem::size_of::<Type>())?;
                let expected = source_descriptor_node_type_v29(plan, Some(node), row.ty, budget)?;
                if held != archived
                    || !invocation_equal_types_v1(actual, archived_type, budget)?
                    || !(invocation_equal_types_v1(actual, &expected, budget)?
                        || source_descriptor_widening_v29(actual, &expected))
                {
                    return Err(execution_cfg_error_v29());
                }
                return Ok(());
            }
            merge_execution_cfg_binding_v29(
                plan.instances.owner().source_semantic().types(),
                row.ty,
                held,
                archived,
                leaves,
                nodes,
                budget,
            )
            .inspect_err(|error| source_reference_record_failure_v29(plan, error))
        }
        SourceReferenceNodeKindV29::Enum { .. } => {
            let (tag, alternative) = source_reference_enum_single_v29(plan, node, budget)?;
            let (
                SemanticValueBindingV1::Enum {
                    discriminant: a,
                    discriminant_ty: at,
                    semantic_type: ay,
                    variant: av,
                    payloads: ap,
                },
                SemanticValueBindingV1::Enum {
                    discriminant: b,
                    discriminant_ty: bt,
                    semantic_type: by,
                    variant: bv,
                    payloads: bp,
                },
            ) = (held, archived)
            else {
                return Err(source_reference_enum_error_v29());
            };
            budget.source_reference_charge_v29(plan, 2)?;
            if av != bv || av.is_some_and(|variant| variant != alternative.variant) {
                return Err(source_reference_enum_error_v29());
            }
            if a != b
                || *at != tag
                || *bt != tag
                || *ay != row.ty
                || *by != row.ty
                || ap.len() != 1
                || bp.len() != 1
            {
                return Err(source_reference_enum_error_v29());
            }
            let a = ap
                .get(&alternative.variant)
                .ok_or_else(source_reference_enum_error_v29)?;
            let b = bp
                .get(&alternative.variant)
                .ok_or_else(source_reference_enum_error_v29)?;
            if a.len() != alternative.count || b.len() != alternative.count {
                return Err(source_reference_enum_error_v29());
            }
            for field in 0..alternative.count {
                budget.source_reference_charge_v29(plan, 1)?;
                let child = plan.children[argument_sum_v1(&[alternative.first, field])?];
                source_reference_merge_node_v29(
                    references, child, &a[field], &b[field], leaves, nodes, budget,
                )?;
            }
            Ok(())
        }
        SourceReferenceNodeKindV29::Aggregate { first, count } => {
            let (
                SemanticValueBindingV1::Aggregate(held),
                SemanticValueBindingV1::Aggregate(archived),
            ) = (held, archived)
            else {
                return Err(execution_cfg_error_v29());
            };
            if held.len() != count || archived.len() != count {
                return Err(execution_cfg_error_v29());
            }
            for index in 0..count {
                budget.source_reference_charge_v29(plan, 1)?;
                let child = *plan
                    .children
                    .get(argument_sum_v1(&[first, index])?)
                    .ok_or_else(execution_cfg_error_v29)?;
                if child >= node {
                    return Err(execution_cfg_error_v29());
                }
                source_reference_merge_node_v29(
                    references,
                    child,
                    &held[index],
                    &archived[index],
                    leaves,
                    nodes,
                    budget,
                )?;
            }
            Ok(())
        }
    }
}

fn source_reference_values_v29(
    references: &SourceReferenceEmissionV29<'_, '_>,
    binding: &SemanticValueBindingV1,
    output: &mut Vec<ValueDef>,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let plan = references.plan;
    references.check(budget)?;
    execution_cfg_charge_node_v29(nodes, budget)
        .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
    match binding {
        SemanticValueBindingV1::Enum {
            discriminant,
            discriminant_ty,
            payloads,
            ..
        } => {
            let ty = execution_cfg_clone_type_v29(discriminant_ty, budget)?;
            source_reference_owned_push_v29(
                plan,
                output,
                ValueDef::new(*discriminant, ty),
                budget,
            )?;
            for fields in payloads.values() {
                budget.source_reference_charge_v29(plan, 1)?;
                for field in fields {
                    source_reference_values_v29(references, field, output, nodes, budget)?;
                }
            }
            Ok(())
        }
        SemanticValueBindingV1::SourceEnumTag(binding) => {
            validate_source_enum_tag_v55(plan, binding, budget)?;
            let ty = execution_cfg_clone_type_v29(&binding.tag.ty, budget)?;
            source_reference_owned_push_v29(plan, output, ValueDef::new(binding.tag.id, ty), budget)
        }
        SemanticValueBindingV1::SourceInactive(binding) => {
            source_reference_validate_inactive_v29(plan, binding, nodes, budget)?;
            for value in &binding.values {
                budget.source_reference_charge_v29(plan, 1)?;
                let ty = execution_cfg_clone_type_v29(&value.ty, budget)
                    .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
                source_reference_owned_push_v29(plan, output, ValueDef::new(value.id, ty), budget)?;
            }
            Ok(())
        }
        SemanticValueBindingV1::SourceReference(binding) => {
            source_reference_validate_binding_v29(references.plan, binding, budget)?;
            for value in &binding.values {
                budget.source_reference_charge_v29(plan, 1)?;
                let ty = execution_cfg_clone_type_v29(&value.ty, budget)
                    .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
                source_reference_owned_push_v29(plan, output, ValueDef::new(value.id, ty), budget)?;
            }
            Ok(())
        }
        SemanticValueBindingV1::Aggregate(fields) => {
            for field in fields {
                source_reference_values_v29(references, field, output, nodes, budget)?;
            }
            Ok(())
        }
        _ => execution_cfg_values_v29(binding, output, nodes, budget)
            .inspect_err(|error| source_reference_record_failure_v29(plan, error)),
    }
}

fn source_reference_call_shape_v29(
    references: &SourceReferenceEmissionV29<'_, '_>,
    node: usize,
    binding: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(Vec<Option<ExecutionCfgLeafV29>>, Vec<Option<ValueId>>), ProductionSemanticKirErrorV1>
{
    let plan = references.plan;
    references.check(budget)?;
    source_reference_owned_prepay_v29::<(Vec<Option<ExecutionCfgLeafV29>>, Vec<Option<ValueId>>)>(
        plan, budget,
    )?;
    let ty = plan
        .nodes
        .get(node)
        .ok_or_else(execution_call_error_v29)?
        .ty;
    let count = execution_cfg_nominal_count_v29(
        plan.instances.owner().source_semantic().types(),
        ty,
        budget,
    )
    .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
    let mut leaves = source_reference_owned_vec_v29(plan, count, budget)?;
    budget.source_reference_charge_v29(plan, count)?;
    leaves.resize_with(count, || None);
    let mut slots = leaves.iter_mut();
    source_reference_merge_node_v29(
        references, node, binding, binding, &mut slots, &mut 0, budget,
    )?;
    if slots.next().is_some() {
        return Err(execution_call_error_v29());
    }
    let mut values = source_reference_owned_vec_v29(plan, 0, budget)?;
    source_reference_values_v29(references, binding, &mut values, &mut 0, budget)?;
    let expected = source_reference_node_types_v29(plan, node, budget)?;
    budget.source_reference_charge_v29(plan, expected.len())?;
    if values.len() != expected.len()
        || values.iter().zip(&expected).any(|(value, ty)| {
            &value.ty != ty
                && !index_and_u64_are_transport_equivalent(&value.ty, ty)
                && !source_descriptor_widening_v29(&value.ty, ty)
        })
    {
        return Err(execution_call_error_v29());
    }
    let mut pending = source_reference_owned_vec_v29(plan, 1, budget)?;
    let mut exact_ids = source_reference_owned_vec_v29(plan, values.len(), budget)?;
    pending.push(node);
    let mut index = 0;
    while let Some(node) = pending.pop() {
        budget.source_reference_charge_v29(plan, 2)?;
        let row = &plan.nodes[node];
        let (count, reference) = match row.kind {
            SourceReferenceNodeKindV29::Absent => return Err(execution_cfg_error_v29()),
            SourceReferenceNodeKindV29::Loan(loan) => (
                source_reference_payload_types_v29(plan, loan, budget)?.len(),
                true,
            ),
            SourceReferenceNodeKindV29::Address(_) => {
                budget
                    .source_reference_selected_pointer_type_v29(plan, node, row.ty)?
                    .ok_or(ArgumentResourceV1::Accounting)?;
                (1, true)
            }
            SourceReferenceNodeKindV29::Plain(_) | SourceReferenceNodeKindV29::Discriminant(_) => {
                source_reference_owned_prepay_v29::<Vec<Type>>(plan, budget)?;
                (
                    source_reference_node_types_v29(plan, node, budget)?.len(),
                    false,
                )
            }
            SourceReferenceNodeKindV29::Enum { .. } => {
                let (tag, alternative) = source_reference_enum_single_v29(plan, node, budget)?;
                let value = values.get(index).ok_or_else(execution_call_error_v29)?;
                if value.ty != tag {
                    return Err(source_reference_enum_error_v29());
                }
                exact_ids.push(None);
                index = argument_sum_v1(&[index, 1])?;
                for field in (0..alternative.count).rev() {
                    budget.source_reference_charge_v29(plan, 1)?;
                    let child = plan.children[argument_sum_v1(&[alternative.first, field])?];
                    source_reference_owned_push_v29(plan, &mut pending, child, budget)?;
                }
                continue;
            }
            SourceReferenceNodeKindV29::EnumView(_) => (
                source_reference_binding_origin_types_v29(
                    plan,
                    SourceReferenceBindingOriginV29::EnumView(node),
                    row.ty,
                    &mut 0,
                    budget,
                )?
                .len(),
                true,
            ),
            SourceReferenceNodeKindV29::Aggregate { first, count } => {
                for field in (0..count).rev() {
                    budget.source_reference_charge_v29(plan, 1)?;
                    let child = plan.children[argument_sum_v1(&[first, field])?];
                    if child >= node {
                        return Err(execution_call_error_v29());
                    }
                    source_reference_owned_push_v29(plan, &mut pending, child, budget)?;
                }
                continue;
            }
        };
        for _ in 0..count {
            budget.source_reference_charge_v29(plan, 2)?;
            let value = values.get(index).ok_or_else(execution_call_error_v29)?;
            exact_ids.push(reference.then_some(value.id));
            index = argument_sum_v1(&[index, 1])?;
        }
    }
    if index != values.len() {
        return Err(execution_call_error_v29());
    }
    Ok((leaves, exact_ids))
}

// The retained child instance selects captured enum and reference carriers.
// Plain siblings still undergo their ordinary structural shape check.
fn source_reference_call_argument_shape_v29(
    references: &SourceReferenceEmissionV29<'_, '_>,
    origin: &PreparedExecutionCallOriginV29<'_>,
    source_argument: u32,
    binding: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let plan = references.plan;
    references.check(budget)?;
    let instances = plan.instances;
    let caller = instances
        .instance(origin.occurrence.caller)
        .ok_or_else(execution_call_error_v29)?;
    budget.source_reference_charge_v29(plan, 12)?;
    if origin.ledger != budget.work_ledger_identity_v1()
        || origin.source.semantic != plan.source
        || origin.source.ssa != plan.ssa
        || origin.function != caller.function()
        || origin.source.root
            != instances
                .instance(plan.root)
                .ok_or_else(execution_call_error_v29)?
                .function()
    {
        return Err(execution_call_error_v29());
    }
    let calls = instances
        .calls(origin.occurrence.caller)
        .ok_or_else(execution_call_error_v29)?;
    budget.source_reference_charge_v29(plan, calls.len())?;
    let mut matches = calls
        .iter()
        .filter(|call| call.occurrence() == origin.occurrence);
    let incoming = matches.next().ok_or_else(execution_call_error_v29)?;
    if matches.next().is_some() {
        return Err(execution_call_error_v29());
    }
    let child = incoming.child().ok_or_else(execution_call_error_v29)?;
    let row = instances
        .instance(child)
        .ok_or_else(execution_call_error_v29)?;
    if row.function() != origin.callee
        || !std::ptr::eq(
            instances
                .incoming(child)
                .ok_or_else(execution_call_error_v29)?,
            incoming,
        )
    {
        return Err(execution_call_error_v29());
    }
    let mut has_carrier = false;
    for (local, declaration) in row.declaration().locals().iter().enumerate() {
        budget.source_reference_charge_v29(plan, 1)?;
        let argument = match declaration.role() {
            SemanticLocalRoleV1::Argument(argument)
            | SemanticLocalRoleV1::RustCallTupleField { argument, .. } => argument,
            _ => continue,
        };
        if argument != source_argument {
            continue;
        }
        let node = source_reference_entry_node_v29(
            plan,
            child,
            SemanticLocalIdV1::from_index(
                u32::try_from(local).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            ),
            None,
            budget,
        )?
        .ok_or_else(execution_call_error_v29)?;
        has_carrier |= source_call_requires_captured_carrier_v55(plan, node, budget)?;
    }
    if !has_carrier {
        return Ok(false);
    }
    if origin.projections.len() != origin.parameter_types.len() {
        return Err(execution_call_error_v29());
    }
    budget.source_reference_charge_v29(plan, origin.projections.len())?;
    let mut projections = origin
        .projections
        .iter()
        .zip(&origin.parameter_types)
        .filter(|(projection, _)| projection.source_argument == source_argument);
    for (local, declaration) in row.declaration().locals().iter().enumerate() {
        budget.source_reference_charge_v29(plan, 1)?;
        let (argument, field) = match declaration.role() {
            SemanticLocalRoleV1::Argument(argument) => (argument, None),
            SemanticLocalRoleV1::RustCallTupleField { argument, field } => (argument, Some(field)),
            _ => continue,
        };
        if argument != source_argument {
            continue;
        }
        let node = source_reference_entry_node_v29(
            plan,
            child,
            SemanticLocalIdV1::from_index(
                u32::try_from(local).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            ),
            None,
            budget,
        )?
        .ok_or_else(execution_call_error_v29)?;
        if plan.nodes[node].ty != declaration.ty() {
            return Err(execution_call_error_v29());
        }
        let selected = match (binding, field) {
            (SemanticValueBindingV1::Aggregate(fields), Some(field)) => fields
                .get(field as usize)
                .ok_or_else(execution_call_error_v29)?,
            (_, None) => binding,
            _ => return Err(execution_call_error_v29()),
        };
        source_reference_call_shape_v29(references, node, selected, budget)?;
        let physical = source_reference_node_types_v29(plan, node, budget)?;
        for (component, expected) in physical.iter().enumerate() {
            budget.source_reference_charge_v29(plan, 2)?;
            let (projection, actual) = projections.next().ok_or_else(execution_call_error_v29)?;
            if projection.tuple_field != field
                || projection.component != Some(component)
                || actual != expected
            {
                return Err(execution_call_error_v29());
            }
        }
    }
    if projections.next().is_some() {
        return Err(execution_call_error_v29());
    }
    Ok(true)
}
fn source_reference_existing_value_local_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    local: u32,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    let declaration = plan
        .instances
        .instance(instance)
        .ok_or_else(execution_cfg_error_v29)?
        .declaration()
        .locals()
        .get(local as usize)
        .ok_or_else(execution_cfg_error_v29)?;
    let mut found = false;
    for (index, loan) in plan.loans.iter().enumerate() {
        budget.source_reference_charge_v29(plan, 6)?;
        let origin = plan
            .origins
            .get(loan.origin)
            .ok_or_else(execution_cfg_error_v29)?;
        if origin.instance != instance || origin.local.index() != local {
            continue;
        }
        if !source_reference_existing_value_loan_v29(plan, index, declaration.ty(), budget)? {
            return Ok(false);
        }
        found = true;
    }
    Ok(found)
}

include!("production_source_existing_receiver_backing_v29.rs");

fn source_reference_cfg_local_types_v29(
    cursor: &ExecutionAvailabilityV29<'_>,
    local: u32,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
    let references = cursor.references.ok_or_else(execution_cfg_error_v29)?;
    let plan = references.plan;
    references.check(budget)?;
    source_reference_owned_prepay_v29::<Vec<Type>>(plan, budget)?;
    budget.source_reference_reserve_v29(
        plan,
        argument_product_v1(2, std::mem::size_of::<Option<Vec<Type>>>())?,
    )?;
    let mut selected: Option<Vec<Type>> = None;
    for entry in &cursor.cfg.entries {
        budget.source_reference_charge_v29(plan, 2)?;
        if entry.local != local {
            continue;
        }
        let node = entry.reference.ok_or_else(execution_cfg_error_v29)?;
        let types = source_reference_cfg_node_types_v29(references.plan, node, budget)?;
        if let Some(previous) = &mut selected {
            budget.source_reference_charge_v29(plan, argument_product_v1(types.len(), 4)?)?;
            if previous.len() != types.len() {
                return Err(execution_cfg_error_v29());
            }
            for (previous, ty) in previous.iter_mut().zip(&types) {
                if previous == ty {
                    continue;
                }
                // Only the summary is widened. Actual block parameters use
                // their individual C1 entry node, never this all-local join.
                match (previous, ty) {
                    (Type::Slice(a), Type::Slice(b))
                        if a.element == b.element && a.access == b.access =>
                    {
                        a.address_space = AddressSpace::Generic;
                    }
                    _ => return Err(execution_cfg_error_v29()),
                }
            }
        } else {
            selected = Some(types);
        }
    }
    selected.ok_or_else(execution_cfg_error_v29)
}
