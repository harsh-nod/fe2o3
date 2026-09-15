// Existing V23 tag7 only. This module neither invents a subgroup/epoch nor
// authorizes an untyped MatrixContext consumer.
mod kernel_matrix_derive_01 {
    use super::capability_ssa_graph_01::{
        CapabilityDefinitionSiteV1, CapabilityLoanV1, CapabilitySsaGraphV1,
    };
    use super::*;
    use fe2o3_mir_model::SemanticExpandedTerminatorOriginV1;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAssignmentV1, SemanticDefinedCapabilityContractV1, SemanticKernelMatrixDeriveV1,
    };
    use fe2o3_pliron::ProductionSemanticMatrixBridgeResultV1;

    type Site = (SemanticBlockIdV1, u32);

    #[derive(Clone, Debug)]
    pub(super) struct MatrixValueV1 {
        pub(super) context: ValueId,
        pub(super) context_type: KernelContextTypeV1,
        pub(super) record: SemanticKernelMatrixDeriveV1,
        pub(super) source: ExecutionCapabilitySourceV1,
        pub(super) result: ProductionSemanticMatrixBridgeResultV1,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum State {
        Pending,
        Transferred,
        Consumed,
    }

    #[derive(Clone, Debug)]
    struct Bridge {
        result: ProductionSemanticMatrixBridgeResultV1,
        record: SemanticKernelMatrixDeriveV1,
        source: ExecutionCapabilitySourceV1,
        assignment: SemanticAssignmentV1,
        getter_assignment: SemanticAssignmentV1,
        current_block: u32,
        receiver: SsaValueV1,
        current: SsaValueV1,
        returned: SsaValueV1,
        destination: SsaValueV1,
        context: SsaValueV1,
        state: State,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct Reference {
        assignment: SemanticAssignmentV1,
        local: u32,
        value: SsaValueV1,
        issuer: SsaValueV1,
    }

    #[derive(Default)]
    pub(super) struct MatrixDerivePlanV1 {
        bridges: BTreeMap<Site, Bridge>,
        references: BTreeMap<Site, Reference>,
    }

    fn rejected(detail: &'static str) -> ProductionSemanticKirErrorV1 {
        unsupported(0, None, None, detail)
    }
    fn mismatch() -> ProductionSemanticKirErrorV1 {
        ProductionSemanticKirErrorV1::CorrespondenceMismatch
    }

    impl MatrixDerivePlanV1 {
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
            graph.charge(bindings.len().saturating_mul(32))?;
            let mut result = Self::default();
            for row in plan.defined_matrix_results() {
                graph.charge(bindings.len())?;
                let mut matches = bindings.iter().filter(|binding| {
                    binding.root() == context.selected_root
                        && binding.callee_instance() == row.getter()
                });
                let binding = matches.next().ok_or_else(mismatch)?;
                if matches.next().is_some() {
                    return Err(mismatch());
                }
                let SemanticDefinedCapabilityContractV1::KernelMatrixDerive(record) =
                    binding.contract()
                else {
                    return Err(mismatch());
                };
                let getter_site = (row.getter_block(), row.getter_statement());
                let bridge_site = (row.block(), row.statement());
                if !global_capability_provenance_matches_v1(context, record.provenance())
                    || record.types().context != context.semantic_type
                    || record.types().matrix != row.matrix()
                    || binding.callee_arguments() != [row.receiver()]
                    || binding.callee_return() != row.destination()
                    || view
                        .instances()
                        .get(row.bridge().index() as usize)
                        .is_none_or(|instance| {
                            instance.parent() != Some(row.getter())
                                || instance.function() != record.bridge().function()
                                || instance.function_identity() != record.bridge().source_identity()
                        })
                {
                    return Err(rejected(
                        "Matrix getter substituted root, source bridge or typed receiver",
                    ));
                }
                let source = CheckedExecutionSourceCarrierV1::source_for_defined_statement(
                    owner,
                    context.selected_root,
                    function,
                    *owner.execution_expansion().identity(),
                    *view.identity(),
                    binding,
                    getter_site.0,
                    getter_site.1,
                )?;
                let events = plan
                    .plan()
                    .resolved_events(SsaBlockIdV1::new(row.block().index()))
                    .ok_or_else(mismatch)?;
                graph.charge(events.len())?;
                let (receiver, current, returned, destination) =
                    matrix_bridge_events_v1(*row, events)?;
                let current_site = graph.definition(current)?;
                let current_body = &function.blocks()[current_site.block as usize];
                let origin = &view.block_origins()[current_site.block as usize];
                if current_site.statement.is_some()
                    || current_site.local != row.current().index()
                    || origin.instance() != row.bridge()
                    || origin.terminator() != SemanticExpandedTerminatorOriginV1::Source
                    || !matches!(current_body.terminator().kind(), SemanticTerminatorKindV1::Call(call)
                        if call.callee() == record.current_callable() && call.arguments().is_empty()
                            && call.destination().is_some_and(|d| d.place().local() == row.current()
                                && d.place().ty() == record.types().unbranded_matrix
                                && d.place().projections().is_empty()))
                    || graph.use_value(row.getter_block().index(), row.destination().index())?
                        != destination
                {
                    return Err(rejected(
                        "Matrix bridge Current lacks its exact normal-edge SSA definition",
                    ));
                }
                let (issuer, loans) = {
                    let mut resolver = ContextResolver {
                        owner,
                        context,
                        graph: &mut graph,
                        references: &mut result.references,
                    };
                    resolver.reference(receiver, record.types().context_reference, 0)?
                };
                for loan in loans {
                    for site in [bridge_site, getter_site] {
                        graph.loan_live(
                            loan,
                            CapabilityDefinitionSiteV1 {
                                block: site.0.index(),
                                statement: Some(site.1),
                                local: row.receiver().index(),
                            },
                        )?;
                    }
                }
                // No block argument may masquerade as the two fixed bridge values.
                for block in plan.plan().reverse_postorder() {
                    let variables = plan
                        .plan()
                        .transport_variables(*block)
                        .ok_or_else(mismatch)?;
                    graph.charge(variables.len())?;
                    if variables.iter().any(|variable| {
                        [
                            row.return_local().index(),
                            row.destination().index(),
                            row.current().index(),
                        ]
                        .contains(&variable.get())
                    }) {
                        return Err(rejected(
                            "Matrix bridge requires a unique acyclic SSA return path",
                        ));
                    }
                }
                let assignment = assignment_at(function, bridge_site)?.clone();
                let getter_assignment = assignment_at(function, getter_site)?.clone();
                if getter_assignment.destination() != binding.destination()
                    || !matches!(getter_assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place))
                        if place.local() == row.destination() && place.ty() == row.matrix() && place.projections().is_empty())
                {
                    return Err(mismatch());
                }
                if result
                    .bridges
                    .insert(
                        bridge_site,
                        Bridge {
                            result: *row,
                            record,
                            source,
                            assignment,
                            getter_assignment,
                            current_block: current_site.block,
                            receiver,
                            current,
                            returned,
                            destination,
                            context: issuer,
                            state: State::Pending,
                        },
                    )
                    .is_some()
                {
                    return Err(mismatch());
                }
            }
            let expected = bindings
                .iter()
                .filter(|binding| {
                    binding.root() == context.selected_root
                        && matches!(
                            binding.contract(),
                            SemanticDefinedCapabilityContractV1::KernelMatrixDerive(_)
                        )
                })
                .count();
            if result.bridges.len() != expected {
                return Err(mismatch());
            }
            Ok(result)
        }
    }

    fn assignment_at(
        function: &SemanticFunctionDeclV1,
        site: Site,
    ) -> Result<&SemanticAssignmentV1, ProductionSemanticKirErrorV1> {
        match function
            .blocks()
            .get(site.0.index() as usize)
            .and_then(|block| block.statements().get(site.1 as usize))
            .map(|s| s.kind())
        {
            Some(SemanticStatementKindV1::Assign(a)) => Ok(a),
            _ => Err(mismatch()),
        }
    }

    include!("kernel_matrix_derive_01/context.rs");
    include!("kernel_matrix_derive_01/bridge_events.rs");
    include!("kernel_matrix_derive_01/production.rs");

    #[cfg(test)]
    mod tests {
        use super::*;
        include!("kernel_matrix_derive_01/tests.rs");
    }
}
use kernel_matrix_derive_01::MatrixDerivePlanV1;
