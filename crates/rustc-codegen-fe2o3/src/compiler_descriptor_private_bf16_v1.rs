//! Private nominal BF16 descriptor construction components, not admission.
//! Only the owning continuation may join these inert bytes to actual LLVM/O.
use super::*;

pub(crate) const PRIVATE_BF16_DESCRIPTOR_PRODUCER_V1: &str = "private-bf16-actual-o-gfx942-cov6-v1";

/// The caller freshly proves actual source/Return/B -> O and actual-O guards.
/// This component neither accepts a Complete relabel nor constructs an owner.
#[allow(clippy::too_many_arguments)]
pub(crate) fn construct_private_bf16_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    source_launch: &fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1,
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    obligations: &fe2o3_kernel_ir::FormalMemoryObligations,
) -> Result<CompilerDescriptorSourceV1, CompilerDescriptorError> {
    let mismatch = CompilerDescriptorError::ProductionDescriptorMismatch;
    if envelope.target().to_string() != "gfx942:xnack-"
        || envelope.code_object_version() != CodeObjectVersion::V6
        || envelope.directional_symbols().total_count() != 0
    {
        return Err(mismatch("private BF16 exact target/empty FFI envelope"));
    }
    let ([root], [semantic_root], [launch_root], [kernel]) = (
        typed_roots,
        semantic.roots(),
        source_launch.roots(),
        output.module().kernels.as_slice(),
    ) else {
        return Err(mismatch(
            "private BF16 complete singleton descriptor roster",
        ));
    };
    let function = semantic
        .functions()
        .get(semantic_root.index() as usize)
        .ok_or(mismatch("private BF16 semantic root function"))?;
    let entry = function
        .kernel_entry()
        .ok_or(mismatch("private BF16 semantic root entry"))?;
    if entry.kernel_binding_identity().as_bytes() != &root.kernel_binding_bytes()
        || std::str::from_utf8(entry.export_symbol().as_bytes()).ok() != Some(root.entry_symbol())
        || kernel.id.as_str() != root.entry_symbol()
        || obligations.kernel() != &kernel.id
        || obligations.entry() != &kernel.entry
        || launch_root.selected_root() != *semantic_root
        || launch_root.semantic_root_identity() != function.identity()
        || launch_root.kernel_binding() != root.kernel_binding_bytes()
    {
        return Err(mismatch(
            "private BF16 ordered source/output/formal identity",
        ));
    }
    validate_production_v1_semantic_root_ownership_evidence(root, semantic, function)?;
    let launch = root
        .source_launch()
        .ok_or(mismatch("private BF16 authenticated source launch"))?;
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
        return Err(mismatch("private BF16 exact retained source launch"));
    }
    // Actual-O allocation/access/alias rows feed the unchanged ownership/ABI
    // validator. Runtime bounds and alias requirements are NOT deleted or
    // represented as checked host allocations by this V1 descriptor.
    let geometry = validate_production_v1_descriptor_root_evidence(
        output.module(),
        root,
        semantic,
        function,
        kernel,
        obligations,
        "gfx942:xnack-",
    )?;
    let profile = DescriptorConstructionProfileV1 {
        rank: geometry.rank(),
        workgroup: geometry.workgroup(),
        max_grid: geometry.max_grid(),
        max_flat_workgroup_size: geometry.max_flat_workgroup_size(),
        static_shared_memory_bytes: geometry.static_shared_memory_bytes(),
        allow_exact_tiled_matrix: geometry.allow_exact_tiled_matrix(),
        allow_workgroup_memory: geometry.allow_workgroup_memory(),
        producer_version: PRIVATE_BF16_DESCRIPTOR_PRODUCER_V1,
    };
    // No new capability waiver: all capabilities use the unchanged common
    // projection. This is data construction, not ordinary formal admission.
    construct_compiler_descriptor_source_with_profiles_v1(
        envelope,
        output.module(),
        compiler_module,
        typed_roots,
        &[profile],
    )?
    .ok_or(mismatch("private BF16 nonempty descriptor source"))
}
