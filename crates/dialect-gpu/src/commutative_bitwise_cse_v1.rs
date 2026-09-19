//! Closed commutative fixed-integer bitwise CSE over existing dominator scopes.
//!
//! Raw and nontransactional: the owning caller must discard a private candidate
//! after every error or observer unwind. No production schedule, wire witness,
//! source/effect proof, publication or runtime authority is selected here.
//! Historical exact CSE and all its key/ledger behavior remain separate.

use crate::{
    cse_v1::BorrowedPureCseKeyV1,
    dominance_cse_v1::{DominanceCseBudgetV1, DominanceCseErrorV1, commutative_bitwise_impl},
    optimization_v1::{BinaryKindAttr, BinaryOp},
};
use pliron::{
    builtin::types::{IntegerType, Signedness},
    context::{Context, Ptr},
    graph::dominance::DomInfo,
    irbuild::{IRStatus, observer::RewriteObserver},
    operation::Operation,
    pass::{AnalysisManager, Pass, PassResult},
    r#type::Typed,
};
use std::{
    collections::hash_map::DefaultHasher,
    error::Error,
    hash::{Hash, Hasher},
};

/// Only verified single-result AND/OR/XOR on I/U8/16/32/64 is eligible.
/// Retain an earlier dominating operation and rewrite/erase its duplicate;
/// never hoist, reorder operands, simplify control flow, or use memory aliases.
/// Upstream dominance/attribute/rewriter costs retain the existing fixed opaque
/// envelope. New key visits and both orientations use the same caller ledger.
pub fn commutative_bitwise_dominance_cse_v1<B: DominanceCseBudgetV1>(
    root: Ptr<Operation>,
    context: &mut Context,
    dominance: &mut DomInfo,
    budget: &mut B,
) -> Result<IRStatus, DominanceCseErrorV1<B::Error>> {
    commutative_bitwise_impl(root, context, dominance, budget, None)
}

/// The identical closed raw transformation with actual replacement/erase events.
/// Observer failure is not an unchanged result; the private candidate is lost.
pub fn commutative_bitwise_dominance_cse_with_observer_v1<B: DominanceCseBudgetV1>(
    root: Ptr<Operation>,
    context: &mut Context,
    dominance: &mut DomInfo,
    budget: &mut B,
    observer: Box<dyn RewriteObserver>,
) -> Result<IRStatus, DominanceCseErrorV1<B::Error>> {
    commutative_bitwise_impl(root, context, dominance, budget, Some(observer))
}

/// Explicit raw adapter; this is not added to an existing production pass list.
pub struct CommutativeBitwiseDominanceCsePassV1<'a, B> {
    budget: &'a mut B,
}
impl<'a, B: DominanceCseBudgetV1> CommutativeBitwiseDominanceCsePassV1<'a, B> {
    /// Borrow the caller's original budget for one complete pass invocation.
    pub fn new(budget: &'a mut B) -> Self {
        Self { budget }
    }
}
impl<B> Pass for CommutativeBitwiseDominanceCsePassV1<'_, B>
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
        let changed =
            commutative_bitwise_dominance_cse_v1(root, context, &mut dominance, self.budget)
                .map_err(|error| pliron::input_error_noloc!(error))?;
        let mut result = PassResult::default();
        result.ir_changed = changed;
        result.set_preserved::<DomInfo>();
        Ok(result)
    }
    fn name(&self) -> &str {
        "gpu-commutative-bitwise-dominance-cse-v1"
    }
}

pub(super) fn key(operation: Ptr<Operation>, context: &Context) -> Option<BorrowedPureCseKeyV1> {
    let key = BorrowedPureCseKeyV1::from_operation(operation, context)?;
    let binary = Operation::get_op::<BinaryOp>(operation, context)?;
    if !matches!(
        binary.kind(context)?,
        BinaryKindAttr::BitAnd | BinaryKindAttr::BitOr | BinaryKindAttr::BitXor
    ) {
        return None;
    }
    let operation = operation.deref(context);
    if operation.get_num_results() != 1 || operation.get_num_operands() != 2 {
        return None;
    }
    let ty = operation.get_operand(0).get_type(context);
    let raw = ty.deref(context);
    let integer = raw.downcast_ref::<IntegerType>()?;
    if !matches!(
        integer.signedness(),
        Signedness::Signed | Signedness::Unsigned
    ) || !matches!(integer.width(), 8 | 16 | 32 | 64)
    {
        return None;
    }
    Some(key)
}

pub(super) fn equal(
    left: BorrowedPureCseKeyV1,
    right: BorrowedPureCseKeyV1,
    context: &Context,
) -> bool {
    let left = left.operation().deref(context);
    let right = right.operation().deref(context);
    left.attributes == right.attributes
        && left.result_types().eq(right.result_types())
        && ((left.get_operand(0) == right.get_operand(0)
            && left.get_operand(1) == right.get_operand(1))
            || (left.get_operand(0) == right.get_operand(1)
                && left.get_operand(1) == right.get_operand(0)))
}

pub(super) fn fingerprint(key: BorrowedPureCseKeyV1, context: &Context) -> u64 {
    let operation = key.operation().deref(context);
    let mut hash = DefaultHasher::new();
    operation.get_num_results().hash(&mut hash);
    for ty in operation.result_types() {
        ty.hash(&mut hash);
    }
    let operand = |index| {
        let mut hash = DefaultHasher::new();
        operation.get_operand(index).hash(&mut hash);
        hash.finish()
    };
    let (a, b) = (operand(0), operand(1));
    a.min(b).hash(&mut hash);
    a.max(b).hash(&mut hash);
    let (mut sum, mut xor) = (0u64, 0u64);
    for (name, attribute) in operation.attributes.0.iter() {
        let mut pair = DefaultHasher::new();
        name.hash(&mut pair);
        attribute.hash(&mut pair);
        let value = pair.finish();
        sum = sum.wrapping_add(value);
        xor ^= value.rotate_left(17);
    }
    operation.attributes.0.len().hash(&mut hash);
    sum.hash(&mut hash);
    xor.hash(&mut hash);
    hash.finish()
}
