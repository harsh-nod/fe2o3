//! Closed transport of shared Global addresses, not Global origin or loan proof.
//!
//! A carrier may contain several Globals. They share only a conservative escape
//! audit: one bad use poisons the connected group. No owner/value is selected or
//! substituted here; the retained source SSA and lowerer still resolve each field.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateKindV1, SemanticAssignmentV1, SemanticLocalDeclV1,
};

// Retain the unchanged Shapes89 implementation and its tests as a cold oracle.
#[cfg(test)]
mod shapes;
mod shapes_indexed;
use shapes_indexed::Shapes;
mod relevance;
use relevance::Relevance;
mod coverage;
pub(super) use coverage::{FieldCoverage, Transport};

#[cfg(test)]
mod tests;

fn map_work(len: usize) -> usize {
    1 + (usize::BITS - len.leading_zeros()) as usize
}

fn push<T>(
    values: &mut Vec<T>,
    value: T,
    budget: &mut Budget,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    let cells = std::mem::size_of::<T>().div_ceil(std::mem::size_of::<usize>());
    if values.len() == values.capacity() {
        let capacity = values
            .capacity()
            .max(1)
            .checked_mul(2)
            .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
        budget.charge(capacity.saturating_add(values.len()).saturating_mul(cells))?;
        values.reserve_exact(capacity - values.len());
        // reserve_exact promises at least the request, not exact capacity. No
        // successful audit may retain an uncharged allocator surplus. An error
        // drops this allocation immediately and propagates out of the audit.
        if let Err(error) = budget.charge(
            values
                .capacity()
                .saturating_sub(capacity)
                .saturating_mul(cells),
        ) {
            *values = Vec::new();
            return Err(error);
        }
    }
    budget.charge(cells.max(1))?;
    values.push(value);
    Ok(())
}

struct Flow {
    by_local: BTreeMap<u32, usize>,
    nodes: Vec<SemanticBorrowCandidateV1>,
    definitions: Vec<bool>,
    edges: Vec<Vec<usize>>,
    roots: Vec<(usize, SemanticTransparentBorrowSiteV1)>,
}

impl Flow {
    fn index(
        &self,
        local: SemanticLocalIdV1,
        budget: &mut Budget,
    ) -> Result<Option<usize>, ProductionSemanticSsaErrorV1> {
        budget.charge(map_work(self.by_local.len()))?;
        Ok(self.by_local.get(&local.index()).copied())
    }

    fn link(
        &mut self,
        destination: usize,
        source: usize,
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        if destination == source {
            self.nodes[destination].valid = false;
        }
        push(&mut self.edges[destination], source, budget)?;
        push(&mut self.edges[source], destination, budget)
    }

    fn define(&mut self, index: usize) {
        if self.definitions[index] {
            self.nodes[index].valid = false;
        }
        self.definitions[index] = true;
    }

    fn reject(&mut self, statement: &SemanticStatementKindV1) {
        // Sentinel sites never equal a source statement. The common exhaustive
        // invalidator therefore grants no candidate-definition exception here.
        invalidate_reference_uses_in_statement_v1(
            statement,
            SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: 0,
            },
            &self.by_local,
            &mut self.nodes,
        );
    }

    fn finish(
        &mut self,
        budget: &mut Budget,
    ) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>, ProductionSemanticSsaErrorV1> {
        let mut pending = Vec::new();
        for index in 0..self.nodes.len() {
            budget.charge(1)?;
            if !self.definitions[index] {
                self.nodes[index].valid = false;
            }
            if !self.nodes[index].valid {
                push(&mut pending, index, budget)?;
            }
        }
        while let Some(index) = pending.pop() {
            budget.charge(1)?;
            for &neighbor in &self.edges[index] {
                budget.charge(1)?;
                if self.nodes[neighbor].valid {
                    self.nodes[neighbor].valid = false;
                    push(&mut pending, neighbor, budget)?;
                }
            }
        }
        let mut sites = BTreeSet::new();
        for &(index, site) in &self.roots {
            budget.charge(1)?;
            if self.nodes[index].valid {
                budget.charge(3 + map_work(sites.len()))?;
                sites.insert(site);
            }
        }
        Ok(sites)
    }
}

pub(super) struct Audit<'a>(Option<State<'a>>);

struct State<'a> {
    function: &'a SemanticFunctionDeclV1,
    shapes: Shapes<'a>,
    flow: Flow,
    statements: &'a global_statement_index_v1::GlobalStatementIndex<'a>,
    explicit: &'a BTreeSet<SemanticLocalIdV1>,
    facts: usize,
    relevance: Relevance,
    pending_fields: Vec<FieldCoverage<'a>>,
}

impl<'a> Audit<'a> {
    pub(super) fn new(
        function: &'a SemanticFunctionDeclV1,
        types: Option<&'a [SemanticTypeDeclV1]>,
        facts: &'a [GlobalBf16BorrowV1],
        statements: &'a global_statement_index_v1::GlobalStatementIndex<'a>,
        explicit: &'a BTreeSet<SemanticLocalIdV1>,
        budget: &mut Budget,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        let Some(types) = types else {
            return Ok(Self(None));
        };
        if facts.is_empty() {
            return Ok(Self(None));
        }
        let mut shapes = Shapes::new(types, facts, budget)?;
        let mut flow = Flow {
            by_local: BTreeMap::new(),
            nodes: Vec::new(),
            definitions: Vec::new(),
            edges: Vec::new(),
            roots: Vec::new(),
        };
        shapes.for_each_transport_declaration(
            function.locals(),
            budget,
            |local, declaration, selected, budget| {
                if !selected {
                    return Ok(());
                }
                budget.charge(3 + map_work(flow.by_local.len()))?;
                flow.by_local.insert(local as u32, flow.nodes.len());
                push(
                    &mut flow.nodes,
                    SemanticBorrowCandidateV1 {
                        site: SemanticTransparentBorrowSiteV1 {
                            block: u32::MAX,
                            statement: u32::MAX,
                        },
                        source_local: local as u32,
                        source_type: declaration.ty(),
                        source_reference: None,
                        value_alias: true,
                        source_kind: SemanticBorrowCandidateSourceV1::Direct,
                        valid: declaration.role() != SemanticLocalRoleV1::Return,
                        consumers: 0,
                        intrinsic_consumer: false,
                    },
                    budget,
                )?;
                push(
                    &mut flow.definitions,
                    matches!(declaration.role(), SemanticLocalRoleV1::Argument(_)),
                    budget,
                )?;
                push(&mut flow.edges, Vec::new(), budget)?;
                Ok(())
            },
        )?;
        if flow.nodes.is_empty() {
            return Ok(Self(None));
        }
        let relevance = Relevance::new(
            function.locals().len(),
            flow.by_local.keys().copied(),
            budget,
        )?;
        Ok(Self(Some(State {
            function,
            shapes,
            flow,
            statements,
            explicit,
            facts: facts.len(),
            relevance,
            pending_fields: Vec::new(),
        })))
    }

    #[cfg(test)]
    pub(super) fn statement(
        &mut self,
        site: SemanticTransparentBorrowSiteV1,
        source: &SemanticStatementKindV1,
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        self.statement_with_transport(site, source, budget).map(|_| ())
    }

    pub(super) fn statement_with_transport<'s>(
        &'s mut self,
        site: SemanticTransparentBorrowSiteV1,
        source: &'s SemanticStatementKindV1,
        budget: &mut Budget,
    ) -> Result<Option<Transport<'s, 'a>>, ProductionSemanticSsaErrorV1> {
        let Some(state) = self.0.as_ref() else {
            return Ok(None);
        };
        if !state.relevance.statement(source, budget)? {
            return Ok(None);
        }
        let destination = self.audit_statement(site, source, budget)?;
        Ok(destination.and_then(|destination| self.0.as_mut().map(|state| Transport {
            state, site, source, destination,
        })))
    }

    // The unchanged full audit also serves as the differential test oracle.
    #[cfg(test)]
    fn statement_relevant(
        &mut self,
        site: SemanticTransparentBorrowSiteV1,
        source: &SemanticStatementKindV1,
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        self.audit_statement(site, source, budget).map(|_| ())
    }

    fn audit_statement(
        &mut self,
        site: SemanticTransparentBorrowSiteV1,
        source: &SemanticStatementKindV1,
        budget: &mut Budget,
    ) -> Result<Option<usize>, ProductionSemanticSsaErrorV1> {
        let Some(State {
            function,
            shapes,
            flow,
            statements,
            explicit,
            facts,
            ..
        }) = self.0.as_mut()
        else {
            return Ok(None);
        };
        budget.charge(2)?;
        let before_visits = budget.remaining;
        lane_work_v1::statement(source, budget)?;
        budget.charge(
            (before_visits - budget.remaining).saturating_mul(map_work(flow.by_local.len())),
        )?;
        let SemanticStatementKindV1::Assign(a) = source else {
            flow.reject(source);
            return Ok(None);
        };
        let destination = flow.index(a.destination().local(), budget)?;
        budget.charge(1)?;
        if function.locals().get(a.destination().local().index() as usize)
                .is_none_or(|local| local.ty() != a.destination().ty())
            || !a.destination().projections().is_empty()
            || a.destination().ty() != a.value().result_type()
        {
            flow.reject(source);
            return Ok(None);
        }
        if let Some(index) = destination {
            flow.define(index);
        }
        if let Some(index) = destination {
            if let Some(owned) = shapes.pointee(a.destination().ty(), budget)? {
                if let SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place,
                } = a.value().kind()
                {
                    let Some(owner) = function.locals().get(place.local().index() as usize) else {
                        return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
                    };
                    if place.ty() == owned {
                        if place.projections().is_empty() && owner.ty() == owned {
                            budget.charge(map_work(explicit.len()))?;
                            if explicit.contains(&place.local())
                                || matches!(owner.role(), SemanticLocalRoleV1::Argument(_))
                            {
                                push(&mut flow.roots, (index, site), budget)?;
                                return Ok(None);
                            }
                        } else if matches!(place.projections(), [p] if p.kind() == SemanticProjectionKindV1::Dereference && p.result_type() == owned)
                            && owner.ty() == a.destination().ty()
                        {
                            if let Some(parent) = flow.index(place.local(), budget)? {
                                flow.link(index, parent, budget)?;
                                push(&mut flow.roots, (index, site), budget)?;
                                return Ok(None);
                            }
                        }
                    }
                }
            }
            if let SemanticRvalueKindV1::Borrow { kind: SemanticBorrowKindV1::Shared, place } = a.value().kind()
                && place.projections().is_empty()
                && function.locals().get(place.local().index() as usize)
                    .is_some_and(|owner| owner.ty() == place.ty())
                && shapes.shared_carrier_pointee(a.destination().ty(), budget)? == Some(place.ty())
                && let Some(parent) = flow.index(place.local(), budget)?
            {
                // Audit this shared edge without publishing its Borrow. A
                // mixed wrapper also carries other roles, whose common custody
                // graph alone may publish it; the Global union cannot override it.
                flow.link(index, parent, budget)?;
                return Ok(None);
            }
        }
        if transport(function, a, destination, shapes, flow, budget)? {
            return Ok(destination);
        }
        // The shared index still runs the full fact matcher. Precharge the
        // maximum number of local lookups its candidate roster can invoke.
        budget.charge(facts.saturating_mul(map_work(flow.by_local.len())))?;
        budget.profile.uses.audit_precharge(facts.saturating_mul(map_work(flow.by_local.len())));
        let captured = statements.find(source, budget, |fact, local| {
            let index = *flow.by_local.get(&local.index())?;
            (function.locals().get(local.index() as usize)?.ty() == fact.pairs()[2].0)
                .then_some(index)
        })?;
        if captured.is_some() && destination.is_none() {
            if let SemanticRvalueKindV1::Aggregate(aggregate) = a.value().kind() {
                // The old exact matrix capture matcher consumes only field
                // zero. It cannot hide a reference in another operand.
                for operand in &aggregate.operands()[1..] {
                    invalidate_reference_operand_v1(operand, &flow.by_local, &mut flow.nodes);
                }
            }
            return Ok(None);
        }
        flow.reject(source);
        Ok(None)
    }

    pub(super) fn terminator(
        &mut self,
        terminator: &SemanticTerminatorKindV1,
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        let Some(state) = self.0.as_ref() else {
            return Ok(());
        };
        if !state.relevance.terminator(terminator, budget)? {
            return Ok(());
        }
        self.terminator_relevant(terminator, budget)
    }

    fn terminator_relevant(
        &mut self,
        terminator: &SemanticTerminatorKindV1,
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        let Some(state) = self.0.as_mut() else {
            return Ok(());
        };
        let before_visits = budget.remaining;
        lane_work_v1::terminator(terminator, budget)?;
        budget.charge(
            (before_visits - budget.remaining).saturating_mul(map_work(state.flow.by_local.len())),
        )?;
        // Only expanded statement transport is admitted. No call, tail call,
        // drop, discriminant, cast or arbitrary load is a Global reference sink.
        validate_reference_uses_in_terminator_v1(
            terminator,
            &[],
            &state.flow.by_local,
            &mut state.flow.nodes,
        );
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn finish(
        self,
        budget: &mut Budget,
    ) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>, ProductionSemanticSsaErrorV1> {
        Ok(self.finish_with_fields(budget)?.sites)
    }
}

fn transport(
    function: &SemanticFunctionDeclV1,
    assignment: &SemanticAssignmentV1,
    destination: Option<usize>,
    shapes: &mut Shapes<'_>,
    flow: &mut Flow,
    budget: &mut Budget,
) -> Result<bool, ProductionSemanticSsaErrorV1> {
    match assignment.value().kind() {
        SemanticRvalueKindV1::Use(operand) => {
            if operand.ty() != assignment.destination().ty() {
                return Ok(false);
            }
            transport_operand(function, operand, destination, shapes, flow, budget)
        }
        SemanticRvalueKindV1::Aggregate(aggregate) if destination.is_some() => {
            let Some(fields) = shapes.fields(assignment.destination().ty()) else {
                return Ok(false);
            };
            let exact_kind = matches!(
                (
                    shapes.types[assignment.destination().ty().index() as usize].shape(),
                    aggregate.kind()
                ),
                (
                    SemanticTypeShapeV1::Aggregate(_),
                    SemanticAggregateKindV1::Aggregate
                ) | (
                    SemanticTypeShapeV1::Tuple(_),
                    SemanticAggregateKindV1::Tuple
                )
            );
            budget.charge(fields.len().saturating_add(aggregate.operands().len()))?;
            if !exact_kind
                || !fields
                    .iter()
                    .copied()
                    .eq(aggregate.operands().iter().map(SemanticOperandV1::ty))
            {
                return Ok(false);
            }
            for operand in aggregate.operands() {
                if !transport_operand(function, operand, destination, shapes, flow, budget)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn transport_operand(
    function: &SemanticFunctionDeclV1,
    operand: &SemanticOperandV1,
    destination: Option<usize>,
    shapes: &mut Shapes<'_>,
    flow: &mut Flow,
    budget: &mut Budget,
) -> Result<bool, ProductionSemanticSsaErrorV1> {
    let selected = shapes.transport_contains(operand.ty(), budget)?;
    let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
        return Ok(!selected);
    };
    let Some(parent) = flow.index(place.local(), budget)? else {
        return Ok(!selected);
    };
    let Some(mut ty) = function
        .locals()
        .get(place.local().index() as usize)
        .map(SemanticLocalDeclV1::ty)
    else {
        return Ok(false);
    };
    for (index, projection) in place.projections().iter().enumerate() {
        budget.charge(1)?;
        if index == 0 && projection.kind() == SemanticProjectionKindV1::Dereference {
            if place.projections().len() < 2
                || shapes.shared_carrier_pointee(ty, budget)? != Some(projection.result_type())
            { return Ok(false) }
            ty = projection.result_type();
            continue;
        }
        let SemanticProjectionKindV1::Field(index) = projection.kind() else {
            return Ok(false);
        };
        let Some(next) = shapes
            .fields(ty)
            .and_then(|fields| fields.get(index as usize))
            .copied()
        else {
            return Ok(false);
        };
        if next != projection.result_type() {
            return Ok(false);
        }
        ty = next;
    }
    if ty != place.ty() {
        return Ok(false);
    }
    // Moves of a projected shared leaf retain their original partial-move
    // certificate requirement; moved sub-carriers remain unsupported.
    if matches!(operand, SemanticOperandV1::Move(_))
        && !place.projections().is_empty()
        && selected
        && shapes.pointee(ty, budget)?.is_none()
    {
        return Ok(false);
    }
    if selected {
        let Some(destination) = destination else {
            return Ok(false);
        };
        flow.link(destination, parent, budget)?;
    }
    Ok(true)
}
