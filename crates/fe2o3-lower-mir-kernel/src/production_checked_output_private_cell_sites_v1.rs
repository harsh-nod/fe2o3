use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirPrivateCellOriginKindV1 as PromotionOriginKind,
    CheckedCanonicalKirPrivateCellPromotionV1 as PromotionRelation,
};
type Site = Option<(SemanticFunctionIdV1, SemanticBlockIdV1, u32)>;
#[path = "production_checked_output_loop_unroll_sites_v1.rs"]
mod loop_unroll_sites;
pub use loop_unroll_sites::ProductionLoopUnrollOriginV1;
pub(super) use loop_unroll_sites::with_checked_loop_unroll_sites;
#[path = "production_checked_output_cross_block_forwarding_sites_v1.rs"]
mod cross_block_forwarding_sites;
pub use cross_block_forwarding_sites::ProductionCrossBlockForwardingOriginV1;
pub(super) use cross_block_forwarding_sites::check_cross_block_forwarding_sites;
#[cfg(test)]
pub(super) use cross_block_forwarding_sites::exercise_cross_block_source_refusals;
pub(super) use cross_block_forwarding_sites::with_checked_cross_block_forwarding_sites;

#[path = "production_checked_output_licm_sites_v1.rs"]
mod licm_sites;
#[cfg(test)]
pub(super) use licm_sites::check_licm_after_preheaders_sites;
pub(super) use licm_sites::with_licm_after_preheaders_sites;
#[path = "production_checked_output_induction_refinement_sites_v1.rs"]
mod induction_refinement_sites;
pub use induction_refinement_sites::ProductionInductionRefinementOriginV1;
pub(super) use induction_refinement_sites::check_induction_refinement_sites;
pub(super) use induction_refinement_sites::with_checked_induction_refinement_sites;
#[cfg(test)]
pub(super) use licm_sites::exercise_licm_source_sites;

pub(super) struct PromotionMapping<'v, 'o, 'r> {
    pub(super) input: &'v CanonicalKirInventoryV1<'o>,
    pub(super) output: &'v CanonicalKirInventoryV1<'o>,
    pub(super) relation: &'v PromotionRelation<'r>,
}

// Keep the historical private path while sharing the checked source view with
// the expanded owner. Its representation and old construction checks are intact.
pub(super) use crate::production_semantic_kir_v1::checked_output_admission_policy3_v1::general::CheckedPromotedSites;

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
    census_sites::<false>(sites, budget, binding)
}

// Decode supplies independently checked *relations*, never an owning optimizer
// witness. Recover source custody once at B/C, then use the same source-site
// transport as the live path through every actual predecessor and output.
#[allow(clippy::too_many_arguments)]
pub(super) fn check_decoded_expanded_sites_v1(
    source: GeneralSourceContextV1<'_>,
    history: &fe2o3_kernel_opt::CheckedLoopUnrollHistoryV1<'_>,
    scalar: &fe2o3_kernel_opt::CheckedScalarFixedPointOwnerV1,
    final_origins: &mut Vec<ProductionExpandedSourceOriginV1>,
    required: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> PResult<Box<[FormalMemoryObligations]>> {
    let p3 = history
        .prefix()
        .prefix()
        .policy7_relation()
        .policy6_relation()
        .policy5_relation()
        .policy4_relation()
        .policy3_relation()
        .semantic_receipt();
    budget.charge_work(2)?;
    if history.limits().loops.operations > source.limits().max_operations {
        return Err(refused(
            "decoded source unroll",
            "unchanged source operation ceiling",
        )
        .into());
    }
    let report_bytes = p3
        .output()
        .module()
        .kernels
        .len()
        .checked_mul(size_of::<FormalMemoryObligations>())
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    budget.reserve_storage(report_bytes)?;
    let initial_reports = check_decoded_source_output_pair_v1(
        source,
        p3.input(),
        p3.output(),
        p3.receipt().candidate(),
        p3.input().canonical().canonical_bytes(),
        required,
        budget,
    )?;
    binding.check(budget)?;
    drop(initial_reports);
    let (coordinates, storage) =
        fe2o3_kernel_analysis::check_canonical_kir_coordinate_preservation_v1(
            source.neutral()?,
            p3.input(),
            budget,
        )
        .map_err(E::Coordinates)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (bound, storage) =
        CanonicalKirInventoryV1::derive(p3.input(), budget).map_err(inventory_error)?;
    binding.check(budget)?;
    budget.reserve_storage(storage.retained_storage())?;
    match source {
        GeneralSourceContextV1::Direct(original) => {
            let sites = private_memory::source_statement_sites_v1(original, &bound, budget)?;
            binding.check(budget)?;
            decoded_expanded_from_bound_sites_v1(
                source,
                history,
                &bound,
                &sites,
                scalar,
                final_origins,
                budget,
                binding,
            )
        }
        GeneralSourceContextV1::Erased(original) => original
            .with_checked_erasure_v1(budget, |erasure, budget| {
                Ok(promotion_scoped(
                    budget.storage(),
                    budget,
                    |budget, inner| {
                        binding.check(budget)?;
                        if !std::ptr::eq(erasure.output(), coordinates.input()) {
                            return Err(E::SourceOutput(
                                ProductionSourceOutputErrorV1::InputCustody,
                            )
                            .into());
                        }
                        let map = ErasedSourceCoordinateMapV1 {
                            deletion: erasure,
                            floor: budget.storage(),
                        };
                        let sites = private_memory::erased_source_statement_sites_v1(
                            original, &map, &bound, budget,
                        )?;
                        inner.check(budget)?;
                        decoded_expanded_from_bound_sites_v1(
                            source,
                            history,
                            &bound,
                            &sites,
                            scalar,
                            final_origins,
                            budget,
                            inner,
                        )
                    },
                ))
            })
            .map_err(E::Source)?,
    }
}

#[allow(clippy::too_many_arguments)]
fn decoded_expanded_from_bound_sites_v1(
    source: GeneralSourceContextV1<'_>,
    history: &fe2o3_kernel_opt::CheckedLoopUnrollHistoryV1<'_>,
    bound: &CanonicalKirInventoryV1<'_>,
    bound_sites: &[Site],
    scalar: &fe2o3_kernel_opt::CheckedScalarFixedPointOwnerV1,
    final_origins: &mut Vec<ProductionExpandedSourceOriginV1>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> PResult<Box<[FormalMemoryObligations]>> {
    let f = history.prefix();
    budget.reserve_storage(
        size_of::<PromotionMapping<'_, '_, '_>>()
            .checked_add(size_of::<CheckedPromotedSites<'_, '_>>())
            .ok_or(AssertOriginResourceV1::Arithmetic)?,
    )?;
    let p8 = f.prefix();
    let p7 = p8.policy7_relation();
    let p6 = p7.policy6_relation();
    let p5 = p6.policy5_relation();
    let p3 = p5.policy4_relation().policy3_relation().semantic_receipt();
    macro_rules! inventory {
        ($owner:expr) => {{
            let (inventory, storage) =
                CanonicalKirInventoryV1::derive($owner, budget).map_err(inventory_error)?;
            binding.check(budget)?;
            budget.reserve_storage(storage.retained_storage())?;
            inventory
        }};
    }
    let c = inventory!(p3.output());
    let o = inventory!(p5.output());
    let i = inventory!(p6.continuation().output());
    let j = inventory!(p7.continuation().relation().output());
    let k = inventory!(p8.output());
    let p = inventory!(f.promotion().output());
    let h = inventory!(f.preheaders().output());
    let l = inventory!(f.licm().output());
    let r = inventory!(f.refinement().output());
    let forwarded = inventory!(f.output());
    let u = inventory!(history.output());
    // P4/P5 are coordinate-preserving, independently checked complete history
    // relations. P3/P6/P8 use their own checked complete occurrence rows.
    let c_sites = sites::retained_sites(
        bound,
        &c,
        bound_sites,
        p3.receipt().candidate().operations,
        budget,
    )?;
    let o_sites = sites::coordinate_sites(&c, &o, &c_sites, budget)?;
    let i_sites = sites::retained_sites(
        &o,
        &i,
        &o_sites,
        p6.continuation().receipt().candidate().operations,
        budget,
    )?;
    let mut j_sites = scratch::<Site>(j.operations().len(), budget)?;
    let j_origins = p7.continuation().relation().retained_operations();
    budget.charge_work(1)?;
    if j_origins.len() != j.operations().len() {
        return Err(refused("decoded source prefix", "complete actual J origins").into());
    }
    for (row, actual) in j_origins.iter().zip(j.operations()) {
        budget.charge_work(8)?;
        if row.output != actual.coordinate {
            return Err(refused("decoded source prefix", "actual J origin coordinate").into());
        }
        j_sites.push(i_sites[operation_ordinal(&i, row.input)?]);
    }
    let k_sites = sites::retained_sites(
        &j,
        &k,
        &j_sites,
        p8.continuation().claims().occurrences.operations,
        budget,
    )?;
    let (p_sites, p_traps) = promoted_sites_and_traps(
        &PromotionMapping {
            input: &k,
            output: &p,
            relation: f.promotion(),
        },
        &k_sites,
        budget,
    )?;
    let mut refinement_origins =
        scratch::<ProductionInductionRefinementOriginV1>(l.operations().len(), budget)?;
    let mut forwarding_origins =
        scratch::<ProductionCrossBlockForwardingOriginV1>(r.operations().len(), budget)?;
    let mut unroll_origins = scratch::<ProductionLoopUnrollOriginV1>(
        history.continuation().origins().operations.len(),
        budget,
    )?;
    binding.check(budget)?;
    with_licm_after_preheaders_sites(
        CheckedPromotedSites {
            source,
            output: &p,
            output_sites: &p_sites,
            traps: &p_traps,
        },
        f.preheaders(),
        f.licm(),
        &h,
        &l,
        budget,
        binding,
        |sites, budget, binding| {
            with_checked_induction_refinement_sites(
                sites,
                f.refinement(),
                &r,
                &mut refinement_origins,
                budget,
                binding,
                |sites, budget, binding| {
                    with_checked_cross_block_forwarding_sites(
                        sites,
                        f.forwarding(),
                        &forwarded,
                        &mut forwarding_origins,
                        budget,
                        binding,
                        |sites, budget, binding| {
                            with_checked_loop_unroll_sites(
                                sites,
                                &r,
                                f.refinement(),
                                f.forwarding(),
                                history.continuation(),
                                &u,
                                &mut unroll_origins,
                                budget,
                                binding,
                                |sites,
                                 intermediate,
                                 input,
                                 refinement,
                                 forwarding,
                                 unroll,
                                 budget,
                                 binding| {
                                    // A borrowed checked view is not an owning producer witness.
                                    // Re-run the old actual-U census before the final scalar census.
                                    let report_bytes = u
                                        .owner()
                                        .module()
                                        .kernels
                                        .len()
                                        .checked_mul(size_of::<FormalMemoryObligations>())
                                        .ok_or(AssertOriginResourceV1::Arithmetic)?;
                                    budget.reserve_storage(report_bytes)?;
                                    budget
                                        .reserve_storage(
                                            size_of::<CheckedPromotedSites<'_, '_>>(),
                                        )?;
                                    let reports = census_loop_unroll_sites(
                                        CheckedPromotedSites {
                                            source: sites.source(),
                                            output: sites.output(),
                                            output_sites: sites.statements(),
                                            traps: sites.traps(),
                                        },
                                        intermediate,
                                        input,
                                        refinement,
                                        forwarding,
                                        unroll,
                                        budget,
                                        binding,
                                    )?;
                                    binding.check(budget)?;
                                    drop(reports);
                                    check_expanded_source_v1(
                                        sites,
                                        intermediate,
                                        input,
                                        refinement,
                                        forwarding,
                                        unroll,
                                        scalar,
                                        final_origins,
                                        budget,
                                    )
                                    .map_err(PError::from)
                                },
                            )
                        },
                    )
                },
            )
        },
    )
}

// Closed final-output adapter: neither detached sites nor a caller-selected
// phase can authorize another graph. The independent complete neutral pair
// authenticates all payloads/control and the unchanged kernel/report roster.
pub(super) fn check_loop_preheader_sites(
    sites: CheckedPromotedSites<'_, '_>,
    pair: &fe2o3_kernel_analysis::CheckedCanonicalKirLoopPreheadersV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> PResult<Box<[FormalMemoryObligations]>> {
    with_checked_loop_preheader_sites(
        sites,
        pair,
        output,
        budget,
        binding,
        |sites, budget, binding| census_sites::<true>(sites, budget, binding),
    )
}

fn with_checked_loop_preheader_sites<'g, 'w, R>(
    sites: CheckedPromotedSites<'_, '_>,
    pair: &fe2o3_kernel_analysis::CheckedCanonicalKirLoopPreheadersV1<'_>,
    output: &CanonicalKirInventoryV1<'g>,
    budget: &mut AssertOriginBudgetV1<'w>,
    binding: &PromotionBinding,
    use_sites: impl for<'s> FnOnce(
        CheckedPromotedSites<'s, 'g>,
        &mut AssertOriginBudgetV1<'w>,
        &PromotionBinding,
    ) -> PResult<R>,
) -> PResult<R> {
    budget.charge_work(7)?;
    if !std::ptr::eq(pair.input(), sites.output.owner())
        || !std::ptr::eq(pair.output(), output.owner())
        || sites.output.operations().len() != output.operations().len()
        || sites.output_sites.len() != output.operations().len()
        || sites.traps.len() != output.operations().len()
        || sites.output.owner().module().kernels.len() != output.owner().module().kernels.len()
    {
        return Err(refused(
            "neutral loop preheaders",
            "exact pair/source/output custody",
        )
        .into());
    }
    for (before, after) in sites.output.operations().iter().zip(output.operations()) {
        budget.charge_work(3)?;
        if before.coordinate != after.coordinate {
            return Err(refused(
                "neutral loop preheaders",
                "unchanged ordered operation coordinates",
            )
            .into());
        }
    }
    binding.check(budget)?;
    use_sites(
        CheckedPromotedSites {
            source: sites.source,
            output,
            output_sites: sites.output_sites,
            traps: sites.traps,
        },
        budget,
        binding,
    )
}

fn census_sites<const PREHEADERS: bool>(
    sites: CheckedPromotedSites<'_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> PResult<Box<[FormalMemoryObligations]>> {
    census_sites_named(
        sites,
        if PREHEADERS {
            "neutral loop preheaders"
        } else {
            "private-cell promotion"
        },
        budget,
        binding,
    )
}

fn census_sites_named(
    sites: CheckedPromotedSites<'_, '_>,
    stage: &'static str,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> PResult<Box<[FormalMemoryObligations]>> {
    census_sites_inner(sites, stage, None, None, None, budget, binding)
}

pub(super) fn census_induction_refinement_sites(
    sites: CheckedPromotedSites<'_, '_>,
    pair: &fe2o3_kernel_analysis::CheckedCanonicalKirInductionRefinementV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> PResult<Box<[FormalMemoryObligations]>> {
    census_sites_inner(
        sites,
        "checked induction refinement",
        Some(pair),
        None,
        None,
        budget,
        binding,
    )
}

pub(super) fn census_refined_forwarding_sites(
    sites: CheckedPromotedSites<'_, '_>,
    intermediate: &CanonicalKirInventoryV1<'_>,
    refinement: &fe2o3_kernel_analysis::CheckedCanonicalKirInductionRefinementV1<'_>,
    forwarding: &fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> PResult<Box<[FormalMemoryObligations]>> {
    census_sites_inner(
        sites,
        "refined cross-block private forwarding",
        Some(refinement),
        Some((forwarding, intermediate)),
        None,
        budget,
        binding,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn census_loop_unroll_sites(
    sites: CheckedPromotedSites<'_, '_>,
    intermediate: &CanonicalKirInventoryV1<'_>,
    final_input: &CanonicalKirInventoryV1<'_>,
    refinement: &fe2o3_kernel_analysis::CheckedCanonicalKirInductionRefinementV1<'_>,
    forwarding: &fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV1<'_>,
    unroll: &fe2o3_kernel_analysis::CheckedCanonicalKirLoopUnrollPairV1<'_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> PResult<Box<[FormalMemoryObligations]>> {
    census_sites_inner(
        sites,
        "bounded source unroll",
        Some(refinement),
        Some((forwarding, intermediate)),
        Some((unroll, final_input)),
        budget,
        binding,
    )
}

pub(super) fn check_licm_sites(
    sites: CheckedPromotedSites<'_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &PromotionBinding,
) -> PResult<Box<[FormalMemoryObligations]>> {
    census_sites_named(sites, "total-integer LICM", budget, binding)
}

#[allow(clippy::too_many_arguments)]
fn census_sites_inner(
    sites: CheckedPromotedSites<'_, '_>,
    stage: &'static str,
    refinement: Option<&fe2o3_kernel_analysis::CheckedCanonicalKirInductionRefinementV1<'_>>,
    forwarding: Option<(
        &fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV1<'_>,
        &CanonicalKirInventoryV1<'_>,
    )>,
    unroll: Option<(
        &fe2o3_kernel_analysis::CheckedCanonicalKirLoopUnrollPairV1<'_, '_, '_>,
        &CanonicalKirInventoryV1<'_>,
    )>,
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
    let authorized_trap = |ordinal: usize, coordinate: CanonicalKirOperationCoordinateV1| {
        Ok(traps.get(ordinal).copied().unwrap_or(false)
            && output
                .operations()
                .get(ordinal)
                .is_some_and(|row| row.coordinate == coordinate))
    };
    if let Some((unroll, final_input)) = unroll {
        let refinement =
            refinement.ok_or_else(|| refused("bounded source unroll", "actual refinement pair"))?;
        let (forwarding, intermediate) =
            forwarding.ok_or_else(|| refused("bounded source unroll", "actual forwarding pair"))?;
        census::native_with_loop_unroll(
            output,
            &private,
            &division,
            &helpers,
            stage,
            authorized_trap,
            refinement,
            forwarding,
            intermediate,
            final_input,
            unroll,
            budget,
        )?;
    } else if let Some((forwarding, intermediate)) = forwarding {
        let pair = refinement.ok_or_else(|| {
            refused(
                "refined cross-block private forwarding",
                "both actual sequential pairs",
            )
        })?;
        census::native_with_refinement_forwarding(
            output,
            &private,
            &division,
            &helpers,
            stage,
            authorized_trap,
            pair,
            forwarding,
            intermediate,
            budget,
        )?;
    } else if let Some(pair) = refinement {
        census::native_with_induction_refinement(
            output,
            &private,
            &division,
            &helpers,
            stage,
            authorized_trap,
            pair,
            budget,
        )?;
    } else {
        census::native(
            output,
            &private,
            &division,
            &helpers,
            stage,
            authorized_trap,
            budget,
        )?;
    }
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
