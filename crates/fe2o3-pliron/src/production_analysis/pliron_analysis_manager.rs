//! Ephemeral shared analyses for one immutable PLIRON verification run.

use std::sync::Arc;

#[cfg(test)]
use std::cell::Cell;

use pliron::{builtin::ops::FuncOp, context::Context, op::Op, operation::Operation};

use crate::production_analysis::pliron_function_inventory::{
    BoundedPlironFunctionInventoryFailureV1, BoundedPlironFunctionInventoryV1,
};
use crate::production_analysis::pliron_invocation_trace::{
    PlironExecutionLayoutV1, PlironInvocationTraceV1, PlironTraceFailureV1,
    pliron_execution_layout_with_inventory_v1, trace_pliron_invocations_with_inputs_v1,
};
use crate::production_analysis::pliron_memory_order::{
    PlironMemoryOrderAnalysisV1, PlironMemoryOrderFailureV1, analyze_pliron_memory_order_v1,
};
use crate::production_analysis::pliron_presburger_adapter::PlironPresburgerAnalysisV1;
use crate::production_analysis::pliron_provenance_alias::{
    PlironProvenanceAliasAnalysisV1, PlironProvenanceFailureV1,
    analyze_pliron_provenance_alias_with_inventory_v1,
};
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisReplacementLimitsV1,
    ProductionAnalysisResourceContractV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};
use crate::production_analysis::pliron_simt_protocol::{
    PlironSimtProtocolAnalysisV1, analyze_pliron_simt_protocol_v1,
};
use crate::production_analysis::pliron_tensor_layout::{
    PlironTensorLayoutDataflowAnalysisV1, PlironTensorLayoutDataflowFailureV1,
    analyze_pliron_tensor_layout_dataflow_with_inventory_v1,
};
use crate::{
    SparseIndexAnalysisV1, SparseIndexFailureV1, analyze_pliron_sparse_indices_with_inventory_v1,
};

/// The manager has a fixed number of cache roots. Each cached analysis has its
/// own independent resource bounds, so a run cannot accumulate unbounded
/// entries by querying different analysis keys.
pub(crate) const MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1: usize = 9;

#[cfg(test)]
thread_local! {
    static PANIC_NEXT_ANALYSIS_MANAGER_PREPARE_V1: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn panic_next_analysis_manager_prepare_for_test_v1() {
    PANIC_NEXT_ANALYSIS_MANAGER_PREPARE_V1.with(|flag| flag.set(true));
}

#[cfg(test)]
fn maybe_panic_during_analysis_manager_prepare_for_test_v1() {
    PANIC_NEXT_ANALYSIS_MANAGER_PREPARE_V1.with(|flag| {
        assert!(
            !flag.replace(false),
            "injected analysis-manager preparation panic"
        );
    });
}

#[cfg(not(test))]
const fn maybe_panic_during_analysis_manager_prepare_for_test_v1() {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PlironMemoryOrderAnalysisFailureV1 {
    Trace(PlironTraceFailureV1),
    Provenance(String),
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
    input_census: Option<ProductionAnalysisInputCensusV1>,
    resource_contract: ProductionAnalysisResourceContractV1,
    lineage_identity_retained_storage: usize,
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
    #[cfg(test)]
    pub(crate) fn new(function: &FuncOp) -> Self {
        Self::new_unmetered_for_standalone_v1(function)
    }

    // Standalone crate-private diagnostics retain their existing per-analysis
    // hard caps. The closed production pipeline uses the checked constructor
    // below and is the only route that contributes compiler authority.
    #[cfg(test)]
    fn new_unmetered_for_standalone_v1(function: &FuncOp) -> Self {
        Self::from_resource_contract(
            function,
            None,
            ProductionAnalysisResourceContractV1::new(
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            ),
            0,
        )
    }

    pub(crate) fn new_with_resource_contract(
        function: &FuncOp,
        input_census: ProductionAnalysisInputCensusV1,
        initial_setup_upper_bound: ProductionAnalysisResourceUpperBoundV1,
        initial_lineage_retained_storage: usize,
        limits: ProductionAnalysisResourceLimitsV1,
    ) -> Result<Self, ProductionAnalysisResourceLimitV1> {
        let phase = ProductionAnalysisResourcePhaseV1::FunctionInventory;
        let work_upper_bound = input_census
            .blocks
            .checked_add(input_census.operations)
            .and_then(|work| work.checked_add(1))
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "function inventory work upper bound",
            })?;
        let retained_storage_upper_bound = input_census
            .blocks
            .checked_mul(2)
            .and_then(|blocks| blocks.checked_add(input_census.operations))
            .ok_or(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "function inventory retained storage upper bound",
            })?;
        let inventory_bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            phase,
            work_upper_bound,
            retained_storage_upper_bound,
            0,
        )?;
        let mut resource_contract = ProductionAnalysisResourceContractV1::new(limits);
        if initial_lineage_retained_storage
            > initial_setup_upper_bound.retained_storage_upper_bound()
        {
            return Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::StructuralIdentity,
                resource: "initial lineage retained storage upper bound",
            });
        }
        resource_contract.admit_retained(
            ProductionAnalysisResourcePhaseV1::StructuralIdentity,
            initial_setup_upper_bound,
        )?;
        resource_contract.admit_retained(phase, inventory_bound)?;
        Ok(Self::from_resource_contract(
            function,
            Some(input_census),
            resource_contract,
            initial_lineage_retained_storage,
        ))
    }

    fn from_resource_contract(
        function: &FuncOp,
        input_census: Option<ProductionAnalysisInputCensusV1>,
        resource_contract: ProductionAnalysisResourceContractV1,
        lineage_identity_retained_storage: usize,
    ) -> Self {
        Self {
            function: function.get_operation(),
            input_census,
            resource_contract,
            lineage_identity_retained_storage,
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

    pub(crate) const fn input_census(&self) -> Option<ProductionAnalysisInputCensusV1> {
        self.input_census
    }

    pub(crate) fn remaining_resource_limits(
        &self,
        phase: ProductionAnalysisResourcePhaseV1,
    ) -> Result<ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourceLimitV1> {
        self.resource_contract.remaining(phase)
    }

    pub(crate) fn remaining_identity_replacement_resource_limits_v1(
        &self,
        replaced_lineage_retained_storage: usize,
    ) -> Result<ProductionAnalysisReplacementLimitsV1, ProductionAnalysisResourceLimitV1> {
        let phase = ProductionAnalysisResourcePhaseV1::PassPreservation;
        if replaced_lineage_retained_storage != self.lineage_identity_retained_storage {
            return Err(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "replaced identity retained storage upper bound",
            });
        }
        Ok(ProductionAnalysisReplacementLimitsV1 {
            input: self.resource_contract.remaining(phase)?,
            output: self
                .resource_contract
                .remaining_for_replacement(phase, replaced_lineage_retained_storage)?,
        })
    }

    pub(crate) fn admit_retained_resource_upper_bound(
        &mut self,
        phase: ProductionAnalysisResourcePhaseV1,
        upper_bound: ProductionAnalysisResourceUpperBoundV1,
    ) -> Result<(), ProductionAnalysisResourceLimitV1> {
        self.resource_contract.admit_retained(phase, upper_bound)
    }

    pub(crate) fn resource_contract_replace_retained_v1(
        &mut self,
        phase: ProductionAnalysisResourcePhaseV1,
        replaced_lineage_retained_storage: usize,
        replacement_stage_upper_bound: ProductionAnalysisResourceUpperBoundV1,
        new_lineage_retained_storage: usize,
    ) -> Result<(), ProductionAnalysisResourceLimitV1> {
        if replaced_lineage_retained_storage != self.lineage_identity_retained_storage {
            return Err(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "replaced identity retained storage upper bound",
            });
        }
        if new_lineage_retained_storage
            > replacement_stage_upper_bound.retained_storage_upper_bound()
        {
            return Err(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "replacement lineage retained storage upper bound",
            });
        }
        self.resource_contract.admit_replacement(
            phase,
            replaced_lineage_retained_storage,
            replacement_stage_upper_bound,
        )?;
        self.lineage_identity_retained_storage = new_lineage_retained_storage;
        Ok(())
    }

    pub(crate) const fn resource_upper_bound(&self) -> ProductionAnalysisResourceUpperBoundV1 {
        self.resource_contract.cumulative()
    }

    pub(crate) fn prepare_function_inventory(&mut self, context: &Context, function: &FuncOp) {
        self.assert_function(function);
        maybe_panic_during_analysis_manager_prepare_for_test_v1();
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
                    analyze_pliron_provenance_alias_with_inventory_v1(context, inventory)
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
    ) -> Result<&PlironProvenanceAliasAnalysisV1, &PlironProvenanceFailureV1> {
        self.provenance_alias
            .as_ref()
            .expect("provenance/alias analysis must be prepared before access")
            .as_ref()
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

    pub(crate) fn has_successful_exact_trace_v1(&self) -> bool {
        matches!(&self.exact_trace, Some(Ok(_)))
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
            (Ok(traces), Ok(provenance)) => analyze_pliron_memory_order_v1(traces, provenance)
                .map_err(PlironMemoryOrderAnalysisFailureV1::MemoryOrder),
            (Err(failure), _) => Err(PlironMemoryOrderAnalysisFailureV1::Trace(failure)),
            (_, Err(failure)) => Err(PlironMemoryOrderAnalysisFailureV1::Provenance(
                failure.bounded_description_v1(),
            )),
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

    include!("pliron_analysis_manager/replacement_limits_v1_tests.rs");

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

    #[test]
    fn function_inventory_bound_is_admitted_exactly_before_cache_construction() {
        let mut context = Context::new();
        let function_type = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "bounded_inventory".try_into().unwrap(),
            function_type,
        );
        let census = ProductionAnalysisInputCensusV1 {
            blocks: 3,
            operations: 5,
            ..ProductionAnalysisInputCensusV1::default()
        };
        // Work = B + O + constructor visit = 9. Retained storage = 2B + O = 11.
        let exact = ProductionAnalysisResourceLimitsV1::new(9, 11);
        let empty_identity = ProductionAnalysisResourceUpperBoundV1::default();
        let manager = PlironAnalysisManagerV1::new_with_resource_contract(
            &function,
            census,
            empty_identity,
            0,
            exact,
        )
        .unwrap();
        assert_eq!(manager.resource_upper_bound().work_upper_bound(), 9);
        assert_eq!(
            manager.resource_upper_bound().peak_storage_upper_bound(),
            11
        );
        assert!(
            PlironAnalysisManagerV1::new_with_resource_contract(
                &function,
                census,
                empty_identity,
                0,
                ProductionAnalysisResourceLimitsV1::new(8, 11),
            )
            .is_err()
        );
        assert_eq!(
            PlironAnalysisManagerV1::new_with_resource_contract(
                &function,
                census,
                empty_identity,
                1,
                ProductionAnalysisResourceLimitsV1::new(9, 11),
            )
            .err(),
            Some(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::StructuralIdentity,
                resource: "initial lineage retained storage upper bound",
            })
        );
        assert!(
            PlironAnalysisManagerV1::new_with_resource_contract(
                &function,
                census,
                empty_identity,
                0,
                ProductionAnalysisResourceLimitsV1::new(9, 10),
            )
            .is_err()
        );
    }

    #[test]
    fn identity_replacement_requires_the_exact_live_owner_bound() {
        let mut context = Context::new();
        let function_type = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "identity_replacement".try_into().unwrap(),
            function_type,
        );
        let setup = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::StructuralIdentity,
            1,
            7,
            0,
        )
        .unwrap();
        let mut manager = PlironAnalysisManagerV1::new_with_resource_contract(
            &function,
            ProductionAnalysisInputCensusV1::default(),
            setup,
            5,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let replacement = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::PassPreservation,
            1,
            11,
            0,
        )
        .unwrap();
        assert_eq!(
            manager.resource_contract_replace_retained_v1(
                ProductionAnalysisResourcePhaseV1::PassPreservation,
                6,
                replacement,
                4,
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                resource: "replaced identity retained storage upper bound",
            })
        );
        manager
            .resource_contract_replace_retained_v1(
                ProductionAnalysisResourcePhaseV1::PassPreservation,
                5,
                replacement,
                4,
            )
            .unwrap();
        assert_eq!(
            manager
                .resource_upper_bound()
                .retained_storage_upper_bound(),
            13
        );

        assert_eq!(
            manager.resource_contract_replace_retained_v1(
                ProductionAnalysisResourcePhaseV1::PassPreservation,
                4,
                second_replacement_with_retained_v1(3),
                4,
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                resource: "replacement lineage retained storage upper bound",
            })
        );

        let second_replacement = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::PassPreservation,
            2,
            9,
            3,
        )
        .unwrap();
        assert_eq!(
            manager.resource_contract_replace_retained_v1(
                ProductionAnalysisResourcePhaseV1::PassPreservation,
                replacement.retained_storage_upper_bound(),
                second_replacement,
                6,
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                resource: "replaced identity retained storage upper bound",
            })
        );
        manager
            .resource_contract_replace_retained_v1(
                ProductionAnalysisResourcePhaseV1::PassPreservation,
                4,
                second_replacement,
                6,
            )
            .unwrap();
        assert_eq!(
            manager
                .resource_upper_bound()
                .retained_storage_upper_bound(),
            18
        );
    }

    fn second_replacement_with_retained_v1(
        retained: usize,
    ) -> ProductionAnalysisResourceUpperBoundV1 {
        ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::PassPreservation,
            1,
            retained,
            0,
        )
        .unwrap()
    }
}
