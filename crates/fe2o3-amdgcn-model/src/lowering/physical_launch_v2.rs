use super::*;

#[cfg(test)]
#[path = "physical_launch_v2/tests.rs"]
mod tests;

/// Complete borrowed physical-launch input for one exact target module.
///
/// The bounds are a conditional caller contract, not authenticated host dispatch
/// authority. The production backend must derive every row from its retained
/// source and typed descriptor before calling the lowering entry. Logical
/// Static extents are never substituted for these physical maxima.
pub struct CompilerPhysicalLaunchV2<'a> {
    module: &'a Module,
    target: LoweringTarget,
    entries: Vec<(
        &'a Kernel,
        Option<fe2o3_kernel_analysis::UniformityPhysicalLaunchV2<'a>>,
    )>,
}

pub(super) fn invalid(module: &Module) -> LoweringErrors {
    LoweringErrors::one(
        LoweringLocation::module(module),
        LoweringDiagnosticCode::InvalidLaunchPolicy,
        "physical convergence context requires the complete exact module/kernel/target roster",
    )
}

impl<'a> CompilerPhysicalLaunchV2<'a> {
    /// Binds every supplied kernel reference in original module order. Higher
    /// ranks retain the legacy proof; helpers receive no ambient kernel context.
    pub fn new(
        module: &'a Module,
        target: fe2o3_amd_target::ProductionAmdTargetProfileV1,
        bounds: &[(&'a Kernel, [u32; 3])],
    ) -> Result<Self, LoweringErrors> {
        let target = match target {
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942 => {
                LoweringTarget::Gfx942XnackMinusV1
            }
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950 => {
                LoweringTarget::Gfx950XnackMinusV1
            }
        };
        if bounds.is_empty()
            || bounds.len() != module.kernels.len()
            || bounds.len() > MAX_COMPILER_MODULE_GRAPH_KERNELS
        {
            return Err(invalid(module));
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(bounds.len())
            .map_err(|_| invalid(module))?;
        for (actual, (kernel, max_grid)) in module.kernels.iter().zip(bounds) {
            if !std::ptr::eq(actual, *kernel) || max_grid.contains(&0) {
                return Err(invalid(module));
            }
            let physical = if matches!(kernel.domain, LaunchDomain::D1 { .. }) {
                Some(
                    fe2o3_kernel_analysis::UniformityPhysicalLaunchV2::new(
                        module,
                        kernel,
                        *max_grid,
                        production_logical_index_width_v19(),
                    )
                    .map_err(|_| invalid(module))?,
                )
            } else {
                None
            };
            entries.push((*kernel, physical));
        }
        Ok(Self {
            module,
            target,
            entries,
        })
    }

    pub(super) fn check(
        &self,
        module: &Module,
        target: LoweringTarget,
    ) -> Result<(), LoweringErrors> {
        if !std::ptr::eq(self.module, module)
            || self.target != target
            || self.entries.len() != module.kernels.len()
            || !self
                .entries
                .iter()
                .zip(&module.kernels)
                .all(|((entry, _), actual)| std::ptr::eq(*entry, actual))
        {
            return Err(invalid(module));
        }
        Ok(())
    }

    pub(super) fn entry(
        &self,
        kernel: &Kernel,
    ) -> Result<Option<&fe2o3_kernel_analysis::UniformityPhysicalLaunchV2<'a>>, LoweringErrors>
    {
        self.entries
            .iter()
            .find(|(actual, _)| std::ptr::eq(*actual, kernel))
            .map(|(_, physical)| physical.as_ref())
            .ok_or_else(|| invalid(self.module))
    }
}

/// Runs the unchanged final target preflight with exact borrowed physical
/// context available only to its corresponding kernel convergence analysis.
/// This conditional emitter grants no launch or publication authority.
pub fn lower_compiler_module_to_xnack_minus_llvm_ir_with_physical_launch_v2(
    module: &Module,
    identity: ProductionSemanticAnchorKirIdentityV1,
    physical: &CompilerPhysicalLaunchV2<'_>,
) -> Result<String, LoweringErrors> {
    lower_compiler_module_with_physical_context_v2(
        module,
        physical.target,
        None,
        Some(SemanticAnchorInputV1::Historical(identity)),
        true,
        None,
        Some(physical),
    )
}
