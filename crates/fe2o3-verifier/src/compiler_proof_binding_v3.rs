//! Independent validation of the compiler's frozen V3 proof-input association.
//!
//! This validates exact canonical content relationships only. It does not execute Verus, prove a
//! property, authenticate compiler origin, establish refinement, or grant runtime authority.

use std::{error::Error, fmt};

use fe2o3_compiler_lineage::{
    InertCanonicalSemanticMirReceiptV3, InertFormalMemoryReceiptV3, InertKernelIrReceiptV3,
    InertLineageContentIdentityV3, InertMiddleEndReceiptV3, InertMirToKirCorrespondenceReceiptV3,
    InertProofBindingAssociationErrorV3, InertProofBindingAssociationV3,
    InertProofBindingReceiptIdentityV3, InertProofBindingReceiptV3,
};

/// Independently decoded V3 proof-input association matched to five exact lineage receipts.
///
/// The value is intentionally non-`Clone` so a later authority-bearing join can consume the one
/// checked occurrence without changing this type's current non-authoritative contract.
#[derive(Debug, Eq, PartialEq)]
pub struct ValidatedCompilerProofBindingAssociationV3 {
    association: InertProofBindingAssociationV3,
    receipt_identity: InertProofBindingReceiptIdentityV3,
}

impl ValidatedCompilerProofBindingAssociationV3 {
    /// Returns the independently decoded association.
    pub const fn association(&self) -> &InertProofBindingAssociationV3 {
        &self.association
    }

    /// Returns the exact outer lineage-receipt identity whose preimage was decoded.
    pub const fn receipt_identity(&self) -> InertProofBindingReceiptIdentityV3 {
        self.receipt_identity
    }

    /// Reports that content association alone does not authenticate Verus execution.
    pub const fn authenticates_verus_execution(&self) -> bool {
        false
    }

    /// Reports that content association alone does not establish compiler refinement.
    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }

    /// Reports that content association alone grants no load or launch authority.
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

/// Strictly decodes and matches the compiler proof association to exact retained V3 receipts.
pub fn validate_compiler_proof_binding_association_v3(
    proof_binding: &InertProofBindingReceiptV3,
    semantic_mir: &InertCanonicalSemanticMirReceiptV3,
    middle_end: &InertMiddleEndReceiptV3,
    kernel_ir: &InertKernelIrReceiptV3,
    mir_to_kir_correspondence: &InertMirToKirCorrespondenceReceiptV3,
    formal_memory: &InertFormalMemoryReceiptV3,
) -> Result<ValidatedCompilerProofBindingAssociationV3, CompilerProofBindingValidationErrorV3> {
    let association = InertProofBindingAssociationV3::decode(proof_binding.canonical_preimage())
        .map_err(CompilerProofBindingValidationErrorV3::Decode)?;
    let inputs = association.inputs();
    for (actual, expected, field) in [
        (
            inputs.semantic_mir(),
            content_identity(
                semantic_mir.identity().sha256(),
                semantic_mir.identity().byte_len(),
            )?,
            "semantic MIR",
        ),
        (
            inputs.middle_end(),
            content_identity(
                middle_end.identity().sha256(),
                middle_end.identity().byte_len(),
            )?,
            "middle end",
        ),
        (
            inputs.kernel_ir(),
            content_identity(
                kernel_ir.identity().sha256(),
                kernel_ir.identity().byte_len(),
            )?,
            "Kernel IR",
        ),
        (
            inputs.mir_to_kir_correspondence(),
            content_identity(
                mir_to_kir_correspondence.identity().sha256(),
                mir_to_kir_correspondence.identity().byte_len(),
            )?,
            "MIR-to-KIR correspondence",
        ),
        (
            inputs.formal_memory(),
            content_identity(
                formal_memory.identity().sha256(),
                formal_memory.identity().byte_len(),
            )?,
            "formal memory",
        ),
    ] {
        if actual != expected {
            return Err(CompilerProofBindingValidationErrorV3::IdentityMismatch { field });
        }
    }
    Ok(ValidatedCompilerProofBindingAssociationV3 {
        association,
        receipt_identity: proof_binding.identity(),
    })
}

fn content_identity(
    sha256: &[u8; 32],
    byte_len: u64,
) -> Result<InertLineageContentIdentityV3, CompilerProofBindingValidationErrorV3> {
    InertLineageContentIdentityV3::new(*sha256, byte_len)
        .map_err(CompilerProofBindingValidationErrorV3::Decode)
}

/// Failure to decode or exactly match one compiler proof-input association.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerProofBindingValidationErrorV3 {
    /// The proof-binding preimage is not the exact canonical frozen format.
    Decode(InertProofBindingAssociationErrorV3),
    /// A named association input differs from its independently retained receipt.
    IdentityMismatch {
        /// The mismatched semantic stage.
        field: &'static str,
    },
}

impl fmt::Display for CompilerProofBindingValidationErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => {
                write!(formatter, "cannot decode compiler proof binding: {error}")
            }
            Self::IdentityMismatch { field } => {
                write!(
                    formatter,
                    "compiler proof binding has substituted {field} identity"
                )
            }
        }
    }
}

impl Error for CompilerProofBindingValidationErrorV3 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            Self::IdentityMismatch { .. } => None,
        }
    }
}
