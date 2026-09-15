mod reusable_lds_01 {
    use super::capability_ssa_graph_01::CapabilitySsaGraphV1;
    use super::*;
    use fe2o3_kernel_ir::{ReusableLdsConversionV1, required_execution_obligations_v1};
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAssignmentV1, SemanticDefinedCapabilityContractV1,
    };
    use fe2o3_pliron::ProductionSemanticReusableLdsResultV1;

    type Site = (SemanticBlockIdV1, u32);
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum State {
        Pending,
        ParameterBound,
        Consumed,
    }
    #[derive(Clone, Debug)]
    struct Transfer {
        row: ProductionSemanticReusableLdsResultV1,
        input_assignment: SemanticAssignmentV1,
        output_assignment: SemanticAssignmentV1,
        allocation: SsaValueV1,
        parameter: SsaValueV1,
        returned: SsaValueV1,
        destination: SsaValueV1,
        source: ExecutionCapabilitySourceV1,
        state: State,
    }
    #[derive(Default)]
    pub(super) struct ReusableLdsPlanV1 {
        transfers: BTreeMap<Site, Transfer>,
        parameters: BTreeMap<Site, Site>,
    }
    fn mismatch() -> ProductionSemanticKirErrorV1 {
        ProductionSemanticKirErrorV1::CorrespondenceMismatch
    }
    fn rejected(message: &'static str) -> ProductionSemanticKirErrorV1 {
        unsupported(0, None, None, message)
    }
    fn same_capability_binding(
        left: Option<&SemanticValueBindingV1>,
        right: Option<&SemanticValueBindingV1>,
    ) -> bool {
        matches!((left, right), (
            Some(SemanticValueBindingV1::Value { id: left, ty: Type::ExecutionCapability(left_ty) }),
            Some(SemanticValueBindingV1::Value { id: right, ty: Type::ExecutionCapability(right_ty) })
        ) if left == right && left_ty == right_ty)
    }
    fn assignment(
        body: &SemanticFunctionDeclV1,
        site: Site,
    ) -> Result<&SemanticAssignmentV1, ProductionSemanticKirErrorV1> {
        match body
            .blocks()
            .get(site.0.index() as usize)
            .and_then(|b| b.statements().get(site.1 as usize))
            .map(|s| s.kind())
        {
            Some(SemanticStatementKindV1::Assign(a)) => Ok(a),
            _ => Err(mismatch()),
        }
    }
    fn frame_kills(
        row: ProductionSemanticReusableLdsResultV1,
        events: &[(u32, SsaResolvedEventV1)],
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut locals = [row.parameter().index(), row.return_local().index()];
        locals.sort_unstable();
        if events.len() != 2 || locals[0].checked_add(1) != Some(locals[1]) {
            return Err(mismatch());
        }
        for ((_, event), local) in events.iter().zip(locals) {
            if !matches!(event, SsaResolvedEventV1::Kill { variable, previous: None } if variable.get() == local) {
                return Err(mismatch());
            }
        }
        Ok(())
    }
    fn input_events(
        row: ProductionSemanticReusableLdsResultV1,
        events: &[(u32, SsaResolvedEventV1)],
    ) -> Result<(SsaValueV1, SsaValueV1), ProductionSemanticKirErrorV1> {
        if events.len() != 5 {
            return Err(mismatch());
        }
        frame_kills(row, &events[..2])?;
        let events = &events[2..];
        let [
            (_, SsaResolvedEventV1::Use { variable, value }),
            (
                _,
                SsaResolvedEventV1::Kill {
                    variable: killed,
                    previous: Some(previous),
                },
            ),
            (
                _,
                SsaResolvedEventV1::Define {
                    variable: output,
                    value: parameter,
                },
            ),
        ] = events
        else {
            return Err(mismatch());
        };
        if variable.get() != row.allocation().index()
            || killed != variable
            || previous != value
            || output.get() != row.parameter().index()
            || parameter == value
        {
            return Err(mismatch());
        }
        Ok((*value, *parameter))
    }
    fn output_events(
        row: ProductionSemanticReusableLdsResultV1,
        parameter: SsaValueV1,
        events: &[(u32, SsaResolvedEventV1)],
    ) -> Result<(SsaValueV1, SsaValueV1), ProductionSemanticKirErrorV1> {
        if events.len() != 8 {
            return Err(mismatch());
        }
        frame_kills(row, &events[6..])?;
        let events = &events[..6];
        let [
            (
                _,
                SsaResolvedEventV1::Use {
                    variable: input,
                    value,
                },
            ),
            (
                _,
                SsaResolvedEventV1::Kill {
                    variable: input_killed,
                    previous: Some(input_previous),
                },
            ),
            (
                _,
                SsaResolvedEventV1::Define {
                    variable: returned,
                    value: returned_value,
                },
            ),
            (
                _,
                SsaResolvedEventV1::Use {
                    variable: used,
                    value: used_value,
                },
            ),
            (
                _,
                SsaResolvedEventV1::Kill {
                    variable: killed,
                    previous: Some(previous),
                },
            ),
            (
                _,
                SsaResolvedEventV1::Define {
                    variable: destination,
                    value: destination_value,
                },
            ),
        ] = events
        else {
            return Err(mismatch());
        };
        if input.get() != row.parameter().index()
            || input != input_killed
            || *value != parameter
            || value != input_previous
            || returned.get() != row.return_local().index()
            || returned != used
            || used != killed
            || returned_value != used_value
            || used_value != previous
            || destination.get() != row.destination().index()
            || returned_value == destination_value
            || *returned_value == parameter
            || *destination_value == parameter
        {
            return Err(mismatch());
        }
        Ok((*returned_value, *destination_value))
    }

    impl ReusableLdsPlanV1 {
        pub(super) fn new(
            owner: &ProductionSemanticSsaOwnerV1,
            function: &SemanticFunctionDeclV1,
            context: &RootKernelContextLoweringV1,
            max_work: usize,
        ) -> Result<Self, ProductionSemanticKirErrorV1> {
            owner
                .verify_replay()
                .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
            let view = owner
                .execution_view_for_root(context.selected_root)
                .ok_or_else(mismatch)?;
            let plan = owner
                .execution_plan_for_root(context.selected_root)
                .ok_or_else(mismatch)?;
            if view.body() != function {
                return Err(mismatch());
            }
            let bindings = owner
                .execution_expansion()
                .defined_capability_bindings(owner.source_semantic())
                .map_err(|_| mismatch())?;
            let mut graph = CapabilitySsaGraphV1::new(function, plan.plan(), max_work)?;
            graph.charge(bindings.len().checked_mul(64).ok_or_else(mismatch)?)?;
            let mut output = Self::default();
            let mut allocated = BTreeSet::new();
            for row in plan.defined_reusable_lds_results() {
                graph.charge(bindings.len())?;
                let mut found = bindings.iter().filter(|b| {
                    b.root() == context.selected_root && b.callee_instance() == row.callee()
                });
                let binding = found.next().ok_or_else(mismatch)?;
                if found.next().is_some()
                    || binding.contract()
                        != SemanticDefinedCapabilityContractV1::ReusableLdsConversion(row.record())
                    || binding.caller_instance() != row.caller()
                    || binding.callee_arguments() != [row.parameter()]
                    || binding.callee_return() != row.return_local()
                    || !global_capability_provenance_matches_v1(context, row.record().provenance())
                {
                    return Err(mismatch());
                }
                let input_site = (row.parameter_block(), row.parameter_statement());
                let output_site = (row.return_block(), row.return_statement());
                if row.parameter_statement() != 2
                    || row.return_statement() != 0
                    || function.blocks()[input_site.0.index() as usize]
                        .statements()
                        .len()
                        != 3
                    || function.blocks()[output_site.0.index() as usize]
                        .statements()
                        .len()
                        != 3
                {
                    return Err(rejected(
                        "reusable LDS requires its exact acyclic empty conversion",
                    ));
                }
                let input = plan
                    .plan()
                    .resolved_events(SsaBlockIdV1::new(input_site.0.index()))
                    .ok_or_else(mismatch)?;
                let result = plan
                    .plan()
                    .resolved_events(SsaBlockIdV1::new(output_site.0.index()))
                    .ok_or_else(mismatch)?;
                graph.charge(input.len() + result.len())?;
                let (allocation, parameter) = input_events(*row, input)?;
                let (returned, destination) = output_events(*row, parameter, result)?;
                let issued = graph.definition(allocation)?;
                if issued.block != row.allocation_block().index()
                    || issued.statement.is_some()
                    || issued.local != row.allocation().index()
                    || !allocated.insert(allocation)
                    || graph.use_value(input_site.0.index(), row.allocation().index())?
                        != allocation
                    || graph.use_value(output_site.0.index(), row.parameter().index())? != parameter
                {
                    return Err(rejected(
                        "reusable LDS allocation does not dominate its single consuming SSA transfer",
                    ));
                }
                // No logical alias/phi replaces the exact producer during the
                // conversion. Later users must transport the new handle role.
                for block in plan.plan().reverse_postorder() {
                    let variables = plan
                        .plan()
                        .transport_variables(*block)
                        .ok_or_else(mismatch)?;
                    graph.charge(variables.len())?;
                    if variables.iter().any(|v| {
                        [
                            row.allocation().index(),
                            row.parameter().index(),
                            row.return_local().index(),
                        ]
                        .contains(&v.get())
                    }) {
                        return Err(rejected(
                            "reusable LDS conversion cannot merge allocation or parameter identities",
                        ));
                    }
                }
                let source = CheckedExecutionSourceCarrierV1::source_for_defined_statement(
                    owner,
                    context.selected_root,
                    function,
                    *owner.execution_expansion().identity(),
                    *view.identity(),
                    binding,
                    output_site.0,
                    output_site.1,
                )?;
                graph.charge(256)?;
                if output.parameters.insert(input_site, output_site).is_some()
                    || output
                        .transfers
                        .insert(
                            output_site,
                            Transfer {
                                row: *row,
                                input_assignment: assignment(function, input_site)?.clone(),
                                output_assignment: assignment(function, output_site)?.clone(),
                                allocation,
                                parameter,
                                returned,
                                destination,
                                source,
                                state: State::Pending,
                            },
                        )
                        .is_some()
                {
                    return Err(mismatch());
                }
            }
            graph.charge(bindings.len())?;
            if output.transfers.len()
                != bindings
                    .iter()
                    .filter(|b| {
                        b.root() == context.selected_root
                            && matches!(
                                b.contract(),
                                SemanticDefinedCapabilityContractV1::ReusableLdsConversion(_)
                            )
                    })
                    .count()
            {
                return Err(mismatch());
            }
            Ok(output)
        }
    }

    impl SemanticFunctionLoweringV1<'_> {
        fn reusable_live(
            &self,
            local: SemanticLocalIdV1,
            value: SsaValueV1,
        ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
            let actual = self
                .locals
                .get(local.index() as usize)
                .and_then(Option::as_ref)
                .ok_or_else(mismatch)?;
            if !same_capability_binding(self.semantic_ssa_bindings.get(&value), Some(actual)) {
                return Err(mismatch());
            }
            Ok(actual.clone())
        }
        fn reusable_kill(&mut self, local: SemanticLocalIdV1) {
            self.locals[local.index() as usize] = None;
            self.retained_local_initialized.remove(&local.index());
        }
        pub(super) fn lower_reusable_lds(
            &mut self,
            block: SemanticBlockIdV1,
            statement: Option<u32>,
            kind: &SemanticStatementKindV1,
            operations: &mut Vec<Operation>,
        ) -> Result<bool, ProductionSemanticKirErrorV1> {
            let Some(statement) = statement else {
                return Ok(false);
            };
            let site = (block, statement);
            let Some(plan) = &self.reusable_lds else {
                return Ok(false);
            };
            if let Some(return_site) = plan.parameters.get(&site).copied() {
                let transfer = plan
                    .transfers
                    .get(&return_site)
                    .ok_or_else(mismatch)?
                    .clone();
                if transfer.state != State::Pending
                    || !matches!(kind, SemanticStatementKindV1::Assign(a) if *a == transfer.input_assignment)
                {
                    return Err(mismatch());
                }
                let binding = self.reusable_live(transfer.row.allocation(), transfer.allocation)?;
                let SemanticValueBindingV1::Value {
                    ty: Type::ExecutionCapability(ref input),
                    ..
                } = binding
                else {
                    return Err(mismatch());
                };
                let record = transfer.row.record();
                let conversion = self.reusable_operation(record)?;
                if conversion.output_type(input).is_none()
                    || input.workgroup_brand != Some(*record.brand().as_bytes())
                    || input.epoch != Some(*record.epoch().as_bytes())
                {
                    return Err(mismatch());
                }
                self.reusable_kill(transfer.row.allocation());
                self.bind_destination(
                    block,
                    Some(statement),
                    transfer.input_assignment.destination(),
                    binding,
                )?;
                if !same_capability_binding(
                    self.semantic_ssa_bindings.get(&transfer.parameter),
                    self.locals[transfer.row.parameter().index() as usize].as_ref(),
                ) {
                    return Err(mismatch());
                }
                self.reusable_lds
                    .as_mut()
                    .unwrap()
                    .transfers
                    .get_mut(&return_site)
                    .unwrap()
                    .state = State::ParameterBound;
                return Ok(true);
            }
            let Some(transfer) = plan.transfers.get(&site).cloned() else {
                return Ok(false);
            };
            if transfer.state != State::ParameterBound
                || !matches!(kind, SemanticStatementKindV1::Assign(a) if *a == transfer.output_assignment)
            {
                return Err(mismatch());
            }
            let binding = self.reusable_live(transfer.row.parameter(), transfer.parameter)?;
            let SemanticValueBindingV1::Value {
                id: allocation,
                ty: Type::ExecutionCapability(input),
            } = binding
            else {
                return Err(mismatch());
            };
            if !same_capability_binding(
                self.semantic_ssa_bindings.get(&transfer.allocation),
                Some(&SemanticValueBindingV1::Value {
                    id: allocation,
                    ty: Type::ExecutionCapability(input.clone()),
                }),
            ) {
                return Err(mismatch());
            }
            let conversion = self.reusable_operation(transfer.row.record())?;
            let output = conversion.output_type(&input).ok_or_else(mismatch)?;
            let operation = ExecutionCapabilityOperationV1::ReusableLdsConversion(conversion);
            let contract = ExecutionCapabilityOpV1 {
                operands: vec![allocation],
                signature: ExecutionCapabilitySignatureV1::new(
                    &[conversion.input],
                    conversion.output,
                )
                .ok_or_else(mismatch)?,
                obligations: ExecutionSafetyObligationsV1::from_bits(
                    required_execution_obligations_v1(&operation),
                ),
                operation,
                provenance: input.provenance,
                workgroup_brand: input.workgroup_brand,
                epoch_before: input.epoch,
                epoch_after: None,
                source: transfer.source,
            };
            if !contract.is_complete() {
                return Err(mismatch());
            }
            let ty = Type::ExecutionCapability(output);
            let value = self.emit_id(
                operations,
                ty.clone(),
                OperationKind::ExecutionCapability(contract),
            )?;
            let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(return_place)) =
                transfer.output_assignment.value().kind()
            else {
                return Err(mismatch());
            };
            self.reusable_kill(transfer.row.parameter());
            let result = SemanticValueBindingV1::Value { id: value, ty };
            self.bind_destination(block, Some(statement), return_place, result.clone())?;
            if !same_capability_binding(
                self.semantic_ssa_bindings.get(&transfer.returned),
                Some(&result),
            ) {
                return Err(mismatch());
            }
            self.bind_destination(
                block,
                Some(statement),
                transfer.output_assignment.destination(),
                result.clone(),
            )?;
            if !same_capability_binding(
                self.semantic_ssa_bindings.get(&transfer.destination),
                Some(&result),
            ) {
                return Err(mismatch());
            }
            self.reusable_kill(transfer.row.return_local());
            self.reusable_lds
                .as_mut()
                .unwrap()
                .transfers
                .get_mut(&site)
                .unwrap()
                .state = State::Consumed;
            Ok(true)
        }
        fn reusable_operation(
            &self,
            record: fe2o3_mir_model::semantic_mir_v1::SemanticReusableLdsConversionV1,
        ) -> Result<ReusableLdsConversionV1, ProductionSemanticKirErrorV1> {
            let layout = execution_element_layout_v1(self.types, record.types().element)?;
            if u64::from(layout.byte_size) != record.element_size()
                || u64::from(layout.byte_alignment) != record.element_align()
                || self.types[record.types().element.index() as usize].layout_identity()
                    != record.element_layout()
            {
                return Err(mismatch());
            }
            Ok(ReusableLdsConversionV1 {
                input: execution_type_identity_v1(self.types, record.types().input)?,
                output: execution_type_identity_v1(self.types, record.types().output)?,
                element: execution_type_identity_v1(self.types, record.types().element)?,
                layout,
                elements: record.elements(),
                defined_function: *record.source_identity().as_bytes(),
                defined_abi: *record.abi_identity().as_bytes(),
                defined_body: *record.body_identity(),
                source_binding: record.source().source_binding,
            })
        }
        pub(super) fn require_reusable_lds_consumed(
            &self,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            if self
                .reusable_lds
                .as_ref()
                .is_some_and(|p| p.transfers.values().any(|t| t.state != State::Consumed))
            {
                return Err(rejected(
                    "reusable LDS source conversion receipt was not consumed",
                ));
            }
            Ok(())
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        include!("reusable_lds_01/tests.rs");
    }
}
use reusable_lds_01::ReusableLdsPlanV1;
