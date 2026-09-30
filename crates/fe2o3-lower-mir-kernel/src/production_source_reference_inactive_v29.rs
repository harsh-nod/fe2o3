// Exact source removals retain physical shape, never loan or read authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceReferenceRemovalKindV29 {
    Move,
    Deinitialize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceInactiveShapeV29 {
    first: usize,
    count: usize,
}

struct SourceReferenceInactiveRemovalV29 {
    site: SourceReferenceSiteV29,
    source: usize,
    source_local: SemanticLocalIdV1,
    kind: SourceReferenceRemovalKindV29,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    ty: SemanticTypeIdV1,
    projections: std::ops::Range<usize>,
    prior: usize,
    after: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SemanticSourceInactiveBindingV29 {
    owner: usize,
    source: [u8; 32],
    ssa: fe2o3_pliron::ProductionSemanticSsaIdentityV1,
    root: ProductionCallInstanceIdV1,
    node: usize,
    source_type: SemanticTypeIdV1,
    values: Vec<ValueDef>,
}

fn source_reference_inactive_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("inactive source transport differs from its original removal shape")
}

fn source_reference_check_removal_source_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    receipt: &SourceReferenceInactiveRemovalV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    budget.source_reference_charge_v29(plan, 8)?;
    let function = plan
        .instances
        .instance(receipt.site.instance)
        .ok_or_else(source_reference_inactive_error_v29)?
        .declaration();
    let block = function
        .blocks()
        .get(receipt.site.block.index() as usize)
        .ok_or_else(source_reference_inactive_error_v29)?;
    let mut found = 0;
    let mut operand = |operand: &SemanticOperandV1| -> Result<(), ProductionSemanticKirErrorV1> {
        budget.source_reference_charge_v29(plan, 2)?;
        if let SemanticOperandV1::Move(place) = operand
            && place as *const SemanticPlaceV1 as usize == receipt.source
        {
            if receipt.kind != SourceReferenceRemovalKindV29::Move
                || place.local() != receipt.source_local
                || place.ty() != receipt.ty
            {
                return Err(source_reference_inactive_error_v29());
            }
            found = argument_sum_v1(&[found, 1])?;
        }
        Ok(())
    };
    if let Some(index) = receipt.site.statement {
        let statement = block
            .statements()
            .get(index)
            .ok_or_else(source_reference_inactive_error_v29)?;
        match statement.kind() {
            SemanticStatementKindV1::Assign(assignment) => {
                assignment.value().kind().try_visit_operands(&mut operand)?
            }
            SemanticStatementKindV1::Store(store) => operand(store.value())?,
            SemanticStatementKindV1::Deinitialize(place) => {
                if receipt.kind == SourceReferenceRemovalKindV29::Deinitialize
                    && place as *const SemanticPlaceV1 as usize == receipt.source
                    && place.local() == receipt.source_local
                    && place.ty() == receipt.ty
                {
                    found = 1;
                }
            }
            _ => {}
        }
    } else {
        use fe2o3_mir_model::semantic_mir_v1::SemanticAssertMessageV1 as Message;
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => {
                for argument in call.arguments() {
                    operand(argument)?;
                }
            }
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => operand(discriminant)?,
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                operand(condition)?;
                match message {
                    Message::BoundsCheck { length, index }
                    | Message::Overflow {
                        left: length,
                        right: index,
                        ..
                    }
                    | Message::MisalignedPointerDereference {
                        required_alignment: length,
                        found_alignment: index,
                    } => {
                        operand(length)?;
                        operand(index)?;
                    }
                    Message::DivisionByZero(value) | Message::RemainderByZero(value) => {
                        operand(value)?
                    }
                    Message::NullPointerDereference
                    | Message::ResumedAfterReturn
                    | Message::ResumedAfterPanic => {}
                }
            }
            _ => {}
        }
    }
    if found != 1 {
        return Err(source_reference_inactive_error_v29());
    }
    Ok(())
}

fn source_reference_inactive_choices_v29<'a>(
    plan: &'a SourceReferencePlanV29<'_, '_>,
    node: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<&'a [usize], ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    budget.source_reference_charge_v29(plan, 3)?;
    let row = plan
        .nodes
        .get(node)
        .ok_or_else(source_reference_inactive_error_v29)?;
    if row.kind != SourceReferenceNodeKindV29::Absent {
        return Err(source_reference_inactive_error_v29());
    }
    let shape = row
        .inactive
        .ok_or_else(source_reference_inactive_error_v29)?;
    let choices = plan
        .inactive_choices
        .get(shape.first..argument_sum_v1(&[shape.first, shape.count])?)
        .ok_or_else(source_reference_inactive_error_v29)?;
    if choices.is_empty() {
        return Err(source_reference_inactive_error_v29());
    }
    budget.source_reference_charge_v29(plan, choices.len())?;
    if choices.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(source_reference_inactive_error_v29());
    }
    Ok(choices)
}

fn source_reference_check_inactive_type_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    receipt: &SourceReferenceInactiveRemovalV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source_reference_check_removal_source_v29(plan, receipt, budget)?;
    budget.source_reference_charge_v29(plan, 5)?;
    let source = plan.instances.owner().source_semantic();
    let local = plan
        .instances
        .instance(receipt.instance)
        .and_then(|instance| {
            instance
                .declaration()
                .locals()
                .get(receipt.local.index() as usize)
        })
        .ok_or_else(source_reference_inactive_error_v29)?;
    let mut ty = local.ty();
    let projections = plan
        .projections
        .get(receipt.projections.clone())
        .ok_or_else(source_reference_inactive_error_v29)?;
    if projections.is_empty()
        || receipt.prior >= node
        || plan.nodes.get(receipt.prior).map(|prior| prior.ty) != Some(receipt.ty)
        || plan.nodes.get(node).map(|row| row.ty) != Some(receipt.ty)
        || plan.storage_snapshots.get(receipt.after).is_none()
    {
        return Err(source_reference_inactive_error_v29());
    }
    let mut active_variant = None;
    for projection in projections {
        budget.source_reference_charge_v29(plan, 4)?;
        let shape = source
            .types()
            .get(ty.index() as usize)
            .ok_or_else(source_reference_inactive_error_v29)?
            .shape();
        ty = match (shape, projection.kind()) {
            (
                SemanticTypeShapeV1::Enum { variants, .. },
                SemanticProjectionKindV1::Downcast(variant),
            ) if active_variant.is_none() => {
                if variants
                    .get(variant as usize)
                    .is_none_or(|variant| variant.is_uninhabited())
                {
                    return Err(source_reference_inactive_error_v29());
                }
                active_variant = Some(variant);
                ty
            }
            (
                SemanticTypeShapeV1::Enum { variants, .. },
                SemanticProjectionKindV1::Field(field),
            ) => {
                let variant = active_variant
                    .take()
                    .ok_or_else(source_reference_inactive_error_v29)?;
                *variants
                    .get(variant as usize)
                    .and_then(|variant| variant.fields().fields().get(field as usize))
                    .ok_or_else(source_reference_inactive_error_v29)?
            }
            (
                SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
                SemanticProjectionKindV1::Field(field),
            ) => *fields
                .fields()
                .get(field as usize)
                .ok_or_else(source_reference_inactive_error_v29)?,
            (
                SemanticTypeShapeV1::Array { element, length },
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length,
                    from_end,
                },
            ) => {
                if minimum_length > *length
                    || (from_end && (offset == 0 || offset > *length))
                    || (!from_end && offset >= *length)
                {
                    return Err(source_reference_inactive_error_v29());
                }
                *element
            }
            _ => return Err(source_reference_inactive_error_v29()),
        };
        if ty != projection.result_type() {
            return Err(source_reference_inactive_error_v29());
        }
    }
    if active_variant.is_some() || ty != receipt.ty {
        return Err(source_reference_inactive_error_v29());
    }
    // Nominal identities have their own source proof and cannot be made inert
    // by the ordinary reference representation planner.
    if execution_cfg_nominal_count_v29(source.types(), ty, budget)
        .inspect_err(|error| source_reference_record_failure_v29(plan, error))?
        != 0
    {
        return Err(source_reference_inactive_error_v29());
    }
    Ok(())
}

fn source_reference_inactive_payload_types_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
    let choices = source_reference_inactive_choices_v29(plan, node, budget)?;
    source_reference_owned_prepay_v29::<Option<Vec<Type>>>(plan, budget)?;
    let mut expected: Option<Vec<Type>> = None;
    for &choice in choices {
        execution_cfg_charge_node_v29(nodes, budget)
            .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
        let receipt = plan
            .inactive_removals
            .get(choice)
            .ok_or_else(source_reference_inactive_error_v29)?;
        source_reference_check_inactive_type_v29(plan, node, receipt, budget)?;
        let mut types = source_reference_owned_vec_v29(plan, 0, budget)?;
        source_reference_append_node_types_v29(
            plan,
            receipt.prior,
            true,
            &mut types,
            nodes,
            budget,
        )?;
        if let Some(expected) = &expected {
            budget.source_reference_charge_v29(
                plan,
                argument_sum_v1(&[expected.len().min(types.len()), 1])?,
            )?;
            let mut equal = expected.len() == types.len();
            for (left, right) in expected.iter().zip(&types) {
                equal &= invocation_equal_types_v1(left, right, budget)
                    .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
            }
            if !equal {
                return Err(source_reference_error_v29(
                    "inactive source alternatives require a common checked backing representation",
                ));
            }
        } else {
            expected = Some(types);
        }
    }
    expected.ok_or_else(source_reference_inactive_error_v29)
}

fn source_reference_validate_inactive_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    binding: &SemanticSourceInactiveBindingV29,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    budget.source_reference_charge_v29(plan, 8)?;
    if binding.owner != plan as *const SourceReferencePlanV29<'_, '_> as usize
        || binding.source != plan.source
        || binding.ssa != plan.ssa
        || binding.root != plan.root
        || plan.nodes.get(binding.node).map(|row| row.ty) != Some(binding.source_type)
    {
        return Err(source_reference_inactive_error_v29());
    }
    let expected = source_reference_inactive_payload_types_v29(plan, binding.node, nodes, budget)?;
    budget.source_reference_charge_v29(plan, argument_sum_v1(&[expected.len(), 1])?)?;
    if !source_reference_inactive_payload_matches_v29(&binding.values, &expected, budget)
        .inspect_err(|error| source_reference_record_failure_v29(plan, error))?
    {
        return Err(source_reference_inactive_error_v29());
    }
    Ok(())
}

fn source_reference_inactive_payload_matches_v29(
    values: &[ValueDef],
    expected: &[Type],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    if values.len() != expected.len() {
        return Ok(false);
    }
    for (value, ty) in values.iter().zip(expected) {
        if !invocation_equal_types_v1(&value.ty, ty, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn source_reference_inactive_values_same_v29(
    left: &[ValueDef],
    right: &[ValueDef],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left.iter().zip(right) {
        budget.charge_work(1)?;
        if left.id != right.id || !invocation_equal_types_v1(&left.ty, &right.ty, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn source_reference_inactive_subset_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    subset: usize,
    target: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let a = source_reference_inactive_choices_v29(plan, subset, budget)?;
    let b = source_reference_inactive_choices_v29(plan, target, budget)?;
    budget.source_reference_charge_v29(plan, argument_sum_v1(&[a.len(), b.len()])?)?;
    let mut index = 0;
    for &choice in a {
        while index < b.len() && b[index] < choice {
            index += 1;
        }
        if b.get(index) != Some(&choice) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn source_reference_inactive_shape_matches_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    binding: &SemanticValueBindingV1,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    execution_cfg_charge_node_v29(nodes, budget)
        .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
    let row = plan
        .nodes
        .get(node)
        .ok_or_else(source_reference_inactive_error_v29)?;
    match (&row.kind, binding) {
        (SourceReferenceNodeKindV29::Absent, SemanticValueBindingV1::SourceInactive(binding)) => {
            source_reference_validate_inactive_v29(plan, binding, nodes, budget)?;
            source_reference_inactive_subset_v29(plan, binding.node, node, budget)
        }
        (SourceReferenceNodeKindV29::Absent, _) => {
            for &choice in source_reference_inactive_choices_v29(plan, node, budget)? {
                let receipt = plan
                    .inactive_removals
                    .get(choice)
                    .ok_or_else(source_reference_inactive_error_v29)?;
                source_reference_check_inactive_type_v29(plan, node, receipt, budget)?;
                if source_reference_inactive_shape_matches_v29(
                    plan,
                    receipt.prior,
                    binding,
                    nodes,
                    budget,
                )? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        (
            SourceReferenceNodeKindV29::Loan(loan),
            SemanticValueBindingV1::SourceReference(binding),
        ) => {
            source_reference_validate_binding_v29(plan, binding, budget)?;
            Ok(
                binding.origin == SourceReferenceBindingOriginV29::SingleLoan(*loan)
                    && binding.source_type == row.ty,
            )
        }
        (
            SourceReferenceNodeKindV29::Aggregate { first, count },
            SemanticValueBindingV1::Aggregate(fields),
        ) => {
            if fields.len() != *count {
                return Ok(false);
            }
            for (offset, field) in fields.iter().enumerate() {
                budget.source_reference_charge_v29(plan, 1)?;
                let child = *plan
                    .children
                    .get(argument_sum_v1(&[*first, offset])?)
                    .ok_or_else(source_reference_inactive_error_v29)?;
                if child >= node {
                    return Err(source_reference_inactive_error_v29());
                }
                if !source_reference_inactive_shape_matches_v29(plan, child, field, nodes, budget)?
                {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (
            SourceReferenceNodeKindV29::Plain(_)
            | SourceReferenceNodeKindV29::Address(_)
            | SourceReferenceNodeKindV29::Discriminant(_),
            _,
        ) => {
            if execution_binding_contains_paid_v29(binding, budget)
                .inspect_err(|error| source_reference_record_failure_v29(plan, error))?
            {
                return Ok(false);
            }
            let expected = source_reference_node_types_v29(plan, node, budget)?;
            let mut values = source_reference_owned_vec_v29(plan, 0, budget)?;
            execution_cfg_values_v29(binding, &mut values, nodes, budget)
                .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
            budget.source_reference_charge_v29(plan, argument_sum_v1(&[expected.len(), 1])?)?;
            source_reference_inactive_payload_matches_v29(&values, &expected, budget)
                .inspect_err(|error| source_reference_record_failure_v29(plan, error))
        }
        _ => Ok(false),
    }
}

fn source_reference_merge_inactive_v29(
    references: &SourceReferenceEmissionV29<'_, '_>,
    node: usize,
    held: &SemanticValueBindingV1,
    archived: &SemanticValueBindingV1,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let plan = references.plan;
    let expected = source_reference_inactive_payload_types_v29(plan, node, nodes, budget)?;
    if !source_reference_inactive_shape_matches_v29(plan, node, held, nodes, budget)?
        || !source_reference_inactive_shape_matches_v29(plan, node, archived, nodes, budget)?
    {
        return Err(source_reference_inactive_error_v29());
    }
    let mut a = source_reference_owned_vec_v29(plan, expected.len(), budget)?;
    let mut b = source_reference_owned_vec_v29(plan, expected.len(), budget)?;
    source_reference_values_v29(references, held, &mut a, nodes, budget)?;
    source_reference_values_v29(references, archived, &mut b, nodes, budget)?;
    budget.source_reference_charge_v29(
        plan,
        argument_sum_v1(&[argument_product_v1(expected.len(), 4)?, 2])?,
    )?;
    if !source_reference_inactive_values_same_v29(&a, &b, budget)
        .inspect_err(|error| source_reference_record_failure_v29(plan, error))?
        || !source_reference_inactive_payload_matches_v29(&a, &expected, budget)
            .inspect_err(|error| source_reference_record_failure_v29(plan, error))?
    {
        return Err(source_reference_inactive_error_v29());
    }
    Ok(())
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn return_storage_value(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        node: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let Some(snapshot) = self.local(instance, local)?.storage else {
            return Ok(node);
        };
        let root = self
            .storage_root
            .as_ref()
            .ok_or_else(source_reference_inactive_error_v29)?;
        if !root
            .snapshot_initialized(self.storage_snapshot(snapshot)?, &[], budget)
            .map_err(|error| self.storage_error(error))?
        {
            return Err(source_reference_error_v29(
                "source return value is not definitely initialized",
            ));
        }
        let mut value = self.plan.nodes[node];
        budget.charge_work(1)?;
        value.value_origin = Some(value.value_origin.unwrap_or(node));
        value.storage = Some(SourceReferenceValueStorageV29 {
            snapshot,
            first: self.plan.projections.len(),
            count: 0,
            selector_source: None,
        });
        let result = self.plan.nodes.len();
        emission_push_v1(&mut self.plan.nodes, value, budget)?;
        Ok(result)
    }

    fn publish_inactive_node(
        &mut self,
        ty: SemanticTypeIdV1,
        choices: &[usize],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        for (index, node) in self.plan.nodes.iter().enumerate() {
            budget.charge_work(2)?;
            if node.ty != ty || node.kind != SourceReferenceNodeKindV29::Absent {
                continue;
            }
            let shape = node
                .inactive
                .ok_or_else(source_reference_inactive_error_v29)?;
            budget.charge_work(argument_sum_v1(&[shape.count.min(choices.len()), 1])?)?;
            let existing = self
                .plan
                .inactive_choices
                .get(shape.first..argument_sum_v1(&[shape.first, shape.count])?)
                .ok_or_else(source_reference_inactive_error_v29)?;
            if existing == choices {
                return Ok(index);
            }
        }
        let first = self.plan.inactive_choices.len();
        for &choice in choices {
            emission_push_v1(&mut self.plan.inactive_choices, choice, budget)?;
        }
        let node = self.node(ty, SourceReferenceNodeKindV29::Absent, budget)?;
        self.plan.nodes[node].inactive = Some(SourceReferenceInactiveShapeV29 {
            first,
            count: choices.len(),
        });
        Ok(node)
    }

    fn inactive_sets_equal(
        &self,
        left: Option<SourceReferenceInactiveShapeV29>,
        right: Option<SourceReferenceInactiveShapeV29>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let (Some(left), Some(right)) = (left, right) else {
            return Err(source_reference_inactive_error_v29());
        };
        budget.charge_work(argument_sum_v1(&[left.count.min(right.count), 1])?)?;
        let a = self
            .plan
            .inactive_choices
            .get(left.first..argument_sum_v1(&[left.first, left.count])?)
            .ok_or_else(source_reference_inactive_error_v29)?;
        let b = self
            .plan
            .inactive_choices
            .get(right.first..argument_sum_v1(&[right.first, right.count])?)
            .ok_or_else(source_reference_inactive_error_v29)?;
        Ok(a == b)
    }

    fn inactive_prior_shapes_equal(
        &self,
        left: usize,
        right: usize,
        depth: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        if depth >= 256 {
            return Err(source_reference_inactive_error_v29());
        }
        let a = self
            .plan
            .nodes
            .get(left)
            .ok_or_else(source_reference_inactive_error_v29)?;
        let b = self
            .plan
            .nodes
            .get(right)
            .ok_or_else(source_reference_inactive_error_v29)?;
        if a.ty != b.ty {
            return Ok(false);
        }
        match (a.kind, b.kind) {
            (
                SourceReferenceNodeKindV29::Discriminant(_),
                SourceReferenceNodeKindV29::Discriminant(_) | SourceReferenceNodeKindV29::Plain(_),
            )
            | (SourceReferenceNodeKindV29::Plain(_), SourceReferenceNodeKindV29::Discriminant(_)) => {
                Ok(true)
            }
            (SourceReferenceNodeKindV29::Enum { .. }, SourceReferenceNodeKindV29::Enum { .. })
            | (SourceReferenceNodeKindV29::EnumView(_), SourceReferenceNodeKindV29::EnumView(_)) => {
                self.nodes_equal(left, right, depth, budget)
            }
            (SourceReferenceNodeKindV29::Plain(_), SourceReferenceNodeKindV29::Plain(_)) => {
                Ok(true)
            }
            (SourceReferenceNodeKindV29::Address(_), SourceReferenceNodeKindV29::Address(_)) => {
                Ok(true)
            }
            (SourceReferenceNodeKindV29::Loan(a), SourceReferenceNodeKindV29::Loan(b)) => {
                Ok(a == b)
            }
            (SourceReferenceNodeKindV29::Absent, SourceReferenceNodeKindV29::Absent) => {
                self.inactive_sets_equal(a.inactive, b.inactive, budget)
            }
            (
                SourceReferenceNodeKindV29::Aggregate { first: a, count },
                SourceReferenceNodeKindV29::Aggregate {
                    first: b,
                    count: other,
                },
            ) if count == other => {
                for offset in 0..count {
                    budget.charge_work(2)?;
                    let a = *self
                        .plan
                        .children
                        .get(argument_sum_v1(&[a, offset])?)
                        .ok_or_else(source_reference_inactive_error_v29)?;
                    let b = *self
                        .plan
                        .children
                        .get(argument_sum_v1(&[b, offset])?)
                        .ok_or_else(source_reference_inactive_error_v29)?;
                    if a >= left || b >= right {
                        return Err(source_reference_inactive_error_v29());
                    }
                    if !self.inactive_prior_shapes_equal(a, b, depth + 1, budget)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn inactive_node(
        &mut self,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        kind: SourceReferenceRemovalKindV29,
        target: &SourceReferencePlaceV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
        let prior = *self
            .plan
            .nodes
            .get(target.node)
            .ok_or_else(source_reference_inactive_error_v29)?;
        if prior.ty != source.ty() {
            return Err(source_reference_inactive_error_v29());
        }
        if prior.kind == SourceReferenceNodeKindV29::Absent {
            if prior.inactive.is_none() {
                return Err(source_reference_inactive_error_v29());
            }
            return Ok(target.node);
        }
        let after = self
            .local(target.instance, target.local)?
            .storage
            .ok_or_else(source_reference_inactive_error_v29)?;
        let root = self
            .storage_root
            .as_ref()
            .ok_or_else(source_reference_inactive_error_v29)?;
        if root
            .snapshot_initialized(self.storage_snapshot(after)?, &target.projections, budget)
            .map_err(|error| self.storage_error(error))?
        {
            return Err(source_reference_inactive_error_v29());
        }
        let mut existing = None;
        for (index, receipt) in self.plan.inactive_removals.iter().enumerate() {
            budget.charge_work(argument_sum_v1(&[target.projections.len(), 10])?)?;
            if receipt.site == site
                && receipt.source == source as *const SemanticPlaceV1 as usize
                && receipt.kind == kind
                && receipt.instance == target.instance
                && receipt.local == target.local
                && receipt.generation == target.generation
                && receipt.ty == prior.ty
                && self.plan.projections[receipt.projections.clone()] == target.projections
                && self.inactive_prior_shapes_equal(receipt.prior, target.node, 0, budget)?
            {
                existing = Some(index);
                break;
            }
        }
        let receipt = if let Some(index) = existing {
            index
        } else {
            budget.reserve_storage(std::mem::size_of::<SourceReferenceInactiveRemovalV29>())?;
            let first = self.plan.projections.len();
            for &projection in &target.projections {
                emission_push_v1(&mut self.plan.projections, projection, budget)?;
            }
            let receipt = SourceReferenceInactiveRemovalV29 {
                site,
                source: source as *const SemanticPlaceV1 as usize,
                source_local: source.local(),
                kind,
                instance: target.instance,
                local: target.local,
                generation: target.generation,
                ty: prior.ty,
                projections: first..self.plan.projections.len(),
                prior: target.node,
                after,
            };
            source_reference_check_removal_source_v29(&self.plan, &receipt, budget)?;
            let index = self.plan.inactive_removals.len();
            emission_push_v1(&mut self.plan.inactive_removals, receipt, budget)?;
            index
        };
        self.publish_inactive_node(prior.ty, &[receipt], budget)
    }

    fn merge_inactive_nodes(
        &mut self,
        left: usize,
        right: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let a = self.plan.nodes[left]
            .inactive
            .ok_or_else(source_reference_inactive_error_v29)?;
        let b = self.plan.nodes[right]
            .inactive
            .ok_or_else(source_reference_inactive_error_v29)?;
        if self.inactive_sets_equal(Some(a), Some(b), budget)? {
            return Ok(left);
        }
        let mut choices =
            source_reference_scratch_v29(argument_sum_v1(&[a.count, b.count])?, budget)?;
        let (mut ai, mut bi) = (0, 0);
        while ai < a.count || bi < b.count {
            budget.charge_work(4)?;
            let av = (ai < a.count).then(|| self.plan.inactive_choices[a.first + ai]);
            let bv = (bi < b.count).then(|| self.plan.inactive_choices[b.first + bi]);
            let value = match (av, bv) {
                (Some(x), Some(y)) if x == y => {
                    ai += 1;
                    bi += 1;
                    x
                }
                (Some(x), Some(y)) if x < y => {
                    ai += 1;
                    x
                }
                (Some(_), Some(y)) => {
                    bi += 1;
                    y
                }
                (Some(x), None) => {
                    ai += 1;
                    x
                }
                (None, Some(y)) => {
                    bi += 1;
                    y
                }
                (None, None) => return Err(source_reference_inactive_error_v29()),
            };
            choices.push(value);
        }
        let temporary = argument_sum_v1(&[
            std::mem::size_of::<Vec<usize>>(),
            argument_product_v1(choices.capacity(), std::mem::size_of::<usize>())?,
        ])?;
        let result = self.publish_inactive_node(self.plan.nodes[left].ty, &choices, budget);
        drop(choices);
        if result.is_ok() {
            budget.release_storage(temporary)?;
        }
        result
    }
}
