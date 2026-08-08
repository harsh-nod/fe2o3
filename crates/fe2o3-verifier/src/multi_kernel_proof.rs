use std::collections::BTreeSet;
use std::fmt;

use fe2o3_artifacts::{DigestAlgorithm, ExecutableCodeObjectVersionV1, MAX_KERNELS, PayloadDigest};

use crate::{
    AuthenticatedControlFlowExecutableBindingV1, ControlFlowProofRequestBindingV1, Digest,
    PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1,
    PersistentlyFreshProofExecutableBindingV1,
};

pub const MULTI_KERNEL_PROOF_ADMISSION_DOMAIN_V1: [u8; 8] = *b"FE2MKPA\0";
pub const MULTI_KERNEL_PROOF_ADMISSION_VERSION_V1: u16 = 1;
const MULTI_KERNEL_SOURCE_CONTRACT_DOMAIN_V1: [u8; 8] = *b"FE2MKSC\0";
pub const PERSISTENT_MULTI_KERNEL_PROOF_ADMISSION_DOMAIN_V1: [u8; 8] = *b"FE2PMKA\0";
pub const PERSISTENT_MULTI_KERNEL_PROOF_ADMISSION_VERSION_V1: u16 = 1;

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

/// Independently checked identity axes for one persistently fresh kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistentlyFreshKernelProofAdmissionIdentityV1 {
    Source,
    Contract,
    ProofRequest,
    AuthenticatedProof,
    PersistentProof,
}

impl PersistentlyFreshKernelProofAdmissionIdentityV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Contract => "contract",
            Self::ProofRequest => "proof request",
            Self::AuthenticatedProof => "authenticated proof",
            Self::PersistentProof => "persistent proof",
        }
    }
}

/// Exact identities required to select one kernel from persistent aggregate
/// evidence.
///
/// This request is deliberately non-clone and has private fields. It can only
/// attach persistent-proof identity through an existing persistent binding.
/// It is still descriptive evidence and carries no runtime authority.
///
/// ```compile_fail
/// # fn process_local_is_not_persistent(
/// #     request: &fe2o3_verifier::ControlFlowProofRequestBindingV1,
/// #     local: &fe2o3_verifier::AuthenticatedProofExecutableBindingV1,
/// # ) {
/// fe2o3_verifier::PersistentlyFreshKernelProofAdmissionRequestV1::new(request, local);
/// # }
/// ```
///
/// ```compile_fail
/// # fn duplicate(
/// #     request: fe2o3_verifier::PersistentlyFreshKernelProofAdmissionRequestV1,
/// # ) {
/// let _copy = request.clone();
/// # }
/// ```
///
/// ```compile_fail
/// let _forged = fe2o3_verifier::PersistentlyFreshKernelProofAdmissionRequestV1 {
///     kernel_id: fe2o3_verifier::Digest::from_bytes([1; 32]),
///     source_identity: fe2o3_verifier::Digest::from_bytes([2; 32]),
///     contract_identity: fe2o3_verifier::Digest::from_bytes([3; 32]),
///     proof_request_identity: fe2o3_verifier::Digest::from_bytes([4; 32]),
///     authenticated_proof_identity: fe2o3_verifier::Digest::from_bytes([5; 32]),
///     persistent_proof_identity: fe2o3_verifier::Digest::from_bytes([6; 32]),
/// };
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct PersistentlyFreshKernelProofAdmissionRequestV1 {
    kernel_id: Digest,
    source_identity: Digest,
    contract_identity: Digest,
    proof_request_identity: Digest,
    authenticated_proof_identity: Digest,
    persistent_proof_identity: Digest,
}

impl PersistentlyFreshKernelProofAdmissionRequestV1 {
    pub fn new(
        request: &ControlFlowProofRequestBindingV1,
        persistent_proof: &PersistentlyFreshProofExecutableBindingV1,
    ) -> Self {
        Self {
            kernel_id: request.target().kernel_id,
            source_identity: request.source().binding_identity(),
            contract_identity: source_contract_identity(request),
            proof_request_identity: request.request_digest(),
            authenticated_proof_identity: persistent_proof.proof_binding().binding_identity(),
            persistent_proof_identity: persistent_proof.binding_identity(),
        }
    }

    pub fn from_binding(
        binding: &PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1,
    ) -> Self {
        Self::new(
            binding.request_binding(),
            binding.proof_executable_binding(),
        )
    }

    pub const fn kernel_id(&self) -> Digest {
        self.kernel_id
    }

    pub const fn source_identity(&self) -> Digest {
        self.source_identity
    }

    pub const fn contract_identity(&self) -> Digest {
        self.contract_identity
    }

    pub const fn proof_request_identity(&self) -> Digest {
        self.proof_request_identity
    }

    pub const fn authenticated_proof_identity(&self) -> Digest {
        self.authenticated_proof_identity
    }

    pub const fn persistent_proof_identity(&self) -> Digest {
        self.persistent_proof_identity
    }
}

/// Non-clone aggregate evidence for proofs consumed by one local persistent
/// freshness ledger.
///
/// Construction consumes the per-kernel persistent bindings, canonicalizes
/// them by kernel identity, and rejects mixed namespaces, repeated generations,
/// repeated kernels, or disagreement in the shared executable and measured
/// tool closure. The ledger is local persistent replay evidence only; it is not
/// rollback-resistant storage. This aggregate grants no load or launch
/// authority.
///
/// ```compile_fail
/// # fn process_local_is_not_persistent(
/// #     bindings: Vec<fe2o3_verifier::AuthenticatedControlFlowExecutableBindingV1>,
/// # ) {
/// let _ = fe2o3_verifier::PersistentlyFreshMultiKernelProofAdmissionV1::new(bindings);
/// # }
/// ```
///
/// ```compile_fail
/// # fn duplicate(value: fe2o3_verifier::PersistentlyFreshMultiKernelProofAdmissionV1) {
/// let _copy = value.clone();
/// # }
/// ```
///
/// ```compile_fail
/// let _forged = fe2o3_verifier::PersistentlyFreshMultiKernelProofAdmissionV1 {
///     bindings: Vec::new(),
/// };
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct PersistentlyFreshMultiKernelProofAdmissionV1 {
    finalized_executable_digest: PayloadDigest,
    code_object_version: ExecutableCodeObjectVersionV1,
    ledger_namespace: Digest,
    bindings: Vec<PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1>,
    binding_identity: Digest,
}

impl PersistentlyFreshMultiKernelProofAdmissionV1 {
    pub fn new(
        mut bindings: Vec<PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1>,
    ) -> Result<Self, PersistentlyFreshMultiKernelProofAdmissionErrorV1> {
        if bindings.is_empty() {
            return Err(PersistentlyFreshMultiKernelProofAdmissionErrorV1::Empty);
        }
        if bindings.len() > MAX_KERNELS {
            return Err(
                PersistentlyFreshMultiKernelProofAdmissionErrorV1::TooManyKernels {
                    max: MAX_KERNELS,
                },
            );
        }

        bindings.sort_unstable_by_key(persistent_kernel_id);
        if let Some(pair) = bindings
            .windows(2)
            .find(|pair| persistent_kernel_id(&pair[0]) == persistent_kernel_id(&pair[1]))
        {
            return Err(
                PersistentlyFreshMultiKernelProofAdmissionErrorV1::DuplicateKernel {
                    kernel_id: persistent_kernel_id(&pair[1]),
                },
            );
        }

        let first_persistent = bindings[0].proof_executable_binding();
        let first_identity = first_persistent.identity();
        let ledger_namespace = first_identity.ledger_namespace();
        let first = persistent_executable_binding(&bindings[0]);
        let finalized_executable_digest = first.executable().finalized_code_object_digest();
        if finalized_executable_digest.algorithm() != DigestAlgorithm::Sha256 {
            return Err(
                PersistentlyFreshMultiKernelProofAdmissionErrorV1::UnsupportedDigestAlgorithm,
            );
        }
        let code_object_version = first.executable().code_object_version();
        let first_plan = first_persistent
            .proof_binding()
            .execution_evidence()
            .invocation_plan();
        let first_execution = first.tool_policy().proof_execution();
        let mut generations = BTreeSet::new();

        for binding in &bindings {
            validate_internal_persistent_kernel_binding(binding)?;
            let persistent = binding.proof_executable_binding();
            let identity = persistent.identity();
            if identity.ledger_namespace() != ledger_namespace {
                return Err(
                    PersistentlyFreshMultiKernelProofAdmissionErrorV1::MixedLedgerNamespace,
                );
            }
            if !generations.insert(identity.ledger_generation()) {
                return Err(
                    PersistentlyFreshMultiKernelProofAdmissionErrorV1::DuplicateLedgerGeneration {
                        generation: identity.ledger_generation(),
                    },
                );
            }

            let executable = persistent_executable_binding(binding);
            require_persistent_equal(
                finalized_executable_digest,
                executable.executable().finalized_code_object_digest(),
                "finalized executable",
            )?;
            require_persistent_equal(
                first.executable().target(),
                executable.executable().target(),
                "target",
            )?;
            require_persistent_equal(
                code_object_version,
                executable.executable().code_object_version(),
                "code-object version",
            )?;
            require_persistent_equal(
                first.tool_policy().compiler(),
                executable.tool_policy().compiler(),
                "compiler",
            )?;
            require_persistent_equal(
                first.tool_policy().artifact_producer(),
                executable.tool_policy().artifact_producer(),
                "artifact producer",
            )?;

            let plan = persistent
                .proof_binding()
                .execution_evidence()
                .invocation_plan();
            require_persistent_equal(
                first_plan.tools(),
                plan.tools(),
                "measured verifier toolchain",
            )?;
            require_persistent_equal(
                first_plan.request().configuration(),
                plan.request().configuration(),
                "proof configuration",
            )?;
            require_persistent_equal(
                first_plan.request().model(),
                plan.request().model(),
                "verification model",
            )?;
            require_persistent_equal(
                first_plan.timeout_seconds(),
                plan.timeout_seconds(),
                "verifier timeout policy",
            )?;

            let execution = executable.tool_policy().proof_execution();
            require_persistent_equal(
                first_execution.model(),
                execution.model(),
                "artifact verification model",
            )?;
            require_persistent_equal(
                first_execution.verifier(),
                execution.verifier(),
                "artifact verifier",
            )?;
            require_persistent_equal(
                first_execution.solver(),
                execution.solver(),
                "artifact solver",
            )?;
            require_persistent_equal(
                first_execution.evidence_recorder(),
                execution.evidence_recorder(),
                "artifact evidence recorder",
            )?;
        }

        let binding_identity = persistent_admission_identity(
            finalized_executable_digest,
            code_object_version,
            ledger_namespace,
            &bindings,
        );
        Ok(Self {
            finalized_executable_digest,
            code_object_version,
            ledger_namespace,
            bindings,
            binding_identity,
        })
    }

    pub const fn version(&self) -> u16 {
        PERSISTENT_MULTI_KERNEL_PROOF_ADMISSION_VERSION_V1
    }

    pub const fn finalized_executable_digest(&self) -> PayloadDigest {
        self.finalized_executable_digest
    }

    pub const fn code_object_version(&self) -> ExecutableCodeObjectVersionV1 {
        self.code_object_version
    }

    pub const fn ledger_namespace(&self) -> Digest {
        self.ledger_namespace
    }

    pub fn kernel_count(&self) -> usize {
        self.bindings.len()
    }

    pub const fn binding_identity(&self) -> Digest {
        self.binding_identity
    }

    pub fn admit_kernel(
        &self,
        request: PersistentlyFreshKernelProofAdmissionRequestV1,
    ) -> Result<
        &PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1,
        PersistentlyFreshMultiKernelProofAdmissionErrorV1,
    > {
        let index = self
            .bindings
            .binary_search_by_key(&request.kernel_id, persistent_kernel_id)
            .map_err(
                |_| PersistentlyFreshMultiKernelProofAdmissionErrorV1::UnknownKernel {
                    kernel_id: request.kernel_id,
                },
            )?;
        let binding = &self.bindings[index];
        let admitted = PersistentlyFreshKernelProofAdmissionRequestV1::from_binding(binding);
        for (field, expected, actual) in [
            (
                PersistentlyFreshKernelProofAdmissionIdentityV1::Source,
                admitted.source_identity,
                request.source_identity,
            ),
            (
                PersistentlyFreshKernelProofAdmissionIdentityV1::Contract,
                admitted.contract_identity,
                request.contract_identity,
            ),
            (
                PersistentlyFreshKernelProofAdmissionIdentityV1::ProofRequest,
                admitted.proof_request_identity,
                request.proof_request_identity,
            ),
            (
                PersistentlyFreshKernelProofAdmissionIdentityV1::AuthenticatedProof,
                admitted.authenticated_proof_identity,
                request.authenticated_proof_identity,
            ),
            (
                PersistentlyFreshKernelProofAdmissionIdentityV1::PersistentProof,
                admitted.persistent_proof_identity,
                request.persistent_proof_identity,
            ),
        ] {
            if expected != actual {
                return Err(
                    PersistentlyFreshMultiKernelProofAdmissionErrorV1::IdentityMismatch {
                        kernel_id: request.kernel_id,
                        field,
                    },
                );
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

fn persistent_executable_binding(
    binding: &PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1,
) -> &fe2o3_artifacts::ProofExecutableBindingV1 {
    binding
        .proof_executable_binding()
        .proof_binding()
        .executable_binding()
}

fn persistent_kernel_id(
    binding: &PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1,
) -> Digest {
    binding.request_binding().target().kernel_id
}

fn validate_internal_persistent_kernel_binding(
    binding: &PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1,
) -> Result<(), PersistentlyFreshMultiKernelProofAdmissionErrorV1> {
    let request_kernel = persistent_kernel_id(binding);
    let proof_kernel = persistent_executable_binding(binding)
        .executable()
        .proof_target()
        .artifact()
        .kernel_id();
    if proof_kernel.algorithm() != DigestAlgorithm::Sha256 {
        return Err(PersistentlyFreshMultiKernelProofAdmissionErrorV1::UnsupportedDigestAlgorithm);
    }
    if request_kernel.as_bytes() != proof_kernel.bytes().as_bytes() {
        return Err(
            PersistentlyFreshMultiKernelProofAdmissionErrorV1::InternalKernelMismatch {
                request: request_kernel,
                proof: Digest::from_bytes(*proof_kernel.bytes().as_bytes()),
            },
        );
    }
    Ok(())
}

fn require_persistent_equal<T: PartialEq>(
    expected: T,
    actual: T,
    field: &'static str,
) -> Result<(), PersistentlyFreshMultiKernelProofAdmissionErrorV1> {
    if expected == actual {
        Ok(())
    } else {
        Err(PersistentlyFreshMultiKernelProofAdmissionErrorV1::SharedBundleMismatch { field })
    }
}

fn persistent_admission_identity(
    finalized_executable_digest: PayloadDigest,
    code_object_version: ExecutableCodeObjectVersionV1,
    ledger_namespace: Digest,
    bindings: &[PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1],
) -> Digest {
    let mut bytes = Vec::with_capacity(88 + bindings.len() * 264);
    bytes.extend_from_slice(&PERSISTENT_MULTI_KERNEL_PROOF_ADMISSION_DOMAIN_V1);
    bytes.extend_from_slice(&PERSISTENT_MULTI_KERNEL_PROOF_ADMISSION_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(finalized_executable_digest.bytes().as_bytes());
    bytes.push(code_object_version.number());
    bytes.extend_from_slice(ledger_namespace.as_bytes());
    bytes.extend_from_slice(&(bindings.len() as u16).to_le_bytes());
    for binding in bindings {
        let request = PersistentlyFreshKernelProofAdmissionRequestV1::from_binding(binding);
        for digest in [
            request.kernel_id,
            request.source_identity,
            request.contract_identity,
            request.proof_request_identity,
            request.authenticated_proof_identity,
            request.persistent_proof_identity,
            binding.binding_identity(),
        ] {
            bytes.extend_from_slice(digest.as_bytes());
        }
        let persistent = binding.proof_executable_binding().identity();
        bytes.extend_from_slice(&persistent.ledger_generation().to_le_bytes());
        bytes.extend_from_slice(persistent.ledger_state_identity().as_bytes());
    }
    sha256(&bytes)
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

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PersistentlyFreshMultiKernelProofAdmissionErrorV1 {
    Empty,
    TooManyKernels {
        max: usize,
    },
    UnsupportedDigestAlgorithm,
    DuplicateKernel {
        kernel_id: Digest,
    },
    MixedLedgerNamespace,
    DuplicateLedgerGeneration {
        generation: u64,
    },
    InternalKernelMismatch {
        request: Digest,
        proof: Digest,
    },
    SharedBundleMismatch {
        field: &'static str,
    },
    UnknownKernel {
        kernel_id: Digest,
    },
    IdentityMismatch {
        kernel_id: Digest,
        field: PersistentlyFreshKernelProofAdmissionIdentityV1,
    },
}

impl fmt::Display for PersistentlyFreshMultiKernelProofAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("persistent multi-kernel proof admission is empty"),
            Self::TooManyKernels { max } => write!(
                formatter,
                "persistent multi-kernel proof admission exceeds {max} kernels"
            ),
            Self::UnsupportedDigestAlgorithm => {
                formatter.write_str("persistent multi-kernel proof admission requires SHA-256")
            }
            Self::DuplicateKernel { kernel_id } => write!(
                formatter,
                "kernel {} has duplicate persistent proof evidence",
                kernel_id.to_hex()
            ),
            Self::MixedLedgerNamespace => formatter.write_str(
                "persistent multi-kernel proof admission spans multiple ledger namespaces",
            ),
            Self::DuplicateLedgerGeneration { generation } => write!(
                formatter,
                "persistent freshness generation {generation} is used by multiple kernels"
            ),
            Self::InternalKernelMismatch { request, proof } => write!(
                formatter,
                "persistent proof kernel {} does not match request kernel {}",
                proof.to_hex(),
                request.to_hex()
            ),
            Self::SharedBundleMismatch { field } => write!(
                formatter,
                "persistent multi-kernel {field} identity does not match"
            ),
            Self::UnknownKernel { kernel_id } => write!(
                formatter,
                "kernel {} has no persistently fresh admitted proof",
                kernel_id.to_hex()
            ),
            Self::IdentityMismatch { kernel_id, field } => write!(
                formatter,
                "kernel {} {} identity does not match persistent admission",
                kernel_id.to_hex(),
                field.as_str()
            ),
        }
    }
}

impl std::error::Error for PersistentlyFreshMultiKernelProofAdmissionErrorV1 {}
