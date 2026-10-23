// All local descriptors are rederived from the SAME borrowed immutable source
// owner. They are arithmetic transport, not reference layout/EXEC authority.
struct CheckedWave64ReferenceLocalsV1<'a> {
    owner: Option<&'a ProductionSemanticSsaOwnerV1>,
    function: SemanticFunctionIdV1,
    locals: BTreeMap<u32, Wave64NominalReferenceDescriptorV1>,
    retained_snapshots: BTreeSet<u32>,
    rank_locals: BTreeSet<u32>,
    borrows: BTreeMap<(u32, u32), Wave64NominalReferenceDescriptorV1>,
    context_borrows: BTreeMap<(u32, u32), Wave64NominalReferenceDescriptorV1>,
}
impl CheckedWave64ReferenceLocalsV1<'_> {
    fn require_source_v1(
        &self,
        function: &SemanticFunctionDeclV1,
        id: SemanticFunctionIdV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.function != id
            || self.owner.is_some_and(|owner| {
                owner
                    .source_semantic()
                    .functions()
                    .get(id.index() as usize)
                    .is_none_or(|original| !std::ptr::eq(original, function))
            })
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(())
    }
}
fn wave64_query_result_v1(
    result: Result<(), wave64_reference_call_v1::ReferenceErrorV1>,
    callback_error: Option<ProductionSemanticKirErrorV1>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    if let Some(error) = callback_error {
        return Err(error);
    }
    match result {
        Ok(()) => Ok(true),
        Err(wave64_reference_call_v1::ReferenceErrorV1::Resource(error)) => Err(error.into()),
        Err(wave64_reference_call_v1::ReferenceErrorV1::Unavailable(_)) => Ok(false),
        Err(wave64_reference_call_v1::ReferenceErrorV1::CallbackPanicked) => Err(unsupported(
            0,
            None,
            None,
            "checked Wave64 source callback unwound",
        )),
    }
}
fn prepare_checked_wave64_reference_locals_v1<'a, 'work>(
    owner: Option<&'a ProductionSemanticSsaOwnerV1>,
    function: &SemanticFunctionDeclV1,
    id: SemanticFunctionIdV1,
    mut budget: Option<&mut (dyn SemanticEmissionBudgetV1 + 'work)>,
) -> Result<CheckedWave64ReferenceLocalsV1<'a>, ProductionSemanticKirErrorV1> {
    let mut output = CheckedWave64ReferenceLocalsV1 {
        owner,
        function: id,
        locals: BTreeMap::new(),
        retained_snapshots: BTreeSet::new(),
        rank_locals: BTreeSet::new(),
        borrows: BTreeMap::new(),
        context_borrows: BTreeMap::new(),
    };
    output.require_source_v1(function, id)?;
    let Some(owner) = owner else {
        return Ok(output);
    };
    if !ordinary_source_has_raw_wave64_shuffle_v1(owner.source_semantic()) {
        output.owner = None;
        return Ok(output);
    }
    let budget = budget
        .as_deref_mut()
        .ok_or(ArgumentResourceV1::Accounting)?;
    // Exact source/capture identity is independent of local type coordinates.
    if owner.plan_for_function(id).is_none() || owner.occurrences_v1().is_none() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    budget.checked_wave64_helper_v1(owner, id, &mut |helper, budget| {
        if matches!(
            helper.representative().argument(0),
            Some(wave64_reference_call_v1::ArgumentV1::Rank { .. })
        ) {
            let mut local = None;
            for (index, row) in function.locals().iter().enumerate() {
                if row.role() == SemanticLocalRoleV1::Argument(0)
                    && local.replace(index as u32).is_some()
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
            }
            let local = local.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            reserve_execution_cfg_map_entry_v29::<u32, ()>(output.rank_locals.len(), budget)?;
            output.rank_locals.insert(local);
        }
        for argument in 0..2 {
            if let Some(descriptor) =
                Wave64NominalReferenceDescriptorV1::from_checked_formal_v1(helper, argument)?
            {
                require_closed_wave64_reference_holder_v1(
                    owner,
                    id,
                    descriptor.local,
                    None,
                    budget,
                )?;
                reserve_execution_cfg_map_entry_v29::<u32, Wave64NominalReferenceDescriptorV1>(
                    output.locals.len(),
                    budget,
                )?;
                if output.locals.insert(descriptor.local, descriptor).is_some() {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
            }
        }
        Ok(())
    })?;
    for (bi, block) in function.blocks().iter().enumerate() {
        for (si, statement) in block.statements().iter().enumerate() {
            budget.charge_work(2)?;
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if !matches!(
                assignment.value().kind(),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    ..
                }
            ) {
                continue;
            }
            budget.checked_wave64_borrow_v1(
                owner,
                id,
                SemanticBlockIdV1::from_index(bi as u32),
                si as u32,
                &mut |source, budget| {
                    let descriptor =
                        Wave64NominalReferenceDescriptorV1::from_checked_borrow_v1(source)?;
                    require_closed_wave64_reference_holder_v1(
                        owner,
                        id,
                        descriptor.local,
                        Some((source.block(), source.statement())),
                        budget,
                    )?;
                    let SemanticRvalueKindV1::Borrow { place, .. } = source.source_rvalue().kind()
                    else {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    };
                    reserve_execution_cfg_map_entry_v29::<u32, Wave64NominalReferenceDescriptorV1>(
                        output.locals.len(),
                        budget,
                    )?;
                    if output
                        .locals
                        .insert(descriptor.local, descriptor)
                        .is_some_and(|old| old != descriptor)
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    reserve_execution_cfg_map_entry_v29::<u32, ()>(
                        output.retained_snapshots.len(),
                        budget,
                    )?;
                    output.retained_snapshots.insert(place.local().index());
                    reserve_execution_cfg_map_entry_v29::<
                        (u32, u32),
                        Wave64NominalReferenceDescriptorV1,
                    >(output.borrows.len(), budget)?;
                    if output
                        .borrows
                        .insert((bi as u32, si as u32), descriptor)
                        .is_some()
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    Ok(())
                },
            )?;
            budget.checked_wave64_context_borrow_v1(
                owner,
                id,
                SemanticBlockIdV1::from_index(bi as u32),
                si as u32,
                &mut |source, budget| {
                    let descriptor =
                        Wave64NominalReferenceDescriptorV1::from_checked_context_borrow_v1(source)?;
                    require_closed_wave64_reference_holder_v1(
                        owner,
                        id,
                        descriptor.local,
                        Some((source.block(), source.statement())),
                        budget,
                    )?;
                    let (issuer_function, _, issuer_local) = source.issuer();
                    if issuer_function != id {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    reserve_execution_cfg_map_entry_v29::<u32, Wave64NominalReferenceDescriptorV1>(
                        output.locals.len(),
                        budget,
                    )?;
                    if output
                        .locals
                        .insert(descriptor.local, descriptor)
                        .is_some_and(|old| old != descriptor)
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    reserve_execution_cfg_map_entry_v29::<u32, ()>(
                        output.retained_snapshots.len(),
                        budget,
                    )?;
                    output.retained_snapshots.insert(issuer_local.index());
                    reserve_execution_cfg_map_entry_v29::<
                        (u32, u32),
                        Wave64NominalReferenceDescriptorV1,
                    >(output.context_borrows.len(), budget)?;
                    if output
                        .context_borrows
                        .insert((bi as u32, si as u32), descriptor)
                        .is_some()
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    Ok(())
                },
            )?;
        }
    }
    Ok(output)
}
fn checked_wave64_helper_parameter_plan_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
    helper: SemanticFunctionIdV1,
    id: FunctionId,
    max_rows: usize,
    closure: &mut ReachableClosureBudgetV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<LoweredFunctionPlanV1>, ProductionSemanticKirErrorV1> {
    if !ordinary_source_has_raw_wave64_shuffle_v1(owner.source_semantic()) {
        return Ok(None);
    }
    let mut output = None;
    let mut callback_error = None;
    let result = wave64_reference_call_v1::with_checked_reference_helper_v1(
        owner,
        helper,
        budget,
        |view, budget| {
            let result = (|| {
                let source = view.representative();
                let function = source.helper_declaration();
                check_argument_function_abi_v1(
                    function,
                    helper,
                    SemanticKirFunctionRoleV1::InternalHelper,
                )?;
                let mut parameters = Vec::new();
                for (local, row) in function.locals().iter().enumerate() {
                    match row.role() {
                        SemanticLocalRoleV1::Argument(argument) => {
                            parameters.push((argument, local, row.ty()))
                        }
                        SemanticLocalRoleV1::RustCallTupleField { .. } => {
                            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                        }
                        _ => {}
                    }
                }
                parameters.sort_unstable_by_key(|row| row.0);
                if parameters.len() != 3
                    || parameters
                        .iter()
                        .enumerate()
                        .any(|(i, row)| row.0 as usize != i)
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let mut types = Vec::new();
                let mut values = Vec::new();
                let mut selectors = Vec::new();
                let mut bindings = Vec::new();
                for &(argument, local, ty) in &parameters {
                    budget.charge_work(8)?;
                    let descriptor = Wave64NominalReferenceDescriptorV1::from_checked_formal_v1(
                        view,
                        argument as usize,
                    )?;
                    let component_types = match descriptor {
                        Some(d) => d.transport_types_v1(),
                        None => vec![lower_parameter_scalar_v1(
                            owner.source_semantic().types(),
                            ty,
                        )?],
                    };
                    closure.charge_parameter_expansion(
                        3,
                        types.len(),
                        component_types.len(),
                        max_rows,
                    )?;
                    let mut components = Vec::new();
                    for (component, component_type) in component_types.into_iter().enumerate() {
                        if component != 0 {
                            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                        }
                        let value = ValueId(
                            u32::try_from(local).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        );
                        types.push(component_type.clone());
                        values.push(value);
                        components.push(ValueDef::new(value, component_type));
                        selectors.push(HelperCallArgumentV1 {
                            source_argument: argument,
                            tuple_field: None,
                            component: Some(0),
                        });
                    }
                    bindings.push(match descriptor {
                        Some(descriptor) => PlannedParameterLocalBindingV1::Wave64Nominal {
                            local,
                            semantic_type: ty,
                            descriptor,
                            values: components,
                        },
                        None => {
                            let [value] = components.as_slice() else {
                                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                            };
                            PlannedParameterLocalBindingV1::Direct {
                                local,
                                value: value.id,
                                ty: value.ty.clone(),
                            }
                        }
                    });
                }
                if types != [Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::F32)] {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let results =
                    helper_result_components_v1(owner.source_semantic().types(), function, helper)?
                        .components
                        .into_iter()
                        .map(|row| row.2)
                        .collect::<Vec<_>>();
                if results != [Type::Scalar(ScalarType::F32)] {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                output = Some(LoweredFunctionPlanV1 {
                    correspondence_owner: root,
                    semantic_function: helper,
                    kernel_ir_function: id,
                    role: SemanticKirFunctionRoleV1::InternalHelper,
                    parameter_declarations: parameters,
                    parameter_types: types,
                    parameter_values: values,
                    call_arguments: selectors,
                    parameter_local_bindings: bindings,
                    parameter_component_bindings: Vec::new(),
                    ignored_parameter_bindings: Vec::new(),
                    result_types: results,
                });
                Ok(())
            })();
            if let Err(error) = result {
                callback_error = Some(error);
                return Err(wave64_reference_call_v1::ReferenceErrorV1::Unavailable(
                    "nominal helper planner refused",
                ));
            }
            Ok(())
        },
    );
    // Preserve the original source-relation diagnostic while leaving the
    // existing generic fallback and every admission decision unchanged.
    if let Err(wave64_reference_call_v1::ReferenceErrorV1::Unavailable(reason)) = &result {
        eprintln!("Wave64 nominal helper {} unavailable: {}", helper.index(), reason);
    }
    wave64_query_result_v1(result, callback_error)?;
    Ok(output)
}
impl SemanticFunctionLoweringV1<'_, '_> {
    fn try_lower_checked_wave64_snapshot_borrow_v1(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        result_type: SemanticTypeIdV1,
        value: &SemanticRvalueKindV1,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let Some(statement) = statement else {
            return Ok(None);
        };
        if let Some(expected) = self
            .wave64_references
            .context_borrows
            .get(&(block.index(), statement))
            .copied()
        {
            return self.lower_checked_wave64_context_borrow_v1(
                block,
                statement,
                result_type,
                value,
                expected,
            );
        }
        let Some(expected) = self
            .wave64_references
            .borrows
            .get(&(block.index(), statement))
            .copied()
        else {
            return Ok(None);
        };
        let owner = self
            .wave64_references
            .owner
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let mut binding = None;
        self.with_emission_budget_v1(|this, budget| {
            let checked = budget.checked_wave64_borrow_v1(
                owner,
                this.semantic_function,
                block,
                statement,
                &mut |source, budget| {
                    budget.charge_work(12)?;
                    if !std::ptr::eq(source.source_rvalue().kind(), value)
                        || source.reference_type() != result_type
                        || Wave64NominalReferenceDescriptorV1::from_checked_borrow_v1(source)?
                            != expected
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    require_closed_wave64_reference_holder_v1(
                        owner,
                        this.semantic_function,
                        expected.local,
                        Some((block, statement)),
                        budget,
                    )?;
                    let (issuer, producer) = source.lane_producer();
                    if issuer != this.semantic_function {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    let Some(SemanticValueBindingV1::WaveLane { value, wave }) =
                        this.semantic_ssa_bindings.get(&producer)
                    else {
                        return Err(unsupported(
                            issuer.index(),
                            Some(block.index()),
                            Some(statement),
                            "original snapshot lane SSA producer has no emitted binding",
                        ));
                    };
                    if wave.width != 64 {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    binding = Some(expected.reconstruct_v1(
                        this.types,
                        result_type,
                        &[ValueDef::new(*value, Type::Scalar(ScalarType::U32))],
                    )?);
                    Ok(())
                },
            )?;
            if !checked || binding.is_none() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            Ok(())
        })?;
        Ok(binding)
    }
    fn check_checked_wave64_call_bindings_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        callee: SemanticFunctionIdV1,
        bindings: &[SemanticValueBindingV1],
        arguments: &[ValueId],
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let owner = self
            .wave64_references
            .owner
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        self.with_emission_budget_v1(|this, budget| {
            let checked = budget.checked_wave64_call_v1(
                owner,
                this.semantic_function,
                block,
                &mut |source, budget| {
                    budget.charge_work(16)?;
                    if source.helper() != callee
                        || !std::ptr::eq(source.source_call(), call)
                        || bindings.len() != 3
                        || !matches!(bindings[1], SemanticValueBindingV1::CollectiveContext)
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    for argument in 0..3 {
                        let original = wave64_original_call_argument_ssa_v1(
                            owner,
                            this.semantic_function,
                            block,
                            argument,
                            budget,
                        )?;
                        let archived = this
                            .semantic_ssa_bindings
                            .get(&original)
                            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                        if !same_wave64_original_binding_v1(
                            archived,
                            &bindings[argument as usize],
                            budget,
                            0,
                        )? {
                            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                        }
                    }
                    let lane = match source.argument(0) {
                        Some(wave64_reference_call_v1::ArgumentV1::Snapshot {
                            snapshot_type,
                            lane_field,
                            ..
                        }) => {
                            let Some(SemanticTypeShapeV1::Aggregate(original_fields)) = this
                                .types
                                .get(snapshot_type.index() as usize)
                                .map(SemanticTypeDeclV1::shape)
                            else {
                                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                            };
                            let SemanticValueBindingV1::Aggregate(actual_fields) = &bindings[0]
                            else {
                                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                            };
                            if original_fields.fields().len() != actual_fields.len() {
                                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                            }
                            for (index, field) in actual_fields.iter().enumerate() {
                                if index != lane_field as usize {
                                    visit_wave64_empty_source_zst_v1(
                                        field,
                                        &mut Wave64ReferenceCheckVisitorV1 { budget },
                                        0,
                                    )?;
                                }
                            }
                            let SemanticValueBindingV1::Aggregate(fields) = &bindings[0] else {
                                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                            };
                            let Some(SemanticValueBindingV1::Value { id, ty }) =
                                fields.get(lane_field as usize)
                            else {
                                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                            };
                            if *ty != Type::Scalar(ScalarType::U32) {
                                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                            }
                            *id
                        }
                        Some(wave64_reference_call_v1::ArgumentV1::Rank { .. }) => {
                            let SemanticValueBindingV1::Value { id, ty } = &bindings[0] else {
                                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                            };
                            if *ty != Type::Scalar(ScalarType::U32) {
                                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                            }
                            *id
                        }
                        _ => return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
                    };
                    let SemanticValueBindingV1::Value { id, ty } = &bindings[2] else {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    };
                    if *ty != Type::Scalar(ScalarType::F32) || arguments != [lane, *id] {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    Ok(())
                },
            )?;
            if !checked {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            Ok(())
        })
    }
}

struct Wave64ReferenceCheckVisitorV1<'a> {
    budget: &'a mut dyn SemanticEmissionBudgetV1,
}
impl SemanticTransportVisitorV1 for Wave64ReferenceCheckVisitorV1<'_> {
    type Error = ProductionSemanticKirErrorV1;
    fn node(&mut self) -> Result<(), Self::Error> {
        self.budget.charge_work(1)
    }
    fn component(
        &mut self,
        _value: ValueId,
        ty: BorrowedTransportTypeV1<'_>,
    ) -> Result<(), Self::Error> {
        self.budget.charge_work(1)?;
        match ty {
            BorrowedTransportTypeV1::Existing(Type::Scalar(ScalarType::U32)) => Ok(()),
            _ => Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
        }
    }
    fn invalid(_: &'static str) -> Self::Error {
        ProductionSemanticKirErrorV1::CorrespondenceMismatch
    }
}
impl CheckedWave64ReferenceLocalsV1<'_> {
    fn arithmetic_lane_v1(
        &self,
        local: u32,
        binding: &SemanticValueBindingV1,
    ) -> Result<Option<ValueId>, ProductionSemanticKirErrorV1> {
        if self.rank_locals.contains(&local) {
            let SemanticValueBindingV1::Value { id, ty } = binding else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            if *ty != Type::Scalar(ScalarType::U32) {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            return Ok(Some(*id));
        }
        if let Some(Wave64NominalReferenceDescriptorV1 {
            lane_field: Some(field),
            ..
        }) = self.locals.get(&local)
        {
            let SemanticValueBindingV1::Aggregate(fields) = binding else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            let Some(SemanticValueBindingV1::Value { id, ty }) = fields.get(*field as usize) else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            if *ty != Type::Scalar(ScalarType::U32) {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            return Ok(Some(*id));
        }
        Ok(None)
    }
}

// A descriptor is coordinates only. Each actual consumer borrows a freshly
// rederived checked view on the SAME ledger, and cannot copy this guard out.
enum LiveWave64DescriptorSourceV1<'v, 'o> {
    Helper(&'v wave64_reference_call_v1::CheckedReferenceHelperV1<'o>),
    Borrow(&'v wave64_reference_call_v1::CheckedReferenceBorrowV1<'o>),
    ContextBorrow(&'v wave64_reference_call_v1::CheckedContextBorrowV1<'o>),
}
struct CheckedWave64DescriptorV1<'v, 'o> {
    descriptor: Wave64NominalReferenceDescriptorV1,
    source: LiveWave64DescriptorSourceV1<'v, 'o>,
}
impl CheckedWave64DescriptorV1<'_, '_> {
    fn transport_types_v1(&self) -> Vec<Type> {
        self.descriptor.transport_types_v1()
    }
    fn reconstruct_v1(
        &self,
        types: &[SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
        values: &[ValueDef],
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.descriptor.reconstruct_v1(types, ty, values)
    }
    fn visit_values_v1<V: SemanticTransportVisitorV1>(
        &self,
        binding: &SemanticValueBindingV1,
        expected: &[Type],
        visitor: &mut V,
    ) -> Result<(), V::Error> {
        self.descriptor.visit_values_v1(binding, expected, visitor)
    }
    fn admits_dereference_v1(
        &self,
        function: SemanticFunctionIdV1,
        local: u32,
        ty: SemanticTypeIdV1,
        projection: &SemanticProjectionV1,
    ) -> bool {
        self.descriptor
            .admits_dereference_v1(function, local, ty, projection)
    }
}
impl CheckedWave64ReferenceLocalsV1<'_> {
    fn with_descriptor_v1(
        &self,
        descriptor: Wave64NominalReferenceDescriptorV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
        inspect: &mut dyn FnMut(
            &CheckedWave64DescriptorV1<'_, '_>,
            &mut dyn SemanticEmissionBudgetV1,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let owner = self
            .owner
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if descriptor.function != self.function
            || self.locals.get(&descriptor.local) != Some(&descriptor)
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let borrow = self
            .borrows
            .iter()
            .find_map(|(site, candidate)| (*candidate == descriptor).then_some(*site));
        let checked = if let Some((block, statement)) = borrow {
            budget.checked_wave64_borrow_v1(
                owner,
                self.function,
                SemanticBlockIdV1::from_index(block),
                statement,
                &mut |view, budget| {
                    if !std::ptr::eq(view.owner(), owner)
                        || Wave64NominalReferenceDescriptorV1::from_checked_borrow_v1(view)?
                            != descriptor
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    require_closed_wave64_reference_holder_v1(
                        owner,
                        self.function,
                        descriptor.local,
                        Some((view.block(), view.statement())),
                        budget,
                    )?;
                    inspect(
                        &CheckedWave64DescriptorV1 {
                            descriptor,
                            source: LiveWave64DescriptorSourceV1::Borrow(view),
                        },
                        budget,
                    )
                },
            )?
        } else if let Some((block, statement)) = self
            .context_borrows
            .iter()
            .find_map(|(site, candidate)| (*candidate == descriptor).then_some(*site))
        {
            budget.checked_wave64_context_borrow_v1(
                owner,
                self.function,
                SemanticBlockIdV1::from_index(block),
                statement,
                &mut |view, budget| {
                    if !std::ptr::eq(view.owner(), owner)
                        || Wave64NominalReferenceDescriptorV1::from_checked_context_borrow_v1(view)?
                            != descriptor
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    require_closed_wave64_reference_holder_v1(
                        owner,
                        self.function,
                        descriptor.local,
                        Some((view.block(), view.statement())),
                        budget,
                    )?;
                    inspect(
                        &CheckedWave64DescriptorV1 {
                            descriptor,
                            source: LiveWave64DescriptorSourceV1::ContextBorrow(view),
                        },
                        budget,
                    )
                },
            )?
        } else {
            budget.checked_wave64_helper_v1(owner, self.function, &mut |view, budget| {
                if !std::ptr::eq(view.representative().owner(), owner) {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let mut matched = false;
                for argument in 0..2 {
                    if Wave64NominalReferenceDescriptorV1::from_checked_formal_v1(view, argument)?
                        == Some(descriptor)
                    {
                        if matched {
                            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                        }
                        matched = true;
                    }
                }
                if !matched {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                require_closed_wave64_reference_holder_v1(
                    owner,
                    self.function,
                    descriptor.local,
                    None,
                    budget,
                )?;
                inspect(
                    &CheckedWave64DescriptorV1 {
                        descriptor,
                        source: LiveWave64DescriptorSourceV1::Helper(view),
                    },
                    budget,
                )
            })?
        };
        if !checked {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(())
    }
    fn remove_checked_retained_snapshots_v1(
        &self,
        candidates: &mut BTreeSet<u32>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let Some(owner) = self.owner else {
            return Ok(());
        };
        for &local in &self.retained_snapshots {
            let mut validated = false;
            for &(block, statement) in self.borrows.keys() {
                budget.checked_wave64_borrow_v1(
                    owner,
                    self.function,
                    SemanticBlockIdV1::from_index(block),
                    statement,
                    &mut |view, budget| {
                        if !std::ptr::eq(view.owner(), owner) {
                            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                        }
                        let SemanticRvalueKindV1::Borrow { place, .. } =
                            view.source_rvalue().kind()
                        else {
                            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                        };
                        if place.local().index() == local {
                            let descriptor =
                                Wave64NominalReferenceDescriptorV1::from_checked_borrow_v1(view)?;
                            require_closed_wave64_reference_holder_v1(
                                owner,
                                self.function,
                                descriptor.local,
                                Some((view.block(), view.statement())),
                                budget,
                            )?;
                            candidates.remove(&local);
                            validated = true;
                        }
                        Ok(())
                    },
                )?;
            }
            for &(block, statement) in self.context_borrows.keys() {
                budget.checked_wave64_context_borrow_v1(
                    owner,
                    self.function,
                    SemanticBlockIdV1::from_index(block),
                    statement,
                    &mut |view, budget| {
                        let (function, _, issuer_local) = view.issuer();
                        if !std::ptr::eq(view.owner(), owner) || function != self.function {
                            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                        }
                        if issuer_local.index() == local {
                            let descriptor =
                                Wave64NominalReferenceDescriptorV1::from_checked_context_borrow_v1(
                                    view,
                                )?;
                            require_closed_wave64_reference_holder_v1(
                                owner,
                                self.function,
                                descriptor.local,
                                Some((view.block(), view.statement())),
                                budget,
                            )?;
                            candidates.remove(&local);
                            validated = true;
                        }
                        Ok(())
                    },
                )?;
            }
            if !validated {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        }
        Ok(())
    }
    fn seed_arithmetic_bound_v1(
        &self,
        local: u32,
        binding: &SemanticValueBindingV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
        bounds: &mut BTreeMap<ValueId, u128>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.rank_locals.contains(&local) {
            let owner = self
                .owner
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let checked =
                budget.checked_wave64_helper_v1(owner, self.function, &mut |view, budget| {
                    if !std::ptr::eq(view.representative().owner(), owner)
                        || !matches!(
                            view.representative().argument(0),
                            Some(wave64_reference_call_v1::ArgumentV1::Rank { .. })
                        )
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    if owner.source_semantic().functions()[self.function.index() as usize]
                        .locals()
                        .get(local as usize)
                        .is_none_or(|row| row.role() != SemanticLocalRoleV1::Argument(0))
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    require_unwritten_wave64_local_v1(owner, self.function, local, None, budget)?;
                    let Some(value) = self.arithmetic_lane_v1(local, binding)? else {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    };
                    bounds.insert(value, 64);
                    Ok(())
                })?;
            if !checked {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        } else if let Some(descriptor) = self.locals.get(&local).copied() {
            self.with_descriptor_v1(descriptor, budget, &mut |guard, _| {
                if guard.descriptor.lane_field.is_some() {
                    let Some(value) = self.arithmetic_lane_v1(local, binding)? else {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    };
                    bounds.insert(value, 64);
                }
                Ok(())
            })?;
        }
        Ok(())
    }
}
impl SemanticFunctionLoweringV1<'_, '_> {
    fn checked_wave64_deref_v1(
        &mut self,
        local: u32,
        ty: SemanticTypeIdV1,
        projection: &SemanticProjectionV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let Some(descriptor) = self.wave64_references.locals.get(&local).copied() else {
            return Ok(false);
        };
        let budget = self
            .emission_work
            .as_deref_mut()
            .ok_or(ArgumentResourceV1::Accounting)?;
        let mut admitted = false;
        self.wave64_references
            .with_descriptor_v1(descriptor, budget, &mut |guard, _| {
                admitted =
                    guard.admits_dereference_v1(self.semantic_function, local, ty, projection);
                Ok(())
            })?;
        Ok(admitted)
    }
}

fn ordinary_source_has_raw_wave64_shuffle_v1(source: &AdmittedInertSemanticMirV1) -> bool {
    source.callables().iter().any(|callable| {
        matches!(
            callable,
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Gfx942Wave64ShuffleIndex { .. },
                ..
            }
        )
    })
}

// The actual reference holder cannot acquire a new SSA meaning after its
// authenticated definition. Conservatively close alias creation and escape;
// ordinary pointer/reference lowering remains unchanged for other functions.
// Bounded reachability over this immutable owner's original captured edges.
// Successor seeding distinguishes a later loop iteration from same-block order.
fn wave64_holder_source_reachability_v1(
    rows: &fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    blocks: usize, start: usize, removed: Option<usize>, successors_only: bool,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<[bool; 128], ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    if blocks > 128 || start >= blocks || removed.is_some_and(|block| block >= blocks) {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let mut reached = [false; 128];
    if !successors_only && removed != Some(start) { reached[start] = true; }
    for edge in rows.successors() {
        budget.charge_work(1)?;
        let from = edge.id().source().get() as usize;
        let to = edge.edge().target().index() as usize;
        if from >= blocks || to >= blocks { return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch); }
        if successors_only && from == start && removed != Some(to) { reached[to] = true; }
    }
    for _ in 0..128 {
        let mut changed = false;
        for edge in rows.successors() {
            budget.charge_work(1)?;
            let from = edge.id().source().get() as usize;
            let to = edge.edge().target().index() as usize;
            if reached[from] && removed != Some(to) && !reached[to] {
                reached[to] = true;
                changed = true;
            }
        }
        if !changed { break; }
    }
    Ok(reached)
}
fn require_ordered_wave64_holder_storage_v1(
    owner: &ProductionSemanticSsaOwnerV1, function: SemanticFunctionIdV1, local: u32,
    borrow: (SemanticBlockIdV1, u32), initial_live: Option<(usize, usize)>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    let held = owner.occurrence_storage().ok_or(ArgumentResourceV1::Accounting)?.retained_storage();
    if budget.storage() < held { return Err(ArgumentResourceV1::Accounting.into()); }
    let rows = owner.occurrences_v1().and_then(|capture| capture.function(function))
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if !std::ptr::eq(rows.owner(), owner) { return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch); }
    let body = &owner.source_semantic().functions()[function.index() as usize];
    let blocks = body.blocks().len();
    let borrow_block = borrow.0.index() as usize;
    let borrow_statement = borrow.1 as usize;
    let entry = body.entry().index() as usize;
    let before_borrow = wave64_holder_source_reachability_v1(&rows, blocks, entry, Some(borrow_block), false, budget)?;
    let after_borrow = wave64_holder_source_reachability_v1(&rows, blocks, borrow_block, None, true, budget)?;
    if let Some((live_block, live_statement)) = initial_live {
        let bypass_live = wave64_holder_source_reachability_v1(&rows, blocks, entry, Some(live_block), false, budget)?;
        if (live_block == borrow_block && live_statement >= borrow_statement)
            || (live_block != borrow_block && bypass_live[borrow_block]) || after_borrow[live_block] {
            return Err(unsupported(function.index(), Some(live_block as u32), Some(live_statement as u32),
                "checked Wave64 holder initial storage live does not precede and dominate its unique borrow"));
        }
    }
    let mut definitions = 0usize;
    for event in rows.events() {
        budget.charge_work(1)?;
        if event.site() == (fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
            block: fe2o3_mir_model::SsaBlockIdV1::new(borrow.0.index()), statement: borrow.1,
        }) && event.role() == fe2o3_pliron::ProductionSemanticSsaEventRoleV1::DestinationDefine
            && event.event() == fe2o3_mir_model::SsaEventV1::Define(fe2o3_mir_model::SsaVariableIdV1::new(local)) {
            if !event.is_reachable() || !event.is_promoted()
                || !matches!(event.resolved(), Some(fe2o3_mir_model::SsaResolvedEventV1::Define { variable, .. }) if variable.get() == local) {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            definitions += 1;
        }
        let fe2o3_mir_model::SsaEventV1::Use(variable) = event.event() else { continue };
        if variable.get() != local { continue; }
        let (block, statement) = match event.site() {
            fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement { block, statement } =>
                (block.get() as usize, statement as usize),
            fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Terminator { block } => (block.get() as usize, usize::MAX),
        };
        if block >= blocks || before_borrow[block] || (block == borrow_block && statement <= borrow_statement) {
            return Err(unsupported(function.index(), Some(block as u32), None,
                "checked Wave64 holder source read is not dominated by its unique borrow"));
        }
    }
    if definitions != 1 { return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch); }
    for (bi, block) in body.blocks().iter().enumerate() {
        for (si, statement) in block.statements().iter().enumerate() {
            budget.charge_work(1)?;
            if !matches!(statement.kind(), SemanticStatementKindV1::StorageDead(value) if value.index() == local) { continue; }
            let after_dead = wave64_holder_source_reachability_v1(&rows, blocks, bi, None, true, budget)?;
            if after_dead[borrow_block] || (bi == borrow_block && si < borrow_statement) {
                return Err(unsupported(function.index(), Some(bi as u32), Some(si as u32),
                    "checked Wave64 holder storage end can reach its borrow again"));
            }
            for event in rows.events() {
                budget.charge_work(1)?;
                if !matches!(event.event(), fe2o3_mir_model::SsaEventV1::Use(variable) if variable.get() == local) { continue; }
                let (use_block, use_statement) = match event.site() {
                    fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement { block, statement } =>
                        (block.get() as usize, statement as usize),
                    fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Terminator { block } => (block.get() as usize, usize::MAX),
                };
                if use_block >= blocks || after_dead[use_block] || (use_block == bi && use_statement > si) {
                    return Err(unsupported(function.index(), Some(bi as u32), Some(si as u32),
                        "checked Wave64 holder storage end can precede an original source read"));
                }
            }
        }
    }
    Ok(())
}

fn require_unwritten_wave64_local_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    local: u32,
    original_borrow: Option<(SemanticBlockIdV1, u32)>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let declaration = owner
        .source_semantic()
        .functions()
        .get(function.index() as usize)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let mut writes = 0usize;
    let mut initial_live = None;
    let mut has_storage_markers = false;
    for (bi, block) in declaration.blocks().iter().enumerate() {
        for (si, statement) in block.statements().iter().enumerate() {
            budget.charge_work(2)?;
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment)
                    if assignment.destination().local().index() == local =>
                {
                    if original_borrow
                        != Some((SemanticBlockIdV1::from_index(bi as u32), si as u32))
                        || !assignment.destination().projections().is_empty()
                        || !matches!(
                            assignment.value().kind(),
                            SemanticRvalueKindV1::Borrow {
                                kind: SemanticBorrowKindV1::Shared,
                                ..
                            }
                        )
                    {
                        return Err(unsupported(
                            function.index(),
                            Some(bi as u32),
                            Some(si as u32),
                            "checked Wave64 holder has another original write",
                        ));
                    }
                    writes += 1;
                }
                SemanticStatementKindV1::StorageLive(value) if value.index() == local => {
                    if original_borrow.is_none() || initial_live.replace((bi, si)).is_some() {
                        return Err(unsupported(function.index(), Some(bi as u32), Some(si as u32),
                            "checked Wave64 holder allocation lifetime restarted or changed for a formal"));
                    }
                    has_storage_markers = true;
                }
                SemanticStatementKindV1::StorageDead(value) if value.index() == local => {
                    if original_borrow.is_none() {
                        return Err(unsupported(function.index(), Some(bi as u32), Some(si as u32),
                            "checked Wave64 formal has a storage lifetime change"));
                    }
                    has_storage_markers = true;
                }
                SemanticStatementKindV1::Deinitialize(place)
                | SemanticStatementKindV1::SetDiscriminant { place, .. }
                    if place.local().index() == local =>
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                SemanticStatementKindV1::Store(store)
                    if store.destination().local().index() == local =>
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                SemanticStatementKindV1::AtomicRmw(atomic)
                    if atomic.destination().local().index() == local
                        || atomic.address().local().index() == local =>
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                SemanticStatementKindV1::AtomicCompareExchange(atomic)
                    if atomic.destination().local().index() == local
                        || atomic.address().local().index() == local =>
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                _ => {}
            }
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
            if call
                .destination()
                .is_some_and(|destination| destination.place().local().index() == local)
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        }
    }
    if writes != usize::from(original_borrow.is_some()) {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    if has_storage_markers {
        require_ordered_wave64_holder_storage_v1(owner, function, local,
            original_borrow.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?, initial_live, budget)?;
    }
    Ok(())
}
fn closed_wave64_scalar_projection_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    place: &SemanticPlaceV1,
) -> bool {
    matches!(
        place
            .projections()
            .first()
            .map(|projection| projection.kind()),
        Some(SemanticProjectionKindV1::Dereference)
    ) && place
        .projections()
        .iter()
        .skip(1)
        .all(|projection| matches!(projection.kind(), SemanticProjectionKindV1::Field(_)))
        && matches!(
            owner
                .source_semantic()
                .types()
                .get(place.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32 | 64
            }))
        )
}
fn require_closed_wave64_reference_holder_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    local: u32,
    original_borrow: Option<(SemanticBlockIdV1, u32)>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    require_unwritten_wave64_local_v1(owner, function, local, original_borrow, budget)?;
    let declaration = &owner.source_semantic().functions()[function.index() as usize];
    for (bi, block) in declaration.blocks().iter().enumerate() {
        for (si, statement) in block.statements().iter().enumerate() {
            budget.charge_work(2)?;
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    assignment.value().kind().try_visit_operands(|operand| {
                        budget.charge_work(1)?;
                        if let SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) =
                            operand
                        {
                            if place.local().index() == local
                                && !closed_wave64_scalar_projection_v1(owner, place)
                            {
                                return Err(unsupported(
                                    function.index(),
                                    Some(bi as u32),
                                    Some(si as u32),
                                    "checked Wave64 reference alias or escape",
                                ));
                            }
                        }
                        Ok(())
                    })?;
                    match assignment.value().kind() {
                        SemanticRvalueKindV1::Borrow { place, .. }
                        | SemanticRvalueKindV1::AddressOf { place, .. }
                        | SemanticRvalueKindV1::Length(place)
                        | SemanticRvalueKindV1::Discriminant(place)
                            if place.local().index() == local =>
                        {
                            return Err(unsupported(
                                function.index(),
                                Some(bi as u32),
                                Some(si as u32),
                                "checked Wave64 reference address or reborrow escape",
                            ));
                        }
                        SemanticRvalueKindV1::Load(load)
                            if load.source().local().index() == local =>
                        {
                            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                        }
                        _ => {}
                    }
                }
                SemanticStatementKindV1::Store(store) => {
                    if let SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) =
                        store.value()
                    {
                        if place.local().index() == local {
                            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                        }
                    }
                }
                _ => {}
            }
        }
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => {
                for (argument, operand) in call.arguments().iter().enumerate() {
                    budget.charge_work(2)?;
                    let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand
                    else {
                        continue;
                    };
                    if place.local().index() != local
                        || closed_wave64_scalar_projection_v1(owner, place)
                    {
                        continue;
                    }
                    if !place.projections().is_empty() {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    match owner
                        .source_semantic()
                        .callables()
                        .get(call.callee().index() as usize)
                    {
                        Some(SemanticCallableDeclV1::Defined { .. }) => {
                            let checked = budget.checked_wave64_call_v1(
                                owner,
                                function,
                                SemanticBlockIdV1::from_index(bi as u32),
                                &mut |view, _| {
                                    if !std::ptr::eq(view.owner(), owner)
                                        || !std::ptr::eq(view.source_call(), call)
                                        || argument > 1
                                    {
                                        return Err(
                                            ProductionSemanticKirErrorV1::CorrespondenceMismatch,
                                        );
                                    }
                                    Ok(())
                                },
                            )?;
                            if !checked {
                                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                            }
                        }
                        Some(SemanticCallableDeclV1::CompilerIntrinsic {
                            operation:
                                SemanticCompilerIntrinsicOperationV1::Gfx942Wave64ShuffleIndex {
                                    ..
                                },
                            ..
                        }) if argument == 0 => {}
                        _ => {
                            return Err(unsupported(
                                function.index(),
                                Some(bi as u32),
                                None,
                                "checked Wave64 reference foreign call escape",
                            ));
                        }
                    }
                }
            }
            SemanticTerminatorKindV1::TailCall(_) => {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            _ => {}
        }
    }
    Ok(())
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn lower_checked_wave64_context_borrow_v1(
        &mut self,
        block: SemanticBlockIdV1,
        statement: u32,
        result_type: SemanticTypeIdV1,
        value: &SemanticRvalueKindV1,
        expected: Wave64NominalReferenceDescriptorV1,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let owner = self
            .wave64_references
            .owner
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let mut result = None;
        self.with_emission_budget_v1(|this, budget| {
            let checked = budget.checked_wave64_context_borrow_v1(
                owner,
                this.semantic_function,
                block,
                statement,
                &mut |view, budget| {
                    if !std::ptr::eq(view.owner(), owner)
                        || !std::ptr::eq(view.source_rvalue().kind(), value)
                        || Wave64NominalReferenceDescriptorV1::from_checked_context_borrow_v1(view)?
                            != expected
                        || view.reference_type() != result_type
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    require_closed_wave64_reference_holder_v1(
                        owner,
                        this.semantic_function,
                        expected.local,
                        Some((block, statement)),
                        budget,
                    )?;
                    let (function, issuer_block, local) = view.issuer();
                    if function != this.semantic_function
                        || !matches!(
                            this.locals.get(local.index() as usize),
                            Some(Some(SemanticValueBindingV1::CollectiveContext))
                        )
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    let SemanticTerminatorKindV1::Call(original) =
                        owner.source_semantic().functions()[function.index() as usize].blocks()
                            [issuer_block.index() as usize]
                            .terminator()
                            .kind()
                    else {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    };
                    if !std::ptr::eq(original, view.source_call()) {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    result = Some(expected.reconstruct_v1(this.types, result_type, &[])?);
                    Ok(())
                },
            )?;
            if !checked || result.is_none() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            Ok(())
        })?;
        Ok(result)
    }
}

fn wave64_original_call_argument_ssa_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    argument: u32,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
    let SemanticTerminatorKindV1::Call(call) = owner.source_semantic().functions()
        [function.index() as usize]
        .blocks()[block.index() as usize]
        .terminator()
        .kind()
    else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    let Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) =
        call.arguments().get(argument as usize)
    else {
        return Err(unsupported(
            function.index(),
            Some(block.index()),
            None,
            "nominal call transport requires actual original SSA operand",
        ));
    };
    if !place.projections().is_empty() {
        return Err(unsupported(
            function.index(),
            Some(block.index()),
            None,
            "nominal call projected source requires exact projected SSA binding join",
        ));
    }
    let occurrences = owner
        .occurrences_v1()
        .and_then(|rows| rows.function(function))
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let mut actual = None;
    for event in occurrences.events() {
        budget.charge_work(2)?;
        if event.site()
            == (fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Terminator {
                block: SsaBlockIdV1::new(block.index()),
            })
            && event.operand()
                == fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::CallArgument(argument)
            && event.role() == fe2o3_pliron::ProductionSemanticSsaEventRoleV1::BaseUse
        {
            let Some(SsaResolvedEventV1::Use { value, .. }) = event.resolved() else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            if !event.is_promoted() || !event.is_reachable() || actual.replace(value).is_some() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        }
    }
    actual.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
}
fn same_wave64_original_binding_v1(
    left: &SemanticValueBindingV1,
    right: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
    depth: usize,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    if depth >= 64 {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    match (left, right) {
        (SemanticValueBindingV1::Unit, SemanticValueBindingV1::Unit)
        | (SemanticValueBindingV1::CollectiveContext, SemanticValueBindingV1::CollectiveContext) => {
            Ok(true)
        }
        (
            SemanticValueBindingV1::Value { id: a, ty: at },
            SemanticValueBindingV1::Value { id: b, ty: bt },
        ) => Ok(a == b && invocation_equal_types_v1(at, bt, budget)?),
        (SemanticValueBindingV1::Aggregate(a), SemanticValueBindingV1::Aggregate(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (a, b) in a.iter().zip(b) {
                if !same_wave64_original_binding_v1(a, b, budget, depth + 1)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

// Fresh replay scopes borrow the real immutable source occurrence payload.
// Callers select the Wave64 path before entering; this private scope accounts
// the actual owner receipt, refunds only that input charge on every exit, and
// preserves body-owned output charges and all failure history.
fn with_wave64_retained_source_input_v1<'work, R>(
    owner: &ProductionSemanticSsaOwnerV1,
    budget: &mut ArgumentBudgetV1<'work>,
    body: impl FnOnce(&mut ArgumentBudgetV1<'work>) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    let receipt = owner.occurrence_storage().ok_or(ArgumentResourceV1::Accounting)?;
    let bytes = receipt.retained_storage();
    let ledger = budget.work_ledger_identity_v1();
    let floor = budget.storage();
    budget.reserve_storage(bytes)?;
    let live_input = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(budget)));
    if budget.work_ledger_identity_v1() != ledger || budget.storage() < live_input {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let retained_output = budget.storage().checked_sub(live_input).ok_or(ArgumentResourceV1::Accounting)?;
    let output_floor = argument_sum_v1(&[floor, retained_output])?;
    let release = budget.storage().checked_sub(output_floor).ok_or(ArgumentResourceV1::Accounting)?;
    if release != bytes {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.release_storage(release)?;
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

// Arithmetic invariant for the existing reduction's closed source counter:
// one u32 init32, then only self >> 1. This issues no EXEC/loop authority.
fn checked_wave64_shift_counter_bound_v1(
    owner: &ProductionSemanticSsaOwnerV1, function: SemanticFunctionIdV1, local: u32,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<u128>, ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    let source = owner.source_semantic();
    let body = source.functions().get(function.index() as usize)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let Some(declaration) = body.locals().get(local as usize) else { return Ok(None) };
    let ty = declaration.ty();
    if declaration.role() != SemanticLocalRoleV1::Temporary
        || source.types().get(ty.index() as usize).is_none_or(|ty|
            ty.rust_type_kind() != fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1::Ordinary
            || !matches!(ty.shape(), SemanticTypeShapeV1::Scalar(
                SemanticScalarTypeV1::Integer { signed: false, bits: 32 }))) { return Ok(None); }
    let mut init = None;
    let mut live = None;
    let mut definitions = 0usize;
    let mut shifts = 0usize;
    for (bi, block) in body.blocks().iter().enumerate() {
        for (si, statement) in block.statements().iter().enumerate() {
            budget.charge_work(2)?;
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    if assignment.destination().local().index() == local {
                        if !assignment.destination().projections().is_empty()
                            || assignment.destination().ty() != ty || assignment.value().result_type() != ty { return Ok(None); }
                        match assignment.value().kind() {
                            SemanticRvalueKindV1::Use(operand)
                                if exact_unsigned_semantic_constant_v1(source.types(), operand, ty) == Some(32) => {
                                if init.replace((bi, si)).is_some() { return Ok(None); }
                            }
                            SemanticRvalueKindV1::Binary { operation: SemanticBinaryOpV1::ShiftRight, left, right } => {
                                if !matches!(left, SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
                                    if place.local().index() == local && place.projections().is_empty() && place.ty() == ty)
                                    || !matches!(right, SemanticOperandV1::Constant(constant)
                                        if matches!(source.types().get(constant.ty().index() as usize).map(SemanticTypeDeclV1::shape),
                                            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { bits: 32, .. })))
                                        && matches!(constant.value(), SemanticConstantValueV1::Scalar(value) if value.size_bytes() == 4 && value.bits() == 1)) {
                                    return Ok(None);
                                }
                                shifts += 1;
                            }
                            _ => return Ok(None),
                        }
                        definitions += 1;
                    }
                    match assignment.value().kind() {
                        SemanticRvalueKindV1::Borrow { place, .. } | SemanticRvalueKindV1::AddressOf { place, .. }
                        | SemanticRvalueKindV1::Length(place) | SemanticRvalueKindV1::Discriminant(place)
                            if place.local().index() == local => return Ok(None),
                        SemanticRvalueKindV1::Load(load) if load.source().local().index() == local => return Ok(None),
                        _ => {}
                    }
                }
                SemanticStatementKindV1::StorageLive(value) if value.index() == local => {
                    if live.replace((bi, si)).is_some() { return Ok(None); }
                }
                SemanticStatementKindV1::Store(store) if store.destination().local().index() == local => return Ok(None),
                SemanticStatementKindV1::Deinitialize(place) | SemanticStatementKindV1::SetDiscriminant { place, .. }
                    if place.local().index() == local => return Ok(None),
                SemanticStatementKindV1::AtomicRmw(atomic)
                    if atomic.destination().local().index() == local || atomic.address().local().index() == local => return Ok(None),
                SemanticStatementKindV1::AtomicCompareExchange(atomic)
                    if atomic.destination().local().index() == local || atomic.address().local().index() == local => return Ok(None),
                _ => {}
            }
        }
        if matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(call)
            if call.destination().is_some_and(|destination| destination.place().local().index() == local)) { return Ok(None); }
    }
    let Some((init_block, init_statement)) = init else { return Ok(None) };
    if shifts == 0 { return Ok(None); }
    let held = owner.occurrence_storage().ok_or(ArgumentResourceV1::Accounting)?.retained_storage();
    if budget.storage() < held { return Err(ArgumentResourceV1::Accounting.into()); }
    let rows = owner.occurrences_v1().and_then(|capture| capture.function(function))
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if !std::ptr::eq(rows.owner(), owner) { return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch); }
    let plan = owner.plan_for_function(function).ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?.plan();
    budget.charge_work(plan.promoted_variables().len() + plan.entry_definitions().len())?;
    if !plan.promoted_variables().iter().any(|variable| variable.get() == local)
        || plan.entry_definitions().iter().any(|argument| argument.variable().get() == local) { return Ok(None); }
    let blocks = body.blocks().len();
    let entry = body.entry().index() as usize;
    let bypass_init = wave64_holder_source_reachability_v1(&rows, blocks, entry, Some(init_block), false, budget)?;
    let after_init = wave64_holder_source_reachability_v1(&rows, blocks, init_block, None, true, budget)?;
    if after_init[init_block] { return Ok(None); }
    if let Some((live_block, live_statement)) = live {
        let bypass_live = wave64_holder_source_reachability_v1(&rows, blocks, entry, Some(live_block), false, budget)?;
        if (live_block == init_block && live_statement >= init_statement)
            || (live_block != init_block && bypass_live[init_block]) || after_init[live_block] { return Ok(None); }
    }
    let mut captured_definitions = 0usize;
    for event in rows.events() {
        budget.charge_work(1)?;
        if event.event().variable().get() != local { continue; }
        if !event.is_reachable() || !event.is_promoted() { return Ok(None); }
        let (block, statement) = match event.site() {
            fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement { block, statement } =>
                (block.get() as usize, statement as usize),
            fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Terminator { block } => (block.get() as usize, usize::MAX),
        };
        if block >= blocks { return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch); }
        match event.event() {
            fe2o3_mir_model::SsaEventV1::Define(_) => {
                if event.role() != fe2o3_pliron::ProductionSemanticSsaEventRoleV1::DestinationDefine
                    || !matches!(event.resolved(), Some(fe2o3_mir_model::SsaResolvedEventV1::Define { variable, .. }) if variable.get() == local) {
                    return Ok(None);
                }
                captured_definitions += 1;
            }
            fe2o3_mir_model::SsaEventV1::Use(_) => {
                if !matches!(event.resolved(), Some(fe2o3_mir_model::SsaResolvedEventV1::Use { variable, .. }) if variable.get() == local)
                    || bypass_init[block] || (block == init_block && statement <= init_statement) { return Ok(None); }
            }
            fe2o3_mir_model::SsaEventV1::Kill(_) => {
                let original = body.blocks()[block].statements().get(statement).map(|statement| statement.kind());
                if !matches!(original, Some(SemanticStatementKindV1::StorageLive(value) | SemanticStatementKindV1::StorageDead(value))
                    if value.index() == local) { return Ok(None); }
            }
        }
    }
    if captured_definitions != definitions { return Ok(None); }
    for (bi, block) in body.blocks().iter().enumerate() {
        for (si, statement) in block.statements().iter().enumerate() {
            budget.charge_work(1)?;
            if !matches!(statement.kind(), SemanticStatementKindV1::StorageDead(value) if value.index() == local) { continue; }
            let after_dead = wave64_holder_source_reachability_v1(&rows, blocks, bi, None, true, budget)?;
            if after_dead[init_block] || (bi == init_block && si < init_statement) { return Ok(None); }
            for event in rows.events() {
                budget.charge_work(1)?;
                if !matches!(event.event(), fe2o3_mir_model::SsaEventV1::Use(variable) if variable.get() == local) { continue; }
                let (use_block, use_statement) = match event.site() {
                    fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement { block, statement } =>
                        (block.get() as usize, statement as usize),
                    fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Terminator { block } => (block.get() as usize, usize::MAX),
                };
                if use_block >= blocks || after_dead[use_block] || (use_block == bi && use_statement > si) { return Ok(None); }
            }
        }
    }
    Ok(Some(33))
}
impl SemanticFunctionLoweringV1<'_, '_> {
    fn seed_checked_wave64_shift_parameter_v1(
        &mut self, block: SemanticBlockIdV1, local: u32, source_value: SsaValueV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let Some(owner) = self.wave64_references.owner else { return Ok(()) };
        self.with_emission_budget_v1(|this, budget| {
            budget.charge_work(4)?;
            let declaration = this.function.locals().get(local as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if declaration.role() != SemanticLocalRoleV1::Temporary { return Ok(()); }
            // This is an attempt selector only; the live source callback below
            // independently rederives the complete invariant before insertion.
            let mut candidate = false;
            for source_block in this.function.blocks() {
                for statement in source_block.statements() {
                    budget.charge_work(1)?;
                    if matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                        if assignment.destination().local().index() == local
                        && matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(operand)
                            if exact_unsigned_semantic_constant_v1(this.types, operand, declaration.ty()) == Some(32))) { candidate = true; }
                }
            }
            if !candidate { return Ok(()); }
            budget.checked_wave64_helper_v1(owner, this.semantic_function, &mut |view, budget| {
                if !std::ptr::eq(view.representative().owner(), owner)
                    || !std::ptr::eq(view.representative().helper_declaration(), this.function) {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                if !matches!(view.representative().argument(0), Some(wave64_reference_call_v1::ArgumentV1::Snapshot { .. })) {
                    return Ok(());
                }
                let Some(bound) = checked_wave64_shift_counter_bound_v1(owner, this.semantic_function, local, budget)? else { return Ok(()) };
                let plan = owner.plan_for_function(this.semantic_function).ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?.plan();
                let variables = plan.transport_variables(SsaBlockIdV1::new(block.index()))
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                budget.charge_work(variables.len())?;
                if !variables.iter().any(|variable| variable.get() == local)
                    || source_value != (SsaValueV1::BlockArgument { block: SsaBlockIdV1::new(block.index()),
                        variable: fe2o3_mir_model::SsaVariableIdV1::new(local) }) { return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch); }
                charge_execution_cfg_lookup_v29(this.semantic_ssa_bindings.len(), budget)?;
                let Some(SemanticValueBindingV1::Value { id, ty }) = this.semantic_ssa_bindings.get(&source_value) else {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                };
                if *ty != Type::Scalar(ScalarType::U32)
                    || !matches!(this.locals.get(local as usize).and_then(Option::as_ref),
                        Some(SemanticValueBindingV1::Value { id: current, ty: Type::Scalar(ScalarType::U32) }) if current == id) {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let id = *id;
                charge_execution_cfg_lookup_v29(this.emitted_unsigned_exclusive_bounds.len(), budget)?;
                if !this.emitted_unsigned_exclusive_bounds.contains_key(&id) {
                    reserve_execution_cfg_map_entry_v29::<ValueId, u128>(this.emitted_unsigned_exclusive_bounds.len(), budget)?;
                }
                this.emitted_unsigned_exclusive_bounds.insert(id, bound);
                Ok(())
            })?;
            Ok(())
        })
    }
}
