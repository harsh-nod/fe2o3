/// Versioned operation coverage for a one-time invocation-to-source-entry edge.
/// This row is coordinates, not independent source or optimization authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionInvocationEntrySpanV1 {
    version: u16,
    correspondence_owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    kernel_ir_block: BlockId,
    first_operation_ordinal: u32,
    operation_count: u32,
}

impl ProductionInvocationEntrySpanV1 {
    /// Relation schema version.
    pub const fn version(self) -> u16 {
        self.version
    }

    /// Original source function, before instance expansion.
    pub const fn semantic_function(self) -> SemanticFunctionIdV1 {
        self.semantic_function
    }

    /// Original synthetic invocation block, before instance expansion.
    pub const fn kernel_ir_block(self) -> BlockId {
        self.kernel_ir_block
    }

    /// First invocation transport operation, after one-time storage setup.
    pub const fn first_operation_ordinal(self) -> u32 {
        self.first_operation_ordinal
    }

    /// Number of transport operations; zero still requires an exact branch.
    pub const fn operation_count(self) -> u32 {
        self.operation_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InvocationArgumentRowV1 {
    original: SsaArgumentV1,
    first_component: usize,
    component_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InvocationComponentRowV1 {
    original: ValueId,
    parameter: ValueId,
    transported: ValueId,
    conversion: Option<u32>,
}

struct InvocationEntryRelationV1 {
    subject: Option<ScopedInitializationSubjectV29>,
    layout: InvocationBlockLayoutV1,
    span: ProductionInvocationEntrySpanV1,
    arguments: Vec<InvocationArgumentRowV1>,
    components: Vec<InvocationComponentRowV1>,
    inputs: Vec<InvocationInputRowV1>,
}

include!("production_invocation_entry_inputs_v1.rs");

struct InvocationTransportCollectorV1<'a> {
    values: Vec<ValueDef>,
    nodes: usize,
    budget: &'a mut dyn SemanticEmissionBudgetV1,
}

fn invocation_equal_types_v1(
    left: &Type,
    right: &Type,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    fn prepay(
        ty: &Type,
        nodes: &mut usize,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        execution_cfg_charge_node_v29(nodes, budget)?;
        match ty {
            Type::Pointer(pointer) => prepay(&pointer.pointee, nodes, budget),
            Type::Slice(slice) => prepay(&slice.element, nodes, budget),
            _ => Ok(()),
        }
    }
    prepay(left, &mut 0, budget)?;
    prepay(right, &mut 0, budget)?;
    Ok(left == right)
}

impl SemanticTransportVisitorV1 for InvocationTransportCollectorV1<'_> {
    type Error = ProductionSemanticKirErrorV1;

    fn node(&mut self) -> Result<(), Self::Error> {
        execution_cfg_charge_node_v29(&mut self.nodes, self.budget)
    }

    fn component(
        &mut self,
        value: ValueId,
        ty: BorrowedTransportTypeV1<'_>,
    ) -> Result<(), Self::Error> {
        self.node()?;
        let ty = match ty {
            BorrowedTransportTypeV1::Existing(ty) => execution_cfg_clone_type_v29(ty, self.budget)?,
            BorrowedTransportTypeV1::Pointer {
                pointee,
                address_space,
                access,
            } => {
                self.budget.charge_work(1)?;
                self.budget.reserve_storage(std::mem::size_of::<Type>())?;
                Type::pointer(
                    execution_cfg_clone_type_v29(pointee, self.budget)?,
                    address_space,
                    access,
                )
            }
        };
        emission_push_v1(&mut self.values, ValueDef::new(value, ty), self.budget)
    }

    fn equal_types(&mut self, left: &Type, right: &Type) -> Result<bool, Self::Error> {
        invocation_equal_types_v1(left, right, self.budget)
    }

    fn invalid(detail: &'static str) -> Self::Error {
        unsupported(0, None, None, detail)
    }
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn initialize_invocation_v1(
        &mut self,
        target: &mut BasicBlock,
    ) -> Result<SemanticBlockPrologueSpansV1, ProductionSemanticKirErrorV1> {
        let block = self.function.entry();
        for local in &self.control_flow_ssa.implicit_entry_locals {
            let declaration = self
                .function
                .locals()
                .get(*local as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let binding = self
                .control_flow_ssa
                .compiler_issued_bindings
                .get(&declaration.ty())
                .copied()
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            self.locals[*local as usize] =
                Some(binding.binding_from_transport(self.types, declaration.ty(), &[])?);
        }
        for (local, value) in &self.control_flow_ssa.entry_definitions {
            let binding = self
                .locals
                .get(*local as usize)
                .and_then(Option::as_ref)
                .ok_or(ProductionSemanticKirErrorV1::MissingLocalDefinition {
                    function: self.semantic_function.index(),
                    block: block.index(),
                    statement: None,
                    local: *local,
                })?;
            if let Some(cursor) = self.execution.as_ref() {
                let budget = self
                    .emission_work
                    .as_deref_mut()
                    .ok_or(ArgumentResourceV1::Accounting)?;
                archive_scoped_binding_v29(
                    cursor,
                    &mut self.semantic_ssa_bindings,
                    &mut self.semantic_ssa_archive_credit,
                    *value,
                    binding,
                    ExecutionArchiveDefinitionSiteV29::Invocation { local: *local },
                    budget,
                )?;
            } else {
                insert_semantic_ssa_binding_v1(
                    &mut self.semantic_ssa_bindings,
                    self.semantic_function.index(),
                    Some(block.index()),
                    None,
                    *value,
                    binding.clone(),
                )?;
            }
        }
        let retained_first = target.operations.len();
        self.emit_retained_local_allocas_v1(target)?;
        let (_, retained_local_storage) =
            measured_operation_span(retained_first, target.operations.len(), target.id, None)?;
        let enum_first = target.operations.len();
        self.emit_enum_payload_allocas_v1(target)?;
        let (_, enum_payload_storage) =
            measured_operation_span(enum_first, target.operations.len(), target.id, None)?;
        Ok(SemanticBlockPrologueSpansV1 {
            retained_local_storage,
            enum_payload_storage,
        })
    }

    fn begin_invocation_v1(
        &mut self,
        plan: &InvocationEntryPlanV1<'_>,
        target: &mut BasicBlock,
    ) -> Result<SemanticBlockPrologueSpansV1, ProductionSemanticKirErrorV1> {
        self.with_emission_budget_v1(|this, budget| {
            plan.check_source(this.function, plan.ssa, budget)
        })?;
        if plan.layout.preheader != Some(target.id)
            || self.invocation_preheader != Some(target.id)
            || !target.parameters.is_empty()
            || !target.operations.is_empty()
            || target.terminator.is_some()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        if let Some(recorder) = &mut self.scoped_memory {
            recorder.block = Some(target.id);
            recorder.frame = None;
        }
        self.private_arrays.begin_invocation_v1(plan, target.id)?;
        self.initialize_invocation_v1(target)
    }

    fn emit_invocation_edge_v1(
        &mut self,
        plan: &InvocationEntryPlanV1<'_>,
        subject: Option<ScopedInitializationSubjectV29>,
        target: &mut BasicBlock,
    ) -> Result<InvocationEntryRelationV1, ProductionSemanticKirErrorV1> {
        if plan.layout.preheader != Some(target.id)
            || self.invocation_preheader != Some(target.id)
            || target.terminator.is_some()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let (mut arguments, mut components, mut branch) =
            self.with_emission_budget_v1(|this, budget| {
                plan.check_source(this.function, plan.ssa, budget)?;
                budget.reserve_storage(std::mem::size_of::<InvocationEntryRelationV1>())?;
                let mut count = 0;
                for argument in plan.entry_arguments() {
                    charge_execution_cfg_lookup_v29(this.control_flow_ssa.promoted.len(), budget)?;
                    let promoted = this
                        .control_flow_ssa
                        .promoted
                        .get(&argument.variable().get())
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    count = argument_sum_v1(&[count, promoted.kernel_types.len()])?;
                }
                Ok((
                    emission_vec_v1(plan.entry_arguments().len(), budget)?,
                    emission_vec_v1(count, budget)?,
                    emission_vec_v1(count, budget)?,
                ))
            })?;
        if self.execution.is_some() {
            self.with_emission_budget_v1(|this, budget| {
                this.execution.as_mut().unwrap().transport_invocation_v1(
                    plan,
                    &this.locals,
                    &this.semantic_ssa_bindings,
                    &this.control_flow_ssa.cfg_carriers,
                    budget,
                )
            })?;
        }
        let first = target.operations.len();
        for original in plan.entry_arguments() {
            let local = original.variable().get();
            let (values, scratch) = self.with_emission_budget_v1(|this, budget| {
                charge_execution_cfg_lookup_v29(this.semantic_ssa_bindings.len(), budget)?;
                charge_execution_cfg_lookup_v29(this.control_flow_ssa.promoted.len(), budget)?;
                let binding = this
                    .semantic_ssa_bindings
                    .get(&original.value())
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                let promoted = this
                    .control_flow_ssa
                    .promoted
                    .get(&local)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                let floor = budget.storage();
                budget.reserve_storage(std::mem::size_of::<InvocationTransportCollectorV1<'_>>())?;
                let mut collector = InvocationTransportCollectorV1 {
                    values: Vec::new(),
                    nodes: 0,
                    budget,
                };
                if promoted.transport == SemanticPromotedTransportV1::Execution {
                    if let Some(cursor) = this.execution.as_ref() {
                        let output = &mut collector.values;
                        with_execution_cfg_local_values_v29(cursor, &this.control_flow_ssa.cfg_carriers,
                            this.function.entry(), local, binding, collector.budget, |values, budget| {
                                for value in values {
                                    let ty = execution_cfg_clone_type_v29(&value.ty, budget)?;
                                    emission_push_v1(output, ValueDef::new(value.id, ty), budget)?;
                                }
                                Ok(())
                            })?;
                    } else {
                        execution_cfg_values_v29(
                            binding,
                            &mut collector.values,
                            &mut collector.nodes,
                            collector.budget,
                        )?;
                    }
                } else {
                    promoted.transport.visit_transport_values(
                        binding,
                        &promoted.kernel_types,
                        &mut collector,
                    )?;
                }
                if collector.values.len() != promoted.kernel_types.len() {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let scratch = collector
                    .budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                Ok((collector.values, scratch))
            })?;
            let first_component = components.len();
            for (component, value) in values.into_iter().enumerate() {
                let (parameter, expected, expected_storage) =
                    self.with_emission_budget_v1(|this, budget| {
                        charge_execution_cfg_lookup_v29(this.block_parameters.len(), budget)?;
                        charge_execution_cfg_lookup_v29(
                            this.control_flow_ssa.promoted.len(),
                            budget,
                        )?;
                        let parameter = this
                            .block_parameters
                            .get(&this.function.entry().index())
                            .and_then(|locals| locals.get(&local))
                            .and_then(|values| values.get(component))
                            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                        let selected = if this.execution.as_ref().is_some_and(|cursor|
                            cursor.cfg.reference_locals.get(local as usize) == Some(&true))
                        {
                            budget.reserve_storage(std::mem::size_of::<Option<Vec<Type>>>())?;
                            Some(source_descriptor_cfg_types_v29(
                                this.execution.as_ref().ok_or_else(invocation_entry_error_v1)?,
                                &this.control_flow_ssa.cfg_carriers,
                                this.function.entry(), local, budget)?)
                        } else { None };
                        let expected = selected.as_deref()
                            .unwrap_or(&this.control_flow_ssa.promoted[&local].kernel_types)
                            .get(component).ok_or_else(invocation_entry_error_v1)?;
                        if !invocation_equal_types_v1(&parameter.ty, expected, budget)? {
                            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                        }
                        let floor = budget.storage();
                        let expected = execution_cfg_clone_type_v29(expected, budget)?;
                        Ok((
                            parameter.id,
                            expected,
                            budget
                                .storage()
                                .checked_sub(floor)
                                .ok_or(ArgumentResourceV1::Accounting)?,
                        ))
                    })?;
                let equal = self.with_emission_budget_v1(|_, budget| {
                    invocation_equal_types_v1(&value.ty, &expected, budget)
                })?;
                let conversion = (!equal)
                    .then(|| {
                        u32::try_from(target.operations.len())
                            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                    })
                    .transpose()?;
                let transported = self.coerce_transport_value_v1(
                    &mut target.operations,
                    self.function.entry(),
                    None,
                    value.id,
                    value.ty,
                    expected,
                    Some(SourceDescriptorDestinationV29::Invocation { local, definition: original.value(), component }),
                    "invocation transport differs from the original SSA entry argument",
                )?;
                self.with_emission_budget_v1(|_, budget| {
                    budget.charge_work(3)?;
                    budget.release_storage(expected_storage)
                })?;
                components.push(InvocationComponentRowV1 {
                    original: value.id,
                    parameter,
                    transported,
                    conversion,
                });
                branch.push(transported);
            }
            self.with_emission_budget_v1(|_, budget| {
                budget.charge_work(1)?;
                budget.release_storage(scratch)
            })?;
            arguments.push(InvocationArgumentRowV1 {
                original: *original,
                first_component,
                component_count: components.len() - first_component,
            });
        }
        let (first_operation_ordinal, operation_count) =
            measured_operation_span(first, target.operations.len(), target.id, None)?;
        let inputs = self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_mut()
                .ok_or_else(invocation_entry_error_v1)?;
            cursor.check_ledger(budget)?;
            cursor.check_source(this.function, plan.ssa)?;
            match cursor.invocation_inputs.take() {
                Some(inputs) => Ok(inputs),
                None if plan.entry_arguments().is_empty() => Ok(Vec::new()),
                None => Err(invocation_entry_error_v1()),
            }
        })?;
        target.terminator = Some(Terminator::Branch {
            target: plan.layout.source_entry,
            arguments: branch,
        });
        Ok(InvocationEntryRelationV1 {
            subject,
            layout: plan.layout,
            span: ProductionInvocationEntrySpanV1 {
                version: INVOCATION_ENTRY_RELATION_VERSION_V1,
                correspondence_owner: self.correspondence_owner,
                semantic_function: self.semantic_function,
                kernel_ir_block: target.id,
                first_operation_ordinal,
                operation_count,
            },
            arguments,
            components,
            inputs,
        })
    }
}
