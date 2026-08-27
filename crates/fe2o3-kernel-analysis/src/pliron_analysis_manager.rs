//! Ephemeral shared analyses for one immutable PLIRON verification run.

use std::sync::Arc;

use pliron::{builtin::ops::FuncOp, context::Context, op::Op, operation::Operation};

use crate::pliron_function_inventory::{
    BoundedPlironFunctionInventoryFailureV1, BoundedPlironFunctionInventoryV1,
};
use crate::pliron_invocation_trace::{
    PlironExecutionLayoutV1, PlironInvocationTraceV1, PlironTraceFailureV1,
    pliron_execution_layout_with_inventory_v1, trace_pliron_invocations_with_inputs_v1,
};
use crate::pliron_memory_order::{
    PlironMemoryOrderAnalysisV1, PlironMemoryOrderFailureV1, analyze_pliron_memory_order_v1,
};
use crate::pliron_provenance_alias::{
    PlironProvenanceAliasAnalysisV1, PlironProvenanceFailureV1,
    collect_pliron_provenance_alias_with_inventory_v1,
};
use crate::pliron_simt_protocol::{PlironSimtProtocolAnalysisV1, analyze_pliron_simt_protocol_v1};
use crate::pliron_tensor_layout::{
    PlironTensorLayoutDataflowAnalysisV1, PlironTensorLayoutDataflowFailureV1,
    analyze_pliron_tensor_layout_dataflow_with_inventory_v1,
};
use crate::{
    PlironPresburgerAnalysisV1, SparseIndexAnalysisV1, SparseIndexFailureV1,
    analyze_pliron_sparse_indices_with_inventory_v1,
};

/// The manager has a fixed number of cache roots. Each cached analysis has its
/// own independent resource bounds, so a run cannot accumulate unbounded
/// entries by querying different analysis keys.
pub(crate) const MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1: usize = 9;

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

/// Cache for exactly one immutable function during one production validation.
///
/// This type is crate-private so only the fixed verifier pipeline can preserve
/// the immutability invariant. Every public single-pass or pipeline entry point
/// constructs a fresh manager; in particular, post-lowering revalidation never
/// observes pre-lowering cache state.
pub(crate) struct PlironAnalysisManagerV1 {
    function: pliron::context::Ptr<Operation>,
    function_inventory: Option<
        Result<Arc<BoundedPlironFunctionInventoryV1>, BoundedPlironFunctionInventoryFailureV1>,
    >,
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
}

impl PlironAnalysisManagerV1 {
    pub(crate) fn new(function: &FuncOp) -> Self {
        Self {
            function: function.get_operation(),
            function_inventory: None,
            sparse_indices: None,
            presburger: None,
            provenance_alias: None,
            execution_layout: None,
            exact_trace: None,
            tensor_layout_dataflow: None,
            memory_order: None,
            simt_protocol: None,
        }
    }

    pub(crate) fn prepare_function_inventory(&mut self, context: &Context, function: &FuncOp) {
        self.assert_function(function);
        if self.function_inventory.is_none() {
            self.function_inventory =
                Some(BoundedPlironFunctionInventoryV1::collect(context, function).map(Arc::new));
        }
        debug_assert!(self.cached_entries() <= MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1);
    }

    pub(crate) fn function_inventory(
        &self,
    ) -> Result<&BoundedPlironFunctionInventoryV1, BoundedPlironFunctionInventoryFailureV1> {
        self.function_inventory
            .as_ref()
            .expect("function inventory must be prepared before access")
            .as_ref()
            .map(Arc::as_ref)
            .map_err(Clone::clone)
    }

    pub(crate) fn function_inventory_handle(
        &self,
    ) -> Result<Arc<BoundedPlironFunctionInventoryV1>, BoundedPlironFunctionInventoryFailureV1>
    {
        self.function_inventory
            .as_ref()
            .expect("function inventory must be prepared before access")
            .as_ref()
            .map(Arc::clone)
            .map_err(Clone::clone)
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
            self.prepare_function_inventory(context, function);
            self.sparse_indices = Some(match self.function_inventory() {
                Ok(inventory) => {
                    analyze_pliron_sparse_indices_with_inventory_v1(context, function, inventory)
                }
                Err(failure) => Err(SparseIndexFailureV1::ResourceLimit {
                    resource: failure.resource(),
                    limit: failure.limit(),
                    actual: failure.actual(),
                }),
            });
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
            self.prepare_function_inventory(context, function);
            self.provenance_alias = Some(match self.function_inventory() {
                Ok(inventory) => {
                    collect_pliron_provenance_alias_with_inventory_v1(context, inventory)
                }
                Err(failure) => Err(PlironProvenanceFailureV1::ResourceLimit {
                    limit: failure.limit(),
                    actual: failure.actual(),
                }),
            });
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
            self.prepare_function_inventory(context, function);
            self.execution_layout = Some(match self.function_inventory() {
                Ok(inventory) => pliron_execution_layout_with_inventory_v1(context, inventory),
                Err(_) => Err(PlironTraceFailureV1::ResourceLimit),
            });
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
        self.exact_trace = Some(match (&self.sparse_indices, &self.execution_layout) {
            (Some(Ok(sparse)), Some(Ok(layout))) => {
                let inventory = self
                    .function_inventory()
                    .expect("trace inventory was prepared");
                trace_pliron_invocations_with_inputs_v1(context, inventory, sparse, *layout)
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
            self.prepare_function_inventory(context, function);
            self.tensor_layout_dataflow = Some(match self.function_inventory() {
                Ok(inventory) => {
                    analyze_pliron_tensor_layout_dataflow_with_inventory_v1(context, inventory)
                }
                Err(_) => Err(PlironTensorLayoutDataflowFailureV1::ResourceLimit),
            });
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

    pub(crate) fn cached_entries(&self) -> usize {
        usize::from(self.function_inventory.is_some())
            + usize::from(self.sparse_indices.is_some())
            + usize::from(self.presburger.is_some())
            + usize::from(self.provenance_alias.is_some())
            + usize::from(self.execution_layout.is_some())
            + usize::from(self.exact_trace.is_some())
            + usize::from(self.tensor_layout_dataflow.is_some())
            + usize::from(self.memory_order.is_some())
            + usize::from(self.simt_protocol.is_some())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pliron::{
        builtin::{ops::FuncOp, types::FunctionType},
        context::Context,
    };

    use super::*;

    #[test]
    fn all_analysis_roots_reuse_one_function_inventory() {
        let mut context = Context::new();
        let function_type = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "analysis_cache".try_into().unwrap(),
            function_type,
        );
        let mut analyses = PlironAnalysisManagerV1::new(&function);
        analyses.prepare_function_inventory(&context, &function);
        let first = analyses.function_inventory_handle().unwrap();

        analyses.prepare_sparse_indices(&context, &function);
        analyses.prepare_presburger(&context, &function);
        analyses.prepare_provenance_alias(&context, &function);
        analyses.prepare_execution_layout(&context, &function);
        analyses.prepare_exact_trace(&context, &function);
        analyses.prepare_tensor_layout_dataflow(&context, &function);
        analyses.prepare_memory_order(&context, &function);
        analyses.prepare_simt_protocol(&context, &function);

        let second = analyses.function_inventory_handle().unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(
            analyses.cached_entries(),
            MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1
        );
    }
}
