use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirPrivateCellOriginKindV1 as PromotionOriginKind,
    CheckedCanonicalKirPrivateCellPromotionV1 as PromotionRelation,
};
type Site = Option<(SemanticFunctionIdV1, SemanticBlockIdV1, u32)>;

pub(super) struct PromotionMapping<'v, 'o, 'r> {
    pub(super) input: &'v CanonicalKirInventoryV1<'o>,
    pub(super) output: &'v CanonicalKirInventoryV1<'o>,
    pub(super) relation: &'v PromotionRelation<'r>,
}

// Only the complete replayed source/prefix/promotion path below constructs this
// borrowed metadata. It is not a safety report or a detached source attachment.
pub(super) struct CheckedPromotedSites<'s, 'g> {
    source: GeneralSourceContextV1<'s>,
    output: &'s CanonicalKirInventoryV1<'g>,
    output_sites: &'s [Site],
    traps: &'s [bool],
}

impl<'s, 'g> CheckedPromotedSites<'s, 'g> {
    pub(super) fn source(&self) -> GeneralSourceContextV1<'s> {
        self.source
    }
    pub(super) fn output(&self) -> &'s CanonicalKirInventoryV1<'g> {
        self.output
    }
    pub(super) fn statements(&self) -> &'s [Site] {
        self.output_sites
    }
    pub(super) fn traps(&self) -> &'s [bool] {
        self.traps
    }
}

pub(super) fn with_checked_sites<'g, 'w, R>(
    prefix: Prefix8<'_>,
    bound: &CanonicalKirInventoryV1<'_>,
    bound_sites: &[Site],
    mapping: PromotionMapping<'_, 'g, '_>,
    budget: &mut AssertOriginBudgetV1<'w>,
    binding: &PromotionBinding,
    use_sites: impl for<'s> FnOnce(
        CheckedPromotedSites<'s, 'g>,
        &mut AssertOriginBudgetV1<'w>,
        &PromotionBinding,
    ) -> PResult<R>,
) -> PResult<R> {
    budget.charge_work(4)?;
    let input = mapping.input;
    let output = mapping.output;
    if !std::ptr::eq(prefix.output(), input.owner())
        || !std::ptr::eq(mapping.relation.input(), input.owner())
        || !std::ptr::eq(mapping.relation.output(), output.owner())
    {
        return Err(refused(
            "private-cell promotion",
            "exact retained input/output owners",
        )
        .into());
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
    let (j, storage) = CanonicalKirInventoryV1::derive(prefix.previous().output(), budget)
        .map_err(inventory_error)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (commutative, storage) = prefix
        .continuation()
        .check_inventories_v1(&j, input, budget)
        .map_err(CError::Continuation)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    // The historical P8 mapping is reconstructed from its actual retained
    // source/prefix and complete checked relations, never from detached sites.
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
    let mut j_sites = scratch::<Site>(j.operations().len(), budget)?;
    let origins = prefix.previous().continuation().retained_operations();
    if origins.len() != j.operations().len() {
        return Err(refused("private-cell promotion", "complete actual J source origins").into());
    }
    for (row, actual) in origins.iter().zip(j.operations()) {
        budget.charge_work(8)?;
        if row.output != actual.coordinate {
            return Err(refused("private-cell promotion", "actual J origin coordinate").into());
        }
        j_sites.push(integer_sites[operation_ordinal(&integer, row.input)?]);
    }
    let input_sites =
        sites::retained_sites(&j, input, &j_sites, commutative.rows().operations, budget)?;
    let (output_sites, traps) = promoted_sites_and_traps(&mapping, &input_sites, budget)?;
    binding.check(budget)?;
    let source = historical.source();
    use_sites(
        CheckedPromotedSites {
            source,
            output,
            output_sites: &output_sites,
            traps: &traps,
        },
        budget,
        binding,
    )
}

pub(super) fn check_sites(
    sites: CheckedPromotedSites<'_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> PResult<Box<[FormalMemoryObligations]>> {
    let source = sites.source();
    let output = sites.output();
    let output_sites = sites.statements();
    let traps = sites.traps();
    let private = private_memory::check(output, source.limits().max_operations, budget)?;
    binding.check(budget)?;
    private_memory::source_lifetimes_from_sites(source.semantic(), &private, output_sites, budget)?;
    binding.check(budget)?;
    let division = unsigned_division::check(output, source.semantic().target(), budget)?;
    binding.check(budget)?;
    let helpers = scalar_helpers::check(output, budget)?;
    binding.check(budget)?;
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
        "private-cell promotion",
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

fn promoted_sites_and_traps(
    mapping: &PromotionMapping<'_, '_, '_>,
    input_sites: &[Site],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> PResult<(Vec<Site>, Vec<bool>)> {
    let input = mapping.input;
    let output = mapping.output;
    let rows = mapping.relation.origins();
    if input_sites.len() != input.operations().len()
        || rows.len() != output.operations().len()
        || !std::ptr::eq(mapping.relation.input(), input.owner())
        || !std::ptr::eq(mapping.relation.output(), output.owner())
    {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    let mut sites = scratch::<Site>(output.operations().len(), budget)?;
    let mut traps = scratch::<bool>(output.operations().len(), budget)?;
    for (row, new) in rows.iter().zip(output.operations()) {
        budget.charge_work(12)?;
        if row.output != new.coordinate {
            return Err(AssertOriginResourceV1::Accounting.into());
        }
        let ordinal = operation_ordinal(input, row.input)?;
        let old = &input.operations()[ordinal];
        // A checked LoadCopy keeps the Load's original source occurrence. Its
        // previous Store/value evidence remains in the independent pair; it
        // must not impersonate that Store's source statement or a retained Call.
        sites.push(input_sites[ordinal]);
        let mut allowed = false;
        if row.kind == PromotionOriginKind::Retained
            && let (
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
            // The complete promotion pair also preserves all control flow and
            // non-memory producers, so a same-name trap alone is insufficient.
            allowed = old.operation == new.operation;
        }
        traps.push(allowed);
    }
    Ok((sites, traps))
}

#[cfg(test)]
#[path = "production_checked_output_private_cell_sites_v1_tests.rs"]
mod tests;
