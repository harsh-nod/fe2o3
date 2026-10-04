// A payload receipt names the original value operand, not permission to access
// the allocation. Address, extent, index authority and guard replay remain separate.
fn scoped_checked_write_payload_header_v84() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<Option<ScopedMemoryStoreSourceV29>>(),
        std::mem::size_of::<SemanticValueBindingV1>(),
        std::mem::size_of::<ExecutionOperandV29>(),
    ])
}

fn scoped_write_payload_operand_v84<'a>(
    function: &'a SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    site: ExecutionSiteV29,
    argument: u32,
    ty: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<&'a SemanticOperandV1, ProductionSemanticKirErrorV1> {
    budget.charge_work(12)?;
    let ExecutionSiteV29::Terminator { block } = site else {
        return Err(scoped_memory_error_v29());
    };
    let Some(SemanticTerminatorKindV1::Call(call)) = function
        .blocks()
        .get(block.get() as usize)
        .map(|block| block.terminator().kind())
    else {
        return Err(scoped_memory_error_v29());
    };
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        operation:
            SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite { element, kind, .. },
        ..
    }) = callables.get(call.callee().index() as usize)
    else {
        return Err(scoped_memory_error_v29());
    };
    let expected = match kind {
        SemanticWriteOnlyDisjointWriteKindV1::Thread { .. } => 2,
        SemanticWriteOnlyDisjointWriteKindV1::GridExclusive
        | SemanticWriteOnlyDisjointWriteKindV1::Block { .. } => 3,
        SemanticWriteOnlyDisjointWriteKindV1::Tiled2d { .. }
        | SemanticWriteOnlyDisjointWriteKindV1::RowStriped2d { .. } => 6,
    };
    let operand = call
        .arguments()
        .get(argument as usize)
        .ok_or_else(scoped_memory_error_v29)?;
    if argument != expected
        || call.arguments().len() != expected as usize + 1
        || !call.variadic_argument_abis().is_empty()
        || *element != ty
        || operand.ty() != ty
    {
        return Err(scoped_memory_error_v29());
    }
    Ok(operand)
}
