//! Exact bounded ORIGINAL proof DATA, never a lazy/candidate expectation.
//! The cfg(test)-only caller prepays this observational policy before ownership.
use super::*;
use crate::production_ranked_projection_v1::assertion_resources_v1::original_cache_content::{
    cache_content_frame, live_cache_content, retired_cache_content,
    CacheContent, ContentResult,
};
pub(in crate::production_ranked_projection_v1) use crate::production_ranked_projection_v1::assertion_resources_v1::original_cache_content::ContentRefusal;
pub(in crate::production_ranked_projection_v1) const CHECKED_ROW_CAP: usize = 32;
pub(in crate::production_ranked_projection_v1) const CHECKED_ENTRY_CAP: usize = 128;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct CheckedContent {
    pub rows: usize,
    pub offsets: [usize; CHECKED_ROW_CAP + 1],
    pub entries: [usize; CHECKED_ENTRY_CAP],
    pub used: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct ProofContent {
    pub phase: u8,
    pub checked: CheckedContent,
    pub dominance: Option<CacheContent>,
    pub zero: Option<CacheContent>,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct ContentWitness {
    pub before: Option<ContentResult<ProofContent>>,
    pub after: Option<ContentResult<ProofContent>>,
}
fn checked_content(rows: &[Vec<usize>]) -> ContentResult<CheckedContent> {
    if rows.len() > CHECKED_ROW_CAP {
        return Err(ContentRefusal::CheckedRowsLimit);
    }
    // Reject coverage before copying, never accept a truncated row or table.
    let mut total = 0usize;
    for row in rows {
        total = total
            .checked_add(row.len())
            .ok_or(ContentRefusal::CheckedEntriesLimit)?;
        if total > CHECKED_ENTRY_CAP {
            return Err(ContentRefusal::CheckedEntriesLimit);
        }
    }
    let mut result = CheckedContent {
        rows: rows.len(),
        offsets: [0; CHECKED_ROW_CAP + 1],
        entries: [0; CHECKED_ENTRY_CAP],
        used: 0,
    };
    for (ordinal, row) in rows.iter().enumerate() {
        result.offsets[ordinal] = result.used;
        for entry in row {
            result.entries[result.used] = *entry;
            result.used += 1; // bounded above by the complete checked total.
        }
        result.offsets[ordinal + 1] = result.used;
    }
    Ok(result)
}
fn optional_live(cache: Option<&AssertionCacheV1<'_>>) -> ContentResult<Option<CacheContent>> {
    match cache {
        Some(cache) => Ok(Some(live_cache_content(cache)?)),
        None => Ok(None),
    }
}
fn optional_retired(
    cache: Option<&RetiredAssertionCacheV1>,
) -> ContentResult<Option<CacheContent>> {
    match cache {
        Some(cache) => Ok(Some(retired_cache_content(cache)?)),
        None => Ok(None),
    }
}
fn empty_content() -> ContentResult<ProofContent> {
    Ok(ProofContent {
        phase: 0,
        checked: checked_content(&[])?,
        dominance: None,
        zero: None,
    })
}
fn proof_content(proof: &SemanticAssertProofsV1<'_>) -> ContentResult<ProofContent> {
    if !matches!(proof.graph, AssertionGraphV1::Borrowed(_))
        || !matches!(proof.definition_counts, AssertionTableV1::Borrowed(_))
        || !matches!(proof.block_definitions, AssertionTableV1::Borrowed(_))
        || !matches!(proof.address_escaped, AssertionTableV1::Borrowed(_))
        || !matches!(proof.assignments, AssertionTableV1::Borrowed(_))
        || proof.statement_definitions.is_some()
    {
        return Err(ContentRefusal::OwnedSide);
    }
    Ok(ProofContent {
        phase: 2,
        checked: checked_content(&proof.checked_assertion_blocks)?,
        dominance: optional_live(Some(&proof.dominance))?,
        zero: optional_live(Some(&proof.zero_exclusion))?,
    })
}
fn owner_content(owner: &OriginalFixedOracleOwnerV1<'_, '_, '_>) -> ContentResult<ProofContent> {
    match &owner.phase {
        Phase::Pending(_) | Phase::Unavailable => empty_content(),
        Phase::Building(building) => Ok(ProofContent {
            phase: 1,
            checked: checked_content(&building.checked)?,
            dominance: optional_live(building.dominance.as_ref())?,
            zero: optional_live(building.zero.as_ref())?,
        }),
        Phase::Ready(session) => proof_content(&session.proof),
    }
}
pub(in crate::production_ranked_projection_v1) fn retained_content(
    payload: &RetiredOriginalFixedOracleV1,
    phase: u8,
) -> ContentResult<ProofContent> {
    if side_snapshot(&payload.side).iter().any(|owned| *owned) {
        return Err(ContentRefusal::OwnedSide);
    }
    Ok(ProofContent {
        phase,
        checked: checked_content(&payload.checked)?,
        dominance: optional_retired(payload.dominance.as_ref())?,
        zero: optional_retired(payload.zero.as_ref())?,
    })
}
pub(in crate::production_ranked_projection_v1) fn exact_content_matches(
    expected: &ContentResult<ProofContent>,
    actual: &ContentResult<ProofContent>,
) -> bool {
    // Two matching refusals are NOT complete equal content evidence.
    match (expected, actual) {
        (Ok(expected), Ok(actual)) => expected == actual,
        _ => false,
    }
}
fn admit_content(resources: &mut Prep<'_, '_>) -> R<usize> {
    if !resources.is_metered() || resources.has_denial() {
        return Err(assertion_resource_accounting_v1());
    }
    let bytes = content_added_frame()?;
    let work = bytes
        .checked_add(content_scan_work()?)
        .ok_or_else(assertion_resource_overflow_v1)?;
    resources.work(work)?;
    resources.reserve_storage(bytes)?;
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn retained_content_queries<'a, 'b, 'w>(
    source: QuerySource<'a, 'a>,
    query_source: QuerySource<'_, '_>,
    resources: &'a mut Prep<'b, 'w>,
    queries: &[usize],
    state: &mut QueryState,
    slot: &mut Option<RetiredOriginalFixedOracleV1>,
    constructor_cuts: &mut Cuts,
    cuts: &mut QueryCuts,
    witness: &mut QueryWitness,
    observation: &mut ContentWitness,
) -> R<QuerySummary> {
    if *state != QueryState::Fresh || slot.is_some() {
        return Err(assertion_resource_accounting_v1());
    }
    // Any returning refusal, including admission/cap refusal, is terminal.
    *state = QueryState::Terminal;
    if *witness != QueryWitness::default()
        || !cuts.is_fresh()
        || *observation != ContentWitness::default()
    {
        return Err(assertion_resource_accounting_v1());
    }
    if queries.len() > QUERY_CAP {
        return Err(Error::Unsupported(
            "original query observation exceeds its closed schedule",
        ));
    }
    let prefix = admit_content(resources)?;
    witness.admitted = true;
    witness.prefix = prefix;
    constructor_cuts.prepare_payload();
    cuts.prepare_payload();
    *state = QueryState::Active;
    let reserved = EmptySlot(slot);
    let mut owner = OriginalFixedOracleOwnerV1::new(source.inputs(), resources);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        for (ordinal, guard) in queries.iter().copied().enumerate() {
            witness.stopped_at = Some(ordinal);
            let (index, extent) = owner_query(&mut owner, query_source, guard, constructor_cuts)?;
            witness.summary.initialized = true;
            witness.summary.rows[ordinal] = Some(QueryDatum {
                guard,
                index,
                extent,
            });
            witness.summary.completed += 1;
            cuts.checkpoint();
        }
        witness.stopped_at = None;
        Ok(witness.summary)
    }));
    capture_live(&owner, witness);
    observation.before = Some(owner_content(&owner));
    let before = owner.snapshot();
    witness.before = Some(before);
    // The SAME persistent checked/cache allocations move before return/resume.
    // No new admission, post-denial gate, allocation or callback in transfer.
    reserved.install(retire(owner));
    witness.installed = true;
    witness.after = slot.as_ref().map(|payload| payload.snapshot(before.phase));
    observation.after = slot
        .as_ref()
        .map(|payload| retained_content(payload, before.phase));
    *state = QueryState::Terminal;
    match outcome {
        Ok(result) => result,
        Err(payload) => resume_unwind(payload),
    }
}

// Separate ORIGINAL reference; no retained/lazy/candidate result as expectation.
pub(in crate::production_ranked_projection_v1) fn original_content_queries<'a>(
    source: QuerySource<'a, 'a>,
    query_source: QuerySource<'_, '_>,
    resources: &'a mut Prep<'_, '_>,
    queries: &[usize],
    witness: &mut QueryWitness,
    observation: &mut ContentWitness,
) -> R<QuerySummary> {
    if *witness != QueryWitness::default() || *observation != ContentWitness::default() {
        return Err(assertion_resource_accounting_v1());
    }
    if queries.len() > QUERY_CAP {
        return Err(Error::Unsupported(
            "original query observation exceeds its closed schedule",
        ));
    }
    witness.prefix = admit_content(resources)?;
    witness.admitted = true;
    let mut session = None;
    let mut pending = Some(resources);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        for (ordinal, guard) in queries.iter().copied().enumerate() {
            witness.stopped_at = Some(ordinal);
            if session.is_none() {
                session = Some(new_fixed_guard_session_v1(
                    source.inputs(),
                    pending
                        .take()
                        .ok_or_else(assertion_resource_accounting_v1)?,
                )?);
                witness.summary.initialized = true;
            }
            let current = session.as_mut().expect("original constructor installed");
            let result = current.query_inputs(query_source.inputs(), guard);
            witness.failed = current.failed;
            witness.logical = current.proof.work;
            let (index, extent) = result?;
            witness.summary.initialized = true;
            witness.summary.rows[ordinal] = Some(QueryDatum {
                guard,
                index,
                extent,
            });
            witness.summary.completed += 1;
        }
        witness.stopped_at = None;
        Ok(witness.summary)
    }));
    observation.before = Some(match &session {
        Some(current) => proof_content(&current.proof),
        None if pending.is_some() => empty_content(),
        None => Err(ContentRefusal::OriginalConstructionUnavailable),
    });
    // This independent original owner intentionally drops only after capture.
    // No result/error replacement: a bounded-content refusal stays in DATA.
    match outcome {
        Ok(result) => result,
        Err(payload) => resume_unwind(payload),
    }
}

// Source-level conservative logical policy; not machine stack or RSS.
// Three complete observations cover live -> retired and one postflight read.
// Cache private-layout rows are separate, six invocations regardless of state.
type OriginalContentCatch = (
    &'static mut Option<PreparedFixedGuardSessionV1<'static>>,
    &'static mut Option<&'static mut Prep<'static, 'static>>,
    &'static QuerySource<'static, 'static>,
    &'static QuerySource<'static, 'static>,
    &'static &'static [usize],
    &'static mut &'static mut QueryWitness,
);
const OBSERVATION_PASSES: usize = 3;
const CONTENT_ROWS: usize = 16;
const DRIVER_ROWS: usize = 9;
fn content_rows() -> R<[usize; CONTENT_ROWS]> {
    Ok([
        frame::<ContentResult<CheckedContent>>(size_of::<(
            &[Vec<usize>],
            usize,
            CheckedContent,
            ContentRefusal,
        )>())?,
        frame::<()>(size_of::<(
            std::slice::Iter<'static, Vec<usize>>,
            Option<&Vec<usize>>,
            &Vec<usize>,
            usize,
            Option<usize>,
            ContentRefusal,
            bool,
        )>())?,
        frame::<CheckedContent>(size_of::<(
            usize,
            [usize; CHECKED_ROW_CAP + 1],
            [usize; CHECKED_ENTRY_CAP],
            CheckedContent,
        )>())?,
        frame::<()>(size_of::<(
            std::iter::Enumerate<std::slice::Iter<'static, Vec<usize>>>,
            Option<(usize, &Vec<usize>)>,
            usize,
            &Vec<usize>,
            std::slice::Iter<'static, usize>,
            Option<&usize>,
            &usize,
            usize,
            &mut usize,
        )>())?,
        frame::<ContentResult<Option<CacheContent>>>(size_of::<(
            Option<&AssertionCacheV1<'static>>,
            &AssertionCacheV1<'static>,
            CacheContent,
            ContentResult<CacheContent>,
            Option<CacheContent>,
            ContentRefusal,
        )>())?,
        frame::<ContentResult<Option<CacheContent>>>(size_of::<(
            Option<&RetiredAssertionCacheV1>,
            &RetiredAssertionCacheV1,
            CacheContent,
            ContentResult<CacheContent>,
            Option<CacheContent>,
            ContentRefusal,
        )>())?,
        frame::<ContentResult<ProofContent>>(size_of::<(
            &[Vec<usize>],
            CheckedContent,
            ProofContent,
            Option<CacheContent>,
            ContentResult<CheckedContent>,
        )>())?,
        frame::<ContentResult<ProofContent>>(size_of::<(
            &SemanticAssertProofsV1<'static>,
            &AssertionGraphV1<'static>,
            &AssertionTableV1<'static, u8>,
            &AssertionTableV1<'static, Vec<usize>>,
            &AssertionTableV1<'static, bool>,
            &AssertionTableV1<'static, Option<ScalarAssignmentSiteV1>>,
            &Option<StatementDefinitionIndexV1>,
            &Vec<Vec<usize>>,
            &AssertionCacheV1<'static>,
            bool,
            ContentRefusal,
            CheckedContent,
            Option<CacheContent>,
            ContentResult<CheckedContent>,
            ContentResult<Option<CacheContent>>,
        )>())?,
        frame::<ContentResult<ProofContent>>(size_of::<(
            &OriginalFixedOracleOwnerV1<'static, 'static, 'static>,
            &Phase<'static, 'static, 'static>,
            &Building<'static>,
            &PreparedFixedGuardSessionV1<'static>,
            ProofContent,
            ContentResult<CheckedContent>,
            ContentResult<Option<CacheContent>>,
        )>())?,
        frame::<ContentResult<ProofContent>>(size_of::<(
            &RetiredOriginalFixedOracleV1,
            u8,
            &Side,
            [bool; 6],
            bool,
            CheckedContent,
            Option<CacheContent>,
            ContentRefusal,
        )>())?,
        frame::<bool>(size_of::<(
            [bool; 6],
            std::slice::Iter<'static, bool>,
            Option<&bool>,
            &bool,
            bool,
        )>())?,
        frame::<ContentResult<ProofContent>>(size_of::<(
            u8,
            CheckedContent,
            Option<CacheContent>,
            Option<CacheContent>,
            ProofContent,
            ContentResult<ProofContent>,
            ContentRefusal,
        )>())?,
        frame::<ContentWitness>(size_of::<(
            ContentWitness,
            Option<ContentResult<ProofContent>>,
            Option<ContentResult<ProofContent>>,
            &ContentWitness,
            &ContentWitness,
            bool,
        )>())?,
        frame::<bool>(size_of::<(
            &ContentResult<ProofContent>,
            &ContentResult<ProofContent>,
            &ProofContent,
            &ProofContent,
            &CheckedContent,
            &CheckedContent,
            &CacheContent,
            &CacheContent,
            std::slice::Iter<'static, usize>,
            std::slice::Iter<'static, Option<((usize, usize), bool)>>,
            usize,
            bool,
        )>())?,
        frame::<Option<ContentResult<ProofContent>>>(size_of::<(
            Option<&RetiredOriginalFixedOracleV1>,
            Option<&AssertionCacheV1<'static>>,
            Option<&RetiredAssertionCacheV1>,
            ContentResult<ProofContent>,
            Option<ContentResult<ProofContent>>,
        )>())?,
        frame::<ContentResult<ProofContent>>(size_of::<(
            ContentRefusal,
            ContentResult<CheckedContent>,
            ContentResult<CacheContent>,
            ContentResult<Option<CacheContent>>,
            ContentResult<ProofContent>,
        )>())?,
    ])
}
fn driver_rows() -> R<[usize; DRIVER_ROWS]> {
    Ok([
        // B1's full retained source/catch/owner prefix is retained separately.
        frame::<QuerySummary>(size_of::<(
            &mut ContentWitness,
            ContentWitness,
            ContentResult<ProofContent>,
            Option<ContentResult<ProofContent>>,
        )>())?,
        // New ORIGINAL reference catch: full actual captures plus outcome.
        frame::<QuerySummary>(size_of::<(
            &mut ContentWitness,
            OriginalContentCatch,
            AssertUnwindSafe<OriginalContentCatch>,
            std::thread::Result<R<QuerySummary>>,
            R<QuerySummary>,
            Panic,
            Option<&PreparedFixedGuardSessionV1<'static>>,
            bool,
            ContentResult<ProofContent>,
            ContentRefusal,
        )>())?,
        // Retained Option-map closure captures payload and before.phase witness.
        frame::<Option<ContentResult<ProofContent>>>(size_of::<(
            &RetiredOriginalFixedOracleV1,
            &Snapshot,
            u8,
            ContentResult<ProofContent>,
            Option<ContentResult<ProofContent>>,
            &mut ContentWitness,
        )>())?,
        frame::<usize>(size_of::<(
            &mut Prep<'static, 'static>,
            usize,
            usize,
            R<usize>,
            R<()>,
            Error,
            bool,
        )>())?,
        // Explicit final postflight read/comparison envelope, no meter loan.
        frame::<bool>(size_of::<(
            &RetiredOriginalFixedOracleV1,
            u8,
            ContentResult<ProofContent>,
            &ContentResult<ProofContent>,
            &ContentWitness,
            bool,
        )>())?,
        frame::<[usize; CONTENT_ROWS]>(size_of::<(
            [usize; CONTENT_ROWS],
            R<[usize; CONTENT_ROWS]>,
            [usize; DRIVER_ROWS],
            R<[usize; DRIVER_ROWS]>,
            &[usize],
            std::slice::Iter<'static, usize>,
            &usize,
            usize,
            Option<usize>,
            R<usize>,
        )>())?,
        frame::<usize>(size_of::<(
            usize,
            usize,
            usize,
            usize,
            Option<usize>,
            R<usize>,
            Error,
        )>())?,
        frame::<ContentResult<ProofContent>>(size_of::<(
            &Option<PreparedFixedGuardSessionV1<'static>>,
            Option<&PreparedFixedGuardSessionV1<'static>>,
            &PreparedFixedGuardSessionV1<'static>,
            bool,
            ContentRefusal,
            ContentResult<ProofContent>,
        )>())?,
        frame::<usize>(size_of::<(
            usize,
            usize,
            Option<usize>,
            R<usize>,
            Error,
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1,
        )>())?,
    ])
}
fn sum(rows: &[usize]) -> R<usize> {
    rows.iter().try_fold(0usize, |total, row| {
        total
            .checked_add(*row)
            .ok_or_else(assertion_resource_overflow_v1)
    })
}
pub(in crate::production_ranked_projection_v1) fn content_added_frame() -> R<usize> {
    let snapshots = sum(&content_rows()?)?
        .checked_mul(OBSERVATION_PASSES)
        .ok_or_else(assertion_resource_overflow_v1)?;
    let caches = cache_content_frame()?
        .checked_mul(2 * OBSERVATION_PASSES)
        .ok_or_else(assertion_resource_overflow_v1)?;
    query_added_frame()?
        .checked_add(sum(&driver_rows()?)?)
        .and_then(|n| n.checked_add(snapshots))
        .and_then(|n| n.checked_add(caches))
        .ok_or_else(assertion_resource_overflow_v1)
}
fn content_scan_work() -> R<usize> {
    // Per pass: row-bound scan + row-copy dispatch + checked values + two caches.
    let scan = CHECKED_ROW_CAP.checked_mul(2)
        .and_then(|n| n.checked_add(CHECKED_ENTRY_CAP))
        .and_then(|n| 2usize.checked_mul(
            crate::production_ranked_projection_v1::assertion_resources_v1::original_cache_content::CACHE_CONTENT_CAP
        ).and_then(|c| n.checked_add(c)))
        .ok_or_else(assertion_resource_overflow_v1)?;
    // Fixed DATA equality visits at most one byte-policy unit per byte in each
    // complete ProofContent per pass; this is not measured machine instructions.
    scan.checked_add(size_of::<ProofContent>())
        .and_then(|n| n.checked_mul(OBSERVATION_PASSES))
        .ok_or_else(assertion_resource_overflow_v1)
}
pub(in crate::production_ranked_projection_v1) fn content_added_work() -> R<usize> {
    content_added_frame()?
        .checked_add(content_scan_work()?)
        .ok_or_else(assertion_resource_overflow_v1)
}

#[test]
fn original_content_checked_rows_are_complete_or_explicitly_refused() {
    let actual = checked_content(&[vec![4, 1], vec![], vec![7]]).unwrap();
    assert_eq!(actual.rows, 3);
    assert_eq!(actual.used, 3);
    assert_eq!(&actual.offsets[..4], &[0, 2, 2, 3]);
    assert_eq!(&actual.entries[..3], &[4, 1, 7]);
    assert_eq!(
        checked_content(&vec![vec![]; CHECKED_ROW_CAP + 1]),
        Err(ContentRefusal::CheckedRowsLimit)
    );
    assert_eq!(
        checked_content(&[vec![0; CHECKED_ENTRY_CAP + 1]]),
        Err(ContentRefusal::CheckedEntriesLimit)
    );
    assert_eq!(
        checked_content(&[vec![0; CHECKED_ENTRY_CAP]]).unwrap().used,
        CHECKED_ENTRY_CAP
    );
}
#[test]
fn original_content_comparison_rejects_equal_count_changes_and_equal_refusals() {
    let mut expected = empty_content().unwrap();
    expected.checked = checked_content(&[vec![4, 1], vec![7]]).unwrap();
    let mut changed = expected;
    changed.checked.entries[1] = 2;
    assert_eq!(changed.checked.used, expected.checked.used);
    assert!(!exact_content_matches(&Ok(expected), &Ok(changed)));
    changed = expected;
    changed.checked.entries.swap(0, 1);
    assert!(!exact_content_matches(&Ok(expected), &Ok(changed)));
    changed = expected;
    changed.checked.offsets[1] = 1;
    assert!(!exact_content_matches(&Ok(expected), &Ok(changed)));
    assert!(exact_content_matches(&Ok(expected), &Ok(expected)));
    let refused = Err(ContentRefusal::CheckedEntriesLimit);
    assert!(!exact_content_matches(&refused, &refused));
}
#[test]
fn original_content_owned_side_is_an_explicit_uncovered_boundary() {
    // Inert retirement DATA only: does not construct a ready proof or source.
    let mut payload = empty_retired();
    payload.side.counts = Some(vec![0]);
    assert_eq!(
        retained_content(&payload, 2),
        Err(ContentRefusal::OwnedSide)
    );
}
#[test]
fn original_content_headers_name_complete_subtotals_and_overflow() {
    let snapshots = content_rows().unwrap();
    let drivers = driver_rows().unwrap();
    assert_eq!(snapshots.len(), CONTENT_ROWS);
    assert_eq!(drivers.len(), DRIVER_ROWS);
    assert_eq!(
        content_added_frame().unwrap(),
        query_added_frame().unwrap()
            + drivers.iter().sum::<usize>()
            + OBSERVATION_PASSES * snapshots.iter().sum::<usize>()
            + 2 * OBSERVATION_PASSES * cache_content_frame().unwrap()
    );
    assert_eq!(
        content_added_work().unwrap(),
        content_added_frame().unwrap() + content_scan_work().unwrap()
    );
    assert!(frame::<ProofContent>(usize::MAX).is_err());
    assert!(sum(&[usize::MAX, 1]).is_err());
}
