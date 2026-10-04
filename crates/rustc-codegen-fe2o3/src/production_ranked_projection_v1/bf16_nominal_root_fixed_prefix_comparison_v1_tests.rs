//! Separate authentic ORIGINAL/candidate observation, cfg(test) only.
//! Old five-mode observer and normal kernel/F2 routing are unchanged.
use super::*;
use crate::production_ranked_projection_v1::assertion_analyzer_resources_v1::original_fixed_constructor_retention::{
    ConstructorCuts as Cuts, QueryCuts, QueryState, QueryWitness, ContentWitness, ProofContent,
    Snapshot as OriginalSnapshot, GenuinePrefixRowV1,
    RetiredOriginalFixedOracleV1, retained_content, exact_content_matches,
    GenuinePrefixExpectedV1, GenuinePrefixEventV1, GenuinePrefixStopV1,
    GenuineSourceStampV1, GenuineErrorDataV1, genuine_error_data_v1,
    genuine_source_stamp_v1, with_original_genuine_prefix_v1,
};
use crate::production_ranked_projection_v1::assertion_analyzer_resources_v1::lazy_fixed_proof_owner_v1::retirement::genuine_content::{
    CandidateContentV1, live_candidate_content_v1, retired_candidate_content_v1,
    candidate_content_frame_v1, candidate_content_work_v1,
};
use crate::production_ranked_projection_v1::assertion_resources_v1::original_cache_content::{
    CacheContent, ContentResult, genuine_cache_raw_frame_v1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixedMode {
    Compare,
    CallbackError,
    CallbackPanic,
    OriginalQueryPanic,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Progress {
    #[default]
    NotEntered,
    OriginalEntered,
    OriginalStopped,
    OriginalReadyCandidateNotEntered,
    CandidateEntered,
    CandidateRetired,
    PostflightsComplete,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Compared {
    visited: usize,
    fixed: usize,
    complete: bool,
}
#[derive(Clone, Copy, Debug, Default)]
struct FixedWitness {
    progress: Progress,
    expected: GenuinePrefixExpectedV1,
    original: QueryWitness,
    original_content: ContentWitness,
    original_final: Option<ContentResult<ProofContent>>,
    candidate_live: Option<CandidateContentV1>,
    candidate_retired: Option<CandidateContentV1>,
    candidate_final: Option<CandidateContentV1>,
    namespace: Option<Namespace>,
    compared: Compared,
    original_error: Option<GenuineErrorDataV1>,
    backend_error: Option<GenuineErrorDataV1>,
    factory_error: Option<GenuineErrorDataV1>,
    frames: Option<[usize; 5]>,
    original_same_panic: bool,
    candidate_same_panic: bool,
    original_retired_before_candidate: bool,
    coverage_complete: bool,
    protected_before_drop: bool,
    dropped_before_refund: bool,
    floor_restored: bool,
    mode_skipped: bool,
    before: [usize; 3],
    protected: [usize; 3],
    after: [usize; 3],
    owned: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn project_borrowed(error: &Error) -> QueryError {
    match genuine_error_data_v1(error) {
        GenuineErrorDataV1::Resource(resource) => QueryError::Resource(resource),
        _ => QueryError::Unavailable("authentic fixed-prefix comparison refused"),
    }
}
fn retain_backend_error(error: Error, saved: &mut Option<Error>) -> Error {
    assert!(saved.is_none(), "one actual backend refusal");
    // Closed static/Copy variants have exact semantic DATA, not stack identity.
    // Unknown owning errors remain owned here but never qualify coverage.
    let relay = match &error {
        Error::Incomplete(message) => Error::Incomplete(message),
        Error::Unsupported(message) => Error::Unsupported(message),
        Error::CanonicalAssertions(
            crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error)
        ) => resource(*error),
        _ => Error::Unsupported("uncovered owning fixed-prefix error"),
    };
    *saved = Some(error);
    relay
}
fn finish_optional_frames() -> Result<Option<[usize; 5]>> {
    FACTORY_FRAMES.with(|cell| {
        let (rows, phase) = cell.take().ok_or_else(|| resource(Resource::Accounting))?;
        match phase {
            0 if rows == [0; 5] => Ok(None), // Factory refused before accepted frame hook.
            1 => Ok(Some(rows)),
            _ => Err(resource(Resource::Accounting)), // caught invalid hook stays rejected.
        }
    })
}
fn compare_source_prefix(
    cfg: &NominalRootCfgSourceV1<'_>,
    lazy: &mut LazyFixedProofOwnerV1<'_, '_, '_>,
    expected: &GenuinePrefixExpectedV1,
    compared: &mut Compared,
) -> Result<()> {
    if expected.source != Some(genuine_source_stamp_v1(cfg))
        || !matches!(
            expected.stop,
            GenuinePrefixStopV1::End | GenuinePrefixStopV1::NonFixedBoundary
        )
        || expected.len > expected.rows.len()
        || *compared != Compared::default()
    {
        return Err(resource(Resource::Accounting));
    }
    for row in &expected.rows[..expected.len] {
        let source = cfg
            .function()
            .blocks()
            .get(row.block)
            .ok_or(Error::Unsupported(
                "genuine candidate source coordinate absent",
            ))?;
        if source as *const _ as usize != row.source_address || row.block != compared.visited {
            return Err(Error::Unsupported(
                "genuine candidate source coordinate differs",
            ));
        }
        let success = match source.terminator().kind() {
            SemanticTerminatorKindV1::Assert {
                message: SemanticAssertMessageV1::BoundsCheck { .. },
                target,
                ..
            } => Some(target.target().index() as usize),
            _ => None,
        };
        if success != row.success {
            return Err(Error::Unsupported(
                "genuine candidate source successor differs",
            ));
        }
        let actual = lazy.visit(row.block)?;
        let matches = match (row.event, actual) {
            (GenuinePrefixEventV1::Other, LazyFixedEventV1::Other)
            | (GenuinePrefixEventV1::LiteralSkip { .. }, LazyFixedEventV1::LiteralSkip)
            | (GenuinePrefixEventV1::NonFixedBoundary, LazyFixedEventV1::NonFixedBoundary) => true,
            (
                GenuinePrefixEventV1::Fixed { index, extent },
                LazyFixedEventV1::Fixed {
                    index: actual_index,
                    extent: actual_extent,
                },
            ) => {
                if index == actual_index && extent == actual_extent {
                    compared.fixed += 1;
                    true
                } else {
                    false
                }
            }
            _ => false,
        };
        if !matches {
            return Err(Error::Unsupported("genuine candidate event DATA differs"));
        }
        compared.visited += 1;
    }
    if compared.fixed != expected.fixed || compared.visited != expected.len {
        return Err(Error::Unsupported(
            "genuine candidate prefix termination differs",
        ));
    }
    compared.complete = true;
    Ok(())
}
fn check_candidate_pending(
    pending: &PendingActualRootPrefixIndicesV1,
    namespace: Namespace,
    successful_return: bool,
) -> Result<()> {
    let phase = if successful_return {
        Phase::InitializedBeforeArgumentWriters
    } else {
        Phase::Terminal
    };
    if !pending.started
        || pending.completed
        || !pending.indices.indices.is_empty()
        || !pending.indices.index_fifo.is_empty()
        || !pending.arguments.initialized
        || !pending.arguments.later.vacant()
        || pending.arguments.function != Some(namespace.function)
        || pending.arguments.next_argument != namespace.next_argument
        || pending.arguments.index_arguments.as_ptr() as usize != namespace.index_address
        || pending.arguments.slice_arguments.as_ptr() as usize != namespace.slice_address
        || pending.prefix.entry_operations.as_ptr() as usize != namespace.operations_address
        || pending.prefix.entry_operations.len() != namespace.operations
        || pending.prefix.next_value != namespace.next_value
        || pending.retired_fixed_proof.is_none()
        || pending.arguments.phase != phase
    {
        return Err(resource(Resource::Accounting));
    }
    Ok(())
}
fn complete_content(
    before: &Option<ContentResult<ProofContent>>,
    after: &Option<ContentResult<ProofContent>>,
) -> bool {
    before
        .as_ref()
        .zip(after.as_ref())
        .is_some_and(|(before, after)| exact_content_matches(before, after))
}
fn run_fixed(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    budget: &mut Budget<'_>,
    mode: FixedMode,
    witness: &mut FixedWitness,
) -> Q<()> {
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        let slot = budget as *const Budget<'_> as usize;
        let ledger = budget.work_ledger_identity_v1();
        let floor = budget.storage();
        witness.before = [budget.work(), floor, budget.peak_storage()];
        let header = admit_fixed_observer(budget)?;
        let mut owned = header;
        let mut pending = PendingActualRootPrefixIndicesV1::new();
        let mut original_slot: Option<RetiredOriginalFixedOracleV1> = None;
        let mut original_state = QueryState::Fresh;
        let mut constructor_cuts = Cuts::new(None);
        let mut query_cuts = QueryCuts::new(if mode == FixedMode::OriginalQueryPanic { Some(1) } else { None });
        let mut original_error = None;
        let mut backend_error = None;
        let mut factory_error = None;
        let mut observation_error = None;
        let mut panic_payload: Option<PanicPayload> = if mode == FixedMode::CallbackPanic {
            Some(Box::new([0x623367656e75696eu64, 0x73616d65626f7831u64]))
        } else { None };
        let panic_address = panic_payload.as_ref()
            .map(|p| p.as_ref() as *const (dyn Any + Send) as *const () as usize);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            owner.with_checked_bf16_nominal_call_v1(
                inventory, source.root(), source.root(), source.call_block(), source.source_call(),
                budget, |checked, budget| {
                    with_nominal_root_cfg_preparation_v1(
                        owner, inventory, source.root(), source.root(), source.call_block(), source.source_call(),
                        budget, |cfg, budget| {
                            budget.charge_work(cfg.function().locals().len().checked_mul(2)
                                .ok_or(QueryError::Resource(Resource::Arithmetic))?)?;
                            witness.progress = Progress::OriginalEntered;
                            // The original Prep loan ends at retirement; its accepted
                            // credits and physical payload do not end here.
                            let original = catch_unwind(AssertUnwindSafe(|| {
                                let mut resources = Prep::new(budget, &mut owned);
                                with_original_genuine_prefix_v1(
                                    cfg, &mut resources, &mut original_state, &mut original_slot,
                                    &mut constructor_cuts, &mut query_cuts, &mut witness.original,
                                    &mut witness.original_content, &mut witness.expected,
                                )
                            }));
                            match original {
                                Ok(Ok(_)) => {
                                    witness.progress = Progress::OriginalReadyCandidateNotEntered;
                                    if original_slot.is_none() || witness.original.before.is_none()
                                        || witness.original.before != witness.original.after
                                        || !complete_content(&witness.original_content.before, &witness.original_content.after)
                                    {
                                        return Err(QueryError::Unavailable("original complete retention unavailable"));
                                    }
                                }
                                Ok(Err(error)) => {
                                    witness.progress = Progress::OriginalStopped;
                                    witness.original_error = Some(genuine_error_data_v1(&error));
                                    let projected = project_borrowed(&error);
                                    original_error = Some(error);
                                    return Err(projected);
                                }
                                Err(payload) => {
                                    witness.progress = Progress::OriginalStopped;
                                    if original_slot.is_some() {
                                        witness.original_same_panic = Some(payload.as_ref() as *const (dyn Any + Send) as *const () as usize)
                                            == query_cuts.original_address;
                                    }
                                    // Unit A/B2 installed the SAME payload before resume.
                                    resume_unwind(payload)
                                }
                            }
                            if mode == FixedMode::OriginalQueryPanic {
                                // No fixed query was reached: report a skipped cut, never
                                // manufacture a ready owner/query merely to exercise it.
                                witness.mode_skipped = true;
                                return Ok(());
                            }
                            witness.original_retired_before_candidate = true;
                            with_nominal_canonical_facts_observation_v1(
                                owner, inventory, source.root(), source.root(), source.call_block(), source.source_call(),
                                budget, |facts| {
                                    with_nominal_recipe_resources_v1(
                                        facts, cfg.source_tables().rich(), &mut owned, |context| {
                                            start_frames();
                                            let immediate = catch_unwind(AssertUnwindSafe(|| {
                                                context.with_actual_root_retired_fixed_proof_observation_v1(
                                                    checked, cfg, actual_inputs, &mut pending, |view, lazy| {
                                                        witness.progress = Progress::CandidateEntered;
                                                        // Catch while lazy is still live so a query
                                                        // unwind also gets a complete bounded observation.
                                                        let observed = catch_unwind(AssertUnwindSafe(|| {
                                                            witness.namespace = Some(observe_namespace(&view, cfg));
                                                            compare_source_prefix(cfg, lazy, &witness.expected, &mut witness.compared)?;
                                                            match mode {
                                                                FixedMode::CallbackError => Err(Error::Incomplete("genuine fixed-prefix callback refusal")),
                                                                FixedMode::CallbackPanic => resume_unwind(panic_payload.take().expect("one prepaid same panic")),
                                                                _ => Ok(()),
                                                            }
                                                        }));
                                                        witness.candidate_live = Some(live_candidate_content_v1(lazy)); // Candidate read 1.
                                                        match observed {
                                                            Ok(Ok(())) => Ok(()),
                                                            Ok(Err(error)) => {
                                                                witness.backend_error = Some(genuine_error_data_v1(&error));
                                                                Err(retain_backend_error(error, &mut backend_error))
                                                            }
                                                            Err(payload) => resume_unwind(payload),
                                                        }
                                                    },
                                                )
                                            }));
                                            // Never replace an in-flight original panic/Err
                                            // with a new observation refusal before resuming it.
                                            match finish_optional_frames() {
                                                Ok(frames) => witness.frames = frames,
                                                Err(error) => observation_error = Some(error),
                                            }
                                            if let (Some(live), Some(retired), Some(namespace)) =
                                                (witness.candidate_live, pending.retired_fixed_proof.as_ref(), witness.namespace)
                                            {
                                                if let Err(error) = check_candidate_pending(&pending, namespace, matches!(&immediate, Ok(Ok(())))) {
                                                    if observation_error.is_none() { observation_error = Some(error); }
                                                }
                                                let observed = retired_candidate_content_v1(retired, live.phase); // Candidate read 2.
                                                witness.candidate_retired = Some(observed);
                                                if observed.raw != live.raw
                                                    || live.row_allocations.is_err() || observed.row_allocations != live.row_allocations
                                                    || !exact_content_matches(&live.content, &observed.content)
                                                {
                                                    if observation_error.is_none() {
                                                        observation_error = Some(Error::Unsupported("candidate retirement content or allocation differs"));
                                                    }
                                                }
                                                witness.progress = Progress::CandidateRetired;
                                            }
                                            match immediate {
                                                Ok(Ok(())) => match observation_error.as_ref() {
                                                    None => Ok(()),
                                                    Some(_) => Err(Error::Unsupported("candidate observation refused")),
                                                },
                                                Ok(Err(error)) => {
                                                    witness.factory_error = Some(genuine_error_data_v1(&error));
                                                    let relay = retain_backend_error(error, &mut factory_error);
                                                    Err(relay)
                                                }
                                                Err(payload) => {
                                                    witness.candidate_same_panic =
                                                        Some(payload.as_ref() as *const (dyn Any + Send) as *const () as usize) == panic_address;
                                                    // Existing facts/CFG catch retains its priority.
                                                    resume_unwind(payload)
                                                }
                                            }
                                        },
                                    ).map_err(project_borrowed_owned)
                                },
                            )
                        },
                    )
                },
            )
        }));
        let result = match outcome {
            Ok(result) => result,
            Err(payload) => { drop(payload); Err(QueryError::CallbackPanicked) },
        };
        // All returning inner wrappers (or their unchanged panic conversion)
        // have completed; both lifetime-free slots remain owned here.
        if let (Some(payload), Some(before)) = (original_slot.as_ref(), witness.original.before) {
            witness.original_final = Some(retained_content(payload, before.phase)); // Original read 3.
            if payload.snapshot(before.phase) != before {
                witness.coverage_complete = false;
            } else {
                witness.coverage_complete = complete_content(&witness.original_content.before, &witness.original_final);
            }
        }
        if let (Some(retired), Some(live)) = (pending.retired_fixed_proof.as_ref(), witness.candidate_live) {
            let observed = retired_candidate_content_v1(retired, live.phase); // Candidate read 3.
            witness.candidate_final = Some(observed);
            witness.coverage_complete &= observed.raw == live.raw
                && live.row_allocations.is_ok() && observed.row_allocations == live.row_allocations
                && exact_content_matches(&live.content, &observed.content)
                && (live.phase != 2 || live.logical_work == Some(witness.original.logical))
                && witness.original_content.before.as_ref()
                    .is_some_and(|expected| exact_content_matches(expected, &live.content));
        } else if witness.original_retired_before_candidate {
            witness.coverage_complete = false;
        }
        witness.coverage_complete &= observation_error.is_none();
        let protected = floor.checked_add(owned);
        witness.protected = [budget.work(), budget.storage(), budget.peak_storage()];
        witness.owned = owned;
        witness.failed_work = budget.failed_work();
        witness.failed_storage = budget.failed_storage();
        let custody = slot == budget as *const Budget<'_> as usize
            && ledger == budget.work_ledger_identity_v1()
            && protected == Some(budget.storage())
            && budget.work() >= witness.before[0]
            && budget.peak_storage() >= witness.before[2];
        witness.protected_before_drop = custody;
        // No retained physical owner or owning error survives its credit refund.
        drop(original_slot);
        drop(pending);
        drop(original_error);
        drop(backend_error);
        drop(factory_error);
        drop(observation_error);
        drop(query_cuts);
        drop(constructor_cuts);
        drop(panic_payload);
        witness.dropped_before_refund = true;
        if !custody { return Err(QueryError::Resource(Resource::Accounting)); }
        // Sticky denial does not prevent valid-custody cleanup.
        budget.release_storage(owned)?;
        witness.floor_restored = budget.storage() == floor;
        witness.after = [budget.work(), budget.storage(), budget.peak_storage()];
        if !witness.floor_restored
            || budget.failed_work() != witness.failed_work
            || budget.failed_storage() != witness.failed_storage
        {
            return Err(QueryError::Resource(Resource::Accounting));
        }
        if result.is_ok() && (!witness.coverage_complete || budget.failed_work().is_some()
            || budget.failed_storage().is_some())
        {
            return Err(QueryError::Unavailable("genuine complete content coverage refused"));
        }
        if witness.progress == Progress::CandidateRetired {
            witness.progress = Progress::PostflightsComplete;
        }
        result
    })
}

// Complete conservative SOURCE-level logical policy; not machine-stack, RSS,
// allocator overcapacity, or bridge-credit measurement. FixedRunCaptureEnvelope
// names all source captures (including precise witness fields) for each nested
// callback/catch. Reusing the full named superset is conservative; it is not an
// opaque lexical allowance. Each independently constructed envelope has a row.
type FixedRunArguments = (
    &'static ProductionPreRankedKirOwnerV1,
    &'static CheckedBf16CallInstanceV1<'static>,
    &'static CanonicalKirInventoryV1<'static>,
    &'static crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
    &'static mut Budget<'static>,
);
type FixedWitnessCaptureEnvelope = (
    &'static mut Progress,
    &'static mut GenuinePrefixExpectedV1,
    &'static mut QueryWitness,
    &'static mut ContentWitness,
    &'static mut Option<ContentResult<ProofContent>>,
    &'static mut Option<CandidateContentV1>,
    &'static mut Option<CandidateContentV1>,
    &'static mut Option<CandidateContentV1>,
    &'static mut Option<Namespace>,
    &'static mut Compared,
    &'static mut Option<GenuineErrorDataV1>,
    &'static mut Option<GenuineErrorDataV1>,
    &'static mut Option<GenuineErrorDataV1>,
    &'static mut Option<[usize; 5]>,
    &'static mut bool,
    &'static mut bool,
    &'static mut bool,
    &'static mut bool,
    &'static mut bool,
    &'static mut bool,
    &'static mut bool,
    &'static mut bool,
    &'static mut [usize; 3],
    &'static mut [usize; 3],
    &'static mut [usize; 3],
    &'static mut usize,
    &'static mut Option<usize>,
    &'static mut Option<usize>,
);
type FixedRunCaptureEnvelope = (
    &'static &'static ProductionPreRankedKirOwnerV1,
    &'static &'static CheckedBf16CallInstanceV1<'static>,
    &'static &'static CanonicalKirInventoryV1<'static>,
    &'static &'static crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
    &'static mut &'static mut Budget<'static>,
    &'static FixedMode,
    FixedWitnessCaptureEnvelope,
    &'static mut usize,
    &'static mut PendingActualRootPrefixIndicesV1,
    &'static mut Option<RetiredOriginalFixedOracleV1>,
    &'static mut QueryState,
    &'static mut Cuts,
    &'static mut QueryCuts,
    &'static mut Option<Error>,
    &'static mut Option<Error>,
    &'static mut Option<Error>,
    &'static mut Option<Error>,
    &'static mut Option<PanicPayload>,
    &'static Option<usize>,
);
const FIXED_ROWS: usize = 43;
fn fixed_observer_rows() -> Result<[usize; FIXED_ROWS]> {
    Ok([
        // row 1: entry
        call_frame::<Q<()>>(size_of::<(FixedRunArguments, [FixedMode; 4], std::array::IntoIter<FixedMode, 4>, FixedMode, FixedWitness, Q<()>, usize, usize, usize, Option<usize>)>())?,
        // row 2: run_args
        call_frame::<Q<()>>(size_of::<(FixedRunArguments, &mut FixedWitness, FixedMode)>())?,
        // row 3: run_locals
        call_frame::<Q<()>>(size_of::<(PendingActualRootPrefixIndicesV1, Option<RetiredOriginalFixedOracleV1>, QueryState, Cuts, QueryCuts, Option<Error>, Option<Error>, Option<Error>, Option<Error>, Option<PanicPayload>, Option<usize>, LedgerId, usize, usize, usize, usize, usize, Option<usize>, [usize; 3], bool, Q<()>, std::thread::Result<Q<()>>)>())?,
        // row 4: entry_callback
        call_frame::<Q<()>>(size_of::<(FixedRunCaptureEnvelope, &mut Budget<'static>)>())?,
        // row 5: outer_catch
        call_frame::<Q<()>>(size_of::<(FixedRunCaptureEnvelope, AssertUnwindSafe<FixedRunCaptureEnvelope>, std::thread::Result<Q<()>>, PanicPayload)>())?,
        // row 6: checked_callback
        call_frame::<Q<()>>(size_of::<(FixedRunCaptureEnvelope, &CheckedBf16NominalCallV1<'static>, &mut Budget<'static>)>())?,
        // row 7: cfg_callback
        call_frame::<Q<()>>(size_of::<(FixedRunCaptureEnvelope, &CheckedBf16NominalCallV1<'static>, &NominalRootCfgSourceV1<'static>, &mut Budget<'static>, usize, Option<usize>)>())?,
        // row 8: original_catch
        call_frame::<Result<GenuinePrefixExpectedV1>>(size_of::<(FixedRunCaptureEnvelope, &NominalRootCfgSourceV1<'static>, AssertUnwindSafe<FixedRunCaptureEnvelope>, Prep<'static, 'static>, std::thread::Result<Result<GenuinePrefixExpectedV1>>, Result<GenuinePrefixExpectedV1>, GenuinePrefixExpectedV1, Error, QueryError, PanicPayload)>())?,
        // row 9: facts_callback
        call_frame::<Q<()>>(size_of::<(FixedRunCaptureEnvelope, &NominalRootCfgSourceV1<'static>, &CheckedBf16NominalCallV1<'static>, &mut CanonicalSourceAssertionFactsV1<'static, 'static, 'static, 'static, 'static>)>())?,
        // row 10: recipe_callback
        call_frame::<Result<()>>(size_of::<(FixedRunCaptureEnvelope, &NominalRootCfgSourceV1<'static>, &CheckedBf16NominalCallV1<'static>, &mut Context)>())?,
        // row 11: factory_catch
        call_frame::<Result<()>>(size_of::<(FixedRunCaptureEnvelope, &mut &mut Context, &&CheckedBf16NominalCallV1<'static>, &&NominalRootCfgSourceV1<'static>, AssertUnwindSafe<FixedRunCaptureEnvelope>, std::thread::Result<Result<()>>, Result<()>, PanicPayload)>())?,
        // row 12: factory_callback
        call_frame::<Result<()>>(size_of::<(FixedRunCaptureEnvelope, &NominalRootCfgSourceV1<'static>, ActualRootArgumentInitializationV1<'static>, &mut LazyFixedProofOwnerV1<'static, 'static, 'static>)>())?,
        // row 13: candidate_catch
        call_frame::<Result<()>>(size_of::<(FixedRunCaptureEnvelope, &ActualRootArgumentInitializationV1<'static>, &&NominalRootCfgSourceV1<'static>, &mut &mut LazyFixedProofOwnerV1<'static, 'static, 'static>, AssertUnwindSafe<FixedRunCaptureEnvelope>, std::thread::Result<Result<()>>, Result<()>, PanicPayload, Error)>())?,
        // row 14: immediate_postflight
        call_frame::<Result<()>>(size_of::<(Result<Option<[usize; 5]>>, Option<[usize; 5]>, Option<CandidateContentV1>, Option<&RetiredLazyProofPayloadsV1>, Option<Namespace>, CandidateContentV1, &RetiredLazyProofPayloadsV1, Namespace, Result<()>, Error, bool, Option<&Error>)>())?,
        // row 15: original_final_snapshot
        call_frame::<OriginalSnapshot>(size_of::<(&RetiredOriginalFixedOracleV1, u8, OriginalSnapshot, Option<OriginalSnapshot>, (usize, usize, usize), [(usize, usize, usize); 32], bool, [bool; 6], Option<(u8, usize, usize, usize)>, Option<(u8, usize, usize, usize)>)>())?,
        // row 16: original_final_rows
        call_frame::<((usize, usize, usize), [(usize, usize, usize); 32], bool)>(size_of::<(&Vec<Vec<usize>>, [(usize, usize, usize); 32], std::iter::Zip<std::slice::IterMut<'static, (usize, usize, usize)>, std::slice::Iter<'static, Vec<usize>>>, Option<(&mut (usize, usize, usize), &Vec<usize>)>, &mut (usize, usize, usize), &Vec<usize>, (usize, usize, usize), bool)>())?,
        // row 17: original_final_side
        call_frame::<[bool; 6]>(size_of::<(&RetiredOriginalFixedOracleV1, [bool; 6], bool)>())?,
        // row 18: final_observations
        call_frame::<bool>(size_of::<(Option<&RetiredOriginalFixedOracleV1>, Option<OriginalSnapshot>, &RetiredOriginalFixedOracleV1, OriginalSnapshot, ContentResult<ProofContent>, Option<ContentResult<ProofContent>>, Option<&RetiredLazyProofPayloadsV1>, Option<CandidateContentV1>, CandidateContentV1, &RetiredLazyProofPayloadsV1, bool)>())?,
        // row 19: compare_source
        call_frame::<Result<()>>(size_of::<(&NominalRootCfgSourceV1<'static>, &mut LazyFixedProofOwnerV1<'static, 'static, 'static>, &GenuinePrefixExpectedV1, &mut Compared, GenuineSourceStampV1, Option<GenuineSourceStampV1>, Compared, bool)>())?,
        // row 20: compare_rows
        call_frame::<Result<()>>(size_of::<(&[GenuinePrefixRowV1], std::slice::Iter<'static, GenuinePrefixRowV1>, Option<&GenuinePrefixRowV1>, &GenuinePrefixRowV1, Option<&SemanticBasicBlockV1>, &SemanticBasicBlockV1, &SemanticTerminatorKindV1, Option<usize>, Result<LazyFixedEventV1>, LazyFixedEventV1, GenuinePrefixEventV1, SemanticLocalIdV1, SemanticLocalIdV1, u64, u64, bool)>())?,
        // row 21: namespace
        call_frame::<Namespace>(size_of::<(&ActualRootArgumentInitializationV1<'static>, &NominalRootCfgSourceV1<'static>, usize, Namespace, &[Option<u32>], &[Option<u32>], &[ProductionRankedOperationV1])>())?,
        // row 22: namespace_index_iteration
        call_frame::<bool>(size_of::<(std::slice::Iter<'static, Option<u32>>, Option<&Option<u32>>, &Option<u32>, bool)>())?,
        // row 23: namespace_slice_iteration
        call_frame::<bool>(size_of::<(std::slice::Iter<'static, Option<u32>>, Option<&Option<u32>>, &Option<u32>, bool)>())?,
        // row 24: pending_postflight
        call_frame::<Result<()>>(size_of::<(&PendingActualRootPrefixIndicesV1, Namespace, bool, Phase, Phase, Option<usize>, bool)>())?,
        // row 25: content_comparison
        call_frame::<bool>(size_of::<(&Option<ContentResult<ProofContent>>, &Option<ContentResult<ProofContent>>, Option<&ContentResult<ProofContent>>, Option<&ContentResult<ProofContent>>, Option<(&ContentResult<ProofContent>, &ContentResult<ProofContent>)>, &ContentResult<ProofContent>, &ContentResult<ProofContent>, bool, ProofContent, ProofContent)>())?,
        // row 26: content_comparison_closure
        call_frame::<bool>(size_of::<((&ContentResult<ProofContent>, &ContentResult<ProofContent>), bool, Option<&ContentResult<ProofContent>>)>())?,
        // row 27: error_projection
        call_frame::<QueryError>(size_of::<(&Error, GenuineErrorDataV1, Resource, QueryError, Error)>())?,
        // row 28: error_retention
        call_frame::<Error>(size_of::<(Error, &mut Option<Error>, &Error, Error, GenuineErrorDataV1, Resource, &&str)>())?,
        // row 29: tls_start
        call_frame::<()>(size_of::<(&Cell<Option<([usize; 5], usize)>>, Option<([usize; 5], usize)>, [usize; 5], usize, bool)>())?,
        // row 30: tls_optional_finish
        call_frame::<Result<Option<[usize; 5]>>>(size_of::<(&Cell<Option<([usize; 5], usize)>>, Option<([usize; 5], usize)>, ([usize; 5], usize), [usize; 5], usize, Option<[usize; 5]>, bool, Error)>())?,
        // row 31: panic_box_and_identity
        call_frame::<()>(size_of::<([u64; 2], Box<[u64; 2]>, PanicPayload, PanicPayload, Option<PanicPayload>, Option<&PanicPayload>, &(dyn Any + Send), *const (), Option<usize>, usize)>())?,
        // row 32: custody
        call_frame::<bool>(size_of::<(&Budget<'static>, usize, usize, LedgerId, LedgerId, usize, Option<usize>, Option<usize>, Option<usize>, [usize; 3], bool)>())?,
        // row 33: drop_refund
        call_frame::<Q<()>>(size_of::<(Option<RetiredOriginalFixedOracleV1>, PendingActualRootPrefixIndicesV1, Option<Error>, Option<Error>, Option<Error>, Option<Error>, QueryCuts, Cuts, Option<PanicPayload>, &mut Budget<'static>, usize, Q<()>, bool)>())?,
        // row 34: logging
        call_frame::<()>(size_of::<(&FixedWitness, FixedMode, std::fmt::Arguments<'static>, &[GenuinePrefixRowV1], std::slice::Iter<'static, GenuinePrefixRowV1>, &GenuinePrefixRowV1, usize, Option<usize>)>())?,
        // row 35: total_summary
        call_frame::<Q<()>>(size_of::<(usize, usize, usize, Option<usize>, &Budget<'static>, std::fmt::Arguments<'static>, Q<()>)>())?,
        // row 36: admission
        call_frame::<Q<usize>>(size_of::<(&mut Budget<'static>, usize, usize, Result<usize>, Q<usize>, Q<()>, std::result::Result<(), Resource>)>())?,
        // row 37: frame_formula
        call_frame::<usize>(size_of::<([usize; FIXED_ROWS], Result<[usize; FIXED_ROWS]>, &[usize], usize, usize, usize, usize, Result<usize>, Option<usize>, Option<usize>)>())?,
        // row 38: work_formula
        call_frame::<usize>(size_of::<(usize, usize, usize, usize, usize, Result<usize>, Option<usize>, Option<usize>)>())?,
        // row 39: row_array
        call_frame::<[usize; FIXED_ROWS]>(size_of::<([usize; FIXED_ROWS], Option<usize>)>())?,
        // row 41: candidate_source_stamp_and_getter_returns
        call_frame::<GenuineSourceStampV1>(size_of::<(&NominalRootCfgSourceV1<'static>, &SemanticFunctionDeclV1, &[SemanticTypeDeclV1], &crate::production_ranked_projection_v1::ProjectedLoopCfgV1, &crate::production_ranked_projection_v1::bf16_nominal_source_preparation_v1::RichNominalSourceTablesV1<'static>, GenuineSourceStampV1, usize, usize, usize, usize, usize)>())?,
        // row 42: cache_telemetry_option_and_borrow_returns
        call_frame::<Option<[usize; 2]>>(size_of::<(&Option<ContentResult<ProofContent>>, Option<&ContentResult<ProofContent>>, Option<&ProofContent>, &ProofContent, Option<&CacheContent>, &CacheContent, Option<usize>, usize, [usize; 2], Option<[usize; 2]>)>())?,
        // row 43: source_stamp_source_table_getter
        call_frame::<&crate::production_ranked_projection_v1::bf16_nominal_source_preparation_v1::RichNominalSourceTablesV1<'static>>(size_of::<(&NominalRootCfgSourceV1<'static>, &crate::production_ranked_projection_v1::bf16_nominal_source_preparation_v1::NominalRootSourceTablesV1<'static>, &crate::production_ranked_projection_v1::bf16_nominal_source_preparation_v1::RichNominalSourceTablesV1<'static>)>())?,
        // row 40: checked_sum
        call_frame::<usize>(size_of::<(&[usize], std::slice::Iter<'static, usize>, usize, &usize, Option<usize>, Result<usize>, Error)>())?,
    ])
}
fn fixed_observer_frame() -> Result<usize> {
    let own = sum(&fixed_observer_rows()?)?;
    // Exactly three actual-side B2 DATA reads, with all raw cache getter rows.
    let candidate = candidate_content_frame_v1()?;
    // The final ORIGINAL raw snapshot is additional to B1/B2's before/after.
    // Its wrapper/checked-row/side rows are named above; two private cache
    // getter paths are paid here. Earlier original paths remain paid by B2.
    let original_raw = genuine_cache_raw_frame_v1()?
        .checked_mul(2)
        .ok_or_else(arithmetic)?;
    own.checked_add(candidate)
        .and_then(|n| n.checked_add(original_raw))
        .ok_or_else(arithmetic)
}
fn fixed_observer_work() -> Result<usize> {
    let frame = fixed_observer_frame()?;
    let candidate_frame = candidate_content_frame_v1()?;
    let candidate_work = candidate_content_work_v1()?;
    let candidate_scans = candidate_work
        .checked_sub(candidate_frame)
        .ok_or_else(arithmetic)?;
    // Candidate prefix scans at most 32 source rows. Complete content equality
    // has at most eight comparisons of named bounded ProofContent DATA: before/
    // after original, live/retired actual, final original, final actual,
    // cross-owner final, three mode/result checks. Byte policy is conservative.
    let content_comparisons = size_of::<ProofContent>()
        .checked_mul(8)
        .ok_or_else(arithmetic)?;
    frame
        .checked_add(candidate_scans)
        .and_then(|n| n.checked_add(content_comparisons))
        .and_then(|n| n.checked_add(32))
        .ok_or_else(arithmetic)
}
fn admit_fixed_observer(budget: &mut Budget<'_>) -> Q<usize> {
    let frame = fixed_observer_frame().map_err(project_borrowed_owned)?;
    let work = fixed_observer_work().map_err(project_borrowed_owned)?;
    budget.charge_work(work)?;
    budget.reserve_storage(frame)?;
    Ok(frame)
}

#[test]
fn genuine_fixed_observer_header_is_typed_checked_and_not_state_dependent() {
    let rows = fixed_observer_rows().unwrap();
    assert_eq!(rows.len(), FIXED_ROWS);
    assert!(rows.iter().all(|row| *row > 0));
    assert_eq!(
        fixed_observer_frame().unwrap(),
        rows.iter().sum::<usize>()
            + candidate_content_frame_v1().unwrap()
            + 2 * genuine_cache_raw_frame_v1().unwrap()
    );
    assert!(fixed_observer_work().unwrap() > fixed_observer_frame().unwrap());
    assert!(call_frame::<()>(usize::MAX).is_err());
    let before = fixed_observer_frame().unwrap();
    start_frames();
    assert_eq!(fixed_observer_frame().unwrap(), before);
    assert_eq!(finish_optional_frames().unwrap(), None);
}
#[test]
fn genuine_fixed_observer_actual_admission_exact_and_one_short_precedes_callbacks() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let frame = fixed_observer_frame().unwrap();
    let work = fixed_observer_work().unwrap();
    // Component DATA debit controls. No authentic owner/source is fabricated.
    for (work_limit, storage_limit, failed_work, failed_storage) in [
        (work, frame, false, false),
        (work - 1, frame, true, false),
        (work, frame - 1, false, true),
    ] {
        let mut ledger = Work::new(work_limit);
        let mut budget = Budget::new(&mut ledger, storage_limit);
        let admitted = admit_fixed_observer(&mut budget);
        assert_eq!(budget.failed_work().is_some(), failed_work);
        assert_eq!(budget.failed_storage().is_some(), failed_storage);
        FACTORY_FRAMES.with(|cell| assert!(cell.get().is_none()));
        if failed_work || failed_storage {
            assert!(matches!(admitted, Err(QueryError::Resource(_))));
            assert_eq!(budget.storage(), 0);
            if failed_storage {
                assert_eq!(budget.work(), work);
            }
        } else {
            assert_eq!(admitted.unwrap(), frame);
            assert_eq!((budget.work(), budget.storage()), (work, frame));
            budget.release_storage(frame).unwrap();
            assert_eq!(budget.storage(), 0);
        }
    }
}
#[test]
fn genuine_fixed_observer_denial_keeps_first_error_and_allows_exact_custody_cleanup() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let frame = fixed_observer_frame().unwrap();
    let work = fixed_observer_work().unwrap();
    for storage_denial in [false, true] {
        let mut ledger = Work::new(work);
        let mut budget = Budget::new(&mut ledger, frame);
        let owned = admit_fixed_observer(&mut budget).unwrap();
        if storage_denial {
            assert!(budget.reserve_storage(1).is_err());
        } else {
            assert!(budget.charge_work(1).is_err());
        }
        let first = (budget.failed_work(), budget.failed_storage());
        let protected = budget.storage();
        assert_eq!(protected, owned);
        // Simulated inert payload, not a real owner: the actual lifetime order
        // is separately source-checked below and exercised by genuine modes.
        let payload = Box::new([1u8; 3]);
        assert_eq!(payload.len(), 3);
        drop(payload);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
        assert_eq!((budget.failed_work(), budget.failed_storage()), first);
        assert_eq!(first.0.is_some(), !storage_denial);
        assert_eq!(first.1.is_some(), storage_denial);
    }
}
#[test]
fn genuine_fixed_optional_recorder_rejects_missing_poison_and_caught_duplicate() {
    assert!(finish_optional_frames().is_err());
    start_frames();
    assert_eq!(finish_optional_frames().unwrap(), None);
    start_frames();
    record_factory_frames_for_test_v1([1, 2, 3, 4, 10]);
    assert_eq!(finish_optional_frames().unwrap(), Some([1, 2, 3, 4, 10]));
    start_frames();
    record_factory_frames_for_test_v1([1, 2, 3, 4, 10]);
    assert!(catch_unwind(|| record_factory_frames_for_test_v1([1, 2, 3, 4, 10])).is_err());
    assert!(finish_optional_frames().is_err());
    start_frames();
    assert!(catch_unwind(|| record_factory_frames_for_test_v1([1, 2, 3, 4, 11])).is_err());
    assert!(finish_optional_frames().is_err());
    FACTORY_FRAMES.with(|cell| assert!(cell.get().is_none()));
}
#[test]
fn genuine_fixed_content_comparison_rejects_missing_refusal_and_equal_count_mutation() {
    use crate::production_ranked_projection_v1::assertion_analyzer_resources_v1::original_fixed_constructor_retention::{
        CheckedContent, CHECKED_ROW_CAP, CHECKED_ENTRY_CAP, ContentRefusal,
    };
    assert!(!complete_content(&None, &None));
    let refused = Some(Err(ContentRefusal::CheckedRowsLimit));
    assert!(!complete_content(&refused, &refused));
    let mut expected = ProofContent {
        phase: 1,
        checked: CheckedContent {
            rows: 1,
            offsets: [0; CHECKED_ROW_CAP + 1],
            entries: [0; CHECKED_ENTRY_CAP],
            used: 1,
        },
        dominance: None,
        zero: None,
    };
    expected.checked.offsets[1] = 1;
    expected.checked.entries[0] = 7;
    let mut actual = expected;
    assert!(complete_content(&Some(Ok(expected)), &Some(Ok(actual))));
    actual.checked.entries[0] = 8;
    assert_eq!(actual.checked.used, expected.checked.used);
    assert!(!complete_content(&Some(Ok(expected)), &Some(Ok(actual))));
}
#[test]
fn genuine_fixed_backend_refusal_is_saved_before_exact_static_data_relay() {
    let mut saved = None;
    let relay = retain_backend_error(Error::Incomplete("same original refusal"), &mut saved);
    assert_eq!(
        genuine_error_data_v1(&relay),
        GenuineErrorDataV1::Incomplete("same original refusal")
    );
    assert_eq!(
        genuine_error_data_v1(saved.as_ref().unwrap()),
        genuine_error_data_v1(&relay)
    );
    drop(relay);
    drop(saved);
}
#[test]
fn genuine_fixed_same_box_component_catch_resume_does_not_replace_payload() {
    let payload: PanicPayload = Box::new([19u64, 23u64]);
    let original = payload.as_ref() as *const (dyn Any + Send) as *const () as usize;
    let caught = catch_unwind(AssertUnwindSafe(|| -> () { resume_unwind(payload) })).unwrap_err();
    assert_eq!(
        caught.as_ref() as *const (dyn Any + Send) as *const () as usize,
        original
    );
    let final_payload =
        catch_unwind(AssertUnwindSafe(|| -> () { resume_unwind(caught) })).unwrap_err();
    assert_eq!(
        final_payload.as_ref() as *const (dyn Any + Send) as *const () as usize,
        original
    );
    // Component identity control; authentic original/candidate mode receipts
    // remain distinct and are emitted only by the actual pipeline test hook.
}
#[test]
fn genuine_fixed_source_orders_same_budget_original_candidate_postflights_drop_refund() {
    let text: String = include_str!("bf16_nominal_root_fixed_prefix_comparison_v1_tests.rs")
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let start = text.find("fnrun_fixed(").unwrap();
    let end = text[start..].find("typeFixedRunArguments=").unwrap() + start;
    let body = &text[start..end];
    let admit = body.find("admit_fixed_observer(budget)?").unwrap();
    let original = body.find("with_original_genuine_prefix_v1(").unwrap();
    let candidate = body
        .find("with_actual_root_retired_fixed_proof_observation_v1(")
        .unwrap();
    let final_content = body
        .find("witness.original_final=Some(retained_content(")
        .unwrap();
    let protected = body.find("witness.protected_before_drop=custody;").unwrap();
    let drop_original = body.find("drop(original_slot);").unwrap();
    let drop_candidate = body.find("drop(pending);").unwrap();
    let drop_error = body.find("drop(observation_error);").unwrap();
    let refund = body.find("budget.release_storage(owned)?;").unwrap();
    assert!(admit < original && original < candidate && candidate < final_content);
    assert!(final_content < protected && protected < drop_original);
    assert!(drop_original < drop_candidate && drop_candidate < drop_error && drop_error < refund);
    assert!(body.contains("Prep::new(budget,&mutowned)"));
    assert!(
        body.contains(
            "with_nominal_recipe_resources_v1(facts,cfg.source_tables().rich(),&mutowned,"
        )
    );
    assert!(!body.contains("Budget::new(") && !body.contains("Work::new("));
    assert!(body.contains("budget.failed_work()!=witness.failed_work"));
    assert!(body.contains("budget.failed_storage()!=witness.failed_storage"));
}
fn project_borrowed_owned(error: Error) -> QueryError {
    project_borrowed(&error)
}

pub(crate) fn observe_actual_root_fixed_prefix_comparison_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Q<()> {
    let before_work = budget.work();
    for mode in [
        FixedMode::Compare,
        FixedMode::CallbackError,
        FixedMode::CallbackPanic,
        FixedMode::OriginalQueryPanic,
    ] {
        let mut witness = FixedWitness::default();
        let result = run_fixed(
            owner,
            source,
            inventory,
            actual_inputs,
            budget,
            mode,
            &mut witness,
        );
        let cache_rows = witness
            .original_content
            .before
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .map(|proof| {
                [
                    proof.dominance.as_ref().map(|cache| cache.len).unwrap_or(0),
                    proof.zero.as_ref().map(|cache| cache.len).unwrap_or(0),
                ]
            });
        eprintln!(
            "fe2o3-genuine-fixed-prefix-v1 mode={mode:?} progress={:?} rows={} fixed={} original_cache_rows={cache_rows:?} original_proof_work={} stop={:?} original_same_panic={} candidate_same_panic={} original_before_candidate={} coverage={} custody={} dropped_before_refund={} floor_restored={} skipped={} frames={:?} before={:?} protected={:?} after={:?} owned={} failed_work={:?} failed_storage={:?}",
            witness.progress,
            witness.expected.len,
            witness.expected.fixed,
            witness.original.logical,
            witness.expected.stop,
            witness.original_same_panic,
            witness.candidate_same_panic,
            witness.original_retired_before_candidate,
            witness.coverage_complete,
            witness.protected_before_drop,
            witness.dropped_before_refund,
            witness.floor_restored,
            witness.mode_skipped,
            witness.frames,
            witness.before,
            witness.protected,
            witness.after,
            witness.owned,
            witness.failed_work,
            witness.failed_storage
        );
        for row in &witness.expected.rows[..witness.expected.len] {
            eprintln!(
                "fe2o3-genuine-fixed-row-v1 block={} source_address={} success={:?} expected={:?}",
                row.block, row.source_address, row.success, row.event
            );
        }
        // No owner was constructed when the new outer prefix itself refused.
        // Preserve its original work/storage error rather than inventing custody.
        if !witness.dropped_before_refund && witness.owned == 0 {
            result?;
            return Err(QueryError::Resource(Resource::Accounting));
        }
        if !witness.protected_before_drop
            || !witness.dropped_before_refund
            || !witness.floor_restored
        {
            return Err(QueryError::Resource(Resource::Accounting));
        }
        match mode {
            FixedMode::Compare => {
                result?;
            }
            FixedMode::CallbackError if witness.compared.complete && witness.coverage_complete => {
                assert!(matches!(result, Err(QueryError::Unavailable(_))));
                assert_eq!(
                    witness.backend_error,
                    Some(GenuineErrorDataV1::Incomplete(
                        "genuine fixed-prefix callback refusal"
                    ))
                );
            }
            FixedMode::CallbackPanic if witness.compared.complete && witness.coverage_complete => {
                assert!(witness.candidate_same_panic);
                assert!(matches!(result, Err(QueryError::CallbackPanicked)));
            }
            FixedMode::OriginalQueryPanic if witness.mode_skipped => {
                assert_eq!(witness.expected.fixed, 0);
                assert!(witness.frames.is_none() && witness.candidate_live.is_none());
                result?;
            }
            FixedMode::OriginalQueryPanic
                if witness.original_same_panic && witness.coverage_complete =>
            {
                assert!(
                    witness.original.installed
                        && witness.frames.is_none()
                        && witness.candidate_live.is_none()
                );
                assert!(matches!(result, Err(QueryError::CallbackPanicked)));
            }
            _ => {
                result?;
                return Err(QueryError::Unavailable(
                    "requested genuine chronology mode was not reached",
                ));
            }
        }
    }
    let after_work = budget.work();
    let total_work = after_work
        .checked_sub(before_work)
        .ok_or(QueryError::Resource(Resource::Accounting))?;
    eprintln!(
        "fe2o3-genuine-fixed-prefix-total-v1 before_work={before_work} after_work={after_work} total_work={total_work} modes=4 ordinary_route=false f2=false"
    );
    Ok(())
}
