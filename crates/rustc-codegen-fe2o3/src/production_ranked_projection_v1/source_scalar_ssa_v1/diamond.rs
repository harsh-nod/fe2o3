use super::*;

pub(super) struct SelectionV1<'a> {
    pub(super) site: Site,
    pub(super) condition: &'a SemanticOperandV1,
    pub(super) when_true: ProductionSemanticSsaValueV1<'a>,
    pub(super) when_false: ProductionSemanticSsaValueV1<'a>,
}

fn tick(charge: &mut impl FnMut() -> bool) -> Result<(), &'static str> {
    charge()
        .then_some(())
        .ok_or("scalar SSA branch correspondence exhausted its shared budget")
}

pub(super) fn two_way<'a>(
    query: &ProductionSemanticSsaSourceQueryV1<'a>,
    incoming: &ProductionSemanticSsaIncomingValuesV1<'a>,
    charge: &mut impl FnMut() -> bool,
) -> Result<SelectionV1<'a>, &'static str> {
    tick(charge)?;
    if !incoming.belongs_to(query)
        || incoming.external_entry().is_some()
        || incoming.edge_count() != 2
    {
        return Err(CLOSED);
    }
    let (first, a) = incoming
        .edge(0, charge)
        .map_err(|_| CUSTODY)?
        .ok_or(CUSTODY)?;
    let (second, b) = incoming
        .edge(1, charge)
        .map_err(|_| CUSTODY)?
        .ok_or(CUSTODY)?;
    if first.id().source() == second.id().source() {
        return Err(CLOSED);
    }
    for (edge, value) in [(first, a), (second, b)] {
        tick(charge)?;
        let body = query
            .function()
            .blocks()
            .get(edge.id().source().get() as usize)
            .ok_or(CUSTODY)?;
        if edge.id().ordinal() != 0
            || edge.target() != incoming.block()
            || !matches!(body.terminator().kind(),SemanticTerminatorKindV1::Goto(target) if target.target().index()==incoming.block().get())
        {
            return Err(CLOSED);
        }
        // Start with branch-local scalar definitions only. Common definitions,
        // loop transport and conceptual call-return definitions are not guessed.
        if !matches!(query.value_origin(&value,charge).map_err(|_|CUSTODY)?,
            ProductionSemanticSsaValueOriginV1::Event{block,..} if block==edge.id().source())
        {
            return Err(CLOSED);
        }
    }
    let (a_parent, a_entry) = arm_parent(query, first.id().source(), incoming.block(), charge)?;
    let (b_parent, b_entry) = arm_parent(query, second.id().source(), incoming.block(), charge)?;
    if a_parent != b_parent || a_entry == b_entry {
        return Err(CLOSED);
    }
    let SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets,
    } = query.function().blocks()[a_parent.get() as usize]
        .terminator()
        .kind()
    else {
        return Err(CLOSED);
    };
    tick(charge)?;
    let (zero, one) = match targets.values() {
        [target] if target.value() <= 1 => {
            let named = target.edge().target().index();
            let other = targets.otherwise().target().index();
            if target.value() == 0 {
                (named, other)
            } else {
                (other, named)
            }
        }
        [x, y] if x.value() != y.value() && x.value() <= 1 && y.value() <= 1 => {
            let fallback = query
                .function()
                .blocks()
                .get(targets.otherwise().target().index() as usize)
                .ok_or(CUSTODY)?;
            if !fallback.statements().is_empty()
                || !matches!(
                    fallback.terminator().kind(),
                    SemanticTerminatorKindV1::Unreachable
                )
            {
                return Err(CLOSED);
            }
            if x.value() == 0 {
                (x.edge().target().index(), y.edge().target().index())
            } else {
                (y.edge().target().index(), x.edge().target().index())
            }
        }
        _ => return Err(CLOSED),
    };
    let site = Site::new(SemanticBlockIdV1::from_index(a_parent.get()), None);
    if (zero, one) == (a_entry.get(), b_entry.get()) {
        Ok(SelectionV1 {
            site,
            condition: discriminant,
            when_true: b,
            when_false: a,
        })
    } else if (zero, one) == (b_entry.get(), a_entry.get()) {
        Ok(SelectionV1 {
            site,
            condition: discriminant,
            when_true: a,
            when_false: b,
        })
    } else {
        Err(CLOSED)
    }
}

fn arm_parent(
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    mut block: SsaBlockIdV1,
    merge: SsaBlockIdV1,
    charge: &mut impl FnMut() -> bool,
) -> Result<(SsaBlockIdV1, SsaBlockIdV1), &'static str> {
    for _ in 0..fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        tick(charge)?;
        if block == merge
            || block.get() == query.function().entry().index()
            || query
                .predecessor_count(block, charge)
                .map_err(|_| CUSTODY)?
                != 1
        {
            return Err(CLOSED);
        }
        let edge = query
            .predecessor(block, 0, charge)
            .map_err(|_| CUSTODY)?
            .ok_or(CUSTODY)?;
        if edge.target() != block {
            return Err(CUSTODY);
        }
        let parent = edge.id().source();
        let body = query
            .function()
            .blocks()
            .get(parent.get() as usize)
            .ok_or(CUSTODY)?;
        match body.terminator().kind() {
            SemanticTerminatorKindV1::SwitchInt { .. } => return Ok((parent, block)),
            SemanticTerminatorKindV1::Goto(target)
                if edge.id().ordinal() == 0 && target.target().index() == block.get() =>
            {
                for statement in body.statements() {
                    tick(charge)?;
                    if !matches!(
                        statement.kind(),
                        SemanticStatementKindV1::Nop
                            | SemanticStatementKindV1::StorageLive(_)
                            | SemanticStatementKindV1::StorageDead(_)
                    ) {
                        return Err(CLOSED);
                    }
                }
                block = parent;
            }
            _ => return Err(CLOSED),
        }
    }
    Err("scalar SSA branch correspondence exceeds its bounded depth")
}
