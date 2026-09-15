//! Private checked-source/SSA emission boundary. Rows select existing events;
//! they neither define source values nor authorize KIR from canonical metadata.
//! A consumer must place operations at these coordinates, not in row order.
use super::*;

#[path = "emission_input.rs"]
mod emission_input;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BoundaryEvent { Define, Use, Kill }
impl BoundaryEvent {
    fn event(self) -> Event {
        match self { Self::Define => Event::Define, Self::Use => Event::Use, Self::Kill => Event::Kill }
    }
    fn matches(self, actual: SsaResolvedEventV1, variable: SsaVariableIdV1, expected: SsaValueV1) -> bool {
        match (self, actual) {
            (Self::Define, SsaResolvedEventV1::Define { variable: v, value })
            | (Self::Use, SsaResolvedEventV1::Use { variable: v, value })
            | (Self::Kill, SsaResolvedEventV1::Kill { variable: v, previous: Some(value) }) =>
                v == variable && value == expected,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Boundary {
    site: Site,
    point: linear_events::Point,
    variable: SsaVariableIdV1,
    value: SsaValueV1,
    kind: BoundaryEvent,
}
impl Boundary {
    pub(super) fn point(self) -> linear_events::Point { self.point }
    pub(super) fn site(self) -> Site { self.site }
    pub(super) fn variable(self) -> SsaVariableIdV1 { self.variable }
    pub(super) fn value(self) -> SsaValueV1 { self.value }
    pub(super) fn kind(self) -> BoundaryEvent { self.kind }

    pub(super) fn at(query: &ProductionSemanticSsaSourceQueryV1<'_>, site: Site,
        variable: SsaVariableIdV1, value: SsaValueV1, kind: BoundaryEvent,
        work: &mut usize) -> PhaseResult<Self> {
        let point = boundaries::event_point(query, site, variable, value, kind.event(), work)?;
        Ok(Self { site, point, variable, value, kind })
    }
    pub(super) fn existing(query: &ProductionSemanticSsaSourceQueryV1<'_>, point: linear_events::Point,
        variable: SsaVariableIdV1, value: SsaValueV1, kind: BoundaryEvent,
        work: &mut usize) -> PhaseResult<Self> {
        let site = query.event_site(point.block, point.event, &mut || spend(work, 1).is_ok())
            .map_err(|_| rejected("phase emission boundary lost its existing source event"))?;
        let found = Self::at(query, site, variable, value, kind, work)?;
        if found.point != point { return Err(rejected("phase emission boundary substituted an existing event")); }
        Ok(found)
    }
    fn verify(self, query: &ProductionSemanticSsaSourceQueryV1<'_>, work: &mut usize) -> PhaseResult<()> {
        let actual_site = query.event_site(self.point.block, self.point.event, &mut || spend(work, 1).is_ok())
            .map_err(|_| rejected("phase emission boundary lost its checked source site"))?;
        if actual_site != self.site { return Err(rejected("phase emission boundary changed its source site")); }
        let events = query.plan().plan().resolved_events(self.point.block)
            .ok_or_else(|| rejected("phase emission boundary is unreachable"))?;
        // The existing planner stores event indices in ascending order. Bound
        // the lookup explicitly; do not rebuild or scan a second SSA graph.
        let mut lo = 0usize;
        let mut hi = events.len();
        while lo < hi {
            spend(work, 1)?;
            let middle = lo + (hi - lo) / 2;
            match events[middle].0.cmp(&self.point.event) {
                std::cmp::Ordering::Less => lo = middle + 1,
                std::cmp::Ordering::Greater => hi = middle,
                std::cmp::Ordering::Equal => return if self.kind.matches(events[middle].1, self.variable, self.value) {
                    Ok(())
                } else { Err(rejected("phase emission boundary changed its exact SSA event")) },
            }
        }
        Err(rejected("phase emission boundary event is absent"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Action {
    OwnerConvert, Begin, Bind { lease: usize }, Seal,
    RelayClosure, RelayDrop, CloseStorage { lease: usize }, End,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Row {
    phase: usize,
    action: Action,
    boundary: Boundary,
}
impl Row {
    pub(super) fn phase(self) -> usize { self.phase }
    pub(super) fn action(self) -> Action { self.action }
    pub(super) fn boundary(self) -> Boundary { self.boundary }
}

/// Owns the complete private checked result until the consumer has visited
/// every row. There is no public/canonical constructor or byte deserializer.
pub(in super::super) struct CheckedEmissionOwner<'mir, 'view> {
    checked: CheckedSsa<'mir, 'view>,
    rows: Vec<Row>,
}

impl<'mir, 'view> CheckedSsa<'mir, 'view> {
    pub(in super::super) fn into_emission_owner(mut self) -> PhaseResult<CheckedEmissionOwner<'mir, 'view>> {
        if self.phases.len() != self.expansion.phases.len() || self.phases.is_empty() {
            return Err(rejected("phase emission lost its complete checked phase roster"));
        }
        let query = self.owner.source_query_for_root(self.expansion.view.root(), self.expansion.view.body())
            .map_err(|_| rejected("phase emission substituted its exact SSA query owner"))?;
        let work = &mut self.expansion.source.remaining_work;
        let mut owners = reserve(self.expansion.bindings.len(), work)?;
        owners.resize(self.expansion.bindings.len(), false);
        spend(work, owners.len())?;
        let mut count = self.phases.len().checked_mul(5)
            .ok_or_else(|| rejected("phase emission row count overflow"))?;
        for (source, phase) in self.expansion.phases.iter().zip(&self.phases) {
            spend(work, 1)?;
            if source.binds.len() != phase.leases.len() {
                return Err(rejected("phase emission lost its complete allocation roster"));
            }
            let seen = owners.get_mut(source.owner).ok_or_else(|| rejected("phase emission owner left checked bindings"))?;
            count = count.checked_add(usize::from(!*seen))
                .and_then(|n| phase.leases.len().checked_mul(2).and_then(|leases| n.checked_add(leases)))
                .ok_or_else(|| rejected("phase emission row count overflow"))?;
            *seen = true;
        }
        spend(work, owners.len())?;
        owners.fill(false);
        let mut rows = reserve(count, work)?;
        for (index, (source, phase)) in self.expansion.phases.iter().zip(&self.phases).enumerate() {
            spend(work, 1)?;
            let owner = &self.expansion.bindings[source.owner];
            let issue = &self.expansion.bindings[source.issue];
            let finish = &self.expansion.bindings[source.finish];
            let bindings = [
                (phase.emission_owner, phase.converted_owner, owner.destination().local()),
                (phase.emission_issue, phase.issued_phase, issue.destination().local()),
                (phase.emission_finish, phase.completion, finish.destination().local()),
            ];
            for (boundary, value, local) in bindings {
                spend(work, 1)?;
                if boundary.value != value || boundary.variable != SsaVariableIdV1::new(local.index())
                    || boundary.kind != BoundaryEvent::Define {
                    return Err(rejected("phase emission result changed its original checked binding"));
                }
            }
            // A shared original conversion is emitted once, but every phase's
            // retained reference to it must still name the same existing event.
            phase.emission_owner.verify(&query, work)?;
            if phase.emission_pack.kind != BoundaryEvent::Define || phase.emission_drop.kind != BoundaryEvent::Use
                || phase.emission_end.kind != BoundaryEvent::Kill || phase.emission_end.point != phase.end
                || [phase.emission_pack, phase.emission_drop, phase.emission_end].iter()
                    .any(|b| b.value != phase.relay || b.variable != phase.emission_pack.variable)
            {
                return Err(rejected("phase emission lost its exact completion relay chain"));
            }
            let mut push = |action, boundary: Boundary| -> PhaseResult<()> {
                spend(work, 1)?;
                boundary.verify(&query, work)?;
                if rows.len() >= count { return Err(rejected("phase emission exceeded its checked row count")); }
                rows.push(Row { phase: index, action, boundary });
                Ok(())
            };
            if !owners[source.owner] {
                push(Action::OwnerConvert, phase.emission_owner)?;
                owners[source.owner] = true;
            }
            push(Action::Begin, phase.emission_issue)?;
            for (lease_index, (binding, lease)) in source.binds.iter().zip(&phase.leases).enumerate() {
                let local = self.expansion.bindings[binding.call].destination().local();
                if lease.emission_definition.kind != BoundaryEvent::Define || lease.emission_close.kind != BoundaryEvent::Kill
                    || lease.emission_close.point != lease.close
                    || [lease.emission_definition, lease.emission_close].iter()
                        .any(|b| b.value != lease.result || b.variable != SsaVariableIdV1::new(local.index())) {
                    return Err(rejected("phase emission lease changed its checked definition or close"));
                }
                push(Action::Bind { lease: lease_index }, lease.emission_definition)?;
                push(Action::CloseStorage { lease: lease_index }, lease.emission_close)?;
            }
            push(Action::Seal, phase.emission_finish)?;
            push(Action::RelayClosure, phase.emission_pack)?;
            push(Action::RelayDrop, phase.emission_drop)?;
            push(Action::End, phase.emission_end)?;
        }
        if rows.len() != count { return Err(rejected("phase emission omitted a checked lifecycle row")); }
        Ok(CheckedEmissionOwner { checked: self, rows })
    }
}

impl CheckedEmissionOwner<'_, '_> {
    /// The private checked owner remains alive through actual module emission.
    /// No detached request returned by this adapter is an authority boundary.
    pub(in super::super) fn lower<'scope>(
        self,
        scope: fe2o3_lower_mir_kernel::ProductionSemanticPhaseEmissionScopeV1<'scope>,
    ) -> Result<fe2o3_lower_mir_kernel::ProductionSemanticPhaseEmissionResultV1<'scope>,
        crate::production_pipeline::ProductionPipelineError>
    {
        use crate::production_pipeline::ProductionPipelineError as Error;
        let mut lowering_error = None;
        let attached = scope.semantic_ssa();
        let output = self.consume_metered(None, |checked, row, input, work| {
            if !std::ptr::eq(attached, checked.owner) {
                return Err(rejected("phase emission changed the live lowerer owner"));
            }
            if input.is_none() {
                *input = Some(emission_input::input(checked, work)?);
            }
            emission_input::append(input.as_mut().unwrap(), row, work)
        }, |checked, input, work| {
            if !std::ptr::eq(scope.semantic_ssa(), checked.owner) {
                return Err(rejected("phase emission lost source custody before lowering"));
            }
            match scope.emit(input.ok_or_else(|| rejected("phase emission has no complete rows"))?, work) {
                Ok(output) => Ok(output),
                Err(error) => {
                    lowering_error = Some(error);
                    Err(rejected("phase target-neutral emission failed"))
                }
            }
        });
        match (output, lowering_error) {
            (Ok(output), None) => Ok(output),
            (Err(_), Some(error)) => Err(Error::TargetNeutralLowering(error)),
            (Err(error), None) => Err(Error::SemanticImport(error)),
            (Ok(_), Some(_)) => unreachable!("failed emission cannot produce an owner"),
        }
    }

    /// Visits the full checked roster or fails. Callback order is recipe order,
    /// not CFG order. Actual KIR insertion must use each exact event boundary,
    /// with Close/End before the existing source kill, and verify all-edge use.
    pub(super) fn consume(self, emit: impl FnMut(&CheckedSsa<'_, '_>, Row) -> PhaseResult<()>) -> PhaseResult<()> {
        self.consume_rows(emit).map(drop)
    }

    /// Lends the existing source work counter, never a fresh allowance, to the
    /// request mapper and final emitter. The checked graph is immutable and
    /// remains owned here until the final callback returns.
    pub(in super::super) fn consume_metered<S, T>(
        mut self,
        mut state: S,
        mut emit: impl FnMut(&CheckedSsa<'_, '_>, Row, &mut S, &mut usize) -> PhaseResult<()>,
        finish: impl FnOnce(&CheckedSsa<'_, '_>, S, &mut usize) -> PhaseResult<T>,
    ) -> PhaseResult<T> {
        // Move, rather than copy, the allowance out of the read-only view.
        let mut work = std::mem::take(&mut self.checked.expansion.source.remaining_work);
        for row in &self.rows {
            spend(&mut work, 1)?;
            emit(&self.checked, *row, &mut state, &mut work)?;
        }
        spend(&mut work, 1)?;
        finish(&self.checked, state, &mut work)
    }

    fn consume_rows(
        mut self,
        mut emit: impl FnMut(&CheckedSsa<'_, '_>, Row) -> PhaseResult<()>,
    ) -> PhaseResult<Self> {
        for row in &self.rows {
            spend(&mut self.checked.expansion.source.remaining_work, 1)?;
            emit(&self.checked, *row)?;
        }
        Ok(self)
    }
}

#[cfg(test)]
#[path = "emission_sites_tests.rs"]
pub(in super::super) mod tests;
