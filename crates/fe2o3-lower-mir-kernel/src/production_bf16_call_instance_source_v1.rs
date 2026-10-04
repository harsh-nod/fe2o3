use super::*;

pub(super) fn call_at(
    owner: &ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
) -> Option<&SemanticDirectCallV1> {
    match owner
        .source_semantic()
        .functions()
        .get(function.index() as usize)?
        .blocks()
        .get(block.index() as usize)?
        .terminator()
        .kind()
    {
        SemanticTerminatorKindV1::Call(call) => Some(call),
        _ => None,
    }
}
fn inherited<T>(result: super::super::Result<T>) -> CallResult<T> {
    match result {
        Ok(value) => Ok(value),
        Err(super::super::Error::Resource(e)) => Err(e.into()),
        Err(super::super::Error::Unavailable(e)) => refuse(e),
        _ => Err(Resource::Accounting.into()),
    }
}
pub(super) fn role(
    operation: &SemanticCompilerIntrinsicOperationV1,
) -> CallResult<Option<CallRole>> {
    if matches!(
        operation,
        SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues { .. }
    ) {
        return Ok(Some(CallRole::Values));
    }
    Ok(
        inherited(super::super::source::role(operation))?.map(|role| match role {
            Role::Context => CallRole::Context,
            Role::Lane => CallRole::Lane,
            Role::Lhs => CallRole::Lhs,
            Role::Rhs => CallRole::Rhs,
            Role::Zero => CallRole::Zero,
            Role::Result => CallRole::Result,
        }),
    )
}
pub(super) fn rows(
    owner: &ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
) -> CallResult<Occurrences<'_>> {
    owner
        .occurrences_v1()
        .and_then(|r| r.function(function))
        .ok_or(CallError::Unavailable(
            "actual function occurrence capture required",
        ))
}
pub(super) fn site(block: SemanticBlockIdV1) -> Site {
    Site::Terminator {
        block: fe2o3_mir_model::SsaBlockIdV1::new(block.index()),
    }
}
pub(super) fn base_use(
    rows: &Occurrences<'_>,
    site: Site,
    operand: OperandRole,
    budget: &mut Budget<'_>,
) -> CallResult<SsaValueV1> {
    let mut result = None;
    for event in rows.events() {
        budget.charge_work(1)?;
        if event.site() == site && event.operand() == operand && event.role() == EventRole::BaseUse
        {
            let Some(SsaResolvedEventV1::Use { value, .. }) = event.resolved() else {
                return refuse("source operand is not an SSA use");
            };
            if !event.is_reachable() || !event.is_promoted() {
                return refuse("source operand is not reachable and promoted");
            }
            one(&mut result, value)?;
        }
    }
    result.ok_or(CallError::Unavailable("missing actual source use"))
}
pub(super) fn whole(operand: &SemanticOperandV1) -> bool {
    matches!(operand, SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p)
        if p.projections().is_empty())
}
fn result(
    owner: &ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    budget: &mut Budget<'_>,
) -> CallResult<Bf16CallInstanceProducerV1> {
    let call = call_at(owner, function, block).ok_or(CallError::Unavailable("producer call"))?;
    let destination = call
        .destination()
        .ok_or(CallError::Unavailable("producer destination"))?;
    if !destination.place().projections().is_empty()
        || !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
    {
        return refuse("projected/exceptional producer transport");
    }
    let local = destination.place().local().index();
    let rows = rows(owner, function)?;
    let mut value = None;
    for edge in rows.edge_definitions() {
        budget.charge_work(1)?;
        if edge.edge().source().get() == block.index() && edge.variable().get() == local {
            if edge.edge().ordinal() != 0 || !edge.is_reachable() || !edge.is_promoted() {
                return refuse("unavailable producer return edge");
            }
            one(
                &mut value,
                edge.value()
                    .ok_or(CallError::Unavailable("no producer SSA definition"))?,
            )?;
        }
    }
    for event in rows.events() {
        budget.charge_work(1)?;
        if event.site() == site(block)
            && event.role() == EventRole::DestinationDefine
            && let Some(SsaResolvedEventV1::Define {
                variable,
                value: actual,
            }) = event.resolved()
            && variable.get() == local
        {
            if !event.is_reachable() || !event.is_promoted() {
                return refuse("producer definition");
            }
            one(&mut value, actual)?;
        }
    }
    Ok(Bf16CallInstanceProducerV1 {
        function,
        block,
        value: value.ok_or(CallError::Unavailable("producer result retained, not SSA"))?,
    })
}
pub(super) fn array_type(
    owner: &ProductionSemanticSsaOwnerV1,
    ty: SemanticTypeIdV1,
) -> CallResult<()> {
    let types = owner.source_semantic().types();
    let Some(decl) = types.get(ty.index() as usize) else {
        return refuse("array source type");
    };
    let SemanticTypeShapeV1::Array { element, length: 4 } = decl.shape() else {
        return refuse("exact four-element source array required");
    };
    if !matches!(
        types
            .get(element.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float {
            bits: 32
        }))
    ) {
        return refuse("source array element is not f32");
    }
    Ok(())
}
// Charge the actual function header before inspecting its lengths. Refuse
// oversized cumulative headers before querying even the first statement count.
// The accessor borrows an existing block by ordinal; it cannot allocate or
// enumerate another source roster in the production caller below.
pub(super) fn scan_bounded_lengths(
    function_blocks: usize,
    function_locals: usize,
    blocks: &mut usize,
    items: &mut usize,
    mut statements: impl FnMut(usize) -> usize,
    budget: &mut Budget<'_>,
) -> CallResult<()> {
    budget.charge_work(1)?;
    let next_blocks = blocks
        .checked_add(function_blocks)
        .ok_or(Resource::Arithmetic)?;
    let next_items = items
        .checked_add(function_locals)
        .ok_or(Resource::Arithmetic)?;
    if next_blocks > 32 || next_items > 4096 {
        return refuse("finite source block/local/statement cap");
    }
    *blocks = next_blocks;
    *items = next_items;
    for index in 0..function_blocks {
        budget.charge_work(1)?;
        *items = items
            .checked_add(statements(index))
            .ok_or(Resource::Arithmetic)?;
        if *items > 4096 {
            return refuse("finite source block/local/statement cap");
        }
    }
    Ok(())
}
fn bounded(owner: &ProductionSemanticSsaOwnerV1, budget: &mut Budget<'_>) -> CallResult<()> {
    let source = owner.source_semantic();
    if source.functions().len() != 2
        || source.roots().len() != 1
        || source.types().len() > 4096
        || source.callables().len() > 4096
    {
        return refuse("two-function finite source roster required");
    }
    let mut blocks = 0usize;
    let mut storage = 0usize;
    for (index, function) in source.functions().iter().enumerate() {
        scan_bounded_lengths(
            function.blocks().len(),
            function.locals().len(),
            &mut blocks,
            &mut storage,
            |index| function.blocks()[index].statements().len(),
            budget,
        )?;
        let rows = rows(owner, SemanticFunctionIdV1::from_index(index as u32))?;
        if rows.events().len() > 32768
            || rows.edge_definitions().len() > 4096
            || rows.entry_definitions().len() > 4096
            || rows.successors().len() > 128
            || rows.elisions().len() > 4096
        {
            return refuse("finite source occurrence cap");
        }
        inherited(super::super::source::reject_source_cycle_edges(
            function.blocks().len(),
            rows.successors().len(),
            |i| {
                let row = rows.successors().get(i)?;
                Some((
                    row.id().source().get() as usize,
                    row.edge().target().index() as usize,
                ))
            },
            budget,
        ))?;
    }
    Ok(())
}
fn helper_abi(
    owner: &ProductionSemanticSsaOwnerV1,
    relation: &Relation,
    budget: &mut Budget<'_>,
) -> CallResult<()> {
    budget.charge_work(64)?;
    let source = owner.source_semantic();
    let helper = &source.functions()[relation.helper.index() as usize];
    let abi = helper.abi();
    use SemanticSourceArgumentOwnershipV1::{ByValue, SharedBorrow};
    if abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.can_unwind()
        || abi.c_variadic()
        || abi.fixed_count() != 4
        || abi.arguments().len() != 4
        || !abi.hidden_arguments().is_empty()
        || abi.source_input_types().len() != 4
        || abi.source_argument_ownership() != [SharedBorrow, ByValue, ByValue, ByValue]
        || abi.return_value().adjusted().is_some()
        || abi.return_value().pointee_override().is_some()
    {
        return refuse("helper ABI/source ownership outside exact nominal profile");
    }
    let call = call_at(owner, relation.root, relation.call)
        .ok_or(CallError::Unavailable("source call"))?;
    let mfma = relation.producers[CallRole::Result.index()];
    let mfma_call =
        call_at(owner, mfma.function, mfma.block).ok_or(CallError::Unavailable("MFMA call"))?;
    let values = relation.producers[CallRole::Values.index()];
    let values_call = call_at(owner, values.function, values.block)
        .ok_or(CallError::Unavailable("values call"))?;
    if call.arguments().len() != 4
        || mfma_call.arguments().len() != 4
        || values_call.arguments().len() != 1
    {
        return refuse("nominal source arity");
    }
    for (i, argument) in abi.arguments().iter().enumerate() {
        if argument.role() != SemanticAbiArgumentRoleV1::Source
            || argument.value().adjusted().is_some()
            || argument.value().pointee_override().is_some()
            || argument.ty() != abi.source_input_types()[i]
            || call.arguments()[i].ty() != argument.ty()
            || mfma_call.arguments()[i].ty() != argument.ty()
        {
            return refuse("actual call/formal/terminal ABI type mismatch");
        }
    }
    for (i, role) in [CallRole::Lhs, CallRole::Rhs, CallRole::Zero]
        .into_iter()
        .enumerate()
    {
        let row = relation.producers[role.index()];
        let ty = call_at(owner, row.function, row.block)
            .and_then(|c| c.destination())
            .ok_or(CallError::Unavailable("nominal producer output"))?
            .place()
            .ty();
        if ty != abi.source_input_types()[i + 1] {
            return refuse("producer/formal nominal type mismatch");
        }
    }
    let mfma_ty = mfma_call
        .destination()
        .ok_or(CallError::Unavailable("MFMA output"))?
        .place()
        .ty();
    let values_ty = values_call
        .destination()
        .ok_or(CallError::Unavailable("values output"))?
        .place()
        .ty();
    if values_call.arguments()[0].ty() != mfma_ty
        || abi.source_output_type() != values_ty
        || call
            .destination()
            .ok_or(CallError::Unavailable("source call destination"))?
            .place()
            .ty()
            != values_ty
    {
        return refuse("actual conversion/return/call output mismatch");
    }
    array_type(owner, values_ty)
}
pub(super) fn derive(
    owner: &ProductionSemanticSsaOwnerV1,
    budget: &mut Budget<'_>,
) -> CallResult<Relation> {
    bounded(owner, budget)?;
    let source = owner.source_semantic();
    let root = source.roots()[0];
    let root_decl = source
        .functions()
        .get(root.index() as usize)
        .ok_or(CallError::Unavailable("root coordinate"))?;
    if root_decl.role() != SemanticFunctionRoleV1::KernelRoot || root_decl.kernel_entry().is_none()
    {
        return refuse("actual kernel root required");
    }
    let mut defined = None;
    let mut producers = [None; 7];
    for (fi, function) in source.functions().iter().enumerate() {
        let fid = SemanticFunctionIdV1::from_index(fi as u32);
        for (bi, block) in function.blocks().iter().enumerate() {
            budget.charge_work(1)?;
            match block.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => {
                    let declaration = source
                        .callables()
                        .get(call.callee().index() as usize)
                        .ok_or(CallError::Unavailable("callable coordinate"))?;
                    match declaration {
                        SemanticCallableDeclV1::Defined { function } => {
                            if fid != root || *function == root {
                                return refuse("nested/recursive nominal call unavailable");
                            }
                            one(
                                &mut defined,
                                (*function, SemanticBlockIdV1::from_index(bi as u32)),
                            )?;
                        }
                        SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } => {
                            if let Some(role) = role(operation)? {
                                let arity = match role {
                                    CallRole::Context | CallRole::Lane => 0,
                                    CallRole::Zero | CallRole::Values => 1,
                                    _ => 4,
                                };
                                if call.arguments().len() != arity {
                                    return refuse("selected source arity");
                                }
                                if (role.index() < 5) != (fid == root) {
                                    return refuse("nominal producer belongs to wrong function");
                                }
                                one(
                                    &mut producers[role.index()],
                                    result(
                                        owner,
                                        fid,
                                        SemanticBlockIdV1::from_index(bi as u32),
                                        budget,
                                    )?,
                                )?;
                            } else if fid != root {
                                return refuse("extra helper intrinsic");
                            }
                        }
                        _ => return refuse("foreign helper/source call unavailable"),
                    }
                }
                SemanticTerminatorKindV1::TailCall(_) => return refuse("tail call unavailable"),
                _ => {}
            }
        }
    }
    let (helper, call) =
        defined.ok_or(CallError::Unavailable("one actual Defined call required"))?;
    let helper_decl = source
        .functions()
        .get(helper.index() as usize)
        .ok_or(CallError::Unavailable("helper coordinate"))?;
    if helper_decl.role() != SemanticFunctionRoleV1::InternalHelper
        || helper_decl.export().is_some()
    {
        return refuse("one internal unexported helper required");
    }
    if producers.iter().any(Option::is_none) {
        return refuse("seven exact nominal producers required");
    }
    let producers = producers.map(|p| p.expect("checked complete producer roster"));
    if producers.iter().skip(5).any(|p| p.function != helper) {
        return refuse("helper producer identity");
    }
    let root_rows = rows(owner, root)?;
    let actual_call = call_at(owner, root, call).ok_or(CallError::Unavailable("source call"))?;
    if actual_call.arguments().len() != 4
        || !matches!(actual_call.unwind(), SemanticUnwindActionV1::Unreachable)
        || !actual_call
            .destination()
            .ok_or(CallError::Unavailable("source call destination"))?
            .place()
            .projections()
            .is_empty()
    {
        return refuse("source call shape");
    }
    let mut args = [None; 4];
    for (i, operand) in actual_call.arguments().iter().enumerate() {
        if !whole(operand) || (i != 0 && !matches!(operand, SemanticOperandV1::Move(_))) {
            return refuse("nominal arguments require actual whole-value moves");
        }
        args[i] = Some(base_use(
            &root_rows,
            site(call),
            OperandRole::CallArgument(i as u32),
            budget,
        )?);
    }
    let helper_rows = rows(owner, helper)?;
    let mut formals = [None; 4];
    for entry in helper_rows.entry_definitions() {
        budget.charge_work(1)?;
        let EntryOrigin::Argument(index) = entry.origin() else {
            return refuse("non-positional helper entry");
        };
        let Some(slot) = formals.get_mut(index as usize) else {
            return refuse("extra helper entry");
        };
        let local = helper_decl
            .locals()
            .get(entry.variable().get() as usize)
            .ok_or(CallError::Unavailable("entry local"))?;
        if local.role() != SemanticLocalRoleV1::Argument(index)
            || local.ty()
                != *helper_decl
                    .abi()
                    .source_input_types()
                    .get(index as usize)
                    .ok_or(CallError::Unavailable("entry ABI index"))?
        {
            return refuse("entry formal source type");
        }
        one(
            slot,
            entry
                .value()
                .ok_or(CallError::Unavailable("nominal formal not promoted"))?,
        )?;
    }
    if formals.iter().any(Option::is_none) {
        return refuse("four actual entry SSA definitions required");
    }
    let mut relation = Relation {
        root,
        helper,
        call,
        producers,
        arguments: args.map(|v| v.expect("checked call arguments")),
        formals: formals.map(|v| v.expect("checked helper entries")),
        permutation: [0; 4],
    };
    helper_abi(owner, &relation, budget)?;
    ssa::check(owner, &mut relation, budget)?;
    Ok(relation)
}

#[cfg(test)]
pub(super) fn cycle_control(
    count: usize,
    edges: &[(usize, usize)],
    budget: &mut Budget<'_>,
) -> CallResult<()> {
    inherited(super::super::source::reject_source_cycle_edges(
        count,
        edges.len(),
        |i| edges.get(i).copied(),
        budget,
    ))
}
