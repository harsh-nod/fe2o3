//! Descriptive live read coordinates. Source authentication stays with the
//! consuming production owner; this query proves only the stated implication.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LiveReadBoundV1 {
    pub(crate) operation: Ptr<Operation>,
    pub(crate) view: LiveValue,
    pub(crate) index: LiveValue,
    pub(crate) extent: LiveValue,
    pub(crate) domain: Domain,
}

pub(super) fn preflight(census: Census, count: usize) -> Result<Bound, Limit> {
    let base = live_preflight(census)?;
    if count > census.operations {
        return Err(live_limit(
            "conditional input roster exceeds operation census",
        ));
    }
    // Each read adds two CFG walks, including literal resolution and roster
    // searches; all walks inspect at most B blocks and scan at most O operations.
    let scans = live_sum(&[
        census.blocks,
        census.operations,
        census.results,
        census.block_arguments,
        census.identifier_bytes,
        census.attributes,
        1,
    ])?;
    let roster = live_mul(live_sum(&[count, 1])?, scans)?;
    let walks = live_mul(live_sum(&[census.blocks, 1])?, roster)?;
    let work = live_mul(live_mul(live_sum(&[count, 1])?, walks)?, 128)?;
    let storage = live_sum(&[
        live_mul(count, size_of::<InputBound<LiveValue>>())?,
        size_of::<Vec<InputBound<LiveValue>>>(),
    ])?;
    let extra = Bound::checked_phase(Phase::HierarchicalOwnership, work, 0, storage)?;
    // Input rows stay alive during the original query's temporary allocations.
    Bound::checked_phase(
        Phase::HierarchicalOwnership,
        live_sum(&[base.work_upper_bound(), extra.work_upper_bound()])?,
        0,
        live_sum(&[
            base.peak_storage_upper_bound(),
            extra.peak_storage_upper_bound(),
        ])?,
    )
}

#[cfg(test)]
mod tests;
