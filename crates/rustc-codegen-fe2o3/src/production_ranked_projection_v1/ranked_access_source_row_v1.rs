//! One original per-access admission predicate shared by legacy and paid maps.
//! No source/graph authority is constructed by this helper.
use super::*;

pub(super) fn row(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    blocks: &[ProductionRankedBlockV1],
    source: &ProjectedAccessSourceV1,
    mut ordinal: impl FnMut(ProjectedSemanticAccessSiteV1) -> Option<u32>,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<Option<ProductionRankedAccessSourceV1>, ProductionRankedProjectionErrorV1> {
    let operation = blocks
        .get(source.block)
        .and_then(|block| block.operations().get(source.operation))
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "ranked access correspondence is outside the projected graph",
        ))?;
    if source.memory_space == MemorySpaceAttr::Private
        && matches!(
            operation,
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Read,
                ..
            }
        )
    {
        if !private_array_read_source_v1::retained_read(types, function, source, facts)? {
            return Ok(None);
        }
        if source
            .semantic_site
            .is_some_and(|site| ordinal(site).is_some())
        {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "private array read has duplicate source correspondence",
            ));
        }
    } else if !retained_ranked_access_source_v1(source.memory_space, operation) {
        if source.memory_space != MemorySpaceAttr::Private
            || !matches!(
                operation,
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Write,
                    ..
                }
            )
        {
            return Ok(None);
        }
        // Original fixed private-write predicate and its original work debit.
        facts.charge_private_array_work(48)?;
        let component = source.semantic_site.and_then(&mut ordinal).unwrap_or(0);
        if !private_array_write_source_v1(types, function, source, component) {
            return Ok(None);
        }
    }
    let site = source
        .semantic_site
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "ranked access correspondence has no exact semantic site",
        ))?;
    let mut retained = ProductionRankedAccessSourceV1::new(
        u32::try_from(site.block).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported("semantic access block does not fit u32")
        })?,
        site.statement.map(u32::try_from).transpose().map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "semantic access statement does not fit u32",
            )
        })?,
        ordinal(site).unwrap_or(0),
        u32::try_from(source.block).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported("ranked access block does not fit u32")
        })?,
        u32::try_from(source.operation).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "ranked access operation does not fit u32",
            )
        })?,
    );
    if let Some(extent) = source.output_extent {
        facts.charge_private_array_work(8)?;
        retained = retained.with_output_extent(extent);
    }
    Ok(Some(retained))
}
