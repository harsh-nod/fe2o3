//! Retains the actual fixed-policy target optimizer output through descriptor construction.
//! No raw Module, caller-selected pass list, or report can construct this owner.
//! Existing canonical-byte/session limits bound the optimizer's allocations;
//! retaining its already-produced canonical owner adds no clone or second replay.
//! This is structural optimizer custody, not a proof of semantic preservation.
use super::ProductionPipelineError;
use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_kernel_ir::Module;
use fe2o3_kernel_opt::{
    KernelIrPlironOptimizationReportV2, OptimizedKernelIrModuleV2, OptimizedKernelIrModuleV3,
};
use fe2o3_lower_mir_kernel::{
    ProductionCanonicalKernelIrIdentityV1, ProductionCanonicalKernelIrVersionV1,
    ProductionFormalMemoryOwnerV1,
};

enum FixedOutputV30 {
    V2(OptimizedKernelIrModuleV2),
    V3(OptimizedKernelIrModuleV3),
}

/// Move-only private compiler custody. The source/formal owner remains alongside
/// this value; admission rejoins its complete versioned canonical identity.
pub(crate) struct RetainedProductionTargetV30 {
    source: ProductionCanonicalKernelIrIdentityV1,
    profile: ProductionAmdTargetProfileV1,
    output: FixedOutputV30,
}

impl RetainedProductionTargetV30 {
    pub(crate) fn try_lower(
        formal: &ProductionFormalMemoryOwnerV1,
        profile: ProductionAmdTargetProfileV1,
    ) -> Result<Self, ProductionPipelineError> {
        let source = formal.semantic_kir().canonical_kernel_ir_identity();
        let bound =
            dialect_amdgcn::bind_production_target_v1(formal.semantic_kir().module(), profile)
                .map_err(ProductionPipelineError::TargetBinding)?;
        let (target_module, kernel_ids) = bound.into_parts();
        let output = match source.version() {
            ProductionCanonicalKernelIrVersionV1::V11 => FixedOutputV30::V3(
                fe2o3_kernel_opt::optimize_production_kernel_ir_module_v3(&target_module)
                    .map_err(ProductionPipelineError::TargetOptimizationV3)?,
            ),
            ProductionCanonicalKernelIrVersionV1::V8 | ProductionCanonicalKernelIrVersionV1::V9 => {
                FixedOutputV30::V2(
                    fe2o3_kernel_opt::optimize_production_kernel_ir_module_v2(&target_module)
                        .map_err(ProductionPipelineError::TargetOptimization)?,
                )
            }
        };
        let retained = Self {
            source,
            profile,
            output,
        };
        if kernel_ids.len() != retained.module().kernels.len()
            || kernel_ids
                .iter()
                .zip(&retained.module().kernels)
                .any(|(id, kernel)| id != &kernel.id)
        {
            return Err(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
            ));
        }
        Ok(retained)
    }

    pub(crate) const fn module(&self) -> &Module {
        match &self.output {
            FixedOutputV30::V2(owner) => owner.module(),
            FixedOutputV30::V3(owner) => owner.module(),
        }
    }
    pub(crate) const fn report(&self) -> &KernelIrPlironOptimizationReportV2 {
        match &self.output {
            FixedOutputV30::V2(owner) => owner.report(),
            FixedOutputV30::V3(owner) => owner.report(),
        }
    }

    /// Identity is only a join inside this privately constructed typed owner.
    /// A matching digest, cloned Module, or caller-written report cannot create it.
    pub(crate) fn joins(
        &self,
        formal: &ProductionFormalMemoryOwnerV1,
        module: &Module,
        device: &str,
    ) -> bool {
        self.source == formal.semantic_kir().canonical_kernel_ir_identity()
            && self.profile.device_target() == device
            && std::ptr::eq(self.module(), module)
            && self.report().is_production_replay_compatible()
    }
}
