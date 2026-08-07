use std::fmt;

use fe2o3_artifacts::{DigestAlgorithm, ExecutableCodeObjectVersionV1, MAX_KERNELS, PayloadDigest};

use crate::{
    AuthenticatedControlFlowExecutableBindingV1, ControlFlowProofRequestBindingV1, Digest,
};

pub const MULTI_KERNEL_PROOF_ADMISSION_DOMAIN_V1: [u8; 8] = *b"FE2MKPA\0";
pub const MULTI_KERNEL_PROOF_ADMISSION_VERSION_V1: u16 = 1;
const MULTI_KERNEL_SOURCE_CONTRACT_DOMAIN_V1: [u8; 8] = *b"FE2MKSC\0";

/// Independently checked identity axes for one requested kernel proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelProofAdmissionIdentityV1 {
    Source,
    Contract,
    ProofRequest,
    AuthenticatedProof,
}

impl KernelProofAdmissionIdentityV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Contract => "contract",
            Self::ProofRequest => "proof request",
            Self::AuthenticatedProof => "authenticated proof",
        }
    }
}

/// Exact per-kernel identities required by a multi-kernel proof admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelProofAdmissionRequestV1 {
    kernel_id: Digest,
    source_identity: Digest,
    contract_identity: Digest,
    proof_request_identity: Digest,
    authenticated_proof_identity: Digest,
}

impl KernelProofAdmissionRequestV1 {
    pub fn new(
        request: &ControlFlowProofRequestBindingV1,
        authenticated_proof_identity: Digest,
    ) -> Self {
        Self {
            kernel_id: request.target().kernel_id,
            source_identity: request.source().binding_identity(),
            contract_identity: source_contract_identity(request),
            proof_request_identity: request.request_digest(),
            authenticated_proof_identity,
        }
    }

    pub fn from_binding(binding: &AuthenticatedControlFlowExecutableBindingV1) -> Self {
        Self::new(
            binding.request_binding(),
            binding.proof_executable_binding().binding_identity(),
        )
    }

    pub const fn kernel_id(self) -> Digest {
        self.kernel_id
    }

    pub const fn source_identity(self) -> Digest {
        self.source_identity
    }

    pub const fn contract_identity(self) -> Digest {
        self.contract_identity
    }

    pub const fn proof_request_identity(self) -> Digest {
        self.proof_request_identity
    }

    pub const fn authenticated_proof_identity(self) -> Digest {
        self.authenticated_proof_identity
    }
}

/// Canonical collection of per-kernel proofs for one finalized executable.
///
/// Each input has already passed authenticated execution, proof/executable,
/// source-contract, and freshness admission. This layer prevents those valid
/// per-kernel records from becoming interchangeable when they share a code
/// object and toolchain. It remains inert evidence and grants no runtime
/// authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiKernelProofAdmissionV1 {
    finalized_executable_digest: PayloadDigest,
    code_object_version: ExecutableCodeObjectVersionV1,
    bindings: Vec<AuthenticatedControlFlowExecutableBindingV1>,
    binding_identity: Digest,
}

impl MultiKernelProofAdmissionV1 {
    pub fn new(
        mut bindings: Vec<AuthenticatedControlFlowExecutableBindingV1>,
    ) -> Result<Self, MultiKernelProofAdmissionErrorV1> {
        if bindings.is_empty() {
            return Err(MultiKernelProofAdmissionErrorV1::Empty);
        }
        if bindings.len() > MAX_KERNELS {
            return Err(MultiKernelProofAdmissionErrorV1::TooManyKernels { max: MAX_KERNELS });
        }

        bindings.sort_unstable_by_key(kernel_id);
        if let Some(pair) = bindings
            .windows(2)
            .find(|pair| kernel_id(&pair[0]) == kernel_id(&pair[1]))
        {
            return Err(MultiKernelProofAdmissionErrorV1::DuplicateKernel {
                kernel_id: kernel_id(&pair[1]),
            });
        }

        let first = executable_binding(&bindings[0]);
        let finalized_executable_digest = first.executable().finalized_code_object_digest();
        if finalized_executable_digest.algorithm() != DigestAlgorithm::Sha256 {
            return Err(MultiKernelProofAdmissionErrorV1::UnsupportedDigestAlgorithm);
        }
        let code_object_version = first.executable().code_object_version();

        for binding in &bindings {
            validate_internal_kernel_binding(binding)?;
            let executable = executable_binding(binding);
            require_equal(
                finalized_executable_digest,
                executable.executable().finalized_code_object_digest(),
                "finalized executable",
            )?;
            require_equal(
                first.executable().target(),
                executable.executable().target(),
                "target",
            )?;
            require_equal(
                code_object_version,
                executable.executable().code_object_version(),
                "code-object version",
            )?;
            require_equal(
                first.tool_policy().compiler(),
                executable.tool_policy().compiler(),
                "compiler",
            )?;
            require_equal(
                first.tool_policy().artifact_producer(),
                executable.tool_policy().artifact_producer(),
                "artifact producer",
            )?;
        }

        let binding_identity = admission_identity(finalized_executable_digest, &bindings);
        Ok(Self {
            finalized_executable_digest,
            code_object_version,
            bindings,
            binding_identity,
        })
    }

    pub const fn version(&self) -> u16 {
        MULTI_KERNEL_PROOF_ADMISSION_VERSION_V1
    }

    pub const fn finalized_executable_digest(&self) -> PayloadDigest {
        self.finalized_executable_digest
    }

    pub const fn code_object_version(&self) -> ExecutableCodeObjectVersionV1 {
        self.code_object_version
    }

    pub fn kernel_count(&self) -> usize {
        self.bindings.len()
    }

    pub const fn binding_identity(&self) -> Digest {
        self.binding_identity
    }

    pub fn admit_kernel(
        &self,
        request: KernelProofAdmissionRequestV1,
    ) -> Result<&AuthenticatedControlFlowExecutableBindingV1, MultiKernelProofAdmissionErrorV1>
    {
        let index = self
            .bindings
            .binary_search_by_key(&request.kernel_id, kernel_id)
            .map_err(|_| MultiKernelProofAdmissionErrorV1::UnknownKernel {
                kernel_id: request.kernel_id,
            })?;
        let binding = &self.bindings[index];
        let admitted = KernelProofAdmissionRequestV1::from_binding(binding);
        for (field, expected, actual) in [
            (
                KernelProofAdmissionIdentityV1::Source,
                admitted.source_identity,
                request.source_identity,
            ),
            (
                KernelProofAdmissionIdentityV1::Contract,
                admitted.contract_identity,
                request.contract_identity,
            ),
            (
                KernelProofAdmissionIdentityV1::ProofRequest,
                admitted.proof_request_identity,
                request.proof_request_identity,
            ),
            (
                KernelProofAdmissionIdentityV1::AuthenticatedProof,
                admitted.authenticated_proof_identity,
                request.authenticated_proof_identity,
            ),
        ] {
            if expected != actual {
                return Err(MultiKernelProofAdmissionErrorV1::IdentityMismatch {
                    kernel_id: request.kernel_id,
                    field,
                });
            }
        }
        Ok(binding)
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn executable_binding(
    binding: &AuthenticatedControlFlowExecutableBindingV1,
) -> &fe2o3_artifacts::ProofExecutableBindingV1 {
    binding.proof_executable_binding().executable_binding()
}

fn kernel_id(binding: &AuthenticatedControlFlowExecutableBindingV1) -> Digest {
    binding.request_binding().target().kernel_id
}

fn validate_internal_kernel_binding(
    binding: &AuthenticatedControlFlowExecutableBindingV1,
) -> Result<(), MultiKernelProofAdmissionErrorV1> {
    let request_kernel = kernel_id(binding);
    let proof_kernel = executable_binding(binding)
        .executable()
        .proof_target()
        .artifact()
        .kernel_id();
    if proof_kernel.algorithm() != DigestAlgorithm::Sha256 {
        return Err(MultiKernelProofAdmissionErrorV1::UnsupportedDigestAlgorithm);
    }
    if request_kernel.as_bytes() != proof_kernel.bytes().as_bytes() {
        return Err(MultiKernelProofAdmissionErrorV1::InternalKernelMismatch {
            request: request_kernel,
            proof: Digest::from_bytes(*proof_kernel.bytes().as_bytes()),
        });
    }
    Ok(())
}

fn admission_identity(
    finalized_executable_digest: PayloadDigest,
    bindings: &[AuthenticatedControlFlowExecutableBindingV1],
) -> Digest {
    let mut bytes = Vec::with_capacity(48 + bindings.len() * 192);
    bytes.extend_from_slice(&MULTI_KERNEL_PROOF_ADMISSION_DOMAIN_V1);
    bytes.extend_from_slice(&MULTI_KERNEL_PROOF_ADMISSION_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(finalized_executable_digest.bytes().as_bytes());
    bytes.extend_from_slice(&(bindings.len() as u16).to_le_bytes());
    for binding in bindings {
        let request = KernelProofAdmissionRequestV1::from_binding(binding);
        for digest in [
            request.kernel_id,
            request.source_identity,
            request.contract_identity,
            request.proof_request_identity,
            request.authenticated_proof_identity,
            binding.binding_identity(),
        ] {
            bytes.extend_from_slice(digest.as_bytes());
        }
    }
    sha256(&bytes)
}

fn require_equal<T: PartialEq>(
    expected: T,
    actual: T,
    field: &'static str,
) -> Result<(), MultiKernelProofAdmissionErrorV1> {
    if expected == actual {
        Ok(())
    } else {
        Err(MultiKernelProofAdmissionErrorV1::ExecutableMismatch { field })
    }
}

fn sha256(bytes: &[u8]) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    Digest::from_bytes(*digest.bytes().as_bytes())
}

fn source_contract_identity(request: &ControlFlowProofRequestBindingV1) -> Digest {
    let target = request.target();
    let mut bytes = Vec::with_capacity(8 + 32 * 5);
    bytes.extend_from_slice(&MULTI_KERNEL_SOURCE_CONTRACT_DOMAIN_V1);
    for digest in [
        target.memory_contract_digest,
        target.effects_contract_digest,
        target.type_layout_digest,
        target.capability_semantics_digest,
        target.functional_specification_digest,
    ] {
        bytes.extend_from_slice(digest.as_bytes());
    }
    sha256(&bytes)
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MultiKernelProofAdmissionErrorV1 {
    Empty,
    TooManyKernels {
        max: usize,
    },
    UnsupportedDigestAlgorithm,
    DuplicateKernel {
        kernel_id: Digest,
    },
    InternalKernelMismatch {
        request: Digest,
        proof: Digest,
    },
    ExecutableMismatch {
        field: &'static str,
    },
    UnknownKernel {
        kernel_id: Digest,
    },
    IdentityMismatch {
        kernel_id: Digest,
        field: KernelProofAdmissionIdentityV1,
    },
}

impl fmt::Display for MultiKernelProofAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("multi-kernel proof admission is empty"),
            Self::TooManyKernels { max } => {
                write!(
                    formatter,
                    "multi-kernel proof admission exceeds {max} kernels"
                )
            }
            Self::UnsupportedDigestAlgorithm => {
                formatter.write_str("multi-kernel proof admission requires SHA-256")
            }
            Self::DuplicateKernel { kernel_id } => {
                write!(
                    formatter,
                    "kernel {} has duplicate proof evidence",
                    kernel_id.to_hex()
                )
            }
            Self::InternalKernelMismatch { request, proof } => write!(
                formatter,
                "proof kernel {} does not match request kernel {}",
                proof.to_hex(),
                request.to_hex()
            ),
            Self::ExecutableMismatch { field } => {
                write!(formatter, "multi-kernel {field} identity does not match")
            }
            Self::UnknownKernel { kernel_id } => {
                write!(
                    formatter,
                    "kernel {} has no admitted proof",
                    kernel_id.to_hex()
                )
            }
            Self::IdentityMismatch { kernel_id, field } => write!(
                formatter,
                "kernel {} {} identity does not match",
                kernel_id.to_hex(),
                field.as_str()
            ),
        }
    }
}

impl std::error::Error for MultiKernelProofAdmissionErrorV1 {}
