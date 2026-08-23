#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

mod capsule;
mod error;
mod proof_binding;
mod receipt;

pub use capsule::{
    INERT_PRODUCTION_SEMANTIC_CAPSULE_MAGIC_V3, INERT_PRODUCTION_SEMANTIC_CAPSULE_VERSION_V3,
    InertProductionSemanticCapsuleIdentityV3, InertProductionSemanticCapsuleV3,
    MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3,
    MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_DECODE_OWNED_BYTES_V3,
    OrderedInertSemanticLineageReceiptsV3,
};
pub use error::{LineageDecodeErrorV3, LineageErrorV3};
pub use proof_binding::{
    INERT_PROOF_BINDING_ASSOCIATION_MAGIC_V3, INERT_PROOF_BINDING_ASSOCIATION_VERSION_V3,
    InertLineageContentIdentityV3, InertProofBindingAssociationErrorV3,
    InertProofBindingAssociationInputsV3, InertProofBindingAssociationV3,
    MAX_INERT_PROOF_BINDING_ASSOCIATION_BYTES_V3,
};
pub use receipt::{
    InertAbiReceiptIdentityV3, InertAbiReceiptV3, InertAmdgpuLoweringReceiptIdentityV3,
    InertAmdgpuLoweringReceiptV3, InertCanonicalSemanticMirIdentityV3,
    InertCanonicalSemanticMirReceiptV3, InertDataLayoutReceiptIdentityV3, InertDataLayoutReceiptV3,
    InertExportManifestReceiptIdentityV3, InertExportManifestReceiptV3,
    InertFinalCompilerModuleCommitmentIdentityV3, InertFinalCompilerModuleCommitmentReceiptV3,
    InertFormalMemoryReceiptIdentityV3, InertFormalMemoryReceiptV3, InertKernelIrReceiptIdentityV3,
    InertKernelIrReceiptV3, InertMiddleEndReceiptIdentityV3, InertMiddleEndReceiptV3,
    InertMirToKirCorrespondenceReceiptIdentityV3, InertMirToKirCorrespondenceReceiptV3,
    InertProofBindingReceiptIdentityV3, InertProofBindingReceiptV3,
    InertRustcIdentityInventoryReceiptIdentityV3, InertRustcIdentityInventoryReceiptV3,
    InertRustcPreflightPlanReceiptIdentityV3, InertRustcPreflightPlanReceiptV3,
    InertSemanticToLlvmReceiptIdentityV3, InertSemanticToLlvmReceiptV3,
    InertTargetBindingReceiptIdentityV3, InertTargetBindingReceiptV3,
    MAX_CANONICAL_SEMANTIC_MIR_BYTES_V3, MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
};
