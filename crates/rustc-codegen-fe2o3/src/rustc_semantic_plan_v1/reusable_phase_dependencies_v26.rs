//! Parent: rustc_semantic_plan_v1. Called after each function's ordinary ABI
//! type loop; uses its existing counts, normalization, layout and identity gates.
use super::*;

pub(super) fn retain<'tcx>(
    preflight: &mut BodyPreflightV1<'_, 'tcx>,
    site: RejectionSiteV1,
) -> Result<(), PendingRejectionV1> {
    let available = preflight
        .limits
        .limit(SemanticMirResourceV1::ValidationWork)
        .checked_sub(preflight.counts.validation_work)
        .ok_or_else(|| reject("phase logical dependency validation-work ceiling", site))?;
    let mut work = usize::try_from(available)
        .map_err(|_| reject("phase logical dependency work conversion", site))?;
    let before = work;
    let observed =
        crate::collector::reusable_phase_logical_dependencies_v26(
            preflight.tcx,
            preflight.instance,
            &mut work,
        );
    preflight.charge(SemanticMirResourceV1::ValidationWork, before - work)?;
    if let Some(values) =
        observed.map_err(|_| reject("authenticated phase logical type dependency", site))?
    {
        for ty in values.into_iter().flatten() {
            preflight.inspect_type(ty, site)?;
        }
    }
    Ok(())
}
