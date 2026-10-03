//! Inert discharge of the retained fill condition against actual packed arguments.
//! Compiler origin, machine refinement, device ownership and publication remain separate gates.

use std::{error::Error, fmt};

use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_artifacts::{Access, PointerWidth, RustDisjointIndexSpaceV1};
use fe2o3_compiler_ffi::{
    CompilerDescriptorSourceErrorV1, CompilerDescriptorSourceV1, CompilerModuleSymbolRoleV1,
    InertSemanticCompilerModuleHandoffV3,
};
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, BlockSizeV1, CodeObjectVersion, DeviceDescriptorTableV1,
    DeviceTargetV1, KernelDescriptorV1, OwnershipSemantics, PhysicalAbiComponentKind, ScalarTypeV1,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticLocalRoleV1, SemanticSourceArgumentOwnershipV1};
use fe2o3_runtime::Gfx942RuntimeBufferAccessV1;
use fe2o3_verifier::{
    CompilerTargetLineageValidationErrorV1, InertConditionalOutputObligationV1,
    ValidatedCompilerTargetLineageV1, ValidatedConditionalCompilerProofInputsV1,
    validate_conditional_compiler_target_lineage_v1,
};

use crate::{
    CompilerGeneratedArgumentLayoutV1, GeneratedArgumentPackingError,
    GeneratedArgumentPackingPlanV1, GeneratedDeviceScalarV1, GeneratedKfdPackedArguments,
    GeneratedRuntimeChargedArgumentsV1, GeneratedRuntimePackedArgumentsV1, KernelId,
    generated_argument_plan::validate_worker_v3_argument_packing,
    generated_kfd_arguments::GeneratedPackedArgumentsViewV1,
};

/// Once-per-artifact binding of conditional proof, exact compiler ABI and generated layout.
///
/// This is not a protected executable or an application-refinement permit. The handoff and proof
/// are borrowed, and deterministic target replay does not authenticate their producer.
///
/// ```compile_fail
/// use fe2o3_host::ConditionalOutputArgumentBindingV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ConditionalOutputArgumentBindingV1<'static>>();
/// ```
#[must_use]
pub struct ConditionalOutputArgumentBindingV1<'proof> {
    proof: &'proof ValidatedConditionalCompilerProofInputsV1,
    handoff: &'proof InertSemanticCompilerModuleHandoffV3,
    lineage: ValidatedCompilerTargetLineageV1,
    source: CompilerDescriptorSourceV1,
    plan: GeneratedArgumentPackingPlanV1,
    output: OutputArgumentLayout,
}

impl<'proof> ConditionalOutputArgumentBindingV1<'proof> {
    pub fn from_handoff(
        proof: &'proof ValidatedConditionalCompilerProofInputsV1,
        handoff: &'proof InertSemanticCompilerModuleHandoffV3,
        generated: &CompilerGeneratedArgumentLayoutV1,
    ) -> Result<Self, ConditionalPackedCoverageErrorV1> {
        use ConditionalPackedCoverageErrorV1 as E;
        let lineage = validate_conditional_compiler_target_lineage_v1(handoff.capsule(), proof)
            .map_err(E::TargetLineage)?;
        let receipts = handoff.capsule().receipts();
        let source = CompilerDescriptorSourceV1::decode(receipts.abi().canonical_preimage())
            .map_err(E::DescriptorSource)?;
        let table = source.table();
        if table.kernels().len() != 1
            || table.device_target() != handoff.capsule().target()
            || table.device_target()
                != DeviceTargetV1::parse("gfx942:xnack-").expect("fixed supported target")
            || table.code_object_version() != CodeObjectVersion::V6
            || table.code_object_version() != handoff.module_handoff().code_object_version()
            || receipts.export_manifest().canonical_preimage()
                != handoff.module_handoff().symbol_manifest().canonical_bytes()
        {
            return Err(E::SourceBinding);
        }
        let descriptor = &table.kernels()[0];
        let semantic = proof.semantic_mir();
        let [root] = semantic.roots() else {
            return Err(E::SourceBinding);
        };
        let function = semantic
            .functions()
            .get(root.index() as usize)
            .ok_or(E::SourceBinding)?;
        let entry = function.kernel_entry().ok_or(E::SourceBinding)?;
        let output_index = proof
            .verus_execution()
            .obligation()
            .reference_output_argument() as usize;
        if entry.kernel_binding_identity().as_bytes() != descriptor.kernel_id().as_bytes()
            || entry.export_symbol().as_bytes() != descriptor.entry_name().as_str().as_bytes()
            || function.abi().source_input_types().len() != descriptor.arguments().len()
            || function.abi().source_argument_ownership().get(output_index)
                != Some(&SemanticSourceArgumentOwnershipV1::ExclusiveOwner)
        {
            return Err(E::SourceBinding);
        }
        let kir = proof.kernel_ir().as_v9().ok_or(E::UnsupportedProfile)?;
        let module = fe2o3_kernel_ir::decode_module_v9(kir.canonical_bytes())
            .map_err(|_| E::SourceBinding)?;
        let kir_function = validate_kir_output(&module, descriptor, output_index)?;
        let selection = semantic.select_kernel_body_v1().ok_or(E::SourceBinding)?;
        let body_function = semantic
            .functions()
            .get(selection.body().index() as usize)
            .ok_or(E::SourceBinding)?;
        let (local, _) = body_function
            .locals()
            .iter()
            .enumerate()
            .find(|(_, local)| local.role() == SemanticLocalRoleV1::Argument(output_index as u32))
            .ok_or(E::SourceBinding)?;
        let parameter = proof
            .correspondence()
            .parameter_bindings()
            .iter()
            .find(|binding| {
                binding.semantic_function() == selection.body().index()
                    && binding.semantic_local() as usize == local
            })
            .ok_or(E::SourceBinding)?;
        if kir_function
            .body
            .as_ref()
            .and_then(|body| body.parameters.get(output_index))
            .map(|value| value.0)
            != Some(parameter.kernel_ir_value())
        {
            return Err(E::SourceBinding);
        }
        let manifest = handoff.module_handoff().symbol_manifest();
        for (role, name) in [
            (
                CompilerModuleSymbolRoleV1::KernelEntry,
                descriptor.entry_name().as_str(),
            ),
            (
                CompilerModuleSymbolRoleV1::KernelDescriptor,
                descriptor.descriptor_symbol().as_str(),
            ),
        ] {
            if manifest
                .symbols(role)
                .filter(|symbol| *symbol == name)
                .count()
                != 1
            {
                return Err(E::SourceBinding);
            }
        }
        let (plan, output) = validate_output_layout(
            proof.verus_execution().obligation(),
            table,
            descriptor,
            generated,
        )?;
        Ok(Self {
            proof,
            handoff,
            lineage,
            source,
            plan,
            output,
        })
    }

    pub const fn proof_inputs(&self) -> &ValidatedConditionalCompilerProofInputsV1 {
        self.proof
    }
    pub const fn handoff(&self) -> &InertSemanticCompilerModuleHandoffV3 {
        self.handoff
    }
    pub const fn target_lineage(&self) -> &ValidatedCompilerTargetLineageV1 {
        &self.lineage
    }
    pub const fn descriptor_source(&self) -> &CompilerDescriptorSourceV1 {
        &self.source
    }
    pub const fn packing_plan(&self) -> &GeneratedArgumentPackingPlanV1 {
        &self.plan
    }
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }

    fn check<'a>(
        &'a self,
        packed: GeneratedPackedArgumentsViewV1<'a>,
        geometry: AqlDispatchGeometryV1,
    ) -> Result<CheckedConditionalPackedCoverageV1<'a>, ConditionalPackedCoverageErrorV1> {
        let elements = check_packed_coverage(
            self.proof.verus_execution().obligation(),
            &self.source.table().kernels()[0],
            &self.plan,
            self.output,
            &packed,
            geometry,
        )?;
        Ok(CheckedConditionalPackedCoverageV1 {
            binding: self,
            packed,
            geometry,
            elements,
        })
    }
}

/// Move-only check retaining the actual packed storage borrow and its conditional proof binding.
///
/// No pointer has been patched, no device selected, and no prepared dispatch or completion is
/// authenticated. This value cannot enter the existing protected Worker admission constructor.
///
/// ```compile_fail
/// use fe2o3_host::CheckedConditionalPackedCoverageV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<CheckedConditionalPackedCoverageV1<'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_host::{CheckedConditionalPackedCoverageV1, ConditionalOutputArgumentBindingV1,
///     GeneratedKfdPackedArguments};
/// fn detach<'a>(binding: &'a ConditionalOutputArgumentBindingV1<'a>,
///     packed: GeneratedKfdPackedArguments<'a>, geometry: fe2o3_aql::AqlDispatchGeometryV1)
///     -> CheckedConditionalPackedCoverageV1<'a> {
///     packed.check_conditional_coverage(binding, geometry).unwrap()
/// }
/// ```
#[must_use]
pub struct CheckedConditionalPackedCoverageV1<'a> {
    binding: &'a ConditionalOutputArgumentBindingV1<'a>,
    packed: GeneratedPackedArgumentsViewV1<'a>,
    geometry: AqlDispatchGeometryV1,
    elements: u64,
}

impl CheckedConditionalPackedCoverageV1<'_> {
    pub const fn output_elements(&self) -> u64 {
        self.elements
    }
    pub const fn geometry(&self) -> AqlDispatchGeometryV1 {
        self.geometry
    }
    pub const fn kernel_id(&self) -> KernelId {
        self.packed.kernel_id
    }
    /// Existing physical V1 observation; typed ABI identity is retained separately, not hashed here.
    pub fn packing_identity(&self) -> &[u8; 32] {
        self.packed.observation.identity()
    }
    pub fn binding(&self) -> &ConditionalOutputArgumentBindingV1<'_> {
        self.binding
    }
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
    pub const fn authenticates_device_storage(&self) -> bool {
        false
    }
}

macro_rules! coverage_method {
    () => {
        /// Checks the conditional extent against these exact borrowed packed inputs and geometry.
        /// Success grants no load, dispatch, publication or completion authority.
        pub fn check_conditional_coverage<'a>(
            &'a self,
            binding: &'a ConditionalOutputArgumentBindingV1<'_>,
            geometry: AqlDispatchGeometryV1,
        ) -> Result<CheckedConditionalPackedCoverageV1<'a>, ConditionalPackedCoverageErrorV1> {
            binding.check(self.packed_view_v1(), geometry)
        }
    };
}
impl GeneratedKfdPackedArguments<'_> {
    coverage_method!();
}
impl GeneratedRuntimePackedArgumentsV1 {
    coverage_method!();
}
impl GeneratedRuntimeChargedArgumentsV1 {
    coverage_method!();
}

#[derive(Clone, Copy)]
struct OutputArgumentLayout {
    argument: usize,
    pointer_offset: usize,
    length_offset: usize,
}

fn validate_kir_output<'a>(
    module: &'a fe2o3_kernel_ir::Module,
    descriptor: &KernelDescriptorV1,
    output_index: usize,
) -> Result<&'a fe2o3_kernel_ir::Function, ConditionalPackedCoverageErrorV1> {
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, FunctionRole, ScalarType, Type, WorkgroupSize,
    };
    let error = || ConditionalPackedCoverageErrorV1::SourceBinding;
    let [kernel] = module.kernels.as_slice() else {
        return Err(error());
    };
    let function = module
        .functions
        .iter()
        .find(|function| function.id == kernel.entry)
        .ok_or_else(error)?;
    if kernel.entry.as_str() != descriptor.entry_name().as_str()
        || kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
        || function.role != FunctionRole::KernelEntry
        || function.signature.parameters.len() != descriptor.arguments().len()
        || function.signature.parameters.get(output_index)
            != Some(&Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::WriteOnly,
            ))
    {
        return Err(error());
    }
    Ok(function)
}

fn validate_output_layout(
    condition: &InertConditionalOutputObligationV1,
    table: &DeviceDescriptorTableV1,
    descriptor: &KernelDescriptorV1,
    generated: &CompilerGeneratedArgumentLayoutV1,
) -> Result<(GeneratedArgumentPackingPlanV1, OutputArgumentLayout), ConditionalPackedCoverageErrorV1>
{
    use ConditionalPackedCoverageErrorV1 as E;
    let argument = condition.reference_output_argument() as usize;
    // These ordinal/alias rules belong to the current compiler's guarded-output projection.
    if condition.ranked_extent_argument() != 0
        || condition.allocation_origin() != u64::from(condition.reference_output_argument()) + 1
        || condition.noalias_class()
            != condition
                .allocation_origin()
                .checked_add(1)
                .ok_or(E::UnsupportedProfile)?
        || condition.element_width_bits() != 32
        || condition.workgroup_extents() != [64, 1, 1]
        || condition.subgroup_size() != 64
    {
        return Err(E::UnsupportedProfile);
    }
    let output = descriptor
        .arguments()
        .get(argument)
        .ok_or(E::SourceBinding)?;
    let source_type = table
        .type_records()
        .iter()
        .find(|ty| ty.identity() == output.source_type())
        .ok_or(E::SourceBinding)?
        .descriptor();
    if usize::from(output.source_index()) != argument
        || output.access() != AccessMode::WriteOnly
        || output.ownership() != OwnershipSemantics::UniqueBorrow
        || output.alias() != AliasSemantics::Exclusive
        || !source_type.is_disjoint_slice()
        || source_type.scalar_type() != ScalarTypeV1::U32
    {
        return Err(E::UnsupportedProfile);
    }
    let plan = validate_worker_v3_argument_packing(table, descriptor, generated)
        .map_err(E::PackingPlan)?;
    let field = plan.argument(argument).ok_or(E::SourceBinding)?;
    if field.access() != Access::WriteOnly
        || field.type_identity()
            != u32::disjoint_slice_type_identity_for_index_space_v1(
                PointerWidth::Bits64,
                RustDisjointIndexSpaceV1::Index1D,
            )
    {
        return Err(E::UnsupportedProfile);
    }
    let mut components = output.physical_components();
    let pointer_offset = match components.next() {
        Some((PhysicalAbiComponentKind::GlobalPointer, offset, 8, 8)) => offset as usize,
        _ => return Err(E::UnsupportedProfile),
    };
    let length_offset = match components.next() {
        Some((PhysicalAbiComponentKind::SliceLengthU64, offset, 8, 8)) => offset as usize,
        _ => return Err(E::UnsupportedProfile),
    };
    if components.next().is_some() {
        return Err(E::UnsupportedProfile);
    }
    Ok((
        plan,
        OutputArgumentLayout {
            argument,
            pointer_offset,
            length_offset,
        },
    ))
}

fn check_packed_coverage(
    condition: &InertConditionalOutputObligationV1,
    descriptor: &KernelDescriptorV1,
    plan: &GeneratedArgumentPackingPlanV1,
    output: OutputArgumentLayout,
    packed: &GeneratedPackedArgumentsViewV1<'_>,
    geometry: AqlDispatchGeometryV1,
) -> Result<u64, ConditionalPackedCoverageErrorV1> {
    use ConditionalPackedCoverageErrorV1 as E;
    let observation = packed.observation;
    if packed.kernel_id != plan.kernel_id()
        || packed.alignment != plan.kernarg_alignment()
        || packed.pointer_width != plan.pointer_width()
        || packed.argument_fields.len() != plan.argument_count()
        || packed
            .argument_fields
            .iter()
            .enumerate()
            .any(|(i, field)| Some(field) != plan.argument(i))
        || packed.explicit_kernarg.len() as u64 != plan.kernarg_size()
        || observation.kernarg_alignment() != packed.alignment
        || !observation.matches_explicit_kernarg(packed.explicit_kernarg)
        || observation.components().len() != plan.component_count()
        || observation
            .components()
            .iter()
            .enumerate()
            .any(|(i, component)| Some(*component) != plan.component(i))
    {
        return Err(E::PackedIdentity);
    }
    let read_u64 = |offset: usize| -> Result<u64, E> {
        let bytes = packed
            .explicit_kernarg
            .get(offset..offset.checked_add(8).ok_or(E::PackedIdentity)?)
            .ok_or(E::PackedIdentity)?;
        Ok(u64::from_le_bytes(
            bytes.try_into().map_err(|_| E::PackedIdentity)?,
        ))
    };
    if read_u64(output.pointer_offset)? != 0 {
        return Err(E::OutputBinding);
    }
    let elements = read_u64(output.length_offset)?;
    let byte_len = usize::try_from(elements.checked_mul(4).ok_or(E::OutputExtent)?)
        .map_err(|_| E::OutputExtent)?;
    let mut observations = observation
        .buffers()
        .iter()
        .filter(|b| b.argument_index() == output.argument);
    let buffer = observations.next().ok_or(E::OutputBinding)?;
    if observations.next().is_some() || buffer.initial_bytes() != byte_len {
        return Err(E::OutputBinding);
    }
    // A fixup must not overwrite any byte of the selected pointer or length component.
    let mut fixups = packed.pointer_fixups.iter().filter(|fixup| {
        let start = fixup.kernarg_offset();
        let end = start.saturating_add(8);
        start < output.length_offset + 8 && output.pointer_offset < end
    });
    if elements == 0 {
        if buffer.buffer_index().is_some()
            || buffer.access().is_some()
            || buffer.initial_sha256() != [0; 32]
            || fixups.next().is_some()
        {
            return Err(E::OutputBinding);
        }
    } else {
        let index = buffer.buffer_index().ok_or(E::OutputBinding)?;
        let actual = packed.buffers.get(index).ok_or(E::OutputBinding)?;
        let fixup = fixups.next().ok_or(E::OutputBinding)?;
        if actual.access() != Gfx942RuntimeBufferAccessV1::WriteOnly
            || actual.bytes().len() != byte_len
            || buffer.access() != Some(Gfx942RuntimeBufferAccessV1::WriteOnly)
            || fixup.kernarg_offset() != output.pointer_offset
            || fixup.buffer_index() != index
            || fixup.buffer_byte_offset() != 0
            || fixup.required_alignment() != 4
            || fixups.next().is_some()
            || packed
                .pointer_fixups
                .iter()
                .filter(|f| f.buffer_index() == index)
                .count()
                != 1
            || observation
                .buffers()
                .iter()
                .filter(|b| b.buffer_index() == Some(index))
                .count()
                != 1
        {
            return Err(E::OutputBinding);
        }
    }
    let grid = geometry.grid();
    let workgroup = geometry.workgroup().map(u64::from);
    let launch = descriptor.launch();
    let exact_block = match launch.block_size() {
        BlockSizeV1::Exact(block) => [block.x(), block.y(), block.z()].map(u64::from),
        _ => return Err(E::Geometry),
    };
    let max_grid = launch.max_grid();
    if geometry.dimensions() != 1
        || grid[1..] != [1, 1]
        || workgroup != condition.workgroup_extents()
        || workgroup != exact_block
        || launch.rank() != 1
        || launch.max_flat_workgroup_size() < 64
        || grid
            .iter()
            .zip([max_grid.x(), max_grid.y(), max_grid.z()])
            .any(|(actual, max)| *actual > max)
        || condition
            .static_global_x_extent()
            .is_some_and(|extent| extent != u64::from(grid[0]))
        || (condition.requires_full_physical_workgroups() && u64::from(grid[0]) % workgroup[0] != 0)
    {
        return Err(E::Geometry);
    }
    if elements > u64::from(grid[0]) {
        return Err(E::Underlaunch {
            elements,
            grid_x: grid[0],
        });
    }
    Ok(elements)
}

#[derive(Debug)]
pub enum ConditionalPackedCoverageErrorV1 {
    TargetLineage(CompilerTargetLineageValidationErrorV1),
    DescriptorSource(CompilerDescriptorSourceErrorV1),
    PackingPlan(GeneratedArgumentPackingError),
    SourceBinding,
    UnsupportedProfile,
    PackedIdentity,
    OutputBinding,
    OutputExtent,
    Geometry,
    Underlaunch { elements: u64, grid_x: u32 },
}

impl fmt::Display for ConditionalPackedCoverageErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetLineage(e) => e.fmt(f),
            Self::DescriptorSource(e) => e.fmt(f),
            Self::PackingPlan(e) => e.fmt(f),
            Self::SourceBinding => f.write_str("conditional proof and exact compiler ABI differ"),
            Self::UnsupportedProfile => {
                f.write_str("conditional output is outside the u32 Index1D fill profile")
            }
            Self::PackedIdentity => {
                f.write_str("actual packed arguments differ from the retained packing plan")
            }
            Self::OutputBinding => {
                f.write_str("conditional output buffer or pointer fixup differs")
            }
            Self::OutputExtent => f.write_str("conditional output byte extent overflows"),
            Self::Geometry => {
                f.write_str("launch geometry differs from the conditional output contract")
            }
            Self::Underlaunch { elements, grid_x } => {
                write!(f, "output length {elements} exceeds grid-X {grid_x}")
            }
        }
    }
}
impl Error for ConditionalPackedCoverageErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TargetLineage(e) => Some(e),
            Self::DescriptorSource(e) => Some(e),
            Self::PackingPlan(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "generated_conditional_coverage_tests.rs"]
pub(crate) mod tests;
