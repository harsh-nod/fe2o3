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
    InvalidEdge {
        source: BlockId,
        ordinal: usize,
    },
    InvalidParameter {
        block: BlockId,
        ordinal: usize,
    },
    InvalidIncomingEdge {
        target: BlockId,
        ordinal: usize,
    },
    InvalidEdgeArguments {
        source: BlockId,
        ordinal: usize,
        target: BlockId,
        expected: usize,
        actual: usize,
    },
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

/// An inert coordinate in this exact raw function, not a type or origin proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionControlFlowParameterV1 {
    pub value: ValueId,
    pub incoming_count: usize,
}

/// One exact incoming edge argument. Parallel source edges retain distinct
/// ordinals; disconnected predecessors are reported, never silently removed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionControlFlowParameterInputV1 {
    pub source: BlockId,
    pub source_ordinal: usize,
    pub source_reachable: bool,
    pub target: BlockId,
    pub parameter_ordinal: usize,
    pub parameter: ValueId,
    pub argument: ValueId,
}

impl<'function> FunctionControlFlowViewV1<'_, 'function, '_> {
    pub fn function(&self) -> &'function Function {
        self.function
    }

    /// Looks up a block parameter without building another value/CFG index.
    /// The incoming count includes every actual edge, including parallel and
    /// unreachable edges. No parameter type, initialization or origin is proved.
    pub fn block_parameter(
        &mut self,
        block: BlockId,
        ordinal: usize,
    ) -> Result<FunctionControlFlowParameterV1, Error> {
        let position = self.block(block)?;
        self.charge(5)?;
        let function = self.function;
        let parameter = function.body.as_ref().expect("analyzed body").blocks[position]
            .parameters
            .get(ordinal)
            .ok_or_else(|| self.fail(Error::InvalidParameter { block, ordinal }))?;
        Ok(FunctionControlFlowParameterV1 {
            value: parameter.id,
            incoming_count: self.flow.indexed_v15().incoming[position].len(),
        })
    }

    /// Reads one actual argument from the shared indexed CFG. The caller must
    /// independently match all returned coordinates to its original source
    /// relation and prove each conditional permission. No origins are merged.
    pub fn block_parameter_input(
        &mut self,
        target: BlockId,
        parameter_ordinal: usize,
        incoming_ordinal: usize,
    ) -> Result<FunctionControlFlowParameterInputV1, Error> {
        let position = self.block(target)?;
        self.charge(11)?;
        let function = self.function;
        let indexed = self.flow;
        let body = function.body.as_ref().expect("analyzed body");
        let parameters = &body.blocks[position].parameters;
        let parameter = parameters
            .get(parameter_ordinal)
            .ok_or_else(|| {
                self.fail(Error::InvalidParameter {
                    block: target,
                    ordinal: parameter_ordinal,
                })
            })?
            .id;
        let flow = indexed.indexed_v15();
        let edge_index = *flow.incoming[position]
            .get(incoming_ordinal)
            .ok_or_else(|| {
                self.fail(Error::InvalidIncomingEdge {
                    target,
                    ordinal: incoming_ordinal,
                })
            })?;
        let edge = flow.edges[edge_index];
        let source = body.blocks[edge.source].id;
        let arguments = flow.edge_arguments(function, edge_index);
        if arguments.len() != parameters.len() {
            return Err(self.fail(Error::InvalidEdgeArguments {
                source,
                ordinal: edge.ordinal,
                target,
                expected: parameters.len(),
                actual: arguments.len(),
            }));
        }
        Ok(FunctionControlFlowParameterInputV1 {
            source,
            source_ordinal: edge.ordinal,
            source_reachable: flow.reachable[edge.source],
            target,
            parameter_ordinal,
            parameter,
            argument: arguments[parameter_ordinal],
        })
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

    /// Reports the unique structural parameter origin while treating `boundary`
    /// as opaque. This permits forwarding a multi-origin parameter without
    /// conflating its predecessors. It proves nothing about the named boundary;
    /// callers must authenticate its exact definition and independently replay
    /// the selected incoming edges. Casts and arbitrary operations are not
    /// traversed. As in `unique_value_origin`, non-parameter values name themselves.
    /// Temporary SCC rows are refunded after this query, including refusal.
    pub fn unique_value_origin_until(
        &mut self,
        value: ValueId,
        boundary: ValueId,
    ) -> Result<Option<ValueId>, Error> {
        self.charge(1)?;
        let floor = self.budget.storage();
        let result = (|| {
            self.budget.reserve_storage(boundary_origin_header_v1()?)?;
            let rows = crate::formal_memory_obligations::structural_origins_until_v1(
                self.function,
                self.flow.indexed_v15(),
                Some(boundary),
                self.budget,
            )?;
            self.charge(rows.len().checked_ilog2().unwrap_or(0) as usize + 3)?;
            Ok(match rows.binary_search_by_key(&value, |row| row.0) {
                Ok(index) => rows[index].1,
                Err(_) => Some(value),
            })
        })();
        let refund = self
            .budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)
            .and_then(|owned| self.budget.release_storage(owned));
        match result {
            Err(error) => Err(self.fail(error)),
            Ok(origin) => {
                refund.map_err(|error| self.fail(error.into()))?;
                Ok(origin)
            }
        }
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

fn boundary_origin_header_v1() -> Result<usize, Resource> {
    [
        size_of::<Vec<(ValueId, Option<ValueId>)>>(),
        size_of::<Result<Vec<(ValueId, Option<ValueId>)>, Resource>>(),
        2 * size_of::<Option<ValueId>>(),
        2 * size_of::<Result<Option<ValueId>, Error>>(),
        size_of::<Result<(), Resource>>(),
        size_of::<Result<usize, usize>>(),
        size_of::<Result<usize, Resource>>(),
        2 * size_of::<usize>(),
        size_of::<(&mut FunctionControlFlowViewV1<'_, '_, '_>, ValueId, ValueId)>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        sum.checked_add(value).ok_or(Resource::Arithmetic)
    })
}

fn function_scope_header_v1() -> Result<usize, Resource> {
    [
        size_of::<Option<MeteredIndexedControlFlowV1>>(),
        size_of::<Option<Error>>(),
        size_of::<Option<Vec<(ValueId, Option<ValueId>)>>>(),
        size_of::<FunctionControlFlowViewV1<'_, '_, '_>>(),
        size_of::<Result<(), Error>>(),
        size_of::<Result<Result<(), Error>, Box<dyn std::any::Any + Send>>>(),
        size_of::<FunctionControlFlowParameterV1>(),
        size_of::<Result<FunctionControlFlowParameterV1, Error>>(),
        size_of::<FunctionControlFlowParameterInputV1>(),
        size_of::<Result<FunctionControlFlowParameterInputV1, Error>>(),
        size_of::<&Function>(),
        size_of::<&MeteredIndexedControlFlowV1>(),
        3 * size_of::<usize>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        sum.checked_add(value).ok_or(Resource::Arithmetic)
    })
}

// The catch closure owns exactly this frame; the callback remains inside it
// until the graph is constructed. All graph/query payload retains its old units.
#[repr(C)]
struct FunctionScopeInvocationV1<'scope, 'function, 'work, F> {
    function: &'function Function,
    limits: ControlFlowLimits,
    budget: &'scope mut Budget<'work>,
    flow: &'scope mut Option<MeteredIndexedControlFlowV1>,
    failure: &'scope mut Option<Error>,
    origins: &'scope mut Option<Vec<(ValueId, Option<ValueId>)>>,
    run: F,
}

impl<'function, 'work, F> FunctionScopeInvocationV1<'_, 'function, 'work, F>
where
    F: for<'scope> FnOnce(
        &mut FunctionControlFlowViewV1<'scope, 'function, 'work>,
    ) -> Result<(), Error>,
{
    fn invoke(self) -> Result<(), Error> {
        let Self {
            function,
            limits,
            budget,
            flow,
            failure,
            origins,
            run,
        } = self;
        *flow = Some(
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
            failure,
            origins,
        };
        run(&mut view)
    }
}

fn function_scope_callback_header_v1<F>(_: &F) -> Result<usize, Resource> {
    // Conservative coexisting parameter, invocation and unwind envelopes.
    // Result/query envelopes are already in function_scope_header_v1.
    [
        size_of::<F>(),
        size_of::<FunctionScopeInvocationV1<'_, '_, '_, F>>(),
        size_of::<AssertUnwindSafe<FunctionScopeInvocationV1<'_, '_, '_, F>>>(),
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
    let headers = function_scope_header_v1()?
        .checked_add(function_scope_callback_header_v1(&run)?)
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(headers)?;
    let mut flow = None;
    let mut failure = None;
    let mut origins = None;
    let invocation = FunctionScopeInvocationV1 {
        function,
        limits,
        budget,
        flow: &mut flow,
        failure: &mut failure,
        origins: &mut origins,
        run,
    };
    let result = catch_unwind(AssertUnwindSafe(move || invocation.invoke()));
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
