#[derive(Clone, Copy)]
struct SourceIssuedActualValueV29<'a> {
    id: ValueId,
    ty: &'a Type,
    operation: Option<&'a Operation>,
    input: Option<usize>,
}

#[derive(Clone, Copy)]
struct SourceIssuedRootArgumentV29 {
    argument: u32,
    local: usize,
    ty: SemanticTypeIdV1,
    entry: bool,
    value: Option<SsaValueV1>,
    physical: Option<usize>,
    input: Option<ValueId>,
}

#[derive(Clone, Copy)]
struct SourceIssuedRootTransportV29 {
    receiver: ValueId,
    input: ValueId,
    checked: bool,
}

struct SourceIssuedActualV29<'a> {
    values: Vec<SourceIssuedActualValueV29<'a>>,
    root_arguments: Vec<SourceIssuedRootArgumentV29>,
}

fn source_issued_root_binding_headers_v29() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            std::mem::size_of::<T>(),
            argument_product_v1(
                2,
                std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>(),
            )?,
        ])
    }
    argument_sum_v1(&[
        h::<&SourceAddressSourceIndexV29<'_>>()?,
        h::<&PendingInstanceSidecarsV29>()?,
        h::<&SemanticKirParameterBindingV1>()?,
        h::<Option<&SemanticKirParameterBindingV1>>()?,
        h::<std::slice::Iter<'_, SemanticKirParameterBindingV1>>()?,
        h::<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>()?,
        h::<Option<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>>()?,
        h::<&mut SourceIssuedRootArgumentV29>()?,
        h::<Option<&mut SourceIssuedRootArgumentV29>>()?,
        h::<&SourceIssuedRootArgumentV29>()?,
        h::<Option<&SourceIssuedRootArgumentV29>>()?,
        h::<std::slice::Iter<'_, SourceIssuedRootArgumentV29>>()?,
        h::<SourceIssuedRootArgumentV29>()?,
        h::<SourceIssuedActualValueV29<'_>>()?,
        h::<SourceIssuedRootTransportV29>()?,
        h::<Option<SsaValueV1>>()?,
        h::<ValueId>()?,
        h::<Option<ValueId>>()?,
        h::<&ValueId>()?,
        h::<Option<&ValueId>>()?,
        h::<usize>()?,
        h::<Option<usize>>()?,
        h::<bool>()?,
        h::<()>()?,
    ])
}

impl<'a> SourceIssuedActualV29<'a> {
    fn from_function(
        function: &'a Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Result<Self, ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<Vec<SourceIssuedActualValueV29<'_>>>(),
            std::mem::size_of::<
                Result<Vec<SourceIssuedActualValueV29<'_>>, ProductionSemanticKirErrorV1>,
            >(),
        ])?)?;
        let body = function.body.as_ref().ok_or_else(source_issued_error_v29)?;
        if body.parameters.len() != function.signature.parameters.len() {
            return Err(source_issued_error_v29());
        }
        let mut count = body.parameters.len();
        for block in &body.blocks {
            budget.charge_work(2)?;
            count = argument_sum_v1(&[count, block.parameters.len()])?;
            for operation in &block.operations {
                budget.charge_work(1)?;
                count = argument_sum_v1(&[count, operation.results.len()])?;
            }
        }
        let mut values = emission_vec_v1(count, budget)?;
        for (ordinal, (&id, ty)) in body
            .parameters
            .iter()
            .zip(&function.signature.parameters)
            .enumerate()
        {
            budget.charge_work(1)?;
            values.push(SourceIssuedActualValueV29 {
                id,
                ty,
                operation: None,
                input: Some(ordinal),
            });
        }
        for block in &body.blocks {
            budget.charge_work(1)?;
            for value in &block.parameters {
                budget.charge_work(1)?;
                values.push(SourceIssuedActualValueV29 {
                    id: value.id,
                    ty: &value.ty,
                    operation: None,
                    input: None,
                });
            }
            for operation in &block.operations {
                budget.charge_work(1)?;
                for value in &operation.results {
                    budget.charge_work(1)?;
                    values.push(SourceIssuedActualValueV29 {
                        id: value.id,
                        ty: &value.ty,
                        operation: Some(operation),
                        input: None,
                    });
                }
            }
        }
        call_splice_sort_work_v1(values.len(), budget).map_err(source_address_call_error_v29)?;
        values.sort_unstable_by_key(|row| row.id);
        for pair in values.windows(2) {
            budget.charge_work(1)?;
            if pair[0].id == pair[1].id {
                return Err(source_issued_error_v29());
            }
        }
        Ok(Self {
            values,
            root_arguments: Vec::new(),
        })
    }

    fn bind_root_arguments(
        &mut self,
        references: &SourceReferencePlanV29<'_, '_>,
        source_index: &SourceAddressSourceIndexV29<'_>,
        actual: &Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let instances = references.instances;
        references.check_owner(instances, budget)?;
        if !self.root_arguments.is_empty() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Vec<SourceIssuedRootArgumentV29>>(),
            std::mem::size_of::<
                Result<Vec<SourceIssuedRootArgumentV29>, ProductionSemanticKirErrorV1>,
            >(),
            source_issued_root_binding_headers_v29()?,
        ])?)?;
        let root = instances
            .instance(instances.root())
            .ok_or_else(source_issued_error_v29)?;
        let function = root.declaration();
        let semantic = instances.owner().source_semantic();
        let body = actual.body.as_ref().ok_or_else(source_issued_error_v29)?;
        if body.parameters.len() != actual.signature.parameters.len()
            || !actual.signature.results.is_empty()
        {
            return Err(source_issued_error_v29());
        }
        check_argument_function_abi_v1(
            function,
            root.function(),
            SemanticKirFunctionRoleV1::KernelEntry,
        )?;
        let occurrences = instances
            .occurrences(instances.root())
            .ok_or_else(source_issued_error_v29)?;
        let mut root_arguments =
            emission_vec_v1(function.abi().source_input_types().len(), budget)?;
        for (local, declaration) in function.locals().iter().enumerate() {
            budget.charge_work(2)?;
            if let SemanticLocalRoleV1::Argument(argument) = declaration.role() {
                if root_arguments.len() == function.abi().source_input_types().len() {
                    return Err(source_issued_error_v29());
                }
                root_arguments.push(SourceIssuedRootArgumentV29 {
                    argument,
                    local,
                    ty: declaration.ty(),
                    entry: false,
                    value: None,
                    physical: None,
                    input: None,
                });
            }
        }
        call_splice_sort_work_v1(root_arguments.len(), budget)
            .map_err(source_address_call_error_v29)?;
        root_arguments.sort_unstable_by_key(|row| row.argument);
        if root_arguments.len() != function.abi().source_input_types().len() {
            return Err(source_issued_error_v29());
        }
        for (argument, (row, &ty)) in root_arguments
            .iter()
            .zip(function.abi().source_input_types())
            .enumerate()
        {
            budget.charge_work(2)?;
            if row.argument as usize != argument || row.ty != ty {
                return Err(source_issued_error_v29());
            }
        }
        for definition in occurrences.entry_definitions() {
            budget.charge_work(3)?;
            let local = definition.variable().get() as usize;
            let declaration = function
                .locals()
                .get(local)
                .ok_or_else(source_issued_error_v29)?;
            if let SemanticLocalRoleV1::Argument(argument) = declaration.role() {
                let row = root_arguments
                    .get_mut(argument as usize)
                    .ok_or_else(source_issued_error_v29)?;
                if row.local != local || row.entry {
                    return Err(source_issued_error_v29());
                }
                row.entry = true;
                row.value = definition.value();
            }
        }
        // Shape selection is exactly the root emitter's service, with the same
        // original descriptor profile. Only ordinals survive; planned ValueIds
        // do not authorize placed output parameters.
        let mut ordinal = 0usize;
        for row in &mut root_arguments {
            let floor = budget.storage();
            source_reference_owned_prepay_v29::<KernelParameterShapeV1>(references, budget)?;
            prepay_argument_shape_v1(semantic, row.ty, budget)?;
            let shape = kernel_parameter_shape_with_descriptors_v29(
                semantic,
                function,
                row.argument,
                row.local,
                row.ty,
                references,
                budget,
            )?;
            match &shape {
                KernelParameterShapeV1::Direct(expected) => {
                    budget.charge_work(2)?;
                    if actual.signature.parameters.get(ordinal) != Some(expected) {
                        return Err(source_issued_error_v29());
                    }
                    row.physical = Some(ordinal);
                    ordinal = ordinal
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                }
                KernelParameterShapeV1::Components(components) => {
                    for (_, _, expected, _, _) in components {
                        budget.charge_work(2)?;
                        if actual.signature.parameters.get(ordinal) != Some(expected) {
                            return Err(source_issued_error_v29());
                        }
                        ordinal = ordinal
                            .checked_add(1)
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                    }
                }
            }
            drop(shape);
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )?;
        }
        if ordinal != body.parameters.len() {
            return Err(source_issued_error_v29());
        }
        // Direct root ABI bindings exist even when the original local is not
        // promoted. They identify real parameters; absence of an SSA value is
        // never repaired by inventing an archive entry or using a typed peer.
        let sidecar = source_index.sidecar(instances.root(), budget)?;
        if sidecar.source_call_instance != Some(instances.root()) {
            return Err(source_issued_error_v29());
        }
        for binding in &sidecar.parameter_bindings {
            budget.charge_work(8)?;
            if binding.correspondence_owner != root.function()
                || binding.semantic_function != root.function()
            {
                return Err(source_issued_error_v29());
            }
            let local = binding.semantic_local.index() as usize;
            let declaration = function
                .locals()
                .get(local)
                .ok_or_else(source_issued_error_v29)?;
            let SemanticLocalRoleV1::Argument(argument) = declaration.role() else {
                return Err(source_issued_error_v29());
            };
            let row = root_arguments
                .get_mut(argument as usize)
                .ok_or_else(source_issued_error_v29)?;
            let ordinal = row.physical.ok_or_else(source_issued_error_v29)?;
            if row.local != local
                || row.ty != declaration.ty()
                || !row.entry
                || row.input.is_some()
                || body.parameters.get(ordinal) != Some(&binding.kernel_ir_value)
                || self.value(binding.kernel_ir_value, budget)?.input != Some(ordinal)
            {
                return Err(source_issued_error_v29());
            }
            row.input = Some(binding.kernel_ir_value);
        }
        for row in &root_arguments {
            budget.charge_work(2)?;
            if row.physical.is_some() != row.input.is_some() {
                return Err(source_issued_error_v29());
            }
        }
        self.root_arguments = root_arguments;
        Ok(())
    }

    fn value(
        &self,
        id: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceIssuedActualValueV29<'a>, ProductionSemanticKirErrorV1> {
        charge_execution_cfg_lookup_v29(self.values.len(), budget)?;
        self.values
            .binary_search_by_key(&id, |row| row.id)
            .ok()
            .map(|index| self.values[index])
            .ok_or_else(source_issued_error_v29)
    }

    fn present(
        &self,
        selector: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ValueId>, ProductionSemanticKirErrorV1> {
        let row = self.value(selector, budget)?;
        budget.charge_work(4)?;
        if *row.ty == Type::BOOL {
            return Ok(Some(selector));
        }
        if let Some(Operation {
            kind:
                OperationKind::Cast {
                    kind: CastKind::ZeroExtend,
                    value,
                    to,
                },
            results,
        }) = row.operation
            && results.len() == 1
            && results[0].id == selector
            && results[0].ty == *to
            && to == row.ty
            && to.as_scalar().is_some_and(ScalarType::is_integer)
            && *self.value(*value, budget)?.ty == Type::BOOL
        {
            return Ok(Some(*value));
        }
        Ok(None)
    }
}

#[derive(Clone, Copy)]
struct SourceIssuedAccessV29 {
    instance: usize,
    anchor: usize,
    issuer_instance: ProductionCallInstanceIdV1,
    issuer: SsaValueV1,
    pointer: ValueId,
    issuer_pointer: ValueId,
    access: MemoryAccess,
    writing: bool,
    present: ValueId,
    block: BlockId,
    guard: Option<(BlockId, usize)>,
}

#[derive(Clone, Copy)]
struct SourceIssuedGuardV29 {
    present: ValueId,
    block: BlockId,
    edge: usize,
}

fn source_issued_constant_bool_v29(value: &Constant) -> Option<bool> {
    match value {
        Constant::Bool(value) => Some(*value),
        Constant::I8(value) => match value {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        Constant::I16(value) => match value {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        Constant::I32(value) => match value {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        Constant::I64(value) => match value {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        Constant::U8(value) => match value {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        Constant::U16(value) => match value {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        Constant::U32(value) => match value {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        Constant::U64(value) | Constant::Index(value) => match value {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        _ => None,
    }
}

fn source_issued_guards_v29(
    function: &Function,
    actual: &SourceIssuedActualV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<SourceIssuedGuardV29>, ProductionSemanticKirErrorV1> {
    let body = function.body.as_ref().ok_or_else(source_issued_error_v29)?;
    let mut rows = emission_vec_v1(body.blocks.len(), budget)?;
    for block in &body.blocks {
        budget.charge_work(3)?;
        let (selector, edge) = match block
            .terminator
            .as_ref()
            .ok_or_else(source_issued_error_v29)?
        {
            Terminator::ConditionalBranch { condition, .. } => {
                if *actual.value(*condition, budget)?.ty != Type::BOOL {
                    return Err(source_issued_error_v29());
                }
                (*condition, 0)
            }
            Terminator::Switch {
                selector, cases, ..
            } => {
                let mut selected = None;
                let mut absent = None;
                for (ordinal, case) in cases.iter().enumerate() {
                    budget.charge_work(3)?;
                    if case.value == 1 && selected.replace(ordinal).is_some() {
                        return Err(source_issued_error_v29());
                    }
                    if case.value == 0 && absent.replace(ordinal).is_some() {
                        return Err(source_issued_error_v29());
                    }
                }
                if selected.unwrap_or(cases.len()) == absent.unwrap_or(cases.len()) {
                    continue;
                }
                (*selector, selected.unwrap_or(cases.len()))
            }
            Terminator::IntegerSwitch {
                selector, cases, ..
            } => {
                let ty = actual.value(*selector, budget)?.ty;
                let mut selected = None;
                let mut absent = None;
                for (ordinal, case) in cases.iter().enumerate() {
                    budget.charge_work(4)?;
                    if case.value.ty() != *ty {
                        return Err(source_issued_error_v29());
                    }
                    if source_issued_constant_bool_v29(&case.value) == Some(true)
                        && selected.replace(ordinal).is_some()
                    {
                        return Err(source_issued_error_v29());
                    }
                    if source_issued_constant_bool_v29(&case.value) == Some(false)
                        && absent.replace(ordinal).is_some()
                    {
                        return Err(source_issued_error_v29());
                    }
                }
                if selected.unwrap_or(cases.len()) == absent.unwrap_or(cases.len()) {
                    continue;
                }
                (*selector, selected.unwrap_or(cases.len()))
            }
            _ => continue,
        };
        if let Some(present) = actual.present(selector, budget)? {
            rows.push(SourceIssuedGuardV29 {
                present,
                block: block.id,
                edge,
            });
        }
    }
    call_splice_sort_work_v1(rows.len(), budget).map_err(source_address_call_error_v29)?;
    rows.sort_unstable_by_key(|row| row.present);
    Ok(rows)
}

impl SourceIssuedOriginalV29<'_, '_, '_> {
    fn direct_call_operand<'a>(
        &'a self,
        call: &'a SemanticDirectCallV1,
        block: SemanticBlockIdV1,
        argument: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(&'a SemanticPlaceV1, &'a SemanticValueBindingV1), ProductionSemanticKirErrorV1>
    {
        budget.charge_work(2)?;
        let Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) =
            call.arguments().get(argument as usize)
        else {
            return Err(source_issued_error_v29());
        };
        if !place.projections().is_empty() {
            return Err(source_issued_error_v29());
        }
        let value = self.use_value(
            ExecutionSiteV29::Terminator {
                block: SsaBlockIdV1::new(block.index()),
            },
            ExecutionOperandV29::CallArgument(argument),
            place,
            budget,
        )?;
        Ok((place, self.archived(value, budget)?))
    }

    fn actual_issuer(
        &self,
        recipe: SourceIssuedRecipeV29,
        references: &SourceReferenceEmissionV29<'_, '_>,
        actual: &SourceIssuedActualV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        (SourceIssuedRootTransportV29, PendingSourceIssuedIssuerV29),
        ProductionSemanticKirErrorV1,
    > {
        references.check(budget)?;
        budget.charge_work(10)?;
        let source = self.instances.owner().source_semantic();
        let function = self
            .instances
            .instance(self.instance)
            .ok_or_else(source_issued_error_v29)?
            .declaration();
        let SemanticTerminatorKindV1::Call(call) = function.blocks()[recipe.block.index() as usize]
            .terminator()
            .kind()
        else {
            return Err(source_issued_error_v29());
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
            source.callables().get(call.callee().index() as usize)
        else {
            return Err(source_issued_error_v29());
        };
        if call.arguments().len() != 2 {
            return Err(source_issued_error_v29());
        }
        let (carrier, writes) = allocation_receiver_contract_v29(
            function,
            source.callables(),
            recipe.block,
            call,
            0,
            budget,
        )?;
        let (receiver_place, receiver) = self.direct_call_operand(call, recipe.block, 0, budget)?;
        // The first slice admits the existing exact borrowed allocation route.
        // A same-typed raw Value does not acquire nominal owner authority here.
        let SemanticValueBindingV1::SourceReference(receiver) = receiver else {
            return Err(source_issued_error_v29());
        };
        let value = source_reference_allocation_borrowed_v29(
            references.plan,
            source.types(),
            receiver,
            receiver_place.ty(),
            carrier,
            writes,
            budget,
        )?
        .ok_or_else(source_issued_error_v29)?;
        let loan = &references.plan.loans[receiver.origin.single_loan()?];
        let SourceReferenceRepresentationV29::ExistingAllocationBinding(anchor) =
            loan.representation
        else {
            return Err(source_issued_error_v29());
        };
        let expected = source_reference_anchor_type_v29(references.plan, anchor, carrier, budget)?
            .ok_or_else(source_issued_error_v29)?;
        let Type::Slice(slice) = &expected else {
            return Err(source_issued_error_v29());
        };
        if value.ty != expected
            || slice.address_space != AddressSpace::Global
            || slice.access != recipe.access
            || *slice.element != Type::Scalar(recipe.element)
        {
            return Err(source_issued_error_v29());
        }
        let transport = self.check_root_slice(anchor, value.id, actual, budget)?;
        let (_, witness) = self.direct_call_operand(call, recipe.block, 1, budget)?;
        let SemanticValueBindingV1::IndexWitness {
            id: index,
            index_space,
            disjoint,
            ..
        } = witness
        else {
            return Err(source_issued_error_v29());
        };
        match operation {
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }
                if *index_space == SemanticDisjointIndexSpaceV1::Index1d && !disjoint => {}
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
                index_space: required,
                ..
            } if index_space == required && *disjoint => {}
            _ => return Err(source_issued_error_v29()),
        }
        if *actual.value(*index, budget)?.ty != Type::INDEX {
            return Err(source_issued_error_v29());
        }
        let source_index = self.source_index;
        budget.charge_work(argument_product_v1(
            call_splice_search_work_v1(source_index.terminators.len()),
            2,
        )?)?;
        let ordinal = source_index
            .terminators
            .binary_search_by_key(
                &(self.instance.index(), recipe.block.index()),
                SourceAddressTerminatorV29::key,
            )
            .map_err(|_| source_issued_error_v29())?;
        let span = source_index.terminators[ordinal].span;
        let first = span
            .first_operation_ordinal
            .checked_add(span.operation_count)
            .and_then(|end| end.checked_sub(4))
            .filter(|first| *first >= span.first_operation_ordinal)
            .ok_or_else(source_issued_error_v29)?;
        let length = source_index.emitted.operation(
            self.instance,
            span.kernel_ir_block,
            first as usize,
            budget,
        )?;
        let compare = source_index.emitted.operation(
            self.instance,
            span.kernel_ir_block,
            first as usize + 1,
            budget,
        )?;
        let data = source_index.emitted.operation(
            self.instance,
            span.kernel_ir_block,
            first as usize + 2,
            budget,
        )?;
        let gep = source_index.emitted.operation(
            self.instance,
            span.kernel_ir_block,
            first as usize + 3,
            budget,
        )?;
        check_source_issued_tail_v29(
            SourceIssuedPhysicalV29::from(recipe),
            value.id,
            *index,
            length,
            compare,
            data,
            gep,
            budget,
        )?;
        budget.charge_work(4)?;
        let root_parameter = actual
            .value(transport.input, budget)?
            .input
            .ok_or_else(source_issued_error_v29)?;
        let [length] = length.results.as_slice() else {
            return Err(source_issued_error_v29());
        };
        let [data] = data.results.as_slice() else {
            return Err(source_issued_error_v29());
        };
        let retained = PendingSourceIssuedIssuerV29 {
            instance: self.instance,
            block: recipe.block,
            definition: recipe.issuer,
            root_parameter,
            root_input: transport.input,
            receiver: value.id,
            index: *index,
            length: length.id,
            present: recipe.present,
            data: data.id,
            pointer: recipe.pointer,
            element: recipe.element,
            access: recipe.access,
        };
        Ok((transport, retained))
    }

    fn check_root_slice(
        &self,
        anchor: SourceReferenceAnchorV29,
        slice: ValueId,
        actual: &SourceIssuedActualV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceIssuedRootTransportV29, ProductionSemanticKirErrorV1> {
        let root = self.instances.root();
        let sidecar = self.source_index.sidecar(root, budget)?;
        let archive = sidecar
            .execution_observation
            .as_ref()
            .ok_or_else(source_issued_error_v29)?;
        charge_execution_cfg_lookup_v29(actual.root_arguments.len(), budget)?;
        let index = actual
            .root_arguments
            .binary_search_by_key(&anchor.argument, |row| row.argument)
            .map_err(|_| source_issued_error_v29())?;
        let row = actual.root_arguments[index];
        if row.ty != anchor.ty || row.physical.is_none() {
            return Err(source_issued_error_v29());
        }
        let input = row.input.ok_or_else(source_issued_error_v29)?;
        let ordinal = row.physical.ok_or_else(source_issued_error_v29)?;
        if let Some(value) = row.value {
            let archived = archive.lookup_original_v29(self.instances, root, value, budget)?;
            let transport =
                source_issued_root_transport_v29(actual, ordinal, slice, archived, budget)?;
            if transport.input != input {
                return Err(source_issued_error_v29());
            }
            Ok(transport)
        } else {
            source_issued_root_input_transport_v29(actual, ordinal, input, slice, budget)
        }
    }
}

fn source_issued_root_transport_v29(
    actual: &SourceIssuedActualV29<'_>,
    ordinal: usize,
    receiver: ValueId,
    archived: &SemanticValueBindingV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceIssuedRootTransportV29, ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    let SemanticValueBindingV1::Value {
        id: input,
        ty: Type::Slice(_),
    } = archived
    else {
        return Err(source_issued_error_v29());
    };
    // The original entry archive must still be the exact physical ABI input.
    // A transported receiver is checked separately against actual CFG edges.
    check_source_issued_root_input_v29(actual, ordinal, *input, archived, budget)?;
    source_issued_root_input_transport_v29(actual, ordinal, *input, receiver, budget)
}

fn source_issued_root_input_transport_v29(
    actual: &SourceIssuedActualV29<'_>,
    ordinal: usize,
    input: ValueId,
    receiver: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceIssuedRootTransportV29, ProductionSemanticKirErrorV1> {
    budget.charge_work(5)?;
    let parameter = actual.value(input, budget)?;
    let Type::Slice(expected) = parameter.ty else {
        return Err(source_issued_error_v29());
    };
    if parameter.input != Some(ordinal) {
        return Err(source_issued_error_v29());
    }
    let Type::Slice(found) = actual.value(receiver, budget)?.ty else {
        return Err(source_issued_error_v29());
    };
    let Type::Scalar(element) = expected.element.as_ref() else {
        return Err(source_issued_error_v29());
    };
    if found.address_space != expected.address_space
        || found.access != expected.access
        || *found.element != Type::Scalar(*element)
    {
        return Err(source_issued_error_v29());
    }
    Ok(SourceIssuedRootTransportV29 {
        receiver,
        input,
        checked: false,
    })
}

fn check_source_issued_root_input_v29(
    actual: &SourceIssuedActualV29<'_>,
    ordinal: usize,
    slice: ValueId,
    archived: &SemanticValueBindingV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(5)?;
    let SemanticValueBindingV1::Value {
        id,
        ty: Type::Slice(expected),
    } = archived
    else {
        return Err(source_issued_error_v29());
    };
    // This first issuer profile has scalar elements. Keep equality fixed-size;
    // admitting nested elements needs the existing metered structural service.
    let Type::Scalar(element) = expected.element.as_ref() else {
        return Err(source_issued_error_v29());
    };
    let physical = actual.value(slice, budget)?;
    let Type::Slice(found) = physical.ty else {
        return Err(source_issued_error_v29());
    };
    if *id != slice
        || physical.input != Some(ordinal)
        || found.address_space != expected.address_space
        || found.access != expected.access
        || *found.element != Type::Scalar(*element)
    {
        return Err(source_issued_error_v29());
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct SourceIssuedPhysicalV29 {
    present: ValueId,
    pointer: ValueId,
    element: ScalarType,
    access: AccessMode,
}

impl From<SourceIssuedRecipeV29> for SourceIssuedPhysicalV29 {
    fn from(recipe: SourceIssuedRecipeV29) -> Self {
        Self {
            present: recipe.present,
            pointer: recipe.pointer,
            element: recipe.element,
            access: recipe.access,
        }
    }
}

fn check_source_issued_tail_v29(
    recipe: SourceIssuedPhysicalV29,
    slice: ValueId,
    index: ValueId,
    length: &Operation,
    compare: &Operation,
    data: &Operation,
    gep: &Operation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(22)?;
    let ([length_result], [present], [base], [pointer]) = (
        length.results.as_slice(),
        compare.results.as_slice(),
        data.results.as_slice(),
        gep.results.as_slice(),
    ) else {
        return Err(source_issued_error_v29());
    };
    let pointer_type = |ty: &Type| {
        matches!(ty, Type::Pointer(pointer) if *pointer.pointee == Type::Scalar(recipe.element)
        && pointer.address_space == AddressSpace::Global && pointer.access == recipe.access)
    };
    if length_result.ty != Type::INDEX
        || present.id != recipe.present
        || present.ty != Type::BOOL
        || pointer.id != recipe.pointer
        || !pointer_type(&base.ty)
        || !pointer_type(&pointer.ty)
        || !matches!(length.kind, OperationKind::SliceLength { slice: actual } if actual == slice)
        || !matches!(compare.kind, OperationKind::Compare { predicate: ComparePredicate::LessThan, lhs, rhs }
            if lhs == index && rhs == length_result.id)
        || !matches!(data.kind, OperationKind::SliceData { slice: actual } if actual == slice)
        || !matches!(gep.kind, OperationKind::GetElementPointer { base: actual, offset }
            if actual == base.id && offset == index)
    {
        return Err(source_issued_error_v29());
    }
    Ok(())
}
