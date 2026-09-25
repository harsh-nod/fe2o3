use super::*;

mod disposal;

#[cfg(test)]
#[path = "disposal_baseline.rs"]
mod disposal_baseline;

#[cfg(test)]
#[path = "settlement_baseline.rs"]
mod settlement_baseline;

#[cfg(test)]
#[path = "unknown_baseline.rs"]
mod unknown_baseline;

#[allow(unused_macros)]
#[macro_use]
mod unknown_templates {
    include!("unknown_wrapper_bodies.rs");
}

#[allow(unused_macros)]
#[macro_use]
mod settlement_templates {
    include!("settlement_wrapper_bodies.rs");
}

macro_rules! settlement_rust_expr {
    ($body:expr) => {
        $body
    };
}

impl ContextVersionJournalV1 {
    /// Settles a complete retained writer using an inert success premise.
    pub fn settle_success(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterSuccessEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        settlement_outcome_body!(self, settle_retained, writer, evidence, true, [])
    }

    /// Preserves burned attempt epochs and prior lineage; no authority is minted.
    pub fn settle_no_effect(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterNoEffectEvidenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        settlement_outcome_body!(self, settle_retained, writer, evidence, false, [])
    }

    /// Retains the complete writer chain until separately authorized recovery.
    pub fn mark_unknown(
        &mut self,
        writer: ContextWriterReferenceV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        journal_unknown_wrapper_body!(self, writer, retained::shared_retained_unknown_v1)
    }

    #[cfg(test)]
    fn retained_header(
        &self,
        writer: ContextWriterReferenceV1,
        allow_unknown: bool,
    ) -> Result<(Option<usize>, usize, bool), ContextVersionJournalErrorV1> {
        retained::shared_retained_header_v1(self, writer, allow_unknown)
    }

    #[cfg(test)]
    fn validate_retained_chain(
        &self,
        writer: ContextWriterReferenceV1,
        head: Option<usize>,
        count: usize,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        retained::shared_retained_chain_v1(self, writer, head, count)
    }

    #[allow(clippy::question_mark)]
    pub(super) fn preflight_settlement(
        &self,
        writer: ContextWriterReferenceV1,
        evidence: ContextWriterReferenceV1,
    ) -> Result<(Option<usize>, usize), ContextVersionJournalErrorV1> {
        settlement_preflight_body!(
            settlement_rust_expr,
            self,
            writer,
            evidence,
            self.free.capacity(),
            self.member_free.capacity(),
            retained::shared_retained_header_v1,
            retained::shared_retained_writer_key_v1,
            retained::shared_retained_chain_v1,
            settlement_scratch::shared_settlement_scratch_scan_v1
        )
    }

    fn settle_retained(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: ContextWriterReferenceV1,
        success: bool,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        settlement_execute_body!(
            settlement_rust_expr,
            self,
            writer,
            evidence,
            success,
            head,
            count,
            preflight_settlement,
            [],
            settlement_scratch::shared_settlement_scratch_stage_v1,
            settlement_commit::shared_settlement_commit_v1,
            [],
            [],
            []
        )
    }
}
