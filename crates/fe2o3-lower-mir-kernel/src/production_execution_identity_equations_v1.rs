impl ExecutionIdentitySourceIndexV1<'_, '_> {
    fn selected_equations(
        &self,
        roots: &[usize],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(ExecutionIdentityEquationsV1, Vec<usize>), ProductionSemanticKirErrorV1> {
        let headers = argument_sum_v1(&[
            argument_product_v1(2, std::mem::size_of::<Vec<usize>>())?,
            std::mem::size_of::<Vec<bool>>(),
        ])?;
        budget.reserve_storage(headers)?;
        let mut mapping = emission_vec_v1(self.channels, budget)?;
        let mut owners = emission_vec_v1(self.channels, budget)?;
        let mut queued = emission_vec_v1(self.rows.len(), budget)?;
        let mut queue = emission_vec_v1(self.rows.len(), budget)?;
        budget.charge_work(argument_sum_v1(&[self.channels, self.rows.len()])?)?;
        mapping.resize(self.channels, usize::MAX);
        queued.resize(self.rows.len(), false);
        for (index, row) in self.rows.iter().enumerate() {
            budget.charge_work(argument_sum_v1(&[row.selection.count, 1])?)?;
            if row.selection.first != owners.len() {
                return Err(execution_identity_error_v1());
            }
            owners.resize(
                argument_sum_v1(&[owners.len(), row.selection.count])?,
                index,
            );
        }
        if owners.len() != self.channels {
            return Err(execution_identity_error_v1());
        }
        for &root in roots {
            budget.charge_work(2)?;
            let present = queued
                .get_mut(root)
                .ok_or_else(execution_identity_error_v1)?;
            if !*present {
                *present = true;
                queue.push(root);
            }
        }
        let mut output = ExecutionIdentityEquationsV1::default();
        let mut next = 0;
        while next < queue.len() {
            budget.charge_work(2)?;
            let row = self
                .rows
                .get(queue[next])
                .ok_or_else(execution_identity_error_v1)?;
            next = argument_sum_v1(&[next, 1])?;
            let first = output.rows.len();
            let first_input = output.inputs.len();
            self.append_equations(row, &mut output, budget)?;
            if output.rows.len() != argument_sum_v1(&[first, row.selection.count])? {
                return Err(execution_identity_error_v1());
            }
            for component in 0..row.selection.count {
                budget.charge_work(1)?;
                mapping[row.selection.first + component] = argument_sum_v1(&[first, component])?;
            }
            for &input in &output.inputs[first_input..] {
                budget.charge_work(2)?;
                let owner = *owners.get(input).ok_or_else(execution_identity_error_v1)?;
                if !queued[owner] {
                    queued[owner] = true;
                    queue.push(owner);
                }
            }
        }
        for input in &mut output.inputs {
            budget.charge_work(2)?;
            *input = *mapping
                .get(*input)
                .filter(|index| **index != usize::MAX)
                .ok_or_else(execution_identity_error_v1)?;
        }
        let temporary = argument_sum_v1(&[
            headers,
            argument_product_v1(owners.capacity(), std::mem::size_of::<usize>())?,
            argument_product_v1(queued.capacity(), std::mem::size_of::<bool>())?,
            argument_product_v1(queue.capacity(), std::mem::size_of::<usize>())?,
        ])?;
        drop((owners, queued, queue));
        budget.release_storage(temporary)?;
        Ok((output, mapping))
    }

    fn append_equations(
        &self,
        row: &ExecutionIdentitySourceRowV1,
        output: &mut ExecutionIdentityEquationsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        match row.kind {
            ExecutionIdentitySourceKindV1::Entry(ordinal)
            | ExecutionIdentitySourceKindV1::RetainedEntry(ordinal) => {
                self.entry_equations(row, ordinal, output, budget)
            }
            ExecutionIdentitySourceKindV1::Event(ordinal) => {
                self.assignment_equations(row, ordinal, output, budget)
            }
            ExecutionIdentitySourceKindV1::Edge(ordinal) => {
                self.call_equations(row, ordinal, output, budget)
            }
            ExecutionIdentitySourceKindV1::Phi(block)
            | ExecutionIdentitySourceKindV1::Header(block) => {
                self.phi_equations(row, block, output, budget)
            }
        }
    }

    fn alias_equations(
        &self,
        source: ExecutionIdentitySelectionV1,
        destination: ExecutionIdentitySelectionV1,
        output: &mut ExecutionIdentityEquationsV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        if source.ty != destination.ty || source.count != destination.count {
            return Err(execution_identity_error_v1());
        }
        for offset in 0..source.count {
            output.push(
                ExecutionIdentityEquationKindV1::Merge,
                &[argument_sum_v1(&[source.first, offset])?],
                budget,
            )?;
        }
        Ok(())
    }

    fn entry_equations(
        &self,
        row: &ExecutionIdentitySourceRowV1,
        ordinal: usize,
        output: &mut ExecutionIdentityEquationsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let occurrences = self
            .instances
            .occurrences(row.instance)
            .ok_or_else(execution_identity_error_v1)?;
        let entry = occurrences
            .entry_definitions()
            .get(ordinal)
            .ok_or_else(execution_identity_error_v1)?;
        if entry.value() != row.value || entry.variable().get() != row.local.index() {
            return Err(execution_identity_error_v1());
        }
        let Some(call) = self.instances.incoming(row.instance) else {
            if row.instance != self.instances.root() {
                return Err(execution_identity_error_v1());
            }
            for _ in 0..row.selection.count {
                output.push(ExecutionIdentityEquationKindV1::Input, &[], budget)?;
            }
            return Ok(());
        };
        // The original parameter mapper retains adjusted RustCall tuple fields,
        // source argument roles and exact operand ownership.
        let parameter = self
            .instances
            .parameter_source(row.instance, row.local, budget)
            .map_err(|error| match error {
                production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => {
                    error.into()
                }
                _ => execution_identity_error_v1(),
            })?;
        let site = call.occurrence();
        let mut source = self.operand(
            site.caller,
            ExecutionSiteV29::Terminator {
                block: SsaBlockIdV1::new(site.block.index()),
            },
            ExecutionOperandV29::CallArgument(parameter.source_argument),
            parameter.operand,
            budget,
        )?;
        if let Some(field) = parameter.tuple_field {
            source = execution_identity_field_v1(
                self.instances.owner().source_semantic().types(),
                source,
                field as usize,
                budget,
            )?;
        }
        if source.ty != parameter.ty {
            return Err(execution_identity_error_v1());
        }
        self.alias_equations(source, row.selection, output, budget)
    }

    fn assignment_equations(
        &self,
        row: &ExecutionIdentitySourceRowV1,
        ordinal: usize,
        output: &mut ExecutionIdentityEquationsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let instance = self
            .instances
            .instance(row.instance)
            .ok_or_else(execution_identity_error_v1)?;
        let occurrences = self
            .instances
            .occurrences(row.instance)
            .ok_or_else(execution_identity_error_v1)?;
        let event = occurrences
            .events()
            .get(ordinal)
            .ok_or_else(execution_identity_error_v1)?;
        let site @ ExecutionSiteV29::Statement { block, statement } = event.site() else {
            return Err(execution_identity_error_v1());
        };
        let SemanticStatementKindV1::Assign(assignment) = instance
            .declaration()
            .blocks()
            .get(block.get() as usize)
            .and_then(|block| block.statements().get(statement as usize))
            .ok_or_else(execution_identity_error_v1)?
            .kind()
        else {
            return Err(execution_identity_error_v1());
        };
        if assignment.destination().local() != row.local
            || !assignment.destination().projections().is_empty()
            || assignment.destination().ty() != row.selection.ty
            || assignment.value().result_type() != row.selection.ty
            || event.operand() != ExecutionOperandV29::Destination
        {
            return Err(execution_identity_error_v1());
        }
        match assignment.value().kind() {
            SemanticRvalueKindV1::Use(operand) => {
                let source = self.operand(
                    row.instance,
                    site,
                    ExecutionOperandV29::RvalueOperand(0),
                    operand,
                    budget,
                )?;
                self.alias_equations(source, row.selection, output, budget)
            }
            SemanticRvalueKindV1::Aggregate(aggregate) => {
                let types = self.instances.owner().source_semantic().types();
                let Some((count, _, _)) = execution_cfg_fields_v29(types, row.selection.ty)? else {
                    return Err(execution_identity_error_v1());
                };
                if count != aggregate.operands().len() {
                    return Err(execution_identity_error_v1());
                }
                for (field, operand) in aggregate.operands().iter().enumerate() {
                    let destination =
                        execution_identity_field_v1(types, row.selection, field, budget)?;
                    if destination.count == 0 {
                        continue;
                    }
                    let source = self.operand(
                        row.instance,
                        site,
                        ExecutionOperandV29::RvalueOperand(
                            u32::try_from(field).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        ),
                        operand,
                        budget,
                    )?;
                    self.alias_equations(source, destination, output, budget)?;
                }
                Ok(())
            }
            SemanticRvalueKindV1::Borrow { place, .. } => {
                let types = self.instances.owner().source_semantic().types();
                if execution_cfg_nominal_kind_v29(types, row.selection.ty)? != Some(true)
                    || row.selection.count != 2
                {
                    return Err(execution_identity_error_v1());
                }
                let source = self.place(
                    row.instance,
                    site,
                    ExecutionOperandV29::RvaluePlace,
                    place,
                    budget,
                )?;
                let SemanticTypeShapeV1::Pointer(pointer) =
                    types[row.selection.ty.index() as usize].shape()
                else {
                    return Err(execution_identity_error_v1());
                };
                if source.count != 1 || source.ty != pointer.pointee() {
                    return Err(execution_identity_error_v1());
                }
                output.push(
                    ExecutionIdentityEquationKindV1::Producer,
                    &[source.first],
                    budget,
                )?;
                output.push(
                    ExecutionIdentityEquationKindV1::Merge,
                    &[source.first],
                    budget,
                )?;
                Ok(())
            }
            _ => Err(execution_identity_error_v1()),
        }
    }

    fn phi_equations(
        &self,
        row: &ExecutionIdentitySourceRowV1,
        block: SsaBlockIdV1,
        output: &mut ExecutionIdentityEquationsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let instance = self
            .instances
            .instance(row.instance)
            .ok_or_else(execution_identity_error_v1)?;
        let occurrences = self
            .instances
            .occurrences(row.instance)
            .ok_or_else(execution_identity_error_v1)?;
        let key = (row.instance.index(), block.get());
        charge_execution_cfg_lookup_v29(self.incoming.len(), budget)?;
        let incoming = self.incoming.get(&key).map_or(&[][..], Vec::as_slice);
        let virtual_entry = block.get() == instance.declaration().entry().index();
        let phi = matches!(row.kind, ExecutionIdentitySourceKindV1::Phi(_));
        let floor = budget.storage();
        budget.reserve_storage(std::mem::size_of::<Vec<usize>>())?;
        let mut dependencies = emission_vec_v1(
            argument_sum_v1(&[
                argument_product_v1(incoming.len(), if phi { 2 } else { 1 })?,
                usize::from(virtual_entry),
            ])?,
            budget,
        )?;
        for component in 0..row.selection.count {
            budget.charge_work(dependencies.len())?;
            dependencies.clear();
            if virtual_entry {
                let arguments = if phi {
                    instance.ssa().plan().entry_arguments()
                } else {
                    instance.ssa().plan().entry_definitions()
                };
                budget.charge_work(arguments.len())?;
                let argument = arguments
                    .iter()
                    .find(|argument| argument.variable().get() == row.local.index())
                    .ok_or_else(execution_identity_error_v1)?;
                let source = self.value(row.instance, argument.value(), row.local, budget)?;
                if source.ty != row.selection.ty || source.count != row.selection.count {
                    return Err(execution_identity_error_v1());
                }
                dependencies.push(argument_sum_v1(&[source.first, component])?);
            }
            for &index in incoming {
                budget.charge_work(2)?;
                let edge = occurrences
                    .successors()
                    .get(index)
                    .ok_or_else(execution_identity_error_v1)?;
                if edge.edge().target().index() != block.get() {
                    return Err(execution_identity_error_v1());
                }
                if phi {
                    let arguments = instance
                        .ssa()
                        .plan()
                        .edge_arguments(edge.id())
                        .ok_or_else(execution_identity_error_v1)?;
                    budget.charge_work(arguments.len())?;
                    let argument = arguments
                        .iter()
                        .find(|argument| argument.variable().get() == row.local.index())
                        .ok_or_else(execution_identity_error_v1)?;
                    let source = self.value(row.instance, argument.value(), row.local, budget)?;
                    if source.ty != row.selection.ty || source.count != row.selection.count {
                        return Err(execution_identity_error_v1());
                    }
                    dependencies.push(argument_sum_v1(&[source.first, component])?);
                }
                let source = self.edge_input(row.instance, edge.id(), row.local, budget)?;
                if source.ty != row.selection.ty || source.count != row.selection.count {
                    return Err(execution_identity_error_v1());
                }
                dependencies.push(argument_sum_v1(&[source.first, component])?);
            }
            output.push(
                ExecutionIdentityEquationKindV1::Merge,
                &dependencies,
                budget,
            )?;
        }
        // Only the temporary predecessor vector is released; new equations
        // and their input rows remain charged in the caller's construction.
        let temporary = argument_sum_v1(&[
            std::mem::size_of::<Vec<usize>>(),
            argument_product_v1(dependencies.capacity(), std::mem::size_of::<usize>())?,
        ])?;
        drop(dependencies);
        if budget.storage() < argument_sum_v1(&[floor, temporary])? {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.release_storage(temporary)?;
        Ok(())
    }
}
