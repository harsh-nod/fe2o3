//! Genuine source observation of two separate lexical component factories.
//! Twelve fresh owners; never a completed combined block stream or admission.
use super::*;
use crate::production_ranked_projection_v1::{
    local_use_frames as frames, local_use_source_oracle as independent,
    root_local_contracts_v1::LocalContractDecisionV1,
};
use frames::{Account, Outcome, WitnessSummary};
use independent::{Coordinate, OriginDecision, ScalarOracle};

const A_ERROR: &str = "actual local contract observation callback refusal";
const A_PANIC: &str = "actual local contract observation callback panic";
const B_ERROR: &str = "actual source use observation callback refusal";
const B_PANIC: &str = "actual source use observation callback panic";
const REGION_REFUSAL: &str =
    "a checked reference is dereferenced outside its authenticated payload region";
const CALL_REFUSAL: &str =
    "actual source use at a call or drop requires full access/effect routing";

struct Slot {
    pending: PendingActualRootPrefixIndicesV1,
    origins: OriginOracle,
    scalar: ScalarOracle,
}
impl Slot {
    fn new() -> Self {
        Self {
            pending: PendingActualRootPrefixIndicesV1::new(),
            origins: OriginOracle::default(),
            scalar: ScalarOracle::default(),
        }
    }
}
#[derive(Clone, Copy)]
enum Mode {
    All,
    OccupiedA,
    ErrorA,
    PanicA,
    ForeignQuery,
    Some,
    None,
    OccupiedB,
    ErrorB,
    PanicB,
    ForeignPending,
    Call,
}
const MODES: [Mode; 12] = [
    Mode::All,
    Mode::OccupiedA,
    Mode::ErrorA,
    Mode::PanicA,
    Mode::ForeignQuery,
    Mode::Some,
    Mode::None,
    Mode::OccupiedB,
    Mode::ErrorB,
    Mode::PanicB,
    Mode::ForeignPending,
    Mode::Call,
];

fn account(b: &Budget<'_>) -> Account {
    Account {
        work: b.work(),
        storage: b.storage(),
        peak: b.peak_storage(),
    }
}
fn context_account(c: &NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_>) -> Account {
    account(c.facts.budget)
}
fn coord(c: Coordinate) -> frames::Coordinate {
    frames::Coordinate {
        block: c.block,
        statement: c.statement,
        ordinal: c.ordinal,
    }
}
fn oracle_coord(c: frames::Coordinate) -> Coordinate {
    Coordinate {
        block: c.block,
        statement: c.statement,
        ordinal: c.ordinal,
    }
}
fn site(c: Coordinate) -> ProjectedSemanticAccessSiteV1 {
    ProjectedSemanticAccessSiteV1 {
        block: c.block,
        statement: c.statement,
    }
}
fn add(n: &mut usize) -> Result<()> {
    *n = n
        .checked_add(1)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    Ok(())
}

/// Streaming first pass. No production local query or use selector is called.
/// The independent origin interpreter has already filled the expected table.
fn discover(
    rich: &RichNominalSourceTablesV1<'_>,
    origins: &OriginOracle,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<WitnessSummary> {
    let mut occurrences = 0;
    let mut expected_some = 0;
    let mut expected_none = 0;
    let mut expected_refused = 0;
    let mut call_boundaries = 0;
    let mut some = None;
    let mut none = None;
    let mut call = None;
    independent::walk_occurrences(
        rich.function(),
        resources,
        &mut |o, r| {
            let expected = independent::expected_origin(
                o.place,
                o.coordinate.block,
                &origins.origins,
                rich.option_dominance(),
                rich.enum_payload_dominance(),
                r,
            )?;
            add(&mut occurrences)?;
            match expected {
                OriginDecision::Allowed(Some(origin)) => {
                    add(&mut expected_some)?;
                    if some.is_none() && independent::eligible_some(origin, &o) {
                        some = Some(coord(o.coordinate));
                    }
                }
                OriginDecision::Allowed(None) => {
                    add(&mut expected_none)?;
                    if none.is_none() {
                        none = Some(coord(o.coordinate));
                    }
                }
                OriginDecision::RegionRefused => add(&mut expected_refused)?,
            }
            Ok(())
        },
        &mut |coordinate, _| {
            add(&mut call_boundaries)?;
            if call.is_none() {
                call = Some(coord(coordinate));
            }
            Ok(())
        },
    )?;
    Ok(WitnessSummary {
        locals: rich.function().locals().len(),
        blocks: rich.function().blocks().len(),
        occurrences,
        expected_some,
        expected_none,
        expected_refused,
        call_boundaries,
        some: some.ok_or(Error::Incomplete(
            "actual source lacks an eligible Some occurrence",
        ))?,
        none: none.ok_or(Error::Incomplete("actual source lacks a None occurrence"))?,
        call: call.ok_or(Error::Incomplete(
            "actual source lacks a call/drop refusal boundary",
        ))?,
    })
}
fn same_witnesses(left: WitnessSummary, right: WitnessSummary) -> Result<()> {
    if left != right {
        return Err(Error::Incomplete(
            "independent actual witness roster changed",
        ));
    }
    Ok(())
}
fn expect_local(
    got: Result<LocalContractDecisionV1>,
    expected: OriginDecision,
    immutable: bool,
    allocation: Option<AllocationContractV1>,
    provenance: Option<LocalAllocationProvenanceV1>,
) -> Result<()> {
    match (got, expected) {
        (Err(Error::Unsupported(REGION_REFUSAL)), OriginDecision::RegionRefused) => Ok(()),
        (Ok(actual), OriginDecision::Allowed(origin))
            if actual.origin == origin
                && actual.immutable == immutable
                && actual.allocation == allocation
                && actual.allocation_provenance == provenance =>
        {
            Ok(())
        }
        _ => Err(Error::Incomplete(
            "actual borrowed local decision differs from independent oracle",
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn inspect_local(
    actual: &ActualRootLocalContractsV1<'_>,
    rich: &RichNominalSourceTablesV1<'_>,
    checked: &CheckedBf16NominalCallV1<'_>,
    context: &mut NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_>,
    origins: &mut OriginOracle,
    scalar: &mut ScalarOracle,
    input_count: usize,
    foreign_query: bool,
) -> Result<WitnessSummary> {
    inspect_origins(
        actual.origins(),
        rich,
        checked,
        context,
        origins,
        input_count,
    )?;
    context.with_resources(|r| {
        scalar.prepare(rich.function(), r)?;
        let n = rich.function().locals().len();
        r.work(n.checked_mul(8).and_then(|n| n.checked_add(64))
            .ok_or_else(|| resource(Resource::Arithmetic))?)?;
        if scalar.counts != rich.scalar_counts() || scalar.assignments != rich.scalar_assignments()
            || scalar.escaped != rich.address_escaped() {
            return Err(Error::Incomplete("independent actual scalar rows differ from rich rows"));
        }
        // COMPLETE first pass before any actual.query. Raw A answers cannot select witnesses.
        let witnesses = discover(rich, origins, r)?;
        independent::walk_occurrences(rich.function(), r, &mut |o, r| {
            let expected = independent::expected_origin(o.place, o.coordinate.block,
                &origins.origins, rich.option_dominance(), rich.enum_payload_dominance(), r)?;
            r.work(64)?;
            let local = o.place.local().index() as usize;
            let immutable = scalar.immutable(rich.function(), local)?;
            let allocation = rich.allocations().get(local).copied().flatten();
            let provenance = rich.allocation_provenance().get(local).copied().flatten();
            expect_local(actual.query(o.place, o.coordinate.block, r), expected,
                immutable, allocation, provenance)?;
            if foreign_query && o.coordinate == oracle_coord(witnesses.some) {
                // Fixed control ledger; never used for source preparation.
                let mut foreign_work = Work::new(0);
                let mut foreign = Budget::new(&mut foreign_work, 0);
                let mut owned = 0;
                let refused = actual.query(o.place, o.coordinate.block,
                    &mut PreparationResourcesV1::new(&mut foreign, &mut owned));
                if !matches!(refused, Err(Error::CanonicalAssertions(
                    crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Accounting)
                ))) || foreign.work() != 0 || foreign.storage() != 0 || owned != 0
                    || foreign.failed_work().is_some() || foreign.failed_storage().is_some()
                {
                    return Err(Error::Incomplete("foreign local query did not refuse before reads/work"));
                }
            }
            Ok(())
        }, &mut |_, _| Ok(()))?;
        Ok(witnesses)
    })
}

#[allow(clippy::too_many_arguments)]
fn inspect_use(
    actual: &ActualRootReferenceUseV1<'_>,
    wanted: Coordinate,
    rich: &RichNominalSourceTablesV1<'_>,
    checked: &CheckedBf16NominalCallV1<'_>,
    context: &mut NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_>,
    oracle: &mut OriginOracle,
    input_count: usize,
    expected_some: bool,
) -> Result<()> {
    inspect_origins(
        actual.origins(),
        rich,
        checked,
        context,
        oracle,
        input_count,
    )?;
    context.with_resources(|r| {
        let mut matched = false;
        independent::walk_occurrences(
            rich.function(),
            r,
            &mut |o, r| {
                if o.coordinate != wanted {
                    return Ok(());
                }
                r.work(128)?;
                if matched {
                    return Err(Error::Incomplete(
                        "independent occurrence coordinate repeated",
                    ));
                }
                matched = true;
                let expected = independent::expected_origin(
                    o.place,
                    wanted.block,
                    &oracle.origins,
                    rich.option_dominance(),
                    rich.enum_payload_dominance(),
                    r,
                )?;
                let OriginDecision::Allowed(origin) = expected else {
                    return Err(Error::Incomplete(
                        "selected actual use witness has refused region",
                    ));
                };
                if origin.is_some() != expected_some
                    || !std::ptr::eq(actual.place(), o.place)
                    || actual.site() != site(wanted)
                    || actual.ordinal() != wanted.ordinal
                    || actual.access() != o.access
                    || actual.origin() != origin
                    || actual.occurrence.atomic != o.atomic
                    || actual.occurrence.requirement != o.requirement
                    || actual.occurrence.source != o.source
                {
                    return Err(Error::Incomplete(
                        "actual source use differs from independent occurrence",
                    ));
                }
                Ok(())
            },
            &mut |_, _| Ok(()),
        )?;
        if !matched {
            return Err(Error::Incomplete(
                "actual use witness absent from independent rescan",
            ));
        }
        Ok(())
    })
}
fn expected_panic(payload: Box<dyn std::any::Any + Send>, expected: &'static str) -> Result<()> {
    let matches = payload
        .downcast_ref::<&'static str>()
        .is_some_and(|s| *s == expected);
    drop(payload);
    if matches {
        Ok(())
    } else {
        Err(Error::Incomplete(
            "unexpected genuine callback panic payload",
        ))
    }
}
fn occupied(result: Result<()>) -> Result<()> {
    if matches!(
        result,
        Err(Error::Incomplete(
            "actual root assembly cannot be replaced or retried"
        ))
    ) {
        Ok(())
    } else {
        Err(Error::Incomplete(
            "actual occupied component did not refuse",
        ))
    }
}

#[allow(clippy::too_many_arguments)]
fn one_run(
    index: usize,
    mode: Mode,
    slot: &mut Slot,
    witnesses: &mut Option<WitnessSummary>,
    checked: &CheckedBf16NominalCallV1<'_>,
    rich: &RichNominalSourceTablesV1<'_>,
    inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    context: &mut NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_>,
) -> Result<()> {
    // This guard lives OUTSIDE the expected callback unwind.
    let run = frames::begin(index, context_account(context));
    let mut entered = false;
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<()> {
        if index < 5 {
            context.with_actual_root_local_contracts_v1(
                checked,
                rich,
                inputs,
                &mut slot.pending,
                |actual, context| {
                    entered = true;
                    let summary = inspect_local(
                        &actual,
                        rich,
                        checked,
                        context,
                        &mut slot.origins,
                        &mut slot.scalar,
                        inputs.inputs().len(),
                        matches!(mode, Mode::ForeignQuery),
                    )?;
                    if let Some(expected) = *witnesses {
                        same_witnesses(summary, expected)?;
                    } else {
                        *witnesses = Some(summary);
                    }
                    match mode {
                        Mode::ErrorA => Err(Error::Incomplete(A_ERROR)),
                        Mode::PanicA => std::panic::panic_any(A_PANIC),
                        _ => Ok(()),
                    }
                },
            )?;
            if matches!(mode, Mode::OccupiedA) {
                occupied(context.with_actual_root_local_contracts_v1(
                    checked,
                    rich,
                    inputs,
                    &mut slot.pending,
                    |_, _| -> Result<()> { panic!("occupied A callback entered") },
                ))?;
            }
            Ok(())
        } else {
            let summary =
                (*witnesses).ok_or(Error::Incomplete("actual A witness pass did not finish"))?;
            let wanted = oracle_coord(match mode {
                Mode::None => summary.none,
                Mode::Call => summary.call,
                _ => summary.some,
            });
            context.with_actual_root_reference_use_v1(
                checked,
                rich,
                inputs,
                &mut slot.pending,
                site(wanted),
                wanted.ordinal,
                |actual, context| {
                    entered = true;
                    if matches!(mode, Mode::Call) {
                        return Err(Error::Incomplete("call/drop source use callback entered"));
                    }
                    inspect_use(
                        &actual,
                        wanted,
                        rich,
                        checked,
                        context,
                        &mut slot.origins,
                        inputs.inputs().len(),
                        !matches!(mode, Mode::None),
                    )?;
                    match mode {
                        Mode::ErrorB => Err(Error::Incomplete(B_ERROR)),
                        Mode::PanicB => std::panic::panic_any(B_PANIC),
                        _ => Ok(()),
                    }
                },
            )?;
            if matches!(mode, Mode::OccupiedB) {
                occupied(context.with_actual_root_reference_use_v1(
                    checked,
                    rich,
                    inputs,
                    &mut slot.pending,
                    site(wanted),
                    wanted.ordinal,
                    |_, _| -> Result<()> { panic!("occupied B callback entered") },
                ))?;
            }
            if matches!(mode, Mode::ForeignPending) {
                let mut foreign_work = Work::new(0);
                let foreign = Budget::new(&mut foreign_work, 0);
                let (saved_slot, saved_ledger) = slot
                    .pending
                    .ledger
                    .ok_or_else(|| resource(Resource::Accounting))?;
                if saved_ledger == foreign.work_ledger_identity_v1() {
                    return Err(resource(Resource::Accounting));
                }
                slot.pending.ledger = Some((saved_slot, foreign.work_ledger_identity_v1()));
                let refused = context.with_actual_root_reference_use_v1(
                    checked,
                    rich,
                    inputs,
                    &mut slot.pending,
                    site(wanted),
                    wanted.ordinal,
                    |_, _| -> Result<()> { panic!("foreign B callback entered") },
                );
                if !matches!(refused, Err(Error::CanonicalAssertions(
                    crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Accounting)
                ))) { return Err(Error::Incomplete("actual foreign pending ledger did not refuse")); }
            }
            Ok(())
        }
    }));
    // Original state/ledger/floor/denial checks after expected error or unwind.
    // No charge/refund or replacement resource context is hidden here.
    context.with_resources(|_| Ok(()))?;
    let outcome = match mode {
        Mode::ErrorA => {
            if !matches!(result, Ok(Err(Error::Incomplete(A_ERROR)))) {
                return Err(Error::Incomplete("A callback error attribution differs"));
            }
            Outcome::CallbackError
        }
        Mode::ErrorB => {
            if !matches!(result, Ok(Err(Error::Incomplete(B_ERROR)))) {
                return Err(Error::Incomplete("B callback error attribution differs"));
            }
            Outcome::CallbackError
        }
        Mode::PanicA | Mode::PanicB => {
            let Err(payload) = result else {
                return Err(Error::Incomplete("expected component panic absent"));
            };
            expected_panic(
                payload,
                if matches!(mode, Mode::PanicA) {
                    A_PANIC
                } else {
                    B_PANIC
                },
            )?;
            Outcome::CallbackPanic
        }
        Mode::Call => {
            if !matches!(result, Ok(Err(Error::Incomplete(CALL_REFUSAL)))) || entered {
                return Err(Error::Incomplete(
                    "existing source call/drop refusal changed",
                ));
            }
            Outcome::CallRefused
        }
        _ => {
            match result {
                Ok(value) => value?,
                Err(payload) => {
                    drop(payload);
                    return Err(Error::Incomplete("unexpected actual component panic"));
                }
            }
            match mode {
                Mode::OccupiedA | Mode::OccupiedB => Outcome::OccupiedRefused,
                Mode::ForeignQuery => Outcome::ForeignQueryRefused,
                Mode::ForeignPending => Outcome::ForeignPendingRefused,
                _ => Outcome::Observed,
            }
        }
    };
    if entered != !matches!(mode, Mode::Call)
        || !slot.pending.completed
        || !slot.pending.guarded.completed()
        || !slot.pending.origins.completed()
        || slot.pending.guarded.frame_credits == 0
        || slot.pending.origins.frame_credits == 0
    {
        return Err(Error::Incomplete(
            "actual component completion/entry state differs",
        ));
    }
    run.finish(
        context_account(context),
        outcome,
        entered,
        slot.pending.guarded.frame_credits,
        slot.pending.origins.frame_credits,
    );
    Ok(())
}

/// Typed, source-owned reference capture set used by the entry/envelope closures.
type SourceCaptures = (
    &'static ProductionPreRankedKirOwnerV1,
    &'static CheckedBf16CallInstanceV1<'static>,
    &'static CanonicalKirInventoryV1<'static>,
    &'static crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
);
type EnvelopeReferences = (
    &'static mut Budget<'static>,
    &'static mut usize,
    &'static mut [Slot; 12],
    &'static mut Option<WitnessSummary>,
);

/// Source-derived logical transfer reservation, not an optimized stack/RSS bound.
/// F is the ACTUAL source-capturing envelope callback, not a guessed residual.
/// Inner rich/context/A/B/graph factories additionally charge their own actual F.
fn headers<F>() -> Q<usize> {
    let mut n = frames::diagnostic_storage_bound();
    for amount in [
        size_of::<[Slot; 12]>().checked_mul(4),
        size_of::<SourceCaptures>().checked_mul(8),
        size_of::<(F, EnvelopeReferences)>().checked_mul(4),
        size_of::<WitnessSummary>().checked_mul(8),
        size_of::<Option<WitnessSummary>>().checked_mul(4),
        size_of::<Account>().checked_mul(16),
        size_of::<Budget<'static>>().checked_mul(4),
        size_of::<Work>().checked_mul(4),
        size_of::<Q<()>>().checked_mul(4),
        size_of::<Result<()>>().checked_mul(4),
        size_of::<std::thread::Result<Q<()>>>().checked_mul(4),
        size_of::<std::thread::Result<Result<()>>>().checked_mul(4),
        size_of::<Box<dyn std::any::Any + Send>>().checked_mul(4),
        // The only deliberately injected payload is a boxed &'static str.
        size_of::<&'static str>().checked_mul(4),
        size_of::<Option<frames::EnvelopeGuard>>().checked_mul(2),
        size_of::<frames::RunGuard>().checked_mul(4),
        size_of::<independent::Occurrence<'static>>().checked_mul(4),
        size_of::<Coordinate>().checked_mul(8),
        size_of::<OriginDecision>().checked_mul(8),
        size_of::<LocalContractDecisionV1>().checked_mul(4),
        size_of::<OriginObservation>().checked_mul(4),
        size_of::<PreparationResourcesV1<'static, 'static>>().checked_mul(4),
        size_of::<[Mode; 12]>().checked_mul(2),
    ] {
        n = n
            .checked_add(amount.ok_or(QueryError::Resource(Resource::Arithmetic))?)
            .ok_or(QueryError::Resource(Resource::Arithmetic))?;
    }
    Ok(n)
}
fn envelope<'w, F>(budget: &mut Budget<'w>, inspect: F) -> Q<()>
where
    F: FnOnce(&mut Budget<'w>, &mut usize, &mut [Slot; 12], &mut Option<WitnessSummary>) -> Q<()>,
{
    let before = account(budget);
    let saved_slot = budget as *const Budget<'_> as usize;
    let saved_ledger = budget.work_ledger_identity_v1();
    let header = headers::<F>()?;
    budget.charge_work(
        header
            .checked_mul(4)
            .ok_or(QueryError::Resource(Resource::Arithmetic))?,
    )?;
    budget.reserve_storage(header)?;
    let mut owned = header;
    // No owning array or diagnostic envelope is constructed before admission.
    let mut slots: [Slot; 12] = std::array::from_fn(|_| Slot::new());
    let mut witnesses = None;
    let mut envelope = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        super::super::super::accepted_frames::require_idle_for_local_use();
        envelope = Some(frames::start(before));
        inspect(budget, &mut owned, &mut slots, &mut witnesses)
    }));
    let result = match result {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(QueryError::CallbackPanicked)
        }
    };
    // All 12 partial owners stay alive through the entire lexical sequence.
    // They are physically dropped before the original outer postflight/refund.
    drop(slots);
    let floor = before
        .storage
        .checked_add(owned)
        .ok_or(QueryError::Resource(Resource::Arithmetic))?;
    if saved_slot != budget as *const Budget<'_> as usize
        || saved_ledger != budget.work_ledger_identity_v1()
        || budget.storage() < floor
        || budget.work() < before.work
        || budget.peak_storage() < before.peak
        || budget.failed_work().is_some()
        || budget.failed_storage().is_some()
    {
        drop(envelope);
        return Err(QueryError::Resource(Resource::Accounting));
    }
    // CLOSED callbacks create no unowned surplus. Only accepted owned credits
    // are refunded: unexpected surplus remains present and refuses the trace.
    budget.release_storage(owned)?;
    if budget.storage() != before.storage {
        return Err(QueryError::Resource(Resource::Accounting));
    }
    result?;
    envelope
        .ok_or(QueryError::Unavailable("actual local/use envelope absent"))?
        .finish(
            account(budget),
            witnesses.ok_or(QueryError::Unavailable("actual local/use witnesses absent"))?,
        );
    Ok(())
}
pub(super) fn observe(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Q<()> {
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        envelope(budget, |budget, owned, slots, witnesses| {
            owner.with_checked_bf16_nominal_call_v1(
                inventory,
                source.root(),
                source.root(),
                source.call_block(),
                source.source_call(),
                budget,
                |checked, budget| {
                    with_nominal_rich_source_preparation_v1(
                        owner,
                        inventory,
                        source.root(),
                        source.root(),
                        source.call_block(),
                        source.source_call(),
                        budget,
                        |rich, budget| {
                            with_nominal_canonical_facts_observation_v1(
                                owner,
                                inventory,
                                source.root(),
                                source.root(),
                                source.call_block(),
                                source.source_call(),
                                budget,
                                |facts| {
                                    with_nominal_recipe_resources_v1(
                                        facts,
                                        rich,
                                        owned,
                                        |context| {
                                            for (index, mode) in MODES.into_iter().enumerate() {
                                                one_run(
                                                    index,
                                                    mode,
                                                    &mut slots[index],
                                                    witnesses,
                                                    checked,
                                                    rich,
                                                    inputs,
                                                    context,
                                                )?;
                                            }
                                            Ok(())
                                        },
                                    )
                                    .map_err(projection)
                                },
                            )
                        },
                    )
                },
            )
        })
    })
}

#[test]
fn local_use_observer_closed_modes_and_owner_headers_are_explicit() {
    assert_eq!(MODES.len(), 12);
    assert!(
        headers::<fn()>().unwrap()
            >= 4 * size_of::<[Slot; 12]>() + frames::diagnostic_storage_bound()
    );
    assert_eq!(
        headers::<[u8; 4096]>().unwrap() - headers::<[u8; 0]>().unwrap(),
        4 * 4096
    );
}

#[test]
fn local_use_decision_comparison_rejects_each_fallback_drift_and_wrong_refusal() {
    let make = || LocalContractDecisionV1 {
        origin: Some(CheckedReferenceSourceV1::GuardedAccess(3)),
        immutable: true,
        allocation: None,
        allocation_provenance: None,
    };
    let expected = OriginDecision::Allowed(Some(CheckedReferenceSourceV1::GuardedAccess(3)));
    expect_local(Ok(make()), expected, true, None, None).unwrap();
    let mut changed = make();
    changed.origin = None;
    assert!(expect_local(Ok(changed), expected, true, None, None).is_err());
    let mut changed = make();
    changed.immutable = false;
    assert!(expect_local(Ok(changed), expected, true, None, None).is_err());
    let mut changed = make();
    changed.allocation = Some(AllocationContractV1 {
        allocation_origin: 7,
        noalias_class: 3,
        writable: true,
        singleton_object: false,
    });
    assert!(expect_local(Ok(changed), expected, true, None, None).is_err());
    let mut changed = make();
    changed.allocation_provenance = Some(LocalAllocationProvenanceV1::Argument(0));
    assert!(expect_local(Ok(changed), expected, true, None, None).is_err());
    expect_local(
        Err(Error::Unsupported(REGION_REFUSAL)),
        OriginDecision::RegionRefused,
        false,
        None,
        None,
    )
    .unwrap();
    assert!(
        expect_local(
            Err(Error::Incomplete(REGION_REFUSAL)),
            OriginDecision::RegionRefused,
            false,
            None,
            None
        )
        .is_err()
    );
    assert!(
        expect_local(
            Err(Error::Unsupported("different refusal")),
            OriginDecision::RegionRefused,
            false,
            None,
            None
        )
        .is_err()
    );
    assert!(expect_local(Ok(make()), OriginDecision::RegionRefused, false, None, None).is_err());
}
#[test]
fn local_use_expected_panic_rejects_nonmatching_payloads() {
    expected_panic(Box::new(A_PANIC), A_PANIC).unwrap();
    assert!(expected_panic(Box::new(B_PANIC), A_PANIC).is_err());
    assert!(expected_panic(Box::new(7usize), A_PANIC).is_err());
}
