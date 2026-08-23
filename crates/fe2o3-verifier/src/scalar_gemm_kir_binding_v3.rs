//! Independent scalar GEMM profile validation for a compiler-associated V3 KIR receipt.
//!
//! The generic proof-binding association authenticates only identity agreement. This module also
//! decodes the retained KIR and requires byte-for-byte equality with the one reviewed canonical
//! scalar GEMM graph. It does not yet prove that the Verus source model refines that graph.

use std::{error::Error, fmt};

use fe2o3_compiler_lineage::{
    InertKernelIrReceiptIdentityV3, InertKernelIrReceiptV3, InertLineageContentIdentityV3,
    InertProofBindingReceiptIdentityV3,
};
use fe2o3_kernel_ir::{
    VerifiedCanonicalKernelIrErrorV5, VerifiedCanonicalKernelIrV5, scalar_gemm_v1_module,
};

use crate::ValidatedCompilerProofBindingAssociationV3;

/// Linear validation that one associated V3 KIR is the exact scalar GEMM V1 graph.
#[derive(Debug)]
#[must_use = "exact KIR profile validation must be joined to Verus and machine refinement"]
pub struct ValidatedScalarGemmCompilerKirV3 {
    proof_binding_receipt_identity: InertProofBindingReceiptIdentityV3,
    kernel_ir_receipt_identity: InertKernelIrReceiptIdentityV3,
    canonical_kir_identity: [u8; 32],
}

impl ValidatedScalarGemmCompilerKirV3 {
    /// Returns the generic proof-binding receipt that named this KIR.
    pub const fn proof_binding_receipt_identity(&self) -> InertProofBindingReceiptIdentityV3 {
        self.proof_binding_receipt_identity
    }

    /// Returns the exact retained KIR receipt identity.
    pub const fn kernel_ir_receipt_identity(&self) -> InertKernelIrReceiptIdentityV3 {
        self.kernel_ir_receipt_identity
    }

    /// Returns the independently rederived canonical KIR V5 identity.
    pub const fn canonical_kir_identity(&self) -> [u8; 32] {
        self.canonical_kir_identity
    }

    /// The decoded graph is byte-exactly the reviewed scalar GEMM V1 KIR.
    pub const fn establishes_exact_scalar_gemm_kir_profile(&self) -> bool {
        true
    }

    /// Exact profile equality does not prove the separately maintained Verus model corresponds.
    pub const fn establishes_verus_model_correspondence(&self) -> bool {
        false
    }

    /// KIR profile validation does not prove LLVM or emitted-machine refinement.
    pub const fn establishes_emitted_machine_refinement(&self) -> bool {
        false
    }

    /// This intermediate validation cannot enter the strict Worker V3 proof gate.
    pub const fn can_enter_worker_v3_gate(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_runtime_authority(&self) -> bool {
        false
    }
}

/// Strictly validates one compiler-associated KIR as the exact scalar GEMM V1 profile.
pub fn validate_scalar_gemm_compiler_kir_v3(
    association: &ValidatedCompilerProofBindingAssociationV3,
    kernel_ir: &InertKernelIrReceiptV3,
) -> Result<ValidatedScalarGemmCompilerKirV3, ScalarGemmCompilerKirValidationErrorV3> {
    let associated = association.association().inputs().kernel_ir();
    let retained = InertLineageContentIdentityV3::new(
        *kernel_ir.identity().sha256(),
        kernel_ir.identity().byte_len(),
    )
    .map_err(|_| ScalarGemmCompilerKirValidationErrorV3::AssociatedKernelIrMismatch)?;
    if associated != retained {
        return Err(ScalarGemmCompilerKirValidationErrorV3::AssociatedKernelIrMismatch);
    }

    let observed =
        VerifiedCanonicalKernelIrV5::from_canonical_bytes(kernel_ir.canonical_preimage().to_vec())
            .map_err(ScalarGemmCompilerKirValidationErrorV3::InvalidCanonicalKernelIr)?;
    let expected = VerifiedCanonicalKernelIrV5::from_module(scalar_gemm_v1_module())
        .map_err(ScalarGemmCompilerKirValidationErrorV3::InvalidReviewedProfile)?;
    if observed.canonical_bytes() != expected.canonical_bytes() {
        return Err(ScalarGemmCompilerKirValidationErrorV3::NonCanonicalScalarGemmProfile);
    }
    observed
        .revalidate()
        .map_err(ScalarGemmCompilerKirValidationErrorV3::InvalidCanonicalKernelIr)?;

    Ok(ValidatedScalarGemmCompilerKirV3 {
        proof_binding_receipt_identity: association.receipt_identity(),
        kernel_ir_receipt_identity: kernel_ir.identity(),
        canonical_kir_identity: *observed.identity().digest(),
    })
}

/// Failure to recover the exact reviewed scalar KIR from compiler-associated bytes.
#[derive(Debug)]
#[non_exhaustive]
pub enum ScalarGemmCompilerKirValidationErrorV3 {
    /// The supplied KIR differs from the KIR named by the validated association.
    AssociatedKernelIrMismatch,
    /// The retained compiler bytes are not valid canonical KIR V5.
    InvalidCanonicalKernelIr(VerifiedCanonicalKernelIrErrorV5),
    /// The in-tree reviewed scalar profile could not be canonicalized.
    InvalidReviewedProfile(VerifiedCanonicalKernelIrErrorV5),
    /// The valid KIR is not the exact reviewed scalar GEMM graph.
    NonCanonicalScalarGemmProfile,
}

impl fmt::Display for ScalarGemmCompilerKirValidationErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AssociatedKernelIrMismatch => {
                formatter.write_str("compiler association names a different Kernel IR receipt")
            }
            Self::InvalidCanonicalKernelIr(error) => {
                write!(formatter, "compiler Kernel IR is not canonical V5: {error}")
            }
            Self::InvalidReviewedProfile(error) => {
                write!(
                    formatter,
                    "reviewed scalar GEMM profile is invalid: {error}"
                )
            }
            Self::NonCanonicalScalarGemmProfile => formatter
                .write_str("compiler Kernel IR is not the exact reviewed scalar GEMM V1 graph"),
        }
    }
}

impl Error for ScalarGemmCompilerKirValidationErrorV3 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidCanonicalKernelIr(error) | Self::InvalidReviewedProfile(error) => {
                Some(error)
            }
            Self::AssociatedKernelIrMismatch | Self::NonCanonicalScalarGemmProfile => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use fe2o3_compiler_lineage::{
        InertCanonicalSemanticMirReceiptV3, InertFormalMemoryReceiptV3, InertMiddleEndReceiptV3,
        InertMirToKirCorrespondenceReceiptV3, InertProofBindingAssociationInputsV3,
        InertProofBindingAssociationV3, InertProofBindingReceiptV3,
    };
    use fe2o3_kernel_ir::{KernelId, Module, VerifiedCanonicalKernelIrV5};

    use super::*;
    use crate::validate_compiler_proof_binding_association_v3;

    struct Fixture {
        semantic_mir: InertCanonicalSemanticMirReceiptV3,
        middle_end: InertMiddleEndReceiptV3,
        kernel_ir: InertKernelIrReceiptV3,
        correspondence: InertMirToKirCorrespondenceReceiptV3,
        formal_memory: InertFormalMemoryReceiptV3,
        proof_binding: InertProofBindingReceiptV3,
    }

    fn content(sha256: &[u8; 32], byte_len: u64) -> InertLineageContentIdentityV3 {
        InertLineageContentIdentityV3::new(*sha256, byte_len).unwrap()
    }

    fn fixture(kernel_ir_bytes: Vec<u8>) -> Fixture {
        let semantic_mir =
            InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(b"semantic".to_vec())
                .unwrap();
        let middle_end =
            InertMiddleEndReceiptV3::from_canonical_preimage(b"middle".to_vec()).unwrap();
        let kernel_ir = InertKernelIrReceiptV3::from_canonical_preimage(kernel_ir_bytes).unwrap();
        let correspondence =
            InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(b"mapping".to_vec())
                .unwrap();
        let formal_memory =
            InertFormalMemoryReceiptV3::from_canonical_preimage(b"formal".to_vec()).unwrap();
        let inputs = InertProofBindingAssociationInputsV3::new(
            content(
                semantic_mir.identity().sha256(),
                semantic_mir.identity().byte_len(),
            ),
            content(
                middle_end.identity().sha256(),
                middle_end.identity().byte_len(),
            ),
            content(
                kernel_ir.identity().sha256(),
                kernel_ir.identity().byte_len(),
            ),
            content(
                correspondence.identity().sha256(),
                correspondence.identity().byte_len(),
            ),
            content(
                formal_memory.identity().sha256(),
                formal_memory.identity().byte_len(),
            ),
        );
        let association = InertProofBindingAssociationV3::new(inputs).unwrap();
        let proof_binding = InertProofBindingReceiptV3::from_canonical_preimage(
            association.canonical_bytes().to_vec(),
        )
        .unwrap();
        Fixture {
            semantic_mir,
            middle_end,
            kernel_ir,
            correspondence,
            formal_memory,
            proof_binding,
        }
    }

    fn validate_association(fixture: &Fixture) -> ValidatedCompilerProofBindingAssociationV3 {
        validate_compiler_proof_binding_association_v3(
            &fixture.proof_binding,
            &fixture.semantic_mir,
            &fixture.middle_end,
            &fixture.kernel_ir,
            &fixture.correspondence,
            &fixture.formal_memory,
        )
        .unwrap()
    }

    fn scalar_bytes() -> Vec<u8> {
        VerifiedCanonicalKernelIrV5::from_module(scalar_gemm_v1_module())
            .unwrap()
            .into_canonical_bytes()
    }

    #[test]
    fn exact_associated_scalar_kir_is_recovered_without_authority() {
        let fixture = fixture(scalar_bytes());
        let association = validate_association(&fixture);
        let validated = validate_scalar_gemm_compiler_kir_v3(&association, &fixture.kernel_ir)
            .expect("exact scalar KIR must validate");
        assert_eq!(
            validated.proof_binding_receipt_identity(),
            fixture.proof_binding.identity()
        );
        assert_eq!(
            validated.kernel_ir_receipt_identity(),
            fixture.kernel_ir.identity()
        );
        assert_ne!(validated.canonical_kir_identity(), [0; 32]);
        assert!(validated.establishes_exact_scalar_gemm_kir_profile());
        assert!(!validated.establishes_verus_model_correspondence());
        assert!(!validated.establishes_emitted_machine_refinement());
        assert!(!validated.can_enter_worker_v3_gate());
        assert!(!validated.grants_artifact_or_runtime_authority());
    }

    #[test]
    fn identity_consistent_but_different_kir_is_rejected() {
        let mut different: Module = scalar_gemm_v1_module();
        different.kernels[0].id = KernelId::new("scalar_gemm_v1_substituted");
        let bytes = VerifiedCanonicalKernelIrV5::from_module(different)
            .unwrap()
            .into_canonical_bytes();
        let fixture = fixture(bytes);
        let association = validate_association(&fixture);
        assert!(matches!(
            validate_scalar_gemm_compiler_kir_v3(&association, &fixture.kernel_ir),
            Err(ScalarGemmCompilerKirValidationErrorV3::NonCanonicalScalarGemmProfile)
        ));
    }

    #[test]
    fn malformed_and_association_substituted_kir_fail_closed() {
        let malformed = fixture(b"not canonical KIR".to_vec());
        let malformed_association = validate_association(&malformed);
        assert!(matches!(
            validate_scalar_gemm_compiler_kir_v3(&malformed_association, &malformed.kernel_ir),
            Err(ScalarGemmCompilerKirValidationErrorV3::InvalidCanonicalKernelIr(_))
        ));

        let exact = fixture(scalar_bytes());
        assert!(matches!(
            validate_scalar_gemm_compiler_kir_v3(&malformed_association, &exact.kernel_ir),
            Err(ScalarGemmCompilerKirValidationErrorV3::AssociatedKernelIrMismatch)
        ));
    }
}
