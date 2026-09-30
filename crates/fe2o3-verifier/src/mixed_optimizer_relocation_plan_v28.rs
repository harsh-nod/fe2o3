//! Private conditional-expression plan over actual independently checked LICM.
//! This is neither a generated theorem nor original-source/native authority.
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1 as InventoryError, CanonicalKirInventoryV18 as Inventory,
    CanonicalKirLicmOriginV1 as Origin, CheckedCanonicalKirLicmV18 as Pair,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirControlFlowScopeErrorV1 as FlowError,
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirOperationCoordinateV1 as Site,
    VerifiedCanonicalKernelIrModuleV18 as Owner, with_canonical_kir_control_flow_v18 as with_flow,
};
use std::{
    cell::Cell,
    fmt,
    mem::{align_of, size_of},
    ops::Range,
};

#[path = "mixed_optimizer_relocation_binding_v28.rs"]
mod binding;
#[path = "mixed_optimizer_relocation_plan_check_v28.rs"]
mod check;
pub use binding::{
    MixedOptimizerRelocationCfgSubjectV28, MixedOptimizerRelocationErrorV28,
    MixedOptimizerRelocationSubjectV28, PreparedMixedComposedRelocationCfgRefinementV28,
    PreparedMixedRelocationCfgRefinementV28, PreparedMixedRelocationExpressionsV28,
    prepare_mixed_relocation_expressions_v28,
};
#[cfg(test)]
#[path = "mixed_optimizer_relocation_plan_v28_tests.rs"]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Resource(Resource),
    Inventory(InventoryError),
    Flow(FlowError),
    Mismatch(&'static str),
}
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<InventoryError> for Error {
    fn from(error: InventoryError) -> Self {
        Self::Inventory(error)
    }
}
impl From<FlowError> for Error {
    fn from(error: FlowError) -> Self {
        Self::Flow(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "relocation expression plan: {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Node {
    input: usize,
    output: usize,
    operands: Range<usize>,
    results: Range<usize>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Operand {
    input: usize,
    expression: Option<(usize, usize)>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResultBinding {
    input: usize,
    output: usize,
    node: usize,
    result: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CutBinding {
    block: usize,
    result: usize,
}

/// Retains expression rows only; endpoint ownership and complete LICM lineage
/// remain borrowed. A source-bound V28 request must supply that separate owner.
pub(super) struct RelocationExpressionPlanV28<'a> {
    input: &'a Owner,
    output: &'a Owner,
    origins: &'a [Origin],
    nodes: Vec<Node>,
    operands: Vec<Operand>,
    results: Vec<ResultBinding>,
    order: Vec<usize>,
    cuts: Vec<CutBinding>,
    floor: usize,
    retained: usize,
    slot: usize,
    ledger: Ledger,
    cleanup: Cell<bool>,
}
impl RelocationExpressionPlanV28<'_> {
    fn custody(&self, budget: &Budget<'_>) -> Result<()> {
        if !self.cleanup.get()
            || self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || self
                .floor
                .checked_add(self.retained)
                .is_none_or(|floor| budget.storage() < floor)
        {
            self.cleanup.set(false);
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
    pub(super) fn replay(&self, pair: &Pair<'_>, budget: &mut Budget<'_>) -> Result<()> {
        self.custody(budget)?;
        check::replay(self, pair, budget)
    }
    pub(super) fn discard(self, budget: &mut Budget<'_>) -> Result<()> {
        let checked = self.custody(budget);
        let retained = self.retained;
        drop(self);
        checked?;
        budget.release_storage(retained)?;
        Ok(())
    }
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| Resource::Arithmetic.into())
}
fn mul(left: usize, right: usize) -> Result<usize> {
    left.checked_mul(right)
        .ok_or_else(|| Resource::Arithmetic.into())
}
fn reserve(budget: &mut Budget<'_>, paid: &mut usize, bytes: usize) -> Result<()> {
    let total = add(*paid, bytes)?;
    budget.reserve_storage(bytes)?;
    *paid = total;
    Ok(())
}
fn vector<T>(count: usize, budget: &mut Budget<'_>, paid: &mut usize) -> Result<Vec<T>> {
    budget.charge_work(add(count, 2)?)?;
    reserve(budget, paid, mul(count, size_of::<T>())?)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    reserve(
        budget,
        paid,
        mul(values.capacity() - count, size_of::<T>())?,
    )?;
    Ok(values)
}
fn initialized<T: Clone>(
    count: usize,
    value: T,
    budget: &mut Budget<'_>,
    paid: &mut usize,
) -> Result<Vec<T>> {
    let mut values = vector(count, budget, paid)?;
    values.resize(count, value);
    Ok(values)
}

fn block_index(inventory: &Inventory<'_>, block: Block) -> Result<usize> {
    let function = inventory
        .functions()
        .get(block.function.0 as usize)
        .ok_or(Error::Mismatch("block function"))?;
    let index = add(function.blocks.start, block.block as usize)?;
    if index >= function.blocks.end || inventory.blocks()[index].coordinate != block {
        return Err(Error::Mismatch("block coordinate"));
    }
    Ok(index)
}
fn operation_index(inventory: &Inventory<'_>, site: Site) -> Result<usize> {
    let block = &inventory.blocks()[block_index(inventory, site.block)?];
    let index = add(block.operations.start, site.operation as usize)?;
    if index >= block.operations.end || inventory.operations()[index].coordinate != site {
        return Err(Error::Mismatch("operation coordinate"));
    }
    Ok(index)
}

/// All heap output is prepaid before any scoped CFG query. Only private
/// constructors run here; no arbitrary callback or destructor owns this scratch.
pub(super) fn build<'a>(
    pair: &Pair<'a>,
    budget: &mut Budget<'_>,
) -> Result<RelocationExpressionPlanV28<'a>> {
    budget.check_prior_denials_v1()?;
    let floor = budget.storage();
    let mut paid = 0;
    let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        type Frame = (Vec<usize>, Vec<u8>, Vec<(usize, usize)>, [usize; 16]);
        reserve(
            budget,
            &mut paid,
            add(
                size_of::<RelocationExpressionPlanV28<'_>>(),
                align_of::<RelocationExpressionPlanV28<'_>>(),
            )?,
        )?;
        let retained_header = paid;
        reserve(
            budget,
            &mut paid,
            add(size_of::<Frame>(), align_of::<Frame>())?,
        )?;
        let (input, input_storage) = Inventory::derive_v18(pair.input(), budget)?;
        reserve(budget, &mut paid, input_storage.retained_storage())?;
        let (output, output_storage) = Inventory::derive_v18(pair.output(), budget)?;
        reserve(budget, &mut paid, output_storage.retained_storage())?;
        budget.charge_work(add(pair.origins().len(), 1)?)?;
        if pair.origins().len() != input.operations().len() {
            return Err(Error::Mismatch("complete LICM operation roster"));
        }
        let count = pair
            .origins()
            .iter()
            .filter(|row| row.hoist.is_some())
            .count();
        let before_map = paid;
        let mut operation_nodes =
            initialized(input.operations().len(), usize::MAX, budget, &mut paid)?;
        let map_storage = paid - before_map;
        let before_output = paid;
        let mut nodes = vector(count, budget, &mut paid)?;
        let mut operand_count = 0;
        let mut result_count = 0;
        for (index, row) in pair.origins().iter().enumerate() {
            budget.charge_work(4)?;
            if row.input != input.operations()[index].coordinate {
                return Err(Error::Mismatch("original LICM order"));
            }
            if row.hoist.is_none() {
                continue;
            }
            operation_nodes[index] = nodes.len();
            let target = operation_index(&output, row.output)?;
            let operands = operand_count;
            let results = result_count;
            operand_count = add(operand_count, input.operations()[index].operands.len())?;
            result_count = add(result_count, input.operations()[index].results.len())?;
            nodes.push(Node {
                input: index,
                output: target,
                operands: operands..operand_count,
                results: results..result_count,
            });
        }
        let mut operands = vector(operand_count, budget, &mut paid)?;
        let mut results = vector(result_count, budget, &mut paid)?;
        for (node_index, node) in nodes.iter().enumerate() {
            let original = &input.operations()[node.input];
            let final_op = &output.operations()[node.output];
            for at in original.operands.clone() {
                budget.charge_work(4)?;
                let definition = input.uses()[at].definition;
                let expression = match input.definitions()[definition].coordinate {
                    Definition::Result { operation, result } => {
                        let node = operation_nodes[operation_index(&input, operation)?];
                        (node != usize::MAX).then_some((node, result as usize))
                    }
                    _ => None,
                };
                operands.push(Operand {
                    input: definition,
                    expression,
                });
            }
            if original.results.len() != final_op.results.len() {
                return Err(Error::Mismatch("moved result arity"));
            }
            for (result, (input, output)) in original
                .results
                .clone()
                .zip(final_op.results.clone())
                .enumerate()
            {
                budget.charge_work(2)?;
                results.push(ResultBinding {
                    input,
                    output,
                    node: node_index,
                    result,
                });
            }
        }
        let mut order = vector(count, budget, &mut paid)?;
        let mut cuts = vector(mul(output.blocks().len(), result_count)?, budget, &mut paid)?;
        let output_storage_bytes = paid - before_output;
        let before_walk = paid;
        let mut state = initialized(count, 0u8, budget, &mut paid)?;
        let mut stack = vector::<(usize, usize)>(count, budget, &mut paid)?;
        for root in 0..count {
            budget.charge_work(1)?;
            if state[root] != 0 {
                continue;
            }
            state[root] = 1;
            stack.push((root, nodes[root].operands.start));
            while let Some((node, next)) = stack.last_mut() {
                budget.charge_work(3)?;
                if *next == nodes[*node].operands.end {
                    state[*node] = 2;
                    order.push(*node);
                    stack.pop();
                    continue;
                }
                let operand = operands[*next];
                *next += 1;
                if let Some((dependency, _)) = operand.expression {
                    match state[dependency] {
                        0 => {
                            state[dependency] = 1;
                            stack.push((dependency, nodes[dependency].operands.start));
                        }
                        1 => return Err(Error::Mismatch("cyclic moved expression")),
                        _ => (),
                    }
                }
            }
        }
        for function in output.functions() {
            if function.blocks.is_empty() {
                continue;
            }
            with_flow(
                pair.output(),
                function.coordinate,
                Default::default(),
                budget,
                |flow, budget| {
                    for block in function.blocks.clone() {
                        let at = output.blocks()[block].coordinate;
                        for (result_index, result) in results.iter().enumerate() {
                            budget.charge_work(3)?;
                            let site = output.operations()[nodes[result.node].output].coordinate;
                            if site.block.function == function.coordinate
                                && site.block != at
                                && flow.dominates(site.block, at, budget)?
                            {
                                cuts.push(CutBinding {
                                    block,
                                    result: result_index,
                                });
                            }
                        }
                    }
                    Ok::<(), Error>(())
                },
            )?;
        }
        let scratch = add(
            add(
                input_storage.retained_storage(),
                output_storage.retained_storage(),
            )?,
            add(
                add(size_of::<Frame>(), align_of::<Frame>())?,
                add(map_storage, paid - before_walk)?,
            )?,
        )?;
        drop((input, output, operation_nodes, state, stack));
        budget.release_storage(scratch)?;
        paid -= scratch;
        if paid != add(retained_header, output_storage_bytes)?
            || budget.storage() != add(floor, paid)?
        {
            return Err(Resource::Accounting.into());
        }
        let plan = RelocationExpressionPlanV28 {
            input: pair.input(),
            output: pair.output(),
            origins: pair.origins(),
            nodes,
            operands,
            results,
            order,
            cuts,
            floor,
            retained: paid,
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            cleanup: Cell::new(true),
        };
        plan.replay(pair, budget)?;
        Ok(plan)
    }));
    match built {
        Ok(Ok(plan)) => Ok(plan),
        other => {
            // Every owned temporary has been destroyed inside the caught
            // constructor before its exact accepted reservations are released.
            if budget.storage() == add(floor, paid)? {
                budget.release_storage(paid)?;
            }
            match other {
                Ok(Err(error)) => Err(error),
                Err(payload) => std::panic::resume_unwind(payload),
                _ => unreachable!(),
            }
        }
    }
}
