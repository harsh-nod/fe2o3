//! Exact replayed Bind policy leaves in the existing single-owner carrier walk.
//! This is not a numerical-policy default or issuer; prior routes take priority.
use super::*;

#[derive(Default)]
pub(super) struct Selection {
    first: Option<Route>,
    ambiguous: bool,
}

impl Selection {
    pub(super) fn observed(&self) -> bool {
        self.first.is_some()
    }

    pub(super) fn ambiguous(&self) -> bool {
        self.ambiguous
    }

    pub(super) fn unique(self) -> Option<Route> {
        (!self.ambiguous).then_some(self.first).flatten()
    }

    fn observe(&mut self, route: Route) {
        if self.first.is_some() {
            self.ambiguous = true;
        } else {
            self.first = Some(route);
        }
    }
}

// One traversal and the original node/depth guards. Multiple secondary leaves
// cannot stop a later primary leaf from retaining its unchanged interpretation.
#[allow(clippy::too_many_arguments)]
pub(super) fn walk(
    types: &[SemanticTypeDeclV1],
    primary: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    policy: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    barriers: &BTreeSet<SemanticTypeIdV1>,
    ty: SemanticTypeIdV1,
    path: &mut [u32; MAX_FIELDS],
    depth: usize,
    nodes: &mut usize,
    found: &mut Option<Route>,
    secondary: &mut Selection,
    budget: &mut Budget,
) -> Result<bool, ProductionSemanticSsaErrorV1> {
    if *nodes == MAX_SHAPE_NODES {
        return Ok(false);
    }
    budget.charge(1)?;
    *nodes += 1;
    if !barriers.is_empty() {
        budget.charge(1 + (usize::BITS - barriers.len().leading_zeros()) as usize)?;
        if barriers.contains(&ty) {
            return Ok(false);
        }
    }
    if let Some(&owned) = primary.get(&ty) {
        return Ok(found
            .replace(Route {
                reference: ty,
                owned,
                fields: *path,
                len: depth,
            })
            .is_none());
    }
    // A later Policy leaf is still a custody obligation. Repeated occurrences
    // remain ambiguous regardless of which field contained the primary.
    if !policy.is_empty() {
        budget.charge(1 + (usize::BITS - policy.len().leading_zeros()) as usize)?;
        if let Some(&owned) = policy.get(&ty) {
            // Logical route copy/state, not a physical allocator size claim.
            budget.charge(MAX_FIELDS + 4)?;
            secondary.observe(Route {
                reference: ty,
                owned,
                fields: *path,
                len: depth,
            });
            return Ok(true);
        }
    }
    let Some(fields) = fields(types, ty) else {
        return Ok(true);
    };
    if !fields.is_empty() && depth == MAX_FIELDS {
        return Ok(false);
    }
    for (index, ty) in fields.iter().enumerate() {
        path[depth] = index as u32;
        if !walk(
            types,
            primary,
            policy,
            barriers,
            *ty,
            path,
            depth + 1,
            nodes,
            found,
            secondary,
            budget,
        )? {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
#[path = "policy_carrier_v1/tests.rs"]
mod tests;
