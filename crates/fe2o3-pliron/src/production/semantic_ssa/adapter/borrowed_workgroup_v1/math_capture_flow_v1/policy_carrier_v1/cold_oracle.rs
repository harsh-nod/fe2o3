fn cold_walk(
    types: &[SemanticTypeDeclV1],
    pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    barriers: &BTreeSet<SemanticTypeIdV1>,
    ty: SemanticTypeIdV1,
    path: &mut [u32; MAX_FIELDS],
    depth: usize,
    nodes: &mut usize,
    found: &mut Option<Route>,
    budget: &mut Budget,
) -> Result<bool, ProductionSemanticSsaErrorV1> {
    if *nodes == MAX_SHAPE_NODES {
        return Ok(false);
    }
    budget.charge(1)?;
    *nodes += 1;
    if !barriers.is_empty() {
        budget.charge(1 + (usize::BITS - barriers.len().leading_zeros()) as usize)?;
        if barriers.contains(&ty) {
            return Ok(false);
        }
    }
    if let Some(&owned) = pairs.get(&ty) {
        return Ok(found
            .replace(Route {
                reference: ty,
                owned,
                fields: *path,
                len: depth,
            })
            .is_none());
    }
    let Some(fields) = fields(types, ty) else {
        return Ok(true);
    };
    if !fields.is_empty() && depth == MAX_FIELDS {
        return Ok(false);
    }
    for (index, ty) in fields.iter().enumerate() {
        path[depth] = index as u32;
        if !cold_walk(types, pairs, barriers, *ty, path, depth + 1, nodes, found, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}
