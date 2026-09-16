//! Exact fixed-width integer neutral identities on verified SSA IR.
//!
//! This raw transform is nontransactional and does not select an execution
//! policy. The caller must discard its candidate after error or observer failure.
//! Accounting covers structural work and visible temporary Vec capacity. The
//! same caller must separately prepay upstream verification/rewriter/IR costs,
//! including at most one persistent false constant per rewritten checked binary.

use std::{error::Error, fmt, mem::size_of};

use pliron::{
    builtin::{
        attributes::IntegerAttr,
        types::{IntegerType, Signedness},
    },
    common_traits::Verify,
    context::{Context, Ptr},
    irbuild::{
        IRStatus,
        inserter::{Inserter, OpInsertionPoint},
        listener::DummyListener,
        observer::RewriteObserver,
        rewriter::{IRRewriter, Rewriter},
    },
    linked_list::{ContainsLinkedList, LinkedList},
    op::Op,
    operation::Operation,
    r#type::{TypeHandle, Typed},
    utils::apint::{APInt, bw},
    value::Value,
};

use crate::optimization_v1::{BinaryKindAttr, BinaryOp, ConstantOp};

/// The existing caller-owned rewrite ledger, not a second accounting authority.
pub use crate::dominance_cse_v1::DominanceCseBudgetV1 as IntegerIdentityBudgetV1;

/// Local failure, preserving an original caller denial without replacement.
#[derive(Debug)]
pub enum IntegerIdentityErrorV1<E> {
    /// The original ledger denial and its failure history.
    Budget(E),
    /// A size or work calculation cannot be represented.
    Overflow,
    /// A fallible temporary collection allocation failed.
    Allocation,
}

impl<E: fmt::Display> fmt::Display for IntegerIdentityErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Budget(error) => write!(formatter, "integer identity budget: {error}"),
            Self::Overflow => formatter.write_str("integer identity resource overflow"),
            Self::Allocation => formatter.write_str("integer identity allocation failed"),
        }
    }
}

impl<E: Error + 'static> Error for IntegerIdentityErrorV1<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Budget(error) => Some(error),
            Self::Overflow | Self::Allocation => None,
        }
    }
}

/// Rewrite the closed neutral-operand family once, without moving operations.
pub fn integer_identity_canonicalization_v1<B: IntegerIdentityBudgetV1>(
    root: Ptr<Operation>,
    context: &mut Context,
    budget: &mut B,
) -> Result<IRStatus, IntegerIdentityErrorV1<B::Error>> {
    canonicalize(root, context, budget, None)
}

/// The identical transform using the caller's existing occurrence observer.
pub fn integer_identity_canonicalization_with_observer_v1<B: IntegerIdentityBudgetV1>(
    root: Ptr<Operation>,
    context: &mut Context,
    budget: &mut B,
    observer: Box<dyn RewriteObserver>,
) -> Result<IRStatus, IntegerIdentityErrorV1<B::Error>> {
    canonicalize(root, context, budget, Some(observer))
}

struct Scratch<'a, B: IntegerIdentityBudgetV1> {
    budget: &'a mut B,
    live: usize,
}

impl<B: IntegerIdentityBudgetV1> Scratch<'_, B> {
    fn work(&mut self, work: usize) -> Result<(), IntegerIdentityErrorV1<B::Error>> {
        self.budget
            .charge_work(work)
            .map_err(IntegerIdentityErrorV1::Budget)
    }

    fn reserve(&mut self, bytes: usize) -> Result<(), IntegerIdentityErrorV1<B::Error>> {
        let live = self
            .live
            .checked_add(bytes)
            .ok_or(IntegerIdentityErrorV1::Overflow)?;
        self.budget
            .reserve_storage(bytes)
            .map_err(IntegerIdentityErrorV1::Budget)?;
        self.live = live;
        Ok(())
    }

    fn release(&mut self, bytes: usize) {
        self.live -= bytes;
        self.budget.release_storage(bytes);
    }
}

impl<B: IntegerIdentityBudgetV1> Drop for Scratch<'_, B> {
    fn drop(&mut self) {
        self.budget.release_storage(self.live);
    }
}

fn bytes<T, E>(capacity: usize) -> Result<usize, IntegerIdentityErrorV1<E>> {
    capacity
        .checked_mul(size_of::<T>())
        .ok_or(IntegerIdentityErrorV1::Overflow)
}

fn reserve_vec<T, B: IntegerIdentityBudgetV1>(
    values: &mut Vec<T>,
    additional: usize,
    scratch: &mut Scratch<'_, B>,
) -> Result<(), IntegerIdentityErrorV1<B::Error>> {
    let required = values
        .len()
        .checked_add(additional)
        .ok_or(IntegerIdentityErrorV1::Overflow)?;
    let before = values.capacity();
    if required <= before {
        return Ok(());
    }
    let requested = before
        .checked_mul(2)
        .ok_or(IntegerIdentityErrorV1::Overflow)?
        .max(4)
        .max(required);
    scratch.work(
        before
            .checked_add(2)
            .ok_or(IntegerIdentityErrorV1::Overflow)?,
    )?;
    scratch.reserve(bytes::<T, B::Error>(requested - before)?)?;
    values
        .try_reserve_exact(requested - values.len())
        .map_err(|_| IntegerIdentityErrorV1::Allocation)?;
    if values.capacity() > requested {
        scratch.reserve(bytes::<T, B::Error>(values.capacity() - requested)?)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Identity {
    value: Value,
    checked: bool,
}

fn literal<B: IntegerIdentityBudgetV1>(
    value: Value,
    ty: TypeHandle,
    context: &Context,
    scratch: &mut Scratch<'_, B>,
) -> Result<Option<u64>, IntegerIdentityErrorV1<B::Error>> {
    scratch.work(1)?;
    let Some(definition) = value.defining_op() else {
        return Ok(None);
    };
    let Some(constant) = Operation::get_op::<ConstantOp>(definition, context) else {
        return Ok(None);
    };
    scratch.work(1)?;
    if constant.verify(context).is_err() || definition.deref(context).attributes.0.len() != 1 {
        return Ok(None);
    }
    let Some(attribute) = constant.get_attr_gpu_constant_value(context) else {
        return Ok(None);
    };
    let Some(integer) = attribute.downcast_ref::<IntegerAttr>() else {
        return Ok(None);
    };
    if TypeHandle::from(integer.get_type()) != ty || integer.verify(context).is_err() {
        return Ok(None);
    }
    // The caller checked the type width, and the attribute verifier checked the
    // payload width before this upstream API clones the APInt.
    scratch.work(1)?;
    Ok(Some(integer.value().to_u64()))
}

fn match_identity<B: IntegerIdentityBudgetV1>(
    operation: Ptr<Operation>,
    context: &Context,
    scratch: &mut Scratch<'_, B>,
) -> Result<Option<Identity>, IntegerIdentityErrorV1<B::Error>> {
    let Some(binary) = Operation::get_op::<BinaryOp>(operation, context) else {
        return Ok(None);
    };
    scratch.work(1)?;
    let Some(kind) = binary.kind(context) else {
        return Ok(None);
    };
    let checked = matches!(
        kind,
        BinaryKindAttr::CheckedAdd
            | BinaryKindAttr::CheckedSubtract
            | BinaryKindAttr::CheckedMultiply
    );
    if !matches!(
        kind,
        BinaryKindAttr::Add
            | BinaryKindAttr::Subtract
            | BinaryKindAttr::Multiply
            | BinaryKindAttr::BitAnd
            | BinaryKindAttr::BitOr
            | BinaryKindAttr::BitXor
            | BinaryKindAttr::CheckedAdd
            | BinaryKindAttr::CheckedSubtract
            | BinaryKindAttr::CheckedMultiply
    ) {
        return Ok(None);
    }
    scratch.work(1)?;
    if operation.deref(context).attributes.0.len() != 1 || binary.verify(context).is_err() {
        return Ok(None);
    }
    let lhs = operation.deref(context).get_operand(0);
    let rhs = operation.deref(context).get_operand(1);
    let ty = lhs.get_type(context);
    let width = {
        let raw = ty.deref(context);
        let Some(integer) = raw.downcast_ref::<IntegerType>() else {
            return Ok(None);
        };
        if !matches!(
            integer.signedness(),
            Signedness::Signed | Signedness::Unsigned
        ) || !matches!(integer.width(), 8 | 16 | 32 | 64)
        {
            return Ok(None);
        }
        integer.width()
    };
    let left = literal(lhs, ty, context, scratch)?;
    let right = literal(rhs, ty, context, scratch)?;
    scratch.work(1)?;
    let neutral = match kind {
        BinaryKindAttr::Multiply | BinaryKindAttr::CheckedMultiply => 1,
        BinaryKindAttr::BitAnd => u64::MAX >> (64 - width),
        _ => 0,
    };
    let value = if right == Some(neutral) {
        Some(lhs)
    } else if !matches!(
        kind,
        BinaryKindAttr::Subtract | BinaryKindAttr::CheckedSubtract
    ) && left == Some(neutral)
    {
        Some(rhs)
    } else {
        None
    };
    Ok(value.map(|value| Identity { value, checked }))
}

fn canonicalize<B: IntegerIdentityBudgetV1>(
    root: Ptr<Operation>,
    context: &mut Context,
    budget: &mut B,
    observer: Option<Box<dyn RewriteObserver>>,
) -> Result<IRStatus, IntegerIdentityErrorV1<B::Error>> {
    // Collections are declared after the guard and drop before its release.
    let mut scratch = Scratch { budget, live: 0 };
    scratch.work(1)?;
    scratch.reserve(size_of::<Vec<Ptr<Operation>>>() + size_of::<Vec<Value>>())?;
    let mut pending = Vec::new();
    let mut replacements = Vec::new();
    reserve_vec(&mut pending, 1, &mut scratch)?;
    pending.push(root);
    let mut rewriter = IRRewriter::<DummyListener>::default();
    rewriter.set_observer(observer);
    rewriter.get_config_mut().set_name_on_value_replacement = false;
    while let Some(container) = pending.pop() {
        scratch.work(1)?;
        let start = pending.len();
        let regions = container.deref(context).num_regions();
        for ordinal in 0..regions {
            scratch.work(1)?;
            let region = container.deref(context).get_region(ordinal);
            let ssa = region.deref(context).has_ssa_dominance(context);
            let mut current_block = region.deref(context).get_head();
            while let Some(block) = current_block {
                current_block = block.deref(context).get_next();
                scratch.work(1)?;
                let mut current = block.deref(context).get_head();
                while let Some(operation) = current {
                    scratch.work(1)?;
                    current = operation.deref(context).get_next();
                    if operation.deref(context).num_regions() != 0 {
                        reserve_vec(&mut pending, 1, &mut scratch)?;
                        pending.push(operation);
                    }
                    if !ssa {
                        continue;
                    }
                    let Some(identity) = match_identity(operation, context, &mut scratch)? else {
                        continue;
                    };
                    let count = if identity.checked { 2 } else { 1 };
                    scratch.work(count + 1)?;
                    let uses = operation
                        .deref(context)
                        .results()
                        .try_fold(0usize, |sum, value| {
                            sum.checked_add(value.num_uses(context))
                        })
                        .ok_or(IntegerIdentityErrorV1::Overflow)?;
                    scratch.work(uses)?;
                    reserve_vec(&mut replacements, count, &mut scratch)?;
                    let replacement_bytes = bytes::<Value, B::Error>(replacements.capacity())?;
                    if identity.checked {
                        scratch.work(1)?;
                    }
                    replacements.push(identity.value);
                    if identity.checked {
                        let boolean = IntegerType::get(context, 1, Signedness::Signless);
                        let constant = ConstantOp::new(
                            context,
                            Box::new(IntegerAttr::new(boolean, APInt::zero(bw(1)))),
                        );
                        rewriter.set_insertion_point(OpInsertionPoint::BeforeOperation(operation));
                        rewriter.insert_op(context, &constant);
                        replacements.push(constant.get_operation().deref(context).get_result(0));
                    }
                    rewriter.replace_operation_with_values(
                        context,
                        operation,
                        std::mem::take(&mut replacements),
                    );
                    scratch.release(replacement_bytes);
                }
            }
        }
        scratch.work(pending.len() - start)?;
        pending[start..].reverse();
    }
    Ok(rewriter.is_modified().into())
}

#[cfg(test)]
mod tests {
    include!("integer_identity_v1_tests.rs");
}
