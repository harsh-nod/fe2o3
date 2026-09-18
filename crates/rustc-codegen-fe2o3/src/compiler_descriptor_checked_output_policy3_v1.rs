//! Inert descriptor construction from admitted, exact Policy3 output.

use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
use fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy3V1;

/// A consumer for the owner's closed checked-output admission subset. The executable and
/// fresh obligations come only from `admitted`; there is no free Module input.
/// This remains inert descriptor input, not legacy lineage or artifact custody.
/// Descriptor encoding and the inherited formal engine are not canonical-ledger
/// metered. Only the owner revalidation and exact N/B target check use `budget`.
#[allow(dead_code)] // Default activation waits for the versioned V12 lineage contract.
pub(crate) fn construct_checked_output_policy3_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    admitted: &ProductionCheckedOutputOwnerPolicy3V1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<CompilerDescriptorSourceV1, CompilerDescriptorError> {
    admitted
        .verify_equivalence(budget)
        .map_err(CompilerDescriptorError::CheckedOutputPolicy3)?;
    let target = envelope.target().to_string();
    let profile = ProductionAmdTargetProfileV1::from_device_target(&target)
        .ok_or_else(|| CompilerDescriptorError::UnsupportedTarget(target.clone()))?;
    let neutral = admitted
        .source_semantic_kir()
        .pre_ranked_executable()
        .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
            "connected historical source",
        ))?;
    // The admission's generic coordinate check permits capability extensions.
    // Descriptor target selection additionally requires the exact binder delta.
    let _ = dialect_amdgcn::check_production_target_coordinate_preservation_v1(
        neutral,
        admitted.bound(),
        profile,
        budget,
    )
    .map_err(CompilerDescriptorError::CheckedOutputTarget)?;
    let geometries =
        validate_checked_output_policy3_descriptor_evidence_v1(typed_roots, admitted, &target)?;
    let producer_version = match profile {
        ProductionAmdTargetProfileV1::Gfx942 => "production-policy3-checked-gfx942-cov6-v1",
        ProductionAmdTargetProfileV1::Gfx950 => "production-policy3-checked-gfx950-cov6-v1",
    };
    let profiles = geometries
        .into_iter()
        .map(|geometry| DescriptorConstructionProfileV1 {
            rank: geometry.rank(),
            workgroup: geometry.workgroup(),
            max_grid: geometry.max_grid(),
            max_flat_workgroup_size: geometry.max_flat_workgroup_size(),
            static_shared_memory_bytes: geometry.static_shared_memory_bytes(),
            allow_exact_tiled_matrix: geometry.allow_exact_tiled_matrix(),
            allow_workgroup_memory: geometry.allow_workgroup_memory(),
            producer_version,
        })
        .collect::<Vec<_>>();
    construct_compiler_descriptor_source_with_profiles_v1(
        envelope,
        admitted.output().module(),
        compiler_module,
        typed_roots,
        &profiles,
    )?
    .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
        "complete checked-output descriptor closure",
    ))
}

fn validate_checked_output_policy3_descriptor_evidence_v1(
    typed_roots: &[TypedDescriptorRootV1],
    admitted: &ProductionCheckedOutputOwnerPolicy3V1,
    target: &str,
) -> Result<Vec<crate::production_geometry_v1::ProductionGeometryV1>, CompilerDescriptorError> {
    let module = admitted.output().module();
    let semantic = admitted.source_semantic_kir().semantic().semantic();
    let source_launch = admitted
        .source_semantic_kir()
        .source_launch_roster()
        .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
            "retained source launch roster",
        ))?;
    if typed_roots.is_empty()
        || typed_roots.len() != semantic.roots().len()
        || typed_roots.len() != module.kernels.len()
        || typed_roots.len() != admitted.kernels().len()
        || typed_roots.len() != source_launch.roots().len()
    {
        return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
            "complete ordered typed/source/output/formal root roster",
        ));
    }
    let mut geometries = Vec::with_capacity(typed_roots.len());
    for ((((root, semantic_root), kernel), obligations), launch_root) in typed_roots
        .iter()
        .zip(semantic.roots())
        .zip(&module.kernels)
        .zip(admitted.kernels())
        .zip(source_launch.roots())
    {
        let function = semantic
            .functions()
            .get(semantic_root.index() as usize)
            .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
                "semantic root function",
            ))?;
        let entry = function.kernel_entry().ok_or(
            CompilerDescriptorError::ProductionDescriptorMismatch("semantic root entry"),
        )?;
        if entry.kernel_binding_identity().as_bytes() != &root.kernel_binding_bytes()
            || std::str::from_utf8(entry.export_symbol().as_bytes()).ok()
                != Some(root.entry_symbol())
            || kernel.id.as_str() != root.entry_symbol()
            || obligations.kernel() != &kernel.id
            || launch_root.selected_root() != *semantic_root
            || launch_root.semantic_root_identity() != function.identity()
            || launch_root.kernel_binding() != root.kernel_binding_bytes()
        {
            return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                "ordered typed/source/output/formal root identity",
            ));
        }
        validate_production_v1_semantic_root_ownership_evidence(root, semantic, function)?;
        let launch =
            root.source_launch()
                .ok_or(CompilerDescriptorError::ProductionDescriptorMismatch(
                    "authenticated source launch",
                ))?;
        let exact_workgroup = match launch.block_size() {
            fe2o3_artifacts::BlockSize::Exact(dimensions) => {
                Some([dimensions.x(), dimensions.y(), dimensions.z()])
            }
            _ => None,
        };
        let grid = launch.max_grid();
        let input = fe2o3_lower_mir_kernel::ProductionSourceLaunchInputV1::new(
            launch.rank(),
            exact_workgroup,
            [grid.x(), grid.y(), grid.z()],
        );
        if input != launch_root.source_launch() {
            return Err(CompilerDescriptorError::ProductionDescriptorMismatch(
                "exact retained source launch fields",
            ));
        }
        // Fresh O obligations are retained by the admitted owner. The same
        // per-root validator checks Global allocation/ownership, runtime bounds,
        // aliases and source launch; no N facts replace O facts here.
        geometries.push(validate_production_v1_descriptor_root_evidence(
            module,
            root,
            semantic,
            function,
            kernel,
            obligations,
            target,
        )?);
    }
    Ok(geometries)
}

#[cfg(test)]
#[path = "compiler_descriptor_checked_output_policy3_v1_tests.rs"]
mod tests;
