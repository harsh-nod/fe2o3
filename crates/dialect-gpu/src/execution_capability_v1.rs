//! Exact target-neutral execution-capability operations for canonical KIR V13.

use dialect_kernel::{
    CanonicalIdentityAttr, ExecutionCapabilityContractAttr, ExecutionCapabilityType,
    SourceCoordinateAttr,
};
use fe2o3_kernel_ir::{
    ExecutionCapabilityOpV1 as KirExecutionCapabilityOp, MAX_EXECUTION_CAPABILITY_OPERANDS_V1,
    MAX_EXECUTION_CAPABILITY_RESULTS_V1,
};
use pliron::{
    builtin::ATTR_KEY_DEBUG_INFO,
    common_traits::Verify,
    context::Context,
    derive::pliron_op,
    op::Op,
    operation::Operation,
    result::Result,
    r#type::{TypeHandle, Typed},
    value::Value,
    verify_err,
};

use crate::TargetNeutralGpuOpInterface;

/// One exact KIR V13 execution operation in the live Pliron graph.
///
/// SSA values remain real operands/results. The immutable semantic contract,
/// source identity, source coordinate, and graph identity are independently
/// checkable attributes; no operation is represented by requirement metadata.
#[pliron_op(
    name = "gpu.execution_capability",
    format,
    interfaces = [TargetNeutralGpuOpInterface],
    attributes = (
        gpu_execution_capability_contract: ExecutionCapabilityContractAttr,
        gpu_execution_capability_graph_epoch: CanonicalIdentityAttr,
        gpu_execution_capability_coordinate: SourceCoordinateAttr,
        gpu_execution_capability_operation_identity: CanonicalIdentityAttr,
        gpu_execution_capability_source_operation_identity: CanonicalIdentityAttr
    )
)]
pub struct ExecutionCapabilityOp;

impl ExecutionCapabilityOp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        context: &mut Context,
        contract: &KirExecutionCapabilityOp,
        operands: Vec<Value>,
        result_types: Vec<TypeHandle>,
        graph_epoch: CanonicalIdentityAttr,
        coordinate: SourceCoordinateAttr,
        operation_identity: CanonicalIdentityAttr,
    ) -> Option<Self> {
        if operands.len() != contract.operands.len()
            || operands.len() > MAX_EXECUTION_CAPABILITY_OPERANDS_V1
            || result_types.len() > MAX_EXECUTION_CAPABILITY_RESULTS_V1
        {
            return None;
        }
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            result_types,
            operands,
            vec![],
            0,
        );
        let operation = Self::from_operation(operation);
        operation.set_attr_gpu_execution_capability_contract(
            context,
            ExecutionCapabilityContractAttr::new(contract)?,
        );
        operation.set_attr_gpu_execution_capability_graph_epoch(context, graph_epoch);
        operation.set_attr_gpu_execution_capability_coordinate(context, coordinate);
        operation.set_attr_gpu_execution_capability_operation_identity(context, operation_identity);
        operation.set_attr_gpu_execution_capability_source_operation_identity(
            context,
            CanonicalIdentityAttr::from_bytes(contract.source.operation),
        );
        Some(operation)
    }

    pub fn contract(&self, context: &Context) -> Option<KirExecutionCapabilityOp> {
        let operand_count = self.get_operation().deref(context).get_num_operands();
        self.get_attr_gpu_execution_capability_contract(context)?
            .contract(operand_count)
    }

    pub fn graph_epoch(&self, context: &Context) -> Option<[u8; 32]> {
        self.get_attr_gpu_execution_capability_graph_epoch(context)?
            .bytes()
    }

    pub fn coordinate(&self, context: &Context) -> Option<SourceCoordinateAttr> {
        self.get_attr_gpu_execution_capability_coordinate(context)
            .map(|value| *value)
    }

    pub fn operation_identity(&self, context: &Context) -> Option<[u8; 32]> {
        self.get_attr_gpu_execution_capability_operation_identity(context)?
            .bytes()
    }

    pub fn source_operation_identity(&self, context: &Context) -> Option<[u8; 32]> {
        self.get_attr_gpu_execution_capability_source_operation_identity(context)?
            .bytes()
    }

    pub fn operands(&self, context: &Context) -> Vec<Value> {
        self.get_operation().deref(context).operands().collect()
    }
}

impl Verify for ExecutionCapabilityOp {
    fn verify(&self, context: &Context) -> Result<()> {
        let raw = self.get_operation().deref(context);
        let debug = raw.attributes.0.get(&*ATTR_KEY_DEBUG_INFO);
        let debug_valid = debug
            .map(|attribute| {
                let id = attribute.get_attr_id();
                id.dialect.as_ref() == "builtin" && AsRef::<str>::as_ref(&id.name) == "debug_info"
            })
            .unwrap_or(true);
        if raw.get_num_operands() > MAX_EXECUTION_CAPABILITY_OPERANDS_V1
            || raw.get_num_results() > MAX_EXECUTION_CAPABILITY_RESULTS_V1
            || raw.get_num_successors() != 0
            || raw.num_regions() != 0
            || raw.attributes.0.len() != 5 + usize::from(debug.is_some())
            || !debug_valid
        {
            return verify_err!(
                self.loc(context),
                "gpu.execution_capability has an invalid bounded graph shape"
            );
        }
        let Some(contract) = self.contract(context) else {
            return verify_err!(
                self.loc(context),
                "gpu.execution_capability has no exact canonical contract"
            );
        };
        if self.graph_epoch(context).is_none()
            || self.coordinate(context).is_none()
            || self.operation_identity(context).is_none()
            || self.source_operation_identity(context) != Some(contract.source.operation)
        {
            return verify_err!(
                self.loc(context),
                "gpu.execution_capability has incomplete or substituted graph identity"
            );
        }
        for result in raw.results() {
            let result_type = result.get_type(context);
            let result_type = result_type.deref(context);
            let Some(capability) = result_type.downcast_ref::<ExecutionCapabilityType>() else {
                continue;
            };
            let Some(capability) = capability.capability() else {
                return verify_err!(
                    self.loc(context),
                    "gpu.execution_capability has a malformed logical result"
                );
            };
            if capability.source_type != contract.signature.output()
                || capability.provenance != contract.provenance
                || capability.workgroup_brand != contract.workgroup_brand
                || capability.epoch != contract.epoch_after.or(contract.epoch_before)
            {
                return verify_err!(
                    self.loc(context),
                    "gpu.execution_capability substituted logical result authority"
                );
            }
        }
        Ok(())
    }
}
