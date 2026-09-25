use super::{
    BeginMemberPlanV1, ContextVersionJournalErrorV1 as ReadErrorV1,
    ContextVersionJournalV1 as JournalContentsV1,
};

include!("settlement_scratch_bodies.rs");

macro_rules! settlement_scratch_rust_expr {
    ($body:expr) => {
        $body
    };
}

#[inline]
fn settlement_scratch_access_v1(journal: &JournalContentsV1) {
    journal.count_indexed_access();
}

pub(super) fn shared_settlement_scratch_scan_v1(
    journal: &JournalContentsV1,
    count: usize,
) -> Result<(), ReadErrorV1> {
    settlement_scratch_scan_body!(settlement_scratch_rust_expr, journal, count, index, [])
}

pub(super) fn shared_settlement_scratch_stage_v1(
    journal: &mut JournalContentsV1,
    initial: Option<usize>,
    count: usize,
) {
    settlement_scratch_stage_body!(
        settlement_scratch_rust_expr,
        journal,
        initial,
        count,
        head,
        index,
        []
    )
}
