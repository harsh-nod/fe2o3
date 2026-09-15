//! Move-only custody of the actual backend result and its two permitted text
//! transformations. These records do not provide functional or machine proof.

use super::{
    AmdProductionBackendV1, AmdProductionTargetV1, ProductionBackendAdapterV1,
    ProductionBackendErrorV1 as Error, ProductionBackendTargetContractV1,
    ProductionBackendTargetV1,
};
use crate::kernel_ir_codegen::{
    InertCompilerModuleTextV1, bind_compiler_descriptor_source_v1,
    retain_production_compiler_module_text_v1,
};
use fe2o3_compiler_ffi::{
    CompilerDescriptorSourceIdentityV1, CompilerDescriptorSourceV1,
    CompilerModuleHandoffIdentityV2, CompilerModuleHandoffV2, CompilerModuleKindV1, DeviceTargetV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVersionV1, Module, VerifiedCanonicalKernelIrIdentityV1,
    VerifiedCanonicalKernelIrV1, VerifiedCanonicalKernelIrV13,
};

/// Only real backend lowering constructs this owner. In particular there is no
/// text/digest constructor and no cloning of a structured result into authority.
#[derive(Debug)]
pub(crate) struct ProductionBackendLoweredModuleV1 {
    target: ProductionBackendTargetContractV1,
    canonical: VerifiedCanonicalKernelIrIdentityV1,
    lowered: dialect_amdgcn::ProductionCanonicalAmdLoweredModuleV1,
}

impl ProductionBackendLoweredModuleV1 {
    pub(super) fn lower_v13(
        target: &AmdProductionTargetV1,
        canonical: &VerifiedCanonicalKernelIrV13,
        epoch: u64,
        closure: &super::AmdProductionCapabilityClosureV1,
    ) -> Result<Self, Error> {
        if closure.profile != target.profile {
            return Err(Error::CapabilityClosureChanged);
        }
        let launch = dialect_amdgcn::ProductionTargetLaunchEvidenceKirV1::from_exact_values(
            canonical.as_common(),
            epoch,
            closure.closure.launch_evidence().values().iter().copied(),
        )
        .map_err(Error::CapabilityClosure)?;
        let declared = dialect_amdgcn::ProductionTargetCapabilityClosureKirV1::decode_canonical(
            closure.closure.canonical_bytes(),
            &launch,
        )
        .map_err(Error::CapabilityCanonical)?;
        Self::lower_declared(target, canonical.as_common(), epoch, &declared)
    }

    pub(super) fn lower_declared(
        target: &AmdProductionTargetV1,
        canonical: &VerifiedCanonicalKernelIrV1,
        epoch: u64,
        closure: &dialect_amdgcn::ProductionTargetCapabilityClosureKirV1,
    ) -> Result<Self, Error> {
        let lowered = dialect_amdgcn::lower_verified_canonical_kir_to_amd_llvm_ir_v1(
            canonical,
            epoch,
            closure.launch_evidence(),
            target.profile,
        )
        .map_err(Error::V13Lowering)?;
        if lowered.capability_closure() != closure {
            return Err(Error::CapabilityClosureChanged);
        }
        Ok(Self {
            target: AmdProductionBackendV1::target_contract(target),
            canonical: *canonical.identity(),
            lowered,
        })
    }

    pub(crate) fn llvm_ir(&self) -> &str {
        self.lowered.llvm_ir()
    }

    pub(crate) fn bind_worker_layout_v1(
        self,
        target: &ProductionBackendTargetV1,
    ) -> Result<ProductionBackendWorkerModuleV1, Error> {
        if self.target != target.contract() {
            return Err(Error::TargetOutput("worker layout target changed"));
        }
        let llvm_ir = target.bind_worker_layout_v1(self.lowered.llvm_ir())?;
        Ok(ProductionBackendWorkerModuleV1 {
            backend: self,
            llvm_ir,
        })
    }

    fn validate_module(&self, module: &Module) -> Result<(), Error> {
        let bytes = match self.canonical.version() {
            CanonicalKernelIrVersionV1::V13 => fe2o3_kernel_ir::encode_module_v13(module),
            CanonicalKernelIrVersionV1::V14 => fe2o3_kernel_ir::encode_module_v14(module),
        }
        .map_err(|error| {
            Error::DeclaredCanonical(fe2o3_kernel_ir::VerifiedCanonicalKernelIrErrorV1::Encode(
                error,
            ))
        })?;
        let observed =
            VerifiedCanonicalKernelIrV1::from_canonical_bytes(bytes, self.canonical.version())
                .map_err(Error::DeclaredCanonical)?;
        if observed.identity() != &self.canonical {
            return Err(Error::TargetOutput(
                "retained module is not the lowered declared graph",
            ));
        }
        Ok(())
    }

    fn validate_translation(&self) -> Result<(), Error> {
        match self.canonical.version() {
            CanonicalKernelIrVersionV1::V13 => {
                // V13 retains its pre-existing diagnostics and proof route;
                // an absent scalar derivation is not relabelled as complete.
                if self.lowered.declared_replay().is_some() {
                    return Err(Error::TargetOutput("V13 structured output changed version"));
                }
            }
            CanonicalKernelIrVersionV1::V14 => {
                if !self
                    .lowered
                    .has_complete_operational_translation_derivation()
                {
                    return Err(Error::TargetOutput(
                        "complete operational translation is unavailable",
                    ));
                }
                let replay = self.lowered.declared_replay().ok_or(Error::TargetOutput(
                    "V14 output lost its declared physical and writer replay",
                ))?;
                if replay.version() != CanonicalKernelIrVersionV1::V14
                    || replay.structured().is_none()
                    || replay.closure_identity() != self.lowered.capability_closure_identity()
                {
                    return Err(Error::TargetOutput(
                        "V14 structured output changed closure or version",
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Exact worker-layout text, still coupled to the complete original output.
#[derive(Debug)]
pub(crate) struct ProductionBackendWorkerModuleV1 {
    backend: ProductionBackendLoweredModuleV1,
    llvm_ir: String,
}

impl ProductionBackendWorkerModuleV1 {
    pub(crate) fn llvm_ir(&self) -> &str {
        &self.llvm_ir
    }

    pub(crate) fn retain_compiler_module(
        self,
        target: DeviceTargetV1,
        module: &Module,
    ) -> Result<ProductionBackendModuleTextV1, Error> {
        if target.to_string() != self.backend.target.canonical_target() {
            return Err(Error::TargetOutput("module target changed"));
        }
        self.backend.validate_module(module)?;
        self.backend.validate_translation()?;
        let compiler_module = retain_production_compiler_module_text_v1(module, self.llvm_ir)
            .map_err(Error::CompilerModule)?;
        Ok(ProductionBackendModuleTextV1 {
            backend: self.backend,
            compiler_module,
        })
    }
}

pub(crate) struct ProductionBackendModuleTextV1 {
    backend: ProductionBackendLoweredModuleV1,
    compiler_module: InertCompilerModuleTextV1,
}

impl ProductionBackendModuleTextV1 {
    pub(crate) fn compiler_module(&self) -> &InertCompilerModuleTextV1 {
        &self.compiler_module
    }

    pub(crate) fn bind_descriptor(
        self,
        descriptor: &CompilerDescriptorSourceV1,
    ) -> Result<ProductionBackendDescriptorModuleV1, Error> {
        if descriptor.table().device_target().to_string() != self.backend.target.canonical_target()
            || u16::from(descriptor.table().code_object_version().number())
                != self.backend.target.code_object_version()
        {
            return Err(Error::TargetOutput("descriptor target changed"));
        }
        let compiler_module = bind_compiler_descriptor_source_v1(self.compiler_module, descriptor)
            .map_err(Error::CompilerModule)?;
        Ok(ProductionBackendDescriptorModuleV1 {
            backend: self.backend,
            compiler_module,
            descriptor: descriptor.identity(),
        })
    }
}

pub(crate) struct ProductionBackendDescriptorModuleV1 {
    backend: ProductionBackendLoweredModuleV1,
    compiler_module: InertCompilerModuleTextV1,
    descriptor: CompilerDescriptorSourceIdentityV1,
}

impl ProductionBackendDescriptorModuleV1 {
    pub(crate) fn compiler_module(&self) -> &InertCompilerModuleTextV1 {
        &self.compiler_module
    }

    pub(crate) fn bind_worker_input(
        self,
        handoff: &CompilerModuleHandoffV2,
        descriptor: &CompilerDescriptorSourceV1,
    ) -> Result<ProductionBackendWorkerOutputV1, Error> {
        let output = ProductionBackendWorkerOutputV1 {
            module: self,
            handoff: handoff.identity(),
        };
        output.validate_worker_input(handoff, descriptor)?;
        Ok(output)
    }
}

/// Retained until publication, paired with the exact complete worker input.
/// It is not a serialized proof, a worker receipt, or machine-effect evidence.
pub(crate) struct ProductionBackendWorkerOutputV1 {
    module: ProductionBackendDescriptorModuleV1,
    handoff: CompilerModuleHandoffIdentityV2,
}

impl ProductionBackendWorkerOutputV1 {
    pub(crate) fn validate_worker_input(
        &self,
        handoff: &CompilerModuleHandoffV2,
        descriptor: &CompilerDescriptorSourceV1,
    ) -> Result<(), Error> {
        let backend = &self.module.backend;
        if handoff.kind() != CompilerModuleKindV1::LlvmTextIr
            || handoff.target().to_string() != backend.target.canonical_target()
            || u16::from(handoff.code_object_version().number())
                != backend.target.code_object_version()
            || handoff.identity() != self.handoff
            || handoff.module_bytes() != self.module.compiler_module.llvm_ir().as_bytes()
            || self.module.compiler_module.descriptor_source_identity()
                != Some(self.module.descriptor)
            || descriptor.identity() != self.module.descriptor
            || !self.module.descriptor.matches(descriptor.canonical_bytes())
        {
            return Err(Error::TargetOutput(
                "worker input, layout, target or descriptor changed",
            ));
        }
        backend.validate_translation()
    }

    /// Existing V3/V5/V8 artifact records have no V14 operational-proof tag.
    pub(crate) fn require_frozen_v13_artifact(&self) -> Result<(), Error> {
        match self.module.backend.canonical.version() {
            CanonicalKernelIrVersionV1::V13 => Ok(()),
            CanonicalKernelIrVersionV1::V14 => Err(Error::TargetOutput(
                "V14 artifact lineage and machine-proof consumption are not implemented",
            )),
        }
    }
}

#[cfg(test)]
#[path = "target_output/tests.rs"]
mod tests;
