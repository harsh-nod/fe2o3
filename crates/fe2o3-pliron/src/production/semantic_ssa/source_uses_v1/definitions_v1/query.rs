use super::*;

include!("definition_lookup_v1.rs");

/// An existing planner value borrowed from an exact source use or incoming
/// argument. Raw SSA IDs alone cannot construct this owner-bound query input.
#[derive(Clone, Copy)]
pub struct ProductionSemanticSsaValueV1<'a> {
    view: &'a SemanticExpandedRootV1,
    plan: &'a ProductionSemanticSsaFunctionPlanV1,
    variable: SsaVariableIdV1,
    value: SsaValueV1,
}

impl<'a> ProductionSemanticSsaSourceUseV1<'a> {
    pub fn retained_value(&self) -> ProductionSemanticSsaValueV1<'a> {
        ProductionSemanticSsaValueV1 {
            view: self.view,
            plan: self.plan,
            variable: self.variable,
            value: self.value,
        }
    }
}

impl ProductionSemanticSsaValueV1<'_> {
    pub fn belongs_to(&self, query: &ProductionSemanticSsaSourceQueryV1<'_>) -> bool {
        std::ptr::eq(self.view, query.view) && std::ptr::eq(self.plan, query.plan)
    }
    pub const fn variable(&self) -> SsaVariableIdV1 {
        self.variable
    }
    pub const fn value(&self) -> SsaValueV1 {
        self.value
    }
}

/// The adapter's exact edge ordinal and role, without normal-edge filtering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticSsaIncomingEdgeV1 {
    row: IncomingRow,
}

impl ProductionSemanticSsaIncomingEdgeV1 {
    pub const fn id(self) -> SsaEdgeIdV1 {
        self.row.edge()
    }
    pub const fn role(self) -> SsaEdgeRoleV1 {
        self.row.role()
    }
    pub const fn target(self) -> SsaBlockIdV1 {
        self.row.target()
    }
}

/// Correspondence only: no origin here proves totality, guard equivalence,
/// memory versions, or that a source expression can be projected.
pub enum ProductionSemanticSsaValueOriginV1<'a> {
    Entry {
        argument: usize,
    },
    Event {
        block: SsaBlockIdV1,
        event: u32,
        site: Site,
    },
    Edge {
        edge: ProductionSemanticSsaIncomingEdgeV1,
        definition: usize,
    },
    BlockArgument(ProductionSemanticSsaIncomingValuesV1<'a>),
}

/// Every reachable predecessor of the actual merge block, plus external entry
/// when that block is the function entry. Construction checks all arguments
/// before returning; queries do not allocate or rebuild a graph.
pub struct ProductionSemanticSsaIncomingValuesV1<'a> {
    view: &'a SemanticExpandedRootV1,
    plan: &'a ProductionSemanticSsaFunctionPlanV1,
    block: SsaBlockIdV1,
    variable: SsaVariableIdV1,
    position: usize,
    rows: &'a [IncomingRow],
    external_entry: Option<SsaValueV1>,
}

impl<'a> ProductionSemanticSsaIncomingValuesV1<'a> {
    pub fn belongs_to(&self, query: &ProductionSemanticSsaSourceQueryV1<'_>) -> bool {
        std::ptr::eq(self.view, query.view) && std::ptr::eq(self.plan, query.plan)
    }
    pub const fn block(&self) -> SsaBlockIdV1 {
        self.block
    }
    pub const fn variable(&self) -> SsaVariableIdV1 {
        self.variable
    }
    pub fn edge_count(&self) -> usize {
        self.rows.len()
    }
    pub fn external_entry(&self) -> Option<ProductionSemanticSsaValueV1<'a>> {
        self.external_entry.map(|value| self.token(value))
    }
    pub fn edge(
        &self,
        index: usize,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<
        Option<(
            ProductionSemanticSsaIncomingEdgeV1,
            ProductionSemanticSsaValueV1<'a>,
        )>,
        QueryError,
    > {
        step(charge)?;
        let Some(row) = self.rows.get(index) else {
            return Ok(None);
        };
        let argument = self
            .plan
            .plan
            .edge_arguments(row.edge())
            .and_then(|arguments| arguments.get(self.position))
            .filter(|argument| argument.variable() == self.variable)
            .ok_or(QueryError::MissingIncoming)?;
        Ok(Some((
            ProductionSemanticSsaIncomingEdgeV1 { row: *row },
            self.token(argument.value()),
        )))
    }
    fn token(&self, value: SsaValueV1) -> ProductionSemanticSsaValueV1<'a> {
        ProductionSemanticSsaValueV1 {
            view: self.view,
            plan: self.plan,
            variable: self.variable,
            value,
        }
    }
}

impl<'a> ProductionSemanticSsaSourceQueryV1<'a> {
    /// Inert complete predecessor inventory from this exact retained planner.
    pub fn predecessor_count(
        &self,
        block: SsaBlockIdV1,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<usize, QueryError> {
        Ok(self.predecessor_rows(block, charge)?.len())
    }

    pub fn predecessor(
        &self,
        block: SsaBlockIdV1,
        index: usize,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<Option<ProductionSemanticSsaIncomingEdgeV1>, QueryError> {
        let rows = self.predecessor_rows(block, charge)?;
        step(charge)?;
        let Some(&row) = rows.get(index) else {
            return Ok(None);
        };
        if row.target() != block || !self.plan.plan.is_reachable(row.edge().source()) {
            return Err(QueryError::MissingIncoming);
        }
        Ok(Some(ProductionSemanticSsaIncomingEdgeV1 { row }))
    }

    fn predecessor_rows(
        &self,
        block: SsaBlockIdV1,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<&'a [IncomingRow], QueryError> {
        step(charge)?;
        if !self.plan.plan.is_reachable(block) {
            return Err(QueryError::InvalidSite);
        }
        let origins = &self.plan.value_origins;
        let start = *origins
            .offsets
            .get(block.get() as usize)
            .ok_or(QueryError::MissingIncoming)? as usize;
        let end = *origins
            .offsets
            .get(block.get() as usize + 1)
            .ok_or(QueryError::MissingIncoming)? as usize;
        origins
            .incoming
            .get(start..end)
            .ok_or(QueryError::MissingIncoming)
    }

    pub fn value_origin(
        &self,
        value: &ProductionSemanticSsaValueV1<'a>,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<ProductionSemanticSsaValueOriginV1<'a>, QueryError> {
        step(charge)?;
        if !value.belongs_to(self) {
            return Err(QueryError::WrongOwner);
        }
        let SsaValueV1::Definition(id) = value.value else {
            let SsaValueV1::BlockArgument { block, variable } = value.value else {
                unreachable!()
            };
            if variable != value.variable {
                return Err(QueryError::MissingIncoming);
            }
            return self
                .incoming_values(block, variable, charge)
                .map(ProductionSemanticSsaValueOriginV1::BlockArgument);
        };
        definition_origin(self.plan, id, Some(value.variable), charge)
            .map(|(_, origin)| origin)
    }

    fn incoming_values(
        &self,
        block: SsaBlockIdV1,
        variable: SsaVariableIdV1,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<ProductionSemanticSsaIncomingValuesV1<'a>, QueryError> {
        let origins = &self.plan.value_origins;
        let variables = self
            .plan
            .plan
            .merge_variables(block)
            .ok_or(QueryError::MissingIncoming)?;
        let position = partition(variables, |actual| *actual < variable, charge)?;
        if variables.get(position) != Some(&variable) {
            return Err(QueryError::MissingIncoming);
        }
        let index = block.get() as usize;
        let start = *origins
            .offsets
            .get(index)
            .ok_or(QueryError::MissingIncoming)?;
        let end = *origins
            .offsets
            .get(index.checked_add(1).ok_or(QueryError::MissingIncoming)?)
            .ok_or(QueryError::MissingIncoming)?;
        let rows = origins
            .incoming
            .get(start as usize..end as usize)
            .ok_or(QueryError::MissingIncoming)?;
        let external_entry = if block == origins.entry {
            step(charge)?;
            Some(
                self.plan
                    .plan
                    .entry_arguments()
                    .get(position)
                    .filter(|argument| argument.variable() == variable)
                    .ok_or(QueryError::MissingIncoming)?
                    .value(),
            )
        } else {
            None
        };
        if rows.is_empty() && external_entry.is_none() {
            return Err(QueryError::MissingIncoming);
        }
        for row in rows {
            step(charge)?;
            if row.target() != block
                || !self.plan.plan.is_reachable(row.edge().source())
                || self
                    .plan
                    .plan
                    .edge_arguments(row.edge())
                    .and_then(|arguments| arguments.get(position))
                    .is_none_or(|argument| argument.variable() != variable)
            {
                return Err(QueryError::MissingIncoming);
            }
        }
        Ok(ProductionSemanticSsaIncomingValuesV1 {
            view: self.view,
            plan: self.plan,
            block,
            variable,
            position,
            rows,
            external_entry,
        })
    }
}
