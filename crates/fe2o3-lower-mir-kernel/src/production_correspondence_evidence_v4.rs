//! Live semantic-KIR conversion into the canonical V4 correspondence contract.

use std::collections::BTreeSet;

use fe2o3_mir_kir_contracts::{
    InertCanonicalMirToKirCorrespondenceEvidenceV4, MirToKirBlockCorrespondenceEvidenceV4,
    MirToKirParameterBindingEvidenceV4, MirToKirStatementSpanEvidenceV4,
    MirToKirSyntheticRuleEvidenceV4, MirToKirSyntheticSpanEvidenceV4,
    MirToKirTerminatorSpanEvidenceV4,
};
use fe2o3_mir_model::{
    InertCanonicalSemanticU32InductionEvidenceV1, SemanticU32InductionNoOverflowReportV1,
};

use crate::{
    ProductionEvidenceConstructionErrorV1, ProductionSemanticKirOwnerV1,
    SemanticKirSyntheticOperationRuleV1,
};

/// Replays one live owner and constructs exact, authority-free V4 correspondence evidence.
pub fn produce_mir_to_kir_correspondence_evidence_v4(
    owner: &ProductionSemanticKirOwnerV1,
    induction_report: &SemanticU32InductionNoOverflowReportV1,
) -> Result<InertCanonicalMirToKirCorrespondenceEvidenceV4, ProductionEvidenceConstructionErrorV1> {
    owner
        .verify_equivalence()
        .map_err(ProductionEvidenceConstructionErrorV1::SemanticKir)?;
    let induction = InertCanonicalSemanticU32InductionEvidenceV1::from_report(induction_report)
        .map_err(ProductionEvidenceConstructionErrorV1::Induction)?;
    let semantic_sha256 = *owner.semantic().semantic().semantic_sha256().as_bytes();
    if &semantic_sha256 != induction.semantic_mir_sha256() {
        return Err(ProductionEvidenceConstructionErrorV1::InvalidOwner(
            "semantic and induction identities differ",
        ));
    }

    let correspondence = owner.correspondence();
    let covered_functions = correspondence
        .blocks()
        .iter()
        .map(|record| record.semantic_function().index())
        .collect::<BTreeSet<_>>();
    let function_count = u32::try_from(covered_functions.len()).map_err(|_| {
        ProductionEvidenceConstructionErrorV1::Overflow("correspondence function count")
    })?;
    let mut blocks = correspondence
        .blocks()
        .iter()
        .map(|record| {
            MirToKirBlockCorrespondenceEvidenceV4::from_parts(
                record.semantic_function().index(),
                record.semantic_block().index(),
                record.kernel_ir_block().0,
                record.source_statement_count(),
            )
        })
        .collect::<Vec<_>>();
    let mut statements = correspondence
        .statement_operation_spans()
        .iter()
        .map(|span| {
            MirToKirStatementSpanEvidenceV4::from_parts(
                span.semantic_function().index(),
                span.semantic_block().index(),
                span.statement_ordinal(),
                span.kernel_ir_block().0,
                span.first_operation_ordinal(),
                span.operation_count(),
            )
        })
        .collect::<Vec<_>>();
    let mut terminators = correspondence
        .terminator_operation_spans()
        .iter()
        .map(|span| {
            MirToKirTerminatorSpanEvidenceV4::from_parts(
                span.semantic_function().index(),
                span.semantic_block().index(),
                span.kernel_ir_block().0,
                span.first_operation_ordinal(),
                span.operation_count(),
            )
        })
        .collect::<Vec<_>>();
    let mut synthetics = correspondence
        .synthetic_operation_spans()
        .iter()
        .map(|span| {
            let rule = match span.rule() {
                SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage => {
                    MirToKirSyntheticRuleEvidenceV4::EnumPayloadStorage
                }
                SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap => {
                    MirToKirSyntheticRuleEvidenceV4::RuntimeAssertFailureTrap
                }
            };
            MirToKirSyntheticSpanEvidenceV4::from_parts(
                rule,
                span.kernel_ir_block().0,
                span.first_operation_ordinal(),
                span.operation_count(),
            )
        })
        .collect::<Vec<_>>();
    let mut parameters = correspondence
        .parameter_bindings()
        .iter()
        .map(|binding| {
            MirToKirParameterBindingEvidenceV4::from_parts(
                binding.semantic_function().index(),
                binding.semantic_local().index(),
                binding.kernel_ir_value().0,
            )
        })
        .collect::<Vec<_>>();

    statements.sort_unstable_by_key(|span| {
        (
            span.semantic_function(),
            span.semantic_block(),
            span.statement(),
        )
    });
    terminators.sort_unstable_by_key(|span| (span.semantic_function(), span.semantic_block()));
    synthetics.sort_unstable();
    parameters
        .sort_unstable_by_key(|binding| (binding.semantic_function(), binding.semantic_local()));
    blocks.sort_unstable_by_key(|record| (record.semantic_function(), record.semantic_block()));

    InertCanonicalMirToKirCorrespondenceEvidenceV4::from_canonical_parts(
        semantic_sha256,
        owner.canonical_kernel_ir_identity(),
        function_count,
        &blocks,
        &statements,
        &terminators,
        &synthetics,
        &parameters,
        &induction,
    )
    .map_err(Into::into)
}
