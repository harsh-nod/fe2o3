//! Exact dormant-value event census over the existing SSA plan. This supplies
//! no source identity, borrow permission, allocation or control-flow proof.
use fe2o3_mir_model::{SsaBlockIdV1, SsaConstructionPlanV1, SsaResolvedEventV1, SsaValueV1, SsaVariableIdV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Point {
    pub block: SsaBlockIdV1,
    pub event: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Work,
    Definition,
    Use,
    Kill,
}

/// First acyclic dormant-lease subset: one exact definition and one existing
/// kill, no reads or copies. The caller authenticates both source coordinates
/// and the complete phase protocol; this check does not insert a missing kill.
pub(super) fn dormant_lease(
    plan: &SsaConstructionPlanV1,
    variable: SsaVariableIdV1,
    value: SsaValueV1,
    definition: Point,
    charge: &mut impl FnMut() -> bool,
) -> Result<Point, Error> {
    let mut defined = false;
    let mut killed = None;
    for block in plan.reverse_postorder() {
        step(charge)?;
        let events = plan.resolved_events(*block).ok_or(Error::Definition)?;
        for (index, event) in events {
            step(charge)?;
            let point = Point { block: *block, event: *index };
            match *event {
                SsaResolvedEventV1::Define { variable: actual, value: assigned }
                    if actual == variable || assigned == value =>
                {
                    if actual != variable || assigned != value || point != definition || defined {
                        return Err(Error::Definition);
                    }
                    defined = true;
                }
                SsaResolvedEventV1::Use { variable: actual, value: used }
                    if actual == variable || used == value => return Err(Error::Use),
                SsaResolvedEventV1::Kill { variable: actual, previous: Some(previous) }
                    if actual == variable || previous == value =>
                {
                    if actual != variable || previous != value || killed.replace(point).is_some() {
                        return Err(Error::Kill);
                    }
                }
                _ => {}
            }
        }
    }
    if !defined { return Err(Error::Definition); }
    killed.ok_or(Error::Kill)
}

fn step(charge: &mut impl FnMut() -> bool) -> Result<(), Error> {
    charge().then_some(()).ok_or(Error::Work)
}

#[cfg(test)]
#[path = "linear_events_tests.rs"]
mod tests;
