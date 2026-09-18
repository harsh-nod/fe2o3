// Included inside private_memory: one original-source kill implementation.
pub(super) fn redundant_store_source_intervals_v1(
    semantic: &AdmittedInertSemanticMirV1,
    input: &CanonicalKirInventoryV1<'_>,
    rows: &[fe2o3_kernel_analysis::CanonicalKirRedundantStoreRowV1],
    sites: &[Option<(SemanticFunctionIdV1, SemanticBlockIdV1, u32)>],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    charge(budget, 2)?;
    if sites.len() != input.operations().len() {
        return Err(refused(
            "redundant Store source",
            "complete input source-site roster",
        ));
    }
    if rows.is_empty() {
        return Ok(());
    }
    let kills = source_kill_index(semantic, budget)?;
    for row in rows {
        charge(budget, 8)?;
        let first = sites[operation_ordinal(input, row.anchor)?].ok_or_else(|| {
            refused(
                "redundant Store source",
                "anchor has exact source statement",
            )
        })?;
        let last = sites[operation_ordinal(input, row.removed)?].ok_or_else(|| {
            refused(
                "redundant Store source",
                "deleted Store has exact source statement",
            )
        })?;
        if (first.0, first.1) != (last.0, last.1) || first.2 > last.2 {
            return Err(refused(
                "redundant Store source",
                "same original block and ordered Store statements",
            ));
        }
        let local = redundant_store_destination_v1(semantic, first, budget)?;
        if redundant_store_destination_v1(semantic, last, budget)? != local {
            return Err(refused(
                "redundant Store source",
                "same direct scalar source local",
            ));
        }
        if source_interval_is_killed(
            &kills,
            SourceKillSite {
                function: first.0,
                block: first.1,
                local,
                statement: first.2,
            },
            last.2,
            budget,
        )? {
            return Err(refused(
                "redundant Store source",
                "no lifetime or Move invalidation between identical Stores",
            ));
        }
    }
    Ok(())
}

fn redundant_store_destination_v1(
    semantic: &AdmittedInertSemanticMirV1,
    site: (SemanticFunctionIdV1, SemanticBlockIdV1, u32),
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<SemanticLocalIdV1> {
    charge(budget, 8)?;
    let function = semantic
        .functions()
        .get(site.0.index() as usize)
        .ok_or_else(|| refused("redundant Store source", "source function exists"))?;
    let statement = function
        .blocks()
        .get(site.1.index() as usize)
        .and_then(|block| block.statements().get(site.2 as usize))
        .ok_or_else(|| refused("redundant Store source", "source Store statement exists"))?;
    let destination = match statement.kind() {
        SemanticStatementKindV1::Assign(value) => value.destination(),
        SemanticStatementKindV1::Store(value) => value.destination(),
        _ => {
            return Err(refused(
                "redundant Store source",
                "source assignment or Store",
            ));
        }
    };
    let declaration = function
        .locals()
        .get(destination.local().index() as usize)
        .and_then(|local| semantic.types().get(local.ty().index() as usize))
        .ok_or_else(|| refused("redundant Store source", "source storage declaration"))?;
    if !destination.projections().is_empty()
        || !matches!(declaration.shape(), SemanticTypeShapeV1::Scalar(_))
    {
        return Err(refused(
            "redundant Store source",
            "direct scalar local destination",
        ));
    }
    Ok(destination.local())
}
