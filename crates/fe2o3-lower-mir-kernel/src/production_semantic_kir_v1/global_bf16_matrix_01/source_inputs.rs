// Replayed source inputs, not a constructor-bounds or memory-event receipt.
// Scalar uses retain their source positions for the common use-specific query.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GlobalBf16SourceOperandV1<'a> {
    pub(super) site: CapabilityDefinitionSiteV1,
    pub(super) operand: &'a SemanticOperandV1,
}

#[derive(Debug)]
pub(super) struct GlobalBf16CapturedInputsV1<'a> {
    body: &'a SemanticFunctionDeclV1,
    call: &'a SemanticDirectCallV1,
    contract: SemanticGlobalBf16MatrixLoadV1,
    consumer: CapabilityDefinitionSiteV1,
    matrix: SsaValueV1,
    construction: CapabilityDefinitionSiteV1,
    global: SsaValueV1,
    global_bind: CapabilityDefinitionSiteV1,
    physical: GlobalBf16SourceOperandV1<'a>,
    geometry: [GlobalBf16SourceOperandV1<'a>; 4],
    lane: GlobalBf16SourceOperandV1<'a>,
    bases: [GlobalBf16SourceOperandV1<'a>; 2],
}

pub(super) struct GlobalBf16SourceQueryV1<'graph, 'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    view: &'a SemanticExpandedRootV1,
    context: &'graph RootKernelContextLoweringV1,
    graph: &'graph mut CapabilitySsaGraphV1<'a>,
}

pub(super) struct GlobalBf16SourceUseV1<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    view: &'a SemanticExpandedRootV1,
    inputs: GlobalBf16CapturedInputsV1<'a>,
}

impl<'a> GlobalBf16SourceUseV1<'a> {
    pub(super) fn owner(&self) -> &'a ProductionSemanticSsaOwnerV1 {
        self.owner
    }
    pub(super) fn view(&self) -> &'a SemanticExpandedRootV1 {
        self.view
    }
    pub(super) fn inputs(&self) -> &GlobalBf16CapturedInputsV1<'a> {
        &self.inputs
    }
}

impl<'graph, 'a> GlobalBf16SourceQueryV1<'graph, 'a> {
    pub(super) fn new(
        owner: &'a ProductionSemanticSsaOwnerV1,
        root: SemanticFunctionIdV1,
        context: &'graph RootKernelContextLoweringV1,
        graph: &'graph mut CapabilitySsaGraphV1<'a>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        owner
            .verify_replay()
            .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
        let view = owner
            .execution_view_for_root(root)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let plan = owner
            .execution_plan_for_root(root)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        // The normal path borrows this same immutable plan. Equal copies still
        // require the complete, charged comparison; identity grants no custody.
        graph.charge(1)?;
        if !std::ptr::eq(graph.ssa, plan.plan()) {
            graph.charge(plan.plan().resources().storage_words())?;
            if graph.ssa != plan.plan() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        }
        if !std::ptr::eq(graph.body, view.body()) || context.selected_root != root {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(Self {
            owner,
            view,
            context,
            graph,
        })
    }

    pub(super) fn capture(
        &mut self,
        block: u32,
    ) -> Result<GlobalBf16SourceUseV1<'a>, ProductionSemanticKirErrorV1> {
        let origin = self
            .view
            .block_origins()
            .get(block as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if origin.terminator() != fe2o3_mir_model::SemanticExpandedTerminatorOriginV1::Source {
            return Err(reject(
                "global BF16 load is not an original source terminal",
            ));
        }
        let source = self.owner.source_semantic();
        let original = source
            .functions()
            .get(origin.function().index() as usize)
            .and_then(|body| body.blocks().get(origin.block().index() as usize))
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let actual = self
            .view
            .body()
            .blocks()
            .get(block as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if !matches!((original.terminator().kind(), actual.terminator().kind()),
            (SemanticTerminatorKindV1::Call(original), SemanticTerminatorKindV1::Call(actual))
                if original.callee() == actual.callee())
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let inputs = capture_inputs(
            self.graph,
            source.types(),
            source.callables(),
            self.context,
            block,
            Some((self.owner, self.view.root())),
        )?;
        Ok(GlobalBf16SourceUseV1 {
            owner: self.owner,
            view: self.view,
            inputs,
        })
    }
}

impl<'a> GlobalBf16CapturedInputsV1<'a> {
    pub(super) fn geometry(&self) -> &[GlobalBf16SourceOperandV1<'a>; 4] {
        &self.geometry
    }
    pub(super) fn physical(&self) -> GlobalBf16SourceOperandV1<'a> {
        self.physical
    }
    pub(super) fn lane(&self) -> GlobalBf16SourceOperandV1<'a> {
        self.lane
    }
    pub(super) fn bases(&self) -> &[GlobalBf16SourceOperandV1<'a>; 2] {
        &self.bases
    }
    pub(super) fn construction(&self) -> CapabilityDefinitionSiteV1 {
        self.construction
    }
    pub(super) fn global(&self) -> SsaValueV1 {
        self.global
    }
    pub(super) fn global_bind(&self) -> CapabilityDefinitionSiteV1 {
        self.global_bind
    }
    pub(super) fn matrix(&self) -> SsaValueV1 {
        self.matrix
    }
    pub(super) fn contract(&self) -> SemanticGlobalBf16MatrixLoadV1 {
        self.contract
    }

    // Matching an occurrence is not consumption of the four read events. The
    // eventual common consumer must additionally close its complete roster.
    pub(super) fn matches_use(
        &self,
        body: &SemanticFunctionDeclV1,
        block: u32,
        call: &SemanticDirectCallV1,
    ) -> bool {
        std::ptr::eq(body, self.body)
            && block == self.consumer.block
            && call == self.call
            && matches!(body.blocks().get(block as usize).map(|b| b.terminator().kind()),
                Some(SemanticTerminatorKindV1::Call(actual)) if actual == call)
    }
}

fn capture_inputs<'a>(
    graph: &mut CapabilitySsaGraphV1<'a>,
    types: &'a [SemanticTypeDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    context: &RootKernelContextLoweringV1,
    block: u32,
    observation_source: Option<(&'a ProductionSemanticSsaOwnerV1, SemanticFunctionIdV1)>,
) -> Result<GlobalBf16CapturedInputsV1<'a>, ProductionSemanticKirErrorV1> {
    let body = graph.body;
    let SemanticTerminatorKindV1::Call(call) = body
        .blocks()
        .get(block as usize)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
        .terminator()
        .kind()
    else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        binding,
        operation: SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { contract },
        ..
    }) = callables.get(call.callee().index() as usize)
    else {
        return Err(reject(
            "global BF16 source use lost its authenticated load contract",
        ));
    };
    let t = contract.types();
    let Some(destination) = call.destination() else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    if !global_capability_provenance_matches_v1(context, contract.provenance())
        || binding.identity() != contract.source_identity()
        || !semantic_global_bf16_matrix_layout_matches_v1(types, t)
        || call.arguments().len() != 4
        || binding.abi().source_input_types().len() != 4
        || binding.abi().source_output_type() != t.fragment
        || binding.abi().c_variadic()
        || !call.variadic_argument_abis().is_empty()
        || binding.abi().source_argument_ownership()
            != [
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ]
        || !global_bf16_shared_reference_v1(types, call.arguments()[0].ty(), t.matrix)
        || !global_bf16_shared_reference_v1(types, call.arguments()[1].ty(), t.lane)
        || call.arguments()[2..]
            .iter()
            .any(|operand| operand.ty() != t.index)
        || call
            .arguments()
            .iter()
            .zip(binding.abi().source_input_types())
            .any(|(operand, ty)| operand.ty() != *ty)
        || destination.place().ty() != t.fragment
        || !destination.place().projections().is_empty()
        || !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
    {
        return Err(reject(
            "global BF16 source use changed root, type, ownership or source ABI",
        ));
    }
    graph.charge(
        std::mem::size_of::<GlobalBf16SourceUseV1<'_>>().div_ceil(std::mem::size_of::<usize>()),
    )?;
    let consumer = CapabilityDefinitionSiteV1 {
        block,
        statement: None,
        local: destination.place().local().index(),
    };
    let mut resolver = Resolver {
        graph,
        types,
        callables,
        active: BTreeSet::new(),
        consumer,
        contract: *contract,
        target: ResolveTarget::MatrixConstruction,
        observation_source,
    };
    let matrix = resolver.operand(
        block,
        &call.arguments()[0],
        &[SemanticProjectionKindV1::Dereference],
    )?;
    let construction = resolver.graph.definition(matrix)?;
    let statement = construction
        .statement
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let SemanticStatementKindV1::Assign(assignment) =
        body.blocks()[construction.block as usize].statements()[statement as usize].kind()
    else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    let fields = aggregate.operands();
    let SemanticTypeShapeV1::Aggregate(field_types) = types[t.matrix.index() as usize].shape()
    else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    if fields
        .iter()
        .zip(field_types.fields())
        .any(|(value, ty)| value.ty() != *ty)
    {
        return Err(reject(
            "global BF16 source construction changed its field types",
        ));
    }
    resolver.target = ResolveTarget::Global;
    let global = resolver.operand(
        block,
        &call.arguments()[0],
        &[
            SemanticProjectionKindV1::Dereference,
            SemanticProjectionKindV1::Field(0),
            SemanticProjectionKindV1::Dereference,
        ],
    )?;
    let global_bind = resolver.graph.definition(global)?;
    let SemanticTerminatorKindV1::Call(bind) = body.blocks()[global_bind.block as usize]
        .terminator()
        .kind()
    else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        operation:
            SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindReadOnly { physical, .. },
        ..
    }) = callables.get(bind.callee().index() as usize)
    else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    if bind.arguments().len() != 2 || bind.arguments()[1].ty() != *physical {
        return Err(reject(
            "global BF16 bind lost its exact physical input operand",
        ));
    }
    Ok(GlobalBf16CapturedInputsV1 {
        body,
        call,
        contract: *contract,
        consumer,
        matrix,
        construction,
        global,
        global_bind,
        physical: GlobalBf16SourceOperandV1 {
            site: global_bind,
            operand: &bind.arguments()[1],
        },
        geometry: std::array::from_fn(|i| GlobalBf16SourceOperandV1 {
            site: construction,
            operand: &fields[i + 1],
        }),
        lane: GlobalBf16SourceOperandV1 {
            site: consumer,
            operand: &call.arguments()[1],
        },
        bases: std::array::from_fn(|i| GlobalBf16SourceOperandV1 {
            site: consumer,
            operand: &call.arguments()[i + 2],
        }),
    })
}

#[cfg(test)]
pub(super) fn capture_component<'a>(
    graph: &mut CapabilitySsaGraphV1<'a>,
    types: &'a [SemanticTypeDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    context: &RootKernelContextLoweringV1,
    block: u32,
) -> Result<GlobalBf16CapturedInputsV1<'a>, ProductionSemanticKirErrorV1> {
    capture_inputs(graph, types, callables, context, block, None)
}
