// A helper access and its original issuer have distinct source instances. Only
// exact ancestor argument edges and actual representation-preserving pointer
// transport connect them; Generic itself never establishes a Global origin.
#[derive(Clone, Copy)]
struct SourceIssuedResolvedAccessV26 {
    issuer_instance: ProductionCallInstanceIdV1,
    recipe: SourceIssuedRecipeV29,
    pointer: ValueId,
}

#[derive(Clone, Copy)]
struct SourceIssuedPointerTransportV26 {
    pointer: ValueId,
    issuer: ValueId,
}

fn check_source_reference_parameter_transport_v29(
    source_index: &SourceAddressSourceIndexV29<'_>,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    source_argument: u32,
    ty: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<()>(budget)?;
    let child = source_index.sidecar(instance, budget)?;
    let inputs = match (&child.invocation_entry, &child.direct_call_inputs) {
        (Some(entry), None) => &entry.inputs,
        (None, Some(inputs)) => inputs,
        _ => return Err(source_issued_error_v29()),
    };
    budget.charge_work(inputs.len())?;
    let mut matching = inputs.iter().filter(|row| row.local == local.index());
    let row = matching.next().ok_or_else(source_issued_error_v29)?;
    if matching.next().is_some()
        || row.ty != ty
        || row.source_argument != source_argument
        || row.tuple_field.is_some()
        || row.parameter_count != 1
    {
        return Err(source_issued_error_v29());
    }
    Ok(())
}

impl SourceIssuedSemanticV29<'_, '_, '_> {
    fn entry_dependency_v26(
        &self,
        requested: SsaValueV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SemanticLocalIdV1>, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        let function = self
            .instances
            .instance(self.instance)
            .ok_or_else(source_issued_error_v29)?
            .declaration();
        let occurrences = self
            .instances
            .occurrences(self.instance)
            .ok_or_else(source_issued_error_v29)?;
        let mut value = requested;
        // Each step consumes a distinct original copy definition. A cyclic or
        // non-copy source cannot manufacture an entry argument.
        for _ in 0..=self.definitions.len() {
            charge_execution_cfg_lookup_v29(self.definitions.len(), budget)?;
            match self.definitions.get(&value).copied() {
                Some(SourceIssuedDefinitionV29::Statement(index)) => {
                    let (site, rvalue) = source_descriptor_assignment_v29(
                        function,
                        &occurrences,
                        index,
                        value,
                        budget,
                    )?;
                    let Some((place, role, reborrow)) = source_reference_pointer_alias_v29(
                        self.instances.owner().source_semantic().types(),
                        function,
                        site,
                        rvalue,
                        budget,
                    )?
                    else {
                        return Ok(None);
                    };
                    budget.charge_work(3)?;
                    if !reborrow && !place.projections().is_empty() {
                        return Ok(None);
                    }
                    value = self.use_value(site, role, place, budget)?;
                }
                Some(SourceIssuedDefinitionV29::Edge(_)) => return Ok(None),
                None => {
                    let mut local = None;
                    for entry in occurrences.entry_definitions() {
                        budget.charge_work(3)?;
                        if entry.value() == Some(value) {
                            if local
                                .replace(SemanticLocalIdV1::from_index(entry.variable().get()))
                                .is_some()
                            {
                                return Err(source_issued_error_v29());
                            }
                        }
                    }
                    let Some(local) = local else {
                        return Ok(None);
                    };
                    let declaration = function
                        .locals()
                        .get(local.index() as usize)
                        .ok_or_else(source_issued_error_v29)?;
                    if !matches!(declaration.role(), SemanticLocalRoleV1::Argument(_)) {
                        return Ok(None);
                    }
                    let Some(SemanticTypeShapeV1::Pointer(pointer)) = self
                        .instances
                        .owner()
                        .source_semantic()
                        .types()
                        .get(declaration.ty().index() as usize)
                        .map(SemanticTypeDeclV1::shape)
                    else {
                        return Ok(None);
                    };
                    if pointer.kind() != SemanticPointerKindV1::Reference
                        || pointer.metadata() != SemanticPointerMetadataV1::None
                    {
                        return Ok(None);
                    }
                    return Ok(Some(local));
                }
            }
        }
        Err(source_issued_error_v29())
    }
}

impl<'scope, 'owner, 'source> SourceIssuedAccessesV29<'scope, 'owner, 'source> {
    fn original_v26(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&mut SourceIssuedOriginalV29<'scope, 'owner, 'source>, ProductionSemanticKirErrorV1>
    {
        charge_execution_cfg_lookup_v29(self.originals.len(), budget)?;
        if !self.originals.contains_key(&instance.index()) {
            let original =
                SourceIssuedOriginalV29::new(self.instances, instance, self.source_index, budget)?;
            reserve_execution_cfg_map_entry_v29::<usize, SourceIssuedOriginalV29<'_, '_, '_>>(
                self.originals.len(),
                budget,
            )?;
            self.originals.insert(instance.index(), original);
        }
        charge_execution_cfg_lookup_v29(self.originals.len(), budget)?;
        self.originals
            .get_mut(&instance.index())
            .ok_or_else(source_issued_error_v29)
    }

    fn resolve_access_v26(
        &mut self,
        plan: &SourceReferencePlanV29<'_, '_>,
        mut instance: ProductionCallInstanceIdV1,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceIssuedResolvedAccessV26>, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(std::mem::size_of::<(
            [usize; 32],
            SourceIssuedResolvedAccessV26,
            Result<Option<SourceIssuedResolvedAccessV26>, ProductionSemanticKirErrorV1>,
        )>())?;
        let requested_instance = instance;
        let original = self.original_v26(instance, budget)?;
        let mut value = original.use_value(site, role, place, budget)?;
        let SemanticValueBindingV1::Value {
            id: pointer,
            ty: Type::Pointer(_),
        } = original.archived(value, budget)?
        else {
            return Ok(None);
        };
        let pointer = *pointer;
        loop {
            budget.charge_work(4)?;
            let original = self.original_v26(instance, budget)?;
            if let Some(recipe) = original.resolve(value, budget)? {
                if recipe.form != SourceIssuedFormV29::Pointer {
                    return Err(source_issued_error_v29());
                }
                original.check_archive(value, recipe, budget)?;
                return Ok(Some(SourceIssuedResolvedAccessV26 {
                    issuer_instance: instance,
                    recipe,
                    pointer,
                }));
            }
            let Some(local) = original.semantic.entry_dependency_v26(value, budget)? else {
                return self.resolve_uniform_selection_v30(
                    plan,
                    requested_instance,
                    site,
                    role,
                    place,
                    pointer,
                    budget,
                );
            };
            let Some(incoming) = self.instances.incoming(instance) else {
                return Ok(None);
            };
            let occurrence = incoming.occurrence();
            if incoming.child() != Some(instance) || occurrence.caller.index() >= instance.index() {
                return Err(source_issued_error_v29());
            }
            let selector = self
                .instances
                .parameter_source(instance, local, budget)
                .map_err(|error| match error {
                    production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(
                        error,
                    ) => error.into(),
                    _ => source_issued_error_v29(),
                })?;
            if selector.tuple_field.is_some() {
                return Ok(None);
            }
            let Some(SemanticOperandV1::Copy(operand) | SemanticOperandV1::Move(operand)) =
                incoming
                    .source()
                    .arguments()
                    .get(selector.source_argument as usize)
            else {
                return Err(source_issued_error_v29());
            };
            if !std::ptr::eq(
                selector.operand,
                &incoming.source().arguments()[selector.source_argument as usize],
            ) || !operand.projections().is_empty()
                || operand.ty() != selector.ty
            {
                return Err(source_issued_error_v29());
            }
            check_source_reference_parameter_transport_v29(
                self.source_index,
                instance,
                local,
                selector.source_argument,
                selector.ty,
                budget,
            )?;
            instance = occurrence.caller;
            value = self.original_v26(instance, budget)?.use_value(
                ExecutionSiteV29::Terminator {
                    block: SsaBlockIdV1::new(occurrence.block.index()),
                },
                ExecutionOperandV29::CallArgument(selector.source_argument),
                operand,
                budget,
            )?;
        }
    }

    fn resolve_uniform_selection_v30(
        &mut self,
        plan: &SourceReferencePlanV29<'_, '_>,
        instance: ProductionCallInstanceIdV1,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        place: &SemanticPlaceV1,
        pointer: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceIssuedResolvedAccessV26>, ProductionSemanticKirErrorV1> {
        plan.check_owner(self.instances, budget)?;
        budget.reserve_storage(argument_sum_v1(&[
            source_reference_emission_headers_v29::<Option<SourceExternalReferenceOriginV29>>()?,
            source_reference_emission_headers_v29::<Option<SourceIssuedResolvedAccessV26>>()?,
            source_reference_emission_headers_v29::<SourceIssuedRecipeV29>()?,
        ])?)?;
        if source_external_reference_origin_from_use_v29(
            plan,
            instance,
            site,
            role,
            place,
            Some(self.source_index),
            budget,
        )?
        .is_none()
        {
            return Ok(None);
        }
        let origin = with_source_reference_selection_v29(
            plan,
            instance,
            site,
            role,
            place,
            budget,
            |graph, budget| {
                let mut origin = None;
                for node in &graph.nodes {
                    budget.charge_work(3)?;
                    if let SourceReferenceSelectionStepV29::Leaf(leaf) = node.step {
                        if !matches!(leaf, SourceExternalReferenceOriginV29::Issued { .. })
                            || origin.is_some_and(|prior| prior != leaf)
                        {
                            return Ok(None);
                        }
                        origin = Some(leaf);
                    }
                }
                // The graph builder already checked every source edge and the
                // seeded recurrence. No single predecessor supplies this fact.
                Ok(origin)
            },
        )?;
        let Some(SourceExternalReferenceOriginV29::Issued {
            instance: issuer_instance,
            recipe,
        }) = origin
        else {
            return Ok(None);
        };
        let recipe = source_issued_archive_recipe_v29(
            self.instances,
            issuer_instance,
            self.source_index,
            recipe,
            budget,
        )?;
        self.original_v26(issuer_instance, budget)?;
        Ok(Some(SourceIssuedResolvedAccessV26 {
            issuer_instance,
            recipe,
            pointer,
        }))
    }
}

fn check_source_issued_pointer_transports_v26(
    function: &Function,
    actual: &SourceIssuedActualV29<'_>,
    rows: &[SourceIssuedPointerTransportV26],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    if rows.is_empty() {
        return Ok(());
    }
    // The CFG query owns the ledger. Prepay the bounded read-only definition
    // lookups and type/cast checks performed beside its metered origin queries.
    source_issued_pointer_walk_quote_v26(actual.values.len(), rows.len(), budget)?;
    for row in rows {
        let Some((element, AddressSpace::Global, access)) =
            source_issued_pointer_shape_v26(actual.value(row.issuer, budget)?.ty)
        else {
            return Err(source_issued_error_v29());
        };
        let Some((actual_element, space, actual_access)) =
            source_issued_pointer_shape_v26(actual.value(row.pointer, budget)?.ty)
        else {
            return Err(source_issued_error_v29());
        };
        if actual_element != element
            || !matches!(space, AddressSpace::Global | AddressSpace::Generic)
            || !(actual_access == access
                || (access == AccessMode::ReadWrite && actual_access == AccessMode::ReadOnly))
        {
            return Err(source_issued_error_v29());
        }
    }
    let mut valid = true;
    fe2o3_kernel_ir::with_function_control_flow_v1(function, Default::default(), budget, |view| {
        for row in rows {
            valid &= source_issued_pointer_walk_v26(actual, row.pointer, Some(row.issuer), view)?
                == Some(row.issuer);
        }
        Ok(())
    })
    .map_err(|error| match error {
        fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1::Resource(error) => error.into(),
        _ => source_issued_error_v29(),
    })?;
    if !valid {
        return Err(source_issued_error_v29());
    }
    Ok(())
}

fn source_issued_pointer_walk_quote_v26(
    values: usize,
    queries: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_product_v1(
        queries,
        argument_product_v1(
            argument_sum_v1(&[values, 1])?,
            argument_sum_v1(&[
                40,
                argument_product_v1(3, call_splice_search_work_v1(values))?,
            ])?,
        )?,
    )?)?;
    budget.reserve_storage(std::mem::size_of::<(
        [usize; 16],
        SourcePointerOriginBoundaryV29,
        Option<ValueId>,
        Result<Option<ValueId>, ProductionSemanticKirErrorV1>,
        Result<Option<ValueId>, fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1>,
    )>())?;
    Ok(())
}

#[derive(Clone, Copy)]
enum SourcePointerOriginBoundaryV29 {
    Global(Option<ValueId>),
    // A structural endpoint only. Its original selected edges and every leaf's
    // memory obligations are checked by the selected-reference relation.
    Selected(ValueId),
}

fn source_reference_pointer_transport_until_v29(
    actual: &SourceIssuedActualV29<'_>,
    pointer: ValueId,
    boundary: ValueId,
    view: &mut fe2o3_kernel_ir::FunctionControlFlowViewV1<'_, '_, '_>,
) -> Result<Option<ValueId>, fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1> {
    source_pointer_transport_walk_v29(
        actual,
        pointer,
        SourcePointerOriginBoundaryV29::Selected(boundary),
        view,
        |_| {},
    )
}

fn source_issued_pointer_walk_v26(
    actual: &SourceIssuedActualV29<'_>,
    pointer: ValueId,
    issuer: Option<ValueId>,
    view: &mut fe2o3_kernel_ir::FunctionControlFlowViewV1<'_, '_, '_>,
) -> Result<Option<ValueId>, fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1> {
    source_issued_pointer_walk_visit_v26(actual, pointer, issuer, view, |_| {})
}

fn source_issued_pointer_walk_visit_v26(
    actual: &SourceIssuedActualV29<'_>,
    pointer: ValueId,
    issuer: Option<ValueId>,
    view: &mut fe2o3_kernel_ir::FunctionControlFlowViewV1<'_, '_, '_>,
    visit: impl FnMut(usize),
) -> Result<Option<ValueId>, fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1> {
    source_pointer_transport_walk_v29(
        actual,
        pointer,
        SourcePointerOriginBoundaryV29::Global(issuer),
        view,
        visit,
    )
}

fn source_pointer_transport_walk_v29(
    actual: &SourceIssuedActualV29<'_>,
    mut pointer: ValueId,
    boundary: SourcePointerOriginBoundaryV29,
    view: &mut fe2o3_kernel_ir::FunctionControlFlowViewV1<'_, '_, '_>,
    mut visit: impl FnMut(usize),
) -> Result<Option<ValueId>, fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1> {
    for _ in 0..=actual.values.len() {
        let Some(origin) = (match boundary {
            SourcePointerOriginBoundaryV29::Global(_) => view.unique_value_origin(pointer)?,
            SourcePointerOriginBoundaryV29::Selected(value) => {
                view.unique_value_origin_until(pointer, value)?
            }
        }) else {
            break;
        };
        let Ok(current_index) = actual
            .values
            .binary_search_by_key(&pointer, |value| value.id)
        else {
            break;
        };
        let Ok(index) = actual
            .values
            .binary_search_by_key(&origin, |value| value.id)
        else {
            break;
        };
        let current_shape = source_issued_pointer_shape_v26(actual.values[current_index].ty);
        let Some((target_element, target_space, target_access)) =
            source_issued_pointer_shape_v26(actual.values[index].ty)
        else {
            break;
        };
        let shape = Some((target_element, target_space, target_access));
        if current_shape != shape {
            break;
        }
        let result = actual.values[index];
        // Classification strips access restriction in either cast order.
        // Exact replay instead stops at the named issuer, even when that
        // issuer's own physical representation is already restricted.
        let stop = match boundary {
            SourcePointerOriginBoundaryV29::Global(Some(issuer)) => {
                target_space == AddressSpace::Global && issuer == origin
            }
            SourcePointerOriginBoundaryV29::Selected(value) => {
                matches!(target_space, AddressSpace::Global | AddressSpace::Generic)
                    && value == origin
            }
            SourcePointerOriginBoundaryV29::Global(None) => {
                target_space == AddressSpace::Global
                    && !matches!(
                        result.operation.map(|operation| &operation.kind),
                        Some(OperationKind::Cast {
                            kind: CastKind::RestrictPointerAccess,
                            ..
                        })
                    )
            }
        };
        if stop {
            return Ok(Some(origin));
        }
        let Some(Operation {
            kind: OperationKind::Cast { kind, value, to },
            results,
            ..
        }) = result.operation
        else {
            break;
        };
        let Ok(input) = actual.values.binary_search_by_key(value, |value| value.id) else {
            break;
        };
        let Some((element, from_space, from_access)) =
            source_issued_pointer_shape_v26(actual.values[input].ty)
        else {
            break;
        };
        if !matches!(results.as_slice(), [definition] if definition.id == origin && source_issued_pointer_shape_v26(&definition.ty) == shape)
            || source_issued_pointer_shape_v26(to) != shape
            || element != target_element
            || !match kind {
                CastKind::PointerToGeneric => {
                    from_space == AddressSpace::Global
                        && target_space == AddressSpace::Generic
                        && from_access == target_access
                }
                CastKind::RestrictPointerAccess => {
                    from_space == target_space
                        && matches!(from_space, AddressSpace::Global | AddressSpace::Generic)
                        && from_access == AccessMode::ReadWrite
                        && target_access == AccessMode::ReadOnly
                }
                _ => false,
            }
        {
            break;
        }
        visit(index);
        pointer = *value;
    }
    Ok(None)
}

include!("production_source_issued_pointer_transport_census_v26.rs");

// Classification only, not source or memory authority. The caller must still
// complete the exact global obligation census and source-root correspondence.
// This query owns and settles all scratch before returning its inert value id.
fn source_issued_global_pointer_origin_v26(
    function: &Function,
    pointer: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<ValueId>, ProductionSemanticKirErrorV1> {
    with_canonical_call_scratch_v1(budget, |budget| {
        let actual = SourceIssuedActualV29::from_function(function, budget)?;
        source_issued_pointer_walk_quote_v26(actual.values.len(), 1, budget)?;
        let mut result = None;
        fe2o3_kernel_ir::with_function_control_flow_v1(
            function,
            Default::default(),
            budget,
            |view| {
                result = source_issued_pointer_walk_v26(&actual, pointer, None, view)?;
                Ok(())
            },
        )
        .map_err(|error| match error {
            fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1::Resource(error) => error.into(),
            _ => source_issued_error_v29(),
        })?;
        Ok(result)
    })
}

fn source_issued_pointer_shape_v26(ty: &Type) -> Option<(ScalarType, AddressSpace, AccessMode)> {
    let Type::Pointer(pointer) = ty else {
        return None;
    };
    let Type::Scalar(element) = pointer.pointee.as_ref() else {
        return None;
    };
    Some((*element, pointer.address_space, pointer.access))
}

fn source_issued_memory_pointer_v26(
    actual: &SourceIssuedActualV29<'_>,
    pointer: ValueId,
    memory: MemoryAccess,
    writing: bool,
    element: ScalarType,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(8)?;
    let Some((actual_element, space, access)) =
        source_issued_pointer_shape_v26(actual.value(pointer, budget)?.ty)
    else {
        return Ok(false);
    };
    Ok(actual_element == element
        && space == memory.address_space
        && matches!(space, AddressSpace::Global | AddressSpace::Generic)
        && if writing {
            access == AccessMode::ReadWrite
        } else {
            access != AccessMode::WriteOnly
        })
}

#[cfg(test)]
mod source_issued_call_provenance_tests_v26 {
    use super::*;
    include!("production_source_issued_call_provenance_v26_tests.rs");
}

include!("production_source_issued_global_origin_batch_v26.rs");
