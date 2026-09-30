// Original reference words validate the captured Rust ABI only. They never
// become KIR pointer/word parameters; physical payload comes from the same loan.
fn source_reference_abi_scalar_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    loan: usize,
    ty: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SemanticBackendScalarV1, ProductionSemanticKirErrorV1> {
    if plan.backing_cell(loan, budget)?.is_none() {
        plan.require_promoted(loan, budget)?;
    }
    budget.source_reference_charge_v29(plan, 20)?;
    let loan = plan.loans.get(loan).ok_or_else(execution_call_error_v29)?;
    let origin = &plan.origins[loan.origin];
    let declaration = &plan.instances.owner().source_semantic().types()[ty.index() as usize];
    let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
        return Err(execution_call_error_v29());
    };
    let SemanticBackendReprV1::Scalar(scalar) = declaration.layout().backend_repr() else {
        return Err(execution_call_error_v29());
    };
    let mutability = match loan.kind {
        SemanticBorrowKindV1::Shared => SemanticMutabilityV1::Immutable,
        SemanticBorrowKindV1::Mutable => SemanticMutabilityV1::Mutable,
        SemanticBorrowKindV1::Fake => return Err(execution_call_error_v29()),
    };
    if ty != loan.source_type
        || pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.pointee() != origin.ty
        || pointer.mutability() != mutability
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || pointer.metadata() != SemanticPointerMetadataV1::None
        || declaration.layout().size_bytes() != Some(8)
        || declaration.layout().alignment_bytes() != 8
        || declaration.layout().is_uninhabited()
        || scalar.primitive() != SemanticBackendPrimitiveV1::pointer(0, 8, 8)
    {
        return Err(execution_call_error_v29());
    }
    Ok(*scalar)
}

// ABI metadata validates only the captured source word, never pointer authority.
// The caller must independently authenticate the selected address and schema.
fn source_reference_raw_abi_scalar_v29(
    declaration: &SemanticTypeDeclV1,
) -> Result<SemanticBackendScalarV1, ProductionSemanticKirErrorV1> {
    let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
        return Err(execution_call_error_v29());
    };
    let SemanticBackendReprV1::Scalar(scalar) = declaration.layout().backend_repr() else {
        return Err(execution_call_error_v29());
    };
    if pointer.kind() != SemanticPointerKindV1::Raw
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || pointer.metadata() != SemanticPointerMetadataV1::None
        || declaration.layout().size_bytes() != Some(8)
        || declaration.layout().alignment_bytes() != 8
        || declaration.layout().is_uninhabited()
        || scalar.primitive() != SemanticBackendPrimitiveV1::pointer(0, 8, 8)
    {
        return Err(execution_call_error_v29());
    }
    Ok(*scalar)
}

fn source_reference_selected_abi_scalar_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    ty: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SemanticBackendScalarV1, ProductionSemanticKirErrorV1> {
    let result = (|| {
        if source_reference_selected_pointer_type_v29(plan, node, ty, budget)?.is_none() {
            return Err(execution_call_error_v29());
        }
        budget.source_reference_charge_v29(plan, 12)?;
        let declaration = plan
            .instances
            .owner()
            .source_semantic()
            .types()
            .get(ty.index() as usize)
            .ok_or_else(execution_call_error_v29)?;
        source_reference_raw_abi_scalar_v29(declaration)
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_reference_child_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: Option<usize>,
    field: usize,
    count: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
    budget.source_reference_charge_v29(plan, 2)?;
    match node.map(|node| plan.nodes[node].kind) {
        None
        | Some(SourceReferenceNodeKindV29::Plain(_) | SourceReferenceNodeKindV29::Address(_)) => {
            Ok(None)
        }
        Some(SourceReferenceNodeKindV29::Aggregate {
            first,
            count: actual,
        }) if actual == count => plan
            .children
            .get(argument_sum_v1(&[first, field])?)
            .copied()
            .map(Some)
            .ok_or_else(execution_call_error_v29),
        _ => Err(execution_call_error_v29()),
    }
}

#[allow(clippy::too_many_arguments)]
fn source_reference_abi_words_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: Option<usize>,
    ty: SemanticTypeIdV1,
    offset: u64,
    path: &mut Vec<SemanticKirParameterProjectionV1>,
    output: &mut Vec<ByValueKernelParameterComponentV1>,
    nodes: &mut usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.source_reference_charge_v29(plan, 8)?;
    *nodes = argument_sum_v1(&[*nodes, 1])?;
    if *nodes > MAX_SSA_VALUE_COMPONENTS_V1 || path.len() >= 256 {
        return Err(source_reference_error_v29(
            "source reference ABI structure exceeds its bound",
        ));
    }
    let types = plan.instances.owner().source_semantic().types();
    let declaration = types
        .get(ty.index() as usize)
        .ok_or_else(execution_call_error_v29)?;
    if node.is_some_and(|node| plan.nodes.get(node).map(|node| node.ty) != Some(ty))
        || declaration.layout().is_uninhabited()
        || declaration.layout().size_bytes().is_none()
    {
        return Err(execution_call_error_v29());
    }
    let tracked = node.and_then(|node| match plan.nodes[node].kind {
        SourceReferenceNodeKindV29::Loan(loan) => Some(loan),
        _ => None,
    });
    if node.is_some_and(|node| plan.nodes[node].kind == SourceReferenceNodeKindV29::Absent) {
        return Err(source_reference_error_v29(
            "source reference ABI cannot materialize an absent holder leaf",
        ));
    }
    let leaf = if let Some(loan) = tracked {
        Some((
            Type::Unit,
            ParameterAbiLeafV1::Scalar(source_reference_abi_scalar_v29(plan, loan, ty, budget)?),
        ))
    } else if let Some(node) = node
        && matches!(
            plan.nodes[node].kind,
            SourceReferenceNodeKindV29::Address(_)
        )
    {
        Some((
            Type::Unit,
            ParameterAbiLeafV1::Scalar(source_reference_selected_abi_scalar_v29(
                plan, node, ty, budget,
            )?),
        ))
    } else {
        match declaration.shape() {
            SemanticTypeShapeV1::Unit => None,
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
                let SemanticBackendReprV1::Scalar(scalar) = declaration.layout().backend_repr()
                else {
                    return Err(execution_call_error_v29());
                };
                Some((
                    lower_scalar_type(types, ty)?,
                    ParameterAbiLeafV1::Scalar(*scalar),
                ))
            }
            SemanticTypeShapeV1::Pointer(_)
                if execution_cfg_nominal_kind_v29(types, ty)? == Some(true) =>
            {
                Some((
                    Type::Unit,
                    ParameterAbiLeafV1::Scalar(execution_reference_abi_scalar_v29(types, ty)?),
                ))
            }
            SemanticTypeShapeV1::Pointer(_) if shared_slice_leaf_v1(types, ty) => {
                let SemanticBackendReprV1::ScalarPair { first, second } =
                    declaration.layout().backend_repr()
                else {
                    return Err(execution_call_error_v29());
                };
                budget.source_reference_reserve_v29(plan, std::mem::size_of::<Type>())?;
                Some((
                    source_descriptor_node_type_v29(plan, node, ty, budget)?,
                    ParameterAbiLeafV1::SharedSlicePair {
                        first: *first,
                        second: *second,
                    },
                ))
            }
            SemanticTypeShapeV1::Pointer(_) => {
                return Err(source_reference_error_v29(
                    "embedded pointer kernel arguments have no owned region binding",
                ));
            }
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details()
                else {
                    return Err(execution_call_error_v29());
                };
                if fields.fields().len() != layout.field_offsets().len() {
                    return Err(execution_call_error_v29());
                }
                for (index, &field) in fields.fields().iter().enumerate() {
                    budget.source_reference_charge_v29(plan, 2)?;
                    let child = source_reference_child_v29(
                        plan,
                        node,
                        index,
                        fields.fields().len(),
                        budget,
                    )?;
                    source_reference_owned_push_v29(
                        plan,
                        path,
                        SemanticKirParameterProjectionV1::Field(
                            u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        ),
                        budget,
                    )?;
                    source_reference_abi_words_v29(
                        plan,
                        child,
                        field,
                        offset
                            .checked_add(layout.field_offsets()[index])
                            .ok_or(ArgumentResourceV1::Arithmetic)?,
                        path,
                        output,
                        nodes,
                        budget,
                    )?;
                    path.pop();
                }
                None
            }
            SemanticTypeShapeV1::Array { element, length } => {
                let SemanticFieldsShapeV1::Array {
                    stride_bytes,
                    count,
                } = declaration.layout().fields()
                else {
                    return Err(execution_call_error_v29());
                };
                if count != length || *length > MAX_SSA_VALUE_COMPONENTS_V1 as u64 {
                    return Err(execution_call_error_v29());
                }
                for index in 0..*length as usize {
                    budget.source_reference_charge_v29(plan, 2)?;
                    let child =
                        source_reference_child_v29(plan, node, index, *length as usize, budget)?;
                    source_reference_owned_push_v29(
                        plan,
                        path,
                        SemanticKirParameterProjectionV1::ArrayIndex(index as u32),
                        budget,
                    )?;
                    source_reference_abi_words_v29(
                        plan,
                        child,
                        *element,
                        stride_bytes
                            .checked_mul(index as u64)
                            .and_then(|x| offset.checked_add(x))
                            .ok_or(ArgumentResourceV1::Arithmetic)?,
                        path,
                        output,
                        nodes,
                        budget,
                    )?;
                    path.pop();
                }
                None
            }
            _ => return Err(execution_call_error_v29()),
        }
    };
    if let Some((validation_type, leaf)) = leaf {
        let mut retained = source_reference_owned_vec_v29(plan, path.len(), budget)?;
        budget.source_reference_charge_v29(plan, path.len())?;
        retained.extend_from_slice(path);
        source_reference_owned_push_v29(
            plan,
            output,
            (retained, ty, validation_type, offset, leaf),
            budget,
        )?;
    }
    Ok(())
}

fn check_source_reference_parameter_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    let mut node = source_reference_entry_node_v29(plan, instance, mapped.local(), None, budget)?
        .ok_or_else(execution_call_error_v29)?;
    if let Some(field) = mapped.local_field() {
        let SourceReferenceNodeKindV29::Aggregate { first, count } = plan.nodes[node].kind else {
            return Ok(false);
        };
        if field as usize >= count {
            return Err(execution_call_error_v29());
        }
        node = plan.children[argument_sum_v1(&[first, field as usize])?];
    }
    if !source_reference_node_has_loan_v29(plan, node, budget)?
        && !source_reference_node_has_selected_pointer_v29(plan, node, budget)?
        && !source_descriptor_node_present_v29(plan, node, &mut 0, budget)?
    {
        return Ok(false);
    }
    let argument = mapped.abi();
    if argument.ty() != plan.nodes[node].ty
        || argument.value().adjusted().is_some()
        || argument.value().pointee_override().is_some()
    {
        return Err(execution_call_error_v29());
    }
    let expected_ownership = if let SourceReferenceNodeKindV29::Loan(loan) = plan.nodes[node].kind {
        if !matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_)) {
            return Err(execution_call_error_v29());
        }
        if mapped.tuple_field().is_some() {
            SemanticSourceArgumentOwnershipV1::ByValue
        } else {
            match plan.loans[loan].kind {
                SemanticBorrowKindV1::Shared => SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticBorrowKindV1::Mutable => SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                SemanticBorrowKindV1::Fake => return Err(execution_call_error_v29()),
            }
        }
    } else if plan.nodes[node].descriptor.is_some() && mapped.tuple_field().is_none() {
        SemanticSourceArgumentOwnershipV1::SharedBorrow
    } else {
        SemanticSourceArgumentOwnershipV1::ByValue
    };
    if mapped.source_ownership() != expected_ownership {
        return Err(execution_call_error_v29());
    }
    let mut path = source_reference_owned_vec_v29(plan, 0, budget)?;
    let mut words = source_reference_owned_vec_v29(plan, 0, budget)?;
    source_reference_abi_words_v29(
        plan,
        Some(node),
        argument.ty(),
        0,
        &mut path,
        &mut words,
        &mut 0,
        budget,
    )?;
    let function = plan
        .instances
        .instance(instance)
        .ok_or_else(execution_call_error_v29)?
        .declaration();
    check_by_value_abi_components_v29(
        plan.instances.owner().source_semantic().types(),
        function,
        argument.value(),
        &words,
        Some(budget),
    )
    .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
    Ok(true)
}

fn source_reference_node_types_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    source_reference_owned_prepay_v29::<Vec<Type>>(plan, budget)?;
    let mut output = source_reference_owned_vec_v29(plan, 0, budget)?;
    source_reference_append_node_types_v29(plan, node, false, &mut output, &mut 0, budget)?;
    Ok(output)
}

fn source_reference_cfg_node_types_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    source_reference_owned_prepay_v29::<Vec<Type>>(plan, budget)?;
    let mut output = source_reference_owned_vec_v29(plan, 0, budget)?;
    source_reference_append_node_types_v29(plan, node, true, &mut output, &mut 0, budget)?;
    Ok(output)
}

fn source_reference_return_types_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<Vec<Type>>, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    source_reference_owned_prepay_v29::<Option<Vec<Type>>>(plan, budget)?;
    budget.source_reference_charge_v29(plan, 4)?;
    let Some(node) = plan.returns.get(instance.index()).copied().flatten() else {
        return Ok(None);
    };
    if !source_reference_node_has_loan_v29(plan, node, budget)?
        && !source_reference_node_has_selected_pointer_v29(plan, node, budget)?
        && !source_descriptor_node_present_v29(plan, node, &mut 0, budget)?
        && execution_cfg_return_transport_count_v29(
            plan.instances.owner().source_semantic().types(),
            plan.nodes[node].ty,
            budget,
        )? == 0
    {
        return Ok(None);
    }
    let function = plan
        .instances
        .instance(instance)
        .ok_or_else(execution_call_error_v29)?
        .declaration();
    let abi = function.abi();
    if plan.nodes[node].ty != abi.source_output_type()
        || abi.return_value().ty() != abi.source_output_type()
        || abi.return_value().adjusted().is_some()
        || abi.return_value().pointee_override().is_some()
    {
        return Err(execution_call_error_v29());
    }
    let mut path = source_reference_owned_vec_v29(plan, 0, budget)?;
    let mut words = source_reference_owned_vec_v29(plan, 0, budget)?;
    source_reference_abi_words_v29(
        plan,
        Some(node),
        abi.source_output_type(),
        0,
        &mut path,
        &mut words,
        &mut 0,
        budget,
    )?;
    check_by_value_abi_components_v29(
        plan.instances.owner().source_semantic().types(),
        function,
        abi.return_value(),
        &words,
        Some(budget),
    )
    .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
    source_reference_node_types_v29(plan, node, budget).map(Some)
}

// A nonreturning body still has an ABI signature. This derives only physical
// result types, never a returned source node, nominal identity, or SSA binding.
fn source_reference_no_normal_result_types_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
    let result = (|| {
        plan.check_owner(instances, budget)?;
        source_reference_owned_prepay_v29::<Vec<Type>>(plan, budget)?;
        budget.source_reference_charge_v29(plan, 6)?;
        if plan.instances.instance_reachable(instance) != Some(true)
            || plan.instances.instance_may_return(instance) != Some(false)
            || plan.returns.get(instance.index()) != Some(&None)
        {
            return Err(execution_call_error_v29());
        }
        let function = plan
            .instances
            .instance(instance)
            .ok_or_else(execution_call_error_v29)?
            .declaration();
        let abi = function.abi();
        budget.source_reference_charge_v29(plan, function.locals().len())?;
        let mut returns = function
            .locals()
            .iter()
            .filter(|local| local.role() == SemanticLocalRoleV1::Return);
        if returns.next().map(|local| local.ty()) != Some(abi.source_output_type())
            || returns.next().is_some()
            || abi.return_value().ty() != abi.source_output_type()
            || abi.return_value().adjusted().is_some()
            || abi.return_value().pointee_override().is_some()
        {
            return Err(execution_call_error_v29());
        }
        // The ABI word walk is independent of a returned value. Its temporary
        // vectors are destroyed before their exact scratch credit is released.
        let floor = budget.storage();
        let validation = (|| {
            let mut path = source_reference_owned_vec_v29(plan, 0, budget)?;
            let mut words = source_reference_owned_vec_v29(plan, 0, budget)?;
            source_reference_abi_words_v29(
                plan,
                None,
                abi.source_output_type(),
                0,
                &mut path,
                &mut words,
                &mut 0,
                budget,
            )?;
            check_by_value_abi_components_v29(
                plan.instances.owner().source_semantic().types(),
                function,
                abi.return_value(),
                &words,
                Some(budget),
            )
        })();
        let released = budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        );
        match validation {
            Ok(()) => released?,
            Err(error) => {
                let _ = released;
                return Err(error);
            }
        }
        source_execution_cfg_types_v29(
            plan.instances.owner().source_semantic().types(),
            abi.source_output_type(),
            budget,
        )
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_reference_append_node_types_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    inactive: bool,
    output: &mut Vec<Type>,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    execution_cfg_charge_node_v29(nodes, budget)
        .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
    let row = plan.nodes.get(node).ok_or_else(execution_call_error_v29)?;
    match row.kind {
        SourceReferenceNodeKindV29::Absent => {
            if !inactive {
                return Err(source_reference_error_v29(
                    "source reference ABI cannot materialize an absent holder leaf",
                ));
            }
            for ty in source_reference_inactive_payload_types_v29(plan, node, nodes, budget)? {
                source_reference_owned_push_v29(plan, output, ty, budget)?;
            }
        }
        SourceReferenceNodeKindV29::Loan(loan) => {
            for ty in source_reference_payload_types_v29(plan, loan, budget)? {
                source_reference_owned_push_v29(plan, output, ty, budget)?;
            }
        }
        SourceReferenceNodeKindV29::Address(_) => {
            let ty = budget
                .source_reference_selected_pointer_type_v29(plan, node, row.ty)?
                .ok_or(ArgumentResourceV1::Accounting)?;
            source_reference_owned_push_v29(plan, output, ty, budget)?;
        }
        SourceReferenceNodeKindV29::Plain(_) | SourceReferenceNodeKindV29::Discriminant(_) => {
            if let SourceReferenceNodeKindV29::Plain(Some(anchor)) = row.kind
                && let Some(ty) = source_reference_anchor_type_v29(plan, anchor, row.ty, budget)?
            {
                source_reference_owned_push_v29(plan, output, ty, budget)?;
                return Ok(());
            }
            if row.descriptor.is_some() {
                source_reference_owned_prepay_v29::<Type>(plan, budget)?;
                budget.source_reference_reserve_v29(plan, std::mem::size_of::<Type>())?;
                let ty = source_descriptor_node_type_v29(plan, Some(node), row.ty, budget)?;
                source_reference_owned_push_v29(plan, output, ty, budget)?;
                return Ok(());
            }
            source_reference_owned_prepay_v29::<Vec<Type>>(plan, budget)?;
            for ty in source_execution_cfg_types_v29(
                plan.instances.owner().source_semantic().types(),
                row.ty,
                budget,
            )
            .inspect_err(|error| source_reference_record_failure_v29(plan, error))?
            {
                source_reference_owned_push_v29(plan, output, ty, budget)?;
            }
        }
        SourceReferenceNodeKindV29::Enum { .. } => {
            let (tag, alternative) = source_reference_enum_single_v29(plan, node, budget)?;
            source_reference_owned_push_v29(plan, output, tag, budget)?;
            for offset in 0..alternative.count {
                budget.source_reference_charge_v29(plan, 1)?;
                let child = plan.children[argument_sum_v1(&[alternative.first, offset])?];
                source_reference_append_node_types_v29(
                    plan, child, inactive, output, nodes, budget,
                )?;
            }
        }
        SourceReferenceNodeKindV29::EnumView(_) => {
            for ty in source_reference_binding_origin_types_v29(
                plan,
                SourceReferenceBindingOriginV29::EnumView(node),
                row.ty,
                nodes,
                budget,
            )? {
                source_reference_owned_push_v29(plan, output, ty, budget)?;
            }
        }
        SourceReferenceNodeKindV29::Aggregate { first, count } => {
            for index in 0..count {
                budget.source_reference_charge_v29(plan, 1)?;
                let child = *plan
                    .children
                    .get(argument_sum_v1(&[first, index])?)
                    .ok_or_else(execution_call_error_v29)?;
                if child >= node {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                source_reference_append_node_types_v29(
                    plan, child, inactive, output, nodes, budget,
                )?;
            }
        }
    }
    Ok(())
}
fn source_reference_same_signature_v29(
    left: &LoweredFunctionSignatureV1,
    right: &LoweredFunctionSignatureV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_sum_v1(&[
        left.parameter_semantic_types.len(),
        argument_product_v1(left.call_arguments.len(), 4)?,
        left.result_types.len(),
        8,
    ])?)?;
    if left.parameter_semantic_types != right.parameter_semantic_types
        || left.result_semantic_type != right.result_semantic_type
        || left.call_arguments.len() != right.call_arguments.len()
        || left.parameter_types.len() != right.parameter_types.len()
        || left.result_types.len() != right.result_types.len()
        || left
            .call_arguments
            .iter()
            .zip(&right.call_arguments)
            .any(|(left, right)| {
                left.source_argument != right.source_argument
                    || left.tuple_field != right.tuple_field
                    || left.component != right.component
            })
    {
        return Err(execution_call_error_v29());
    }
    fn same(
        left: &Type,
        right: &Type,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(5)?;
        Ok(match (left, right) {
            (Type::Pointer(left), Type::Pointer(right)) => {
                left.address_space == right.address_space
                    && left.access == right.access
                    && same(&left.pointee, &right.pointee, budget)?
            }
            (Type::Slice(left), Type::Slice(right)) => {
                left.address_space == right.address_space
                    && left.access == right.access
                    && same(&left.element, &right.element, budget)?
            }
            _ => left == right,
        })
    }
    for (left, right) in left
        .parameter_types
        .iter()
        .chain(&left.result_types)
        .zip(right.parameter_types.iter().chain(&right.result_types))
    {
        if !same(left, right, budget)? {
            return Err(execution_call_error_v29());
        }
    }
    Ok(())
}
