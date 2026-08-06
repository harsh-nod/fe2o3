use crate::AmdGpuTarget;
use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
    ArtifactContainerV1, BlockSize, CodeObjectFormat, CodeObjectIdentity, CodeObjectPayload,
    CompilerIdentity, ContainerValidationError, DeclaredRustLayoutIdentity,
    DeclaredRustTypeIdentity, DigestAlgorithm, DigestBytes, Dimensions, Endianness, IdentityText,
    KernelEntry, LaunchContract, ManifestV1, Mutability, Name, PointerWidth, ToolIdentity,
    TypeIdentity, ValidationError,
};
use std::fmt;

const SOURCE_DIGEST_DOMAIN: &[u8] = b"fe2o3.typed-vecadd.source-llvm-ir.v1\0";
const EXECUTABLE_DIGEST_DOMAIN: &[u8] = b"fe2o3.typed-vecadd.executable-hsaco.v1\0";
const KERNEL_ID_DOMAIN: &[u8] = b"fe2o3.typed-vecadd.kernel-id.v1\0";
const TYPE_ID_DOMAIN: &[u8] = b"fe2o3.rust-type.v1\0";
const LAYOUT_ID_DOMAIN: &[u8] = b"fe2o3.rust-layout.v1\0";

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
    let kernel_id = domain_digest(
        algorithm,
        KERNEL_ID_DOMAIN,
        &[
            logical_name.as_bytes(),
            export_name.as_bytes(),
            source_digest.as_bytes(),
            executable_digest.as_bytes(),
        ],
    );

    let kernel = KernelEntry::new(
        kernel_id,
        name(logical_name)?,
        name(export_name)?,
        source_digest,
        executable_digest,
        code_object_digest,
        vec![],
        typed_vecadd_launch()?,
        typed_vecadd_abi()?,
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
    let artifact_id = hex_digest(algorithm.calculate(&container).bytes());

    Ok(GeneratedTypedArtifactV1 {
        artifact_id,
        container,
    })
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

fn typed_vecadd_abi() -> Result<AbiLayout, TypedArtifactError> {
    let shared_slice = type_identity("&[f32]", "slice-f32-ptr64-size16-align8");
    let disjoint_slice = type_identity(
        "fe2o3_device::DisjointSlice<f32>",
        "disjoint-slice-f32-ptr64-size16-align8",
    );
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
            shared_slice,
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
            shared_slice,
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
            disjoint_slice,
            ArgumentOwnership::UniqueBorrow,
            AliasClass::Exclusive,
        )
        .map_err(TypedArtifactError::Model)?,
    ];

    AbiLayout::new(48, 8, PointerWidth::Bits64, fields).map_err(TypedArtifactError::Model)
}

fn type_identity(rust_type: &str, layout: &str) -> TypeIdentity {
    TypeIdentity::new(
        DeclaredRustTypeIdentity::from_untrusted_bytes(domain_digest(
            DigestAlgorithm::Sha256,
            TYPE_ID_DOMAIN,
            &[rust_type.as_bytes()],
        )),
        DeclaredRustLayoutIdentity::from_untrusted_bytes(domain_digest(
            DigestAlgorithm::Sha256,
            LAYOUT_ID_DOMAIN,
            &[layout.as_bytes()],
        )),
    )
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

fn hex_digest(digest: DigestBytes) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in digest.as_bytes() {
        use fmt::Write as _;
        write!(encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
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
}

impl fmt::Display for TypedArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLlvmIr => formatter.write_str("typed artifact LLVM IR is empty"),
            Self::LengthOverflow => formatter.write_str("typed artifact length exceeds u64"),
            Self::Model(error) => error.fmt(formatter),
            Self::Container(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TypedArtifactError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Model(error) => Some(error),
            Self::Container(error) => Some(error),
            Self::EmptyLlvmIr | Self::LengthOverflow => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build() -> GeneratedTypedArtifactV1 {
        build_typed_vecadd_artifact_v1(
            "vecadd",
            "vecadd",
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
        assert_eq!(kernel.abi(), &typed_vecadd_abi().unwrap());
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
            &AmdGpuTarget::new("gfx942"),
            b"changed finalized IR",
            b"synthetic-hsaco".to_vec(),
        )
        .unwrap();
        assert_ne!(first.artifact_id(), changed.artifact_id());

        let changed_executable = build_typed_vecadd_artifact_v1(
            "vecadd",
            "vecadd",
            &AmdGpuTarget::new("gfx942"),
            b"define amdgpu_kernel void @vecadd() { ret void }",
            b"different-hsaco".to_vec(),
        )
        .unwrap();
        assert_ne!(first.artifact_id(), changed_executable.artifact_id());

        let changed_target = build_typed_vecadd_artifact_v1(
            "vecadd",
            "vecadd",
            &AmdGpuTarget::new("gfx950"),
            b"define amdgpu_kernel void @vecadd() { ret void }",
            b"synthetic-hsaco".to_vec(),
        )
        .unwrap();
        assert_ne!(first.artifact_id(), changed_target.artifact_id());
    }

    #[test]
    fn rejects_empty_source_and_payload() {
        assert!(matches!(
            build_typed_vecadd_artifact_v1(
                "vecadd",
                "vecadd",
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
