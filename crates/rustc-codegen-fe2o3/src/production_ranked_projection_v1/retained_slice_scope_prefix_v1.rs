//! The original slice-scope reservation retained by the whole-root pending owner.
//! No slice scratch, extent result or completed producer is manufactured here.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_preparation_resources_v1::{
    PreparationCustodySnapshotV1 as Snapshot, PreparationResourcesV1 as Prep,
    retained_custody_snapshot_frame_v1,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Complete,
}
pub(in crate::production_ranked_projection_v1) struct RetainedSliceScopePrefixV1 {
    phase: Phase,
    count: Option<usize>,
    entry: Option<Snapshot>,
    held: Option<Snapshot>,
    retained: usize,
}
impl RetainedSliceScopePrefixV1 {
    pub(in crate::production_ranked_projection_v1) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            count: None,
            entry: None,
            held: None,
            retained: 0,
        }
    }
    pub(in crate::production_ranked_projection_v1) fn prepare_into(
        &mut self,
        count: usize,
        resources: &mut Prep<'_, '_>,
    ) -> Result<(), Error> {
        if self.phase != Phase::Fresh {
            return Err(resource(Resource::Accounting));
        }
        self.phase = Phase::Terminal;
        self.count = Some(count);
        // Exact original order: work4, size formula, original floor, reserve.
        resources.work(4)?;
        let requested = storage([count; 5])?;
        self.entry = resources.retained_custody_snapshot_v1();
        let entry = self.entry.ok_or_else(|| resource(Resource::Accounting))?;
        if entry.denied_work || entry.denied_storage || entry.owned > entry.storage {
            return Err(resource(Resource::Accounting));
        }
        resources.reserve_storage(requested)?;
        self.retained = requested;
        self.held = resources.retained_custody_snapshot_v1();
        self.phase = Phase::Complete;
        self.before_writers(count, resources)
    }
    pub(in crate::production_ranked_projection_v1) fn before_writers(
        &self,
        count: usize,
        resources: &Prep<'_, '_>,
    ) -> Result<(), Error> {
        let entry = self.entry.ok_or_else(|| resource(Resource::Accounting))?;
        let held = self.held.ok_or_else(|| resource(Resource::Accounting))?;
        let now = resources
            .retained_custody_snapshot_v1()
            .ok_or_else(|| resource(Resource::Accounting))?;
        let growth = now
            .owned
            .checked_sub(entry.owned)
            .ok_or_else(|| resource(Resource::Accounting))?;
        let expected = entry
            .storage
            .checked_add(growth)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        if self.phase != Phase::Complete
            || self.count != Some(count)
            || self.retained != storage([count; 5])?
            || now.budget_slot != entry.budget_slot
            || now.work_ledger != entry.work_ledger
            || now.owned_slot != entry.owned_slot
            || now.storage != expected
            || now.owned < held.owned
            || now.storage < held.storage
            || now.work < held.work
            || now.peak < held.peak
            || now.denied_work
            || now.denied_storage
        {
            return Err(resource(Resource::Accounting));
        }
        Ok(())
    }
}
pub(in crate::production_ranked_projection_v1) fn retained_slice_prefix_frame_v1()
-> Result<usize, Error> {
    let rows = [
        size_of::<(RetainedSliceScopePrefixV1, Phase, Option<usize>, usize,
            Option<Snapshot>, Snapshot, Result<Snapshot, Error>,
            &mut RetainedSliceScopePrefixV1, &RetainedSliceScopePrefixV1,
            &mut Prep<'static, 'static>, &Prep<'static, 'static>, Resource,
            crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1,
            Error, Result<(), Error>, Result<usize, Error>)>(),
        size_of::<([usize; 5], [usize; 5], usize, usize, Option<usize>,
            std::iter::Zip<std::array::IntoIter<usize, 5>, std::array::IntoIter<usize, 5>>,
            (usize, usize), usize, Result<usize, Error>, Scratch, Scope<'static>,
            Vec<BoundsLocalDefinitionV1<'static>>) >(),
        size_of::<([usize; 4], std::array::IntoIter<usize, 4>, usize, usize, Option<usize>)>(),
        retained_custody_snapshot_frame_v1(),
    ];
    rows.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row)
            .ok_or_else(|| resource(Resource::Arithmetic))
    })
}

#[cfg(test)]
#[path = "retained_slice_scope_prefix_v1_tests.rs"]
mod tests;
