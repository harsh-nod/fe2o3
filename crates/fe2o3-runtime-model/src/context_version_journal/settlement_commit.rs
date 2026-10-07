use super::{
    ContextVersionJournalV1 as JournalContentsV1, ContextWriterReferenceV1 as WriterReferenceV1,
};

include!("settlement_commit_body.rs");

macro_rules! settlement_commit_rust_expr {
    ($body:expr) => {
        $body
    };
}

#[inline]
fn settlement_commit_access_v1(journal: &JournalContentsV1) {
    journal.count_indexed_access();
}

pub(super) fn shared_settlement_commit_v1(
    journal: &mut JournalContentsV1,
    writer: WriterReferenceV1,
    count: usize,
    success: bool,
) {
    settlement_commit_body!(
        settlement_commit_rust_expr,
        journal,
        writer,
        count,
        success,
        index,
        []
    )
}
