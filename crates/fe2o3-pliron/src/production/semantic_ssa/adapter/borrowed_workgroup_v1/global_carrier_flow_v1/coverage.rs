//! Deferred pure-Global field checks within the existing complete escape audit.
//! A transport handle cannot outlive the mutable borrow of that audit.
use super::*;

pub(in super::super) struct Transport<'s, 'a> {
    pub(super) state: &'s mut State<'a>,
    pub(super) site: SemanticTransparentBorrowSiteV1,
    pub(super) source: &'s SemanticStatementKindV1,
    pub(super) destination: usize,
}

pub(in super::super) struct FieldCoverage<'a> {
    assignment: &'a SemanticAssignmentV1,
    field: usize,
    source: usize,
    destination: usize,
    closed: bool,
}

impl FieldCoverage<'_> {
    pub(in super::super) fn assignment(&self) -> &SemanticAssignmentV1 {
        self.assignment
    }
    pub(in super::super) fn field(&self) -> usize {
        self.field
    }
    pub(in super::super) fn closed(&self) -> bool {
        self.closed
    }
}

pub(in super::super) struct Completed<'a> {
    pub(in super::super) sites: BTreeSet<SemanticTransparentBorrowSiteV1>,
    pub(in super::super) fields: Vec<FieldCoverage<'a>>,
}

impl Transport<'_, '_> {
    /// True records an obligation, not permission to publish a Borrow site.
    /// The requested role must be the whole shared leaf, not another role in
    /// a mixed subcarrier that happens to contain a Global address.
    pub(in super::super) fn defer_leaf(
        &mut self,
        expected: &SemanticAssignmentV1,
        field: usize,
        reference: SemanticTypeIdV1,
        owned: SemanticTypeIdV1,
        budget: &mut Budget,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        budget.charge(6)?;
        let Some(original) = self
            .state
            .function
            .blocks()
            .get(self.site.block as usize)
            .and_then(|block| block.statements().get(self.site.statement as usize))
            .map(|statement| statement.kind())
        else {
            return Ok(false);
        };
        if !std::ptr::eq(original, self.source) {
            return Ok(false);
        }
        let SemanticStatementKindV1::Assign(assignment) = original else {
            return Ok(false);
        };
        if !std::ptr::eq(assignment, expected) {
            return Ok(false);
        }
        let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
            return Ok(false);
        };
        let Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) =
            aggregate.operands().get(field)
        else {
            return Ok(false);
        };
        if place.ty() != reference || self.state.shapes.pointee(reference, budget)? != Some(owned) {
            return Ok(false);
        }
        let Some(source) = self.state.flow.index(place.local(), budget)? else {
            return Ok(false);
        };
        // The handle exists only after the original transport checked every
        // field/projection and linked every selected operand to this node.
        if self.state.pending_fields.is_empty() {
            budget.charge(3)?;
        }
        push(
            &mut self.state.pending_fields,
            FieldCoverage {
                assignment,
                field,
                source,
                destination: self.destination,
                closed: false,
            },
            budget,
        )?;
        Ok(true)
    }
}

impl<'a> Audit<'a> {
    pub(in super::super) fn finish_with_fields(
        self,
        budget: &mut Budget,
    ) -> Result<Completed<'a>, ProductionSemanticSsaErrorV1> {
        let Some(mut state) = self.0 else {
            return Ok(Completed {
                sites: BTreeSet::new(),
                fields: Vec::new(),
            });
        };
        let sites = state.flow.finish(budget)?;
        for field in &mut state.pending_fields {
            budget.charge(3)?;
            field.closed =
                state.flow.nodes[field.source].valid && state.flow.nodes[field.destination].valid;
        }
        Ok(Completed {
            sites,
            fields: state.pending_fields,
        })
    }
}
