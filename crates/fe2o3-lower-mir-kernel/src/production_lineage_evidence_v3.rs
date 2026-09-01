//! Live owner conversion into legacy canonical V3 evidence contracts.

use std::collections::{BTreeMap, BTreeSet};

use fe2o3_kernel_ir::{InertCanonicalFormalMemoryObligationReceiptV1, VerifiedCanonicalKernelIrV5};
use fe2o3_mir_kir_contracts::{
    InertCanonicalFormalMemoryAdmissionEvidenceV3, InertCanonicalMirToKirCorrespondenceEvidenceV3,
    MAX_MIR_TO_KIR_CORRESPONDENCE_BLOCKS_V3, MAX_MIR_TO_KIR_CORRESPONDENCE_FUNCTIONS_V3,
    MirToKirBlockCorrespondenceEvidenceV3,
};

use crate::{
    ProductionEvidenceConstructionErrorV1, ProductionFormalMemoryOwnerV1,
    ProductionSemanticKirOwnerV1,
};

/// Replays one semantic-KIR owner and constructs exact legacy V3 correspondence evidence.
pub fn produce_mir_to_kir_correspondence_evidence_v3(
    owner: &ProductionSemanticKirOwnerV1,
) -> Result<InertCanonicalMirToKirCorrespondenceEvidenceV3, ProductionEvidenceConstructionErrorV1> {
    owner
        .verify_equivalence()
        .map_err(ProductionEvidenceConstructionErrorV1::SemanticKir)?;
    let (function_count, blocks) = exact_correspondence_from_owner(owner)?;
    let canonical_kir = VerifiedCanonicalKernelIrV5::from_module(owner.module().clone())
        .map_err(ProductionEvidenceConstructionErrorV1::CanonicalKernelIrV5)?;
    canonical_kir
        .revalidate()
        .map_err(ProductionEvidenceConstructionErrorV1::CanonicalKernelIrV5)?;
    InertCanonicalMirToKirCorrespondenceEvidenceV3::from_canonical_parts(
        *owner.semantic().semantic().semantic_sha256().as_bytes(),
        *canonical_kir.identity().digest(),
        function_count,
        &blocks,
    )
    .map_err(Into::into)
}

/// Replays one formal-memory owner and constructs exact legacy V3 admission evidence.
pub fn produce_formal_memory_admission_evidence_v3(
    owner: &ProductionFormalMemoryOwnerV1,
) -> Result<InertCanonicalFormalMemoryAdmissionEvidenceV3, ProductionEvidenceConstructionErrorV1> {
    owner
        .verify_equivalence()
        .map_err(ProductionEvidenceConstructionErrorV1::FormalMemory)?;
    let canonical_kir =
        VerifiedCanonicalKernelIrV5::from_module(owner.semantic_kir().module().clone())
            .map_err(ProductionEvidenceConstructionErrorV1::CanonicalKernelIrV5)?;
    canonical_kir
        .revalidate()
        .map_err(ProductionEvidenceConstructionErrorV1::CanonicalKernelIrV5)?;
    let [kernel] = owner.kernels() else {
        return Err(ProductionEvidenceConstructionErrorV1::InvalidOwner(
            "V3 evidence requires one formal kernel",
        ));
    };
    if !kernel.obligations().inter_invocation_conflicts().is_empty() {
        return Err(ProductionEvidenceConstructionErrorV1::InvalidOwner(
            "formal owner retains inter-invocation conflicts",
        ));
    }
    let receipt =
        InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(kernel.obligations())
            .map_err(ProductionEvidenceConstructionErrorV1::FormalReceipt)?;
    receipt
        .revalidate()
        .map_err(ProductionEvidenceConstructionErrorV1::FormalReceipt)?;
    InertCanonicalFormalMemoryAdmissionEvidenceV3::from_canonical_parts(
        *canonical_kir.identity().digest(),
        *receipt.identity().digest(),
        kernel.witness_invocation_count(),
        receipt.canonical_bytes(),
    )
    .map_err(Into::into)
}

fn exact_correspondence_from_owner(
    owner: &ProductionSemanticKirOwnerV1,
) -> Result<(u32, Vec<MirToKirBlockCorrespondenceEvidenceV3>), ProductionEvidenceConstructionErrorV1>
{
    let semantic = owner.semantic().semantic();
    let covered_functions = owner
        .correspondence()
        .blocks()
        .iter()
        .map(|record| record.semantic_function().index())
        .collect::<BTreeSet<_>>();
    if covered_functions.is_empty()
        || covered_functions.len() > MAX_MIR_TO_KIR_CORRESPONDENCE_FUNCTIONS_V3
    {
        return Err(ProductionEvidenceConstructionErrorV1::InvalidOwner(
            "correspondence function coverage is empty or exceeds its bound",
        ));
    }
    let target_functions = owner
        .module()
        .functions
        .iter()
        .filter(|function| function.body.is_some())
        .collect::<Vec<_>>();
    if target_functions.len() != covered_functions.len() {
        return Err(ProductionEvidenceConstructionErrorV1::InvalidOwner(
            "semantic and defined Kernel IR function coverage differs",
        ));
    }
    let target_by_semantic_function = covered_functions
        .iter()
        .copied()
        .zip(target_functions)
        .collect::<BTreeMap<_, _>>();
    let function_count = u32::try_from(covered_functions.len()).map_err(|_| {
        ProductionEvidenceConstructionErrorV1::Overflow("correspondence function count")
    })?;
    let expected_blocks = covered_functions.iter().try_fold(
        0_usize,
        |total, function| -> Result<usize, ProductionEvidenceConstructionErrorV1> {
            let function = semantic.functions().get(*function as usize).ok_or(
                ProductionEvidenceConstructionErrorV1::InvalidOwner(
                    "covered semantic function locator is absent",
                ),
            )?;
            total.checked_add(function.blocks().len()).ok_or(
                ProductionEvidenceConstructionErrorV1::Overflow("correspondence block count"),
            )
        },
    )?;
    if expected_blocks == 0
        || expected_blocks > MAX_MIR_TO_KIR_CORRESPONDENCE_BLOCKS_V3
        || owner.correspondence().blocks().len() != expected_blocks
    {
        return Err(ProductionEvidenceConstructionErrorV1::InvalidOwner(
            "semantic and retained block coverage differs",
        ));
    }

    let mut blocks = owner
        .correspondence()
        .blocks()
        .iter()
        .map(|record| {
            MirToKirBlockCorrespondenceEvidenceV3::from_parts(
                record.semantic_function().index(),
                record.semantic_block().index(),
                record.kernel_ir_block().0,
                record.source_statement_count(),
            )
        })
        .collect::<Vec<_>>();
    blocks.sort_unstable_by_key(|record| (record.semantic_function(), record.semantic_block()));
    for record in &blocks {
        let function = semantic
            .functions()
            .get(record.semantic_function() as usize)
            .ok_or(ProductionEvidenceConstructionErrorV1::InvalidOwner(
                "semantic function locator is absent",
            ))?;
        let block = function
            .blocks()
            .get(record.semantic_block() as usize)
            .ok_or(ProductionEvidenceConstructionErrorV1::InvalidOwner(
                "semantic block locator is absent",
            ))?;
        if usize::try_from(record.source_statement_count()) != Ok(block.statements().len()) {
            return Err(ProductionEvidenceConstructionErrorV1::InvalidOwner(
                "source statement count differs from exact semantic MIR",
            ));
        }
        let target_function = target_by_semantic_function
            .get(&record.semantic_function())
            .ok_or(ProductionEvidenceConstructionErrorV1::InvalidOwner(
                "corresponding Kernel IR function is absent",
            ))?;
        let body = target_function.body.as_ref().ok_or(
            ProductionEvidenceConstructionErrorV1::InvalidOwner(
                "corresponding Kernel IR function has no body",
            ),
        )?;
        if !body
            .blocks
            .iter()
            .any(|block| block.id.0 == record.kernel_ir_block())
        {
            return Err(ProductionEvidenceConstructionErrorV1::InvalidOwner(
                "corresponding Kernel IR block is absent",
            ));
        }
    }
    Ok((function_count, blocks))
}
