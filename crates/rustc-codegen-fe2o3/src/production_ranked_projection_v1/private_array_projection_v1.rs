// Annotate immediately after the actual operand projector call.

fn bind_private_array_projected_role_v1(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
    role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    sources: &mut [ProjectedAccessSourceV1],
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    facts.charge_private_array_work(4)?;
    let local = function
        .locals()
        .get(place.local().index() as usize)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "private array annotation local is absent",
        ))?;
    let declaration = types.get(local.ty().index() as usize).ok_or(
        ProductionRankedProjectionErrorV1::Unsupported("private array annotation type is absent"),
    )?;
    if !matches!(declaration.shape(), SemanticTypeShapeV1::Array { .. }) {
        return Ok(());
    }
    if sources.len() > 1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "one private array operand emitted multiple direct access rows",
        ));
    }
    if let Some(source) = sources.first_mut() {
        facts.charge_private_array_work(3)?;
        if source.memory_space == MemorySpaceAttr::Private
            && source.private_array_role.replace(role).is_some()
        {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "private array access was assigned more than one source role",
            ));
        }
    }
    Ok(())
}

fn bind_private_array_projected_operand_v1(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    operand: &SemanticOperandV1,
    role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    sources: &mut [ProjectedAccessSourceV1],
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    facts.charge_private_array_work(1)?;
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            bind_private_array_projected_role_v1(types, function, place, role, sources, facts)
        }
        SemanticOperandV1::Constant(_) => Ok(()),
    }
}

fn retain_materialized_private_array_source_v1(
    source: &ProjectedAccessSourceV1,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    let Some(role) = source.private_array_role else {
        return Ok(false);
    };
    facts.charge_private_array_work(4)?;
    let site = source
        .semantic_site
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "private array access has no exact source statement",
        ))?;
    let statement = site
        .statement
        .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
            "private array terminator access requires a separate correspondence relation",
        ))?;
    let block = u32::try_from(site.block).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "private array source block does not fit u32",
        )
    })?;
    let statement = u32::try_from(statement).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "private array source statement does not fit u32",
        )
    })?;
    facts.private_array_access(
        fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
            block: fe2o3_mir_model::SsaBlockIdV1::new(block),
            statement,
        },
        role,
    )
}

#[derive(Clone, Copy)]
struct CheckedPrivateArrayIndexV1 {
    array_local: SemanticLocalIdV1,
    index_local: SemanticLocalIdV1,
    value: u64,
}

#[allow(clippy::too_many_arguments)]
fn checked_private_array_statement_index_v1(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block: usize,
    statement: usize,
    place: &SemanticPlaceV1,
    role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    constants: &[Option<u64>],
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<Option<CheckedPrivateArrayIndexV1>, ProductionRankedProjectionErrorV1> {
    facts.charge_private_array_work(2)?;
    let [projection] = place.projections() else {
        return Ok(None);
    };
    let SemanticProjectionKindV1::Index(index) = projection.kind() else {
        return Ok(None);
    };
    facts.charge_private_array_work(2)?;
    if constants
        .get(index.index() as usize)
        .copied()
        .flatten()
        .is_some()
    {
        return Ok(None);
    }
    facts.charge_private_array_work(3)?;
    let local = function
        .locals()
        .get(place.local().index() as usize)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "private array index local is absent",
        ))?;
    let declaration = types.get(local.ty().index() as usize).ok_or(
        ProductionRankedProjectionErrorV1::Unsupported("private array index type is absent"),
    )?;
    if !matches!(declaration.shape(), SemanticTypeShapeV1::Array { .. }) {
        return Ok(None);
    }
    facts.charge_private_array_work(3)?;
    let block = u32::try_from(block).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported("private array index block exceeds u32")
    })?;
    let statement = u32::try_from(statement).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported("private array index statement exceeds u32")
    })?;
    let value = facts.private_array_constant_index(
        fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
            block: fe2o3_mir_model::SsaBlockIdV1::new(block),
            statement,
        },
        role,
    )?;
    // Result/handle construction and the later three fixed place/index guards.
    facts.charge_private_array_work(5)?;
    Ok(value.map(|value| CheckedPrivateArrayIndexV1 {
        array_local: place.local(),
        index_local: index,
        value,
    }))
}
