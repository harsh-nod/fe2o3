//! Concrete replay of actual effect-free blocks and simultaneous edge bindings.
//! The caller still proves every source correspondence and control association.
use super::super::Inventory;
use super::{
    Error, ExpressionV30, NodeV30, Resource, Result, Writer, canonical,
    canonical::control::{TargetBlock, TargetBranch},
    vector,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirFunctionCoordinateV1 as Function,
};
use std::{mem::size_of, ops::Range};

pub(super) struct ConcreteTrace<'i, 'g> {
    inventory: &'i Inventory<'g>,
    function: Function,
    definitions: Range<usize>,
    pub nodes: Vec<NodeV30>,
    pub values: Vec<Option<usize>>,
    slot: usize,
    ledger: Ledger,
    required: usize,
    epoch: usize,
}

pub(super) struct AppendedBlock<'a> {
    target: &'a TargetBlock,
    translated: Vec<usize>,
    trace: usize,
    epoch: usize,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("original MIR concrete connector trace differs")
}

impl<'i, 'g> ConcreteTrace<'i, 'g> {
    pub(super) fn arguments(
        inventory: &'i Inventory<'g>,
        physical: usize,
        capacity: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        Self::seed(inventory, physical, capacity, false, out)
    }

    /// Symbolic pre-state for one source-owned block segment. The composing
    /// relation must bind observed symbols to the complete source invariant;
    /// this does not assert that arbitrary definitions are initialized.
    pub(super) fn boundary(
        inventory: &'i Inventory<'g>,
        physical: usize,
        capacity: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        Self::seed(inventory, physical, capacity, true, out)
    }

    fn seed(
        inventory: &'i Inventory<'g>,
        physical: usize,
        capacity: usize,
        boundary: bool,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        let function = inventory.functions().get(physical).ok_or_else(mismatch)?;
        let count = if boundary {
            function.definitions.len()
        } else {
            function.function.signature.parameters.len()
        };
        if function.coordinate.0 as usize != physical || capacity < count {
            return Err(mismatch());
        }
        let mut nodes = vector(capacity, out)?;
        let mut values = vector(function.definitions.len(), out)?;
        out.budget.charge_work(function.definitions.len())?;
        values.resize(function.definitions.len(), None);
        for argument in 0..count {
            out.budget.charge_work(3)?;
            let definition = function
                .definitions
                .start
                .checked_add(argument)
                .ok_or(Resource::Arithmetic)?;
            let row = inventory
                .definitions()
                .get(definition)
                .ok_or_else(mismatch)?;
            if !boundary
                && row.coordinate
                    != (Definition::FunctionArgument {
                        function: function.coordinate,
                        argument: u32::try_from(argument).map_err(|_| Resource::Arithmetic)?,
                    })
            {
                return Err(mismatch());
            }
            values[argument] = Some(nodes.len());
            nodes.push(NodeV30 {
                scalar: canonical::scalar(row.ty)?,
                expression: ExpressionV30::Argument(
                    u32::try_from(definition).map_err(|_| Resource::Arithmetic)?,
                ),
            });
        }
        Ok(Self {
            inventory,
            function: function.coordinate,
            definitions: function.definitions.clone(),
            nodes,
            values,
            slot: std::ptr::from_ref(&*out.budget) as usize,
            ledger: out.budget.work_ledger_identity_v1(),
            required: out.budget.storage(),
            epoch: 0,
        })
    }

    pub(super) fn value(&self, definition: usize, out: &Writer<'_, '_>) -> Result<usize> {
        self.check(out)?;
        self.values
            .get(
                definition
                    .checked_sub(self.definitions.start)
                    .ok_or_else(mismatch)?,
            )
            .copied()
            .flatten()
            .ok_or_else(mismatch)
    }

    fn check(&self, out: &Writer<'_, '_>) -> Result<()> {
        if self.slot != std::ptr::from_ref(&*out.budget) as usize
            || self.ledger != out.budget.work_ledger_identity_v1()
            || out.budget.storage() < self.required
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }

    pub(super) fn append<'a>(
        &mut self,
        target: &'a TargetBlock,
        out: &mut Writer<'_, '_>,
    ) -> Result<AppendedBlock<'a>> {
        self.check(out)?;
        let coordinate = target.coordinate(self.inventory)?;
        if coordinate.function != self.function
            || target.program.definition_start != self.definitions.start
            || target.program.definitions.len() != self.definitions.len()
        {
            return Err(mismatch());
        }
        let mut translated = vector(target.program.nodes.len(), out)?;
        for node in &target.program.nodes {
            out.budget.charge_work(5)?;
            let edge = |at: usize| translated.get(at).copied().ok_or_else(mismatch);
            let expression = match node.expression {
                ExpressionV30::Argument(definition) => {
                    let slot = (definition as usize)
                        .checked_sub(self.definitions.start)
                        .ok_or_else(mismatch)?;
                    let value = self
                        .values
                        .get(slot)
                        .copied()
                        .flatten()
                        .ok_or_else(mismatch)?;
                    if self.nodes.get(value).map(|row| row.scalar) != Some(node.scalar) {
                        return Err(mismatch());
                    }
                    translated.push(value);
                    continue;
                }
                ExpressionV30::Constant(value) => ExpressionV30::Constant(value),
                ExpressionV30::Not(input) => ExpressionV30::Not(edge(input)?),
                ExpressionV30::Binary {
                    operation,
                    left,
                    right,
                } => ExpressionV30::Binary {
                    operation,
                    left: edge(left)?,
                    right: edge(right)?,
                },
            };
            if self.nodes.len() == self.nodes.capacity() {
                return Err(Resource::Accounting.into());
            }
            translated.push(self.nodes.len());
            self.nodes.push(NodeV30 {
                scalar: node.scalar,
                expression,
            });
        }
        for (slot, node) in target.program.definitions.iter().enumerate() {
            out.budget.charge_work(2)?;
            if let Some(node) = node {
                self.values[slot] = Some(*translated.get(*node).ok_or_else(mismatch)?);
            }
        }
        self.epoch = self.epoch.checked_add(1).ok_or(Resource::Arithmetic)?;
        Ok(AppendedBlock {
            target,
            translated,
            trace: std::ptr::from_ref(&*self) as usize,
            epoch: self.epoch,
            required: out.budget.storage(),
        })
    }

    pub(super) fn edge(
        &mut self,
        appended: AppendedBlock<'_>,
        ordinal: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Block> {
        self.check(out)?;
        if out.budget.storage() < appended.required {
            return Err(Resource::Accounting.into());
        }
        if appended.trace != std::ptr::from_ref(&*self) as usize || appended.epoch != self.epoch {
            return Err(mismatch());
        }
        let target = appended.target;
        let translated = &appended.translated;
        if target.coordinate(self.inventory)?.function != self.function {
            return Err(mismatch());
        }
        let edge = target.edges.get(ordinal).ok_or_else(mismatch)?;
        if edge.target.function != self.function || translated.len() != target.program.nodes.len() {
            return Err(mismatch());
        }
        // Values are immutable expression indices from the whole pre-edge
        // state. Ordered writes therefore still implement simultaneous phis.
        for &(definition, value) in &edge.arguments {
            out.budget.charge_work(3)?;
            let slot = definition
                .checked_sub(self.definitions.start)
                .ok_or_else(mismatch)?;
            *self.values.get_mut(slot).ok_or_else(mismatch)? =
                Some(*translated.get(value).ok_or_else(mismatch)?);
        }
        self.epoch = self.epoch.checked_add(1).ok_or(Resource::Arithmetic)?;
        Ok(edge.target)
    }

    pub(super) fn appended_value(
        &self,
        appended: &AppendedBlock<'_>,
        node: usize,
        out: &Writer<'_, '_>,
    ) -> Result<usize> {
        self.check(out)?;
        if out.budget.storage() < appended.required {
            return Err(Resource::Accounting.into());
        }
        if appended.trace != std::ptr::from_ref(self) as usize || appended.epoch != self.epoch {
            return Err(mismatch());
        }
        appended.translated.get(node).copied().ok_or_else(mismatch)
    }

    /// Replays only finite single-successor connectors. A branch, return, cycle
    /// or missing exact boundary is a refusal, not an assumed stuttering step.
    pub(super) fn connectors(
        &mut self,
        mut at: Block,
        targets: &[TargetBlock],
        boundaries: &[bool],
        out: &mut Writer<'_, '_>,
    ) -> Result<(Block, Vec<usize>)> {
        self.check(out)?;
        let function = self
            .inventory
            .functions()
            .get(self.function.0 as usize)
            .ok_or_else(mismatch)?;
        if targets.len() != function.blocks.len() || boundaries.len() != targets.len() {
            return Err(mismatch());
        }
        let mut seen = vector(targets.len(), out)?;
        out.budget.charge_work(targets.len())?;
        seen.resize(targets.len(), false);
        let mut path = vector(targets.len(), out)?;
        for _ in 0..=targets.len() {
            self.check(out)?;
            out.budget.charge_work(4)?;
            if at.function != self.function {
                return Err(mismatch());
            }
            let index = at.block as usize;
            let target = targets.get(index).ok_or_else(mismatch)?;
            if target.coordinate(self.inventory)? != at {
                return Err(mismatch());
            }
            if *boundaries.get(index).ok_or_else(mismatch)? {
                return Ok((at, path));
            }
            if std::mem::replace(seen.get_mut(index).ok_or_else(mismatch)?, true)
                || !matches!(target.branch, TargetBranch::Goto)
                || target.edges.len() != 1
            {
                return Err(mismatch());
            }
            path.push(index);
            let appended = self.append(target, out)?;
            at = self.edge(appended, 0, out)?;
        }
        Err(mismatch())
    }
}

pub(super) fn headers() -> usize {
    type Frame<'a, 'i, 'g> = (
        ConcreteTrace<'i, 'g>,
        Result<ConcreteTrace<'i, 'g>>,
        &'i Inventory<'g>,
        &'a mut Writer<'a, 'a>,
        &'a TargetBlock,
        &'a [TargetBlock],
        &'a [bool],
        &'a [usize],
        Vec<usize>,
        Vec<bool>,
        Result<Vec<usize>>,
        Result<(Block, Vec<usize>)>,
        AppendedBlock<'a>,
        Result<AppendedBlock<'a>>,
        NodeV30,
        ExpressionV30,
        Result<ExpressionV30>,
        Result<Block>,
        Result<()>,
        Result<usize>,
        bool,
        [usize; 12],
        &'a mut ConcreteTrace<'i, 'g>,
        &'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'g>,
        &'a AppendedBlock<'a>,
    );
    size_of::<Frame<'_, '_, '_>>() + std::mem::align_of::<Frame<'_, '_, '_>>()
}

#[cfg(test)]
#[path = "original_semantic_mir_target_trace_v35_tests.rs"]
mod tests;
