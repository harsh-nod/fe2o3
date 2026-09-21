//! Bounded def-use closure of the actual flat identity prescan.
//!
//! This does not replace PLIRON verification or bound its dominance algorithm.
//! It establishes the local cardinality premise needed by that separate bound.

use super::*;
use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
    InvocationObserverV1, require_observed_v1,
};
use crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitV1;
use pliron::{
    linked_list::LinkedList,
    value::{DefiningEntity, Use},
};
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

struct Budget<'a> {
    limits: ProductionAnalysisResourceLimitsV1,
    work: usize,
    base: usize,
    peak: usize,
    incoming: usize,
    observer: Option<&'a InvocationObserverV1<'a, 'a>>,
}

impl<'a> Budget<'a> {
    fn new(limits: ProductionAnalysisResourceLimitsV1) -> Checked<Self> {
        Self::with_observer(limits, None)
    }

    fn with_observer(
        limits: ProductionAnalysisResourceLimitsV1,
        observer: Option<&'a InvocationObserverV1<'a, 'a>>,
    ) -> Checked<Self> {
        let mut budget = Self {
            limits,
            work: 0,
            base: FRAME_CELLS,
            peak: FRAME_CELLS,
            incoming: 0,
            observer,
        };
        budget.charge(32)?;
        Ok(budget)
    }
    fn bound(&self) -> Checked<ProductionAnalysisResourceUpperBoundV1> {
        ProductionAnalysisResourceUpperBoundV1::checked_phase(PHASE, self.work, 0, self.peak)
            .map_err(Failure::Resource)
    }
    fn require(&self) -> Checked<()> {
        require_observed_v1(self.limits, PHASE, Ok(self.bound()?), self.observer)
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

#[derive(Debug)]
struct Table {
    buckets: usize,
    storage: usize,
    lookup: usize,
}

impl Table {
    fn pointers<T>(entries: usize) -> Checked<Self> {
        Self::entries::<T, T>(entries, cells::<HashSet<T>>())
    }

    fn entries<K, E>(entries: usize, header: usize) -> Checked<Self> {
        // Pinned hashbrown 0.16.1 policy for pointer keys, not tiny scalar keys.
        if size_of::<K>() < 4 {
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
        let alignment = align_of::<E>().max(16);
        let payload = mul(buckets, size_of::<E>())?;
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
            add(16, mul(2, size_of::<K>())?)?,
            mul(add(buckets, 16)?, add(2, size_of::<K>())?)?,
        )?;
        Ok(Self {
            buckets,
            storage: add(header, heap.div_ceil(size_of::<usize>()))?,
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

#[derive(Debug)]
struct Owners {
    blocks: HashSet<Ptr<BasicBlock>>,
    operations: HashMap<Ptr<Operation>, usize>,
    block_table: Table,
    operation_table: Table,
    same_block_queries: usize,
    #[cfg(test)]
    index_lifetime: OrderIndexLifetime,
}

impl Owners {
    fn new(
        context: &Context,
        function: &FuncOp,
        prescan: &PrescanV1,
        budget: &mut Budget<'_>,
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
        let operation_table = Table::entries::<Ptr<Operation>, (Ptr<Operation>, usize)>(
            count,
            cells::<HashMap<Ptr<Operation>, usize>>(),
        )?;
        budget.base = add(
            FRAME_CELLS,
            add(floor, add(block_table.storage, operation_table.storage)?)?,
        )?;
        budget.scratch(0)?;
        budget.charge(add(add(block_table.buckets, operation_table.buckets)?, 32)?)?;
        let mut blocks = HashSet::new();
        let mut operations = HashMap::new();
        #[cfg(test)]
        trace(|trace| trace.owner_reservations += 1);
        blocks
            .try_reserve(prescan.blocks.len())
            .map_err(|_| resource("def-use block index allocation"))?;
        operations
            .try_reserve(count)
            .map_err(|_| resource("def-use operation index allocation"))?;
        #[cfg(test)]
        let index_lifetime = OrderIndexLifetime::new();
        if blocks.capacity() > block_table.usable()
            || operations.capacity() > operation_table.usable()
        {
            return Err(Failure::Invalid(
                "def-use owner index exceeds pinned capacity policy",
            ));
        }
        let region = function.get_region(context);
        budget.charge(4)?;
        let mut next_block = region.deref(context).get_head();
        for (ordinal, block) in prescan.blocks.iter().copied().enumerate() {
            budget.charge(add(block_table.lookup, 8)?)?;
            if !blocks.insert(block) || block.deref(context).get_parent_region() != Some(region) {
                return Err(Failure::Invalid(
                    "identity block roster is duplicate or detached",
                ));
            }
            if next_block != Some(block) {
                return Err(Failure::Invalid(
                    "identity block roster differs from physical order",
                ));
            }
            next_block = block.deref(context).get_next();
            let mut next_operation = block.deref(context).get_head();
            for (position, operation) in prescan.operations[ordinal].iter().copied().enumerate() {
                budget.charge(add(operation_table.lookup, 9)?)?;
                if operations.insert(operation, position).is_some()
                    || operation.deref(context).get_parent_block() != Some(block)
                {
                    return Err(Failure::Invalid(
                        "identity operation roster is duplicate or detached",
                    ));
                }
                if next_operation != Some(operation) {
                    return Err(Failure::Invalid(
                        "identity operation roster differs from physical order",
                    ));
                }
                next_operation = operation.deref(context).get_next();
            }
            budget.charge(4)?;
            if next_operation.is_some()
                || block.deref(context).get_tail() != prescan.operations[ordinal].last().copied()
            {
                return Err(Failure::Invalid(
                    "identity operation roster differs from physical order",
                ));
            }
        }
        budget.charge(4)?;
        if next_block.is_some()
            || region.deref(context).get_tail() != prescan.blocks.last().copied()
        {
            return Err(Failure::Invalid(
                "identity block roster differs from physical order",
            ));
        }
        Ok(Self {
            blocks,
            operations,
            block_table,
            operation_table,
            same_block_queries: 0,
            #[cfg(test)]
            index_lifetime,
        })
    }

    fn value(&self, context: &Context, value: Value, budget: &mut Budget<'_>) -> Checked<bool> {
        match value.defining_entity() {
            DefiningEntity::Op(operation) => {
                budget.charge(self.operation_table.lookup)?;
                if !self.operations.contains_key(&operation) {
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
        budget: &mut Budget<'_>,
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
            if !self.operations.contains_key(&user) {
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
        budget: &mut Budget<'_>,
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
            if !self.operations.contains_key(&user) {
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

/// Fresh, single-use order index for this synchronous immutable capture only.
/// It is not mutation-surviving analysis or durable graph identity.
pub(super) struct CheckedOrder<'a> {
    context: &'a Context,
    root: Ptr<Operation>,
    operations: HashMap<Ptr<Operation>, usize>,
    bound: ProductionAnalysisResourceUpperBoundV1,
    remaining_queries: usize,
    #[cfg(test)]
    _index_lifetime: OrderIndexLifetime,
}

impl fmt::Debug for CheckedOrder<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedOrder")
            .field("bound", &self.bound)
            .finish_non_exhaustive()
    }
}

impl CheckedOrder<'_> {
    pub(super) fn resource_upper_bound(&self) -> ProductionAnalysisResourceUpperBoundV1 {
        self.bound
    }

    pub(super) fn belongs_to(&self, context: &Context, function: &FuncOp) -> bool {
        std::ptr::eq(self.context, context) && self.root == function.get_operation()
    }

    pub(super) fn strictly_precedes(&mut self, definition: Ptr<Operation>, user: usize) -> bool {
        let Some(remaining) = self.remaining_queries.checked_sub(1) else {
            return false;
        };
        self.remaining_queries = remaining;
        self.operations
            .get(&definition)
            .is_some_and(|position| *position < user)
    }
}

pub(super) fn check<'a>(
    context: &'a Context,
    function: &FuncOp,
    prescan: &PrescanV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Checked<CheckedOrder<'a>> {
    check_with_observation_v1(context, function, prescan, limits, None)
}

pub(super) fn check_with_observation_v1<'a>(
    context: &'a Context,
    function: &FuncOp,
    prescan: &PrescanV1,
    limits: ProductionAnalysisResourceLimitsV1,
    observer: Option<&InvocationObserverV1<'_, '_>>,
) -> Checked<CheckedOrder<'a>> {
    let result = match observer {
        None => check_observed_inner_v1(context, function, prescan, limits, None),
        Some(observer) => observer.with_projection(&Ok, |nested| {
            check_observed_inner_v1(context, function, prescan, limits, Some(nested))
        }),
    };
    if let (Some(observer), Err(Failure::Resource(error))) = (observer, &result) {
        observer.deny(*error);
    }
    result
}

fn check_observed_inner_v1<'a>(
    context: &'a Context,
    function: &FuncOp,
    prescan: &PrescanV1,
    limits: ProductionAnalysisResourceLimitsV1,
    observer: Option<&InvocationObserverV1<'_, '_>>,
) -> Checked<CheckedOrder<'a>> {
    let mut budget = match observer {
        None => Budget::new(limits)?,
        Some(observer) => Budget::with_observer(limits, Some(observer))?,
    };
    let owners = match check_inner(context, function, prescan, &mut budget) {
        Ok(owners) => owners,
        Err(failure) => {
            budget.admit_failure(&failure)?;
            return Err(failure);
        }
    };
    budget.charge(add(
        8,
        mul(
            owners.same_block_queries,
            add(owners.operation_table.lookup, 4)?,
        )?,
    )?)?;
    let heap = owners
        .operation_table
        .storage
        .checked_sub(cells::<HashMap<Ptr<Operation>, usize>>())
        .ok_or_else(overflow)?;
    let retained = add(cells::<CheckedOrder<'_>>(), heap)?;
    budget.scratch(cells::<CheckedOrder<'_>>())?;
    let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        PHASE,
        budget.work,
        retained,
        budget.peak.checked_sub(retained).ok_or_else(overflow)?,
    )
    .map_err(Failure::Resource)?;
    Ok(CheckedOrder {
        context,
        root: function.get_operation(),
        operations: owners.operations,
        bound,
        remaining_queries: owners.same_block_queries,
        #[cfg(test)]
        _index_lifetime: owners.index_lifetime,
    })
}

fn check_inner(
    context: &Context,
    function: &FuncOp,
    prescan: &PrescanV1,
    budget: &mut Budget<'_>,
) -> Checked<Owners> {
    let mut owners = Owners::new(context, function, prescan, budget)?;
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
                budget.charge(4)?;
                if let DefiningEntity::Op(definition) = value.defining_entity()
                    && definition.deref(context).get_parent_block() == Some(prescan.blocks[block])
                {
                    owners.same_block_queries = add(owners.same_block_queries, 1)?;
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
    Ok(owners)
}

pub(super) fn identity_failure(
    context: &Context,
    prescan: &PrescanV1,
    failure: Failure,
) -> BuildIdentityFailureV1 {
    identity_failure_with_observation_v1(context, prescan, failure, None)
}

pub(super) fn identity_failure_with_observation_v1(
    context: &Context,
    prescan: &PrescanV1,
    failure: Failure,
    observer: RenderObserverV1<'_, '_, '_>,
) -> BuildIdentityFailureV1 {
    #[cfg(test)]
    require_order_index_retired_for_tests();
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
    let name = match render_operation_name_observed_v1(
        context,
        pointer,
        PlironPreserveLocationV1::Block { block },
        observer,
    ) {
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
    order_indexes: usize,
    live_order_indexes: usize,
    retired_order_indexes: usize,
    retirement_checks: usize,
}
#[cfg(test)]
std::thread_local! { static TRACE: std::cell::Cell<Trace> = const { std::cell::Cell::new(Trace {
    owner_reservations: 0, definition_rosters: 0, user_rosters: 0,
    value_use_vectors: 0, successor_use_vectors: 0, full_verifications: 0,
    order_indexes: 0, live_order_indexes: 0, retired_order_indexes: 0,
    retirement_checks: 0,
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
#[derive(Debug)]
struct OrderIndexLifetime;

#[cfg(test)]
impl OrderIndexLifetime {
    fn new() -> Self {
        trace(|trace| {
            trace.live_order_indexes += 1;
            trace.order_indexes += 1;
        });
        Self
    }
}

#[cfg(test)]
impl Drop for OrderIndexLifetime {
    fn drop(&mut self) {
        trace(|trace| {
            trace.live_order_indexes -= 1;
            trace.retired_order_indexes += 1;
        });
    }
}

#[cfg(test)]
pub(super) fn require_order_index_retired_for_tests() {
    assert_eq!(TRACE.get().live_order_indexes, 0);
    trace(|trace| trace.retirement_checks += 1);
}

#[cfg(test)]
pub(super) fn order_index_counts_for_tests() -> (usize, usize, usize, usize) {
    let trace = TRACE.get();
    (
        trace.order_indexes,
        trace.live_order_indexes,
        trace.retired_order_indexes,
        trace.retirement_checks,
    )
}

// Structural closure alone is independent of the production operation allowlist.
// This test census grants no identity or production analysis admission.
#[cfg(test)]
pub(super) fn native_census(context: &Context, function: &FuncOp) -> PrescanV1 {
    let blocks: Vec<_> = function
        .get_region(context)
        .deref(context)
        .iter(context)
        .collect();
    let operations: Vec<Vec<_>> = blocks
        .iter()
        .map(|block| block.deref(context).iter(context).collect())
        .collect();
    let mut scan = PrescanV1 {
        blocks,
        operations,
        values: 0,
        operands: 0,
        successors: 0,
        block_arguments: 0,
        attributes: 0,
        type_nodes: 0,
        max_operation_arity: 0,
        max_successor_arity: 0,
    };
    for block in &scan.blocks {
        scan.block_arguments += block.deref(context).get_num_arguments();
    }
    scan.values = scan.block_arguments;
    for operation in scan.operations.iter().flatten() {
        let raw = operation.deref(context);
        scan.values += raw.get_num_results();
        scan.operands += raw.get_num_operands();
        scan.successors += raw.get_num_successors();
    }
    scan
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod borrowed_observer_regression_tests {
    use super::*;
    use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as ReceiptFailure, InvocationReceiptV1 as Receipt,
    };
    use crate::production_analysis::pliron_resource_envelope::{
        ProductionAnalysisResourceLimitsV1 as Limits,
        ProductionAnalysisResourceUpperBoundV1 as Bound,
    };
    use dialect_kernel::{
        BranchArgsOp, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp, ReturnOp,
    };

    fn hard() -> Limits {
        Limits::production_hard_ceiling()
    }

    fn push(context: &Context, block: Ptr<BasicBlock>, op: impl Op) {
        op.get_operation().insert_at_back(block, context);
    }

    fn fixture(kind: u8) -> (Context, FuncOp) {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(&mut context, "observed".try_into().unwrap(), signature);
        let entry = function.get_entry_block(&context);
        let index = IndexType::get(&context).into();
        let join = BasicBlock::new(&mut context, None, vec![index]);
        join.insert_at_back(function.get_region(&context), &context);
        let constant = IndexConstantOp::new(&mut context, 7);
        let value = constant.result(&context);
        push(&context, entry, constant);
        let edge = BranchArgsOp::new(&mut context, vec![value], join);
        let pointer = edge.get_operation();
        push(&context, entry, edge);
        let ret = ReturnOp::new(&mut context);
        push(&context, join, ret);
        if kind != 0 {
            let signature = FunctionType::get(&context, vec![], vec![]);
            let foreign = FuncOp::new(&mut context, "foreign".try_into().unwrap(), signature);
            let block = foreign.get_entry_block(&context);
            let constant = IndexConstantOp::new(&mut context, 9);
            let external = constant.result(&context);
            push(&context, block, constant);
            if kind == 1 {
                Operation::replace_operand(pointer, &context, 0, external);
            } else {
                let user = IndexBinaryOp::new(&mut context, IndexBinaryKindAttr::Add, value, value);
                push(&context, block, user);
            }
            let ret = ReturnOp::new(&mut context);
            push(&context, block, ret);
        }
        (context, function)
    }

    #[test]
    fn semantic_rejections_keep_the_real_accepted_prefix_without_denial() {
        for kind in [1, 2] {
            let (context, function) = fixture(kind);
            let scan = prescan(&context, &function).unwrap();
            let mut budget = Budget::new(hard()).unwrap();
            let failure = check_inner(&context, &function, &scan, &mut budget).unwrap_err();
            budget.admit_failure(&failure).unwrap();
            TRACE.set(Trace::default());
            let mut receipt = Receipt::new(Bound::default(), hard()).unwrap();
            let phase = receipt.phase(PHASE, 0).unwrap();
            let error = check_with_observation_v1(
                &context,
                &function,
                &scan,
                hard(),
                Some(&phase.observer(&Ok)),
            )
            .unwrap_err();
            assert!(matches!(
                (kind, error),
                (
                    1,
                    Failure::Operand {
                        block: 0,
                        operation: 1,
                        operand: 0
                    }
                ) | (
                    2,
                    Failure::Invalid("SSA uses exceed this function's operand roster")
                )
            ));
            drop(phase);
            let state = receipt.snapshot();
            assert_eq!(state.current, state.committed);
            assert_eq!(
                (
                    state.committed.work_upper_bound(),
                    state.committed.peak_storage_upper_bound()
                ),
                (budget.work, budget.peak)
            );
            assert!(budget.work > 32);
            assert_eq!(state.first_denial, None);
            assert!(!state.caught_panic);
            assert_eq!(receipt.complete().unwrap(), state.committed);
            let trace = TRACE.get();
            assert_eq!(
                (
                    trace.owner_reservations,
                    trace.live_order_indexes,
                    trace.retired_order_indexes
                ),
                (1, 0, 1)
            );
        }
    }

    #[test]
    fn inherited_floor_exact_one_short_and_sticky_first_denial() {
        for short in [0, 1, 2] {
            TRACE.set(Trace::default());
            let (context, function) = fixture(0);
            let scan = prescan(&context, &function).unwrap();
            let floor_owner = check(&context, &function, &scan, hard()).unwrap();
            let bound = floor_owner.resource_upper_bound();
            assert_eq!(
                (
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound(),
                    bound.retained_storage_upper_bound()
                ),
                (4331, 167, 28)
            );
            let total = bound.checked_then_retain(bound, PHASE).unwrap();
            let limits = Limits::new(
                total.work_upper_bound() - usize::from(short == 1),
                total.peak_storage_upper_bound() - usize::from(short == 2),
            );
            {
                let mut receipt = Receipt::new(bound, limits).unwrap();
                let phase = receipt.phase(PHASE, 0).unwrap();
                let result = check_with_observation_v1(
                    &context,
                    &function,
                    &scan,
                    hard(),
                    Some(&phase.observer(&Ok)),
                );
                if short == 0 {
                    let owner = result.unwrap();
                    assert!(owner.belongs_to(&context, &function));
                    assert_eq!(owner.resource_upper_bound(), bound);
                    assert_eq!(TRACE.get().live_order_indexes, 2);
                    phase.commit(bound).unwrap();
                    assert_eq!(receipt.complete().unwrap(), bound);
                    let released = receipt
                        .drop_owner(PHASE, owner, bound.retained_storage_upper_bound())
                        .unwrap();
                    assert_eq!(
                        (
                            released.work_upper_bound(),
                            released.peak_storage_upper_bound(),
                            released.retained_storage_upper_bound()
                        ),
                        (4331, 167, 0)
                    );
                } else {
                    let Err(Failure::Resource(first)) = result else {
                        panic!("expected cumulative denial");
                    };
                    drop(phase);
                    assert_eq!(first.phase, PHASE);
                    assert_eq!(
                        first.resource,
                        if short == 1 {
                            "work upper bound"
                        } else {
                            "peak storage upper bound"
                        }
                    );
                    let before = receipt.snapshot();
                    assert!(before.committed.work_upper_bound() > 0);
                    assert_eq!(before.first_denial, Some(first));
                    limits
                        .require(
                            PHASE,
                            bound.checked_then_retain(before.committed, PHASE).unwrap(),
                        )
                        .unwrap();
                    let local = if short == 1 {
                        Limits::new(usize::MAX, 0)
                    } else {
                        Limits::new(0, usize::MAX)
                    };
                    let phase = receipt.phase(PHASE, 0).unwrap();
                    let Err(Failure::Resource(second)) = check_with_observation_v1(
                        &context,
                        &function,
                        &scan,
                        local,
                        Some(&phase.observer(&Ok)),
                    ) else {
                        panic!("expected second denial");
                    };
                    drop(phase);
                    assert_ne!(first.resource, second.resource);
                    assert_eq!(receipt.snapshot(), before);
                    assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(first)));
                }
                assert_eq!(TRACE.get().live_order_indexes, 1);
            }
            drop(floor_owner);
            assert_eq!(TRACE.get().live_order_indexes, 0);
        }
    }
}
