use fe2o3_pliron::ProductionSemanticMathBridgeResultV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MathBridgeStateV1 {
    Pending,
    Transferred,
    Consumed,
}

/// No KIR value exists until the original getter return consumes this receipt.
#[derive(Clone, Debug)]
struct MathBridgeV1 {
    result: ProductionSemanticMathBridgeResultV1,
    assignment: SemanticAssignmentV1,
    receiver: SsaValueV1,
    current: SsaValueV1,
    returned: SsaValueV1,
    destination: SsaValueV1,
    context: SsaValueV1,
    state: MathBridgeStateV1,
}

fn math_bridge_plans_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    context: &RootKernelContextLoweringV1,
    custody: &MathCustodyPlanV1,
    producers: &BTreeMap<MathStatementSiteV1, MathProducerV1>,
    max_work: usize,
) -> Result<BTreeMap<MathStatementSiteV1, MathBridgeV1>, ProductionSemanticKirErrorV1> {
    use capability_ssa_graph_01::CapabilitySsaGraphV1;
    let view = owner
        .execution_view_for_root(context.selected_root)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let plan = owner
        .execution_plan_for_root(context.selected_root)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let mut graph = CapabilitySsaGraphV1::new(view.body(), plan.plan(), max_work)?;
    let mut bridges = BTreeMap::new();
    for result in plan.defined_math_results() {
        graph.charge(1)?;
        let getter_site = MathStatementSiteV1 {
            block: result.getter_block(),
            statement: result.getter_statement(),
        };
        let Some(producer) = producers.get(&getter_site) else {
            return Err(unsupported(
                0,
                Some(result.block().index()),
                Some(result.statement()),
                "defined Math result has no checked consumer custody",
            ));
        };
        let getter = custody
            .occurrences
            .getters
            .get(&getter_site)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let site = MathStatementSiteV1 {
            block: result.block(),
            statement: result.statement(),
        };
        if getter.binding.callee_instance() != result.getter()
            || getter.bridge_instance != result.bridge()
            || getter.bridge_return_site != site
            || getter.record.types().math != result.math()
            || getter.binding.callee_return() != result.destination()
            || getter.binding.callee_arguments() != [result.receiver()]
            || !matches!(
                producer.operation,
                NumericalPolicyMathOperationV1::MathDerive { .. }
            )
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let [context] = producer.dependencies.as_slice() else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let events = plan
            .plan()
            .resolved_events(SsaBlockIdV1::new(result.block().index()))
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        graph.charge(events.len())?;
        let (receiver, current, returned, destination) = math_bridge_events_v1(*result, events)?;
        let current_site = graph.definition(current)?;
        if current_site.block != getter.current_block.index()
            || current_site.statement.is_some()
            || current_site.local != result.current().index()
            || graph.use_value(result.getter_block().index(), result.destination().index())?
                != destination
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        // These two fixed return locals have no join semantics. A receipt may
        // not be substituted for an arbitrary block argument or transported value.
        for block in plan.plan().reverse_postorder() {
            let variables = plan
                .plan()
                .transport_variables(*block)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            graph.charge(variables.len())?;
            if variables.iter().any(|v| {
                [result.return_local().index(), result.destination().index()].contains(&v.get())
            }) {
                return Err(unsupported(
                    0,
                    Some(block.get()),
                    None,
                    "Math bridge receipt cannot cross an SSA join",
                ));
            }
        }
        let SemanticStatementKindV1::Assign(assignment) =
            view.body().blocks()[site.block.index() as usize].statements()[site.statement as usize]
                .kind()
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        if bridges
            .insert(
                site,
                MathBridgeV1 {
                    result: *result,
                    assignment: assignment.clone(),
                    receiver,
                    current,
                    returned,
                    destination,
                    context: *context,
                    state: MathBridgeStateV1::Pending,
                },
            )
            .is_some()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
    }
    if bridges.len()
        != producers
            .values()
            .filter(|p| {
                matches!(
                    p.operation,
                    NumericalPolicyMathOperationV1::MathDerive { .. }
                )
            })
            .count()
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(bridges)
}

include!("bridge_events.rs");

impl SemanticFunctionLoweringV1<'_> {
    fn lower_math_bridge(
        &mut self,
        site: MathStatementSiteV1,
        kind: &SemanticStatementKindV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let Some(bridge) = self
            .math_ssa
            .as_ref()
            .and_then(|p| p.bridges.get(&site))
            .cloned()
        else {
            return Ok(false);
        };
        if bridge.state != MathBridgeStateV1::Pending
            || !self.is_kernel_entry
            || !matches!(kind, SemanticStatementKindV1::Assign(a) if *a == bridge.assignment)
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let receiver = self.math_live_binding(bridge.result.receiver().index(), bridge.receiver)?;
        if receiver
            .values()
            .map_err(|d| unsupported(0, Some(site.block.index()), Some(site.statement), d))?
            != [self.math_issuer_value(bridge.context)?]
            || !matches!(
                self.math_live_binding(bridge.result.current().index(), bridge.current)?,
                SemanticValueBindingV1::MathContext
            )
        {
            return Err(unsupported(
                0,
                Some(site.block.index()),
                Some(site.statement),
                "Math bridge lost its actual getter receiver or Current result",
            ));
        }
        for (local, value) in [
            (bridge.result.return_local(), bridge.returned),
            (bridge.result.destination(), bridge.destination),
        ] {
            if self
                .pending_semantic_ssa_definitions
                .get_mut(&(site.block.index(), local.index()))
                .and_then(VecDeque::pop_front)
                != Some(value)
                || self.semantic_ssa_bindings.contains_key(&value)
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            self.locals[local.index() as usize] = None;
            self.retained_local_initialized.remove(&local.index());
        }
        self.math_ssa
            .as_mut()
            .unwrap()
            .bridges
            .get_mut(&site)
            .unwrap()
            .state = MathBridgeStateV1::Transferred;
        Ok(true)
    }

    fn math_bridge_entry(&self, block: u32, local: u32, value: SsaValueV1) -> bool {
        self.math_ssa.as_ref().is_some_and(|p| {
            p.bridges.values().any(|b| {
                b.state == MathBridgeStateV1::Transferred
                    && b.result.getter_block().index() == block
                    && b.result.destination().index() == local
                    && b.destination == value
            })
        })
    }

    fn consume_math_bridge(
        &mut self,
        site: MathStatementSiteV1,
        assignment: &SemanticAssignmentV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let bridge = self
            .math_ssa
            .as_mut()
            .and_then(|p| {
                p.bridges.values_mut().find(|b| {
                    b.result.getter_block() == site.block
                        && b.result.getter_statement() == site.statement
                })
            })
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if bridge.state != MathBridgeStateV1::Transferred
            || !matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place))
                if place.local() == bridge.result.destination() && place.projections().is_empty() && place.ty() == bridge.result.math())
            || self.locals[bridge.result.destination().index() as usize].is_some()
            || self.semantic_ssa_bindings.contains_key(&bridge.destination)
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        bridge.state = MathBridgeStateV1::Consumed;
        Ok(())
    }

    fn require_math_bridges_consumed(&self) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.math_ssa.as_ref().is_some_and(|p| {
            p.bridges
                .values()
                .any(|b| b.state != MathBridgeStateV1::Consumed)
        }) {
            return Err(unsupported(
                0,
                None,
                None,
                "defined Math bridge result was not consumed at its original getter return",
            ));
        }
        Ok(())
    }

    fn math_live_binding(
        &self,
        local: u32,
        value: SsaValueV1,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.math_live_field_binding(local, value, &[])
    }

    fn math_live_field_binding(
        &self,
        local: u32,
        value: SsaValueV1,
        projections: &[SemanticProjectionV1],
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        if projections.len() > 16 || projections.iter().any(|p| !matches!(p.kind(), SemanticProjectionKindV1::Field(_))) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let fields = || projections.iter().map(|p| match p.kind() {
            SemanticProjectionKindV1::Field(field) => field,
            _ => unreachable!(),
        });
        let actual = self
            .locals
            .get(local as usize)
            .and_then(Option::as_ref)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let original = self
            .semantic_ssa_bindings
            .get(&value)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let actual = math_capture_binding_field(actual, fields())?;
        let original = math_capture_binding_field(original, fields())?;
        let matches = math_reference_binding_matches(actual, original);
        if !matches {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(actual.clone())
    }

    fn lower_math_reference(
        &mut self,
        site: MathStatementSiteV1,
        kind: &SemanticStatementKindV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let Some(flow) = self
            .math_ssa
            .as_ref()
            .and_then(|p| p.custody.references.get(&site))
            .cloned()
        else {
            return Ok(false);
        };
        if !matches!(kind, SemanticStatementKindV1::Assign(a) if *a == flow.assignment) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let projections = match flow.assignment.value().kind() {
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) => place.projections(),
            _ => &[],
        };
        let actual = self.math_live_field_binding(flow.source_local, flow.source, projections)?;
        let issuer = self
            .semantic_ssa_bindings
            .get(&flow.issuer)
            .cloned()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if actual
            .values()
            .map_err(|d| unsupported(0, Some(site.block.index()), Some(site.statement), d))?
            != issuer
                .values()
                .map_err(|d| unsupported(0, Some(site.block.index()), Some(site.statement), d))?
        {
            return Err(unsupported(
                0,
                Some(site.block.index()),
                Some(site.statement),
                "Math reference differs from its checked live SSA issuer",
            ));
        }
        if let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place)) =
            flow.assignment.value().kind()
        {
            if !place.projections().is_empty() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            self.locals[flow.source_local as usize] = None;
            self.retained_local_initialized.remove(&flow.source_local);
        }
        self.bind_destination(
            site.block,
            Some(site.statement),
            flow.assignment.destination(),
            issuer,
        )?;
        Ok(true)
    }
}
