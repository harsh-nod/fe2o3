// A completed source enum guard is not payload, issuer or live-loan authority.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticEnumVariantV1, SemanticSwitchTargetsV1};

pub(super) type Key = (SsaValueV1, SemanticTypeIdV1, u32, u32);

#[derive(Default)]
pub(super) struct Memo<'a> {
    pub(super) owner: Option<(
        &'a SemanticFunctionDeclV1,
        &'a SsaConstructionPlanV1,
        &'a [SemanticTypeDeclV1],
    )>,
    pub(super) rows: BTreeMap<Key, bool>,
}

fn canonical_whole_value(
    graph: &mut CapabilitySsaGraphV1<'_>,
    mut value: SsaValueV1,
    ty: SemanticTypeIdV1,
) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
    // Only whole-value Copy/Move aliases are transparent. A nontrivial Phi is
    // its own SSA identity. Cyclic aliases spend the same finite Graph budget.
    loop {
        graph.charge(1)?;
        if matches!(value, SsaValueV1::BlockArgument { .. }) {
            return Ok(value);
        }
        graph.charge(graph.ssa.entry_definitions().len())?;
        if graph
            .ssa
            .entry_definitions()
            .iter()
            .any(|entry| entry.value() == value)
        {
            return Ok(value);
        }
        let site = graph.definition(value)?;
        let Some(statement) = site.statement else {
            return Ok(value);
        };
        let body = graph.body;
        let SemanticStatementKindV1::Assign(assignment) =
            body.blocks()[site.block as usize].statements()[statement as usize].kind()
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        if assignment.destination().ty() != ty || assignment.value().result_type() != ty {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let SemanticRvalueKindV1::Use(
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
        ) = assignment.value().kind()
        else {
            return Ok(value);
        };
        if place.ty() != ty || !place.projections().is_empty() {
            return Ok(value);
        }
        value = graph.use_value(site.block, place.local().index())?;
    }
}

pub(super) fn selected_variant(
    graph: &mut CapabilitySsaGraphV1<'_>,
    types: &[SemanticTypeDeclV1],
    value: SsaValueV1,
    enum_type: SemanticTypeIdV1,
    variant: u32,
    use_block: u32,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let Some(SemanticTypeShapeV1::Enum {
        discriminant: tag_type,
        variants,
    }) = types
        .get(enum_type.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Ok(false);
    };
    if !variants
        .get(variant as usize)
        .is_some_and(|v| !v.is_uninhabited())
    {
        return Ok(false);
    }
    let value = canonical_whole_value(graph, value, enum_type)?;
    let body = graph.body;
    graph.charge(body.blocks().len())?;
    for (block_index, block) in body.blocks().iter().enumerate() {
        let block_index = u32::try_from(block_index)
            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if !graph.ssa.is_reachable(SsaBlockIdV1::new(block_index)) {
            continue;
        }
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = block.terminator().kind()
        else {
            continue;
        };
        let (SemanticOperandV1::Copy(tag) | SemanticOperandV1::Move(tag)) = discriminant else {
            continue;
        };
        if !tag.projections().is_empty() || tag.ty() != *tag_type {
            continue;
        }
        graph.charge(block.statements().len())?;
        let mut candidate = None;
        for (statement_index, statement) in block.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if assignment.destination().local() != tag.local() {
                continue;
            }
            if assignment.destination().ty() != *tag_type
                || !assignment.destination().projections().is_empty()
                || assignment.value().result_type() != *tag_type
            {
                continue;
            }
            let SemanticRvalueKindV1::Discriminant(place) = assignment.value().kind() else {
                continue;
            };
            if place.ty() != enum_type || !place.projections().is_empty() {
                continue;
            }
            if candidate.replace((statement_index, place)).is_some() {
                return Ok(false);
            }
        }
        let Some((statement, place)) = candidate else {
            continue;
        };
        let tag_value = graph.use_value(block_index, tag.local().index())?;
        let tag_site = graph.definition(tag_value)?;
        if tag_site.block != block_index
            || tag_site.statement != Some(statement as u32)
            || tag_site.local != tag.local().index()
        {
            continue;
        }
        let tested = graph.use_value(block_index, place.local().index())?;
        if canonical_whole_value(graph, tested, enum_type)? != value {
            continue;
        }
        if let Some(ordinal) = selecting_edge(graph, variants, targets, variant)?
            && edge_dominates_use(graph, block_index, ordinal, use_block)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn target_index(
    graph: &mut CapabilitySsaGraphV1<'_>,
    targets: &SemanticSwitchTargetsV1,
    value: u128,
) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
    let (mut lo, mut hi) = (0, targets.values().len());
    while lo < hi {
        graph.charge(1)?;
        let mid = lo + (hi - lo) / 2;
        if targets.values()[mid].value() < value {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    graph.charge(1)?;
    Ok(targets
        .values()
        .get(lo)
        .filter(|target| target.value() == value)
        .map(|_| lo))
}

fn selecting_edge(
    graph: &mut CapabilitySsaGraphV1<'_>,
    variants: &[SemanticEnumVariantV1],
    targets: &SemanticSwitchTargetsV1,
    variant: u32,
) -> Result<Option<u32>, ProductionSemanticKirErrorV1> {
    let Some(expected) = variants.get(variant as usize) else {
        return Ok(None);
    };
    if let Some(index) = target_index(graph, targets, expected.discriminant())? {
        return u32::try_from(index)
            .map(Some)
            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    graph.charge(variants.len())?;
    for (index, other) in variants.iter().enumerate() {
        if index != variant as usize
            && !other.is_uninhabited()
            && target_index(graph, targets, other.discriminant())?.is_none()
        {
            return Ok(None);
        }
    }
    u32::try_from(targets.values().len())
        .map(Some)
        .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)
}

fn edge_dominates_use(
    graph: &mut CapabilitySsaGraphV1<'_>,
    source: u32,
    ordinal: u32,
    target: u32,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    if !graph.ssa.is_reachable(SsaBlockIdV1::new(target)) {
        return Ok(false);
    }
    let body = graph.body;
    let count = body.blocks().len();
    let word = std::mem::size_of::<usize>();
    let pending_words = count
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
        .div_ceil(word);
    let seen_words = count.div_ceil(word);
    let header_words =
        (std::mem::size_of::<Vec<u32>>() + std::mem::size_of::<Vec<u8>>()).div_ceil(word);
    graph.charge(
        pending_words
            .checked_add(seen_words)
            .and_then(|n| n.checked_add(header_words))
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
    )?;
    let mut seen = Vec::new();
    let mut pending = Vec::new();
    seen.try_reserve_exact(count)
        .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    pending
        .try_reserve_exact(count)
        .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if seen.capacity() != count || pending.capacity() != count {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    graph.charge(count)?;
    seen.resize(count, 0u8);
    let entry = body.entry().index();
    seen[entry as usize] = 1;
    pending.push(entry);
    let mut cursor = 0;
    while cursor < pending.len() {
        graph.charge(1)?;
        let block = pending[cursor];
        cursor += 1;
        if block == target {
            return Ok(false);
        }
        let mut edge_index = 0u32;
        body.blocks()[block as usize]
            .terminator()
            .kind()
            .try_for_each_edge::<ProductionSemanticKirErrorV1>(|edge| {
                graph.charge(1)?;
                let skipped = block == source && edge_index == ordinal;
                edge_index = edge_index
                    .checked_add(1)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                if skipped {
                    return Ok(());
                }
                let next = edge.target().index();
                let mark = seen
                    .get_mut(next as usize)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                if *mark == 0 {
                    *mark = 1;
                    pending.push(next);
                }
                Ok(())
            })?;
    }
    Ok(true)
}
