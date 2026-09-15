//! Typed carrier edges in the existing borrow-use graph, never issuer evidence.

use super::*;
use std::borrow::Cow;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateKindV1, SemanticAssignmentV1, SemanticMutabilityV1, SemanticPointerKindV1,
    SemanticPointerMetadataV1, SemanticTypeShapeV1,
};

#[path = "math_capture_flow_v1/policy_carrier_v1.rs"]
mod policy_carrier_v1;
#[path = "math_capture_flow_v1/shared_workgroup_carrier_v1.rs"]
mod shared_workgroup_carrier_v1;
#[path = "math_capture_flow_v1/workgroup_role_v1.rs"]
mod workgroup_role_v1;
use shared_workgroup_carrier_v1::SharedCarrier;

const MAX_FIELDS: usize = 16;
const MAX_SHAPE_NODES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Route {
    reference: SemanticTypeIdV1,
    owned: SemanticTypeIdV1,
    fields: [u32; MAX_FIELDS],
    len: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SecondaryRole {
    Policy,
    Workgroup,
}

impl SecondaryRole {
    fn slot(self) -> usize {
        match self { Self::Policy => 0, Self::Workgroup => 1 }
    }
}

pub(super) type CheckedFields = [Option<(SecondaryRole, usize)>; 2];
type SecondaryRoutes = [Option<(SecondaryRole, Route)>; 2];

/// One primary route and at most one unique Policy and Workgroup secondary.
/// Every participating field enters the same all-use graph.
pub(super) struct Routes<'a> {
    function: &'a SemanticFunctionDeclV1,
    types: &'a [SemanticTypeDeclV1],
    routes: BTreeMap<SemanticTypeIdV1, Route>,
    shared: BTreeMap<SemanticTypeIdV1, SharedCarrier>,
    secondaries: BTreeMap<SemanticTypeIdV1, SecondaryRoutes>,
    failed_secondaries: BTreeSet<SemanticTypeIdV1>,
}

impl<'a> Routes<'a> {
    pub(super) fn new(
        function: &'a SemanticFunctionDeclV1,
        types: Option<&'a [SemanticTypeDeclV1]>,
        callables: &[SemanticCallableDeclV1],
        budget: &mut Budget,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        Self::new_with_matrix(function, types, callables, &MatrixBorrowSitesV1::default(), budget)
    }

    pub(super) fn new_with_matrix(
        function: &'a SemanticFunctionDeclV1,
        types: Option<&'a [SemanticTypeDeclV1]>,
        callables: &[SemanticCallableDeclV1],
        matrix: &MatrixBorrowSitesV1<'_>,
        budget: &mut Budget,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        Self::new_with_matrix_and_leaves(function, types, callables, matrix, std::iter::empty(), budget)
    }
    pub(super) fn new_with_matrix_and_leaves(
        function: &'a SemanticFunctionDeclV1,
        types: Option<&'a [SemanticTypeDeclV1]>,
        callables: &[SemanticCallableDeclV1],
        matrix: &MatrixBorrowSitesV1<'_>,
        extra: impl IntoIterator<Item = (SemanticTypeIdV1, SemanticTypeIdV1)>,
        budget: &mut Budget,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        let result = Self {
            function,
            types: types.unwrap_or_default(),
            routes: BTreeMap::new(),
            shared: BTreeMap::new(),
            secondaries: BTreeMap::new(),
            failed_secondaries: BTreeSet::new(),
        };
        let Some(types) = types else {
            return Ok(result);
        };
        let mut pairs = BTreeMap::new();
        let mut workgroups = BTreeMap::new();
        let mut barriers = Cow::Borrowed(matrix.carrier_barriers());
        budget.charge(callables.len())?;
        for callable in callables {
            if shared_workgroup_carrier_v1::potential(callable) {
                budget.charge(16)?;
                if let Some((reference, owned)) = shared_workgroup_carrier_v1::pair(types, callable) {
                    budget.charge(5 + (usize::BITS - workgroups.len().leading_zeros()) as usize)?;
                    if workgroups.insert(reference, owned).is_some_and(|old| old != owned)
                    {
                        return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
                    }
                }
            }
            if !matches!(
                callable,
                SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { .. },
                    ..
                }
            ) {
                continue;
            }
            budget.charge(16)?;
            if let Some(fact) = MathConsumerBorrowV1::for_callable(types, callable) {
                let (reference, owned) = fact.pair();
                budget.charge(2)?;
                if pairs
                    .insert(reference, owned)
                    .is_some_and(|old| old != owned)
                {
                    return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
                }
                // A checked Math bound is an owned capability, not an ordinary
                // carrier of its internal Policy reference. Its exact Bind
                // retains both loans; only &bound may use the generic route.
                let copy_work = if matches!(barriers, Cow::Borrowed(_)) {
                    3 + barriers.len().saturating_mul(2)
                } else { 0 };
                budget.charge(copy_work + 2 + shared_workgroup_carrier_v1::lookup_work(barriers.len()))?;
                barriers.to_mut().insert(owned);
            }
        }
        for (&reference, &owned) in matrix.carrier_leaves() {
            // Logical ordered-map operations plus retained pair storage.
            budget.charge(5 + (usize::BITS - pairs.len().leading_zeros()) as usize)?;
            if pairs.insert(reference, owned).is_some_and(|old| old != owned) {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
        }
        for (reference, owned) in extra {
            budget.charge(5 + (usize::BITS - pairs.len().leading_zeros()) as usize)?;
            if pairs.insert(reference, owned).is_some_and(|old| old != owned) {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
        }
        Self::from_pairs_with_workgroups(function, types, &pairs, matrix.policy_carrier_leaves(),
            &barriers, &workgroups, budget)
    }

    pub(super) fn from_pairs(
        function: &'a SemanticFunctionDeclV1,
        types: &'a [SemanticTypeDeclV1],
        pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
        barriers: &BTreeSet<SemanticTypeIdV1>,
        budget: &mut Budget,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        Self::from_pairs_with_policy(function, types, pairs, &BTreeMap::new(), barriers, budget)
    }

    fn from_pairs_with_policy(
        function: &'a SemanticFunctionDeclV1,
        types: &'a [SemanticTypeDeclV1],
        pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
        policy: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
        barriers: &BTreeSet<SemanticTypeIdV1>,
        budget: &mut Budget,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        Self::from_pairs_with_workgroups(function, types, pairs, policy, barriers, &BTreeMap::new(), budget)
    }

    fn from_pairs_with_workgroups(
        function: &'a SemanticFunctionDeclV1,
        types: &'a [SemanticTypeDeclV1],
        pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
        policy: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
        barriers: &BTreeSet<SemanticTypeIdV1>,
        workgroups: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
        budget: &mut Budget,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        let mut result = Self { function, types, routes: BTreeMap::new(), shared: BTreeMap::new(),
            secondaries: BTreeMap::new(), failed_secondaries: BTreeSet::new() };
        let primary = !pairs.is_empty() || !policy.is_empty();
        if !primary && workgroups.is_empty() { return Ok(result) }
        // Reuse the set already built by the primary walk (or the unchanged
        // Workgroup-only walk). No second index, local roster or capacity copy.
        budget.charge(MAX_FIELDS + if primary { 0 } else { 3 })?;
        let mut seen = BTreeSet::new();
        for local in function.locals() {
            // Preserve the existing primary or Workgroup-only census debit.
            budget.charge(if primary { 1 } else {
                2 + shared_workgroup_carrier_v1::lookup_work(seen.len())
                    + shared_workgroup_carrier_v1::lookup_work(result.routes.len())
            })?;
            if !seen.insert(local.ty()) {
                continue;
            }
            if !primary {
                result.add_workgroup_type(local.ty(), workgroups, barriers, budget)?;
                continue;
            }
            // These are logical work/state units, not a physical allocator cap.
            budget.charge(1)?;
            let mut route = None;
            let mut secondary = policy_carrier_v1::Selection::default();
            let mut fields = [0; MAX_FIELDS];
            let mut nodes = 0;
            let complete = policy_carrier_v1::walk(
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
            )?;
            if (!complete && !workgroups.is_empty() && (route.is_some() || secondary.observed()))
                || (secondary.ambiguous() && (route.is_some() || !workgroups.is_empty()))
            {
                result.fail_secondary(local.ty(), budget)?;
            }
            if complete {
                let policy = secondary.unique();
                if let Some(route) = route.or(policy) {
                    budget.charge(MAX_FIELDS + 4)?;
                    result.routes.insert(local.ty(), route);
                    if let Some(policy) = policy.filter(|policy| *policy != route) {
                        result.add_secondary(local.ty(), route, SecondaryRole::Policy, policy, budget)?;
                    }
                }
            }
        }
        if primary {
            result.add_workgroup_routes(&seen, workgroups, barriers, budget)?;
        }
        result.add_shared_workgroups(&seen, workgroups, budget)?;
        Ok(result)
    }

    pub(super) fn reference(&self, ty: SemanticTypeIdV1) -> Option<SemanticTypeIdV1> {
        self.routes.get(&ty).map(|route| route.reference)
    }

    pub(super) fn owned(&self, ty: SemanticTypeIdV1) -> Option<&SemanticTypeIdV1> {
        self.routes.get(&ty).map(|route| &route.owned)
    }

    /// Exact shared-leaf extraction is an edge, not initialization evidence.
    /// Original Move events still require the planner's partial-move certificate.
    pub(super) fn source<'b>(
        &self,
        assignment: &'b SemanticAssignmentV1,
        budget: &mut Budget,
    ) -> Result<Option<&'b SemanticPlaceV1>, ProductionSemanticSsaErrorV1> {
        #[cfg(test)]
        super::tests::candidate_source_tests::source_query(budget.profile.stage);
        if !assignment.destination().projections().is_empty()
            || assignment.destination().ty() != assignment.value().result_type()
        {
            return Ok(None);
        }
        let Some((route, shared_carrier)) = self.route_for(assignment.destination().ty(), budget)? else {
            return Ok(None);
        };
        self.source_for(assignment, route, shared_carrier, budget)
    }

    fn source_for<'b>(
        &self,
        assignment: &'b SemanticAssignmentV1,
        route: Route,
        shared_carrier: Option<SemanticTypeIdV1>,
        budget: &mut Budget,
    ) -> Result<Option<&'b SemanticPlaceV1>, ProductionSemanticSsaErrorV1> {
        let (operand, tail) = match assignment.value().kind() {
            SemanticRvalueKindV1::Borrow { kind: SemanticBorrowKindV1::Shared, place }
                if shared_carrier == Some(place.ty())
                    && (place.projections().is_empty() || matches!(place.projections(),
                        [deref] if deref.kind() == SemanticProjectionKindV1::Dereference)) =>
            {
                // The selected carrier is borrowed, not copied or initialized.
                // Its existing component parent and the original SSA lifetime
                // checks still govern whether this exact Borrow is transparent.
                return self.matches_source(place, route, budget);
            }
            SemanticRvalueKindV1::Use(operand) => {
                if operand.ty() != assignment.value().result_type() {
                    return Ok(None);
                }
                // The existing direct-reference classifier owns these edges.
                if route.len == 0
                    && matches!(operand,
                    SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) if p.projections().is_empty())
                {
                    return Ok(None);
                }
                (operand, route)
            }
            SemanticRvalueKindV1::Aggregate(aggregate) if route.len > 0 => {
                let Some(fields) = fields(self.types, assignment.destination().ty()) else {
                    return Ok(None);
                };
                let exact_kind = matches!(
                    (
                        self.types[assignment.destination().ty().index() as usize].shape(),
                        aggregate.kind()
                    ),
                    (
                        SemanticTypeShapeV1::Tuple(_),
                        SemanticAggregateKindV1::Tuple
                    ) | (
                        SemanticTypeShapeV1::Aggregate(_),
                        SemanticAggregateKindV1::Aggregate
                    )
                );
                budget.charge(fields.len().saturating_add(aggregate.operands().len()))?;
                if !exact_kind
                    || fields.len() != aggregate.operands().len()
                    || !fields
                        .iter()
                        .copied()
                        .eq(aggregate.operands().iter().map(SemanticOperandV1::ty))
                {
                    return Ok(None);
                }
                let Some(operand) = aggregate.operands().get(route.fields[0] as usize) else {
                    return Ok(None);
                };
                let mut tail = route;
                tail.fields.copy_within(1..route.len, 0);
                tail.len -= 1;
                (operand, tail)
            }
            _ => return Ok(None),
        };
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
            return Ok(None);
        };
        if matches!(operand, SemanticOperandV1::Move(_)) && !place.projections().is_empty() {
            budget.charge(1)?;
            // Do not admit moved sub-carriers, aggregate operands, raw pointers,
            // or exclusive references through this shared-leaf exception.
            if route.len != 0
                || !matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(_))
                || !matches!(self.types.get(route.reference.index() as usize).map(SemanticTypeDeclV1::shape),
                    Some(SemanticTypeShapeV1::Pointer(p))
                        if p.kind() == SemanticPointerKindV1::Reference
                            && p.mutability() == SemanticMutabilityV1::Immutable
                            && p.pointee() == route.owned
                            && p.address_space() == 0 && p.pointer_width_bits() == 64
                            && p.metadata() == SemanticPointerMetadataV1::None)
            {
                return Ok(None);
            }
        }
        self.matches_source(place, tail, budget)
    }

    fn matches_source<'b>(
        &self,
        place: &'b SemanticPlaceV1,
        tail: Route,
        budget: &mut Budget,
    ) -> Result<Option<&'b SemanticPlaceV1>, ProductionSemanticSsaErrorV1> {
        let Some((source, projected, len)) = self.projected_role(place, Some(tail.reference), budget)? else {
            return Ok(None);
        };
        Ok((source.reference == tail.reference
            && source.owned == tail.owned
            && len <= source.len
            && source.fields[..len] == projected[..len]
            && source.fields[len..source.len] == tail.fields[..tail.len])
            .then_some(place))
    }

    /// Do not hide other capability-reference operands in a captured aggregate.
    pub(super) fn invalidate_siblings(
        &self,
        assignment: &SemanticAssignmentV1,
        by_reference: &BTreeMap<u32, usize>,
        candidates: &mut [SemanticBorrowCandidateV1],
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        self.invalidate_siblings_checked(assignment, by_reference, candidates, [None; 2], budget)
    }

    pub(super) fn invalidate_siblings_checked(
        &self,
        assignment: &SemanticAssignmentV1,
        by_reference: &BTreeMap<u32, usize>,
        candidates: &mut [SemanticBorrowCandidateV1],
        checked_fields: CheckedFields,
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        self.invalidate_siblings_with_global(assignment, by_reference, candidates,
            checked_fields, None, budget)
    }

    pub(super) fn invalidate_siblings_with_global(
        &self,
        assignment: &SemanticAssignmentV1,
        by_reference: &BTreeMap<u32, usize>,
        candidates: &mut [SemanticBorrowCandidateV1],
        checked_fields: CheckedFields,
        mut global: Option<&mut global_carrier_flow_v1::Transport<'_, '_>>,
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        if let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() {
            let route = self
                .routes
                .get(&assignment.destination().ty())
                .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
            budget.charge(aggregate.operands().len().saturating_mul(3))?;
            for (field, operand) in aggregate.operands().iter().enumerate() {
                if field != route.fields[0] as usize
                    && !checked_fields.iter().flatten().any(|(_, checked)| *checked == field)
                {
                    if let Some(transport) = global.as_mut()
                        && let SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) = operand
                    {
                        budget.charge(shared_workgroup_carrier_v1::lookup_work(by_reference.len()))?;
                        if let Some(&parent) = by_reference.get(&place.local().index())
                            && transport.defer_leaf(assignment, field, place.ty(), candidates[parent].source_type, budget)?
                        {
                            // Publication remains pending until the complete
                            // Global audit validates this exact operand's group.
                            continue;
                        }
                    }
                    // An ambiguous or unaccounted role cannot leave the primary
                    // carrier publishable, even when no secondary was checked.
                    if let SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) = operand {
                        budget.charge(2 * shared_workgroup_carrier_v1::lookup_work(by_reference.len()))?;
                        if by_reference.contains_key(&place.local().index())
                            && let Some(&destination) = by_reference.get(&assignment.destination().local().index())
                        { candidates[destination].valid = false; }
                    }
                    invalidate_reference_operand_v1(operand, by_reference, candidates);
                }
            }
        }
        Ok(())
    }

    pub(super) fn resolve_global_siblings(
        fields: &[global_carrier_flow_v1::FieldCoverage<'_>],
        by_reference: &BTreeMap<u32, usize>,
        candidates: &mut [SemanticBorrowCandidateV1],
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        for field in fields {
            budget.charge(1)?;
            if field.closed() { continue }
            budget.charge(3 + 2 * shared_workgroup_carrier_v1::lookup_work(by_reference.len()))?;
            let assignment = field.assignment();
            let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            };
            let operand = aggregate.operands().get(field.field())
                .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
            invalidate_reference_operand_v1(operand, by_reference, candidates);
            if let Some(&destination) = by_reference.get(&assignment.destination().local().index()) {
                candidates[destination].valid = false;
            }
        }
        Ok(())
    }

    /// Copying or moving a typed disjoint field touches no checked role.
    /// This grants nothing to the sibling; original Move events and the
    /// planner's partial-move certificate still govern initialization and reuse.
    pub(super) fn disjoint_field_use(
        &self,
        assignment: &SemanticAssignmentV1,
        budget: &mut Budget,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        let SemanticRvalueKindV1::Use(
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
        ) = assignment.value().kind()
        else {
            return Ok(false);
        };
        if assignment.destination().ty() != assignment.value().result_type()
            || assignment.value().result_type() != place.ty()
        {
            return Ok(false);
        }
        let Some((route, projected, len)) = self.projected(place, budget)? else {
            return Ok(false);
        };
        if route.fields[..route.len.min(len)] == projected[..route.len.min(len)] {
            return Ok(false);
        }
        let Some(local) = self.function.locals().get(place.local().index() as usize) else {
            return Ok(false);
        };
        for (other, _) in self.alternates_for(local.ty(), budget)?.into_iter().flatten() {
            let common = other.len.min(len);
            budget.charge(1 + common)?;
            if other.fields[..common] == projected[..common] { return Ok(false) }
        }
        Ok(true)
    }

    fn projected(
        &self,
        place: &SemanticPlaceV1,
        budget: &mut Budget,
    ) -> Result<Option<(Route, [u32; MAX_FIELDS], usize)>, ProductionSemanticSsaErrorV1> {
        self.projected_role(place, None, budget)
    }

    fn projected_role(
        &self,
        place: &SemanticPlaceV1,
        reference: Option<SemanticTypeIdV1>,
        budget: &mut Budget,
    ) -> Result<Option<(Route, [u32; MAX_FIELDS], usize)>, ProductionSemanticSsaErrorV1> {
        if place.projections().len() > MAX_FIELDS {
            return Ok(None);
        }
        budget.charge(place.projections().len().saturating_add(1))?;
        let Some(local) = self.function.locals().get(place.local().index() as usize) else {
            return Ok(None);
        };
        let Some((mut route, mut shared_carrier)) = self.route_for(local.ty(), budget)? else {
            return Ok(None);
        };
        if reference.is_some_and(|reference| reference != route.reference) {
            let Some(other) = self.alternates_for(local.ty(), budget)?.into_iter().flatten()
                .find(|(route, _)| Some(route.reference) == reference)
            else { return Ok(None) };
            (route, shared_carrier) = other;
        }
        let mut ty = local.ty();
        let projections = if let (Some(pointee), Some((first, rest))) =
            (shared_carrier, place.projections().split_first())
        {
            if first.kind() != SemanticProjectionKindV1::Dereference
                || first.result_type() != pointee
            { return Ok(None) }
            ty = pointee;
            rest
        } else { place.projections() };
        let mut projected = [0; MAX_FIELDS];
        for (slot, projection) in projected.iter_mut().zip(projections) {
            let SemanticProjectionKindV1::Field(field) = projection.kind() else {
                return Ok(None);
            };
            let Some(next) = fields(self.types, ty).and_then(|fields| fields.get(field as usize))
            else {
                return Ok(None);
            };
            if *next != projection.result_type() {
                return Ok(None);
            }
            ty = *next;
            *slot = field;
        }
        Ok((ty == place.ty()).then_some((route, projected, projections.len())))
    }
}

fn fields(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> Option<&[SemanticTypeIdV1]> {
    match types.get(ty.index() as usize)?.shape() {
        SemanticTypeShapeV1::Tuple(tuple) => Some(tuple.fields()),
        SemanticTypeShapeV1::Aggregate(aggregate) => Some(aggregate.fields()),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn walk(
    types: &[SemanticTypeDeclV1],
    pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    ty: SemanticTypeIdV1,
    path: &mut [u32; MAX_FIELDS],
    depth: usize,
    nodes: &mut usize,
    found: &mut Option<Route>,
    budget: &mut Budget,
) -> Result<bool, ProductionSemanticSsaErrorV1> {
    walk_with_barriers(types, pairs, &BTreeSet::new(), ty, path, depth, nodes, found, budget)
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn walk_with_barriers(
    types: &[SemanticTypeDeclV1],
    pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    barriers: &BTreeSet<SemanticTypeIdV1>,
    ty: SemanticTypeIdV1,
    path: &mut [u32; MAX_FIELDS],
    depth: usize,
    nodes: &mut usize,
    found: &mut Option<Route>,
    budget: &mut Budget,
) -> Result<bool, ProductionSemanticSsaErrorV1> {
    policy_carrier_v1::walk(types, pairs, &BTreeMap::new(), barriers, ty, path, depth,
        nodes, found, &mut policy_carrier_v1::Selection::default(), budget)
}

#[cfg(test)]
#[path = "math_capture_flow_v1/tests.rs"]
mod tests;
