//! Live-owner evidence construction errors at the lowering boundary.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{FormalMemoryReceiptErrorV1, VerifiedCanonicalKernelIrErrorV5};
use fe2o3_mir_kir_contracts::{
    ProductionCorrespondenceEvidenceErrorV4, ProductionFormalMemoryEvidenceErrorV4,
    ProductionLineageEvidenceErrorV3,
};
use fe2o3_mir_model::SemanticU32InductionEvidenceErrorV1;

use crate::{ProductionFormalMemoryErrorV1, ProductionSemanticKirErrorV1};

/// Failure while converting live lowering owners into authority-free evidence contracts.
#[derive(Debug)]
pub enum ProductionEvidenceConstructionErrorV1 {
    /// The live semantic-to-KIR owner failed its independent replay.
    SemanticKir(ProductionSemanticKirErrorV1),
    /// The live formal-memory owner failed its independent replay.
    FormalMemory(ProductionFormalMemoryErrorV1),
    /// Legacy V5 Kernel IR canonicalization failed.
    CanonicalKernelIrV5(VerifiedCanonicalKernelIrErrorV5),
    /// A canonical formal-obligation receipt could not be produced.
    FormalReceipt(FormalMemoryReceiptErrorV1),
    /// Canonical induction evidence could not be produced.
    Induction(SemanticU32InductionEvidenceErrorV1),
    /// A V3 evidence contract rejected the extracted producer fields.
    ContractV3(ProductionLineageEvidenceErrorV3),
    /// The V4 correspondence contract rejected the extracted producer fields.
    CorrespondenceV4(ProductionCorrespondenceEvidenceErrorV4),
    /// The V4 formal-memory contract rejected the extracted producer fields.
    FormalMemoryV4(ProductionFormalMemoryEvidenceErrorV4),
    /// Live owner state violated a producer-side relationship.
    InvalidOwner(&'static str),
    /// Producer-side bounded arithmetic overflowed.
    Overflow(&'static str),
}

impl fmt::Display for ProductionEvidenceConstructionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemanticKir(error) => write!(formatter, "semantic-KIR owner failed: {error}"),
            Self::FormalMemory(error) => write!(formatter, "formal-memory owner failed: {error}"),
            Self::CanonicalKernelIrV5(error) => {
                write!(formatter, "canonical Kernel IR V5 failed: {error}")
            }
            Self::FormalReceipt(error) => {
                write!(formatter, "formal-memory receipt failed: {error}")
            }
            Self::Induction(error) => {
                write!(formatter, "semantic induction evidence failed: {error}")
            }
            Self::ContractV3(error) => write!(formatter, "V3 evidence contract failed: {error}"),
            Self::CorrespondenceV4(error) => {
                write!(formatter, "V4 correspondence contract failed: {error}")
            }
            Self::FormalMemoryV4(error) => {
                write!(formatter, "V4 formal-memory contract failed: {error}")
            }
            Self::InvalidOwner(detail) => {
                write!(formatter, "live evidence owner is invalid: {detail}")
            }
            Self::Overflow(field) => write!(formatter, "live evidence field overflowed: {field}"),
        }
    }
}

impl Error for ProductionEvidenceConstructionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SemanticKir(error) => Some(error),
            Self::FormalMemory(error) => Some(error),
            Self::CanonicalKernelIrV5(error) => Some(error),
            Self::FormalReceipt(error) => Some(error),
            Self::Induction(error) => Some(error),
            Self::ContractV3(error) => Some(error),
            Self::CorrespondenceV4(error) => Some(error),
            Self::FormalMemoryV4(error) => Some(error),
            Self::InvalidOwner(_) | Self::Overflow(_) => None,
        }
    }
}

impl From<ProductionLineageEvidenceErrorV3> for ProductionEvidenceConstructionErrorV1 {
    fn from(error: ProductionLineageEvidenceErrorV3) -> Self {
        Self::ContractV3(error)
    }
}

impl From<ProductionCorrespondenceEvidenceErrorV4> for ProductionEvidenceConstructionErrorV1 {
    fn from(error: ProductionCorrespondenceEvidenceErrorV4) -> Self {
        Self::CorrespondenceV4(error)
    }
}

impl From<ProductionFormalMemoryEvidenceErrorV4> for ProductionEvidenceConstructionErrorV1 {
    fn from(error: ProductionFormalMemoryEvidenceErrorV4) -> Self {
        Self::FormalMemoryV4(error)
    }
}
