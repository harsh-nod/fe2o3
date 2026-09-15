//! Epoch convenience view over the shared replay-checked occurrence mapping.

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticExpandedWorkgroupEpochProjectionV1 {
    projection: SemanticWorkgroupEpochProjectionV1,
    binding: SemanticExpandedDefinedCapabilityV1,
}

impl SemanticExpandedWorkgroupEpochProjectionV1 {
    pub const fn projection(&self) -> SemanticWorkgroupEpochProjectionV1 {
        self.projection
    }
    pub const fn binding(&self) -> &SemanticExpandedDefinedCapabilityV1 {
        &self.binding
    }
    pub const fn expansion_identity(&self) -> &[u8; 32] {
        self.binding.expansion_identity()
    }
    pub const fn root_identity(&self) -> &[u8; 32] {
        self.binding.root_identity()
    }
    pub const fn root(&self) -> SemanticFunctionIdV1 {
        self.binding.root()
    }
    pub const fn caller_instance(&self) -> SemanticCallInstanceIdV1 {
        self.binding.caller_instance()
    }
    pub const fn callee_instance(&self) -> SemanticCallInstanceIdV1 {
        self.binding.callee_instance()
    }
    pub const fn caller_function(&self) -> SemanticFunctionIdV1 {
        self.binding.caller_function()
    }
    pub const fn call_block(&self) -> SemanticBlockIdV1 {
        self.binding.call_block()
    }
    pub const fn expanded_call_block(&self) -> SemanticBlockIdV1 {
        self.binding.expanded_call_block()
    }
    pub const fn expanded_projection_block(&self) -> SemanticBlockIdV1 {
        self.binding.expanded_entry_block()
    }
    pub fn receiver(&self) -> &SemanticOperandV1 {
        &self.binding.arguments()[0]
    }
    pub fn callee_receiver(&self) -> SemanticLocalIdV1 {
        self.binding.callee_arguments()[0]
    }
    pub const fn callee_return(&self) -> SemanticLocalIdV1 {
        self.binding.callee_return()
    }
}

impl SemanticCallExpansionV1 {
    pub fn workgroup_epoch_projection_bindings(
        &self,
        source: &AdmittedInertSemanticMirV1,
    ) -> Result<Vec<SemanticExpandedWorkgroupEpochProjectionV1>> {
        self.defined_capability_bindings(source)?
            .into_iter()
            .filter_map(|binding| match binding.contract() {
                SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(projection) => {
                    if binding.arguments().len() != 1 || binding.callee_arguments().len() != 1 {
                        return Some(Err(SemanticCallExpansionErrorV1::ReplayMismatch));
                    }
                    Some(Ok(SemanticExpandedWorkgroupEpochProjectionV1 {
                        projection,
                        binding,
                    }))
                }
                SemanticDefinedCapabilityContractV1::KernelMathDerive(_)
                | SemanticDefinedCapabilityContractV1::PolicyMathBind(_)
                | SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_)
                | SemanticDefinedCapabilityContractV1::KernelMatrixDerive(_)
                | SemanticDefinedCapabilityContractV1::ReusableLdsConversion(_)
                | SemanticDefinedCapabilityContractV1::GuardedGridLeader(_)
                | SemanticDefinedCapabilityContractV1::ReusablePhase(_)
                | SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_) => None,
            })
            .collect()
    }
}
