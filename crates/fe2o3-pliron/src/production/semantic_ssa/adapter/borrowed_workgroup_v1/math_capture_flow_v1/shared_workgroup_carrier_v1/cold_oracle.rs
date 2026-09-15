// Test-only pre80 oracle, mechanically extracted from the pinned two parents.
// Only method names, receiver spelling and the primary return type changed.
// The old Policy helper is pinned below; production now collects later leaves.
fn cold_primary<'a>(
    function: &'a SemanticFunctionDeclV1,
    types: &'a [SemanticTypeDeclV1],
    pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    policy: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    barriers: &BTreeSet<SemanticTypeIdV1>,
    budget: &mut Budget,
) -> Result<Routes<'a>, ProductionSemanticSsaErrorV1> {
    let mut result = Routes {
        function,
        types,
        routes: BTreeMap::new(),
        shared: BTreeMap::new(),
        secondaries: BTreeMap::new(),
        failed_secondaries: BTreeSet::new(),
    };
    if pairs.is_empty() && policy.is_empty() {
        return Ok(result);
    }
    budget.charge(MAX_FIELDS)?;
    let mut seen = BTreeSet::new();
    for local in function.locals() {
        budget.charge(1)?;
        if !seen.insert(local.ty()) {
            continue;
        }
        // These are logical work/state units, not a physical allocator cap.
        budget.charge(1)?;
        let mut route = None;
        let mut secondary = ColdPolicySelection::default();
        let mut fields = [0; MAX_FIELDS];
        let mut nodes = 0;
        if cold_policy_walk(
            types,
            pairs,
            policy,
            barriers,
            local.ty(),
            &mut fields,
            0,
            &mut nodes,
            &mut route,
            &mut secondary,
            budget,
        )? {
            if let Some(route) = route.or_else(|| secondary.unique()) {
                budget.charge(MAX_FIELDS + 4)?;
                result.routes.insert(local.ty(), route);
            }
        }
    }
    Ok(result)
}

fn cold_workgroup(
    routes: &mut Routes<'_>,
    pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    barriers: &BTreeSet<SemanticTypeIdV1>,
    budget: &mut Budget,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    if pairs.is_empty() {
        return Ok(());
    }
    budget.charge(MAX_FIELDS + 3)?;
    let mut seen = BTreeSet::new();
    for local in routes.function.locals() {
        budget.charge(2 + lookup_work(seen.len()) + lookup_work(routes.routes.len()))?;
        // Never reinterpret a pre-existing Math, matrix, policy or phase
        // route. Only types without a prior route enter this bounded walk.
        if !seen.insert(local.ty()) || routes.routes.contains_key(&local.ty()) {
            continue;
        }
        let mut route = None;
        if cold_policy_walk(
            routes.types,
            pairs,
            &BTreeMap::new(),
            barriers,
            local.ty(),
            &mut [0; MAX_FIELDS],
            0,
            &mut 0,
            &mut route,
            &mut ColdPolicySelection::default(),
            budget,
        )? {
            if let Some(route) = route {
                budget.charge(MAX_FIELDS + 4 + lookup_work(routes.routes.len()))?;
                routes.routes.insert(local.ty(), route);
            }
        }
    }
    Ok(())
}

fn cold_shared(
    routes: &mut Routes<'_>,
    pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    budget: &mut Budget,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    if pairs.is_empty() {
        return Ok(());
    }
    budget.charge(3)?;
    for local in routes.function.locals() {
        budget.charge(8)?;
        let Some(pointee) = shared_pointee(routes.types, local.ty()) else {
            continue;
        };
        budget.charge(
            2 * lookup_work(routes.routes.len())
                + lookup_work(routes.shared.len())
                + lookup_work(pairs.len()),
        )?;
        // Existing direct routes have priority. Never recursively wrap an
        // already shared carrier or infer a route through arbitrary pointers.
        if routes.routes.contains_key(&local.ty()) || routes.shared.contains_key(&local.ty()) {
            continue;
        }
        let Some(route) = routes.routes.get(&pointee) else {
            continue;
        };
        if route.len == 0
            || route.len >= MAX_FIELDS
            || pairs.get(&route.reference) != Some(&route.owned)
        {
            continue;
        }
        budget.charge(MAX_FIELDS + 6)?;
        routes.shared.insert(
            local.ty(),
            SharedCarrier {
                pointee,
                route: *route,
            },
        );
    }
    Ok(())
}

#[derive(Default)]
struct ColdPolicySelection {
    first: Option<Route>,
    ambiguous: bool,
}

impl ColdPolicySelection {
    fn unique(self) -> Option<Route> {
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

// Frozen pre118 policy_carrier_v1::walk, including the primary short circuit.
#[allow(clippy::too_many_arguments)]
fn cold_policy_walk(
    types: &[SemanticTypeDeclV1],
    primary: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    policy: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    barriers: &BTreeSet<SemanticTypeIdV1>,
    ty: SemanticTypeIdV1,
    path: &mut [u32; MAX_FIELDS],
    depth: usize,
    nodes: &mut usize,
    found: &mut Option<Route>,
    secondary: &mut ColdPolicySelection,
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
    // Once a primary leaf exists, secondary selection cannot affect the result.
    // Still visit every remaining structural node and enforce all old barriers.
    if found.is_none() && !policy.is_empty() {
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
        if !cold_policy_walk(
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
