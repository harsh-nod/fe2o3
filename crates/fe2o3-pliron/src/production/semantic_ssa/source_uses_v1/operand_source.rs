//! Exact source correspondence without a promoted SSA value.
use super::*;

/// A reachable original whole-local operand in this exact replayed body/plan.
/// This is not a reaching value, initialization, issuer or lifetime proof.
pub struct ProductionSemanticSsaSourceOperandV1<'a> {
    view: &'a SemanticExpandedRootV1,
    plan: &'a ProductionSemanticSsaFunctionPlanV1,
    operand: &'a SemanticOperandV1,
    site: Site,
    local: SemanticLocalIdV1,
}

impl<'a> ProductionSemanticSsaSourceOperandV1<'a> {
    /// Whether this selection belongs to this exact source query owner.
    pub fn belongs_to(&self, query: &ProductionSemanticSsaSourceQueryV1<'_>) -> bool {
        std::ptr::eq(self.view, query.view) && std::ptr::eq(self.plan, query.plan)
    }

    /// The pointer-identical operand retained by the execution body.
    pub const fn operand(&self) -> &'a SemanticOperandV1 {
        self.operand
    }

    /// The exact statement or terminator containing the operand.
    pub const fn site(&self) -> ProductionSemanticSsaSourceSiteV1 {
        self.site
    }

    /// The original whole-local storage identifier, not an SSA value.
    pub const fn local(&self) -> SemanticLocalIdV1 {
        self.local
    }
}

impl<'a> ProductionSemanticSsaSourceQueryV1<'a> {
    /// Selects an exact reachable Copy/Move occurrence without asserting a
    /// promoted value. Consumers must separately prove storage and lifetime.
    /// Constants, projections, cloned operands and foreign sites are rejected.
    pub fn operand_source(
        &self,
        site: Site,
        operand: &'a SemanticOperandV1,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<ProductionSemanticSsaSourceOperandV1<'a>, QueryError> {
        step(charge)?;
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
            return Err(QueryError::UnsupportedOperand);
        };
        if !place.projections().is_empty()
            || self
                .function
                .locals()
                .get(place.local().index() as usize)
                .is_none_or(|local| local.ty() != place.ty())
        {
            return Err(QueryError::UnsupportedOperand);
        }
        if !self.contains_operand(site, operand, charge)? {
            return Err(QueryError::OperandOutsideSite);
        }
        if !self
            .plan
            .plan
            .is_reachable(SsaBlockIdV1::new(site.block.index()))
        {
            return Err(QueryError::InvalidSite);
        }
        Ok(ProductionSemanticSsaSourceOperandV1 {
            view: self.view,
            plan: self.plan,
            operand,
            site,
            local: place.local(),
        })
    }
}
