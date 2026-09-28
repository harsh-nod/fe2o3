//! Scoped, live-metered queries over one immutable raw function graph.
//! These facts do not establish SSA validity, source correspondence or admission.

use super::{
    ControlFlowError, ControlFlowLimits, MeteredControlFlowErrorV1, MeteredIndexedControlFlowV1,
    analyze_control_flow_with_verification_budget_v1,
};
use crate::{
    BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Function, ValueId,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FunctionControlFlowScopeErrorV1 {
    Resource(Resource),
    ControlFlow(ControlFlowError),
    InvalidBlock(BlockId),
    InvalidEdge { source: BlockId, ordinal: usize },
}
use FunctionControlFlowScopeErrorV1 as Error;

impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "function CFG scope: {self:?}")
    }
}
impl std::error::Error for Error {}

/// The scope owns the CFG and exclusively borrows its ledger. There is no
/// callback budget, alternative-ledger query, owned graph or mutation interface.
pub struct FunctionControlFlowViewV1<'scope, 'function, 'work> {
    function: &'function Function,
    flow: &'scope MeteredIndexedControlFlowV1,
    budget: &'scope mut Budget<'work>,
    failure: &'scope mut Option<Error>,
    origins: &'scope mut Option<Vec<(ValueId, Option<ValueId>)>>,
}

impl<'function> FunctionControlFlowViewV1<'_, 'function, '_> {
    pub fn function(&self) -> &'function Function {
        self.function
    }

    /// Observes the unique non-block-parameter origin through every actual edge
    /// argument. Seeded invariant recurrences are supported; ambiguous or
    /// unseeded recurrences return `None`. Non-parameters return unchanged.
    ///
    /// This is structural equality only: callers must separately establish
    /// definition existence, typing, original source identity and permissions.
    /// The shared metered SCC engine is built once and retained by this scope.
    pub fn unique_value_origin(&mut self, value: ValueId) -> Result<Option<ValueId>, Error> {
        self.charge(1)?;
        if self.origins.is_none() {
            let result = crate::formal_memory_obligations::structural_origins_v1(
                self.function,
                self.flow.indexed_v15(),
                self.budget,
            );
            let rows = result.map_err(|error| self.fail(error.into()))?;
            *self.origins = Some(rows);
        }
        let count = self.origins.as_ref().expect("constructed origins").len();
        self.charge(count.checked_ilog2().unwrap_or(0) as usize + 3)?;
        let rows = self.origins.as_ref().expect("constructed origins");
        Ok(match rows.binary_search_by_key(&value, |row| row.0) {
            Ok(index) => rows[index].1,
            Err(_) => Some(value),
        })
    }

    fn fail(&mut self, error: Error) -> Error {
        self.failure.get_or_insert(error).clone()
    }

    fn charge(&mut self, amount: usize) -> Result<(), Error> {
        if let Some(error) = self.failure.as_ref() {
            return Err(error.clone());
        }
        self.budget
            .charge_work(amount)
            .map_err(|error| self.fail(error.into()))
    }

    fn block(&mut self, block: BlockId) -> Result<usize, Error> {
        let count = self.flow.indexed_v15().block_count();
        self.charge(count.checked_ilog2().unwrap_or(0) as usize + 3)?;
        self.flow
            .indexed_v15()
            .block_position(block)
            .ok_or_else(|| self.fail(Error::InvalidBlock(block)))
    }

    /// A valid disconnected block returns false; an absent block is an error.
    pub fn is_reachable(&mut self, block: BlockId) -> Result<bool, Error> {
        let position = self.block(block)?;
        self.charge(1)?;
        Ok(self.flow.indexed_v15().reachable[position])
    }

    /// Disconnected self-dominance never counts as entry-reachable dominance.
    pub fn dominates(&mut self, definition: BlockId, use_block: BlockId) -> Result<bool, Error> {
        let definition = self.block(definition)?;
        let use_block = self.block(use_block)?;
        self.charge(4)?;
        let flow = self.flow.indexed_v15();
        Ok(flow.reachable[use_block] && flow.dominates_positions(definition, use_block))
    }

    /// Checks that the selected edge is the only entry into a region dominating
    /// the use. Other incoming edges must be internal backedges. This sufficient
    /// relation preserves parallel edge ordinals and never enumerates paths.
    pub fn success_edge_dominates(
        &mut self,
        source: BlockId,
        ordinal: usize,
        use_block: BlockId,
    ) -> Result<bool, Error> {
        let source_position = self.block(source)?;
        let use_position = self.block(use_block)?;
        self.charge(3)?;
        let range = self.flow.indexed_v15().outgoing[source_position].clone();
        let edge = range
            .start
            .checked_add(ordinal)
            .filter(|&edge| edge < range.end)
            .ok_or_else(|| self.fail(Error::InvalidEdge { source, ordinal }))?;
        let target = self.flow.indexed_v15().edges[edge].target;
        self.charge(5)?;
        let flow = self.flow.indexed_v15();
        if !flow.reachable[source_position]
            || !flow.reachable[use_position]
            || target == 0
            || !flow.dominates_positions(target, use_position)
        {
            return Ok(false);
        }
        let count = flow.incoming[target].len();
        for offset in 0..count {
            self.charge(6)?;
            let flow = self.flow.indexed_v15();
            let incoming = flow.incoming[target][offset];
            if incoming == edge {
                continue;
            }
            let predecessor = flow.edges[incoming].source;
            if flow.reachable[predecessor] && !flow.dominates_positions(target, predecessor) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn function_scope_header_v1() -> Result<usize, Resource> {
    [
        size_of::<Option<MeteredIndexedControlFlowV1>>(),
        size_of::<Option<Error>>(),
        size_of::<Option<Vec<(ValueId, Option<ValueId>)>>>(),
        size_of::<FunctionControlFlowViewV1<'_, '_, '_>>(),
        size_of::<Result<(), Error>>(),
        size_of::<Result<Result<(), Error>, Box<dyn std::any::Any + Send>>>(),
        3 * size_of::<usize>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        sum.checked_add(value).ok_or(Resource::Arithmetic)
    })
}

/// Reuses the existing bounded CFG builder on a borrowed raw Function. Its
/// inherited logical row-cell storage units are retained, not relabeled as bytes.
/// The wrapper additionally reserves its explicit stack-envelope bytes.
///
/// The callback returns unit and receives no budget. It can copy inert query
/// results into prepaid caller storage, but cannot lose or transfer CFG custody.
/// Query failures remain sticky if ignored. Backing drops before exact refund
/// on constructor/callback failure and panic; the original panic is resumed.
/// No SSA, source, memory-validity or canonical-admission authority is implied.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{with_function_control_flow_v1, Function, ControlFlowLimits,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn cannot_escape(function: &Function, budget: &mut Budget<'_>) {
///     let mut escaped = None;
///     with_function_control_flow_v1(function, ControlFlowLimits::DEFAULT, budget,
///         |view| { escaped = Some(view); Ok(()) }).unwrap();
///     drop(escaped);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::{with_function_control_flow_v1, Function, ControlFlowLimits,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn cannot_refund(function: &Function, budget: &mut Budget<'_>) {
///     with_function_control_flow_v1(function, ControlFlowLimits::DEFAULT, budget,
///         |_| { budget.release_storage(1).unwrap(); Ok(()) }).unwrap();
/// }
/// ```
pub fn with_function_control_flow_v1<'function, 'work>(
    function: &'function Function,
    limits: ControlFlowLimits,
    budget: &mut Budget<'work>,
    run: impl for<'scope> FnOnce(
        &mut FunctionControlFlowViewV1<'scope, 'function, 'work>,
    ) -> Result<(), Error>,
) -> Result<(), Error> {
    // Entry and settlement work is prepaid; failure cleanup spends no work.
    budget.charge_work(4)?;
    let before = budget.storage();
    budget.reserve_storage(function_scope_header_v1()?)?;
    let mut flow = None;
    let mut failure = None;
    let mut origins = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        flow = Some(
            analyze_control_flow_with_verification_budget_v1(function, limits, budget).map_err(
                |error| match error {
                    MeteredControlFlowErrorV1::Resource(error) => Error::Resource(error),
                    MeteredControlFlowErrorV1::ControlFlow(error) => Error::ControlFlow(error),
                },
            )?,
        );
        let mut view = FunctionControlFlowViewV1 {
            function,
            flow: flow.as_ref().expect("constructed CFG"),
            budget,
            failure: &mut failure,
            origins: &mut origins,
        };
        run(&mut view)
    }));
    let owned = budget
        .storage()
        .checked_sub(before)
        .ok_or(Resource::Accounting);
    drop((flow, origins));
    let cleanup = owned.and_then(|owned| budget.release_storage(owned));
    match result {
        Err(payload) => resume_unwind(payload),
        Ok(returned) => {
            if let Some(error) = failure {
                return Err(error);
            }
            returned?;
            cleanup?;
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "function_control_flow_scope_v1_tests.rs"]
mod tests;
