use super::*;

#[cfg(test)]
#[path = "physical_target_context_v2/tests.rs"]
mod tests;

// Only lower_production_target supplies these rows, immediately after the
// genuine source/root/typed-descriptor join and full target geometry check.
// This does not accept ambient launch overrides or infer bounds from Static.
pub(super) fn bind<'a>(
    source: &fe2o3_kernel_ir::Module,
    target: &'a fe2o3_kernel_ir::Module,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    geometry: &[crate::production_geometry_v1::ProductionGeometryV1],
) -> Result<dialect_amdgcn::CompilerPhysicalLaunchV2<'a>, ProductionPipelineError> {
    let mismatch = || {
        ProductionPipelineError::Geometry(
            crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
        )
    };
    if source.kernels.is_empty()
        || source.kernels.len() != target.kernels.len()
        || source.kernels.len() != geometry.len()
    {
        return Err(mismatch());
    }
    let mut bounds = Vec::with_capacity(geometry.len());
    for ((original, optimized), geometry) in
        source.kernels.iter().zip(&target.kernels).zip(geometry)
    {
        let rank = match optimized.domain {
            fe2o3_kernel_ir::LaunchDomain::D1 { .. } => 1,
            fe2o3_kernel_ir::LaunchDomain::D2 { .. } => 2,
            fe2o3_kernel_ir::LaunchDomain::D3 { .. } => 3,
        };
        let workgroup = optimized.workgroup_size.ok_or_else(mismatch)?;
        if original.id != optimized.id
            || original.entry != optimized.entry
            || original.domain != optimized.domain
            || original.workgroup_size != optimized.workgroup_size
            || geometry.rank() != rank
            || geometry.workgroup() != [workgroup.x, workgroup.y, workgroup.z]
        {
            return Err(mismatch());
        }
        bounds.push((optimized, geometry.max_grid()));
    }
    dialect_amdgcn::CompilerPhysicalLaunchV2::new(target, profile, &bounds)
        .map_err(ProductionPipelineError::TargetLowering)
}
