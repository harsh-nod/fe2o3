//! Current-graph replay of inert execution-discharge evidence in both compiler proof routes.
//!
//! This preserves every raw obligation. It does not discharge incomplete effects, replace the
//! semantic/correspondence checks in the callers, or grant compiler, native, or runtime authority.

use fe2o3_kernel_ir::{InertFormalMemoryReceiptFormatV4, Module, verify_module_ref};
use fe2o3_lower_mir_kernel::{
    InertCanonicalFormalMemoryAdmissionEvidenceV4, InertCanonicalFormalMemoryAdmissionEvidenceV5,
    InertFormalMemoryAdmissionEvidenceFormatV5, ProductionFormalMemoryEvidenceErrorV5,
    revalidate_legacy_formal_memory_receipt_against_verified_module_v1,
};

use super::CompilerProofInputValidationErrorV3;
use crate::compiler_multi_root_proof_v1::{
    CompilerMultiRootProofValidationErrorV1, decode_formal_root_payload_v1,
};

const EXECUTION_DISCHARGE_MAGIC: &[u8; 8] = b"F2FMA5\0\0";

enum ReplayError {
    Evidence(ProductionFormalMemoryEvidenceErrorV5),
    Binding(&'static str),
}

fn decode_replayed(
    bytes: &[u8],
    module: &Module,
    ordinal: usize,
    kernel_id: &str,
) -> Result<InertCanonicalFormalMemoryAdmissionEvidenceV5, ReplayError> {
    let evidence = InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(bytes)
        .map_err(ReplayError::Evidence)?;
    let kernel = module.kernels.get(ordinal).ok_or(ReplayError::Binding(
        "execution-discharge evidence names an absent current kernel",
    ))?;
    if evidence.kernel_ordinal() != ordinal
        || evidence.kernel_id() != kernel_id
        || kernel.id.as_str() != kernel_id
        || evidence.entry_id() != kernel.entry.as_str()
    {
        return Err(ReplayError::Binding(
            "execution-discharge evidence names a different current kernel or entry",
        ));
    }
    let verified = verify_module_ref(module).map_err(|_| {
        ReplayError::Binding("execution-discharge replay requires verified current Kernel IR")
    })?;
    evidence
        .revalidate_against_verified_module(verified)
        .map_err(ReplayError::Evidence)?;
    Ok(evidence)
}

pub(super) fn decode_singleton_v1(
    bytes: &[u8],
    module: &Module,
) -> Result<InertFormalMemoryAdmissionEvidenceFormatV5, CompilerProofInputValidationErrorV3> {
    if bytes.get(..8) != Some(EXECUTION_DISCHARGE_MAGIC.as_slice()) {
        let evidence = InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(bytes)
            .map(InertFormalMemoryAdmissionEvidenceFormatV5::Legacy)
            .map_err(CompilerProofInputValidationErrorV3::FormalMemoryV4Decode)?;
        let verified = verify_module_ref(module).map_err(|_| {
            CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                detail: "legacy formal replay requires verified current Kernel IR",
            }
        })?;
        // Legacy outer zero counts do not authenticate the nested raw conflict roster.
        evidence
            .revalidate_against_verified_module(verified)
            .map_err(CompilerProofInputValidationErrorV3::FormalMemoryV5Replay)?;
        return Ok(evidence);
    }
    let [kernel] = module.kernels.as_slice() else {
        return Err(
            CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                detail: "singleton execution-discharge evidence requires exactly one current kernel",
            },
        );
    };
    decode_replayed(bytes, module, 0, kernel.id.as_str())
        .map(InertFormalMemoryAdmissionEvidenceFormatV5::ExecutionDischarged)
        .map_err(|error| match error {
            ReplayError::Evidence(error) => {
                CompilerProofInputValidationErrorV3::FormalMemoryV5Replay(error)
            }
            ReplayError::Binding(detail) => {
                CompilerProofInputValidationErrorV3::StructuralCorrespondence { detail }
            }
        })
}

pub(crate) fn decode_multi_root_v1(
    ordinal: usize,
    kernel_id: &str,
    bytes: &[u8],
    module: &Module,
) -> Result<
    (
        InertFormalMemoryReceiptFormatV4,
        Option<InertCanonicalFormalMemoryAdmissionEvidenceV5>,
    ),
    CompilerMultiRootProofValidationErrorV1,
> {
    if bytes.get(..8) != Some(EXECUTION_DISCHARGE_MAGIC.as_slice()) {
        let receipt = decode_formal_root_payload_v1(ordinal, kernel_id, bytes)?;
        let verified = verify_module_ref(module).map_err(|_| {
            CompilerMultiRootProofValidationErrorV1::RootMismatch {
                root: ordinal,
                detail: "legacy formal replay requires verified current Kernel IR",
            }
        })?;
        // A raw descriptive receipt is not a substitute for the V5 discharge envelope.
        revalidate_legacy_formal_memory_receipt_against_verified_module_v1(
            verified, ordinal, bytes,
        )
        .map_err(|source| {
            CompilerMultiRootProofValidationErrorV1::FormalMemoryExecutionReplay {
                root: ordinal,
                source,
            }
        })?;
        return Ok((receipt, None));
    }
    let evidence =
        decode_replayed(bytes, module, ordinal, kernel_id).map_err(|error| match error {
            ReplayError::Evidence(source) => {
                CompilerMultiRootProofValidationErrorV1::FormalMemoryExecutionReplay {
                    root: ordinal,
                    source,
                }
            }
            ReplayError::Binding(detail) => CompilerMultiRootProofValidationErrorV1::RootMismatch {
                root: ordinal,
                detail,
            },
        })?;
    let receipt = decode_formal_root_payload_v1(
        ordinal,
        kernel_id,
        evidence.formal_obligation_receipt_bytes(),
    )?;
    Ok((receipt, Some(evidence)))
}

#[cfg(test)]
#[path = "compiler_formal_execution_discharge_v1_tests.rs"]
mod tests;
