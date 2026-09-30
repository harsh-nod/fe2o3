//! Genuine checked-source observation of the private pre-writer component.
//! Actual Rust-source evidence exists only when the separately gated source ladder calls this.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_dense_v1::CompletedNominalFifoDriverV1;
use crate::production_ranked_projection_v1::canonical_assertion_facts_v1::with_checked_nominal_facts_observation_v1;
use fe2o3_lower_mir_kernel::CheckedBf16CallInstanceV1;
use std::mem::size_of;
#[path = "whole_root_earlier_oracle_v1_tests.rs"]
mod earlier_original;
#[path = "whole_root_capability_oracle_v1_tests.rs"]
mod original;
#[path = "whole_root_shared_query_oracle_v1_tests.rs"]
mod shared_original;
use earlier_original::Observation;
use original::OriginalCapabilityOracleV1;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrefixPhase {
    Fresh,
    Terminal,
    Complete,
}
#[derive(Clone, Copy, Debug)]
struct PrefixObservation {
    constants: usize,
    nonempty_constants: usize,
    blocks: usize,
    layouts: usize,
    bound_reads: usize,
    first_visits: usize,
    repeated_visits: usize,
    original_work: usize,
    query_visits: [usize; 3],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Compare,
    CallbackError,
    CallbackPanic,
}
#[derive(Clone, Copy, Debug)]
struct ObservationWhole {
    prefix: PrefixObservation,
    shared_queries: usize,
    shared_accepted: usize,
    induction_certificates: usize,
    singleton_entries: usize,
    borrow_present: bool,
    runtime_arguments: usize,
    writer: Option<initial_strided_reads::genuine_empty::EmptyReadObservation>,
}
struct Originals<'s> {
    capability: OriginalCapabilityOracleV1,
    induction: Option<SemanticU32InductionNoOverflowReportV1>,
    earlier: Option<Observation>,
    views: Option<slice_projection_v1::ProjectedViewsV1<'static>>,
    indices: Vec<Option<u32>>,
    slices: Vec<Option<u32>>,
    shared: Option<ProductionSemanticSharedReadsV1<'s>>,
    observation: Option<ObservationWhole>,
    empty_reads: initial_strided_reads::genuine_empty::EmptyReadOracle,
}
impl Originals<'_> {
    fn new() -> Self {
        Self {
            capability: OriginalCapabilityOracleV1::new(),
            induction: None,
            earlier: None,
            views: None,
            indices: Vec::new(),
            slices: Vec::new(),
            shared: None,
            observation: None,
            empty_reads: initial_strided_reads::genuine_empty::EmptyReadOracle::new(),
        }
    }
}
const CALLBACK_ERROR: &str = "whole-root pre-writer callback control";
fn original_induction_error(error: SemanticU32InductionMeteredErrorV1<Resource>) -> Backend {
    match error {
        SemanticU32InductionMeteredErrorV1::Analysis(e) => Backend::SemanticU32Induction(e),
        SemanticU32InductionMeteredErrorV1::Meter(e) => resource(e),
        SemanticU32InductionMeteredErrorV1::Allocation => resource(Resource::Allocation),
        SemanticU32InductionMeteredErrorV1::Arithmetic => arithmetic(),
    }
}
#[allow(clippy::too_many_arguments)]
fn compare<'s>(
    pending: &PendingWholeRootBeforeArgumentWritersV1<'s>,
    expected: &mut Originals<'s>,
    owner: &'s ProductionPreRankedKirOwnerV1,
    checked: &CheckedBf16NominalCallV1<'_>,
    actual: &'s ActualRetainedRankedInputsV1<'_>,
    facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
    owned: &mut usize,
) -> BResult<()> {
    let id = checked.emission().root();
    pending.completed_for(owner, id, facts, owned)?;
    let entry = pending.entry.ok_or_else(accounting)?;
    let semantic = owner.semantic_ssa().source_semantic();
    let function = &semantic.functions()[id.index() as usize];
    let source = Source {
        function,
        types: semantic.types(),
        callables: semantic.callables(),
        ledger: (entry.budget_slot, entry.work_ledger),
    };
    // Independently invoke unchanged originals. All completed owning results are
    // attached outside the checked-facts callback and survive its postflight.
    with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
        let view = pending.earlier.view(&source, resources)?;
        expected.earlier = Some(earlier_original::compare(view, resources)?);
        let constants = pending.constants.completed_for(function, resources)?;
        expected.capability.prepare(
            function,
            semantic.callables(),
            semantic.types(),
            checked,
            owner,
            actual,
            &pending.prefix,
            constants,
            resources,
        )?;
        expected.induction = Some(
            fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_with_meter_v1(
                semantic,
                id,
                SemanticU32InductionAnalysisLimitsV1::default(),
                &mut InductionMeter(resources),
            )
            .map_err(original_induction_error)?,
        );
        let induction = pending
            .induction
            .completed_for(semantic, id)
            .map_err(|_| induction_error(&pending.induction))?;
        let expected_induction = expected.induction.as_ref().ok_or_else(accounting)?;
        resources.work(
            expected_induction
                .certificates()
                .len()
                .checked_mul(32)
                .and_then(|n| n.checked_add(32))
                .ok_or_else(arithmetic)?,
        )?;
        if expected_induction != induction {
            return Err(accounting());
        }
        let count = function.locals().len();
        // Exact old final constructors, independently evaluated after admission.
        // Any observed excess is charged only after attaching the actual owner.
        resources.work(count.checked_mul(3).ok_or_else(arithmetic)?)?;
        resources.reserve_storage(
            count
                .checked_mul(size_of::<Option<u32>>())
                .ok_or_else(arithmetic)?,
        )?;
        expected.indices = vec![None; count];
        resources.reserve_storage(
            expected
                .indices
                .capacity()
                .checked_sub(count)
                .ok_or_else(accounting)?
                .checked_mul(size_of::<Option<u32>>())
                .ok_or_else(arithmetic)?,
        )?;
        resources.reserve_storage(
            count
                .checked_mul(size_of::<Option<u32>>())
                .ok_or_else(arithmetic)?,
        )?;
        expected.slices = vec![None; count];
        resources.reserve_storage(
            expected
                .slices
                .capacity()
                .checked_sub(count)
                .ok_or_else(accounting)?
                .checked_mul(size_of::<Option<u32>>())
                .ok_or_else(arithmetic)?,
        )?;
        resources.reserve_storage(
            count
                .checked_mul(size_of::<Option<ProjectedViewV1>>())
                .ok_or_else(arithmetic)?,
        )?;
        expected.views = Some(slice_projection_v1::ProjectedViewsV1::new(count, None));
        pending.views.compare_original_initial_v1(
            expected.views.as_ref().ok_or_else(accounting)?,
            resources,
        )?;
        if pending.arguments.runtime_index_arguments != expected.indices
            || pending.arguments.runtime_slice_extent_arguments != expected.slices
            || pending.arguments.edge_count != 0
            || !pending.arguments.borrowed_locals.is_empty()
            || pending.arguments.next_runtime_argument != 1
        {
            return Err(accounting());
        }
        Ok(())
    })?;
    let (constants, preparation, singletons, borrows) =
        with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
            Ok((
                pending.constants.completed_for(function, resources)?,
                pending.earlier.view(&source, resources)?,
                pending
                    .singleton
                    .completed_for(semantic.types(), function, resources)?,
                pending.borrows.completed_for(
                    semantic.types(),
                    function,
                    semantic.target(),
                    resources,
                )?,
            ))
        })?;
    let inputs = NominalCapabilityInputsV1::from_borrowed_source_v1(
        function,
        preparation.enum_dominance(),
        preparation.allocation_contracts(),
        constants,
    );
    let prefix = with_nominal_capability_consumer_v1(facts, |site, consumer| {
        let actual = pending.driver.completed_for(site, &inputs, consumer)?;
        expected
            .capability
            .compare(function, actual, pending.query_visits, consumer)
    })
    .map_err(nominal_error)?;
    scalar_singleton_projection_v1::with_scalar_private_singletons_v1(
        semantic.types(),
        function,
        facts,
        |original, facts| {
            facts
                .charge_private_array_work(original.len().checked_add(1).ok_or_else(arithmetic)?)?;
            if original != singletons {
                return Err(accounting());
            }
            Ok(())
        },
    )?;
    scalar_borrow_projection_v1::with_scalar_private_borrows_v1(
        semantic.types(),
        function,
        semantic.target(),
        facts,
        |original, facts| {
            scalar_borrow_projection_v1::compare_retained_original_v1(original, borrows, facts)
        },
    )?;
    // Public contains is the observable oracle. Private rows, duplicate counts
    // and ordering are NOT inspected or claimed equal by this whole-root control.
    // The unchanged old view must be released inside its original canonical
    // lifetime (before that scope refunds its report); only the candidate keeps
    // its retained rows across postflight.
    expected.shared = Some(facts.shared_value_reads_v1(owner.semantic_ssa(), id)?);
    let query_result = shared_original::compare(
        owner,
        id,
        function,
        expected.shared.as_ref().ok_or_else(accounting)?,
        pending.shared.view().ok_or_else(accounting)?,
        facts,
        owned,
    );
    let old = expected.shared.take().ok_or_else(accounting)?;
    let cleanup = facts.release_shared_value_reads_v1(old);
    let (shared_queries, shared_accepted) = query_result?;
    cleanup?;
    if shared_queries == 0 {
        return Err(accounting());
    }
    expected.observation = Some(ObservationWhole {
        prefix,
        shared_queries,
        shared_accepted,
        induction_certificates: expected
            .induction
            .as_ref()
            .ok_or_else(accounting)?
            .certificates()
            .len(),
        singleton_entries: singletons.len(),
        borrow_present: borrows.is_some(),
        runtime_arguments: function.locals().len(),
        writer: None,
    });
    pending.completed_for(owner, id, facts, owned)?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn run(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    actual: &ActualRetainedRankedInputsV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
    mode: Mode,
    writers: bool,
) -> Result<ObservationWhole> {
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        let before = Custody::take(budget)?;
        let mut owned = 0usize;
        Prep::new(budget, &mut owned)
            .reserve_storage(frame()?)
            .map_err(query_error)?;
        let mut pending = PendingWholeRootBeforeArgumentWritersV1::new();
        let mut expected = Originals::new();
        let result = with_checked_nominal_facts_observation_v1(
            owner,
            inventory,
            source.root(),
            source.root(),
            source.call_block(),
            source.source_call(),
            budget,
            &mut owned,
            |checked, facts, owned| {
                if !checked.belongs_to(inventory)
                    || !std::ptr::eq(checked.source_call(), source.source_call())
                    || !actual.belongs_to(owner)
                {
                    return Err(QueryError::Unavailable("whole-root checked source differs"));
                }
                pending.prepare_into(owner, checked, actual, facts, owned)?;
                if let Err(error) = compare(
                    &pending,
                    &mut expected,
                    owner,
                    checked,
                    actual,
                    facts,
                    owned,
                ) {
                    let mapped = saved_query_error(&error);
                    pending.failure = Some(error);
                    return Err(mapped);
                }
                // A second entry must refuse without consuming or modifying any
                // owned result, original ledger work, storage or counter.
                let held = facts
                    .retained_whole_root_snapshot_v1(owner, source.root(), owned, None)
                    .map_err(|e| saved_query_error(&e))?;
                let refused = pending.prepare_into(owner, checked, actual, facts, owned);
                let after = facts
                    .retained_whole_root_snapshot_v1(owner, source.root(), owned, Some(held))
                    .map_err(|e| saved_query_error(&e))?;
                if refused != Err(QueryError::Resource(Resource::Accounting))
                    || after.work != held.work
                    || after.storage != held.storage
                    || after.owned != held.owned
                    || pending.phase != WholePhase::BeforeArgumentWriters
                {
                    return Err(QueryError::Unavailable(
                        "whole-root one-shot continuation differs",
                    ));
                }
                if writers {
                    let writer = initial_strided_reads::genuine_empty::observe_in_scope(
                        &mut pending,
                        &mut expected.empty_reads,
                        &mut expected.indices,
                        owner,
                        source.root(),
                        facts,
                        owned,
                    )
                    .map_err(|error| {
                        let mapped = saved_query_error(&error);
                        pending.failure = Some(error);
                        mapped
                    })?;
                    expected
                        .observation
                        .as_mut()
                        .ok_or(QueryError::Unavailable(
                            "before-writer comparison observation absent",
                        ))?
                        .writer = Some(writer);
                }
                match mode {
                    Mode::Compare => Ok(()),
                    Mode::CallbackError => Err(QueryError::Unavailable(CALLBACK_ERROR)),
                    Mode::CallbackPanic => std::panic::panic_any(()),
                }
            },
        );
        let custody = before.check(budget, owned);
        let phase = pending.phase;
        let observation = expected.observation;
        let retained = owned;
        let denied = budget.failed_work().is_some() || budget.failed_storage().is_some();
        // Actual checked/canonical postflight is over. Both candidate owners and
        // completed expected reports remain physically present until this point.
        drop(expected);
        drop(pending);
        custody?;
        budget.release_storage(retained)?;
        if denied {
            return Err(Resource::Accounting.into());
        }
        match mode {
            Mode::Compare => result?,
            Mode::CallbackError if result == Err(QueryError::Unavailable(CALLBACK_ERROR)) => {}
            Mode::CallbackPanic if result == Err(QueryError::CallbackPanicked) => {}
            _ => {
                return Err(QueryError::Unavailable(
                    "whole-root callback mapping differs",
                ));
            }
        }
        let expected_phase = if writers {
            WholePhase::InitialStridedReadsTerminal
        } else {
            WholePhase::BeforeArgumentWriters
        };
        if phase != expected_phase {
            return Err(Resource::Accounting.into());
        }
        observation.ok_or(QueryError::Unavailable(
            "whole-root original comparisons absent",
        ))
    })
}
fn frame() -> Result<usize> {
    let rows=[
        size_of::<(PendingWholeRootBeforeArgumentWritersV1<'static>,Originals<'static>,
            OriginalCapabilityOracleV1,Option<Observation>,Option<ObservationWhole>,ObservationWhole,
            PrefixPhase,PrefixObservation,Mode,[Mode;3],std::array::IntoIter<Mode,3>)>(),
        size_of::<(&ProductionPreRankedKirOwnerV1,&CheckedBf16CallInstanceV1<'static>,
            &ActualRetainedRankedInputsV1<'static>,&CanonicalKirInventoryV1<'static>,&mut Budget<'static>,
            Custody,usize,&mut usize,Result<()>,Result<ObservationWhole>)>(),
        size_of::<(&mut PendingWholeRootBeforeArgumentWritersV1<'static>,&mut Originals<'static>,
            &CheckedBf16NominalCallV1<'static>,&mut CanonicalSourceAssertionFactsV1<'static,'static,'static,'static,'static>,
            Source<'static>,Ledger,Snapshot,BResult<Snapshot>,&AdmittedInertSemanticMirV1,
            &SemanticFunctionDeclV1,SemanticFunctionIdV1,usize,u32,bool,Option<usize>)>(),
        size_of::<(Option<SemanticU32InductionNoOverflowReportV1>,SemanticU32InductionNoOverflowReportV1,
            &SemanticU32InductionNoOverflowReportV1,
            std::result::Result<SemanticU32InductionNoOverflowReportV1,SemanticU32InductionMeteredErrorV1<Resource>>,
            SemanticU32InductionMeteredErrorV1<Resource>,SemanticU32InductionAnalysisLimitsV1,
            InductionMeter<'static,'static,'static>,Option<ProductionSemanticSharedReadsV1<'static>>,
            ProductionSemanticSharedReadsV1<'static>,BResult<ProductionSemanticSharedReadsV1<'static>>) >(),
        size_of::<(Option<slice_projection_v1::ProjectedViewsV1<'static>>,slice_projection_v1::ProjectedViewsV1<'static>,
            Vec<Option<u32>>,Vec<Option<u32>>,&Vec<Option<u32>>,usize,Option<u32>,
            &mut Prep<'static,'static>,&mut OriginalCapabilityOracleV1,&RootEntryPrefixV1,
            &slice_projection_v1::ProjectedViewsV1<'static>,Option<&slice_projection_v1::ProjectedViewsV1<'static>>,
            BResult<&slice_projection_v1::ProjectedViewsV1<'static>>,
            Option<&SemanticU32InductionNoOverflowReportV1>,BResult<&SemanticU32InductionNoOverflowReportV1>,Option<&mut dyn ProjectedAssertionFactsV1>) >(),
        size_of::<(BeforeCapabilitiesV1<'static>,&[Option<u64>],&[u8],
            Option<&scalar_borrow_projection_v1::ScalarPrivateBorrowsV1<'static>>,
            BResult<(&[Option<u64>],BeforeCapabilitiesV1<'static>,&[u8],
                Option<&scalar_borrow_projection_v1::ScalarPrivateBorrowsV1<'static>>)>,
            NominalCapabilityInputsV1<'static>,CompletedNominalFifoDriverV1<'static>,
            Result<CompletedNominalFifoDriverV1<'static>>,Result<PrefixObservation>)>(),
        size_of::<(&mut dyn NominalCapabilityConsumerV1,
            &crate::production_ranked_projection_v1::bf16_nominal_capabilities_v1::NominalCallerSiteV1<'static>,
            &NominalCapabilityInputsV1<'static>,&RetainedNominalCapabilityDriverV1,
            &OriginalCapabilityOracleV1,[usize;3],(usize,usize),BResult<(usize,usize)>,
            Option<&ProductionSemanticSharedReadsV1<'static>>,
            BResult<&ProductionSemanticSharedReadsV1<'static>>,BResult<()>,Backend,QueryError,Resource,
            CanonicalAssertionErrorV1,Option<Backend>)>(),
        size_of::<(&mut dyn FnMut(&[u8],&mut CanonicalSourceAssertionFactsV1<'static,'static,'static,'static,'static>)->BResult<()>,
            &mut dyn FnMut(&mut Prep<'static,'static>)->BResult<()>,&mut dyn FnMut()->Result<()>,
            std::result::Result<Result<()>,PanicPayload>,PanicPayload,WholePhase)>(),
        size_of::<([usize;13],std::array::IntoIter<usize,13>,usize,usize,Option<usize>,Result<usize>,bool)>(),
        initial_strided_reads::genuine_empty::frame().map_err(query_error)?,
        shared_original::frame(),
        scalar_borrow_projection_v1::comparison_frame_v1(),
        allocation_frame::<(),()>().map_err(query_error)?,
    ];
    rows.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row)
            .ok_or(QueryError::Resource(Resource::Arithmetic))
    })
}
pub(in crate::production_ranked_projection_v1) fn observe(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    actual: &ActualRetainedRankedInputsV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    owner.with_bf16_nominal_entry_resources_v1(inventory,budget,|budget|{
        let before=Custody::take(budget)?;
        let bytes=frame()?;
        budget.reserve_storage(bytes)?;
        let outcome=catch_unwind(AssertUnwindSafe(||->Result<()>{
            for mode in [Mode::Compare,Mode::CallbackError,Mode::CallbackPanic] {
                let floor=budget.storage();
                let observed=run(owner,source,actual,inventory,budget,mode,false)?;
                if budget.storage()!=floor{return Err(Resource::Accounting.into());}
                eprintln!("fe2o3-whole-root-before-writers-v1 mode={mode:?} shared_queries={} shared_accepted={} singleton_entries={} borrow_present={} induction_certificates={} constants={} nonempty_constants={} blocks={} layouts={} bound_reads={} first_visits={} repeated_visits={} original_work={} capability_queries={:?} runtime_arguments={} retained_postflight=true original_observable_data=true private_shared_rows_compared=false writers_started=false ordinary_route=false",
                    observed.shared_queries,observed.shared_accepted,observed.singleton_entries,observed.borrow_present,
                    observed.induction_certificates,observed.prefix.constants,observed.prefix.nonempty_constants,
                    observed.prefix.blocks,observed.prefix.layouts,observed.prefix.bound_reads,
                    observed.prefix.first_visits,observed.prefix.repeated_visits,observed.prefix.original_work,
                    observed.prefix.query_visits,observed.runtime_arguments);
            }
            Ok(())
        }));
        let result=match outcome{Ok(result)=>result,Err(payload)=>{drop(payload);Err(QueryError::CallbackPanicked)}};
        before.check(budget,bytes)?;
        budget.release_storage(bytes)?;
        result
    })
}

pub(in crate::production_ranked_projection_v1) fn observe_initial_reads(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    actual: &ActualRetainedRankedInputsV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        let before = Custody::take(budget)?;
        let bytes = frame()?;
        budget.reserve_storage(bytes)?;
        let outcome = catch_unwind(AssertUnwindSafe(|| -> Result<()> {
            for mode in [Mode::Compare, Mode::CallbackError, Mode::CallbackPanic] {
                let floor = budget.storage();
                let observed = run(owner, source, actual, inventory, budget, mode, true)?;
                if budget.storage() != floor { return Err(Resource::Accounting.into()); }
                let writer = observed.writer.ok_or(QueryError::Unavailable(
                    "genuine empty-read writer observation absent"))?;
                eprintln!("fe2o3-whole-root-initial-empty-reads-v1 mode={mode:?} blocks={} read_views={} projected_rows={} prefix_operations={} next_value={} next_argument={} lookup_visits={} retained_postflight=true original_empty_donor=true before_writer_comparison=true writers_started=true empty_view_profile=true nonempty_view_coverage=false private_component=true ordinary_route=false old_completion_refused=true foreign_counter_refused=true reentry_terminal=true",
                    writer.blocks, writer.read_views, writer.projected_rows,
                    writer.prefix_operations, writer.next_value, writer.next_argument,
                    writer.lookup_visits);
            }
            Ok(())
        }));
        let result = match outcome {
            Ok(result) => result,
            Err(payload) => { drop(payload); Err(QueryError::CallbackPanicked) }
        };
        before.check(budget, bytes)?;
        budget.release_storage(bytes)?;
        result
    })
}

#[path = "whole_root_initial_graph_genuine_v1_tests.rs"]
mod graph_observer;
pub(in crate::production_ranked_projection_v1) use graph_observer::observe as observe_initial_graph;
