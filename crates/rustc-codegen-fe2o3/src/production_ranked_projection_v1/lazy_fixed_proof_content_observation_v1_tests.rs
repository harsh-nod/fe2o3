//! Actual-side read-only DATA from the existing lazy owner, cfg(test) only.
//! Never used to construct original expectations or source authority.
use super::*;
use crate::production_ranked_projection_v1::assertion_analyzer_resources_v1::original_fixed_constructor_retention::{
    ProofContent, ContentRefusal, CHECKED_ROW_CAP, genuine_live_content_parts_v1, genuine_retired_content_parts_v1,
    genuine_candidate_content_frame_v1, genuine_candidate_content_work_v1,
};
use crate::production_ranked_projection_v1::assertion_resources_v1::original_cache_content::{
    ContentResult, genuine_cache_raw_frame_v1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct CandidateContentV1 {
    pub phase: u8,
    pub content: ContentResult<ProofContent>,
    pub raw: Snapshot,
    // Proof-local semantic work DATA exists only while a Ready owner is live.
    pub logical_work: Option<usize>,
    pub row_allocations: ContentResult<[(usize, usize, usize); CHECKED_ROW_CAP]>,
}
fn checked_row_allocations_v1(
    rows: &[Vec<usize>],
) -> ContentResult<[(usize, usize, usize); CHECKED_ROW_CAP]> {
    if rows.len() > CHECKED_ROW_CAP {
        return Err(ContentRefusal::CheckedRowsLimit);
    }
    let mut result = [(0, 0, 0); CHECKED_ROW_CAP];
    for (output, row) in result.iter_mut().zip(rows) {
        *output = (row.as_ptr() as usize, row.len(), row.capacity());
    }
    Ok(result)
}
pub(in crate::production_ranked_projection_v1) fn live_candidate_content_v1(
    owner: &LazyFixedProofOwnerV1<'_, '_, '_>,
) -> CandidateContentV1 {
    let (phase, content) = match &owner.phase {
        Phase::Pending(_) | Phase::Unavailable => {
            (0, genuine_live_content_parts_v1(0, &[], None, None, false))
        }
        Phase::Building(building) => (
            1,
            genuine_live_content_parts_v1(
                1,
                &building.checked,
                building.dominance.as_ref(),
                building.zero_exclusion.as_ref(),
                false,
            ),
        ),
        Phase::Ready(session) => {
            let proof = &session.proof;
            let side = !matches!(proof.graph, AssertionGraphV1::Borrowed(_))
                || !matches!(proof.definition_counts, AssertionTableV1::Borrowed(_))
                || !matches!(proof.block_definitions, AssertionTableV1::Borrowed(_))
                || !matches!(proof.address_escaped, AssertionTableV1::Borrowed(_))
                || !matches!(proof.assignments, AssertionTableV1::Borrowed(_))
                || proof.statement_definitions.is_some();
            (
                2,
                genuine_live_content_parts_v1(
                    2,
                    &proof.checked_assertion_blocks,
                    Some(&proof.dominance),
                    Some(&proof.zero_exclusion),
                    side,
                ),
            )
        }
    };
    let logical_work = match &owner.phase {
        Phase::Ready(session) => Some(session.proof.work),
        _ => None,
    };
    let checked: &[Vec<usize>] = match &owner.phase {
        Phase::Building(building) => &building.checked,
        Phase::Ready(session) => &session.proof.checked_assertion_blocks,
        _ => &[],
    };
    CandidateContentV1 {
        phase,
        content,
        raw: live_snapshot(owner),
        logical_work,
        row_allocations: checked_row_allocations_v1(checked),
    }
}
pub(in crate::production_ranked_projection_v1) fn retired_candidate_content_v1(
    payload: &RetiredLazyProofPayloadsV1,
    phase: u8,
) -> CandidateContentV1 {
    let side = payload.side.graph.is_some()
        || payload.side.definition_counts.is_some()
        || payload.side.block_definitions.is_some()
        || payload.side.address_escaped.is_some()
        || payload.side.assignments.is_some()
        || payload.side.statement_definitions.is_some();
    CandidateContentV1 {
        phase,
        content: genuine_retired_content_parts_v1(
            phase,
            &payload.checked,
            payload.dominance.as_ref(),
            payload.zero_exclusion.as_ref(),
            side,
        ),
        raw: payload.snapshot(),
        logical_work: None, // Retired payload deliberately has no proof/session.
        row_allocations: checked_row_allocations_v1(&payload.checked),
    }
}
const CANDIDATE_ROWS: usize = 10;
fn candidate_content_rows_v1() -> Result<[usize; CANDIDATE_ROWS]> {
    Ok([
        frame::<ContentResult<[(usize, usize, usize); CHECKED_ROW_CAP]>>(size_of::<(
            &[Vec<usize>],
            [(usize, usize, usize); CHECKED_ROW_CAP],
            std::iter::Zip<
                std::slice::IterMut<'static, (usize, usize, usize)>,
                std::slice::Iter<'static, Vec<usize>>,
            >,
            Option<(&mut (usize, usize, usize), &Vec<usize>)>,
            &mut (usize, usize, usize),
            &Vec<usize>,
            (usize, usize, usize),
            bool,
            ContentRefusal,
        )>())?,
        frame::<ContentResult<[(usize, usize, usize); CHECKED_ROW_CAP]>>(size_of::<(
            &LazyFixedProofOwnerV1<'static, 'static, 'static>,
            &RetiredLazyProofPayloadsV1,
            &[Vec<usize>],
            &Vec<Vec<usize>>,
            ContentResult<[(usize, usize, usize); CHECKED_ROW_CAP]>,
        )>())?,
        frame::<CandidateContentV1>(size_of::<(
            &LazyFixedProofOwnerV1<'static, 'static, 'static>,
            &Phase<'static, 'static, 'static>,
            &Building<'static>,
            &PreparedFixedGuardSessionV1<'static>,
            &SemanticAssertProofsV1<'static>,
            u8,
            ContentResult<ProofContent>,
            (u8, ContentResult<ProofContent>),
            bool,
            CandidateContentV1,
            Snapshot,
            Option<usize>,
            usize,
            &[Vec<usize>],
        )>())?,
        frame::<bool>(size_of::<(
            &SemanticAssertProofsV1<'static>,
            &AssertionGraphV1<'static>,
            &AssertionTableV1<'static, u8>,
            &AssertionTableV1<'static, Vec<usize>>,
            &AssertionTableV1<'static, bool>,
            &AssertionTableV1<'static, Option<ScalarAssignmentSiteV1>>,
            &Option<StatementDefinitionIndexV1>,
            bool,
        )>())?,
        frame::<CandidateContentV1>(size_of::<(
            &RetiredLazyProofPayloadsV1,
            u8,
            bool,
            &SideOwners,
            &[Vec<usize>],
            Option<&RetiredAssertionCacheV1>,
            Option<&RetiredAssertionCacheV1>,
            ContentResult<ProofContent>,
            Snapshot,
            CandidateContentV1,
        )>())?,
        frame::<ContentResult<ProofContent>>(size_of::<(
            u8,
            &[Vec<usize>],
            Option<&AssertionCacheV1<'static>>,
            Option<&AssertionCacheV1<'static>>,
            Option<&RetiredAssertionCacheV1>,
            Option<&RetiredAssertionCacheV1>,
            bool,
            ContentResult<ProofContent>,
        )>())?,
        frame::<[usize; CANDIDATE_ROWS]>(size_of::<(
            [usize; CANDIDATE_ROWS],
            Result<[usize; CANDIDATE_ROWS]>,
            &[usize],
        )>())?,
        frame::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            Option<&usize>,
            &usize,
            usize,
            Option<usize>,
            Result<usize>,
            Error,
        )>())?,
        frame::<usize>(size_of::<(
            usize,
            usize,
            usize,
            usize,
            Option<usize>,
            Option<usize>,
            Result<usize>,
            Result<usize>,
            Result<usize>,
            Error,
        )>())?,
        frame::<usize>(size_of::<(usize, usize, Option<usize>, Result<usize>, Error)>())?,
    ])
}
pub(in crate::production_ranked_projection_v1) fn candidate_content_frame_v1() -> Result<usize> {
    let wrappers = sum(&candidate_content_rows_v1()?)?
        .checked_mul(3)
        .ok_or_else(arithmetic)?;
    // The old helper names live/retired raw layouts but explicitly excludes
    // private nonempty-cache getter bodies. Pay those separately, six calls.
    let raw = empty_observation_snapshot_frame_for_test_v1()?
        .checked_mul(3)
        .ok_or_else(arithmetic)?;
    let caches = genuine_cache_raw_frame_v1()?
        .checked_mul(6)
        .ok_or_else(arithmetic)?;
    genuine_candidate_content_frame_v1()?
        .checked_add(wrappers)
        .and_then(|n| n.checked_add(raw))
        .and_then(|n| n.checked_add(caches))
        .ok_or_else(arithmetic)
}
pub(in crate::production_ranked_projection_v1) fn candidate_content_work_v1() -> Result<usize> {
    let bytes = candidate_content_frame_v1()?;
    let b2_bytes = genuine_candidate_content_frame_v1()?;
    let b2_work = genuine_candidate_content_work_v1()?;
    b2_work
        .checked_sub(b2_bytes)
        .and_then(|scan| bytes.checked_add(scan))
        .and_then(|n| {
            CHECKED_ROW_CAP
                .checked_mul(3)
                .and_then(|rows| n.checked_add(rows))
        })
        .ok_or_else(arithmetic)
}

#[test]
fn candidate_content_three_observations_pay_nonempty_raw_getters_separately() {
    assert_eq!(candidate_content_rows_v1().unwrap().len(), CANDIDATE_ROWS);
    assert_eq!(
        candidate_content_frame_v1().unwrap(),
        genuine_candidate_content_frame_v1().unwrap()
            + 3 * candidate_content_rows_v1().unwrap().iter().sum::<usize>()
            + 3 * empty_observation_snapshot_frame_for_test_v1().unwrap()
            + 6 * genuine_cache_raw_frame_v1().unwrap()
    );
    assert!(candidate_content_work_v1().unwrap() > candidate_content_frame_v1().unwrap());
}

#[test]
fn candidate_checked_allocation_rows_are_complete_and_later_rows_matter() {
    let rows = vec![vec![3usize], vec![5usize, 7usize], vec![]];
    let expected = checked_row_allocations_v1(&rows).unwrap();
    assert_eq!(
        expected[1],
        (rows[1].as_ptr() as usize, 2, rows[1].capacity())
    );
    let mut altered = expected;
    altered[1].0 = altered[1].0.wrapping_add(1);
    assert_eq!(altered[0], expected[0]);
    assert_ne!(altered, expected);
    assert_eq!(
        checked_row_allocations_v1(&vec![vec![]; CHECKED_ROW_CAP + 1]),
        Err(ContentRefusal::CheckedRowsLimit)
    );
    // Synthetic identity DATA control only, not genuine source execution.
}
