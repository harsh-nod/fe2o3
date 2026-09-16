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
        transport: Option<CallResultTransportV1>,
    },
    Return {
        input: Option<ValueId>,
        conversion: Option<u32>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CallResultTransportV1 {
    slot: u32,
    conversion: Option<u32>,
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

fn helper_result_shape_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    function_id: SemanticFunctionIdV1,
) -> Result<(SemanticLocalIdV1, Option<Type>), ProductionSemanticKirErrorV1> {
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
    {
        return Err(unsupported(
            function_id.index(),
            None,
            None,
            "helper return ABI type changed",
        ));
    }
    let ty = match abi.return_value().mode() {
        SemanticAbiPassModeV1::Ignore
            if types[abi.source_output_type().index() as usize]
                .layout()
                .size_bytes()
                == Some(0) =>
        {
            None
        }
        SemanticAbiPassModeV1::Direct(_) => {
            Some(lower_scalar_type(types, abi.source_output_type())?)
        }
        _ => {
            return Err(unsupported(
                function_id.index(),
                None,
                None,
                "helper return is not one ignored zero-sized value or one direct scalar",
            ));
        }
    };
    let local =
        u32::try_from(local).map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    Ok((SemanticLocalIdV1::from_index(local), ty))
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
        if self.call_returns.rows.len() >= self.call_returns.requested {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        self.call_returns.rows.push(SemanticKirCallReturnV1 {
            correspondence_owner: self.correspondence_owner,
            semantic_function: self.semantic_function,
            semantic_block: block,
            kind,
        });
        Ok(())
    }
}
