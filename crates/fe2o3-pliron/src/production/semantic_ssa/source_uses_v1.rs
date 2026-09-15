//! Borrowed source-to-planner correspondence, not scalar or proof authority.
use super::*;
use fe2o3_mir_model::{SsaResolvedEventV1, SsaValueV1};
use std::ops::Range;

mod borrow_place;
mod operand_source;
pub use operand_source::ProductionSemanticSsaSourceOperandV1;

pub(super) mod definitions_v1;
pub use definitions_v1::{
    ProductionSemanticSsaIncomingEdgeV1, ProductionSemanticSsaIncomingValuesV1,
    ProductionSemanticSsaValueOriginV1, ProductionSemanticSsaValueV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticSsaSourceSiteV1 {
    block: SemanticBlockIdV1,
    statement: Option<u32>,
}

impl ProductionSemanticSsaSourceSiteV1 {
    pub const fn new(block: SemanticBlockIdV1, statement: Option<u32>) -> Self {
        Self { block, statement }
    }
    pub const fn block(self) -> SemanticBlockIdV1 {
        self.block
    }
    pub const fn statement(self) -> Option<u32> {
        self.statement
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticSsaSourceQueryErrorV1 {
    WrongOwner,
    InvalidSite,
    UnsupportedOperand,
    OperandOutsideSite,
    NoPromotedUse,
    DisagreeingUses,
    MissingEventOrigin,
    MissingDefinition,
    MissingIncoming,
    WorkLimit,
}

impl fmt::Display for ProductionSemanticSsaSourceQueryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "semantic SSA source query rejected: {self:?}")
    }
}
impl Error for ProductionSemanticSsaSourceQueryErrorV1 {}

/// A query over one exact execution body and the existing owner's plan.
///
/// Creation does not replay the owner. Production consumers must retain their
/// existing verify_replay boundary before advancing custody. Every variable
/// cost query requires the caller's existing work charger; no budget is made
/// or reset here. This object contains no mutable cache or alternative graph.
#[derive(Clone, Copy)]
pub struct ProductionSemanticSsaSourceQueryV1<'a> {
    view: &'a SemanticExpandedRootV1,
    function: &'a SemanticFunctionDeclV1,
    plan: &'a ProductionSemanticSsaFunctionPlanV1,
}

/// The exact existing SSA value at a pointer-identical source operand site.
/// Several uses in one site are accepted only when every matching value agrees.
/// This does not select a reaching source expression or authorize an operation.
pub struct ProductionSemanticSsaSourceUseV1<'a> {
    view: &'a SemanticExpandedRootV1,
    plan: &'a ProductionSemanticSsaFunctionPlanV1,
    operand: &'a SemanticOperandV1,
    site: ProductionSemanticSsaSourceSiteV1,
    variable: SsaVariableIdV1,
    value: SsaValueV1,
    event_range: Range<usize>,
    agreeing_uses: usize,
}

impl<'a> ProductionSemanticSsaSourceUseV1<'a> {
    pub fn belongs_to(&self, query: &ProductionSemanticSsaSourceQueryV1<'_>) -> bool {
        std::ptr::eq(self.view, query.view) && std::ptr::eq(self.plan, query.plan)
    }
    pub const fn operand(&self) -> &'a SemanticOperandV1 {
        self.operand
    }
    pub const fn site(&self) -> ProductionSemanticSsaSourceSiteV1 {
        self.site
    }
    pub const fn variable(&self) -> SsaVariableIdV1 {
        self.variable
    }
    pub const fn value(&self) -> SsaValueV1 {
        self.value
    }
    pub fn event_range(&self) -> Range<usize> {
        self.event_range.clone()
    }
    pub const fn agreeing_uses(&self) -> usize {
        self.agreeing_uses
    }
}

impl ProductionSemanticSsaOwnerV1 {
    pub fn source_query_for_root<'a>(
        &'a self,
        root: SemanticFunctionIdV1,
        function: &'a SemanticFunctionDeclV1,
    ) -> Result<ProductionSemanticSsaSourceQueryV1<'a>, ProductionSemanticSsaSourceQueryErrorV1>
    {
        let view = self
            .execution_view_for_root(root)
            .ok_or(QueryError::WrongOwner)?;
        let plan = self
            .execution_plan_for_root(root)
            .ok_or(QueryError::WrongOwner)?;
        if !std::ptr::eq(function, view.body())
            || plan.function_identity() != function.identity()
            || plan.function() != view.source_body()
        {
            return Err(QueryError::WrongOwner);
        }
        Ok(ProductionSemanticSsaSourceQueryV1 {
            view,
            function,
            plan,
        })
    }
}

type QueryError = ProductionSemanticSsaSourceQueryErrorV1;
type Site = ProductionSemanticSsaSourceSiteV1;

fn step(charge: &mut impl FnMut() -> bool) -> Result<(), QueryError> {
    if charge() {
        Ok(())
    } else {
        Err(QueryError::WorkLimit)
    }
}

fn partition<T>(
    rows: &[T],
    mut before: impl FnMut(&T) -> bool,
    charge: &mut impl FnMut() -> bool,
) -> Result<usize, QueryError> {
    let (mut start, mut end) = (0, rows.len());
    while start < end {
        step(charge)?;
        let middle = start + (end - start) / 2;
        if before(&rows[middle]) {
            start = middle + 1;
        } else {
            end = middle;
        }
    }
    Ok(start)
}

impl<'a> ProductionSemanticSsaSourceQueryV1<'a> {
    pub const fn root(&self) -> SemanticFunctionIdV1 {
        self.view.root()
    }
    pub const fn function(&self) -> &'a SemanticFunctionDeclV1 {
        self.function
    }
    pub const fn plan(&self) -> &'a ProductionSemanticSsaFunctionPlanV1 {
        self.plan
    }

    fn site_range(
        &self,
        site: Site,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<Range<usize>, QueryError> {
        let block = site.block.index();
        if !self.plan.plan.is_reachable(SsaBlockIdV1::new(block)) {
            return Err(QueryError::InvalidSite);
        }
        step(charge)?;
        let ends = self
            .plan
            .event_origins
            .block_ends(block)
            .ok_or(QueryError::NoPromotedUse)?;
        let statements = ends.len().checked_sub(1).ok_or(QueryError::NoPromotedUse)?;
        let index = match site.statement {
            Some(statement) if (statement as usize) < statements => statement as usize,
            Some(_) => return Err(QueryError::NoPromotedUse),
            None => statements,
        };
        let end = *ends.get(index).ok_or(QueryError::NoPromotedUse)? as usize;
        let start = index
            .checked_sub(1)
            .and_then(|index| ends.get(index))
            .map_or(0, |end| *end as usize);
        if start >= end {
            return Err(QueryError::NoPromotedUse);
        }
        Ok(start..end)
    }

    pub fn operand_use(
        &self,
        site: Site,
        operand: &'a SemanticOperandV1,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<ProductionSemanticSsaSourceUseV1<'a>, QueryError> {
        let source = self.operand_source(site, operand, charge)?;
        let variable = SsaVariableIdV1::new(source.local().index());
        let (value, range, agreeing_uses) = self.selected_site_use(site, variable, charge)?;
        Ok(ProductionSemanticSsaSourceUseV1 {
            view: self.view,
            plan: self.plan,
            operand,
            site,
            variable,
            value,
            event_range: range,
            agreeing_uses,
        })
    }

    fn contains_operand(
        &self,
        site: Site,
        operand: &SemanticOperandV1,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<bool, QueryError> {
        let block = self
            .function
            .blocks()
            .get(site.block.index() as usize)
            .ok_or(QueryError::InvalidSite)?;
        let mut found = false;
        let mut inspect = |actual: &SemanticOperandV1| {
            step(charge)?;
            found |= std::ptr::eq(actual, operand);
            Ok(())
        };
        match site.statement {
            Some(statement) => {
                match block
                    .statements()
                    .get(statement as usize)
                    .ok_or(QueryError::InvalidSite)?
                    .kind()
                {
                    SemanticStatementKindV1::Assign(assignment) => {
                        assignment.value().kind().try_visit_operands(&mut inspect)?
                    }
                    SemanticStatementKindV1::Store(store) => inspect(store.value())?,
                    _ => return Err(QueryError::UnsupportedOperand),
                }
            }
            None => {
                match block.terminator().kind() {
                    SemanticTerminatorKindV1::Call(call) => {
                        for argument in call.arguments() { inspect(argument)?; }
                    }
                    SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => inspect(discriminant)?,
                    _ => return Err(QueryError::UnsupportedOperand),
                }
            }
        }
        Ok(found)
    }

    /// Maps an actual resolved event to the emitting source site. This is not a
    /// reverse SSA-value definition index and never guesses a statement count.
    pub fn event_site(
        &self,
        block: SsaBlockIdV1,
        event: u32,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<Site, QueryError> {
        self.plan.source_event_site(block, event, charge)
    }
}

include!("source_uses_v1/resolved_events_v1.rs");

#[cfg(test)]
mod tests;
