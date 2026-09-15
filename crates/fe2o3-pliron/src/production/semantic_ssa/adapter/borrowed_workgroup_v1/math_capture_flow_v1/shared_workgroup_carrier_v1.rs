//! One shared reference edge around an existing single-Workgroup-leaf route.
//! This describes transport only; initialization and every use stay in the
//! common borrow graph and original SSA/partial-move/lifetime validation.
use super::super::matrix_access_borrow::shared_pointee;
use super::*;

#[derive(Clone, Copy)]
pub(super) struct SharedCarrier {
    pub(super) pointee: SemanticTypeIdV1,
    pub(super) route: Route,
}

pub(super) fn potential(callable: &SemanticCallableDeclV1) -> bool {
    matches!(callable, SemanticCallableDeclV1::CompilerIntrinsic {
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }, ..
    } if matches!(contract.operation(), E::SubgroupDeriveBorrowed { width: 64, .. }))
}

pub(super) fn pair(
    types: &[SemanticTypeDeclV1],
    callable: &SemanticCallableDeclV1,
) -> Option<(SemanticTypeIdV1, SemanticTypeIdV1)> {
    let contract = contract(callable)?;
    let E::SubgroupDeriveBorrowed {
        workgroup_reference,
        workgroup,
        subgroup,
        width: 64,
    } = contract.operation()
    else {
        return None;
    };
    (shared_pointee(types, workgroup_reference) == Some(workgroup)
        && contract.signature().arguments().eq([workgroup_reference])
        && contract.signature().output() == subgroup
        && contract.epoch_after().is_none())
    .then_some((workgroup_reference, workgroup))
}

pub(super) fn lookup_work(len: usize) -> usize {
    1 + (usize::BITS - len.leading_zeros()) as usize
}

impl Routes<'_> {
    pub(in super::super) fn observation_body_matches(&self, body: &SemanticFunctionDeclV1) -> bool {
        std::ptr::eq(self.function, body)
    }

    /// Render existing choices only. Never rerun the route or shared-pair checks.
    pub(in super::super) fn write_type_observation(
        &self,
        out: &mut impl std::io::Write,
        ty: SemanticTypeIdV1,
        remaining: &mut usize,
    ) -> std::io::Result<bool> {
        let cost = 8 + 4 * MAX_FIELDS + 5 * 8 + lookup_work(self.routes.len())
            + lookup_work(self.shared.len()) + lookup_work(self.secondaries.len())
            + lookup_work(self.failed_secondaries.len());
        let Some(left) = remaining.checked_sub(cost) else {
            *remaining = 0;
            return Ok(false);
        };
        *remaining = left;
        writeln!(out, "carrier-type ty={} selected_route={:?} shared_route={:?} secondary_roles={:?}",
            ty.index(), self.routes.get(&ty),
            self.shared.get(&ty).map(|c| (c.pointee.index(), c.route)), self.secondaries.get(&ty))?;
        let carrier = self.shared.get(&ty).map_or(ty, |shared| shared.pointee);
        writeln!(out, "carrier-incomplete-secondary={}", self.failed_secondaries.contains(&carrier))?;
        let Some(decl) = self.types.get(ty.index() as usize) else {
            writeln!(out, "carrier-shape missing=true")?;
            return Ok(true);
        };
        writeln!(out, "carrier-shape kind={:?}", std::mem::discriminant(decl.shape()))?;
        if let Some(fields) = fields(self.types, ty) {
            writeln!(out, "carrier-fields count={} prefix={:?} truncated={}", fields.len(),
                &fields[..fields.len().min(MAX_FIELDS)], fields.len() > MAX_FIELDS)?;
            for (index, field) in fields.iter().take(4).enumerate() {
                let pointer = self.types.get(field.index() as usize).and_then(|decl| match decl.shape() {
                    SemanticTypeShapeV1::Pointer(p) => Some(p),
                    _ => None,
                });
                writeln!(out, "carrier-field index={index} ty={} pointer={pointer:?}", field.index())?;
            }
            writeln!(out, "carrier-field-details truncated={}", fields.len() > 4)?;
        }
        if let SemanticTypeShapeV1::Pointer(pointer) = decl.shape() {
            writeln!(out, "carrier-pointer kind={:?} mutability={:?} pointee={} space={} bits={} metadata={:?}",
                pointer.kind(), pointer.mutability(), pointer.pointee().index(),
                pointer.address_space(), pointer.pointer_width_bits(), pointer.metadata())?;
        }
        Ok(true)
    }

    pub(super) fn add_workgroup_routes(
        &mut self,
        local_types: &BTreeSet<SemanticTypeIdV1>,
        pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
        barriers: &BTreeSet<SemanticTypeIdV1>,
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        if pairs.is_empty() {
            return Ok(());
        }
        budget.charge(MAX_FIELDS)?;
        for &ty in local_types {
            budget.charge(1 + lookup_work(self.routes.len()))?;
            self.add_workgroup_type(ty, pairs, barriers, budget)?;
        }
        Ok(())
    }

    pub(super) fn add_workgroup_type(
        &mut self,
        ty: SemanticTypeIdV1,
        pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
        barriers: &BTreeSet<SemanticTypeIdV1>,
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        // Callers debit the ordered lookup before entering. Primary routes
        // always win; the bounded walk does not depend on other route entries.
        let primary = self.routes.get(&ty).copied();
        if let Some(route) = primary {
            if route.len == 0 { return Ok(()) }
            budget.charge(16 + lookup_work(pairs.len()))?;
            if pairs.get(&route.reference) == Some(&route.owned)
                || shared_pointee(self.types, route.reference) != Some(route.owned)
            { return Ok(()) }
        }
        let mut route = None;
        let complete = policy_carrier_v1::walk(
            self.types, pairs, &BTreeMap::new(), barriers, ty,
            &mut [0; MAX_FIELDS], 0, &mut 0, &mut route,
            &mut policy_carrier_v1::Selection::default(), budget,
        )?;
        if !complete && primary.is_some() && route.is_some() {
            self.fail_secondary(ty, budget)?;
        }
        if complete {
            if let Some(route) = route {
                if let Some(primary) = primary {
                    return self.add_secondary(ty, primary, SecondaryRole::Workgroup, route, budget);
                }
                budget.charge(MAX_FIELDS + 4 + lookup_work(self.routes.len()))?;
                self.routes.insert(ty, route);
            }
        }
        Ok(())
    }

    pub(super) fn add_shared_workgroups(
        &mut self,
        local_types: &BTreeSet<SemanticTypeIdV1>,
        pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        if pairs.is_empty() {
            return Ok(());
        }
        budget.charge(3)?;
        for &ty in local_types {
            budget.charge(8)?;
            let Some(pointee) = shared_pointee(self.types, ty) else {
                continue;
            };
            budget.charge(
                2 * lookup_work(self.routes.len())
                    + lookup_work(self.shared.len())
                    + lookup_work(pairs.len()),
            )?;
            // Existing direct routes have priority. Never recursively wrap an
            // already shared carrier or infer a route through arbitrary pointers.
            if self.routes.contains_key(&ty) || self.shared.contains_key(&ty) {
                continue;
            }
            if !self.secondaries.is_empty() {
                budget.charge(lookup_work(self.secondaries.len()))?;
            }
            let Some(route) = self.secondaries.get(&pointee)
                .and_then(|roles| roles[SecondaryRole::Workgroup.slot()].as_ref().map(|(_, route)| route))
                .or_else(|| self.routes.get(&pointee)) else {
                continue;
            };
            if route.len == 0
                || route.len >= MAX_FIELDS
                || pairs.get(&route.reference) != Some(&route.owned)
            {
                continue;
            }
            budget.charge(MAX_FIELDS + 6)?;
            self.shared.insert(
                ty,
                SharedCarrier {
                    pointee,
                    route: *route,
                },
            );
        }
        Ok(())
    }

    pub(in super::super) fn shared_owned(
        &self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<Option<SemanticTypeIdV1>, ProductionSemanticSsaErrorV1> {
        if self.shared.is_empty() {
            return Ok(None);
        }
        budget.charge(1 + lookup_work(self.shared.len()))?;
        Ok(self.shared.get(&ty).map(|carrier| carrier.route.owned))
    }

    pub(super) fn route_for(
        &self,
        ty: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<Option<(Route, Option<SemanticTypeIdV1>)>, ProductionSemanticSsaErrorV1> {
        if let Some(route) = self.routes.get(&ty) {
            return Ok(Some((*route, None)));
        }
        if self.shared.is_empty() {
            return Ok(None);
        }
        budget.charge(lookup_work(self.shared.len()))?;
        let Some(carrier) = self.shared.get(&ty) else {
            return Ok(None);
        };
        budget.charge(MAX_FIELDS + 5)?;
        Ok(Some((carrier.route, Some(carrier.pointee))))
    }
}

#[cfg(test)]
#[path = "shared_workgroup_carrier_v1/reuse_tests.rs"]
mod reuse_tests;
