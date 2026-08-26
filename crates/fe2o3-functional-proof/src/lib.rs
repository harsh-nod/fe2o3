#![forbid(unsafe_code)]

//! Strict import boundary for workload-neutral functional-refinement proof receipts.
//!
//! A receipt becomes [`ImportedFunctionalRefinementProofV2`] only after an import policy pins the
//! signer, complete Verus/solver toolchain, and covered boundary; Ed25519 authenticates the
//! canonical fixed-width message; and the message matches the supplied reference, kernel MIR, and
//! normalized obligation/effect identities. Import proves those checks passed under that policy.
//! It does not establish that the policy or expected identities came from compiler custody.
//!
//! Compiler authority therefore requires a separate, private join to compiler-retained rustc MIR
//! and compiler configuration. In that join, Verus proves compiler-derived effect formulas only
//! conditional on the trusted MIR-to-effect extractor and the exact numeric model encoded by the
//! generator. No receipt in this crate proves a full MIR operational-semantics theorem, lowering,
//! LLVM/ISA correspondence, artifact integrity, loading, launch, runtime behavior, or hardware
//! execution.

mod mir_pliron_semantic_contract_v1;
mod parallel_reference_contract_v1;

pub use mir_pliron_semantic_contract_v1::{
    HARD_MAX_SEMANTIC_COLLECTIVES_V1, HARD_MAX_SEMANTIC_DOMAINS_V1, HARD_MAX_SEMANTIC_LOOPS_V1,
    HARD_MAX_SEMANTIC_OUTPUTS_V1, HARD_MAX_SEMANTIC_ROOTS_V1, MirPlironSemanticContractErrorV1,
    MirPlironSemanticContractV1, SemanticCollectiveContractV1, SemanticCollectiveKindV1,
    SemanticCoverageBindingV1, SemanticEvaluationOrderV1, SemanticFiniteDomainV1,
    SemanticFiniteExtentV1, SemanticIeeeExceptionalValueV1, SemanticIeeeRoundingV1,
    SemanticLoopContractV1, SemanticLoopDirectionV1, SemanticNumericalPolicyV1,
    SemanticOutputContractV1, SemanticScalarTypeV1, SemanticTypedRootV1,
};
pub use parallel_reference_contract_v1::{
    COMPLETE_GPU_HIERARCHY_V1, HARD_MAX_AGGREGATE_FUNCTIONAL_OUTPUTS_V1,
    HARD_MAX_PARALLEL_CALL_ARGUMENTS_V1, HARD_MAX_PARALLEL_OUTPUT_RELATIONS_V1,
    ParallelFoldOrderV1, ParallelHierarchyLevelV1, ParallelNumericalPolicyV1,
    ParallelOutputRelationV1, ParallelReferenceContractErrorV1, ParallelReferenceContractV1,
    ParallelScheduleRelationV1,
};

use std::{error::Error, fmt};

#[cfg(any(test, feature = "internal-proof-staging"))]
use ed25519_dalek::{Signature, VerifyingKey};
use fe2o3_proof_contracts::DigestV1;
#[cfg(any(test, feature = "internal-proof-staging"))]
use sha2::{Digest, Sha256};
#[cfg(any(test, feature = "internal-proof-staging"))]
use std::collections::BTreeSet;

pub const FUNCTIONAL_REFINEMENT_RECEIPT_VERSION_V2: u16 = 2;
pub const FUNCTIONAL_REFINEMENT_RECEIPT_MAGIC_V2: [u8; 8] = *b"F2FRPV2\0";
pub const FUNCTIONAL_REFINEMENT_SIGNATURE_BYTES_V2: usize = 64;
pub const HARD_MAX_IMPORTED_FUNCTIONAL_REFINEMENT_RECEIPTS_V2: usize = 4_096;

#[cfg(any(test, feature = "internal-proof-staging"))]
const SIGNER_DOMAIN_V2: &[u8] = b"FE2O3/FUNCTIONAL-REFINEMENT/SIGNER/V2\0";
#[cfg(any(test, feature = "internal-proof-staging"))]
const RECEIPT_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/FUNCTIONAL-REFINEMENT/RECEIPT/V2\0";
const HEADER_BYTES_V2: usize = 8 + 2 + 1 + 1;
const DIGEST_FIELD_COUNT_V2: usize = 14;
const SIGNED_MESSAGE_BYTES_V2: usize = HEADER_BYTES_V2 + DIGEST_FIELD_COUNT_V2 * 32;
pub const FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2: usize =
    SIGNED_MESSAGE_BYTES_V2 + FUNCTIONAL_REFINEMENT_SIGNATURE_BYTES_V2;

/// Which safe-reference inputs the receipt claims were selected.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SafeReferenceKindV2 {
    SourceAndMir = 1,
    Mir = 2,
}

impl SafeReferenceKindV2 {
    #[cfg(any(test, feature = "internal-proof-staging"))]
    fn decode(value: u8) -> Result<Self, FunctionalRefinementImportErrorV2> {
        match value {
            1 => Ok(Self::SourceAndMir),
            2 => Ok(Self::Mir),
            value => Err(FunctionalRefinementImportErrorV2::UnknownReferenceKind(
                value,
            )),
        }
    }
}

/// Exact semantic boundary claimed by the proof and no later compiler stage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum FunctionalRefinementBoundaryV2 {
    SafeReferenceMirToKernelMir = 1,
    SafeReferenceSourceToKernelMir = 2,
    SafeReferenceMirToLivePliron = 3,
}

impl FunctionalRefinementBoundaryV2 {
    #[cfg(any(test, feature = "internal-proof-staging"))]
    fn decode(value: u8) -> Result<Self, FunctionalRefinementImportErrorV2> {
        match value {
            1 => Ok(Self::SafeReferenceMirToKernelMir),
            2 => Ok(Self::SafeReferenceSourceToKernelMir),
            3 => Ok(Self::SafeReferenceMirToLivePliron),
            value => Err(FunctionalRefinementImportErrorV2::UnknownBoundary(value)),
        }
    }
}

/// Verus execution result claimed by the signed receipt.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum FunctionalRefinementResultV2 {
    Proved = 1,
    Failed = 2,
    Inconclusive = 3,
}

impl FunctionalRefinementResultV2 {
    #[cfg(any(test, feature = "internal-proof-staging"))]
    fn decode(value: u8) -> Result<Self, FunctionalRefinementImportErrorV2> {
        match value {
            1 => Ok(Self::Proved),
            2 => Ok(Self::Failed),
            3 => Ok(Self::Inconclusive),
            value => Err(FunctionalRefinementImportErrorV2::UnknownResult(value)),
        }
    }
}

/// Complete receipt-bound identity of the Verus and solver execution closure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VerusToolchainIdentityV2 {
    verus_executable: DigestV1,
    verus_configuration: DigestV1,
    solver_executable: DigestV1,
    solver_configuration: DigestV1,
    runtime_closure: DigestV1,
}

impl VerusToolchainIdentityV2 {
    pub fn new(
        verus_executable: DigestV1,
        verus_configuration: DigestV1,
        solver_executable: DigestV1,
        solver_configuration: DigestV1,
        runtime_closure: DigestV1,
    ) -> Result<Self, FunctionalRefinementImportErrorV2> {
        let result = Self {
            verus_executable,
            verus_configuration,
            solver_executable,
            solver_configuration,
            runtime_closure,
        };
        result.validate()?;
        Ok(result)
    }

    pub const fn verus_executable(&self) -> DigestV1 {
        self.verus_executable
    }
    pub const fn verus_configuration(&self) -> DigestV1 {
        self.verus_configuration
    }
    pub const fn solver_executable(&self) -> DigestV1 {
        self.solver_executable
    }
    pub const fn solver_configuration(&self) -> DigestV1 {
        self.solver_configuration
    }
    pub const fn runtime_closure(&self) -> DigestV1 {
        self.runtime_closure
    }

    fn validate(&self) -> Result<(), FunctionalRefinementImportErrorV2> {
        for (field, digest) in [
            ("Verus executable", self.verus_executable),
            ("Verus configuration", self.verus_configuration),
            ("solver executable", self.solver_executable),
            ("solver configuration", self.solver_configuration),
            ("runtime closure", self.runtime_closure),
        ] {
            require_digest(field, digest)?;
        }
        Ok(())
    }
}

/// Exact workload-neutral statement encoded in a V2 receipt.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionalRefinementBindingV2 {
    safe_reference_kind: SafeReferenceKindV2,
    safe_reference_identity: DigestV1,
    safe_reference_source_hash: DigestV1,
    safe_reference_mir_hash: DigestV1,
    kernel_subject_identity: DigestV1,
    kernel_mir_hash: DigestV1,
    normalized_obligation_effect_ir_hash: DigestV1,
}

/// Exact current subjects used before the normalized obligation/effect digest is derived.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionalRefinementSubjectsV2 {
    safe_reference_kind: SafeReferenceKindV2,
    safe_reference_identity: DigestV1,
    safe_reference_source_hash: DigestV1,
    safe_reference_mir_hash: DigestV1,
    kernel_subject_identity: DigestV1,
    kernel_mir_hash: DigestV1,
}

impl FunctionalRefinementSubjectsV2 {
    pub fn new(
        safe_reference_kind: SafeReferenceKindV2,
        safe_reference_identity: DigestV1,
        safe_reference_source_hash: DigestV1,
        safe_reference_mir_hash: DigestV1,
        kernel_subject_identity: DigestV1,
        kernel_mir_hash: DigestV1,
    ) -> Result<Self, FunctionalRefinementImportErrorV2> {
        let subjects = Self {
            safe_reference_kind,
            safe_reference_identity,
            safe_reference_source_hash,
            safe_reference_mir_hash,
            kernel_subject_identity,
            kernel_mir_hash,
        };
        subjects.validate()?;
        Ok(subjects)
    }

    pub const fn safe_reference_kind(&self) -> SafeReferenceKindV2 {
        self.safe_reference_kind
    }
    pub const fn safe_reference_identity(&self) -> DigestV1 {
        self.safe_reference_identity
    }
    pub const fn safe_reference_source_hash(&self) -> DigestV1 {
        self.safe_reference_source_hash
    }
    pub const fn safe_reference_mir_hash(&self) -> DigestV1 {
        self.safe_reference_mir_hash
    }
    pub const fn kernel_subject_identity(&self) -> DigestV1 {
        self.kernel_subject_identity
    }
    pub const fn kernel_mir_hash(&self) -> DigestV1 {
        self.kernel_mir_hash
    }

    fn validate(&self) -> Result<(), FunctionalRefinementImportErrorV2> {
        require_digest("safe-reference identity", self.safe_reference_identity)?;
        if self.safe_reference_kind == SafeReferenceKindV2::SourceAndMir {
            require_digest(
                "safe-reference source hash",
                self.safe_reference_source_hash,
            )?;
        } else if !self.safe_reference_source_hash.is_zero() {
            return Err(FunctionalRefinementImportErrorV2::NonCanonicalAbsentSourceHash);
        }
        require_digest("safe-reference MIR hash", self.safe_reference_mir_hash)?;
        require_digest("kernel subject identity", self.kernel_subject_identity)?;
        require_digest("kernel MIR hash", self.kernel_mir_hash)?;
        Ok(())
    }
}

impl FunctionalRefinementBindingV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        safe_reference_kind: SafeReferenceKindV2,
        safe_reference_identity: DigestV1,
        safe_reference_source_hash: DigestV1,
        safe_reference_mir_hash: DigestV1,
        kernel_subject_identity: DigestV1,
        kernel_mir_hash: DigestV1,
        normalized_obligation_effect_ir_hash: DigestV1,
    ) -> Result<Self, FunctionalRefinementImportErrorV2> {
        let subjects = FunctionalRefinementSubjectsV2::new(
            safe_reference_kind,
            safe_reference_identity,
            safe_reference_source_hash,
            safe_reference_mir_hash,
            kernel_subject_identity,
            kernel_mir_hash,
        )?;
        Self::from_subjects(subjects, normalized_obligation_effect_ir_hash)
    }

    pub fn from_subjects(
        subjects: FunctionalRefinementSubjectsV2,
        normalized_obligation_effect_ir_hash: DigestV1,
    ) -> Result<Self, FunctionalRefinementImportErrorV2> {
        subjects.validate()?;
        let result = Self {
            safe_reference_kind: subjects.safe_reference_kind,
            safe_reference_identity: subjects.safe_reference_identity,
            safe_reference_source_hash: subjects.safe_reference_source_hash,
            safe_reference_mir_hash: subjects.safe_reference_mir_hash,
            kernel_subject_identity: subjects.kernel_subject_identity,
            kernel_mir_hash: subjects.kernel_mir_hash,
            normalized_obligation_effect_ir_hash,
        };
        result.validate()?;
        Ok(result)
    }

    pub const fn safe_reference_kind(&self) -> SafeReferenceKindV2 {
        self.safe_reference_kind
    }
    pub const fn safe_reference_identity(&self) -> DigestV1 {
        self.safe_reference_identity
    }
    pub const fn safe_reference_source_hash(&self) -> DigestV1 {
        self.safe_reference_source_hash
    }
    pub const fn safe_reference_mir_hash(&self) -> DigestV1 {
        self.safe_reference_mir_hash
    }
    pub const fn kernel_subject_identity(&self) -> DigestV1 {
        self.kernel_subject_identity
    }
    pub const fn kernel_mir_hash(&self) -> DigestV1 {
        self.kernel_mir_hash
    }
    pub const fn normalized_obligation_effect_ir_hash(&self) -> DigestV1 {
        self.normalized_obligation_effect_ir_hash
    }
    pub const fn subjects(&self) -> FunctionalRefinementSubjectsV2 {
        FunctionalRefinementSubjectsV2 {
            safe_reference_kind: self.safe_reference_kind,
            safe_reference_identity: self.safe_reference_identity,
            safe_reference_source_hash: self.safe_reference_source_hash,
            safe_reference_mir_hash: self.safe_reference_mir_hash,
            kernel_subject_identity: self.kernel_subject_identity,
            kernel_mir_hash: self.kernel_mir_hash,
        }
    }

    fn validate(&self) -> Result<(), FunctionalRefinementImportErrorV2> {
        require_digest("safe-reference identity", self.safe_reference_identity)?;
        if self.safe_reference_kind == SafeReferenceKindV2::SourceAndMir {
            require_digest(
                "safe-reference source hash",
                self.safe_reference_source_hash,
            )?;
        } else if !self.safe_reference_source_hash.is_zero() {
            return Err(FunctionalRefinementImportErrorV2::NonCanonicalAbsentSourceHash);
        }
        require_digest("safe-reference MIR hash", self.safe_reference_mir_hash)?;
        require_digest("kernel subject identity", self.kernel_subject_identity)?;
        require_digest("kernel MIR hash", self.kernel_mir_hash)?;
        require_digest(
            "normalized obligation/effect IR hash",
            self.normalized_obligation_effect_ir_hash,
        )?;
        Ok(())
    }
}

/// Caller-supplied signer, proof-environment, and boundary import policy.
///
/// Constructing this policy does not make it a compiler trust root. Compiler production must
/// obtain its policy from private compiler configuration and retain that custody separately.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg(any(test, feature = "internal-proof-staging"))]
pub struct FunctionalRefinementImportPolicyV2 {
    verifying_key: VerifyingKey,
    signer_identity: DigestV1,
    toolchain: VerusToolchainIdentityV2,
    boundary: FunctionalRefinementBoundaryV2,
}

#[cfg(any(test, feature = "internal-proof-staging"))]
impl FunctionalRefinementImportPolicyV2 {
    pub fn new(
        verifying_key: [u8; 32],
        toolchain: VerusToolchainIdentityV2,
        boundary: FunctionalRefinementBoundaryV2,
    ) -> Result<Self, FunctionalRefinementImportErrorV2> {
        toolchain.validate()?;
        let verifying_key = VerifyingKey::from_bytes(&verifying_key)
            .map_err(|_| FunctionalRefinementImportErrorV2::InvalidVerifyingKey)?;
        if verifying_key.is_weak() {
            return Err(FunctionalRefinementImportErrorV2::WeakVerifyingKey);
        }
        let signer_identity = signer_identity(verifying_key.as_bytes());
        Ok(Self {
            verifying_key,
            signer_identity,
            toolchain,
            boundary,
        })
    }

    pub const fn signer_identity(&self) -> DigestV1 {
        self.signer_identity
    }
    pub const fn toolchain(&self) -> VerusToolchainIdentityV2 {
        self.toolchain
    }
    pub const fn boundary(&self) -> FunctionalRefinementBoundaryV2 {
        self.boundary
    }
}

/// Canonical unsigned message builder for a functional-refinement receipt.
///
/// This low-level constructor is available only to workspace verifier staging and tests.
/// Even there, callers can choose every field, construct a message, and attach bytes;
/// the result is non-authoritative. A production signer must accept messages only from its
/// private verifier-owned successful-execution join, and a compiler must independently retain
/// rustc custody before admitting the resulting receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg(any(test, feature = "internal-proof-staging"))]
pub struct UnsignedFunctionalRefinementReceiptV2 {
    message: [u8; SIGNED_MESSAGE_BYTES_V2],
}

#[cfg(any(test, feature = "internal-proof-staging"))]
impl UnsignedFunctionalRefinementReceiptV2 {
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn from_verified_execution_join(
        signer_identity: DigestV1,
        binding: FunctionalRefinementBindingV2,
        toolchain: VerusToolchainIdentityV2,
        execution_identity: DigestV1,
        result: FunctionalRefinementResultV2,
        boundary: FunctionalRefinementBoundaryV2,
    ) -> Result<Self, FunctionalRefinementImportErrorV2> {
        require_digest("signer identity", signer_identity)?;
        binding.validate()?;
        toolchain.validate()?;
        require_digest("execution identity", execution_identity)?;
        let mut writer = WireWriterV2::new();
        writer.bytes(&FUNCTIONAL_REFINEMENT_RECEIPT_MAGIC_V2);
        writer.bytes(&FUNCTIONAL_REFINEMENT_RECEIPT_VERSION_V2.to_le_bytes());
        writer.byte(result as u8);
        writer.byte(boundary as u8);
        for digest in [
            signer_identity,
            binding.safe_reference_identity,
            binding.safe_reference_source_hash,
            binding.safe_reference_mir_hash,
            binding.kernel_subject_identity,
            binding.kernel_mir_hash,
            binding.normalized_obligation_effect_ir_hash,
            toolchain.verus_executable,
            toolchain.verus_configuration,
            toolchain.solver_executable,
            toolchain.solver_configuration,
            toolchain.runtime_closure,
            execution_identity,
        ] {
            writer.digest(digest);
        }
        writer.byte(binding.safe_reference_kind as u8);
        writer.zeros(31);
        Ok(Self {
            message: writer.finish(),
        })
    }

    pub const fn signing_bytes(&self) -> &[u8; SIGNED_MESSAGE_BYTES_V2] {
        &self.message
    }

    /// Attaches untrusted signature bytes. Only the strict importer authenticates them.
    pub fn attach_signature(
        self,
        signature: [u8; FUNCTIONAL_REFINEMENT_SIGNATURE_BYTES_V2],
    ) -> [u8; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2] {
        let mut wire = [0; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2];
        wire[..SIGNED_MESSAGE_BYTES_V2].copy_from_slice(&self.message);
        wire[SIGNED_MESSAGE_BYTES_V2..].copy_from_slice(&signature);
        wire
    }
}

/// Caller-supplied expected inputs against which one signed receipt is imported.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(test, feature = "internal-proof-staging"))]
pub struct FunctionalRefinementImportExpectationV2 {
    binding: FunctionalRefinementBindingV2,
}

#[cfg(any(test, feature = "internal-proof-staging"))]
impl FunctionalRefinementImportExpectationV2 {
    pub const fn new(binding: FunctionalRefinementBindingV2) -> Self {
        Self { binding }
    }
    pub const fn binding(&self) -> FunctionalRefinementBindingV2 {
        self.binding
    }
}

/// Domain-separated identity of the signed statement, independent of signature representation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionalRefinementReceiptIdentityV2(DigestV1);

impl FunctionalRefinementReceiptIdentityV2 {
    pub const fn digest(self) -> DigestV1 {
        self.0
    }
}

/// Move-only receipt that passed one exact import policy and binding expectation.
///
/// This value authenticates the receipt bytes relative to caller-supplied policy and expectation.
/// It is not a compiler-authenticated MIR-refinement capability. Only a private compiler join may
/// combine it with retained rustc custody and decide whether it is admissible for production.
///
/// ```compile_fail
/// use fe2o3_functional_proof::ImportedFunctionalRefinementProofV2;
/// fn duplicate(proof: ImportedFunctionalRefinementProofV2) {
///     let _copy = proof.clone();
/// }
/// ```
///
/// A caller-verified receipt exposes no compiler-authority capability:
///
/// ```compile_fail
/// use fe2o3_functional_proof::ImportedFunctionalRefinementProofV2;
/// fn claim_compiler_authority(proof: &ImportedFunctionalRefinementProofV2) {
///     assert!(proof.grants_functional_refinement_evidence());
/// }
/// ```
#[must_use = "a policy-verified functional-refinement receipt must be consumed or discarded"]
#[derive(Debug, Eq, PartialEq)]
pub struct ImportedFunctionalRefinementProofV2 {
    receipt_identity: FunctionalRefinementReceiptIdentityV2,
    signer_identity: DigestV1,
    binding: FunctionalRefinementBindingV2,
    toolchain: VerusToolchainIdentityV2,
    execution_identity: DigestV1,
    boundary: FunctionalRefinementBoundaryV2,
}

impl ImportedFunctionalRefinementProofV2 {
    pub const fn receipt_identity(&self) -> FunctionalRefinementReceiptIdentityV2 {
        self.receipt_identity
    }
    pub const fn signer_identity(&self) -> DigestV1 {
        self.signer_identity
    }
    pub const fn binding(&self) -> FunctionalRefinementBindingV2 {
        self.binding
    }
    pub const fn toolchain(&self) -> VerusToolchainIdentityV2 {
        self.toolchain
    }
    pub const fn execution_identity(&self) -> DigestV1 {
        self.execution_identity
    }
    pub const fn boundary(&self) -> FunctionalRefinementBoundaryV2 {
        self.boundary
    }
    /// Reports that signature, result, policy, boundary, and expected binding checks succeeded.
    ///
    /// This is deliberately a policy-verification fact, not compiler refinement authority.
    pub const fn signature_and_policy_verified(&self) -> bool {
        true
    }
}

/// Bounded stateful importer. Successful receipt identities cannot be imported twice.
#[cfg(any(test, feature = "internal-proof-staging"))]
pub struct FunctionalRefinementReceiptImporterV2 {
    policy: FunctionalRefinementImportPolicyV2,
    max_receipts: usize,
    imported: BTreeSet<FunctionalRefinementReceiptIdentityV2>,
}

#[cfg(any(test, feature = "internal-proof-staging"))]
impl FunctionalRefinementReceiptImporterV2 {
    pub fn new(
        policy: FunctionalRefinementImportPolicyV2,
        max_receipts: usize,
    ) -> Result<Self, FunctionalRefinementImportErrorV2> {
        if max_receipts == 0 || max_receipts > HARD_MAX_IMPORTED_FUNCTIONAL_REFINEMENT_RECEIPTS_V2 {
            return Err(FunctionalRefinementImportErrorV2::InvalidReceiptLimit {
                actual: max_receipts,
                hard_max: HARD_MAX_IMPORTED_FUNCTIONAL_REFINEMENT_RECEIPTS_V2,
            });
        }
        Ok(Self {
            policy,
            max_receipts,
            imported: BTreeSet::new(),
        })
    }

    pub fn import(
        &mut self,
        expectation: FunctionalRefinementImportExpectationV2,
        wire: &[u8],
    ) -> Result<ImportedFunctionalRefinementProofV2, FunctionalRefinementImportErrorV2> {
        if wire.len() != FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2 {
            return Err(FunctionalRefinementImportErrorV2::WrongWireLength {
                expected: FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2,
                actual: wire.len(),
            });
        }
        let message: &[u8; SIGNED_MESSAGE_BYTES_V2] = wire[..SIGNED_MESSAGE_BYTES_V2]
            .try_into()
            .map_err(|_| FunctionalRefinementImportErrorV2::WrongWireLength {
                expected: FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2,
                actual: wire.len(),
            })?;
        let decoded = decode_message(message)?;
        let receipt_identity = receipt_identity(message);
        if self.imported.contains(&receipt_identity) {
            return Err(FunctionalRefinementImportErrorV2::DuplicateReceipt(
                receipt_identity,
            ));
        }
        if self.imported.len() >= self.max_receipts {
            return Err(FunctionalRefinementImportErrorV2::ReceiptLimitExceeded);
        }
        if decoded.signer_identity != self.policy.signer_identity {
            return Err(FunctionalRefinementImportErrorV2::WrongSigner);
        }
        let signature_bytes: [u8; FUNCTIONAL_REFINEMENT_SIGNATURE_BYTES_V2] = wire
            [SIGNED_MESSAGE_BYTES_V2..]
            .try_into()
            .map_err(|_| FunctionalRefinementImportErrorV2::WrongWireLength {
                expected: FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2,
                actual: wire.len(),
            })?;
        self.policy
            .verifying_key
            .verify_strict(message, &Signature::from_bytes(&signature_bytes))
            .map_err(|_| FunctionalRefinementImportErrorV2::SignatureRejected)?;
        if decoded.result != FunctionalRefinementResultV2::Proved {
            return Err(FunctionalRefinementImportErrorV2::ResultNotProved(
                decoded.result,
            ));
        }
        if decoded.boundary != self.policy.boundary {
            return Err(FunctionalRefinementImportErrorV2::WrongBoundary);
        }
        if decoded.toolchain != self.policy.toolchain {
            return Err(FunctionalRefinementImportErrorV2::WrongToolchain);
        }
        if decoded.binding != expectation.binding {
            return Err(binding_mismatch(decoded.binding, expectation.binding));
        }
        self.imported.insert(receipt_identity);
        Ok(ImportedFunctionalRefinementProofV2 {
            receipt_identity,
            signer_identity: decoded.signer_identity,
            binding: decoded.binding,
            toolchain: decoded.toolchain,
            execution_identity: decoded.execution_identity,
            boundary: decoded.boundary,
        })
    }

    pub fn imported_count(&self) -> usize {
        self.imported.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(test, feature = "internal-proof-staging"))]
struct DecodedReceiptV2 {
    signer_identity: DigestV1,
    binding: FunctionalRefinementBindingV2,
    toolchain: VerusToolchainIdentityV2,
    execution_identity: DigestV1,
    result: FunctionalRefinementResultV2,
    boundary: FunctionalRefinementBoundaryV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FunctionalRefinementImportErrorV2 {
    InvalidReceiptLimit { actual: usize, hard_max: usize },
    WrongWireLength { expected: usize, actual: usize },
    WrongMagic,
    WrongVersion(u16),
    UnknownReferenceKind(u8),
    UnknownResult(u8),
    UnknownBoundary(u8),
    InvalidVerifyingKey,
    WeakVerifyingKey,
    ZeroDigest(&'static str),
    NonCanonicalAbsentSourceHash,
    NonCanonicalReservedBytes,
    WrongSigner,
    SignatureRejected,
    ResultNotProved(FunctionalRefinementResultV2),
    WrongBoundary,
    WrongToolchain,
    StaleSafeReferenceIdentity,
    StaleSafeReferenceSource,
    StaleSafeReferenceMir,
    StaleKernelSubject,
    StaleKernelMir,
    StaleNormalizedObligationEffectIr,
    DuplicateReceipt(FunctionalRefinementReceiptIdentityV2),
    ReceiptLimitExceeded,
}

impl fmt::Display for FunctionalRefinementImportErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidReceiptLimit { actual, hard_max } => write!(
                formatter,
                "functional-refinement receipt limit {actual} is outside 1..={hard_max}"
            ),
            Self::WrongWireLength { expected, actual } => write!(
                formatter,
                "functional-refinement receipt length {actual} does not equal {expected}"
            ),
            Self::WrongMagic => formatter.write_str("functional-refinement receipt magic mismatch"),
            Self::WrongVersion(version) => write!(
                formatter,
                "unsupported functional-refinement receipt version {version}"
            ),
            Self::UnknownReferenceKind(kind) => {
                write!(formatter, "unknown safe-reference input kind {kind}")
            }
            Self::UnknownResult(result) => {
                write!(formatter, "unknown functional-refinement result {result}")
            }
            Self::UnknownBoundary(boundary) => write!(
                formatter,
                "unknown functional-refinement boundary {boundary}"
            ),
            Self::InvalidVerifyingKey => formatter.write_str("invalid Ed25519 verifying key"),
            Self::WeakVerifyingKey => formatter.write_str("weak Ed25519 verifying key"),
            Self::ZeroDigest(field) => write!(formatter, "{field} is the reserved zero digest"),
            Self::NonCanonicalAbsentSourceHash => formatter
                .write_str("MIR-only safe reference must encode an all-zero absent source hash"),
            Self::NonCanonicalReservedBytes => {
                formatter.write_str("functional-refinement receipt reserved bytes are nonzero")
            }
            Self::WrongSigner => formatter.write_str("functional-refinement signer mismatch"),
            Self::SignatureRejected => {
                formatter.write_str("functional-refinement Ed25519 signature rejected")
            }
            Self::ResultNotProved(result) => write!(
                formatter,
                "functional-refinement receipt result is {result:?}, not Proved"
            ),
            Self::WrongBoundary => {
                formatter.write_str("functional-refinement covered boundary mismatch")
            }
            Self::WrongToolchain => {
                formatter.write_str("functional-refinement Verus/solver toolchain mismatch")
            }
            Self::StaleSafeReferenceIdentity => {
                formatter.write_str("safe-reference identity is stale")
            }
            Self::StaleSafeReferenceSource => {
                formatter.write_str("safe-reference source hash is stale")
            }
            Self::StaleSafeReferenceMir => formatter.write_str("safe-reference MIR hash is stale"),
            Self::StaleKernelSubject => formatter.write_str("kernel subject identity is stale"),
            Self::StaleKernelMir => formatter.write_str("kernel MIR hash is stale"),
            Self::StaleNormalizedObligationEffectIr => {
                formatter.write_str("normalized obligation/effect IR hash is stale")
            }
            Self::DuplicateReceipt(_) => {
                formatter.write_str("functional-refinement receipt was already imported")
            }
            Self::ReceiptLimitExceeded => {
                formatter.write_str("functional-refinement receipt import limit exceeded")
            }
        }
    }
}

impl Error for FunctionalRefinementImportErrorV2 {}

#[cfg(any(test, feature = "internal-proof-staging"))]
fn decode_message(
    message: &[u8; SIGNED_MESSAGE_BYTES_V2],
) -> Result<DecodedReceiptV2, FunctionalRefinementImportErrorV2> {
    let mut reader = WireReaderV2::new(message);
    if reader.bytes::<8>() != FUNCTIONAL_REFINEMENT_RECEIPT_MAGIC_V2 {
        return Err(FunctionalRefinementImportErrorV2::WrongMagic);
    }
    let version = u16::from_le_bytes(reader.bytes::<2>());
    if version != FUNCTIONAL_REFINEMENT_RECEIPT_VERSION_V2 {
        return Err(FunctionalRefinementImportErrorV2::WrongVersion(version));
    }
    let result = FunctionalRefinementResultV2::decode(reader.byte())?;
    let boundary = FunctionalRefinementBoundaryV2::decode(reader.byte())?;
    let signer_identity = reader.digest();
    let safe_reference_identity = reader.digest();
    let safe_reference_source_hash = reader.digest();
    let safe_reference_mir_hash = reader.digest();
    let kernel_subject_identity = reader.digest();
    let kernel_mir_hash = reader.digest();
    let normalized_obligation_effect_ir_hash = reader.digest();
    let toolchain = VerusToolchainIdentityV2::new(
        reader.digest(),
        reader.digest(),
        reader.digest(),
        reader.digest(),
        reader.digest(),
    )?;
    let execution_identity = reader.digest();
    let safe_reference_kind = SafeReferenceKindV2::decode(reader.byte())?;
    if reader.bytes::<31>() != [0; 31] {
        return Err(FunctionalRefinementImportErrorV2::NonCanonicalReservedBytes);
    }
    require_digest("signer identity", signer_identity)?;
    require_digest("execution identity", execution_identity)?;
    let binding = FunctionalRefinementBindingV2::new(
        safe_reference_kind,
        safe_reference_identity,
        safe_reference_source_hash,
        safe_reference_mir_hash,
        kernel_subject_identity,
        kernel_mir_hash,
        normalized_obligation_effect_ir_hash,
    )?;
    Ok(DecodedReceiptV2 {
        signer_identity,
        binding,
        toolchain,
        execution_identity,
        result,
        boundary,
    })
}

#[cfg(any(test, feature = "internal-proof-staging"))]
fn binding_mismatch(
    actual: FunctionalRefinementBindingV2,
    expected: FunctionalRefinementBindingV2,
) -> FunctionalRefinementImportErrorV2 {
    if actual.safe_reference_kind != expected.safe_reference_kind
        || actual.safe_reference_identity != expected.safe_reference_identity
    {
        FunctionalRefinementImportErrorV2::StaleSafeReferenceIdentity
    } else if actual.safe_reference_source_hash != expected.safe_reference_source_hash {
        FunctionalRefinementImportErrorV2::StaleSafeReferenceSource
    } else if actual.safe_reference_mir_hash != expected.safe_reference_mir_hash {
        FunctionalRefinementImportErrorV2::StaleSafeReferenceMir
    } else if actual.kernel_subject_identity != expected.kernel_subject_identity {
        FunctionalRefinementImportErrorV2::StaleKernelSubject
    } else if actual.kernel_mir_hash != expected.kernel_mir_hash {
        FunctionalRefinementImportErrorV2::StaleKernelMir
    } else {
        FunctionalRefinementImportErrorV2::StaleNormalizedObligationEffectIr
    }
}

fn require_digest(
    field: &'static str,
    digest: DigestV1,
) -> Result<(), FunctionalRefinementImportErrorV2> {
    if digest.is_zero() {
        Err(FunctionalRefinementImportErrorV2::ZeroDigest(field))
    } else {
        Ok(())
    }
}

#[cfg(any(test, feature = "internal-proof-staging"))]
fn signer_identity(verifying_key: &[u8; 32]) -> DigestV1 {
    domain_digest(SIGNER_DOMAIN_V2, verifying_key)
}

#[cfg(any(test, feature = "internal-proof-staging"))]
fn receipt_identity(
    message: &[u8; SIGNED_MESSAGE_BYTES_V2],
) -> FunctionalRefinementReceiptIdentityV2 {
    FunctionalRefinementReceiptIdentityV2(domain_digest(RECEIPT_IDENTITY_DOMAIN_V2, message))
}

#[cfg(any(test, feature = "internal-proof-staging"))]
fn domain_digest(domain: &[u8], bytes: &[u8]) -> DigestV1 {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_le_bytes());
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

#[cfg(any(test, feature = "internal-proof-staging"))]
struct WireWriterV2 {
    bytes: [u8; SIGNED_MESSAGE_BYTES_V2],
    offset: usize,
}

#[cfg(any(test, feature = "internal-proof-staging"))]
impl WireWriterV2 {
    fn new() -> Self {
        Self {
            bytes: [0; SIGNED_MESSAGE_BYTES_V2],
            offset: 0,
        }
    }
    fn byte(&mut self, value: u8) {
        self.bytes[self.offset] = value;
        self.offset += 1;
    }
    fn bytes(&mut self, value: &[u8]) {
        let end = self.offset + value.len();
        self.bytes[self.offset..end].copy_from_slice(value);
        self.offset = end;
    }
    fn digest(&mut self, value: DigestV1) {
        self.bytes(value.as_bytes());
    }
    fn zeros(&mut self, count: usize) {
        self.offset += count;
    }
    fn finish(self) -> [u8; SIGNED_MESSAGE_BYTES_V2] {
        debug_assert_eq!(self.offset, SIGNED_MESSAGE_BYTES_V2);
        self.bytes
    }
}

#[cfg(any(test, feature = "internal-proof-staging"))]
struct WireReaderV2<'a> {
    bytes: &'a [u8; SIGNED_MESSAGE_BYTES_V2],
    offset: usize,
}

#[cfg(any(test, feature = "internal-proof-staging"))]
impl<'a> WireReaderV2<'a> {
    fn new(bytes: &'a [u8; SIGNED_MESSAGE_BYTES_V2]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn byte(&mut self) -> u8 {
        let result = self.bytes[self.offset];
        self.offset += 1;
        result
    }
    fn bytes<const N: usize>(&mut self) -> [u8; N] {
        let end = self.offset + N;
        let result = self.bytes[self.offset..end]
            .try_into()
            .expect("fixed-width receipt reader stays within canonical message");
        self.offset = end;
        result
    }
    fn digest(&mut self) -> DigestV1 {
        DigestV1::from_untrusted_bytes(self.bytes::<32>())
    }
}
