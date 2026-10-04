use super::*;
use fe2o3_kernel_analysis::CheckedCanonicalKirCommutativeBitwiseCseV1 as Relation;
type Site = Option<(SemanticFunctionIdV1, SemanticBlockIdV1, u32)>;

pub(super) fn check(
    prefix: Prefix<'_>,
    bound: &CanonicalKirInventoryV1<'_>,
    bound_sites: &[Site],
    relation: &Relation<'_, '_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &Binding,
) -> CResult<Box<[FormalMemoryObligations]>> {
    budget.charge_work(4)?;
    if !std::ptr::eq(prefix.output(), relation.input().owner()) {
        return Err(refused("commutative K", "exact retained J inventory").into());
    }
    let historical = prefix.historical();
    let checked = historical.checked();
    let p5 = checked.intermediate_policy5();
    let p3 = p5.intermediate_policy4().intermediate_policy3();
    let (canonical, storage) =
        CanonicalKirInventoryV1::derive(p3.owner(), budget).map_err(inventory_error)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (forwarded, storage) =
        CanonicalKirInventoryV1::derive(p5.owner(), budget).map_err(inventory_error)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (integer, storage) =
        CanonicalKirInventoryV1::derive(historical.output(), budget).map_err(inventory_error)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let canonical_sites = sites::retained_sites(
        bound,
        &canonical,
        bound_sites,
        p3.occurrences().candidate().operations,
        budget,
    )?;
    let forwarded_sites =
        sites::coordinate_sites(&canonical, &forwarded, &canonical_sites, budget)?;
    let integer_sites = sites::retained_sites(
        &forwarded,
        &integer,
        &forwarded_sites,
        checked.continuation().occurrences().candidate().operations,
        budget,
    )?;
    let input = relation.input();
    let output = relation.output();
    let mut j_sites = scratch::<Site>(input.operations().len(), budget)?;
    let origins = prefix.continuation().retained_operations();
    if origins.len() != input.operations().len() {
        return Err(refused("commutative K", "complete actual J source origins").into());
    }
    for (row, actual) in origins.iter().zip(input.operations()) {
        budget.charge_work(8)?;
        if row.output != actual.coordinate {
            return Err(refused("commutative K", "actual J origin coordinate").into());
        }
        j_sites.push(integer_sites[operation_ordinal(&integer, row.input)?]);
    }
    let rows = relation.rows().operations;
    let output_sites = sites::retained_sites(input, output, &j_sites, rows, budget)?;
    let traps = checked_traps(relation, budget)?;
    let source = historical.source();
    let private = private_memory::check(output, source.limits().max_operations, budget)?;
    binding.check(budget)?;
    private_memory::source_lifetimes_from_sites(
        source.semantic(),
        &private,
        &output_sites,
        budget,
    )?;
    binding.check(budget)?;
    let division = unsigned_division::check(output, source.semantic().target(), budget)?;
    binding.check(budget)?;
    let helpers = scalar_helpers::check(output, budget)?;
    binding.check(budget)?;
    // Every lookup below is prepaid before the census. The immutable checked
    // relation binds the entire retained operation and predecessor control/use
    // graph; the origin coordinate alone is never the semantic justification.
    budget.charge_work(
        output
            .operations()
            .len()
            .checked_mul(3)
            .ok_or(AssertOriginResourceV1::Arithmetic)?,
    )?;
    census::native(
        output,
        &private,
        &division,
        &helpers,
        "commutative K",
        |ordinal, coordinate| {
            Ok(traps.get(ordinal).copied().unwrap_or(false)
                && output
                    .operations()
                    .get(ordinal)
                    .is_some_and(|row| row.coordinate == coordinate))
        },
        budget,
    )?;
    binding.check(budget)?;
    let reports = derive_checked_output_guarded_obligations_v1(
        output.owner(),
        source.limits().max_operations,
    )
    .map_err(E::Formal)?;
    census::formal(output, &private, &reports, budget)?;
    binding.check(budget)?;
    Ok(reports)
}

// Called only with the privately constructed independent relation. Assertions
// lower to zero-argument calls; substituted predicate SSA lives in the retained
// predecessor operations/terminators, all checked by that complete relation.
fn checked_traps(
    relation: &Relation<'_, '_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> CResult<Vec<bool>> {
    let input = relation.input();
    let output = relation.output();
    let rows = relation.rows().operations;
    let mut result = scratch::<bool>(output.operations().len(), budget)?;
    if rows.len() != output.operations().len() {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    for (row, new) in rows.iter().zip(output.operations()) {
        budget.charge_work(10)?;
        if row.output != new.coordinate {
            return Err(AssertOriginResourceV1::Accounting.into());
        }
        let CanonicalKirOperationOriginV1::Retained(origin) = row.origin else {
            return Err(refused("commutative K", "only checked retained operations").into());
        };
        let old = &input.operations()[operation_ordinal(input, origin)?];
        let mut allowed = false;
        if let (
            OperationKind::Call {
                callee: a,
                arguments: aa,
            },
            OperationKind::Call {
                callee: b,
                arguments: ba,
            },
        ) = (&old.operation.kind, &new.operation.kind)
            && aa.is_empty()
            && ba.is_empty()
            && old.operation.results.is_empty()
            && new.operation.results.is_empty()
        {
            budget.charge_work(
                a.as_str()
                    .len()
                    .checked_add(b.as_str().len())
                    .and_then(|n| n.checked_add(2))
                    .ok_or(AssertOriginResourceV1::Arithmetic)?,
            )?;
            allowed = old.operation == new.operation;
        }
        result.push(allowed);
    }
    Ok(result)
}
