//! Ephemeral shared analyses for one immutable PLIRON verification run.

use pliron::{builtin::ops::FuncOp, context::Context, op::Op, operation::Operation};

use crate::pliron_invocation_trace::{
    PlironExecutionLayoutV1, PlironInvocationTraceV1, PlironTraceFailureV1,
    pliron_execution_layout_v1, trace_pliron_invocations_with_inputs_v1,
};
use crate::pliron_tensor_layout::{
    PlironTensorLayoutDataflowAnalysisV1, PlironTensorLayoutDataflowFailureV1,
    analyze_pliron_tensor_layout_dataflow_v1,
};
use crate::{SparseIndexAnalysisV1, SparseIndexFailureV1, analyze_pliron_sparse_indices_v1};

/// The manager has a fixed number of cache roots. Each cached analysis has its
/// own independent resource bounds, so a run cannot accumulate unbounded
/// entries by querying different analysis keys.
pub(crate) const MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1: usize = 4;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlironAnalysisComputationCountsV1 {
    pub(crate) sparse_indices: usize,
    pub(crate) execution_layout: usize,
    pub(crate) exact_trace: usize,
    pub(crate) tensor_layout_dataflow: usize,
}

/// Cache for exactly one immutable function during one production validation.
///
/// This type is crate-private so only the fixed verifier pipeline can preserve
/// the immutability invariant. Every public single-pass or pipeline entry point
/// constructs a fresh manager; in particular, post-lowering revalidation never
/// observes pre-lowering cache state.
pub(crate) struct PlironAnalysisManagerV1 {
    function: pliron::context::Ptr<Operation>,
    sparse_indices: Option<Result<SparseIndexAnalysisV1, SparseIndexFailureV1>>,
    execution_layout: Option<Result<Option<PlironExecutionLayoutV1>, PlironTraceFailureV1>>,
    exact_trace: Option<Result<Vec<PlironInvocationTraceV1>, PlironTraceFailureV1>>,
    tensor_layout_dataflow:
        Option<Result<PlironTensorLayoutDataflowAnalysisV1, PlironTensorLayoutDataflowFailureV1>>,
    computations: PlironAnalysisComputationCountsV1,
}

impl PlironAnalysisManagerV1 {
    pub(crate) fn new(function: &FuncOp) -> Self {
        Self {
            function: function.get_operation(),
            sparse_indices: None,
            execution_layout: None,
            exact_trace: None,
            tensor_layout_dataflow: None,
            computations: PlironAnalysisComputationCountsV1::default(),
        }
    }

    fn assert_function(&self, function: &FuncOp) {
        assert_eq!(
            self.function,
            function.get_operation(),
            "PLIRON analysis manager cannot be reused for another function"
        );
    }

    pub(crate) fn prepare_sparse_indices(&mut self, context: &Context, function: &FuncOp) {
        self.assert_function(function);
        if self.sparse_indices.is_none() {
            self.computations.sparse_indices += 1;
            self.sparse_indices = Some(analyze_pliron_sparse_indices_v1(context, function));
        }
        debug_assert!(self.cached_entries() <= MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1);
    }

    pub(crate) fn sparse_indices(&self) -> Result<&SparseIndexAnalysisV1, SparseIndexFailureV1> {
        self.sparse_indices
            .as_ref()
            .expect("sparse indices must be prepared before access")
            .as_ref()
            .map_err(Clone::clone)
    }

    pub(crate) fn prepare_execution_layout(&mut self, context: &Context, function: &FuncOp) {
        self.assert_function(function);
        if self.execution_layout.is_none() {
            self.computations.execution_layout += 1;
            self.execution_layout = Some(pliron_execution_layout_v1(context, function));
        }
        debug_assert!(self.cached_entries() <= MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1);
    }

    pub(crate) fn execution_layout(
        &self,
    ) -> Result<Option<PlironExecutionLayoutV1>, PlironTraceFailureV1> {
        self.execution_layout
            .as_ref()
            .expect("execution layout must be prepared before access")
            .as_ref()
            .copied()
            .map_err(Clone::clone)
    }

    pub(crate) fn prepare_exact_trace(&mut self, context: &Context, function: &FuncOp) {
        self.assert_function(function);
        if self.exact_trace.is_some() {
            return;
        }
        self.prepare_sparse_indices(context, function);
        self.prepare_execution_layout(context, function);
        self.computations.exact_trace += 1;
        self.exact_trace = Some(match (&self.sparse_indices, &self.execution_layout) {
            (Some(Ok(sparse)), Some(Ok(layout))) => {
                trace_pliron_invocations_with_inputs_v1(context, function, sparse, *layout)
            }
            (Some(Err(failure)), _) => Err(PlironTraceFailureV1::Sparse(failure.clone())),
            (_, Some(Err(failure))) => Err(failure.clone()),
            _ => unreachable!("trace prerequisites were prepared above"),
        });
        debug_assert!(self.cached_entries() <= MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1);
    }

    pub(crate) fn exact_trace(&self) -> Result<&[PlironInvocationTraceV1], PlironTraceFailureV1> {
        self.exact_trace
            .as_ref()
            .expect("exact trace must be prepared before access")
            .as_ref()
            .map(Vec::as_slice)
            .map_err(Clone::clone)
    }

    pub(crate) fn prepare_tensor_layout_dataflow(&mut self, context: &Context, function: &FuncOp) {
        self.assert_function(function);
        if self.tensor_layout_dataflow.is_none() {
            self.computations.tensor_layout_dataflow += 1;
            self.tensor_layout_dataflow =
                Some(analyze_pliron_tensor_layout_dataflow_v1(context, function));
        }
        debug_assert!(self.cached_entries() <= MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1);
    }

    pub(crate) fn tensor_layout_dataflow(
        &self,
    ) -> Result<&PlironTensorLayoutDataflowAnalysisV1, PlironTensorLayoutDataflowFailureV1> {
        self.tensor_layout_dataflow
            .as_ref()
            .expect("tensor layout dataflow must be prepared before access")
            .as_ref()
            .map_err(Clone::clone)
    }

    #[cfg(test)]
    pub(crate) const fn computation_counts(&self) -> PlironAnalysisComputationCountsV1 {
        self.computations
    }

    pub(crate) fn cached_entries(&self) -> usize {
        usize::from(self.sparse_indices.is_some())
            + usize::from(self.execution_layout.is_some())
            + usize::from(self.exact_trace.is_some())
            + usize::from(self.tensor_layout_dataflow.is_some())
    }
}
