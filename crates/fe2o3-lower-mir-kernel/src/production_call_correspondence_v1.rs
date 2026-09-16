// Live emission anchors. These are not a serialized proof relation or a second
// value graph; source meaning still comes from whole-owner lowering replay.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SemanticKirCallReturnV1 {
    correspondence_owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    semantic_block: SemanticBlockIdV1,
    kind: SemanticKirCallReturnKindV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SemanticKirCallReturnKindV1 {
    Call {
        arguments_first: u32,
        call_operation: u32,
        destination_end: u32,
        destination: SemanticKirCallDestinationV1,
        transport: CallComponentSpanV1,
    },
    Return {
        components: CallComponentSpanV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CallComponentSpanV1 {
    first: u32,
    count: u32,
}

impl CallComponentSpanV1 {
    const EMPTY: Self = Self { first: 0, count: 0 };

    fn range(self) -> Result<std::ops::Range<usize>, ProductionSemanticKirErrorV1> {
        if self.count == 0 && self.first != 0 {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let end = self
            .first
            .checked_add(self.count)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        Ok(self.first as usize..end as usize)
    }

    fn rebase(&mut self, offset: u32) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.count != 0 {
            self.first = self
                .first
                .checked_add(offset)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
        self.range()?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallResultComponentV1 {
    Return {
        input: ValueId,
        conversion: Option<u32>,
    },
    Transport {
        slot: u32,
        conversion: Option<u32>,
    },
}

impl SemanticKirCallReturnV1 {
    fn components(self) -> CallComponentSpanV1 {
        match self.kind {
            SemanticKirCallReturnKindV1::Call { transport, .. } => transport,
            SemanticKirCallReturnKindV1::Return { components } => components,
        }
    }

    fn rebase_components(&mut self, offset: u32) -> Result<(), ProductionSemanticKirErrorV1> {
        match &mut self.kind {
            SemanticKirCallReturnKindV1::Call { transport, .. } => transport.rebase(offset),
            SemanticKirCallReturnKindV1::Return { components } => components.rebase(offset),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SemanticKirCallDestinationV1 {
    Local,
    Retained {
        pointer: ValueId,
        access: MemoryAccess,
    },
    Projected {
        pointer: ValueId,
        access: MemoryAccess,
    },
}

struct HelperResultShapeV1 {
    local: SemanticLocalIdV1,
    source_type: SemanticTypeIdV1,
    aggregate: bool,
    components: Vec<ByValueKernelParameterComponentV1>,
}

fn helper_result_components_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    function_id: SemanticFunctionIdV1,
) -> Result<HelperResultShapeV1, ProductionSemanticKirErrorV1> {
    let mut returns = function
        .locals()
        .iter()
        .enumerate()
        .filter(|(_, local)| local.role() == SemanticLocalRoleV1::Return);
    let Some((local, declaration)) = returns.next().filter(|_| returns.next().is_none()) else {
        return Err(unsupported(
            function_id.index(),
            None,
            None,
            "helper must have one return local",
        ));
    };
    let abi = function.abi();
    if declaration.ty() != abi.source_output_type()
        || abi.return_value().ty() != abi.source_output_type()
        || abi.return_value().adjusted().is_some()
        || abi.return_value().pointee_override().is_some()
    {
        return Err(unsupported(
            function_id.index(),
            None,
            None,
            "helper return ABI type changed",
        ));
    }
    let aggregate = !matches!(
        types[abi.source_output_type().index() as usize].shape(),
        SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
    );
    let components = lower_by_value_abi_components_v1(
        types,
        function,
        abi.return_value(),
        ParameterLeafPolicyV1::PointerFree,
    )?;
    let local =
        u32::try_from(local).map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    Ok(HelperResultShapeV1 {
        local: SemanticLocalIdV1::from_index(local),
        source_type: abi.source_output_type(),
        aggregate,
        components,
    })
}

fn call_operation_ordinal_v1(
    operations: &[Operation],
    block: SemanticBlockIdV1,
) -> Result<u32, ProductionSemanticKirErrorV1> {
    measured_operation_span(
        operations.len(),
        operations.len(),
        BlockId(block.index()),
        None,
    )
    .map(|(first, _)| first)
}

impl SemanticFunctionLoweringV1<'_> {
    fn record_call_return_v1(
        &mut self,
        block: SemanticBlockIdV1,
        kind: SemanticKirCallReturnKindV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.call_returns.sites.push(SemanticKirCallReturnV1 {
            correspondence_owner: self.correspondence_owner,
            semantic_function: self.semantic_function,
            semantic_block: block,
            kind,
        })
    }
}
