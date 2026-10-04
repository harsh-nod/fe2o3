use super::*;
use fe2o3_pliron::{
    ProductionSemanticSsaEventRoleV1 as EventRole,
    ProductionSemanticSsaOperandRoleV1 as OperandRole,
};

type OperandKey = (u32, u8, u32, u32, u8);
type BlockKey = (u32, u32, u32);

pub(super) struct Indices {
    operands: Vec<(OperandKey, usize)>,
    blocks: Vec<(BlockKey, usize)>,
}
impl Indices {
    pub(super) fn derive(
        owner: &ProductionScalarSsaEmissionOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.reserve_storage(size_of::<Self>())?;
        let mut operands = Vec::new();
        let mut blocks = Vec::new();
        let original = owner.original();
        let occurrences = original
            .semantic_ssa()
            .occurrences_v1()
            .ok_or(Error::Mismatch("guard original source occurrences"))?;
        for plan in original.semantic_ssa().plans() {
            budget.charge_work(1)?;
            let function = occurrences
                .function(plan.function())
                .ok_or(Error::Mismatch("guard source occurrence function"))?;
            for (ordinal, row) in function.events().iter().enumerate() {
                budget.charge_work(1)?;
                if !row.is_reachable() || !row.is_promoted() {
                    continue;
                }
                let role = match (row.role(), row.operand()) {
                    (EventRole::BaseUse, OperandRole::RvalueOperand(0)) => 0,
                    (EventRole::BaseUse, OperandRole::RvalueOperand(1)) => 1,
                    (EventRole::BaseUse, OperandRole::SwitchDiscriminant) => 2,
                    (EventRole::DestinationDefine, OperandRole::Destination) => 3,
                    _ => continue,
                };
                let (site, block, statement) = match row.site() {
                    Site::Statement { block, statement } => (0, block.get(), statement),
                    Site::Terminator { block } => (1, block.get(), 0),
                };
                append(
                    &mut operands,
                    (
                        (plan.function().index(), site, block, statement, role),
                        ordinal,
                    ),
                    budget,
                )?;
            }
        }
        for (ordinal, row) in original.correspondence.blocks().iter().enumerate() {
            append(
                &mut blocks,
                (
                    (
                        row.correspondence_owner().index(),
                        row.semantic_function().index(),
                        row.semantic_block().index(),
                    ),
                    ordinal,
                ),
                budget,
            )?;
        }
        resources::sort_work(operands.len(), budget)?;
        resources::sort_work(blocks.len(), budget)?;
        operands.sort_unstable_by_key(|row| row.0);
        blocks.sort_unstable_by_key(|row| row.0);
        budget.charge_work(
            operands
                .len()
                .checked_add(blocks.len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if operands.windows(2).any(|pair| pair[0].0 == pair[1].0)
            || blocks.windows(2).any(|pair| pair[0].0 == pair[1].0)
        {
            return Err(Error::Mismatch("guard duplicate source index coordinate"));
        }
        Ok(Self { operands, blocks })
    }

    pub(super) fn operand(
        &self,
        owner: &ProductionScalarSsaEmissionOwnerV1,
        function: SemanticFunctionIdV1,
        site: Site,
        role: u8,
        variable: SsaVariableIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<SsaValueV1> {
        let (site, block, statement) = match site {
            Site::Statement { block, statement } => (0, block.get(), statement),
            Site::Terminator { block } => (1, block.get(), 0),
        };
        charge_lookup(self.operands.len(), budget)?;
        let position = self
            .operands
            .binary_search_by_key(&(function.index(), site, block, statement, role), |row| {
                row.0
            })
            .map_err(|_| Error::Mismatch("guard exact source operand occurrence"))?;
        budget.charge_work(3)?;
        let function_rows = owner
            .original()
            .semantic_ssa()
            .occurrences_v1()
            .and_then(|rows| rows.function(function))
            .ok_or(Error::Mismatch("guard source operand function"))?;
        let row = function_rows
            .events()
            .get(self.operands[position].1)
            .ok_or(Error::Mismatch("guard source operand index target"))?;
        budget.charge_work(6)?;
        let actual_site = match row.site() {
            Site::Statement { block, statement } => (0, block.get(), statement),
            Site::Terminator { block } => (1, block.get(), 0),
        };
        let actual_role = match (row.role(), row.operand()) {
            (EventRole::BaseUse, OperandRole::RvalueOperand(0)) => Some(0),
            (EventRole::BaseUse, OperandRole::RvalueOperand(1)) => Some(1),
            (EventRole::BaseUse, OperandRole::SwitchDiscriminant) => Some(2),
            (EventRole::DestinationDefine, OperandRole::Destination) => Some(3),
            _ => None,
        };
        if !row.is_reachable()
            || !row.is_promoted()
            || actual_site != (site, block, statement)
            || actual_role != Some(role)
        {
            return Err(Error::Mismatch("guard exact source occurrence key"));
        }
        match (role, row.resolved()) {
            (
                3,
                Some(SsaResolvedEventV1::Define {
                    variable: actual,
                    value,
                }),
            ) if actual == variable => Ok(value),
            (
                0..=2,
                Some(SsaResolvedEventV1::Use {
                    variable: actual,
                    value,
                }),
            ) if actual == variable => Ok(value),
            _ => Err(Error::Mismatch("guard source operand resolution")),
        }
    }

    pub(super) fn block<'i, 'g>(
        &self,
        owner: &ProductionScalarSsaEmissionOwnerV1,
        inventory: &'i Inventory<'g>,
        function: &EmittedFunction,
        source_block: SemanticBlockIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<&'i fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'g>> {
        charge_lookup(self.blocks.len(), budget)?;
        let position = self
            .blocks
            .binary_search_by_key(
                &(
                    function.root.index(),
                    function.source.index(),
                    source_block.index(),
                ),
                |row| row.0,
            )
            .map_err(|_| Error::Mismatch("guard actual source block mapping"))?;
        budget.charge_work(1)?;
        let row = owner
            .original()
            .correspondence
            .blocks()
            .get(self.blocks[position].1)
            .ok_or(Error::Mismatch("guard source block index target"))?;
        budget.charge_work(3)?;
        if row.correspondence_owner() != function.root
            || row.semantic_function() != function.source
            || row.semantic_block() != source_block
        {
            return Err(Error::Mismatch("guard exact source block key"));
        }
        inventory
            .block_for_id(
                function
                    .coordinate
                    .ok_or(Error::Mismatch("guard physical function"))?,
                row.kernel_ir_block(),
                budget,
            )?
            .ok_or(Error::Mismatch("guard mapped N block"))
    }
}

pub(super) struct Incoming {
    rows: Vec<(Block, usize, Option<Edge>)>,
    count: usize,
}
impl Incoming {
    pub(super) fn prepare(
        owner: &ProductionScalarSsaEmissionOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.reserve_storage(size_of::<Self>())?;
        let mut count = 0usize;
        for function in &owner.original().executable().module().functions {
            budget.charge_work(1)?;
            if let Some(body) = &function.body {
                budget.charge_work(body.blocks.len())?;
                count = count
                    .checked_add(body.blocks.len())
                    .ok_or(Resource::Arithmetic)?;
                if count > MAX_ROWS {
                    return Err(Error::Limit);
                }
            }
        }
        let mut rows = Vec::new();
        reserve(&mut rows, count, budget)?;
        Ok(Self { rows, count })
    }

    pub(super) fn populate(
        &mut self,
        inventory: &Inventory<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        budget.charge_work(1)?;
        if inventory.blocks().len() != self.count || !self.rows.is_empty() {
            return Err(Error::Mismatch("guard actual block census"));
        }
        for block in inventory.blocks() {
            push(&mut self.rows, (block.coordinate, 0, None), budget)?;
        }
        budget.charge_work(self.rows.len())?;
        if self.rows.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
            return Err(Error::Mismatch("guard complete ordered N block index"));
        }
        for edge in inventory.edges() {
            self.observe(edge.target, edge.coordinate, budget)?;
        }
        Ok(())
    }

    fn observe(&mut self, target: Block, edge: Edge, budget: &mut Budget<'_>) -> Result<()> {
        charge_lookup(self.rows.len(), budget)?;
        let position = self
            .rows
            .binary_search_by_key(&target, |row| row.0)
            .map_err(|_| Error::Mismatch("guard actual edge target"))?;
        budget.charge_work(2)?;
        self.rows[position].1 = self.rows[position]
            .1
            .checked_add(1)
            .ok_or(Resource::Arithmetic)?;
        self.rows[position].2 = Some(edge);
        Ok(())
    }

    pub(super) fn unique(
        &self,
        target: Block,
        expected: Edge,
        budget: &mut Budget<'_>,
    ) -> Result<bool> {
        charge_lookup(self.rows.len(), budget)?;
        let position = self
            .rows
            .binary_search_by_key(&target, |row| row.0)
            .map_err(|_| Error::Mismatch("guard incoming block lookup"))?;
        budget.charge_work(2)?;
        Ok(self.rows[position].1 == 1 && self.rows[position].2 == Some(expected))
    }
}

#[cfg(test)]
#[path = "production_scalar_ssa_guard_indices_v1_tests.rs"]
mod tests;
