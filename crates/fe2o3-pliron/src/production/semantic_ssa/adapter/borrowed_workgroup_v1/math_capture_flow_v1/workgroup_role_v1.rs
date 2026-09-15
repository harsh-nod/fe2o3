//! Bounded secondary custody paths; never replacement primaries or issuers.
use super::super::matrix_access_borrow::shared_pointee;
use super::*;
use shared_workgroup_carrier_v1::lookup_work;

impl Routes<'_> {
    pub(super) fn fail_secondary(
        &mut self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        if self.failed_secondaries.is_empty() {
            budget.charge(3)?;
        }
        budget.charge(2 + lookup_work(self.failed_secondaries.len()))?;
        self.failed_secondaries.insert(ty);
        Ok(())
    }

    pub(in super::super) fn secondary_complete(
        &self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        if self.failed_secondaries.is_empty() {
            return Ok(true);
        }
        budget
            .charge(lookup_work(self.failed_secondaries.len()) + lookup_work(self.shared.len()))?;
        let carrier = self.shared.get(&ty).map_or(ty, |shared| shared.pointee);
        Ok(!self.failed_secondaries.contains(&carrier))
    }

    pub(super) fn add_secondary(
        &mut self,
        ty: SemanticTypeIdV1,
        primary: Route,
        role: SecondaryRole,
        route: Route,
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        budget.charge(16 + MAX_FIELDS + lookup_work(self.secondaries.len()))?;
        let overlaps = |a: Route, b: Route| {
            let common = a.len.min(b.len);
            a.fields[..common] == b.fields[..common]
        };
        if primary.len == 0
            || route.len == 0
            || shared_pointee(self.types, primary.reference) != Some(primary.owned)
            || overlaps(primary, route)
        {
            return Ok(());
        }
        if let Some(others) = self.secondaries.get(&ty) {
            budget.charge(2 * (1 + MAX_FIELDS))?;
            if others
                .iter()
                .flatten()
                .any(|(_, other)| overlaps(*other, route))
            {
                return Ok(());
            }
        }
        if self.secondaries.is_empty() {
            budget.charge(3)?;
        }
        // Two inline slots, no per-type vector or further shape traversal.
        budget.charge(2 * (MAX_FIELDS + 5) + lookup_work(self.secondaries.len()))?;
        self.secondaries.entry(ty).or_insert([None; 2])[role.slot()] = Some((role, route));
        Ok(())
    }

    pub(super) fn alternates_for(
        &self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<[Option<(Route, Option<SemanticTypeIdV1>)>; 2], ProductionSemanticSsaErrorV1> {
        let mut result = [None; 2];
        if self.secondaries.is_empty() {
            return Ok(result);
        }
        budget.charge(lookup_work(self.secondaries.len()))?;
        if let Some(routes) = self.secondaries.get(&ty) {
            for (slot, &(_, route)) in result.iter_mut().zip(routes.iter().flatten()) {
                budget.charge(MAX_FIELDS + 5)?;
                *slot = Some((route, None));
            }
            return Ok(result);
        }
        budget.charge(lookup_work(self.shared.len()))?;
        let Some(carrier) = self.shared.get(&ty) else {
            return Ok(result);
        };
        budget.charge(lookup_work(self.secondaries.len()) + lookup_work(self.routes.len()))?;
        let Some(routes) = self.secondaries.get(&carrier.pointee) else {
            return Ok(result);
        };
        if routes[SecondaryRole::Workgroup.slot()]
            != Some((SecondaryRole::Workgroup, carrier.route))
        {
            return Ok(result);
        }
        // A shared wrapper still selects Workgroup. Its other paths use exactly
        // the same checked pointee and single dereference, not another wrapper.
        for (slot, route) in result.iter_mut().zip(
            self.routes
                .get(&carrier.pointee)
                .copied()
                .into_iter()
                .chain(routes[SecondaryRole::Policy.slot()].map(|(_, route)| route)),
        ) {
            budget.charge(MAX_FIELDS + 5)?;
            *slot = Some((route, Some(carrier.pointee)));
        }
        Ok(result)
    }

    /// A present role with no exact source is a failed obligation, not an absent
    /// role. Whole-carrier forwarding already retains its original parent edge.
    pub(in super::super) fn secondary_sources<'b>(
        &self,
        assignment: &'b SemanticAssignmentV1,
        budget: &mut Budget,
    ) -> Result<
        [Option<(SecondaryRole, usize, Option<&'b SemanticPlaceV1>)>; 2],
        ProductionSemanticSsaErrorV1,
    > {
        let mut result = [None; 2];
        if self.secondaries.is_empty()
            || !matches!(
                assignment.value().kind(),
                SemanticRvalueKindV1::Aggregate(_)
            )
        {
            return Ok(result);
        }
        budget.charge(lookup_work(self.secondaries.len()))?;
        let Some(routes) = self.secondaries.get(&assignment.destination().ty()) else {
            return Ok(result);
        };
        for (slot, &(role, route)) in result.iter_mut().zip(routes.iter().flatten()) {
            budget.charge(MAX_FIELDS + 5)?;
            let source = if assignment.destination().projections().is_empty()
                && assignment.destination().ty() == assignment.value().result_type()
            {
                self.source_for(assignment, route, None, budget)?
            } else {
                None
            };
            *slot = Some((role, route.fields[0] as usize, source));
        }
        Ok(result)
    }
}
