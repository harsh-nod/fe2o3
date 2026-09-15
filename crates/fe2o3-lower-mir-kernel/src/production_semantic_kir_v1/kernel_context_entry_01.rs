// Exact source-checked Context entry transfer, without rewriting semantic MIR.

/// Inert coordinates of an authenticated frontend source edge erased by rustc.
/// Construction grants no authority. The production importer must prove the
/// initializer-to-argument binding and retain this record in its Context custody.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProductionKernelContextEntryTransferV1 {
    semantic: [u8; 32],
    root: SemanticFunctionIdentityV1,
    helper: SemanticFunctionIdentityV1,
    issuer: SemanticFunctionIdentityV1,
    context: SemanticTypeIdentityV1,
    issuer_block: SemanticBlockIdV1,
    issuer_local: SemanticLocalIdV1,
    call_block: SemanticBlockIdV1,
    argument: u32,
    source_binding: [u8; 32],
}

impl ProductionKernelContextEntryTransferV1 {
    /// Records inert source coordinates; construction does not authenticate them.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        semantic: [u8; 32],
        root: SemanticFunctionIdentityV1,
        helper: SemanticFunctionIdentityV1,
        issuer: SemanticFunctionIdentityV1,
        context: SemanticTypeIdentityV1,
        issuer_block: SemanticBlockIdV1,
        issuer_local: SemanticLocalIdV1,
        call_block: SemanticBlockIdV1,
        argument: u32,
        source_binding: [u8; 32],
    ) -> Self {
        Self {
            semantic,
            root,
            helper,
            issuer,
            context,
            issuer_block,
            issuer_local,
            call_block,
            argument,
            source_binding,
        }
    }

    /// Complete fixed-width commitment for the private frontend custody digest.
    /// This is not a MIR/KIR wire schema or an authentication token.
    pub fn commitment_bytes(self) -> [u8; 208] {
        let mut bytes = [0; 208];
        for (slot, identity) in bytes[..192].chunks_exact_mut(32).zip([
            &self.semantic,
            self.root.as_bytes(),
            self.helper.as_bytes(),
            self.issuer.as_bytes(),
            self.context.as_bytes(),
            &self.source_binding,
        ]) {
            slot.copy_from_slice(identity);
        }
        for (slot, value) in bytes[192..].chunks_exact_mut(4).zip([
            self.issuer_block.index(),
            self.issuer_local.index(),
            self.call_block.index(),
            self.argument,
        ]) {
            slot.copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct KernelContextEntryPlanV1 {
    block: SemanticBlockIdV1,
    statement: u32,
    destination: SemanticLocalIdV1,
    context: SemanticTypeIdV1,
    issuer: SsaValueV1,
    parameter: SsaValueV1,
}

impl KernelContextEntryPlanV1 {
    fn new(
        owner: &ProductionSemanticSsaOwnerV1,
        root: SemanticFunctionIdV1,
        context: SemanticTypeIdV1,
        input: ProductionKernelContextEntryTransferV1,
        max_work: usize,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        use self::capability_ssa_graph_01::CapabilitySsaGraphV1;
        use fe2o3_mir_model::{
            SemanticExpandedStatementOriginV1 as S, SemanticExpandedTerminatorOriginV1 as T,
            SsaEdgeIdV1,
        };
        let reject = || {
            unsupported(
                root.index(),
                None,
                None,
                "Context entry source transfer changed",
            )
        };
        let semantic = owner.source_semantic();
        let source = semantic
            .functions()
            .get(root.index() as usize)
            .ok_or_else(reject)?;
        if source.identity() != input.root
            || input.source_binding == [0; 32]
            || semantic.semantic_sha256().as_bytes() != &input.semantic
            || semantic
                .types()
                .get(context.index() as usize)
                .map(SemanticTypeDeclV1::identity)
                != Some(input.context)
        {
            return Err(reject());
        }
        let source_call = |block: SemanticBlockIdV1| {
            let data = source
                .blocks()
                .get(block.index() as usize)
                .ok_or_else(reject)?;
            let SemanticTerminatorKindV1::Call(call) = data.terminator().kind() else {
                return Err(reject());
            };
            Ok(call)
        };
        let issuer_call = source_call(input.issuer_block)?;
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context: issued },
            ..
        }) = semantic
            .callables()
            .get(issuer_call.callee().index() as usize)
        else {
            return Err(reject());
        };
        if *issued != context
            || binding.identity() != input.issuer
            || !issuer_call.arguments().is_empty()
            || !issuer_call.destination().is_some_and(|d| {
                d.place().local() == input.issuer_local
                    && d.place().projections().is_empty()
                    && d.place().ty() == context
            })
        {
            return Err(reject());
        }
        let call = source_call(input.call_block)?;
        let Some(SemanticCallableDeclV1::Defined { function: helper }) =
            semantic.callables().get(call.callee().index() as usize)
        else {
            return Err(reject());
        };
        let helper_body = &semantic.functions()[helper.index() as usize];
        if helper_body.identity() != input.helper
            || helper_body
                .abi()
                .source_input_types()
                .get(input.argument as usize)
                != Some(&context)
            || !matches!(call.arguments().get(input.argument as usize),
                Some(SemanticOperandV1::Constant(c)) if c.ty() == context && matches!(c.value(), SemanticConstantValueV1::ZeroSized))
        {
            return Err(reject());
        }
        let view = owner.execution_view_for_root(root).ok_or_else(reject)?;
        let ssa = owner
            .execution_plan_for_root(root)
            .ok_or_else(reject)?
            .plan();
        let mut graph = CapabilitySsaGraphV1::new(view.body(), ssa, max_work)?;
        let mut issuer_site = None;
        let mut transfer_site = None;
        for (block, origin) in view.block_origins().iter().enumerate() {
            graph.charge(1)?;
            if origin.function() != root || origin.instance().index() != 0 {
                continue;
            }
            if origin.block() == input.issuer_block && origin.terminator() == T::Source {
                let SemanticTerminatorKindV1::Call(call) =
                    view.body().blocks()[block].terminator().kind()
                else {
                    return Err(reject());
                };
                let destination = call.destination().ok_or_else(reject)?.place();
                let local_origin = view
                    .local_origins()
                    .get(destination.local().index() as usize)
                    .ok_or_else(reject)?;
                if local_origin.function() != root
                    || local_origin.instance().index() != 0
                    || local_origin.local() != input.issuer_local
                {
                    return Err(reject());
                }
                let definitions = ssa
                    .edge_definitions(SsaEdgeIdV1::new(SsaBlockIdV1::new(block as u32), 0))
                    .ok_or_else(reject)?;
                graph.charge(definitions.len())?;
                for definition in definitions {
                    if definition.variable().get() == destination.local().index()
                        && issuer_site
                            .replace((block as u32, destination.local(), definition.value()))
                            .is_some()
                    {
                        return Err(reject());
                    }
                }
            }
            if origin.block() == input.call_block {
                let T::CallEntry { callee } = origin.terminator() else {
                    return Err(reject());
                };
                for (statement, attribution) in origin.statements().iter().enumerate() {
                    graph.charge(1)?;
                    if *attribution
                        != (S::ParameterTransfer {
                            callee,
                            argument: input.argument,
                        })
                    {
                        continue;
                    }
                    let SemanticStatementKindV1::Assign(assignment) =
                        view.body().blocks()[block].statements()[statement].kind()
                    else {
                        return Err(reject());
                    };
                    let destination = assignment.destination();
                    let local_origin = view
                        .local_origins()
                        .get(destination.local().index() as usize)
                        .ok_or_else(reject)?;
                    if local_origin.function() != *helper
                        || local_origin.instance() != callee
                        || helper_body
                            .locals()
                            .get(local_origin.local().index() as usize)
                            .map(|local| local.role())
                            != Some(SemanticLocalRoleV1::Argument(input.argument))
                        || !destination.projections().is_empty()
                        || destination.ty() != context
                        || !matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(c))
                            if c.ty() == context && matches!(c.value(), SemanticConstantValueV1::ZeroSized))
                        || transfer_site
                            .replace((block as u32, statement as u32, destination.local()))
                            .is_some()
                    {
                        return Err(reject());
                    }
                }
            }
        }
        let (issuer_block, issuer_local, issuer) = issuer_site.ok_or_else(reject)?;
        let (block, statement, destination) = transfer_site.ok_or_else(reject)?;
        let transfer_region = graph.path_region(issuer_block, block)?;
        if !transfer_region[block as usize] {
            return Err(reject());
        }
        // The frontend proved the pre-erasure use. Independently reject source
        // mutations that kill or overwrite that exact issuer before transfer.
        for reachable in ssa.reverse_postorder() {
            let current = reachable.get();
            if current == issuer_block || !transfer_region[current as usize] {
                continue;
            }
            let data = &view.body().blocks()[current as usize];
            let end = if current == block {
                statement as usize
            } else {
                data.statements().len()
            };
            graph.charge(end)?;
            for item in &data.statements()[..end] {
                if self::capability_ssa_graph_01::invalidates(item.kind(), issuer_local.index()) {
                    return Err(reject());
                }
            }
            if current != block {
                if let SemanticTerminatorKindV1::Call(call) = data.terminator().kind() {
                    graph.charge(call.arguments().len())?;
                    if call
                        .arguments()
                        .iter()
                        .any(|a| self::capability_ssa_graph_01::moves(a, issuer_local.index()))
                        || call
                            .destination()
                            .is_some_and(|d| d.place().local() == issuer_local)
                    {
                        return Err(reject());
                    }
                }
                if matches!(data.terminator().kind(), SemanticTerminatorKindV1::Drop { place, .. } if place.local() == issuer_local)
                {
                    return Err(reject());
                }
            }
        }
        let events = ssa
            .resolved_events(SsaBlockIdV1::new(block))
            .ok_or_else(reject)?;
        graph.charge(events.len())?;
        let mut parameter = None;
        for (_, event) in events {
            if let SsaResolvedEventV1::Define { variable, value } = event
                && variable.get() == destination.index()
                && parameter.replace(*value).is_some()
            {
                return Err(reject());
            }
        }
        let parameter = parameter.ok_or_else(|| {
            // Planner constructs this immutable roster in ascending local order.
            let promoted = ssa
                .promoted_variables()
                .binary_search_by_key(&destination.index(), |variable| variable.get())
                .is_ok();
            unsupported(
                root.index(),
                Some(block),
                Some(statement),
                if promoted {
                    "Context entry source transfer changed: missing parameter SSA definition; destination promoted=true"
                } else {
                    "Context entry source transfer changed: missing parameter SSA definition; destination promoted=false"
                },
            )
        })?;
        Ok(Self {
            block: SemanticBlockIdV1::from_index(block),
            statement,
            destination,
            context,
            issuer,
            parameter,
        })
    }
}

impl SemanticFunctionLoweringV1<'_> {
    fn lower_kernel_context_entry_v1(
        &self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let Some(context) = self.kernel_context else {
            return Ok(None);
        };
        let Some(plan) = &context.entry_transfer else {
            return Ok(None);
        };
        if plan.block != block || statement != Some(plan.statement) {
            return Ok(None);
        }
        let reject = || {
            unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                statement,
                "Context entry lost its exact source SSA issuer",
            )
        };
        if assignment.destination().local() != plan.destination
            || !assignment.destination().projections().is_empty()
            || assignment.destination().ty() != plan.context
            || assignment.value().result_type() != plan.context
            || !matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(c)) if c.ty() == plan.context && matches!(c.value(), SemanticConstantValueV1::ZeroSized))
        {
            return Err(reject());
        }
        let binding = self
            .semantic_ssa_bindings
            .get(&plan.issuer)
            .ok_or_else(reject)?;
        if !matches!(binding, SemanticValueBindingV1::KernelContext { context: actual, .. } if *actual == context.context_type)
        {
            return Err(reject());
        }
        Ok(Some(binding.clone()))
    }
}
