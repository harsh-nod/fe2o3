//! Exact CSE scoped by Pliron's dominator tree, with caller-owned accounting.
//!
//! This is a raw, nontransactional transform of already verified SSA regions.
//! On an error or observer failure the production caller must discard its private
//! candidate. No policy, receipt identity, or publication authority is selected
//! here. Non-SSA regions and entry-unreachable blocks are not transformed.
//!
//! Accounting covers new traversal/index storage and structured key visits.
//! Pliron dominance construction, verification, dynamic attribute hash/equality,
//! rewriter internals, and allocator bookkeeping remain opaque upstream costs;
//! the fixed execution envelope must cover those separately.

use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    error::Error,
    fmt,
    hash::{Hash, Hasher},
    mem::size_of,
};

use pliron::{
    basic_block::BasicBlock,
    context::{Context, Ptr},
    graph::dominance::{DomInfo, DomTree},
    irbuild::{
        IRStatus,
        listener::DummyListener,
        observer::RewriteObserver,
        rewriter::{IRRewriter, Rewriter},
    },
    linked_list::{ContainsLinkedList, LinkedList},
    operation::Operation,
    pass::{AnalysisManager, Pass, PassResult},
    region::Region,
    value::Value,
};

use crate::cse_v1::BorrowedPureCseKeyV1;

/// Authority-free adapter to the caller's existing ledger. A denied operation
/// must retain its original error and failure history. Storage units are bytes
/// of visible Vec capacity and conservative logical hash-table slots, not RSS.
pub trait DominanceCseBudgetV1 {
    /// The caller's original denial type; it is returned without replacement.
    type Error;
    /// Charge before the corresponding bounded work is performed.
    fn charge_work(&mut self, work: usize) -> Result<(), Self::Error>;
    /// Reserve live storage before allocation, including subsequent excess.
    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Self::Error>;
    /// Release only after the corresponding owned storage has been dropped.
    fn release_storage(&mut self, bytes: usize);
}

/// A caller denial is distinct from local arithmetic/allocation failure.
#[derive(Debug)]
pub enum DominanceCseErrorV1<E> {
    /// Original caller error, including its original failed resource request.
    Budget(E),
    /// The requested logical bound is not representable.
    Overflow,
    /// A fallible collection allocation failed.
    Allocation,
}

impl<E: fmt::Display> fmt::Display for DominanceCseErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Budget(error) => write!(formatter, "dominance CSE budget: {error}"),
            Self::Overflow => formatter.write_str("dominance CSE resource overflow"),
            Self::Allocation => formatter.write_str("dominance CSE allocation failed"),
        }
    }
}

impl<E: Error + 'static> Error for DominanceCseErrorV1<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Budget(error) => Some(error),
            Self::Overflow | Self::Allocation => None,
        }
    }
}

/// An explicitly budgeted Pass adapter, not a new selectable production policy.
pub struct DominancePureCsePassV1<'a, B> {
    budget: &'a mut B,
}

impl<'a, B: DominanceCseBudgetV1> DominancePureCsePassV1<'a, B> {
    /// Borrow the same caller ledger for the entire pass invocation. The caller
    /// must separately admit the opaque upstream analysis/execution envelope.
    pub fn new(budget: &'a mut B) -> Self {
        Self { budget }
    }
}

impl<B> Pass for DominancePureCsePassV1<'_, B>
where
    B: DominanceCseBudgetV1,
    B::Error: Error + Send + Sync + 'static,
{
    fn run(
        &mut self,
        root: Ptr<Operation>,
        context: &mut Context,
        analyses: &mut AnalysisManager,
    ) -> pliron::result::Result<PassResult> {
        let mut dominance = analyses.get_analysis_mut::<DomInfo>(root, context)?;
        let changed = dominance_pure_cse_v1(root, context, &mut dominance, self.budget)
            .map_err(|error| pliron::input_error_noloc!(error))?;
        let mut result = PassResult::default();
        result.ir_changed = changed;
        result.set_preserved::<DomInfo>();
        Ok(result)
    }

    fn name(&self) -> &str {
        "gpu-dominance-pure-cse-v1"
    }
}

/// Eliminate exact, verified total expressions using an up-to-date DomInfo.
pub fn dominance_pure_cse_v1<B: DominanceCseBudgetV1>(
    root: Ptr<Operation>,
    context: &mut Context,
    dominance: &mut DomInfo,
    budget: &mut B,
) -> Result<IRStatus, DominanceCseErrorV1<B::Error>> {
    dominance_pure_cse_impl(root, context, dominance, budget, None)
}

/// The identical transform with the existing independent mutation observer.
pub fn dominance_pure_cse_with_observer_v1<B: DominanceCseBudgetV1>(
    root: Ptr<Operation>,
    context: &mut Context,
    dominance: &mut DomInfo,
    budget: &mut B,
    observer: Box<dyn RewriteObserver>,
) -> Result<IRStatus, DominanceCseErrorV1<B::Error>> {
    dominance_pure_cse_impl(root, context, dominance, budget, Some(observer))
}

// This guard is created before its collections and therefore drops after them,
// including unwinding. Region guards borrow the same ledger independently.
struct StorageScope<'a, B: DominanceCseBudgetV1> {
    budget: &'a mut B,
    live: usize,
}

impl<'a, B: DominanceCseBudgetV1> StorageScope<'a, B> {
    fn new(budget: &'a mut B) -> Self {
        Self { budget, live: 0 }
    }

    fn work(&mut self, work: usize) -> Result<(), DominanceCseErrorV1<B::Error>> {
        self.budget
            .charge_work(work)
            .map_err(DominanceCseErrorV1::Budget)
    }

    fn reserve(&mut self, bytes: usize) -> Result<(), DominanceCseErrorV1<B::Error>> {
        let live = self
            .live
            .checked_add(bytes)
            .ok_or(DominanceCseErrorV1::Overflow)?;
        self.budget
            .reserve_storage(bytes)
            .map_err(DominanceCseErrorV1::Budget)?;
        self.live = live;
        Ok(())
    }

    fn release(&mut self, bytes: usize) {
        self.live -= bytes;
        self.budget.release_storage(bytes);
    }
}

impl<B: DominanceCseBudgetV1> Drop for StorageScope<'_, B> {
    fn drop(&mut self) {
        self.budget.release_storage(self.live);
    }
}

fn bytes<T, E>(capacity: usize) -> Result<usize, DominanceCseErrorV1<E>> {
    capacity
        .checked_mul(size_of::<T>())
        .ok_or(DominanceCseErrorV1::Overflow)
}

fn reserve_vec<T, B: DominanceCseBudgetV1>(
    values: &mut Vec<T>,
    additional: usize,
    scope: &mut StorageScope<'_, B>,
) -> Result<(), DominanceCseErrorV1<B::Error>> {
    let required = values
        .len()
        .checked_add(additional)
        .ok_or(DominanceCseErrorV1::Overflow)?;
    let before = values.capacity();
    if required <= before {
        return Ok(());
    }
    let requested = before
        .checked_mul(2)
        .ok_or(DominanceCseErrorV1::Overflow)?
        .max(4)
        .max(required);
    scope.work(before.checked_add(2).ok_or(DominanceCseErrorV1::Overflow)?)?;
    scope.reserve(bytes::<T, _>(requested - before)?)?;
    values
        .try_reserve_exact(requested - values.len())
        .map_err(|_| DominanceCseErrorV1::Allocation)?;
    // Reconcile immediately, before another fallible operation. A denial keeps
    // the actual excess request in the caller's history while cleanup drops Vec.
    reconcile_vec_capacity::<T, B>(requested, values.capacity(), scope)
}

fn reconcile_vec_capacity<T, B: DominanceCseBudgetV1>(
    requested: usize,
    actual: usize,
    scope: &mut StorageScope<'_, B>,
) -> Result<(), DominanceCseErrorV1<B::Error>> {
    if actual > requested {
        scope.reserve(bytes::<T, _>(actual - requested)?)?;
    }
    Ok(())
}

// HashMap does not expose allocation layout. Charge a conservative logical
// table envelope: next power-of-two slots above reported capacity, with a full
// key/value pair, one usize, and control byte per slot. This is not allocator RSS.
fn table_bytes<E>(capacity: usize) -> Result<usize, DominanceCseErrorV1<E>> {
    if capacity == 0 {
        return Ok(0);
    }
    capacity
        .checked_add(1)
        .and_then(usize::checked_next_power_of_two)
        .and_then(|slots| slots.checked_mul(size_of::<(u64, usize)>() + size_of::<usize>() + 1))
        .ok_or(DominanceCseErrorV1::Overflow)
}

fn reserve_table<B: DominanceCseBudgetV1>(
    table: &mut HashMap<u64, usize>,
    scope: &mut StorageScope<'_, B>,
) -> Result<(), DominanceCseErrorV1<B::Error>> {
    if table.len() < table.capacity() {
        return Ok(());
    }
    let before = table.capacity();
    let requested = before
        .checked_mul(2)
        .ok_or(DominanceCseErrorV1::Overflow)?
        .max(4);
    scope.work(before.checked_add(2).ok_or(DominanceCseErrorV1::Overflow)?)?;
    let prepaid = table_bytes::<B::Error>(requested)?;
    scope.reserve(prepaid - table_bytes::<B::Error>(before)?)?;
    table
        .try_reserve(requested - table.len())
        .map_err(|_| DominanceCseErrorV1::Allocation)?;
    let actual = table_bytes::<B::Error>(table.capacity())?;
    if actual > prepaid {
        scope.reserve(actual - prepaid)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Available {
    key: BorrowedPureCseKeyV1,
    fingerprint: u64,
    previous: Option<usize>,
}

#[derive(Clone, Copy)]
enum Visit {
    Enter(Ptr<BasicBlock>),
    Exit(usize),
}

fn dominance_pure_cse_impl<B: DominanceCseBudgetV1>(
    root: Ptr<Operation>,
    context: &mut Context,
    dominance: &mut DomInfo,
    budget: &mut B,
    observer: Option<Box<dyn RewriteObserver>>,
) -> Result<IRStatus, DominanceCseErrorV1<B::Error>> {
    let mut scope = StorageScope::new(budget);
    scope.work(1)?;
    scope.reserve(size_of::<Vec<Ptr<Operation>>>())?;
    let mut pending = Vec::new();
    reserve_vec(&mut pending, 1, &mut scope)?;
    pending.push(root);
    let mut rewriter = IRRewriter::<DummyListener>::default();
    rewriter.set_observer(observer);
    rewriter.get_config_mut().set_name_on_value_replacement = false;
    while let Some(container) = pending.pop() {
        scope.work(1)?;
        let nested_start = pending.len();
        let region_count = container.deref(context).num_regions();
        for index in 0..region_count {
            scope.work(1)?;
            let region = container.deref(context).get_region(index);
            for block in region.deref(context).iter(context) {
                scope.work(1)?;
                for operation in block.deref(context).iter(context) {
                    scope.work(1)?;
                    if operation.deref(context).num_regions() != 0 {
                        reserve_vec(&mut pending, 1, &mut scope)?;
                        pending.push(operation);
                    }
                }
            }
            if region.deref(context).has_ssa_dominance(context) {
                scope.work(1)?;
                let tree = dominance.get_dom_tree(context, region);
                eliminate_region(tree, context, &mut rewriter, scope.budget)?;
            }
        }
        scope.work(pending.len() - nested_start)?;
        pending[nested_start..].reverse();
    }
    Ok(rewriter.is_modified().into())
}

fn eliminate_region<B: DominanceCseBudgetV1>(
    tree: &DomTree<Ptr<Region>, Context>,
    context: &mut Context,
    rewriter: &mut IRRewriter<DummyListener>,
    budget: &mut B,
) -> Result<(), DominanceCseErrorV1<B::Error>> {
    let mut scope = StorageScope::new(budget);
    scope.work(1)?;
    let Some(entry) = tree.root() else {
        return Ok(());
    };
    scope.reserve(
        size_of::<HashMap<u64, usize>>()
            + size_of::<Vec<Available>>()
            + size_of::<Vec<Visit>>()
            + size_of::<Vec<Value>>(),
    )?;
    let mut table = HashMap::new();
    let mut available = Vec::<Available>::new();
    let mut visits = Vec::new();
    // Pliron consumes this Vec; release its capacity only after the call returns.
    let mut replacements = Vec::new();
    reserve_vec(&mut visits, 1, &mut scope)?;
    visits.push(Visit::Enter(entry));
    while let Some(visit) = visits.pop() {
        scope.work(1)?;
        match visit {
            Visit::Exit(start) => {
                while available.len() > start {
                    scope.work(1)?;
                    let old = available.pop().expect("nonempty scope");
                    if let Some(previous) = old.previous {
                        table.insert(old.fingerprint, previous);
                    } else {
                        table.remove(&old.fingerprint);
                    }
                }
            }
            Visit::Enter(block) => {
                let start = available.len();
                let mut current = block.deref(context).get_head();
                while let Some(operation) = current {
                    scope.work(1)?;
                    current = operation.deref(context).get_next();
                    let width = BorrowedPureCseKeyV1::structured_width(operation, context)
                        .ok_or(DominanceCseErrorV1::Overflow)?;
                    scope.work(width)?;
                    let Some(key) = BorrowedPureCseKeyV1::from_operation(operation, context) else {
                        continue;
                    };
                    scope.work(width.checked_add(1).ok_or(DominanceCseErrorV1::Overflow)?)?;
                    let fingerprint = fingerprint(key, context);
                    let previous = table.get(&fingerprint).copied();
                    let mut candidate = previous;
                    let mut earlier = None;
                    while let Some(index) = candidate {
                        let row = available[index];
                        let earlier_width =
                            BorrowedPureCseKeyV1::structured_width(row.key.operation(), context)
                                .ok_or(DominanceCseErrorV1::Overflow)?;
                        scope.work(
                            width
                                .checked_add(earlier_width)
                                .and_then(|n| n.checked_add(1))
                                .ok_or(DominanceCseErrorV1::Overflow)?,
                        )?;
                        if key.exactly_equal(row.key, context) {
                            earlier = Some(row.key.operation());
                            break;
                        }
                        candidate = row.previous;
                    }
                    if let Some(earlier) = earlier {
                        let count = earlier.deref(context).get_num_results();
                        scope.work(count.checked_add(1).ok_or(DominanceCseErrorV1::Overflow)?)?;
                        let uses = operation
                            .deref(context)
                            .results()
                            .try_fold(0usize, |sum, value| {
                                sum.checked_add(value.num_uses(context))
                            })
                            .ok_or(DominanceCseErrorV1::Overflow)?;
                        scope.work(uses)?;
                        reserve_vec(&mut replacements, count, &mut scope)?;
                        replacements.extend(earlier.deref(context).results());
                        let replacement_bytes = bytes::<Value, B::Error>(replacements.capacity())?;
                        rewriter.replace_operation_with_values(
                            context,
                            operation,
                            std::mem::take(&mut replacements),
                        );
                        scope.release(replacement_bytes);
                    } else {
                        scope.work(1)?;
                        reserve_vec(&mut available, 1, &mut scope)?;
                        if previous.is_none() {
                            reserve_table(&mut table, &mut scope)?;
                        }
                        let index = available.len();
                        available.push(Available {
                            key,
                            fingerprint,
                            previous,
                        });
                        table.insert(fingerprint, index);
                    }
                }
                reserve_vec(&mut visits, 1, &mut scope)?;
                visits.push(Visit::Exit(start));
                let child_start = visits.len();
                for child in tree.children(&block) {
                    scope.work(1)?;
                    reserve_vec(&mut visits, 1, &mut scope)?;
                    visits.push(Visit::Enter(child));
                }
                scope.work(visits.len() - child_start)?;
                visits[child_start..].reverse();
            }
        }
    }
    Ok(())
}

fn fingerprint(key: BorrowedPureCseKeyV1, context: &Context) -> u64 {
    let mut result = DefaultHasher::new();
    key.hash_non_attribute_fields(context, &mut result);
    let operation = key.operation().deref(context);
    let mut sum = 0u64;
    let mut xor = 0u64;
    for (name, attribute) in operation.attributes.0.iter() {
        let mut pair = DefaultHasher::new();
        name.hash(&mut pair);
        attribute.hash(&mut pair);
        let hash = pair.finish();
        sum = sum.wrapping_add(hash);
        xor ^= hash.rotate_left(17);
    }
    operation.attributes.0.len().hash(&mut result);
    sum.hash(&mut result);
    xor.hash(&mut result);
    result.finish()
}

#[cfg(test)]
mod tests {
    include!("dominance_cse_v1_tests.rs");
}
