//! Retained original lazy induction scope at the pre-writer boundary.
//! The captured error stays deferred, exactly as in the old with_scope entry.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_preparation_resources_v1::resource;
use std::mem::size_of;

pub(in crate::production_ranked_projection_v1) struct RetainedLazyScopePrefixV1 {
    scope: Option<Scope>,
    entered: bool,
}
impl RetainedLazyScopePrefixV1 {
    pub(in crate::production_ranked_projection_v1) fn new() -> Self {
        Self {
            scope: None,
            entered: false,
        }
    }
    pub(in crate::production_ranked_projection_v1) fn capture_into(
        &mut self,
        facts: &mut dyn ProjectedAssertionFactsV1,
    ) -> Result<()> {
        if self.entered || self.scope.is_some() {
            return Err(resource(Resource::Accounting));
        }
        self.entered = true;
        // Original storage getter then identity getter; no eager query/check,
        // no header debit, and no conversion of a deferred Err into empty DATA.
        self.scope = Some(Scope {
            entry: facts.scalar_private_storage_v1().and_then(|floor| {
                facts
                    .helper_value_ledger_v1()
                    .map(|(address, ledger)| (address, ledger, floor))
            }),
            retained: 0,
            started: false,
        });
        Ok(())
    }
    pub(in crate::production_ranked_projection_v1) fn before_writers(&self) -> Result<()> {
        let scope = self
            .scope
            .as_ref()
            .ok_or_else(|| resource(Resource::Accounting))?;
        if !self.entered || scope.started || scope.retained != 0 {
            return Err(resource(Resource::Accounting));
        }
        // Do not force scope.entry: the original wrapper did not do that before
        // its first induction request, and no such request has occurred here.
        Ok(())
    }
}
pub(in crate::production_ranked_projection_v1) fn retained_lazy_prefix_frame_v1() -> usize {
    size_of::<(
        RetainedLazyScopePrefixV1, Option<Scope>, Scope,
        &mut RetainedLazyScopePrefixV1, &RetainedLazyScopePrefixV1, &Scope,
        Option<&Scope>, Result<&Scope>, bool, usize,
        &mut dyn ProjectedAssertionFactsV1,
        Result<(usize, Ledger, usize)>, (usize, Ledger, usize),
        Result<(usize, Ledger)>, (usize, Ledger), Result<usize>,
        Result<()>, ProductionRankedProjectionErrorV1,
        crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1,
        Resource, &mut dyn FnMut(usize) -> Result<(usize, Ledger, usize)>,
    )>()
}

#[cfg(test)]
#[path = "retained_lazy_scope_prefix_v1_tests.rs"]
mod tests;
