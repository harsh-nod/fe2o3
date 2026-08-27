//! Ephemeral shared analyses for one immutable PLIRON verification run.

use pliron::{builtin::ops::FuncOp, context::Context, op::Op, operation::Operation};

use crate::pliron_invocation_trace::{
    PlironExecutionLayoutV1, PlironInvocationTraceV1, PlironTraceFailureV1,
    pliron_execution_layout_v1, trace_pliron_invocations_with_inputs_v1,
};
use crate::pliron_memory_order::{
    PlironMemoryOrderAnalysisV1, PlironMemoryOrderFailureV1, analyze_pliron_memory_order_v1,
};
use crate::pliron_provenance_alias::{
    PlironProvenanceAliasAnalysisV1, PlironProvenanceFailureV1, collect_pliron_provenance_alias_v1,
};
use crate::pliron_simt_protocol::{PlironSimtProtocolAnalysisV1, analyze_pliron_simt_protocol_v1};
use crate::pliron_tensor_layout::{
    PlironTensorLayoutDataflowAnalysisV1, PlironTensorLayoutDataflowFailureV1,
    analyze_pliron_tensor_layout_dataflow_v1,
};
use crate::{
    PlironPresburgerAnalysisV1, SparseIndexAnalysisV1, SparseIndexFailureV1,
    analyze_pliron_sparse_indices_v1,
};

/// The manager has a fixed number of cache roots. Each cached analysis has its
/// own independent resource bounds, so a run cannot accumulate unbounded
/// entries by querying different analysis keys.
pub(crate) const MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PlironMemoryOrderAnalysisFailureV1 {
    Trace(PlironTraceFailureV1),
    Provenance(PlironProvenanceFailureV1),
    MemoryOrder(PlironMemoryOrderFailureV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PlironSimtProtocolAnalysisFailureV1 {
    Trace(PlironTraceFailureV1),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlironAnalysisComputationCountsV1 {
    pub(crate) sparse_indices: usize,
    pub(crate) presburger: usize,
    pub(crate) provenance_alias: usize,
    pub(crate) execution_layout: usize,
    pub(crate) exact_trace: usize,
    pub(crate) tensor_layout_dataflow: usize,
    pub(crate) memory_order: usize,
    pub(crate) simt_protocol: usize,
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
    presburger: Option<Result<PlironPresburgerAnalysisV1, SparseIndexFailureV1>>,
    provenance_alias: Option<Result<PlironProvenanceAliasAnalysisV1, PlironProvenanceFailureV1>>,
    execution_layout: Option<Result<Option<PlironExecutionLayoutV1>, PlironTraceFailureV1>>,
    exact_trace: Option<Result<Vec<PlironInvocationTraceV1>, PlironTraceFailureV1>>,
    tensor_layout_dataflow:
        Option<Result<PlironTensorLayoutDataflowAnalysisV1, PlironTensorLayoutDataflowFailureV1>>,
    memory_order: Option<Result<PlironMemoryOrderAnalysisV1, PlironMemoryOrderAnalysisFailureV1>>,
    simt_protocol:
        Option<Result<PlironSimtProtocolAnalysisV1, PlironSimtProtocolAnalysisFailureV1>>,
    computations: PlironAnalysisComputationCountsV1,
}

impl PlironAnalysisManagerV1 {
    pub(crate) fn new(function: &FuncOp) -> Self {
        Self {
            function: function.get_operation(),
            sparse_indices: None,
            presburger: None,
            provenance_alias: None,
            execution_layout: None,
            exact_trace: None,
            tensor_layout_dataflow: None,
            memory_order: None,
            simt_protocol: None,
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

    pub(crate) fn prepare_presburger(&mut self, context: &Context, function: &FuncOp) {
        self.assert_function(function);
        if self.presburger.is_some() {
            return;
        }
        self.prepare_sparse_indices(context, function);
        self.computations.presburger += 1;
        self.presburger = Some(match &self.sparse_indices {
            Some(Ok(sparse)) => Ok(PlironPresburgerAnalysisV1::from_sparse(sparse)),
            Some(Err(failure)) => Err(failure.clone()),
            None => unreachable!("sparse indices were prepared above"),
        });
        debug_assert!(self.cached_entries() <= MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1);
    }

    pub(crate) fn presburger(&self) -> Result<&PlironPresburgerAnalysisV1, SparseIndexFailureV1> {
        self.presburger
            .as_ref()
            .expect("Presburger analysis must be prepared before access")
            .as_ref()
            .map_err(Clone::clone)
    }

    pub(crate) fn prepare_provenance_alias(&mut self, context: &Context, function: &FuncOp) {
        self.assert_function(function);
        if self.provenance_alias.is_none() {
            self.computations.provenance_alias += 1;
            self.provenance_alias = Some(collect_pliron_provenance_alias_v1(context, function));
        }
        debug_assert!(self.cached_entries() <= MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1);
    }

    pub(crate) fn provenance_alias(
        &self,
    ) -> Result<&PlironProvenanceAliasAnalysisV1, PlironProvenanceFailureV1> {
        self.provenance_alias
            .as_ref()
            .expect("provenance/alias analysis must be prepared before access")
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

    pub(crate) fn prepare_memory_order(&mut self, context: &Context, function: &FuncOp) {
        self.assert_function(function);
        if self.memory_order.is_some() {
            return;
        }
        self.prepare_exact_trace(context, function);
        self.prepare_provenance_alias(context, function);
        self.computations.memory_order += 1;
        self.memory_order = Some(match (self.exact_trace(), self.provenance_alias()) {
            (Ok(traces), Ok(provenance)) => {
                match provenance.validate_space(dialect_kernel::MemorySpaceAttr::Workgroup) {
                    Ok(()) => analyze_pliron_memory_order_v1(traces, provenance)
                        .map_err(PlironMemoryOrderAnalysisFailureV1::MemoryOrder),
                    Err(failure) => Err(PlironMemoryOrderAnalysisFailureV1::Provenance(failure)),
                }
            }
            (Err(failure), _) => Err(PlironMemoryOrderAnalysisFailureV1::Trace(failure)),
            (_, Err(failure)) => Err(PlironMemoryOrderAnalysisFailureV1::Provenance(failure)),
        });
        debug_assert!(self.cached_entries() <= MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1);
    }

    pub(crate) fn memory_order(
        &self,
    ) -> Result<&PlironMemoryOrderAnalysisV1, PlironMemoryOrderAnalysisFailureV1> {
        self.memory_order
            .as_ref()
            .expect("memory order must be prepared before access")
            .as_ref()
            .map_err(Clone::clone)
    }

    pub(crate) fn prepare_simt_protocol(&mut self, context: &Context, function: &FuncOp) {
        self.assert_function(function);
        if self.simt_protocol.is_some() {
            return;
        }
        self.prepare_exact_trace(context, function);
        self.computations.simt_protocol += 1;
        self.simt_protocol = Some(match self.exact_trace() {
            Ok(traces) => Ok(analyze_pliron_simt_protocol_v1(traces)),
            Err(failure) => Err(PlironSimtProtocolAnalysisFailureV1::Trace(failure)),
        });
        debug_assert!(self.cached_entries() <= MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1);
    }

    pub(crate) fn simt_protocol(
        &self,
    ) -> Result<&PlironSimtProtocolAnalysisV1, PlironSimtProtocolAnalysisFailureV1> {
        self.simt_protocol
            .as_ref()
            .expect("SIMT protocol must be prepared before access")
            .as_ref()
            .map_err(Clone::clone)
    }

    #[cfg(test)]
    pub(crate) const fn computation_counts(&self) -> PlironAnalysisComputationCountsV1 {
        self.computations
    }

    pub(crate) fn cached_entries(&self) -> usize {
        usize::from(self.sparse_indices.is_some())
            + usize::from(self.presburger.is_some())
            + usize::from(self.provenance_alias.is_some())
            + usize::from(self.execution_layout.is_some())
            + usize::from(self.exact_trace.is_some())
            + usize::from(self.tensor_layout_dataflow.is_some())
            + usize::from(self.memory_order.is_some())
            + usize::from(self.simt_protocol.is_some())
    }
}
