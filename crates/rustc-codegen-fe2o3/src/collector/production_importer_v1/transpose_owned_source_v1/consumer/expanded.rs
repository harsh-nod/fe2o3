use super::*;

pub(super) fn call_block(
    view: &SemanticExpandedRootV1,
    instance: SemanticCallInstanceIdV1,
    source: (SemanticFunctionIdV1, SemanticBlockIdV1),
    defined: Option<SemanticFunctionIdV1>,
    work: &mut usize,
) -> PlanResult<(SemanticBlockIdV1, Option<SemanticCallInstanceIdV1>)> {
    let mut found = None;
    for (index, origin) in view.block_origins().iter().enumerate() {
        bounded::charge(work, 1)?;
        if origin.instance() != instance || (origin.function(), origin.block()) != source {
            continue;
        }
        let callee = match (defined, origin.terminator()) {
            (None, TerminatorOrigin::Source) => None,
            (Some(function), TerminatorOrigin::CallEntry { callee }) => {
                let child = view
                    .instances()
                    .get(callee.index() as usize)
                    .ok_or(Error::Source(
                        "transpose call instance is outside expansion",
                    ))?;
                if child.parent() != Some(instance)
                    || child.function() != function
                    || child.call_block() != Some(source.1)
                {
                    return Err(Error::Source(
                        "transpose call changed its exact parent/function/site",
                    )
                    .into());
                }
                Some(callee)
            }
            _ => continue,
        };
        let block = SemanticBlockIdV1::from_index(u32::try_from(index).map_err(|_| Error::Work)?);
        if found.replace((block, callee)).is_some() {
            return Err(
                Error::Source("transpose call has duplicate expanded source attribution").into(),
            );
        }
    }
    found
        .ok_or_else(|| Error::Source("transpose original call lost its expanded occurrence").into())
}

pub(super) fn source_statement(
    view: &SemanticExpandedRootV1,
    instance: SemanticCallInstanceIdV1,
    source: (SemanticFunctionIdV1, SemanticBlockIdV1, u32),
    work: &mut usize,
) -> PlanResult<QuerySite> {
    let mut found = None;
    for (block, origin) in view.block_origins().iter().enumerate() {
        bounded::charge(work, 1)?;
        if origin.instance() != instance
            || origin.function() != source.0
            || origin.block() != source.1
        {
            continue;
        }
        for (statement, kind) in origin.statements().iter().enumerate() {
            bounded::charge(work, 1)?;
            if *kind
                != (StatementOrigin::Source {
                    statement: source.2,
                })
            {
                continue;
            }
            let site = QuerySite::new(
                SemanticBlockIdV1::from_index(u32::try_from(block).map_err(|_| Error::Work)?),
                Some(u32::try_from(statement).map_err(|_| Error::Work)?),
            );
            if found.replace(site).is_some() {
                return Err(Error::Source(
                    "transpose source statement has duplicate expansion attribution",
                )
                .into());
            }
        }
    }
    found.ok_or_else(|| {
        Error::Source("transpose source statement lost expansion attribution").into()
    })
}

pub(super) fn transfer_statement(
    view: &SemanticExpandedRootV1,
    parent: SemanticCallInstanceIdV1,
    callee: SemanticCallInstanceIdV1,
    source: (SemanticFunctionIdV1, SemanticBlockIdV1),
    work: &mut usize,
) -> PlanResult<QuerySite> {
    bounded::charge(work, 2)?;
    let parent_instance = view
        .instances()
        .get(parent.index() as usize)
        .ok_or(Error::Source(
            "transpose return parent is outside expansion",
        ))?;
    let child = view
        .instances()
        .get(callee.index() as usize)
        .ok_or(Error::Source(
            "transpose return callee is outside expansion",
        ))?;
    if parent_instance.function() != source.0
        || child.parent() != Some(parent)
        || child.call_block() != Some(source.1)
    {
        return Err(
            Error::Source("transpose return changed its exact caller instance/site").into(),
        );
    }
    let mut found = None;
    for (block, origin) in view.block_origins().iter().enumerate() {
        bounded::charge(work, 1)?;
        // The ReturnTransfer belongs to the callee's original Return block.
        // Its call instance, not a substituted caller block, ties it to source.
        if origin.instance() != callee
            || origin.function() != child.function()
            || origin.terminator() != (TerminatorOrigin::CallReturn { callee })
        {
            continue;
        }
        for (statement, kind) in origin.statements().iter().enumerate() {
            bounded::charge(work, 1)?;
            if *kind != (StatementOrigin::ReturnTransfer { callee }) {
                continue;
            }
            let site = QuerySite::new(
                SemanticBlockIdV1::from_index(u32::try_from(block).map_err(|_| Error::Work)?),
                Some(u32::try_from(statement).map_err(|_| Error::Work)?),
            );
            if found.replace(site).is_some() {
                return Err(
                    Error::Source("transpose return has duplicate expanded transfer").into(),
                );
            }
        }
    }
    found.ok_or_else(|| Error::Source("transpose exact original return transfer is absent").into())
}

pub(super) fn borrow_operand<'a>(
    view: &'a SemanticExpandedRootV1,
    instance: SemanticCallInstanceIdV1,
    source: (SemanticFunctionIdV1, SemanticBlockIdV1),
    work: &mut usize,
) -> PlanResult<(QuerySite, &'a SemanticOperandV1)> {
    argument_operand(view, instance, source, 0, 1, work)
}

pub(super) fn argument_operand<'a>(
    view: &'a SemanticExpandedRootV1,
    instance: SemanticCallInstanceIdV1,
    source: (SemanticFunctionIdV1, SemanticBlockIdV1),
    argument: u32,
    arity: usize,
    work: &mut usize,
) -> PlanResult<(QuerySite, &'a SemanticOperandV1)> {
    if argument as usize >= arity {
        return Err(Error::Source("source argument is outside exact call arity").into());
    }
    let mut found = None;
    for (block, origin) in view.block_origins().iter().enumerate() {
        bounded::charge(work, 1)?;
        if origin.instance() != instance || (origin.function(), origin.block()) != source {
            continue;
        }
        let block_id =
            SemanticBlockIdV1::from_index(u32::try_from(block).map_err(|_| Error::Work)?);
        match origin.terminator() {
            TerminatorOrigin::Source => {
                let operands = call(view, block_id)?.arguments();
                if operands.len() != arity {
                    return Err(Error::Source("Workgroup shared source arity changed").into());
                }
                let operand = &operands[argument as usize];
                if found
                    .replace((QuerySite::new(block_id, None), operand))
                    .is_some()
                {
                    return Err(Error::Source("Workgroup borrow source was expanded twice").into());
                }
            }
            TerminatorOrigin::CallEntry { callee } => {
                for (statement, kind) in origin.statements().iter().enumerate() {
                    bounded::charge(work, 1)?;
                    if *kind
                        != (StatementOrigin::ParameterTransfer {
                            callee,
                            argument,
                        })
                    {
                        continue;
                    }
                    let site = QuerySite::new(
                        block_id,
                        Some(u32::try_from(statement).map_err(|_| Error::Work)?),
                    );
                    let SemanticRvalueKindV1::Use(operand) = assignment(view, site)?.value().kind()
                    else {
                        return Err(Error::Source(
                            "Workgroup shared parameter is not original source transfer",
                        )
                        .into());
                    };
                    if found.replace((site, operand)).is_some() {
                        return Err(Error::Source(
                            "Workgroup shared parameter has duplicate transfer",
                        )
                        .into());
                    }
                }
            }
            TerminatorOrigin::CallReturn { .. } => {}
        }
    }
    found
        .ok_or_else(|| Error::Source("Workgroup shared use lost its exact expanded operand").into())
}
