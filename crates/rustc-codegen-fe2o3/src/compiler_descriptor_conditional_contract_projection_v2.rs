//! V2 projection from the actual CPU-bound execution, never a V1 upgrade.
use super::*;
use fe2o3_kernel_descriptor::{
    CONDITIONAL_INVOCATION_CODEC_STORAGE_V2, ConditionalInvocationContractInputV2 as InputV2,
    ConditionalInvocationContractV2 as ViewV2, ConditionalTheoremV2 as TheoremV2,
    MAX_CONDITIONAL_INVOCATION_BYTES_V2, decode_conditional_invocation_contract_v2,
    encode_conditional_invocation_contract_v2, encoded_conditional_invocation_contract_v2_len,
};
use fe2o3_verifier::ProductionConditionalFormulaExecutionV2;

const PROJECTION_STORAGE_V2: usize = size_of::<Scratch>()
    + size_of::<InputV2<'static>>()
    + 4 * size_of::<FunctionalRefinementBindingV2>()
    + 4 * size_of::<Subjects>()
    + 4 * size_of::<TheoremV2>()
    + 8 * size_of::<Read>()
    + 8 * size_of::<Argument>()
    + 1024;
const ENCODING_STORAGE_V2: usize =
    MAX_CONDITIONAL_INVOCATION_BYTES_V2 + CONDITIONAL_INVOCATION_CODEC_STORAGE_V2;

/// Uses the V1 row and scratch custody rules, with the real V2 theorem/codec.
/// Copied bytes remain inert and unreserved when generated-fields returns.
pub(crate) fn with_conditional_contract_projection_v2<'w, R>(
    fields: &ConditionalGeneratedFieldsV1<'_>,
    execution: &ProductionConditionalFormulaExecutionV2,
    budget: &mut Budget<'w>,
    consume: impl for<'wire> FnOnce(ViewV2<'wire>, &mut Budget<'w>) -> R,
) -> Result<R, Error> {
    let report = execution.report();
    with_projected_rows(
        fields,
        report.binding(),
        report.statement_identity().as_bytes(),
        PROJECTION_STORAGE_V2,
        budget,
        |rows, staging, budget| {
            let theorem = TheoremV2 {
                statement_identity: *report.statement_identity().as_bytes(),
                generated_source_identity: *report.generated_source_identity().as_bytes(),
                execution_identity: *report.execution_identity().as_bytes(),
                receipt_identity: *report.receipt_identity().as_bytes(),
                staging_receipt_identity: *staging.receipt_identity().digest().as_bytes(),
                staging_obligation_identity: *staging
                    .binding()
                    .normalized_obligation_effect_ir_hash()
                    .as_bytes(),
                staging_signer_identity: *staging.signer_identity().as_bytes(),
                staging_execution_identity: *staging.execution_identity().as_bytes(),
                cpu_input_commitment: *report.cpu_input_commitment().as_bytes(),
            };
            let projected = InputV2 {
                numerical_domain: ConditionalNumericalDomainV1::LittleEndianSharedIeeeV1,
                subjects: rows.subjects,
                theorem,
                typed_roots: rows.typed_roots,
                arguments: rows.arguments,
                output: rows.output,
                reads: rows.reads,
                premises: rows.premises,
            };
            with_encoded_v2(&projected, budget, consume)
        },
    )
}

// Caller-authored rows are accepted only by private codec fixture tests.
fn with_encoded_v2<'w, R>(
    input: &InputV2<'_>,
    budget: &mut Budget<'w>,
    consume: impl for<'wire> FnOnce(ViewV2<'wire>, &mut Budget<'w>) -> R,
) -> Result<R, Error> {
    with_scratch(budget, ENCODING_STORAGE_V2, |budget| {
        budget.charge_work(MAX_CONDITIONAL_INVOCATION_BYTES_V2)?;
        let mut wire = [0; MAX_CONDITIONAL_INVOCATION_BYTES_V2];
        let len =
            encoded_conditional_invocation_contract_v2_len(input, &mut |n| budget.charge_work(n))?;
        encode_conditional_invocation_contract_v2(input, &mut wire[..len], &mut |n| {
            budget.charge_work(n)
        })?;
        let view = decode_conditional_invocation_contract_v2(&wire[..len], &mut |n| {
            budget.charge_work(n)
        })?;
        Ok(consume(view, budget))
    })
}

#[cfg(test)]
#[path = "compiler_descriptor_conditional_contract_projection_v2_tests.rs"]
mod tests;
