//! Independent ORIGINAL query/retirement component, cfg(test) only.
//! A synthetic closed schedule is not source-prefix or genuine Fixed evidence.
use super::*;

pub(in crate::production_ranked_projection_v1) const QUERY_CAP: usize = 32;
#[derive(Clone, Copy)]
pub(in crate::production_ranked_projection_v1) struct QuerySource<'a, 'r> {
    pub types: &'a [SemanticTypeDeclV1],
    pub function: &'a SemanticFunctionDeclV1,
    pub graph: &'a ProjectedLoopCfgV1,
    pub rich: &'a Rich<'r>,
}
impl<'a, 'r> QuerySource<'a, 'r> {
    fn inputs(self) -> FixedGuardInputsV1<'a, 'r> {
        FixedGuardInputsV1 {
            types: self.types,
            function: self.function,
            graph: self.graph,
            rich: self.rich,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) enum QueryState {
    Fresh,
    Active,
    Terminal,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct QueryDatum {
    pub guard: usize,
    pub index: SemanticLocalIdV1,
    pub extent: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct QuerySummary {
    pub rows: [Option<QueryDatum>; QUERY_CAP],
    pub completed: usize,
    pub initialized: bool,
}
impl Default for QuerySummary {
    fn default() -> Self {
        Self {
            rows: [None; QUERY_CAP],
            completed: 0,
            initialized: false,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct QueryWitness {
    pub admitted: bool,
    pub prefix: usize,
    pub installed: bool,
    pub before: Option<Snapshot>,
    pub after: Option<Snapshot>,
    pub summary: QuerySummary,
    pub stopped_at: Option<usize>,
    pub failed: bool,
    pub logical: usize,
}
pub(in crate::production_ranked_projection_v1) struct QueryCuts {
    pub after_query: Option<usize>,
    pub seen: usize,
    pub original_address: Option<usize>,
    payload: Option<Panic>,
}
impl QueryCuts {
    pub fn new(after_query: Option<usize>) -> Self {
        Self {
            after_query,
            seen: 0,
            original_address: None,
            payload: None,
        }
    }
    fn is_fresh(&self) -> bool {
        self.seen == 0
            && self.original_address.is_none()
            && self.payload.is_none()
            && self.after_query.is_none_or(|n| n > 0 && n <= QUERY_CAP)
    }
    fn prepare_payload(&mut self) {
        if self.after_query.is_some() {
            let payload: Panic = Box::new([0x6f72696771756572u64, 0x72657461696e6564u64]);
            self.original_address =
                Some(payload.as_ref() as *const (dyn Any + Send) as *const () as usize);
            self.payload = Some(payload);
        }
    }
    fn checkpoint(&mut self) {
        // At most QUERY_CAP successful queries; no unbounded counter growth.
        self.seen += 1;
        if self.after_query == Some(self.seen) {
            resume_unwind(
                self.payload
                    .take()
                    .expect("prepaid original query panic payload"),
            );
        }
    }
}
fn admit_query(resources: &mut Prep<'_, '_>) -> R<usize> {
    if !resources.is_metered() || resources.has_denial() {
        return Err(assertion_resource_accounting_v1());
    }
    let bytes = query_added_frame()?;
    resources.work(bytes)?;
    resources.reserve_storage(bytes)?;
    Ok(bytes)
}
fn owner_query(
    owner: &mut OriginalFixedOracleOwnerV1<'_, '_, '_>,
    source: QuerySource<'_, '_>,
    guard: usize,
    cuts: &mut Cuts,
) -> R<FixedGuardDataV1> {
    if !owner.attempted {
        owner.prepare(cuts)?;
    }
    let Phase::Ready(session) = &mut owner.phase else {
        return Err(assertion_resource_accounting_v1());
    };
    // THE ORIGINAL suffix, including source identity, denial and error order.
    session.query_inputs(source.inputs(), guard)
}
fn capture_live(owner: &OriginalFixedOracleOwnerV1<'_, '_, '_>, witness: &mut QueryWitness) {
    if let Phase::Ready(session) = &owner.phase {
        witness.summary.initialized = true;
        witness.failed = session.failed;
        witness.logical = session.proof.work;
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn retained_queries<'a, 'b, 'w>(
    source: QuerySource<'a, 'a>,
    query_source: QuerySource<'_, '_>,
    resources: &'a mut Prep<'b, 'w>,
    queries: &[usize],
    state: &mut QueryState,
    slot: &mut Option<RetiredOriginalFixedOracleV1>,
    constructor_cuts: &mut Cuts,
    cuts: &mut QueryCuts,
    witness: &mut QueryWitness,
) -> R<QuerySummary> {
    if *state != QueryState::Fresh || slot.is_some() {
        return Err(assertion_resource_accounting_v1());
    }
    // Any returning refusal, including admission/cap refusal, is terminal.
    *state = QueryState::Terminal;
    if *witness != QueryWitness::default() || !cuts.is_fresh() {
        return Err(assertion_resource_accounting_v1());
    }
    if queries.len() > QUERY_CAP {
        return Err(Error::Unsupported(
            "original query observation exceeds its closed schedule",
        ));
    }
    let prefix = admit_query(resources)?;
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
    let before = owner.snapshot();
    witness.before = Some(before);
    // The SAME persistent checked/cache allocations move before return/resume.
    // No new admission, post-denial gate, allocation or callback in transfer.
    reserved.install(retire(owner));
    witness.installed = true;
    witness.after = slot.as_ref().map(|payload| payload.snapshot(before.phase));
    *state = QueryState::Terminal;
    match outcome {
        Ok(result) => result,
        Err(payload) => resume_unwind(payload),
    }
}

/// Original expected path: separate session, same explicit observational prefix.
/// This reference intentionally drops its original payload on return/refusal;
/// it is not evidence of retained physical custody across outer postflights.
pub(in crate::production_ranked_projection_v1) fn original_queries<'a>(
    source: QuerySource<'a, 'a>,
    query_source: QuerySource<'_, '_>,
    resources: &'a mut Prep<'_, '_>,
    queries: &[usize],
    witness: &mut QueryWitness,
) -> R<QuerySummary> {
    if *witness != QueryWitness::default() {
        return Err(assertion_resource_accounting_v1());
    }
    if queries.len() > QUERY_CAP {
        return Err(Error::Unsupported(
            "original query observation exceeds its closed schedule",
        ));
    }
    witness.prefix = admit_query(resources)?;
    witness.admitted = true;
    let mut session = None;
    let mut pending = Some(resources);
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
}

// Explicit source-level conservative policy, NOT machine stack/RSS.
// Unit A's unchanged helper vertices/retirement/nested rows remain a separate
// subtotal. New driver/catch/roster/payload/formula vertices below are additive.
type QueryCatch = (
    &'static mut OriginalFixedOracleOwnerV1<'static, 'static, 'static>,
    &'static QuerySource<'static, 'static>,
    &'static &'static [usize],
    &'static mut &'static mut Cuts,
    &'static mut &'static mut QueryCuts,
    &'static mut &'static mut QueryWitness,
);
const QUERY_ROWS: usize = 21;
fn query_rows() -> R<[usize; QUERY_ROWS]> {
    Ok([
        // retained entry: complete args, input/owner, catch, installation/result
        frame::<QuerySummary>(size_of::<(
            QuerySource<'static, 'static>,
            QuerySource<'static, 'static>,
            &mut Prep<'static, 'static>,
            &[usize],
            &mut QueryState,
            &mut Option<RetiredOriginalFixedOracleV1>,
            &mut Cuts,
            &mut QueryCuts,
            &mut QueryWitness,
            usize,
            EmptySlot<'static>,
            FixedGuardInputsV1<'static, 'static>,
            OriginalFixedOracleOwnerV1<'static, 'static, 'static>,
            std::thread::Result<R<QuerySummary>>,
            Snapshot,
            Panic,
        )>())?,
        // independent reference: Option construction/borrow and actual result
        frame::<QuerySummary>(size_of::<(
            QuerySource<'static, 'static>,
            QuerySource<'static, 'static>,
            &mut Prep<'static, 'static>,
            &[usize],
            &mut QueryWitness,
            Option<PreparedFixedGuardSessionV1<'static>>,
            Option<&mut Prep<'static, 'static>>,
            &mut PreparedFixedGuardSessionV1<'static>,
            R<PreparedFixedGuardSessionV1<'static>>,
            FixedGuardInputsV1<'static, 'static>,
            R<FixedGuardDataV1>,
            SemanticLocalIdV1,
            u64,
            QueryDatum,
            Option<QueryDatum>,
        )>())?,
        // both bounded drive loops: iterator/ordinal/guard and Copy DATA transfer
        frame::<QuerySummary>(size_of::<(
            std::iter::Enumerate<std::iter::Copied<std::slice::Iter<'static, usize>>>,
            Option<(usize, usize)>,
            usize,
            usize,
            R<FixedGuardDataV1>,
            SemanticLocalIdV1,
            u64,
            QueryDatum,
            Option<QueryDatum>,
            &mut QueryWitness,
            QuerySummary,
            Option<usize>,
        )>())?,
        // retained constructor/query dispatch, original suffix has its own roster
        frame::<FixedGuardDataV1>(size_of::<(
            &mut OriginalFixedOracleOwnerV1<'static, 'static, 'static>,
            QuerySource<'static, 'static>,
            usize,
            &mut Cuts,
            &mut Phase<'static, 'static, 'static>,
            &mut PreparedFixedGuardSessionV1<'static>,
            FixedGuardInputsV1<'static, 'static>,
            R<()>,
            R<FixedGuardDataV1>,
            Error,
        )>())?,
        // source argument conversion; no reconstructed source/cache authority
        frame::<FixedGuardInputsV1<'static, 'static>>(size_of::<QuerySource<'static, 'static>>())?,
        // complete exact captures + unwind envelope and result copies
        frame::<R<QuerySummary>>(size_of::<(
            QueryCatch,
            AssertUnwindSafe<QueryCatch>,
            std::thread::Result<R<QuerySummary>>,
            R<QuerySummary>,
            Panic,
        )>())?,
        // postcatch live state and original ready-proof scalar fields
        frame::<()>(size_of::<(
            &OriginalFixedOracleOwnerV1<'static, 'static, 'static>,
            &mut QueryWitness,
            &Phase<'static, 'static, 'static>,
            &PreparedFixedGuardSessionV1<'static>,
            bool,
            usize,
        )>())?,
        // own snapshot Option map capture/return, original snapshot rows separate
        frame::<Option<Snapshot>>(size_of::<(
            Option<&RetiredOriginalFixedOracleV1>,
            &RetiredOriginalFixedOracleV1,
            &Snapshot,
            Snapshot,
            Option<Snapshot>,
            &mut QueryWitness,
        )>())?,
        // final state/result/error/same Box match and resume
        frame::<QuerySummary>(size_of::<(
            &mut QueryState,
            QueryState,
            std::thread::Result<R<QuerySummary>>,
            R<QuerySummary>,
            Panic,
            &(dyn Any + Send),
            Error,
        )>())?,
        // checked admission, original identity/denial and two debit result values
        frame::<usize>(size_of::<(
            &mut Prep<'static, 'static>,
            bool,
            usize,
            R<usize>,
            R<()>,
            Error,
        )>())?,
        // fresh/cap refusal gates, no arbitrary callback or work after denial
        frame::<()>(size_of::<(
            &QueryState,
            QueryState,
            &Option<RetiredOriginalFixedOracleV1>,
            &[usize],
            usize,
            bool,
            Error,
            R<QuerySummary>,
        )>())?,
        // closed checkpoint setup: literal AND boxed payload bytes, trait-object cast
        frame::<()>(size_of::<(
            &mut QueryCuts,
            QueryCuts,
            Option<usize>,
            [u64; 2],
            [u64; 2],
            Box<[u64; 2]>,
            Panic,
            Option<Panic>,
            &(dyn Any + Send),
            *const (dyn Any + Send),
            *const (),
            usize,
        )>())?,
        // checkpoint take/resume and counter; bounded by QUERY_CAP
        frame::<()>(size_of::<(
            &mut QueryCuts,
            usize,
            Option<usize>,
            Option<Panic>,
            Panic,
        )>())?,
        // summary Default fixed array and its return construction
        frame::<QuerySummary>(size_of::<(
            QuerySummary,
            [Option<QueryDatum>; QUERY_CAP],
            usize,
            bool,
        )>())?,
        // QueryCuts constructor return/Option initialization, outside query cuts
        frame::<QueryCuts>(size_of::<(QueryCuts, Option<usize>, Option<Panic>, usize)>())?,
        // witness Default return construction and nested summary result
        frame::<QueryWitness>(size_of::<(
            QueryWitness,
            QuerySummary,
            [Option<QueryDatum>; QUERY_CAP],
            Option<Snapshot>,
            Option<usize>,
            usize,
            bool,
        )>())?,
        // additive formula combines ORIGINAL Unit A subtotal with new rows
        frame::<usize>(size_of::<(usize, usize, Option<usize>, R<usize>, Error)>())?,
        // formula full array, result, iterator and checked accumulator
        frame::<[usize; QUERY_ROWS]>(size_of::<(
            [usize; QUERY_ROWS],
            R<[usize; QUERY_ROWS]>,
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
            R<usize>,
        )>())?,
        // fresh closed recorder/Default comparison and range predicate
        frame::<bool>(size_of::<(
            &QueryCuts,
            Option<usize>,
            Option<&Panic>,
            usize,
            bool,
            QueryWitness,
        )>())?,
        // derived fixed DATA equality used only by the fresh witness gate
        frame::<bool>(size_of::<(
            &QueryWitness,
            &QueryWitness,
            &QuerySummary,
            &QuerySummary,
            std::slice::Iter<'static, Option<QueryDatum>>,
            Option<&Option<QueryDatum>>,
            &Option<QueryDatum>,
            &QueryDatum,
            &Snapshot,
            usize,
            bool,
        )>())?,
        // checked frame helper/overflow result construction (no opaque padding)
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
pub(in crate::production_ranked_projection_v1) fn query_added_frame() -> R<usize> {
    let own = query_rows()?.iter().try_fold(0usize, |total, row| {
        total
            .checked_add(*row)
            .ok_or_else(assertion_resource_overflow_v1)
    })?;
    super::added_frame()?
        .checked_add(own)
        .ok_or_else(assertion_resource_overflow_v1)
}
#[test]
fn original_query_driver_added_rows_are_explicit_and_checked() {
    let rows = query_rows().unwrap();
    assert_eq!(rows.len(), QUERY_ROWS);
    assert_eq!(
        query_added_frame().unwrap(),
        super::added_frame().unwrap() + rows.iter().sum::<usize>()
    );
    assert!(frame::<QuerySummary>(usize::MAX).is_err());
}

#[test]
fn original_query_driver_closed_cut_recording_has_no_reused_or_unbounded_start() {
    let mut cuts = QueryCuts::new(None);
    assert!(cuts.is_fresh());
    cuts.seen = usize::MAX;
    assert!(!cuts.is_fresh());
    for n in [0, QUERY_CAP + 1, usize::MAX] {
        assert!(!QueryCuts::new(Some(n)).is_fresh());
    }
    for n in [1, QUERY_CAP] {
        assert!(QueryCuts::new(Some(n)).is_fresh());
    }
    let mut cuts = QueryCuts::new(None);
    cuts.original_address = Some(1);
    assert!(!cuts.is_fresh());
}
