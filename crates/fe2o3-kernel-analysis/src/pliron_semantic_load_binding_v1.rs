//! Live structural load bindings, deliberately not semantic equality evidence.
//!
//! This initial subset admits one block, readonly global views, and unordered
//! nonvolatile reads. It proves neither bounds nor memory stability nor a KIR
//! correspondence. The canonical identity is an invalidation subject supplied
//! by the retained owner; the owner must separately prove that correspondence.
//! Refinement analysis rejects these reads until a memory-equivalence consumer
//! exists. In particular, bindings cannot be converted to expression symbols.

use std::{
    collections::HashSet,
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

use dialect_kernel::{
    AccessKindAttr, DimensionOp, IndexBinaryOp, IndexConstantOp, IndexUnknownOp,
    IndexUnsignedCastOp, InvocationIndexOp, MemorySpaceAttr, RankedViewOp, ReturnOp,
    SEMANTIC_TYPED_READ_SYMBOL_BASE_V1, SemanticReadOrderingAttr, SemanticReadVolatilityAttr,
    SemanticTypedBinaryOp, SemanticTypedCastOp, SemanticTypedCompareOp, SemanticTypedConstantOp,
    SemanticTypedExpressionRootOp, SemanticTypedReadOp, SemanticTypedScalarV1,
    SemanticTypedSelectOp, SemanticTypedSymbolOp, SemanticTypedUnaryOp, ranked_view_type,
};
use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrIdentityV13, VerifiedCanonicalKernelIrV13};
use fe2o3_pliron_owner_core::{ContextIdentity, require_context_identity};
use pliron::{
    basic_block::BasicBlock,
    builtin::ops::FuncOp,
    context::{Context, IrMutationAttemptEpoch, Ptr},
    op::Op,
    operation::Operation,
    value::Value,
};

use crate::{
    PlironIrStructuralIdentityV1, derive_pliron_ir_structural_identity_v1,
    pliron_function_inventory::BoundedPlironFunctionInventoryV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlironSemanticLoadBindingErrorV1 {
    MissingContextIdentity,
    ContextChanged,
    FunctionChanged,
    CanonicalSubjectChanged,
    MutationEpochChanged,
    InvalidGraph,
    UnsupportedControlFlow,
    UnsupportedOperation,
    NonDominatingOperand,
    UnknownOrWritableAllocation,
    UnsupportedAddressSpace,
    UnsupportedVolatilityOrOrdering,
    UnboundLoadSymbol,
    DuplicateLoadIdentity,
    UnconsumedRead,
    EmptyReadRoster,
    ResourceLimit,
}

impl fmt::Display for PlironSemanticLoadBindingErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "live semantic load binding rejected: {self:?}")
    }
}
impl Error for PlironSemanticLoadBindingErrorV1 {}

/// Exact observations of one actual SSA producer. Private construction, no
/// proof status, no free-symbol conversion, and no equality relation on reads.
pub struct PlironSemanticReadBindingV1 {
    operation: usize,
    producer: Ptr<Operation>,
    result: Value,
    symbol: u32,
    view: Value,
    indices: Vec<Value>,
    allocation_origin: u64,
    scalar: SemanticTypedScalarV1,
    guarded: Option<(Value, Value)>,
}

impl PlironSemanticReadBindingV1 {
    pub const fn block(&self) -> usize {
        0
    }
    pub const fn operation(&self) -> usize {
        self.operation
    }
    pub const fn producer(&self) -> Ptr<Operation> {
        self.producer
    }
    pub const fn result(&self) -> Value {
        self.result
    }
    pub const fn symbol(&self) -> u32 {
        self.symbol
    }
    pub const fn view(&self) -> Value {
        self.view
    }
    pub fn indices(&self) -> &[Value] {
        &self.indices
    }
    pub const fn allocation_origin(&self) -> u64 {
        self.allocation_origin
    }
    pub const fn scalar(&self) -> SemanticTypedScalarV1 {
        self.scalar
    }
    pub const fn guarded(&self) -> Option<(Value, Value)> {
        self.guarded
    }
    pub const fn kind(&self) -> AccessKindAttr {
        AccessKindAttr::Read
    }
    pub const fn memory_space(&self) -> MemorySpaceAttr {
        MemorySpaceAttr::Global
    }
    pub const fn volatility(&self) -> SemanticReadVolatilityAttr {
        SemanticReadVolatilityAttr::NonVolatile
    }
    pub const fn ordering(&self) -> SemanticReadOrderingAttr {
        SemanticReadOrderingAttr::Unordered
    }
}

/// A retained observation roster. It is not a safety or refinement witness.
pub struct LivePlironSemanticLoadBindingsV1 {
    context: ContextIdentity,
    function: Ptr<Operation>,
    canonical: VerifiedCanonicalKernelIrIdentityV13,
    canonical_epoch: u64,
    mutation_epoch: IrMutationAttemptEpoch,
    structural: PlironIrStructuralIdentityV1,
    blocks: Vec<Ptr<BasicBlock>>,
    operations: Vec<Ptr<Operation>>,
    reads: Vec<PlironSemanticReadBindingV1>,
}

impl LivePlironSemanticLoadBindingsV1 {
    /// Revalidate before and after a caller inspects the observations. Keeping
    /// a copied label or Value does not keep a validated binding alive.
    pub fn with_live_reads<R>(
        &self,
        context: &Context,
        function: &FuncOp,
        canonical: &VerifiedCanonicalKernelIrV13,
        canonical_epoch: u64,
        inspect: impl FnOnce(&[PlironSemanticReadBindingV1]) -> R,
    ) -> Result<R, PlironSemanticLoadBindingErrorV1> {
        self.revalidate(context, function, canonical, canonical_epoch)?;
        let result = inspect(&self.reads);
        self.revalidate(context, function, canonical, canonical_epoch)?;
        Ok(result)
    }

    pub fn revalidate(
        &self,
        context: &Context,
        function: &FuncOp,
        canonical: &VerifiedCanonicalKernelIrV13,
        canonical_epoch: u64,
    ) -> Result<(), PlironSemanticLoadBindingErrorV1> {
        use PlironSemanticLoadBindingErrorV1 as E;
        if require_context_identity(context).map_err(|_| E::MissingContextIdentity)? != self.context
        {
            return Err(E::ContextChanged);
        }
        if function.get_operation() != self.function {
            return Err(E::FunctionChanged);
        }
        if canonical.identity() != &self.canonical || canonical_epoch != self.canonical_epoch {
            return Err(E::CanonicalSubjectChanged);
        }
        if mutation_epoch(context)? != self.mutation_epoch {
            return Err(E::MutationEpochChanged);
        }
        guarded(|| {
            let current = derive_pliron_ir_structural_identity_v1(context, function)
                .map_err(|_| E::InvalidGraph)?;
            let inventory = BoundedPlironFunctionInventoryV1::collect(context, function)
                .map_err(|_| E::ResourceLimit)?;
            if !current.exactly_matches(&self.structural)
                || inventory.blocks() != self.blocks.as_slice()
                || !inventory
                    .operations()
                    .iter()
                    .map(|site| site.pointer())
                    .eq(self.operations.iter().copied())
            {
                return Err(E::InvalidGraph);
            }
            if mutation_epoch(context)? != self.mutation_epoch {
                return Err(E::MutationEpochChanged);
            }
            Ok(())
        })
    }
}

/// Collect all reads from the actual graph, with no caller-supplied roster to
/// omit a producer or substitute an inert recipe record. The caller supplies
/// an owner-associated canonical seal, not evidence of memory equivalence.
pub fn validate_live_pliron_semantic_load_bindings_v1(
    context: &Context,
    function: &FuncOp,
    canonical: &VerifiedCanonicalKernelIrV13,
    canonical_epoch: u64,
) -> Result<LivePlironSemanticLoadBindingsV1, PlironSemanticLoadBindingErrorV1> {
    use PlironSemanticLoadBindingErrorV1 as E;
    let identity = require_context_identity(context).map_err(|_| E::MissingContextIdentity)?;
    let epoch = mutation_epoch(context)?;
    guarded(|| {
        let structural = derive_pliron_ir_structural_identity_v1(context, function)
            .map_err(|_| E::InvalidGraph)?;
        let inventory = BoundedPlironFunctionInventoryV1::collect(context, function)
            .map_err(|_| E::ResourceLimit)?;
        if inventory.blocks().len() != 1 {
            return Err(E::UnsupportedControlFlow);
        }
        let mut available = inventory.blocks()[0]
            .deref(context)
            .arguments()
            .collect::<HashSet<_>>();
        let mut consumed = HashSet::new();
        let mut symbols = HashSet::new();
        let mut reads = Vec::new();
        for site in inventory.operations() {
            let pointer = site.pointer();
            let raw = pointer.deref(context);
            let operation = Operation::get_op_dyn(pointer, context);
            if raw.get_num_successors() != 0 || !supported_operation(operation.as_ref()) {
                return Err(E::UnsupportedOperation);
            }
            for operand in raw.operands() {
                if !available.contains(&operand) {
                    return Err(E::NonDominatingOperand);
                }
                consumed.insert(operand);
            }
            if let Some(symbol) = operation.downcast_ref::<SemanticTypedSymbolOp>() {
                if symbol
                    .symbol(context)
                    .is_none_or(|id| id >= SEMANTIC_TYPED_READ_SYMBOL_BASE_V1)
                {
                    return Err(E::UnboundLoadSymbol);
                }
            }
            if let Some(read) = operation.downcast_ref::<SemanticTypedReadOp>() {
                if read.volatility(context) != Some(SemanticReadVolatilityAttr::NonVolatile)
                    || read.ordering(context) != Some(SemanticReadOrderingAttr::Unordered)
                {
                    return Err(E::UnsupportedVolatilityOrOrdering);
                }
                if read.memory_space(context) != Some(MemorySpaceAttr::Global) {
                    return Err(E::UnsupportedAddressSpace);
                }
                let view = read.view(context);
                let view_type =
                    ranked_view_type(view, context).ok_or(E::UnknownOrWritableAllocation)?;
                let definition = Operation::get_op_dyn(
                    view.defining_op().ok_or(E::UnknownOrWritableAllocation)?,
                    context,
                );
                let view_op = definition
                    .downcast_ref::<RankedViewOp>()
                    .ok_or(E::UnknownOrWritableAllocation)?;
                let allocation = view_op
                    .allocation_origin(context)
                    .filter(|id| *id != 0)
                    .ok_or(E::UnknownOrWritableAllocation)?;
                if view_type.deref(context).writable()
                    || view_op.result(context) != view
                    || view_op.memory_space(context) != Some(MemorySpaceAttr::Global)
                {
                    return Err(E::UnknownOrWritableAllocation);
                }
                let symbol = read.symbol(context).ok_or(E::InvalidGraph)?;
                if !symbols.insert(symbol) {
                    return Err(E::DuplicateLoadIdentity);
                }
                reads.push(PlironSemanticReadBindingV1 {
                    operation: site.operation(),
                    producer: pointer,
                    result: read.result(context),
                    symbol,
                    view,
                    indices: read.indices(context).ok_or(E::InvalidGraph)?,
                    allocation_origin: allocation,
                    scalar: read.scalar(context).ok_or(E::InvalidGraph)?,
                    guarded: read.guarded(context),
                });
            }
            available.extend(raw.results());
        }
        if reads.is_empty() {
            return Err(E::EmptyReadRoster);
        }
        if reads.iter().any(|read| !consumed.contains(&read.result)) {
            return Err(E::UnconsumedRead);
        }
        if mutation_epoch(context)? != epoch {
            return Err(E::MutationEpochChanged);
        }
        Ok(LivePlironSemanticLoadBindingsV1 {
            context: identity,
            function: function.get_operation(),
            canonical: *canonical.identity(),
            canonical_epoch,
            mutation_epoch: epoch,
            structural,
            blocks: inventory.blocks().to_vec(),
            operations: inventory
                .operations()
                .iter()
                .map(|site| site.pointer())
                .collect(),
            reads,
        })
    })
}

// No calls, writes, allocation effects, atomics, or opaque effect carriers.
// Supporting any such operation requires a separate memory-stability analysis.
fn supported_operation(operation: &dyn Op) -> bool {
    operation.downcast_ref::<RankedViewOp>().is_some()
        || operation.downcast_ref::<IndexConstantOp>().is_some()
        || operation.downcast_ref::<IndexUnknownOp>().is_some()
        || operation.downcast_ref::<IndexBinaryOp>().is_some()
        || operation.downcast_ref::<IndexUnsignedCastOp>().is_some()
        || operation.downcast_ref::<InvocationIndexOp>().is_some()
        || operation.downcast_ref::<DimensionOp>().is_some()
        || operation.downcast_ref::<SemanticTypedReadOp>().is_some()
        || operation
            .downcast_ref::<SemanticTypedConstantOp>()
            .is_some()
        || operation.downcast_ref::<SemanticTypedSymbolOp>().is_some()
        || operation.downcast_ref::<SemanticTypedUnaryOp>().is_some()
        || operation.downcast_ref::<SemanticTypedBinaryOp>().is_some()
        || operation.downcast_ref::<SemanticTypedCompareOp>().is_some()
        || operation.downcast_ref::<SemanticTypedCastOp>().is_some()
        || operation.downcast_ref::<SemanticTypedSelectOp>().is_some()
        || operation
            .downcast_ref::<SemanticTypedExpressionRootOp>()
            .is_some()
        || operation.downcast_ref::<ReturnOp>().is_some()
}

fn mutation_epoch(
    context: &Context,
) -> Result<IrMutationAttemptEpoch, PlironSemanticLoadBindingErrorV1> {
    context
        .ir_mutation_attempt_epoch()
        .map_err(|_| PlironSemanticLoadBindingErrorV1::MutationEpochChanged)
}

fn guarded<T>(
    run: impl FnOnce() -> Result<T, PlironSemanticLoadBindingErrorV1>,
) -> Result<T, PlironSemanticLoadBindingErrorV1> {
    catch_unwind(AssertUnwindSafe(run))
        .map_err(|_| PlironSemanticLoadBindingErrorV1::InvalidGraph)?
}

#[cfg(test)]
mod tests;
