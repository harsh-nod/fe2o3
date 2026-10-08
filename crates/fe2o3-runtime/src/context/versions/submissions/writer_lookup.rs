use super::*;

include!("writer_lookup_body.rs");

// A returned reference is only the already-retained metadata, not settlement authority.
pub(super) fn preflight_settlement_writer_v1(
    submissions: &HashMap<RuntimeSubmissionIdV1, SubmissionRecordV1>,
    versions: Option<&ContextVersionsV1>,
    id: RuntimeSubmissionIdV1,
    domain: SubmissionWriterDomainV1,
) -> Result<Option<ContextWriterReferenceV1>, ContextVersionJournalErrorV1> {
    completion_writer_lookup_body_v1!(
        completion_journal_rust_syntax,
        submissions,
        versions,
        id,
        domain
    )
}

#[cfg(test)]
#[path = "writer_lookup_tests.rs"]
mod tests;
