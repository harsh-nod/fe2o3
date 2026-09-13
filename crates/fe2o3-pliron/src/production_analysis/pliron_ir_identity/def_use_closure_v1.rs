//! Bounded def-use closure of the actual flat identity prescan.
//!
//! This does not replace PLIRON verification or bound its dominance algorithm.
//! It establishes the local cardinality premise needed by that separate bound.

use super::*;
use crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitV1;
use pliron::value::{DefiningEntity, Use};
use std::{
    collections::HashSet,
    mem::{align_of, size_of},
};

const PHASE: ProductionAnalysisResourcePhaseV1 =
    ProductionAnalysisResourcePhaseV1::StructuralIdentity;
const FRAME_CELLS: usize = 64;

#[derive(Debug)]
pub(super) enum Failure {
    Resource(ProductionAnalysisResourceLimitV1),
    Invalid(&'static str),
    Operand {
        block: usize,
        operation: usize,
        operand: usize,
    },
    Successor {
        block: usize,
        operation: usize,
        successor: usize,
    },
}

type Checked<T> = Result<T, Failure>;

fn overflow() -> Failure {
    resource("def-use closure resource arithmetic")
}
fn resource(resource: &'static str) -> Failure {
    Failure::Resource(ProductionAnalysisResourceLimitV1 {
        phase: PHASE,
        resource,
    })
}
fn add(a: usize, b: usize) -> Checked<usize> {
    a.checked_add(b).ok_or_else(overflow)
}
fn mul(a: usize, b: usize) -> Checked<usize> {
    a.checked_mul(b).ok_or_else(overflow)
}
fn cells<T>() -> usize {
    size_of::<T>().div_ceil(size_of::<usize>())
}

struct Budget {
    limits: ProductionAnalysisResourceLimitsV1,
    work: usize,
    base: usize,
    peak: usize,
    incoming: usize,
}

impl Budget {
    fn new(limits: ProductionAnalysisResourceLimitsV1) -> Checked<Self> {
        let mut budget = Self {
            limits,
            work: 0,
            base: FRAME_CELLS,
            peak: FRAME_CELLS,
            incoming: 0,
        };
        budget.charge(32)?;
        Ok(budget)
    }
    fn bound(&self) -> Checked<ProductionAnalysisResourceUpperBoundV1> {
        ProductionAnalysisResourceUpperBoundV1::checked_phase(PHASE, self.work, 0, self.peak)
            .map_err(Failure::Resource)
    }
    fn require(&self) -> Checked<()> {
        self.limits
            .require(PHASE, self.bound()?)
            .map(|_| ())
            .map_err(Failure::Resource)
    }
    fn charge(&mut self, work: usize) -> Checked<()> {
        self.work = add(self.work, work)?;
        self.require()
    }
    fn scratch(&mut self, extra: usize) -> Checked<()> {
        self.peak = self.peak.max(add(self.base, extra)?);
        self.require()
    }

    fn admit_failure(&mut self, failure: &Failure) -> Checked<()> {
        let (work, bytes) = match failure {
            Failure::Resource(_) => return Ok(()),
            Failure::Invalid(detail) => (add(32, detail.len())?, detail.len()),
            Failure::Operand { .. } | Failure::Successor { .. } => {
                // Prescan already bounded the closed operation name. Cover its
                // OpObj, identifier copies, rendering growth and final strings.
                let bytes = mul(4, MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1)?;
                (add(64, bytes)?, bytes)
            }
        };
        // The owner indexes and use vectors have dropped with check_inner.
        self.base = add(FRAME_CELLS, self.incoming)?;
        self.scratch(bytes.div_ceil(size_of::<usize>()))?;
        self.charge(work)
    }
}

struct Table {
    buckets: usize,
    storage: usize,
    lookup: usize,
}

impl Table {
    fn pointers<T>(entries: usize) -> Checked<Self> {
        // Pinned hashbrown 0.16.1 policy for pointer keys, not tiny scalar keys.
        if size_of::<T>() < 4 {
            return Err(overflow());
        }
        let buckets = if entries == 0 {
            0
        } else {
            mul(entries, 8)?
                .div_ceil(7)
                .checked_next_power_of_two()
                .ok_or_else(overflow)?
                .max(4)
        };
        let alignment = align_of::<T>().max(16);
        let payload = mul(buckets, size_of::<T>())?;
        let heap = if buckets == 0 {
            0
        } else {
            add(
                mul(payload.div_ceil(alignment), alignment)?,
                add(buckets, 16)?,
            )?
        };
        // Prepay a complete probe cycle, including the final control group.
        // No collision-free or worst-case constant-time hash assumption.
        let lookup = add(
            add(16, mul(2, size_of::<T>())?)?,
            mul(add(buckets, 16)?, add(2, size_of::<T>())?)?,
        )?;
        Ok(Self {
            buckets,
            storage: add(cells::<HashSet<T>>(), heap.div_ceil(size_of::<usize>()))?,
            lookup,
        })
    }
    fn usable(&self) -> usize {
        if self.buckets < 8 {
            self.buckets.saturating_sub(1)
        } else {
            self.buckets / 8 * 7
        }
    }
}

fn use_storage<T>(count: usize) -> Checked<usize> {
    // SmallSet's exact-size iterator is not TrustedLen on the pinned toolchain.
    let capacity = if count == 0 { 0 } else { count.max(4) };
    add(cells::<Vec<T>>(), mul(capacity, cells::<T>())?)
}

struct Owners {
    blocks: HashSet<Ptr<BasicBlock>>,
    operations: HashSet<Ptr<Operation>>,
    block_table: Table,
    operation_table: Table,
}

impl Owners {
    fn new(
        context: &Context,
        function: &FuncOp,
        prescan: &PrescanV1,
        budget: &mut Budget,
    ) -> Checked<Self> {
        budget.charge(add(16, prescan.operations.len())?)?;
        let mut floor = add(
            cells::<PrescanV1>(),
            mul(prescan.blocks.capacity(), cells::<Ptr<BasicBlock>>())?,
        )?;
        floor = add(
            floor,
            mul(
                prescan.operations.capacity(),
                cells::<Vec<Ptr<Operation>>>(),
            )?,
        )?;
        let mut count = 0;
        for operations in &prescan.operations {
            floor = add(
                floor,
                mul(operations.capacity(), cells::<Ptr<Operation>>())?,
            )?;
            count = add(count, operations.len())?;
        }
        budget.incoming = floor;
        if prescan.blocks.len() != prescan.operations.len() {
            return Err(Failure::Invalid(
                "identity block and operation rosters disagree",
            ));
        }
        let block_table = Table::pointers::<Ptr<BasicBlock>>(prescan.blocks.len())?;
        let operation_table = Table::pointers::<Ptr<Operation>>(count)?;
        budget.base = add(
            FRAME_CELLS,
            add(floor, add(block_table.storage, operation_table.storage)?)?,
        )?;
        budget.scratch(0)?;
        budget.charge(add(add(block_table.buckets, operation_table.buckets)?, 32)?)?;
        let mut blocks = HashSet::new();
        let mut operations = HashSet::new();
        #[cfg(test)]
        trace(|trace| trace.owner_reservations += 1);
        blocks
            .try_reserve(prescan.blocks.len())
            .map_err(|_| resource("def-use block index allocation"))?;
        operations
            .try_reserve(count)
            .map_err(|_| resource("def-use operation index allocation"))?;
        if blocks.capacity() > block_table.usable()
            || operations.capacity() > operation_table.usable()
        {
            return Err(Failure::Invalid(
                "def-use owner index exceeds pinned capacity policy",
            ));
        }
        let region = function.get_region(context);
        for (ordinal, block) in prescan.blocks.iter().copied().enumerate() {
            budget.charge(add(block_table.lookup, 4)?)?;
            if !blocks.insert(block) || block.deref(context).get_parent_region() != Some(region) {
                return Err(Failure::Invalid(
                    "identity block roster is duplicate or detached",
                ));
            }
            for operation in prescan.operations[ordinal].iter().copied() {
                budget.charge(add(operation_table.lookup, 4)?)?;
                if !operations.insert(operation)
                    || operation.deref(context).get_parent_block() != Some(block)
                {
                    return Err(Failure::Invalid(
                        "identity operation roster is duplicate or detached",
                    ));
                }
            }
        }
        Ok(Self {
            blocks,
            operations,
            block_table,
            operation_table,
        })
    }

    fn value(&self, context: &Context, value: Value, budget: &mut Budget) -> Checked<bool> {
        match value.defining_entity() {
            DefiningEntity::Op(operation) => {
                budget.charge(self.operation_table.lookup)?;
                if !self.operations.contains(&operation) {
                    return Ok(false);
                }
                let raw = operation.deref(context);
                #[cfg(test)]
                trace(|trace| trace.definition_rosters += 1);
                for result in raw.results() {
                    budget.charge(1)?;
                    if result == value {
                        return Ok(true);
                    }
                }
            }
            DefiningEntity::Block(block) => {
                budget.charge(self.block_table.lookup)?;
                if !self.blocks.contains(&block) {
                    return Ok(false);
                }
                let raw = block.deref(context);
                #[cfg(test)]
                trace(|trace| trace.definition_rosters += 1);
                for argument in raw.arguments() {
                    budget.charge(1)?;
                    if argument == value {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    fn scalar_uses(
        &self,
        context: &Context,
        value: Value,
        ordinal: usize,
        remaining: &mut usize,
        budget: &mut Budget,
    ) -> Checked<()> {
        let prefix = add(ordinal, 1)?;
        budget.charge(add(8, prefix)?)?;
        let count = value.num_uses(context);
        if count > *remaining {
            return Err(Failure::Invalid(
                "SSA uses exceed this function's operand roster",
            ));
        }
        if count == 0 {
            return Ok(());
        }
        budget.scratch(use_storage::<Use<Value>>(count)?)?;
        budget.charge(add(prefix, mul(count, 4)?)?)?;
        #[cfg(test)]
        trace(|trace| trace.value_use_vectors += 1);
        let uses = value.uses(context);
        if uses.len() != count || uses.capacity() > count.max(4) {
            return Err(Failure::Invalid(
                "SSA use roster exceeds its admitted shape",
            ));
        }
        for usage in uses {
            budget.charge(self.operation_table.lookup)?;
            let user = usage.user_op();
            if !self.operations.contains(&user) {
                return Err(Failure::Invalid("SSA use leaves this function"));
            }
            #[cfg(test)]
            trace(|trace| trace.user_rosters += 1);
            let raw = user.deref(context);
            let mut found = false;
            for index in 0..raw.get_num_operands() {
                budget.charge(1)?;
                if raw.get_operand_as_use(index) == usage {
                    found = raw.get_operand(index) == value;
                    break;
                }
            }
            if !found {
                return Err(Failure::Invalid(
                    "SSA use does not identify its exact local operand",
                ));
            }
        }
        *remaining -= count;
        Ok(())
    }

    fn successor_uses(
        &self,
        context: &Context,
        block: Ptr<BasicBlock>,
        remaining: &mut usize,
        budget: &mut Budget,
    ) -> Checked<()> {
        budget.charge(8)?;
        let count = block.num_preds(context);
        if count > *remaining {
            return Err(Failure::Invalid(
                "block uses exceed this function's successor roster",
            ));
        }
        if count == 0 {
            return Ok(());
        }
        budget.scratch(use_storage::<Use<Ptr<BasicBlock>>>(count)?)?;
        budget.charge(mul(count, 4)?)?;
        #[cfg(test)]
        trace(|trace| trace.successor_use_vectors += 1);
        let uses = block.uses(context);
        if uses.len() != count || uses.capacity() > count.max(4) {
            return Err(Failure::Invalid(
                "successor use roster exceeds its admitted shape",
            ));
        }
        for usage in uses {
            budget.charge(self.operation_table.lookup)?;
            let user = usage.user_op();
            if !self.operations.contains(&user) {
                return Err(Failure::Invalid("successor use leaves this function"));
            }
            #[cfg(test)]
            trace(|trace| trace.user_rosters += 1);
            let raw = user.deref(context);
            let mut found = false;
            for index in 0..raw.get_num_successors() {
                budget.charge(1)?;
                if raw.get_successor_as_use(index) == usage {
                    found = raw.get_successor(index) == block;
                    break;
                }
            }
            if !found {
                return Err(Failure::Invalid(
                    "successor use does not identify its exact local edge",
                ));
            }
        }
        *remaining -= count;
        Ok(())
    }
}

pub(super) fn check(
    context: &Context,
    function: &FuncOp,
    prescan: &PrescanV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Checked<ProductionAnalysisResourceUpperBoundV1> {
    let mut budget = Budget::new(limits)?;
    if let Err(failure) = check_inner(context, function, prescan, &mut budget) {
        budget.admit_failure(&failure)?;
        return Err(failure);
    }
    budget.bound()
}

fn check_inner(
    context: &Context,
    function: &FuncOp,
    prescan: &PrescanV1,
    budget: &mut Budget,
) -> Checked<()> {
    let owners = Owners::new(context, function, prescan, budget)?;
    // Check direction-sensitive ownership before reverse lists can follow an
    // external user. Lookup errors remain compact; no upstream error rendering.
    for (block, operations) in prescan.operations.iter().enumerate() {
        for (operation, pointer) in operations.iter().copied().enumerate() {
            budget.charge(4)?;
            let raw = pointer.deref(context);
            for (operand, value) in raw.operands().enumerate() {
                budget.charge(4)?;
                if !owners.value(context, value, budget)? {
                    return Err(Failure::Operand {
                        block,
                        operation,
                        operand,
                    });
                }
            }
            for (successor, target) in raw.successors().enumerate() {
                budget.charge(add(4, owners.block_table.lookup)?)?;
                if !owners.blocks.contains(&target) {
                    return Err(Failure::Successor {
                        block,
                        operation,
                        successor,
                    });
                }
            }
        }
    }
    let mut operands = prescan.operands;
    let mut successors = prescan.successors;
    for (ordinal, block) in prescan.blocks.iter().copied().enumerate() {
        budget.charge(4)?;
        for (argument, value) in block.deref(context).arguments().enumerate() {
            owners.scalar_uses(context, value, argument, &mut operands, budget)?;
        }
        for operation in &prescan.operations[ordinal] {
            budget.charge(4)?;
            for (result, value) in operation.deref(context).results().enumerate() {
                owners.scalar_uses(context, value, result, &mut operands, budget)?;
            }
        }
        owners.successor_uses(context, block, &mut successors, budget)?;
    }
    if operands != 0 || successors != 0 {
        return Err(Failure::Invalid(
            "local operands or successors have missing use backlinks",
        ));
    }
    // Unique use identities plus exact-slot checks and equal cardinality prove
    // closure without retaining a second, A/S-sized use table.
    Ok(())
}

pub(super) fn identity_failure(
    context: &Context,
    prescan: &PrescanV1,
    failure: Failure,
) -> BuildIdentityFailureV1 {
    let (block, operation) = match failure {
        Failure::Resource(error) => return BuildIdentityFailureV1::ResourceLimit(error),
        Failure::Invalid(detail) => {
            return PlironIrIdentityErrorV1::StructuralVerificationFailed {
                detail: detail.to_owned(),
            }
            .into();
        }
        Failure::Operand {
            block, operation, ..
        }
        | Failure::Successor {
            block, operation, ..
        } => (block, operation),
    };
    let pointer = prescan.operations[block][operation];
    let name =
        match render_operation_name_v1(context, pointer, PlironPreserveLocationV1::Block { block })
        {
            Ok(name) => name,
            Err(error) => return error.into(),
        };
    let location = PlironPreserveLocationV1::Operation {
        block,
        operation,
        name,
    };
    match failure {
        Failure::Operand { operand, .. } => PlironIrIdentityErrorV1::ExternalOperand {
            location,
            operand,
            value: "<external SSA value>".to_owned(),
        }
        .into(),
        Failure::Successor { successor, .. } => PlironIrIdentityErrorV1::ExternalSuccessor {
            location,
            successor,
        }
        .into(),
        _ => unreachable!(),
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default)]
struct Trace {
    owner_reservations: usize,
    definition_rosters: usize,
    user_rosters: usize,
    value_use_vectors: usize,
    successor_use_vectors: usize,
    full_verifications: usize,
}
#[cfg(test)]
std::thread_local! { static TRACE: std::cell::Cell<Trace> = const { std::cell::Cell::new(Trace {
    owner_reservations: 0, definition_rosters: 0, user_rosters: 0,
    value_use_vectors: 0, successor_use_vectors: 0, full_verifications: 0,
}) }; }
#[cfg(test)]
fn trace(update: impl FnOnce(&mut Trace)) {
    TRACE.with(|trace| {
        let mut value = trace.get();
        update(&mut value);
        trace.set(value);
    });
}
#[cfg(test)]
pub(super) fn full_verification() {
    trace(|trace| trace.full_verifications += 1);
}

#[cfg(test)]
mod tests;
