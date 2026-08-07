use std::fmt;

use crate::{
    ArtifactContainerV1, BundleIndexV1, BundleValidationError, CodeObjectFormat,
    CodeObjectIdentity, CodeObjectPayload, CompilerIdentity, ContainerDecodeError,
    ContainerValidationError, DigestBytes, Endianness, ExecutableCodeObjectVersionV1, KernelEntry,
    ManifestV1, PayloadDigest, PointerWidth, ProofExecutableBindingV1, ProofTargetError,
    ToolIdentity, ValidationError,
};

pub const GFX942_TWO_KERNEL_BUNDLE_VERSION_V1: u16 = 1;
pub const GFX942_TWO_KERNEL_COUNT: usize = 2;

/// One independently keyed proof binding supplied to the gfx942 bundle profile.
///
/// The explicit kernel and effects identities are compared with the binding;
/// they prevent an otherwise valid proof for one entry from being substituted
/// for another entry in the same payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942KernelProofBindingV1 {
    kernel_id: DigestBytes,
    effects_identity: PayloadDigest,
    binding: ProofExecutableBindingV1,
}

impl Gfx942KernelProofBindingV1 {
    pub const fn new(
        kernel_id: DigestBytes,
        effects_identity: PayloadDigest,
        binding: ProofExecutableBindingV1,
    ) -> Self {
        Self {
            kernel_id,
            effects_identity,
            binding,
        }
    }

    pub const fn kernel_id(&self) -> DigestBytes {
        self.kernel_id
    }

    pub const fn effects_identity(&self) -> PayloadDigest {
        self.effects_identity
    }

    pub const fn binding(&self) -> &ProofExecutableBindingV1 {
        &self.binding
    }
}

/// Canonical V1 profile for two kernels in one native gfx942 code object.
///
/// This is a validated composition of the existing manifest, container,
/// bundle-index, and proof-binding models. It deliberately introduces no wire
/// format: `to_container_bytes` and `from_container_bytes` use the canonical
/// `ArtifactContainerV1` encoding, while proof bindings remain independently
/// supplied evidence.
#[derive(Debug, Eq, PartialEq)]
pub struct Gfx942TwoKernelBundleV1 {
    container: ArtifactContainerV1,
    index: BundleIndexV1,
    proof_bindings: [Gfx942KernelProofBindingV1; GFX942_TWO_KERNEL_COUNT],
}

impl Gfx942TwoKernelBundleV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        compiler: CompilerIdentity,
        producer: ToolIdentity,
        target: crate::TargetIdentity,
        kernels: [KernelEntry; GFX942_TWO_KERNEL_COUNT],
        payload: CodeObjectPayload,
        proof_bindings: [Gfx942KernelProofBindingV1; GFX942_TWO_KERNEL_COUNT],
    ) -> Result<Self, Gfx942BundleError> {
        let payload_digest = payload.digest();
        for kernel in &kernels {
            if kernel.code_object_digest() != payload_digest.bytes() {
                return Err(Gfx942BundleError::ConflictingPayloadIdentity {
                    kernel_id: kernel.kernel_id(),
                });
            }
        }

        let code_object = CodeObjectIdentity::new(
            payload_digest.bytes(),
            CodeObjectFormat::NativeExecutable,
            payload.bytes().len() as u64,
        )?;
        let manifest = ManifestV1::new(
            compiler,
            producer,
            target,
            vec![code_object],
            Vec::from(kernels),
        )?;
        let container =
            ArtifactContainerV1::new(manifest, payload_digest.algorithm(), vec![payload])?;
        Self::from_container(container, proof_bindings)
    }

    pub fn from_container_bytes(
        bytes: &[u8],
        proof_bindings: [Gfx942KernelProofBindingV1; GFX942_TWO_KERNEL_COUNT],
    ) -> Result<Self, Gfx942BundleError> {
        Self::from_container(ArtifactContainerV1::from_bytes(bytes)?, proof_bindings)
    }

    pub fn from_container(
        container: ArtifactContainerV1,
        mut proof_bindings: [Gfx942KernelProofBindingV1; GFX942_TWO_KERNEL_COUNT],
    ) -> Result<Self, Gfx942BundleError> {
        validate_container_shape(&container)?;

        proof_bindings.sort_unstable_by_key(Gfx942KernelProofBindingV1::kernel_id);
        if proof_bindings[0].kernel_id() == proof_bindings[1].kernel_id() {
            return Err(Gfx942BundleError::DuplicateProofKernel(
                proof_bindings[0].kernel_id(),
            ));
        }

        for (kernel, proof) in container.manifest().kernels().iter().zip(&proof_bindings) {
            validate_kernel_proof(&container, kernel, proof)?;
        }

        let index = BundleIndexV1::from_containers(std::slice::from_ref(&container))?;
        debug_assert_eq!(index.target_associations().len(), 1);
        debug_assert_eq!(index.payloads().len(), 1);
        debug_assert_eq!(index.kernels().len(), GFX942_TWO_KERNEL_COUNT);
        debug_assert!(
            index
                .kernels()
                .iter()
                .all(|kernel| { kernel.payload_digests() == [index.payloads()[0].digest()] })
        );

        Ok(Self {
            container,
            index,
            proof_bindings,
        })
    }

    pub const fn version(&self) -> u16 {
        GFX942_TWO_KERNEL_BUNDLE_VERSION_V1
    }

    pub const fn container(&self) -> &ArtifactContainerV1 {
        &self.container
    }

    pub const fn index(&self) -> &BundleIndexV1 {
        &self.index
    }

    pub fn proof_bindings(&self) -> &[Gfx942KernelProofBindingV1; GFX942_TWO_KERNEL_COUNT] {
        &self.proof_bindings
    }

    pub fn to_container_bytes(&self) -> Vec<u8> {
        self.container.to_bytes()
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn validate_container_shape(container: &ArtifactContainerV1) -> Result<(), Gfx942BundleError> {
    let manifest = container.manifest();
    let target = manifest.target();
    if target.triple().as_str() != "amdgcn-amd-amdhsa" {
        return Err(Gfx942BundleError::TargetMismatch("target triple"));
    }
    if target.architecture().as_str() != "gfx942" {
        return Err(Gfx942BundleError::TargetMismatch("target architecture"));
    }
    if target.pointer_width() != PointerWidth::Bits64 {
        return Err(Gfx942BundleError::TargetMismatch("pointer width"));
    }
    if target.endianness() != Endianness::Little {
        return Err(Gfx942BundleError::TargetMismatch("endianness"));
    }
    require_exact_count("code objects", manifest.code_objects().len(), 1)?;
    require_exact_count("payloads", container.payloads().len(), 1)?;
    require_exact_count("kernels", manifest.kernels().len(), GFX942_TWO_KERNEL_COUNT)?;

    let code_object = &manifest.code_objects()[0];
    if code_object.format() != CodeObjectFormat::NativeExecutable {
        return Err(Gfx942BundleError::NonNativePayload(code_object.format()));
    }
    let shared_payload = container.payloads()[0].digest();
    if code_object.digest() != shared_payload.bytes() {
        return Err(Gfx942BundleError::ConflictingPayloadIdentity {
            kernel_id: manifest.kernels()[0].kernel_id(),
        });
    }
    for kernel in manifest.kernels() {
        if kernel.code_object_digest() != shared_payload.bytes() {
            return Err(Gfx942BundleError::ConflictingPayloadIdentity {
                kernel_id: kernel.kernel_id(),
            });
        }
    }
    Ok(())
}

fn validate_kernel_proof(
    container: &ArtifactContainerV1,
    kernel: &KernelEntry,
    proof: &Gfx942KernelProofBindingV1,
) -> Result<(), Gfx942BundleError> {
    if proof.kernel_id() != kernel.kernel_id() {
        return Err(Gfx942BundleError::KernelSetMismatch {
            expected: kernel.kernel_id(),
            actual: proof.kernel_id(),
        });
    }

    let binding = proof.binding();
    let executable = binding.executable();
    let proof_target = executable.proof_target();
    let artifact = proof_target.artifact();
    if artifact.kernel_id().bytes() != proof.kernel_id() {
        return Err(Gfx942BundleError::CrossKernelProofSubstitution {
            declared: proof.kernel_id(),
            bound: artifact.kernel_id().bytes(),
        });
    }

    require_kernel_identity(
        kernel,
        artifact.source_tree_digest().bytes(),
        kernel.source_digest(),
        "source identity",
    )?;
    require_kernel_identity(
        kernel,
        executable.kernel_semantic_identity().bytes(),
        kernel.executable_digest(),
        "executable identity",
    )?;
    if executable.finalized_code_object_digest() != container.payloads()[0].digest() {
        return Err(Gfx942BundleError::KernelIdentityMismatch {
            kernel_id: kernel.kernel_id(),
            field: "shared payload identity",
        });
    }
    if executable.target() != container.manifest().target() {
        return Err(Gfx942BundleError::KernelIdentityMismatch {
            kernel_id: kernel.kernel_id(),
            field: "target identity",
        });
    }
    if executable.code_object_version() != ExecutableCodeObjectVersionV1::V5 {
        return Err(Gfx942BundleError::KernelIdentityMismatch {
            kernel_id: kernel.kernel_id(),
            field: "code-object version",
        });
    }
    if executable.abi() != kernel.abi() {
        return Err(Gfx942BundleError::KernelIdentityMismatch {
            kernel_id: kernel.kernel_id(),
            field: "ABI identity",
        });
    }
    if executable.launch() != kernel.launch() {
        return Err(Gfx942BundleError::KernelIdentityMismatch {
            kernel_id: kernel.kernel_id(),
            field: "launch identity",
        });
    }
    if executable.source_contracts().effects_digest() != proof.effects_identity() {
        return Err(Gfx942BundleError::KernelIdentityMismatch {
            kernel_id: kernel.kernel_id(),
            field: "effects identity",
        });
    }

    let reconstructed = container.manifest().proof_target(
        artifact.kernel_id(),
        artifact.instance_digest(),
        artifact.source_tree_digest(),
        artifact.crate_graph_digest(),
        artifact.executable_digest(),
        executable.finalized_code_object_digest(),
        proof_target.source_contracts(),
        binding.tool_policy().compiler(),
        binding.tool_policy().artifact_producer(),
        artifact.environment_digest().algorithm(),
    )?;
    if reconstructed != proof_target {
        return Err(Gfx942BundleError::KernelIdentityMismatch {
            kernel_id: kernel.kernel_id(),
            field: "proof target identity",
        });
    }

    Ok(())
}

fn require_kernel_identity(
    kernel: &KernelEntry,
    actual: DigestBytes,
    expected: DigestBytes,
    field: &'static str,
) -> Result<(), Gfx942BundleError> {
    if actual == expected {
        Ok(())
    } else {
        Err(Gfx942BundleError::KernelIdentityMismatch {
            kernel_id: kernel.kernel_id(),
            field,
        })
    }
}

fn require_exact_count(
    field: &'static str,
    actual: usize,
    expected: usize,
) -> Result<(), Gfx942BundleError> {
    if actual == expected {
        Ok(())
    } else {
        Err(Gfx942BundleError::UnexpectedCount {
            field,
            expected,
            actual,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942BundleError {
    Model(ValidationError),
    Container(ContainerValidationError),
    Decode(ContainerDecodeError),
    Bundle(BundleValidationError),
    ProofTarget(ProofTargetError),
    TargetMismatch(&'static str),
    UnexpectedCount {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    NonNativePayload(CodeObjectFormat),
    ConflictingPayloadIdentity {
        kernel_id: DigestBytes,
    },
    DuplicateProofKernel(DigestBytes),
    KernelSetMismatch {
        expected: DigestBytes,
        actual: DigestBytes,
    },
    CrossKernelProofSubstitution {
        declared: DigestBytes,
        bound: DigestBytes,
    },
    KernelIdentityMismatch {
        kernel_id: DigestBytes,
        field: &'static str,
    },
}

impl fmt::Display for Gfx942BundleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(error) => error.fmt(formatter),
            Self::Container(error) => error.fmt(formatter),
            Self::Decode(error) => error.fmt(formatter),
            Self::Bundle(error) => error.fmt(formatter),
            Self::ProofTarget(error) => error.fmt(formatter),
            Self::TargetMismatch(field) => {
                write!(formatter, "gfx942 bundle {field} does not match")
            }
            Self::UnexpectedCount {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "gfx942 bundle requires {expected} {field}, found {actual}"
            ),
            Self::NonNativePayload(format) => {
                write!(formatter, "gfx942 bundle payload is not native: {format:?}")
            }
            Self::ConflictingPayloadIdentity { kernel_id } => write!(
                formatter,
                "kernel {kernel_id:?} does not reference the shared bundle payload"
            ),
            Self::DuplicateProofKernel(kernel_id) => {
                write!(
                    formatter,
                    "duplicate proof binding for kernel {kernel_id:?}"
                )
            }
            Self::KernelSetMismatch { expected, actual } => write!(
                formatter,
                "proof kernel set mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::CrossKernelProofSubstitution { declared, bound } => write!(
                formatter,
                "proof declared for kernel {declared:?} is bound to kernel {bound:?}"
            ),
            Self::KernelIdentityMismatch { kernel_id, field } => {
                write!(formatter, "kernel {kernel_id:?} {field} does not match")
            }
        }
    }
}

impl std::error::Error for Gfx942BundleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Model(error) => Some(error),
            Self::Container(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Bundle(error) => Some(error),
            Self::ProofTarget(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ValidationError> for Gfx942BundleError {
    fn from(value: ValidationError) -> Self {
        Self::Model(value)
    }
}

impl From<ContainerValidationError> for Gfx942BundleError {
    fn from(value: ContainerValidationError) -> Self {
        Self::Container(value)
    }
}

impl From<ContainerDecodeError> for Gfx942BundleError {
    fn from(value: ContainerDecodeError) -> Self {
        Self::Decode(value)
    }
}

impl From<BundleValidationError> for Gfx942BundleError {
    fn from(value: BundleValidationError) -> Self {
        Self::Bundle(value)
    }
}

impl From<ProofTargetError> for Gfx942BundleError {
    fn from(value: ProofTargetError) -> Self {
        Self::ProofTarget(value)
    }
}
