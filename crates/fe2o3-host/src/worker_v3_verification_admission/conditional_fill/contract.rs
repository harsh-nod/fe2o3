use super::WorkerV3ConditionalFillAssociationErrorV1 as Error;
use crate::GeneratedDeviceScalarV1;
use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize,
    Dimensions, LaunchContract, Mutability, Name, PointerWidth, RustDisjointIndexSpaceV1,
    derive_generated_host_contract_identity_v1,
};
use fe2o3_compiler_ffi::CompilerDescriptorSourceV1;
use fe2o3_kernel_descriptor::{
    AccessMode, BlockSizeV1, CapabilityV1, CodeObjectVersion, DeviceLayoutDescriptorV1,
    DeviceLayoutRecordV1, DeviceTargetV1, DimensionsV1, KernelAbiLayoutV1, KernelDescriptorV1,
    LogicalArgumentV1, ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeRecordV1, ValidName,
};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticBackendPrimitiveV1, SemanticBackendReprV1,
    SemanticKernelEntryV1, SemanticScalarTypeV1, SemanticSourceArgumentOwnershipV1,
    SemanticTypeLayoutV1, SemanticTypeShapeV1,
};
use fe2o3_verifier::CheckedConditionalFillProgramV1;
use reserved_fe2o3_symbols::MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1;

/// Reconstructs the closed fill's host contract without an application marker.
///
/// The descriptor source must be the exact ABI committed by the checked program's
/// target lineage. The whole-program checker establishes the u32/Index1D write
/// semantics; this adds exact descriptor, Rust layout and launch/resource checks.
/// The returned digest is inert. It authenticates neither compiler provenance nor
/// proof custody, publication currentness, executable loading or an invocation.
pub fn derive_worker_v3_conditional_fill_host_contract_v1(
    program: &CheckedConditionalFillProgramV1<'_>,
    source: &CompilerDescriptorSourceV1,
) -> Result<[u8; 32], Error> {
    let association = program
        .lineage()
        .semantic_to_llvm()
        .inputs()
        .map_err(|_| Error::CompilerInputs("target lineage"))?;
    check_source_identity(source, association.abi.sha256(), association.abi.byte_len())?;
    let target = program
        .lineage()
        .target_binding()
        .inputs()
        .map_err(|_| Error::CompilerInputs("target lineage"))?;
    let expected = fe2o3_amd_target::PRODUCTION_GFX942_DEVICE_TARGET_V1;
    if target.configured_target != expected
        || target.wave_width_bits != 64
        || source.table().device_target()
            != DeviceTargetV1::parse(expected).expect("fixed fill target")
        || source.table().code_object_version() != CodeObjectVersion::V6
    {
        return Err(Error::CompilerInputs("descriptor target"));
    }
    let [descriptor] = source.table().kernels() else {
        return Err(Error::CompilerInputs("singleton descriptor"));
    };
    let semantic = program.inputs().semantic_mir();
    let [root] = semantic.roots() else {
        return Err(Error::CompilerInputs("singleton semantic root"));
    };
    let selection = semantic
        .select_kernel_body_for_root_v1(*root)
        .ok_or(Error::CompilerInputs("semantic body selection"))?;
    let entry = semantic.functions()[root.index() as usize]
        .kernel_entry()
        .ok_or(Error::CompilerInputs("semantic entry"))?;
    let body = &semantic.functions()[selection.body().index() as usize];
    let [output] = body.abi().source_input_types() else {
        return Err(Error::CompilerInputs("semantic argument count"));
    };
    if body.abi().source_argument_ownership() != [SemanticSourceArgumentOwnershipV1::ExclusiveOwner]
    {
        return Err(Error::CompilerInputs("semantic ownership"));
    }
    check_output_type(semantic, &semantic.types()[output.index() as usize])?;
    derive_contract(descriptor, entry)
}

fn check_source_identity(
    source: &CompilerDescriptorSourceV1,
    expected_sha256: [u8; 32],
    expected_length: u64,
) -> Result<(), Error> {
    // Lineage commits the domain-separated ABI receipt, not the plain source hash.
    let abi = fe2o3_compiler_lineage::InertAbiReceiptV3::from_canonical_preimage(
        source.canonical_bytes(),
    )
    .map_err(|_| Error::CompilerInputs("descriptor receipt"))?
    .identity();
    if *abi.sha256() != expected_sha256 || abi.byte_len() != expected_length {
        return Err(Error::CompilerInputs("original descriptor source"));
    }
    Ok(())
}

fn derive_contract(
    descriptor: &KernelDescriptorV1,
    entry: &SemanticKernelEntryV1,
) -> Result<[u8; 32], Error> {
    if entry.kernel_binding_identity().as_bytes() != descriptor.kernel_id().as_bytes()
        || entry.export_symbol().as_bytes() != descriptor.entry_name().as_str().as_bytes()
    {
        return Err(Error::CompilerInputs("semantic entry binding"));
    }
    let expected = LogicalArgumentV1::disjoint_slice(
        0,
        ValidName::new("arg0").expect("fixed argument name"),
        &SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::U32)),
        &DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::U32)),
        AccessMode::WriteOnly,
        0,
    )
    .expect("fixed fill descriptor argument");
    if descriptor.arguments() != [expected]
        || descriptor.abi_layout()
            != KernelAbiLayoutV1::new(16, 272, 8).expect("fixed fill kernarg layout")
    {
        return Err(Error::CompilerInputs("descriptor ABI"));
    }
    let declared = descriptor.launch();
    let block = DimensionsV1::new(64, 1, 1).expect("fixed block");
    let source = entry.source_contract();
    let launch = source
        .launch()
        .ok_or(Error::CompilerInputs("explicit source launch"))?;
    if launch.required().map(|value| value.as_array()) != Some([64, 1, 1])
        || launch
            .maximum()
            .is_some_and(|value| value.as_array() != [64, 1, 1])
        || launch.min_workgroups_per_compute_unit().is_some()
        || declared.rank() != 1
        || declared.block_size() != BlockSizeV1::Exact(block)
        || declared.max_flat_workgroup_size() != 64
        || declared.max_grid().y() != 1
        || declared.max_grid().z() != 1
    {
        return Err(Error::CompilerInputs("launch geometry"));
    }
    if descriptor.capabilities() != [CapabilityV1::AmdWave]
        || declared.static_shared_memory_bytes() != 0
        || declared.max_dynamic_shared_memory_bytes() != 0
        || source.resources().is_some()
        || source.unsafe_assembly().is_some()
        || source.reachable_assembly().is_some()
    {
        return Err(Error::CompilerInputs("unsupported resources"));
    }
    let abi = AbiLayout::new(
        16,
        8,
        PointerWidth::Bits64,
        vec![
            AbiField::new(
                Name::new("arg0").expect("fixed host argument name"),
                0,
                16,
                8,
                AbiKind::Slice {
                    element_size: 4,
                    element_alignment: 4,
                },
                Mutability::Mutable,
                Access::WriteOnly,
                AddressSpace::Global,
                u32::disjoint_slice_type_identity_for_index_space_v1(
                    PointerWidth::Bits64,
                    RustDisjointIndexSpaceV1::Index1D,
                ),
                ArgumentOwnership::UniqueBorrow,
                AliasClass::Exclusive,
            )
            .expect("fixed fill host field"),
        ],
    )
    .expect("fixed fill host ABI");
    let launch = LaunchContract::new(
        1,
        BlockSize::Exact(Dimensions::new(64, 1, 1).expect("fixed block")),
        Dimensions::new(declared.max_grid().x(), 1, 1).expect("validated descriptor grid"),
        0,
        0,
    )
    .expect("checked fill launch");
    Ok(*derive_generated_host_contract_identity_v1(
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        *descriptor.kernel_id().as_bytes(),
        descriptor.logical_name().as_str(),
        descriptor.entry_name().as_str(),
        &abi,
        &launch,
    )
    .as_bytes())
}

fn check_output_type(
    semantic: &AdmittedInertSemanticMirV1,
    output: &fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1,
) -> Result<(), Error> {
    check_slice_layout(output.layout())?;
    // The admitted write intrinsic fixes the aggregate's field offsets, mutable
    // pointer, u32 element and Index1D branding. Check the target ABI alignments too.
    let SemanticTypeShapeV1::Aggregate(fields) = output.shape() else {
        return Err(Error::CompilerInputs("semantic slice type"));
    };
    let element = fields
        .fields()
        .iter()
        .find_map(
            |field| match semantic.types()[field.index() as usize].shape() {
                SemanticTypeShapeV1::Pointer(pointer) => {
                    Some(&semantic.types()[pointer.pointee().index() as usize])
                }
                _ => None,
            },
        )
        .ok_or(Error::CompilerInputs("semantic element type"))?;
    if !matches!(
        element.shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32
        })
    ) {
        return Err(Error::CompilerInputs("semantic element type"));
    }
    check_element_layout(element.layout())
}

fn check_element_layout(layout: &SemanticTypeLayoutV1) -> Result<(), Error> {
    if layout.size_bytes() != Some(4)
        || layout.rustc_size_bytes() != 4
        || layout.alignment_bytes() != 4
        || !matches!(layout.backend_repr(), SemanticBackendReprV1::Scalar(scalar)
            if scalar.primitive() == SemanticBackendPrimitiveV1::integer(false, 32, 4))
    {
        return Err(Error::CompilerInputs("semantic element layout"));
    }
    Ok(())
}

fn check_slice_layout(layout: &SemanticTypeLayoutV1) -> Result<(), Error> {
    if layout.size_bytes() != Some(16)
        || layout.rustc_size_bytes() != 16
        || layout.alignment_bytes() != 8
        || !matches!(layout.backend_repr(), SemanticBackendReprV1::ScalarPair { first, second }
            if first.primitive() == SemanticBackendPrimitiveV1::pointer(0, 8, 8)
                && second.primitive() == SemanticBackendPrimitiveV1::integer(false, 64, 8))
    {
        return Err(Error::CompilerInputs("semantic slice layout"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
