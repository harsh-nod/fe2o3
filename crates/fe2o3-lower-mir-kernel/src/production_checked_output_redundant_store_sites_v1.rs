use super::*;
use fe2o3_kernel_ir::CanonicalKirOperationTransitionV1;

type Site = Option<(SemanticFunctionIdV1, SemanticBlockIdV1, u32)>;

fn retained_sites(
    input: &CanonicalKirInventoryV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    source: &[Site],
    rows: &[CanonicalKirOperationTransitionV1],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Vec<Site>> {
    charge(budget, 3)?;
    if source.len() != input.operations().len() || rows.len() != output.operations().len() {
        return Err(refused(
            "redundant Store source",
            "total origin and source-site rosters",
        ));
    }
    let mut result = scratch::<Site>(rows.len(), budget)?;
    for (row, actual) in rows.iter().zip(output.operations()) {
        charge(budget, 5)?;
        if row.output != actual.coordinate {
            return Err(refused(
                "redundant Store source",
                "exact retained operation coordinate",
            ));
        }
        result.push(match row.origin {
            CanonicalKirOperationOriginV1::Retained(origin) => {
                source[operation_ordinal(input, origin)?]
            }
            CanonicalKirOperationOriginV1::ConstantFrom(_) => None,
        });
    }
    Ok(result)
}

fn coordinate_sites(
    input: &CanonicalKirInventoryV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    sites: &[Site],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Vec<Site>> {
    charge(budget, 3)?;
    if input.operations().len() != output.operations().len()
        || sites.len() != input.operations().len()
    {
        return Err(refused(
            "redundant Store source",
            "coordinate-preserving forwarding roster",
        ));
    }
    let mut result = scratch::<Site>(sites.len(), budget)?;
    for ((input, output), site) in input
        .operations()
        .iter()
        .zip(output.operations())
        .zip(sites)
    {
        charge(budget, 3)?;
        if input.coordinate != output.coordinate {
            return Err(refused(
                "redundant Store source",
                "coordinate-preserving forwarding row",
            ));
        }
        result.push(*site);
    }
    Ok(result)
}

pub(super) fn check_store_sites(
    prefix: StorePrefix<'_>,
    deletion: StoreDeletionView<'_>,
    bound: &CanonicalKirInventoryV1<'_>,
    bound_sites: &[Site],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> StoreResult<Box<[FormalMemoryObligations]>> {
    let checked = prefix.checked();
    let p5 = checked.intermediate_policy5();
    let p3 = p5.intermediate_policy4().intermediate_policy3();
    let (canonical, storage) =
        CanonicalKirInventoryV1::derive(p3.owner(), budget).map_err(inventory_error)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (forwarded, storage) =
        CanonicalKirInventoryV1::derive(p5.owner(), budget).map_err(inventory_error)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (input, storage) =
        CanonicalKirInventoryV1::derive(prefix.output(), budget).map_err(inventory_error)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (output, storage) =
        CanonicalKirInventoryV1::derive(deletion.output(), budget).map_err(inventory_error)?;
    budget.reserve_storage(storage.retained_storage())?;
    let canonical_sites = retained_sites(
        bound,
        &canonical,
        bound_sites,
        p3.occurrences().candidate().operations,
        budget,
    )?;
    // The unchanged P4/P5 relation has already been fully replayed. Both
    // forwarding stages preserve operation coordinates, not operation values.
    let forwarded_sites = coordinate_sites(&canonical, &forwarded, &canonical_sites, budget)?;
    let input_sites = retained_sites(
        &forwarded,
        &input,
        &forwarded_sites,
        checked.continuation().occurrences().candidate().operations,
        budget,
    )?;
    let rows = deletion.retained_operations();
    if rows.len() != output.operations().len() {
        return Err(refused(
            "redundant Store source",
            "complete J retained-origin roster",
        )
        .into());
    }
    let mut output_sites = scratch::<Site>(rows.len(), budget)?;
    for (row, actual) in rows.iter().zip(output.operations()) {
        charge(budget, 5)?;
        if row.output != actual.coordinate {
            return Err(refused(
                "redundant Store source",
                "exact J retained operation coordinate",
            )
            .into());
        }
        output_sites.push(input_sites[operation_ordinal(&input, row.input)?]);
    }
    let source = prefix.source();
    private_memory::redundant_store_source_intervals_v1(
        source.semantic(),
        &input,
        deletion.rows(),
        &input_sites,
        budget,
    )?;
    let private = private_memory::check(&output, source.limits().max_operations, budget)?;
    private_memory::source_lifetimes_from_sites(
        source.semantic(),
        &private,
        &output_sites,
        budget,
    )?;
    let division = unsigned_division::check(&output, source.semantic().target(), budget)?;
    let helpers = scalar_helpers::check(&output, budget)?;
    census::native(
        &output,
        &private,
        &division,
        &helpers,
        "redundant Store J",
        |ordinal, coordinate| {
            let row = rows
                .get(ordinal)
                .ok_or_else(|| refused("J", "complete retained trap origin"))?;
            if row.output != coordinate {
                return Err(refused("J", "exact trap coordinate"));
            }
            let old = &input.operations()[operation_ordinal(&input, row.input)?];
            let new = &output.operations()[operation_ordinal(&output, coordinate)?];
            Ok(old.coordinate == row.input && old.operation == new.operation)
        },
        budget,
    )?;
    // The shared caller prepays these rows before any scoped erasure callback.
    // Borrowed checking discards them; closed consuming admission moves them.
    let reports = derive_checked_output_guarded_obligations_v1(
        deletion.output(),
        source.limits().max_operations,
    )
    .map_err(E::Formal)?;
    census::formal(&output, &private, &reports, budget)?;
    Ok(reports)
}

#[cfg(test)]
#[path = "production_checked_output_redundant_store_origins_v1_tests.rs"]
mod tests;
