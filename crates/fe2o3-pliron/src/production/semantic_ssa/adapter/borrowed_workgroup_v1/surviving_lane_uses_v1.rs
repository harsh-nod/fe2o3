//! Sparse statement selection for surviving components, never an acceptance rule.
use super::*;

pub(super) struct SurvivingUses {
    locals: Vec<bool>,
}

impl SurvivingUses {
    pub(super) fn new(
        function: &SemanticFunctionDeclV1,
        candidates: &[SemanticBorrowCandidateV1],
        children: &[Vec<usize>],
        roots: impl Iterator<Item = usize>,
        budget: &mut Budget,
    ) -> Result<Option<Self>, ProductionSemanticSsaErrorV1> {
        let mut result: Option<Self> = None;
        for root in roots {
            budget.charge(1)?;
            let Ok(members) =
                borrow_components_v1::members(candidates, children, root, budget, |_| true)?
            else {
                continue;
            };
            if result.is_none() {
                // Header, logical cells and initialization visits, before allocation.
                let count = function.locals().len();
                budget.charge(3)?;
                budget.charge(count)?;
                budget.charge(count)?;
                let locals = vec![false; count];
                budget.charge(locals.capacity() - count)?;
                result = Some(Self { locals });
            }
            let result = result.as_mut().expect("allocated for the first survivor");
            for index in members {
                budget.charge(2)?;
                let site = candidates[index].site;
                let assignment = function
                    .blocks()
                    .get(site.block as usize)
                    .and_then(|block| block.statements().get(site.statement as usize))
                    .and_then(|statement| match statement.kind() {
                        SemanticStatementKindV1::Assign(a) => Some(a),
                        _ => None,
                    })
                    .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
                let live = result
                    .locals
                    .get_mut(assignment.destination().local().index() as usize)
                    .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
                *live = true;
            }
        }
        Ok(result)
    }

    fn local(&self, local: SemanticLocalIdV1) -> bool {
        self.locals
            .get(local.index() as usize)
            .copied()
            .unwrap_or(false)
    }

    fn place(
        &self,
        place: &SemanticPlaceV1,
        budget: &mut Budget,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        budget.charge(1)?;
        if self.local(place.local()) {
            return Ok(true);
        }
        for projection in place.projections() {
            budget.charge(1)?;
            if let SemanticProjectionKindV1::Index(local) = projection.kind() {
                if self.local(local) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn operand(
        &self,
        operand: &SemanticOperandV1,
        budget: &mut Budget,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                self.place(place, budget)
            }
            SemanticOperandV1::Constant(_) => {
                budget.charge(1)?;
                Ok(false)
            }
        }
    }

    fn rvalue(
        &self,
        value: &SemanticRvalueKindV1,
        budget: &mut Budget,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        // A found use terminates this selection scan, not the full audit. An
        // actual budget failure is kept distinct and returned unchanged.
        match value.try_visit_operands(|operand| match self.operand(operand, budget) {
            Ok(true) => Err(None),
            Ok(false) => Ok(()),
            Err(error) => Err(Some(error)),
        }) {
            Err(None) => return Ok(true),
            Err(Some(error)) => return Err(error),
            Ok(()) => {}
        }
        match value {
            SemanticRvalueKindV1::Borrow { place, .. }
            | SemanticRvalueKindV1::AddressOf { place, .. }
            | SemanticRvalueKindV1::Length(place)
            | SemanticRvalueKindV1::Discriminant(place) => self.place(place, budget),
            SemanticRvalueKindV1::Load(load) => self.place(load.source(), budget),
            SemanticRvalueKindV1::Use(_)
            | SemanticRvalueKindV1::Unary { .. }
            | SemanticRvalueKindV1::Binary { .. }
            | SemanticRvalueKindV1::CheckedBinary(_)
            | SemanticRvalueKindV1::UncheckedBinary(_)
            | SemanticRvalueKindV1::Cast { .. }
            | SemanticRvalueKindV1::Aggregate(_) => Ok(false),
        }
    }

    pub(super) fn touches(
        &self,
        statement: &SemanticStatementKindV1,
        budget: &mut Budget,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        budget.charge(1)?;
        match statement {
            SemanticStatementKindV1::Assign(a) => Ok(
                self.place(a.destination(), budget)? || self.rvalue(a.value().kind(), budget)?
            ),
            SemanticStatementKindV1::Store(store) => {
                Ok(self.place(store.destination(), budget)?
                    || self.operand(store.value(), budget)?)
            }
            SemanticStatementKindV1::AtomicRmw(op) => Ok(self.place(op.destination(), budget)?
                || self.place(op.address(), budget)?
                || self.operand(op.value(), budget)?),
            SemanticStatementKindV1::AtomicCompareExchange(op) => Ok(self
                .place(op.destination(), budget)?
                || self.place(op.address(), budget)?
                || self.operand(op.expected(), budget)?
                || self.operand(op.replacement(), budget)?),
            SemanticStatementKindV1::SetDiscriminant { place, .. }
            | SemanticStatementKindV1::Deinitialize(place) => self.place(place, budget),
            SemanticStatementKindV1::Assume(operand) => self.operand(operand, budget),
            // These were no-ops in the all-use audit. Original lifetime and
            // retained-initialization passes still inspect every such event.
            SemanticStatementKindV1::StorageLive(_)
            | SemanticStatementKindV1::StorageDead(_)
            | SemanticStatementKindV1::Nop => Ok(false),
        }
    }
}
