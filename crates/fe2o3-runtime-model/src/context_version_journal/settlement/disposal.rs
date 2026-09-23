use super::*;

#[allow(unused_macros)]
#[macro_use]
mod templates {
    include!("../disposal_bodies.rs");
}

macro_rules! disposal_rust_expr {
    ($body:expr) => {
        $body
    };
}

#[allow(clippy::question_mark)]
fn scan_scratch(
    journal: &ContextVersionJournalV1,
    count: usize,
) -> Result<(), ContextVersionJournalErrorV1> {
    disposal_scratch_scan_body!(disposal_rust_expr, journal, count, index, [])
}

fn stage(journal: &mut ContextVersionJournalV1, initial: Option<usize>, count: usize) {
    disposal_stage_body!(
        disposal_rust_expr,
        journal,
        initial,
        count,
        ContextVersionJournalV1::count_indexed_access,
        head,
        index,
        []
    );
}

fn commit(journal: &mut ContextVersionJournalV1, writer: ContextWriterReferenceV1, count: usize) {
    disposal_commit_body!(
        disposal_rust_expr,
        journal,
        writer,
        count,
        ContextVersionJournalV1::count_indexed_access,
        index,
        []
    );
}

impl ContextVersionJournalV1 {
    /// Preflights the complete Unknown roster without authorizing native effects.
    /// Partial disposal must remain retained by the production owner until every
    /// member is confirmed gone. This model never clears a successful prefix.
    pub fn validate_unknown_disposal(
        &self,
        writer: ContextWriterReferenceV1,
        canonical: &[ContextAllocationWriteV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        disposal_validate_body!(self, writer, canonical, unknown_disposal_plan, [])
    }

    /// Retires an exact Unknown writer and all its allocations atomically.
    /// No allocation becomes available, and no successful lineage is published.
    /// Writer issuance history remains burned. Callers must not reuse disposed
    /// allocation keys. This operation is O(k), with no allocation or callback;
    /// partition preservation assumes a valid prestate.
    #[allow(clippy::question_mark)]
    pub fn dispose_unknown(
        &mut self,
        writer: ContextWriterReferenceV1,
        evidence: &ContextWriterDisposalEvidenceV1<'_>,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        disposal_execute_body!(
            disposal_rust_expr,
            self,
            writer,
            evidence,
            retained::shared_retained_header_v1,
            retained::shared_retained_writer_key_v1,
            unknown_disposal_plan,
            [],
            stage,
            commit,
            head,
            count,
            [],
            [],
            []
        )
    }

    #[allow(clippy::question_mark)]
    fn unknown_disposal_plan(
        &self,
        writer: ContextWriterReferenceV1,
        canonical: &[ContextAllocationWriteV1],
    ) -> Result<(Option<usize>, usize), ContextVersionJournalErrorV1> {
        disposal_plan_body!(
            disposal_rust_expr,
            self,
            writer,
            canonical,
            self.free.capacity(),
            self.member_free.capacity(),
            self.allocation_free.capacity(),
            retained::shared_retained_header_v1,
            retained::shared_retained_chain_v1,
            retained::shared_retained_allocation_v1,
            scan_scratch,
            head,
            count,
            cursor,
            index,
            [],
            [],
            []
        )
    }
}
