//! Fresh actual-L safety and complete source-site transport after tail replay.
use super::*;
type Site = Option<(SemanticFunctionIdV1, SemanticBlockIdV1, u32)>;

pub(super) fn check(
    prefix: &ProductionCheckedOutputOwnerPolicy6V1,
    tail: &Tail,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &Binding,
) -> CResult<Box<[FormalMemoryObligations]>> {
    let source = GeneralSourceContextV1::Direct(prefix.source_semantic_kir());
    let checked = prefix.checked_output();
    let p5 = checked.intermediate_policy5();
    let p3 = p5.intermediate_policy4().intermediate_policy3();
    let (bound, storage) =
        CanonicalKirInventoryV1::derive(prefix.bound(), budget).map_err(inventory_error)?;
    budget.reserve_storage(storage.retained_storage())?;
    let bound_sites =
        private_memory::source_statement_sites_v1(prefix.source_semantic_kir(), &bound, budget)?;
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
        CanonicalKirInventoryV1::derive(tail.output(), budget).map_err(inventory_error)?;
    budget.reserve_storage(storage.retained_storage())?;
    binding.check(budget)?;
    let canonical_sites = sites::retained_sites(
        &bound,
        &canonical,
        &bound_sites,
        p3.occurrences().candidate().operations,
        budget,
    )?;
    let forwarded_sites =
        sites::coordinate_sites(&canonical, &forwarded, &canonical_sites, budget)?;
    let integer_sites = sites::retained_sites(
        &forwarded,
        &input,
        &forwarded_sites,
        checked.continuation().occurrences().candidate().operations,
        budget,
    )?;
    // Private caller has replayed the complete exact permutation against its
    // actual retained I. Inert receipt coordinates alone are not authority.
    let rows = tail.transition_receipt().candidate().operations;
    let output_sites: Vec<Site> =
        sites::retained_sites(&input, &output, &integer_sites, rows, budget)?;
    let mut traps = scratch::<bool>(output.operations().len(), budget)?;
    for (row, actual) in rows.iter().zip(output.operations()) {
        budget.charge_work(12)?;
        let CanonicalKirOperationOriginV1::Retained(origin) = row.origin else {
            return Err(refused("local-order L", "exact retained permutation").into());
        };
        let old = input.operations()[operation_ordinal(&input, origin)?].operation;
        let new = actual.operation;
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
        ) = (&old.kind, &new.kind)
            && aa.is_empty()
            && ba.is_empty()
            && old.results.is_empty()
            && new.results.is_empty()
        {
            budget.charge_work(
                a.as_str()
                    .len()
                    .checked_add(b.as_str().len())
                    .and_then(|n| n.checked_add(2))
                    .ok_or(AssertOriginResourceV1::Arithmetic)?,
            )?;
            allowed = old == new;
        }
        traps.push(allowed);
    }
    let report_bytes = output
        .owner()
        .module()
        .kernels
        .len()
        .checked_mul(size_of::<FormalMemoryObligations>())
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    budget.reserve_storage(report_bytes)?;
    let private = private_memory::check(&output, source.limits().max_operations, budget)?;
    binding.check(budget)?;
    private_memory::source_lifetimes_from_sites(
        source.semantic(),
        &private,
        &output_sites,
        budget,
    )?;
    let division = unsigned_division::check(&output, source.semantic().target(), budget)?;
    let helpers = scalar_helpers::check(&output, budget)?;
    binding.check(budget)?;
    budget.charge_work(
        output
            .operations()
            .len()
            .checked_mul(3)
            .ok_or(AssertOriginResourceV1::Arithmetic)?,
    )?;
    census::native(
        &output,
        &private,
        &division,
        &helpers,
        "local-order L",
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
    census::formal(&output, &private, &reports, budget)?;
    binding.check(budget)?;
    Ok(reports)
}
