//! Def-use closure of the unchanged exact integer-neutral rule family.
//!
//! This is a raw nontransactional transform, not an admission or pass selector.
//! The fixed production owner must verify the input, prepay upstream IR and
//! observer costs, and discard its candidate after any refusal. Historical V1
//! remains a single traversal. V2 revisits affected original users only.
use super::*;
use pliron::value::Use;

/// The existing caller ledger, with no independent work or storage allowance.
pub use super::IntegerIdentityBudgetV1 as IntegerIdentityBudgetV2;
/// The same typed arithmetic/allocation/caller-denial error boundary as V1.
pub use super::IntegerIdentityErrorV1 as IntegerIdentityErrorV2;

#[derive(Clone, Copy)]
struct Row {
    operation: Ptr<Operation>,
    alive: bool,
    queued: bool,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct Key {
    operation: Ptr<Operation>,
    ordinal: usize,
}

// Transient exact pointer order is only an index. Queue and observer order use
// source ordinals; neither arena ordering nor context identity is serialized.

fn grow<T: Copy, B: IntegerIdentityBudgetV1>(
    values: &mut Vec<T>,
    additional: usize,
    scratch: &mut Scratch<'_, B>,
) -> Result<(), IntegerIdentityErrorV1<B::Error>> {
    let required = values
        .len()
        .checked_add(additional)
        .ok_or(IntegerIdentityErrorV1::Overflow)?;
    if required <= values.capacity() {
        return Ok(());
    }
    let capacity = values
        .capacity()
        .checked_mul(2)
        .ok_or(IntegerIdentityErrorV1::Overflow)?
        .max(4)
        .max(required);
    scratch.work(
        values
            .len()
            .checked_add(3)
            .ok_or(IntegerIdentityErrorV1::Overflow)?,
    )?;
    scratch.reserve(bytes::<T, B::Error>(capacity)?)?;
    let mut replacement = Vec::new();
    replacement
        .try_reserve_exact(capacity)
        .map_err(|_| IntegerIdentityErrorV1::Allocation)?;
    scratch.reserve(bytes::<T, B::Error>(
        replacement
            .capacity()
            .checked_sub(capacity)
            .ok_or(IntegerIdentityErrorV1::Overflow)?,
    )?)?;
    replacement.extend_from_slice(values);
    let old = std::mem::replace(values, replacement);
    let retired = bytes::<T, B::Error>(old.capacity())?;
    drop(old);
    scratch.release(retired);
    Ok(())
}

fn sift<T: Ord>(values: &mut [T], mut root: usize, end: usize) {
    while root < end / 2 {
        let mut child = root * 2 + 1;
        if child + 1 < end {
            if values[child] < values[child + 1] {
                child += 1;
            }
        }
        if values[root] >= values[child] {
            break;
        }
        values.swap(root, child);
        root = child;
    }
}

fn sort<T: Ord, B: IntegerIdentityBudgetV1>(
    values: &mut [T],
    scratch: &mut Scratch<'_, B>,
) -> Result<(), IntegerIdentityErrorV1<B::Error>> {
    // At most two sifts per element, each with at most bit_length(n) levels.
    // Pay five visits per level plus one per element and a fixed header. This
    // O(n log n) ceiling is independent of context identity and input order.
    let levels = (usize::BITS - values.len().leading_zeros()) as usize;
    let work = levels
        .checked_mul(10)
        .and_then(|n| n.checked_add(1))
        .and_then(|n| n.checked_mul(values.len()))
        .and_then(|n| n.checked_add(1))
        .ok_or(IntegerIdentityErrorV1::Overflow)?;
    scratch.work(work)?;
    for root in (0..values.len() / 2).rev() {
        sift(values, root, values.len());
    }
    for end in (1..values.len()).rev() {
        values.swap(0, end);
        sift(values, 0, end);
    }
    Ok(())
}

fn find<B: IntegerIdentityBudgetV1>(
    operation: Ptr<Operation>,
    rows: &[Row],
    keys: &[Key],
    scratch: &mut Scratch<'_, B>,
) -> Result<Option<usize>, IntegerIdentityErrorV1<B::Error>> {
    // Exact ordered keys have no hash collision scan. Pay the complete binary
    // search bound before lookup, not its context-dependent successful depth.
    let levels = (usize::BITS - keys.len().leading_zeros()) as usize;
    scratch.work(2 * levels + 3)?;
    let (mut first, mut end) = (0, keys.len());
    while first < end {
        let middle = first + (end - first) / 2;
        if keys[middle].operation < operation {
            first = middle + 1;
        } else {
            end = middle;
        }
    }
    if let Some(key) = keys.get(first) {
        if rows[key.ordinal].operation == operation {
            return Ok(Some(key.ordinal));
        }
    }
    Ok(None)
}

/// Saturate V1's closed neutral-operand rules without moving operations.
/// Original candidates are removed at most once; newly inserted Boolean false
/// constants are not candidates. The caller owns upstream verification and IR
/// allocation accounting, as for V1.
pub fn integer_identity_canonicalization_v2<B: IntegerIdentityBudgetV2>(
    root: Ptr<Operation>,
    context: &mut Context,
    budget: &mut B,
) -> Result<IRStatus, IntegerIdentityErrorV2<B::Error>> {
    canonicalize(root, context, budget, None)
}

/// Identical closure with the caller's genuine occurrence observer retained.
pub fn integer_identity_canonicalization_with_observer_v2<B: IntegerIdentityBudgetV2>(
    root: Ptr<Operation>,
    context: &mut Context,
    budget: &mut B,
    observer: Box<dyn RewriteObserver>,
) -> Result<IRStatus, IntegerIdentityErrorV2<B::Error>> {
    canonicalize(root, context, budget, Some(observer))
}

fn canonicalize<B: IntegerIdentityBudgetV1>(
    root: Ptr<Operation>,
    context: &mut Context,
    budget: &mut B,
    observer: Option<Box<dyn RewriteObserver>>,
) -> Result<IRStatus, IntegerIdentityErrorV1<B::Error>> {
    let mut scratch = Scratch { budget, live: 0 };
    scratch.work(1)?;
    scratch.reserve(
        size_of::<Vec<Row>>()
            + size_of::<Vec<Key>>()
            + size_of::<Vec<Ptr<Operation>>>()
            + 3 * size_of::<Vec<usize>>() // Ring, affected users, replacement in grow.
            + size_of::<Vec<Value>>()
            + size_of::<Vec<Use<Value>>>()
            + size_of::<Row>()
            + size_of::<Key>()
            + size_of::<Ptr<Operation>>()
            + size_of::<IRRewriter<DummyListener>>()
            + size_of::<Scratch<'_, B>>()
            + 10 * size_of::<usize>(),
    )?;
    // All owned collections drop before their original ledger credit.
    let mut rows = Vec::new();
    let mut keys = Vec::new();
    let mut pending = Vec::new();
    let mut queue = Vec::new();
    let mut affected = Vec::new();
    let mut replacements = Vec::new();
    grow(&mut pending, 1, &mut scratch)?;
    pending.push(root);
    while let Some(container) = pending.pop() {
        scratch.work(1)?;
        let start = pending.len();
        for region in 0..container.deref(context).num_regions() {
            scratch.work(1)?;
            let region = container.deref(context).get_region(region);
            let ssa = region.deref(context).has_ssa_dominance(context);
            let mut block = region.deref(context).get_head();
            while let Some(current_block) = block {
                scratch.work(1)?;
                block = current_block.deref(context).get_next();
                let mut operation = current_block.deref(context).get_head();
                while let Some(current) = operation {
                    scratch.work(2)?;
                    operation = current.deref(context).get_next();
                    if current.deref(context).num_regions() != 0 {
                        grow(&mut pending, 1, &mut scratch)?;
                        pending.push(current);
                    }
                    if ssa && Operation::get_op::<BinaryOp>(current, context).is_some() {
                        grow(&mut rows, 1, &mut scratch)?;
                        rows.push(Row {
                            operation: current,
                            alive: true,
                            queued: true,
                        });
                    }
                }
            }
        }
        scratch.work(pending.len() - start)?;
        pending[start..].reverse();
    }
    grow(&mut keys, rows.len(), &mut scratch)?;
    grow(&mut queue, rows.len(), &mut scratch)?;
    for (ordinal, row) in rows.iter().enumerate() {
        scratch.work(2)?;
        keys.push(Key {
            operation: row.operation,
            ordinal,
        });
        queue.push(ordinal);
    }
    sort(&mut keys, &mut scratch)?;
    let mut head = 0;
    let mut queued = queue.len();
    let mut rewriter = IRRewriter::<DummyListener>::default();
    rewriter.set_observer(observer);
    rewriter.get_config_mut().set_name_on_value_replacement = false;
    while queued != 0 {
        scratch.work(3)?;
        let ordinal = queue[head];
        head = (head + 1) % queue.len();
        queued -= 1;
        rows[ordinal].queued = false;
        if !rows[ordinal].alive {
            continue;
        }
        let operation = rows[ordinal].operation;
        let Some(identity) = match_identity(operation, context, &mut scratch)? else {
            continue;
        };
        affected.clear();
        for value in operation.deref(context).results() {
            scratch.work(2)?;
            let count = value.num_uses(context);
            scratch.work(count)?;
            scratch.reserve(bytes::<Use<Value>, B::Error>(count)?)?;
            let uses = value.uses(context);
            scratch.reserve(bytes::<Use<Value>, B::Error>(
                uses.capacity()
                    .checked_sub(count)
                    .ok_or(IntegerIdentityErrorV1::Overflow)?,
            )?)?;
            for actual_use in &uses {
                scratch.work(1)?;
                if Operation::get_op::<BinaryOp>(actual_use.user_op(), context).is_none() {
                    continue;
                }
                if let Some(user) = find(actual_use.user_op(), &rows, &keys, &mut scratch)? {
                    if rows[user].alive && !rows[user].queued {
                        grow(&mut affected, 1, &mut scratch)?;
                        affected.push(user);
                    }
                }
            }
            let retired = bytes::<Use<Value>, B::Error>(uses.capacity())?;
            drop(uses);
            scratch.release(retired);
        }
        sort(&mut affected, &mut scratch)?;
        rewrite_identity(
            operation,
            identity,
            context,
            &mut scratch,
            &mut rewriter,
            &mut replacements,
        )?;
        rows[ordinal].alive = false;
        for user in affected.iter().copied() {
            scratch.work(2)?;
            if rows[user].alive && !rows[user].queued {
                let tail = head
                    .checked_add(queued)
                    .ok_or(IntegerIdentityErrorV1::Overflow)?
                    % queue.len();
                queue[tail] = user;
                queued += 1;
                rows[user].queued = true;
            }
        }
    }
    Ok(rewriter.is_modified().into())
}
