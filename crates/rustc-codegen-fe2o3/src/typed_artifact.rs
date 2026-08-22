use crate::AmdGpuTarget;
use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
    ArtifactContainerV1, BlockSize, CodeObjectFormat, CodeObjectIdentity, CodeObjectPayload,
    CompilerIdentity, ContainerValidationError, DigestAlgorithm, DigestBytes, Dimensions,
    Endianness, IdentityText, KernelEntry, LaunchContract, ManifestV1, Mutability, Name,
    PointerWidth, ToolIdentity, TypeIdentity, ValidationError, derive_generated_kernel_identity_v2,
};
use reserved_fe2o3_symbols::{KernelBindingIdV1, TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2};
use std::fmt;

const SOURCE_DIGEST_DOMAIN: &[u8] = b"fe2o3.typed-vecadd.source-llvm-ir.v1\0";
const EXECUTABLE_DIGEST_DOMAIN: &[u8] = b"fe2o3.typed-vecadd.executable-hsaco.v1\0";
const TYPED_VECADD_EXPLICIT_KERNARG_BYTES: u64 = 48;

/// Canonical container and content identity for one finalized typed vecadd.
pub(crate) struct GeneratedTypedArtifactV1 {
    artifact_id: String,
    container: Vec<u8>,
}

impl GeneratedTypedArtifactV1 {
    pub(crate) fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    pub(crate) fn container(&self) -> &[u8] {
        &self.container
    }
}

/// Builds the first exact generated-kernel artifact profile.
///
/// `llvm_ir` is the finalized device IR accepted by the backend and `hsaco`
/// is the exact native payload that will be embedded in the host object. The
/// profile is deliberately limited to `(&[f32], &[f32], DisjointSlice<f32>)`.
pub(crate) fn build_typed_vecadd_artifact_v1(
    logical_name: &str,
    export_name: &str,
    kernel_binding: KernelBindingIdV1,
    type_identities: [TypeIdentity; 3],
    target: &AmdGpuTarget,
    llvm_ir: &[u8],
    hsaco: Vec<u8>,
) -> Result<GeneratedTypedArtifactV1, TypedArtifactError> {
    if llvm_ir.is_empty() {
        return Err(TypedArtifactError::EmptyLlvmIr);
    }

    let algorithm = DigestAlgorithm::Sha256;
    let payload =
        CodeObjectPayload::from_bytes(algorithm, hsaco).map_err(TypedArtifactError::Container)?;
    let code_object_digest = payload.digest().bytes();
    let byte_len =
        u64::try_from(payload.bytes().len()).map_err(|_| TypedArtifactError::LengthOverflow)?;
    let code_object = CodeObjectIdentity::new(
        code_object_digest,
        CodeObjectFormat::NativeExecutable,
        byte_len,
    )
    .map_err(TypedArtifactError::Model)?;

    let source_digest = domain_digest(algorithm, SOURCE_DIGEST_DOMAIN, &[llvm_ir]);
    let executable_digest = domain_digest(
        algorithm,
        EXECUTABLE_DIGEST_DOMAIN,
        &[target.as_str().as_bytes(), payload.bytes()],
    );
    let launch = typed_vecadd_launch()?;
    let abi = typed_vecadd_abi(type_identities)?;
    let kernel_id = derive_generated_kernel_identity_v2(
        TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
        kernel_binding.as_bytes(),
        logical_name,
        export_name,
        source_digest,
        executable_digest,
        &abi,
        &launch,
    );

    let kernel = KernelEntry::new(
        kernel_id,
        name(logical_name)?,
        name(export_name)?,
        source_digest,
        executable_digest,
        code_object_digest,
        vec![],
        launch,
        abi,
    )
    .map_err(TypedArtifactError::Model)?;
    let manifest = ManifestV1::new(
        CompilerIdentity::new(
            text("rustc-codegen-fe2o3")?,
            text(env!("CARGO_PKG_VERSION"))?,
        ),
        ToolIdentity::new(text("fe2o3")?, text(env!("CARGO_PKG_VERSION"))?),
        fe2o3_artifacts::TargetIdentity::new(
            text(dialect_amdgcn::AMDGPU_TRIPLE)?,
            text(target.as_str())?,
            PointerWidth::Bits64,
            Endianness::Little,
            vec![],
        )
        .map_err(TypedArtifactError::Model)?,
        vec![code_object],
        vec![kernel],
    )
    .map_err(TypedArtifactError::Model)?;
    let container = ArtifactContainerV1::new(manifest, algorithm, vec![payload])
        .map_err(TypedArtifactError::Container)?
        .to_bytes();
    let artifact_id = crate::encode_hex(algorithm.calculate(&container).bytes().as_bytes());

    Ok(GeneratedTypedArtifactV1 {
        artifact_id,
        container,
    })
}

/// Validates the finalized executable's physical AMDHSA ABI before it can be
/// embedded behind the typed host adapter.
pub(crate) fn validate_typed_vecadd_hsaco_v2(
    export_name: &str,
    target: &AmdGpuTarget,
    hsaco: &[u8],
) -> Result<(), TypedArtifactError> {
    use fe2o3_hsaco::{
        ArgumentAccess, ArgumentAddressSpace, ExplicitValueKind, KernelKind,
        inspect_and_bind_kernel_descriptors,
    };

    let bound = inspect_and_bind_kernel_descriptors(hsaco).map_err(TypedArtifactError::Hsaco)?;
    let inspection = bound.inspection();
    let kernels = inspection.kernels();
    if inspection.target().to_string() != target.as_str() {
        return Err(TypedArtifactError::PhysicalAbi(format!(
            "HSACO target {} does not match requested target {}",
            inspection.target(),
            target.as_str()
        )));
    }
    if inspection.has_printf_metadata() {
        return Err(TypedArtifactError::PhysicalAbi(
            "typed HSACO must not contain printf metadata".to_owned(),
        ));
    }
    if kernels.len() != 1 || bound.bindings().len() != 1 {
        return Err(TypedArtifactError::PhysicalAbi(format!(
            "typed HSACO must contain exactly one bound kernel, found {} metadata entries and {} bindings",
            kernels.len(),
            bound.bindings().len()
        )));
    }

    let kernel = &kernels[0];
    let expected_symbol = format!("{export_name}.kd");
    if kernel.name() != export_name || kernel.symbol() != expected_symbol {
        return Err(TypedArtifactError::PhysicalAbi(format!(
            "typed HSACO kernel identity is {} / {}, expected {export_name} / {expected_symbol}",
            kernel.name(),
            kernel.symbol()
        )));
    }
    if kernel.kind() != KernelKind::Normal {
        return Err(TypedArtifactError::PhysicalAbi(
            "typed HSACO kernel must be a normal dispatchable entry".to_owned(),
        ));
    }
    if !is_production_typed_vecadd_kernarg_shape(
        kernel.kernarg_segment_size(),
        kernel.kernarg_segment_alignment(),
        kernel.implicit_argument_offset(),
    ) {
        return Err(TypedArtifactError::PhysicalAbi(format!(
            "typed HSACO kernarg segment has size {}, alignment {}, implicit offset {:?}; expected optimized size 48, alignment 8, and no implicit arguments",
            kernel.kernarg_segment_size(),
            kernel.kernarg_segment_alignment(),
            kernel.implicit_argument_offset()
        )));
    }
    if kernel.sgpr_spill_count() != Some(0) || kernel.vgpr_spill_count() != Some(0) {
        return Err(TypedArtifactError::PhysicalAbi(format!(
            "typed HSACO contains register spills: sgpr={:?}, vgpr={:?}; expected both counts to be zero",
            kernel.sgpr_spill_count(),
            kernel.vgpr_spill_count()
        )));
    }
    if kernel.group_segment_fixed_size() != 0 {
        return Err(TypedArtifactError::PhysicalAbi(
            "typed vecadd HSACO unexpectedly requires static workgroup memory".to_owned(),
        ));
    }
    if kernel.uses_dynamic_stack() || kernel.device_enqueue_symbol().is_some() {
        return Err(TypedArtifactError::PhysicalAbi(
            "typed vecadd HSACO uses an unsupported dynamic stack or device enqueue entry"
                .to_owned(),
        ));
    }

    let arguments = kernel.explicit_arguments();
    if arguments.len() != 6 {
        return Err(TypedArtifactError::PhysicalAbi(format!(
            "typed vecadd HSACO must expose six physical arguments, found {}",
            arguments.len()
        )));
    }
    for (index, argument) in arguments.iter().enumerate() {
        let expected_offset = (index as u64) * 8;
        let pointer = index.is_multiple_of(2);
        let expected_kind = if pointer {
            ExplicitValueKind::GlobalBuffer
        } else {
            ExplicitValueKind::ByValue
        };
        let expected_address_space = pointer.then_some(ArgumentAddressSpace::Global);
        if argument.offset() != expected_offset
            || argument.size() != 8
            || !matches!(argument.alignment(), None | Some(8))
            || argument.value_kind() != expected_kind
            || argument.address_space() != expected_address_space
        {
            return Err(TypedArtifactError::PhysicalAbi(format!(
                "typed vecadd HSACO argument {index} has offset {}, size {}, alignment {:?}, kind {:?}, address space {:?}",
                argument.offset(),
                argument.size(),
                argument.alignment(),
                argument.value_kind(),
                argument.address_space()
            )));
        }

        let expected_access = match index {
            0 | 2 => ArgumentAccess::ReadOnly,
            4 => ArgumentAccess::WriteOnly,
            _ => continue,
        };
        if argument
            .access()
            .is_some_and(|access| access != expected_access)
            || argument
                .actual_access()
                .is_some_and(|access| access != expected_access)
        {
            return Err(TypedArtifactError::PhysicalAbi(format!(
                "typed vecadd HSACO pointer argument {index} has incompatible access metadata"
            )));
        }
    }

    Ok(())
}

fn is_production_typed_vecadd_kernarg_shape(
    size: u64,
    alignment: u64,
    implicit_offset: Option<u64>,
) -> bool {
    size == TYPED_VECADD_EXPLICIT_KERNARG_BYTES && alignment == 8 && implicit_offset.is_none()
}

fn typed_vecadd_launch() -> Result<LaunchContract, TypedArtifactError> {
    LaunchContract::new(
        1,
        BlockSize::Exact(Dimensions::new(256, 1, 1).map_err(TypedArtifactError::Model)?),
        Dimensions::new(u32::MAX, 1, 1).map_err(TypedArtifactError::Model)?,
        0,
        0,
    )
    .map_err(TypedArtifactError::Model)
}

fn typed_vecadd_abi(type_identities: [TypeIdentity; 3]) -> Result<AbiLayout, TypedArtifactError> {
    let slice_kind = AbiKind::Slice {
        element_size: 4,
        element_alignment: 4,
    };

    let fields = vec![
        AbiField::new(
            name("arg0")?,
            0,
            16,
            8,
            slice_kind,
            Mutability::Immutable,
            Access::ReadOnly,
            AddressSpace::Global,
            type_identities[0],
            ArgumentOwnership::SharedBorrow,
            AliasClass::SharedReadOnly,
        )
        .map_err(TypedArtifactError::Model)?,
        AbiField::new(
            name("arg1")?,
            16,
            16,
            8,
            slice_kind,
            Mutability::Immutable,
            Access::ReadOnly,
            AddressSpace::Global,
            type_identities[1],
            ArgumentOwnership::SharedBorrow,
            AliasClass::SharedReadOnly,
        )
        .map_err(TypedArtifactError::Model)?,
        AbiField::new(
            name("arg2")?,
            32,
            16,
            8,
            slice_kind,
            Mutability::Mutable,
            Access::WriteOnly,
            AddressSpace::Global,
            type_identities[2],
            ArgumentOwnership::UniqueBorrow,
            AliasClass::Exclusive,
        )
        .map_err(TypedArtifactError::Model)?,
    ];

    AbiLayout::new(48, 8, PointerWidth::Bits64, fields).map_err(TypedArtifactError::Model)
}

fn domain_digest(algorithm: DigestAlgorithm, domain: &[u8], fields: &[&[u8]]) -> DigestBytes {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(domain);
    for field in fields {
        canonical.extend_from_slice(&(field.len() as u64).to_le_bytes());
        canonical.extend_from_slice(field);
    }
    algorithm.calculate(&canonical).bytes()
}

fn name(value: &str) -> Result<Name, TypedArtifactError> {
    Name::new(value).map_err(TypedArtifactError::Model)
}

fn text(value: &str) -> Result<IdentityText, TypedArtifactError> {
    IdentityText::new(value).map_err(TypedArtifactError::Model)
}

#[derive(Debug)]
pub(crate) enum TypedArtifactError {
    EmptyLlvmIr,
    LengthOverflow,
    Model(ValidationError),
    Container(ContainerValidationError),
    Hsaco(fe2o3_hsaco::KernelBindingError),
    PhysicalAbi(String),
}

impl fmt::Display for TypedArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLlvmIr => formatter.write_str("typed artifact LLVM IR is empty"),
            Self::LengthOverflow => formatter.write_str("typed artifact length exceeds u64"),
            Self::Model(error) => error.fmt(formatter),
            Self::Container(error) => error.fmt(formatter),
            Self::Hsaco(error) => write!(formatter, "invalid typed HSACO: {error}"),
            Self::PhysicalAbi(reason) => write!(formatter, "invalid typed HSACO ABI: {reason}"),
        }
    }
}

impl std::error::Error for TypedArtifactError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Model(error) => Some(error),
            Self::Container(error) => Some(error),
            Self::Hsaco(error) => Some(error),
            Self::EmptyLlvmIr | Self::LengthOverflow | Self::PhysicalAbi(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_artifacts::{
        RustDisjointIndexSpaceV1, RustLayoutEvidenceV1, RustPhysicalComponentKindV1,
        RustPhysicalComponentV1, RustPointerMutabilityV1, RustScalarElementTypeV1,
        RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1,
    };

    fn binding() -> KernelBindingIdV1 {
        KernelBindingIdV1::from_bytes([0x42; 32])
    }

    fn evidence(
        source_type: RustSourceTypeShapeV1,
        mutability: RustPointerMutabilityV1,
    ) -> TypeIdentity {
        RustLayoutEvidenceV1::new(
            RustTypeEvidenceV1::new(source_type),
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            16,
            8,
            vec![
                RustPhysicalComponentV1::new(
                    0,
                    8,
                    8,
                    RustPhysicalComponentKindV1::Pointer {
                        mutability,
                        pointee: RustScalarElementTypeV1::F32,
                    },
                )
                .unwrap(),
                RustPhysicalComponentV1::new(8, 8, 8, RustPhysicalComponentKindV1::Usize).unwrap(),
            ],
        )
        .unwrap()
        .type_identity()
    }

    fn type_identities() -> [TypeIdentity; 3] {
        let shared = evidence(
            RustSourceTypeShapeV1::shared_slice(RustScalarElementTypeV1::F32),
            RustPointerMutabilityV1::Const,
        );
        let output = evidence(
            RustSourceTypeShapeV1::disjoint_slice(
                RustScalarElementTypeV1::F32,
                RustDisjointIndexSpaceV1::Index1D,
            ),
            RustPointerMutabilityV1::Mut,
        );
        [shared, shared, output]
    }

    #[test]
    fn production_kernarg_shape_is_explicit_only() {
        assert!(is_production_typed_vecadd_kernarg_shape(48, 8, None));
        assert!(!is_production_typed_vecadd_kernarg_shape(304, 8, Some(48)));
        assert!(!is_production_typed_vecadd_kernarg_shape(48, 4, None));
        assert!(!is_production_typed_vecadd_kernarg_shape(52, 8, None));
        assert!(!is_production_typed_vecadd_kernarg_shape(48, 8, Some(48)));
    }

    fn build() -> GeneratedTypedArtifactV1 {
        build_typed_vecadd_artifact_v1(
            "vecadd",
            "vecadd",
            binding(),
            type_identities(),
            &AmdGpuTarget::new("gfx942"),
            b"define amdgpu_kernel void @vecadd() { ret void }",
            b"synthetic-hsaco".to_vec(),
        )
        .unwrap()
    }

    #[test]
    fn emits_canonical_exact_vecadd_profile() {
        let generated = build();
        let decoded = ArtifactContainerV1::from_bytes(generated.container()).unwrap();
        let manifest = decoded.manifest();
        assert_eq!(manifest.kernels().len(), 1);
        assert_eq!(manifest.code_objects().len(), 1);
        assert_eq!(manifest.target().architecture().as_str(), "gfx942");

        let kernel = &manifest.kernels()[0];
        assert_eq!(kernel.name().as_str(), "vecadd");
        assert_eq!(kernel.symbol().as_str(), "vecadd");
        assert_eq!(kernel.abi(), &typed_vecadd_abi(type_identities()).unwrap());
        assert_eq!(kernel.launch(), &typed_vecadd_launch().unwrap());
        assert_eq!(decoded.payloads()[0].bytes(), b"synthetic-hsaco");
    }

    #[test]
    fn output_is_deterministic_and_content_addressed() {
        let first = build();
        let second = build();
        assert_eq!(first.container(), second.container());
        assert_eq!(first.artifact_id(), second.artifact_id());
        assert_eq!(first.artifact_id().len(), 64);
        assert!(
            first
                .artifact_id()
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );

        let changed = build_typed_vecadd_artifact_v1(
            "vecadd",
            "vecadd",
            binding(),
            type_identities(),
            &AmdGpuTarget::new("gfx942"),
            b"changed finalized IR",
            b"synthetic-hsaco".to_vec(),
        )
        .unwrap();
        assert_ne!(first.artifact_id(), changed.artifact_id());

        let changed_executable = build_typed_vecadd_artifact_v1(
            "vecadd",
            "vecadd",
            binding(),
            type_identities(),
            &AmdGpuTarget::new("gfx942"),
            b"define amdgpu_kernel void @vecadd() { ret void }",
            b"different-hsaco".to_vec(),
        )
        .unwrap();
        assert_ne!(first.artifact_id(), changed_executable.artifact_id());

        let changed_target = build_typed_vecadd_artifact_v1(
            "vecadd",
            "vecadd",
            binding(),
            type_identities(),
            &AmdGpuTarget::new("gfx950"),
            b"define amdgpu_kernel void @vecadd() { ret void }",
            b"synthetic-hsaco".to_vec(),
        )
        .unwrap();
        assert_ne!(first.artifact_id(), changed_target.artifact_id());

        let changed_binding = build_typed_vecadd_artifact_v1(
            "vecadd",
            "vecadd",
            KernelBindingIdV1::from_bytes([0x43; 32]),
            type_identities(),
            &AmdGpuTarget::new("gfx942"),
            b"define amdgpu_kernel void @vecadd() { ret void }",
            b"synthetic-hsaco".to_vec(),
        )
        .unwrap();
        assert_ne!(first.artifact_id(), changed_binding.artifact_id());

        let mut changed_identities = type_identities();
        changed_identities[2] = changed_identities[0];
        let changed_layout = build_typed_vecadd_artifact_v1(
            "vecadd",
            "vecadd",
            binding(),
            changed_identities,
            &AmdGpuTarget::new("gfx942"),
            b"define amdgpu_kernel void @vecadd() { ret void }",
            b"synthetic-hsaco".to_vec(),
        )
        .unwrap();
        assert_ne!(first.artifact_id(), changed_layout.artifact_id());
    }

    #[test]
    fn rejects_empty_source_and_payload() {
        assert!(matches!(
            build_typed_vecadd_artifact_v1(
                "vecadd",
                "vecadd",
                binding(),
                type_identities(),
                &AmdGpuTarget::new("gfx942"),
                b"",
                b"hsaco".to_vec(),
            ),
            Err(TypedArtifactError::EmptyLlvmIr)
        ));
        assert!(matches!(
            build_typed_vecadd_artifact_v1(
                "vecadd",
                "vecadd",
                binding(),
                type_identities(),
                &AmdGpuTarget::new("gfx942"),
                b"ir",
                vec![],
            ),
            Err(TypedArtifactError::Container(
                ContainerValidationError::EmptyPayload
            ))
        ));
    }
}
