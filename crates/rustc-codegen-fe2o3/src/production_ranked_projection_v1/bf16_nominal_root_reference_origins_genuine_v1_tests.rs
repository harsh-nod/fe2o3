//! Genuine same-owner S5A oracle. It does not call the origin producer, definition
//! visitor, source-call join helper, or the ordinary checked-reference adapter.
use super::*;
use crate::production_ranked_projection_v1::root_guarded_access_preparation_v1::RootGuardedSourceCallV1;

#[derive(Default)]
struct OriginOracle {
    guarded: Oracle,
    definitions: Vec<u8>,
    origins: Vec<Option<CheckedReferenceOriginV1>>,
    fifo: Vec<usize>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OriginObservation {
    guarded: Observation,
    associations: usize,
    origins: usize,
    fifo: usize,
    seeds: usize,
    origin_frame: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OriginMode {
    Observe,
    Error,
    Panic,
    Occupied,
    ForeignPendingLedger,
}
const ORIGIN_REFUSAL: &str = "actual root reference origins callback refusal";

fn expect_call(
    actual: Option<&RootGuardedSourceCallV1>,
    ordinal: usize,
    block: usize,
    callee: SemanticCallableIdV1,
    destination: SemanticLocalIdV1,
    guard: usize,
) -> Result<()> {
    if actual.is_none_or(|row| {
        row.source_call_ordinal != ordinal
            || row.block != block
            || row.callee != callee
            || row.destination != destination
            || row.guarded_access != guard
    }) {
        return Err(Error::Incomplete(
            "origin source oracle call association differs",
        ));
    }
    Ok(())
}
fn expect_origin(
    actual: Option<CheckedReferenceOriginV1>,
    expected: Option<CheckedReferenceOriginV1>,
) -> Result<()> {
    if actual != expected {
        return Err(Error::Incomplete(
            "origin source oracle reference payload differs",
        ));
    }
    Ok(())
}
fn record_definition(place: &SemanticPlaceV1, counts: &mut [u8]) {
    if matches!(
        place.projections().first().map(|p| p.kind()),
        Some(SemanticProjectionKindV1::Dereference)
    ) {
        return;
    }
    if let Some(slot) = counts.get_mut(place.local().index() as usize) {
        *slot = slot.saturating_add(1);
    }
}
fn inspect_origins(
    actual: &ActualRootReferenceOriginsV1<'_>,
    rich: &RichNominalSourceTablesV1<'_>,
    checked: &CheckedBf16NominalCallV1<'_>,
    context: &mut NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_>,
    oracle: &mut OriginOracle,
    input_count: usize,
) -> Result<OriginObservation> {
    // The independent S4 source rescan authenticates graph rows/order, actual
    // prefix/index namespace, every guard field and the absence of sites first.
    let guarded = inspect_payload(
        &actual.guarded,
        rich,
        checked,
        context,
        &mut oracle.guarded,
        input_count,
    )?;
    let function = actual.guarded.prefix.function;
    let callables = checked
        .emission()
        .owner()
        .semantic_ssa()
        .source_semantic()
        .callables();
    context.with_resources(|resources| {
        let locals = function.locals().len();
        resources.work(
            locals
                .checked_mul(2)
                .and_then(|n| n.checked_add(256))
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )?;
        resources.reserve(&mut oracle.definitions, locals)?;
        oracle.definitions.resize(locals, 0);
        resources.reserve(&mut oracle.origins, locals)?;
        oracle.origins.resize(locals, None);
        // Independent explicit statement cases; no shared definition visitor.
        for block in function.blocks() {
            resources.work(32)?;
            for statement in block.statements() {
                resources.work(128)?;
                match statement.kind() {
                    SemanticStatementKindV1::Assign(value) => {
                        record_definition(value.destination(), &mut oracle.definitions)
                    }
                    SemanticStatementKindV1::Store(value) => {
                        record_definition(value.destination(), &mut oracle.definitions)
                    }
                    SemanticStatementKindV1::AtomicRmw(value) => {
                        record_definition(value.destination(), &mut oracle.definitions);
                        record_definition(value.address(), &mut oracle.definitions);
                    }
                    SemanticStatementKindV1::AtomicCompareExchange(value) => {
                        record_definition(value.destination(), &mut oracle.definitions);
                        record_definition(value.address(), &mut oracle.definitions);
                    }
                    SemanticStatementKindV1::SetDiscriminant { place, .. }
                    | SemanticStatementKindV1::Deinitialize(place) => {
                        record_definition(place, &mut oracle.definitions)
                    }
                    SemanticStatementKindV1::StorageLive(_)
                    | SemanticStatementKindV1::StorageDead(_)
                    | SemanticStatementKindV1::Assume(_)
                    | SemanticStatementKindV1::Nop => {}
                }
            }
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
                resources.work(64)?;
                if let Some(destination) = call.destination() {
                    record_definition(destination.place(), &mut oracle.definitions);
                }
            }
        }
        let mut seeds = 0usize;
        // Shared-borrow seeds precede every source-call seed, matching source order.
        for block in function.blocks() {
            resources.work(32)?;
            for statement in block.statements() {
                resources.work(128)?;
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                let destination = assignment.destination();
                if !destination.projections().is_empty()
                    || oracle.definitions.get(destination.local().index() as usize) != Some(&1)
                {
                    continue;
                }
                let SemanticRvalueKindV1::Borrow { kind, place } = assignment.value().kind() else {
                    continue;
                };
                resources.work(
                    place
                        .projections()
                        .len()
                        .checked_mul(4)
                        .and_then(|n| n.checked_add(16))
                        .ok_or_else(|| resource(Resource::Arithmetic))?,
                )?;
                if !matches!(kind, SemanticBorrowKindV1::Shared)
                    || !place.projections().iter().any(|projection| {
                        matches!(
                            projection.kind(),
                            SemanticProjectionKindV1::Index(_)
                                | SemanticProjectionKindV1::ConstantIndex { .. }
                        )
                    })
                {
                    continue;
                }
                let local = destination.local().index() as usize;
                oracle.origins[local] = Some(CheckedReferenceOriginV1 {
                    source: CheckedReferenceSourceV1::ProjectedSharedBorrow,
                    availability: None,
                });
                resources.push(&mut oracle.fifo, local)?;
                seeds = seeds
                    .checked_add(1)
                    .ok_or_else(|| resource(Resource::Arithmetic))?;
            }
        }
        let mut source_ordinal = 0usize;
        let mut guard = 0usize;
        for (block_index, block) in function.blocks().iter().enumerate() {
            resources.work(64)?;
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            resources.work(256)?;
            let ordinal = source_ordinal;
            source_ordinal = source_ordinal
                .checked_add(1)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. },
                ..
            }) = callables.get(call.callee().index() as usize)
            else {
                continue;
            };
            let place = call
                .destination()
                .ok_or(Error::Incomplete("origin oracle missing call destination"))?
                .place();
            if !place.projections().is_empty() {
                return Err(Error::Incomplete(
                    "origin oracle projected call destination",
                ));
            }
            let destination = place.local();
            expect_call(
                actual.guarded.source_calls.get(guard),
                ordinal,
                block_index,
                call.callee(),
                destination,
                guard,
            )?;
            let local = destination.index() as usize;
            if oracle.definitions.get(local) != Some(&1) || oracle.origins.get(local) != Some(&None)
            {
                return Err(Error::Incomplete(
                    "origin oracle call definition or origin conflicts",
                ));
            }
            let availability =
                rich.option_dominance()
                    .availability(destination)
                    .ok_or(Error::Incomplete(
                        "origin oracle call Option availability absent",
                    ))?;
            oracle.origins[local] = Some(CheckedReferenceOriginV1 {
                source: CheckedReferenceSourceV1::GuardedAccess(guard),
                availability: Some(CapabilityAvailabilityV1::Option(availability)),
            });
            resources.push(&mut oracle.fifo, local)?;
            guard = guard
                .checked_add(1)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            seeds = seeds
                .checked_add(1)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
        }
        assert_eq!(actual.guarded.source_calls.len(), guard);
        assert_eq!(actual.guarded.accesses.len(), guard);
        let mut cursor = 0usize;
        // Independent index FIFO interpreter over the already source-checked graph.
        while let Some(source) = oracle.fifo.get(cursor).copied() {
            resources.work(64)?;
            cursor = cursor
                .checked_add(1)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            let Some(origin) = oracle.origins[source] else {
                continue;
            };
            let edges = actual
                .guarded
                .prefix
                .graph
                .edges()
                .get(source)
                .ok_or(Error::Incomplete("origin oracle graph source absent"))?;
            for edge in edges {
                resources.work(256)?;
                let authorization = match edge.kind {
                    CapabilityEdgeKindV1::Alias
                    | CapabilityEdgeKindV1::AuthenticatedOptionPayload => edge.use_block,
                    CapabilityEdgeKindV1::AuthenticatedEnumPayload {
                        construction_block, ..
                    } => construction_block,
                    _ => continue,
                };
                if let Some(availability) = origin.availability
                    && !capability_availability_allows(
                        rich.option_dominance(),
                        rich.enum_payload_dominance(),
                        availability,
                        SemanticBlockIdV1::from_index(authorization as u32),
                    )
                {
                    return Err(Error::Incomplete(
                        "origin oracle transport availability refused",
                    ));
                }
                if oracle.definitions.get(edge.destination) != Some(&1) {
                    return Err(Error::Incomplete(
                        "origin oracle transport definition differs",
                    ));
                }
                let expected = match edge.kind {
                    CapabilityEdgeKindV1::AuthenticatedEnumPayload { availability, .. } => {
                        CheckedReferenceOriginV1 {
                            availability: Some(CapabilityAvailabilityV1::EnumPayload(availability)),
                            ..origin
                        }
                    }
                    _ => origin,
                };
                let slot = oracle
                    .origins
                    .get_mut(edge.destination)
                    .ok_or(Error::Incomplete(
                        "origin oracle transport destination absent",
                    ))?;
                match slot {
                    None => {
                        *slot = Some(expected);
                        resources.push(&mut oracle.fifo, edge.destination)?;
                    }
                    Some(existing) if *existing == expected => {}
                    Some(_) => return Err(Error::Incomplete("origin oracle transport conflict")),
                }
            }
        }
        resources.work(
            locals
                .checked_mul(64)
                .and_then(|n| {
                    oracle
                        .fifo
                        .len()
                        .checked_mul(32)
                        .and_then(|q| n.checked_add(q))
                })
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )?;
        assert_eq!(actual.origins.definitions, oracle.definitions);
        assert_eq!(actual.origins.origins.len(), locals);
        let mut origins = 0usize;
        for (local, expected) in oracle.origins.iter().copied().enumerate() {
            expect_origin(actual.origins.origins[local], expected)?;
            origins += usize::from(expected.is_some());
        }
        assert_eq!(actual.origins.fifo, oracle.fifo);
        assert_eq!(actual.origins.cursor, cursor);
        assert_eq!(cursor, oracle.fifo.len());
        Ok(OriginObservation {
            guarded,
            associations: guard,
            origins,
            fifo: cursor,
            seeds,
            origin_frame: 0,
        })
    })
}

fn run_origins(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    budget: &mut Budget<'_>,
    mode: OriginMode,
) -> Q<OriginObservation> {
    let _accepted_run = super::super::accepted_frames::begin(match mode {
        OriginMode::Observe => super::super::accepted_frames::Mode::Observe,
        OriginMode::Occupied => super::super::accepted_frames::Mode::Occupied,
        OriginMode::Error => super::super::accepted_frames::Mode::Error,
        OriginMode::Panic => super::super::accepted_frames::Mode::Panic,
        OriginMode::ForeignPendingLedger => {
            super::super::accepted_frames::Mode::ForeignPendingLedger
        }
    });
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        let slot = budget as *const Budget<'_> as usize;
        let identity = budget.work_ledger_identity_v1();
        let before = budget.storage();
        let before_work = budget.work();
        let before_peak = budget.peak_storage();
        budget.charge_work(4 * HEADERS)?;
        budget.reserve_storage(HEADERS)?;
        let mut owned = HEADERS;
        // These physical owners precede every rich/facts/graph factory callback.
        let mut pending = PendingActualRootPrefixIndicesV1::new();
        let mut oracle = OriginOracle::default();
        let mut entered = false;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            owner.with_checked_bf16_nominal_call_v1(
                inventory, source.root(), source.root(), source.call_block(), source.source_call(),
                budget, |checked, budget| {
                    with_nominal_rich_source_preparation_v1(
                        owner, inventory, source.root(), source.root(), source.call_block(),
                        source.source_call(), budget, |rich, budget| {
                            with_nominal_canonical_facts_observation_v1(
                                owner, inventory, source.root(), source.root(), source.call_block(),
                                source.source_call(), budget, |facts| {
                                    with_nominal_recipe_resources_v1(facts, rich, &mut owned, |context| {
                                        let accepted_stage = super::super::accepted_frames::enter();
                                        let observed = context.with_actual_root_reference_origins_v1(
                                            checked, rich, actual_inputs, &mut pending,
                                            |actual, context| {
                                                entered = true;
                                                let observed = inspect_origins(
                                                    &actual, rich, checked, context, &mut oracle,
                                                    actual_inputs.inputs().len(),
                                                )?;
                                                match mode {
                                                    OriginMode::Error => Err(Error::Incomplete(ORIGIN_REFUSAL)),
                                                    OriginMode::Panic => panic!("{ORIGIN_REFUSAL}"),
                                                    _ => Ok(observed),
                                                }
                                            },
                                        )?;
                                        drop(accepted_stage);
                                        if mode == OriginMode::Occupied {
                                            let refused = context.with_actual_root_reference_origins_v1(
                                                checked, rich, actual_inputs, &mut pending,
                                                |_, _| -> Result<()> { panic!("occupied origins entered") },
                                            );
                                            assert!(matches!(refused, Err(Error::Incomplete(
                                                "actual root assembly cannot be replaced or retried"
                                            ))));
                                        }
                                        if mode == OriginMode::ForeignPendingLedger {
                                            // Inert corruption only; no source preparation uses this ledger.
                                            let mut foreign_work = Work::new(0);
                                            let foreign = Budget::new(&mut foreign_work, 0);
                                            let (slot, saved) = pending.ledger.unwrap();
                                            assert!(saved != foreign.work_ledger_identity_v1());
                                            pending.ledger = Some((slot, foreign.work_ledger_identity_v1()));
                                            let refused = context.with_actual_root_reference_origins_v1(
                                                checked, rich, actual_inputs, &mut pending,
                                                |_, _| -> Result<()> { panic!("foreign origins entered") },
                                            );
                                            assert!(matches!(refused, Err(Error::CanonicalAssertions(
                                                crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Accounting)
                                            ))));
                                        }
                                        Ok(observed)
                                    }).map_err(projection)
                                },
                            )
                        },
                    )
                },
            )
        }));
        let mut result = match outcome {
            Ok(result) => result,
            Err(payload) => { drop(payload); Err(QueryError::CallbackPanicked) }
        };
        assert!(entered && pending.completed && pending.guarded.completed() && pending.origins.completed());
        assert!(pending.origins.frame_credits > 0);
        if let Ok(observed) = &mut result {
            observed.guarded.assembly_frame = pending.frame_credits;
            observed.guarded.access_frame = pending.guarded.frame_credits;
            observed.origin_frame = pending.origins.frame_credits;
        }
        // Actual callback-error/panic controls, not arbitrary instruction-point injection.
        drop(pending);
        drop(oracle);
        let floor = before.checked_add(owned).ok_or(QueryError::Resource(Resource::Arithmetic))?;
        if slot != budget as *const Budget<'_> as usize || identity != budget.work_ledger_identity_v1()
            || budget.storage() < floor || budget.work() < before_work || budget.peak_storage() < before_peak
            || budget.failed_work().is_some() || budget.failed_storage().is_some()
        {
            let _ = result;
            return Err(QueryError::Resource(Resource::Accounting));
        }
        budget.release_storage(owned)?;
        assert_eq!(budget.storage(), before);
        result
    })
}
pub(super) fn observe_actual_root_reference_origins_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Q<()> {
    super::super::accepted_frames::start(super::super::accepted_frames::Scope::S5A);
    let before = budget.work();
    let observed = run_origins(
        owner,
        source,
        inventory,
        actual_inputs,
        budget,
        OriginMode::Observe,
    )?;
    assert_eq!(
        run_origins(
            owner,
            source,
            inventory,
            actual_inputs,
            budget,
            OriginMode::Occupied
        )?,
        observed
    );
    assert!(matches!(
        run_origins(
            owner,
            source,
            inventory,
            actual_inputs,
            budget,
            OriginMode::Error
        ),
        Err(QueryError::Unavailable(_))
    ));
    assert!(matches!(
        run_origins(
            owner,
            source,
            inventory,
            actual_inputs,
            budget,
            OriginMode::Panic
        ),
        Err(QueryError::CallbackPanicked)
    ));
    assert_eq!(
        run_origins(
            owner,
            source,
            inventory,
            actual_inputs,
            budget,
            OriginMode::ForeignPendingLedger
        )?,
        observed
    );
    let work = budget
        .work()
        .checked_sub(before)
        .ok_or(QueryError::Resource(Resource::Accounting))?;
    eprintln!(
        "fe2o3-root-reference-origin-v1 associations={} origins={} fifo={} seeds={} assembly_frame={} access_frame={} origin_frame={} work={}",
        observed.associations,
        observed.origins,
        observed.fifo,
        observed.seeds,
        observed.guarded.assembly_frame,
        observed.guarded.access_frame,
        observed.origin_frame,
        work,
    );
    eprintln!(
        "fe2o3-root-reference-origin-controls-v1 occupied=pass error=pass panic=pass foreign_pending_ledger=pass"
    );
    super::super::accepted_frames::flush(
        super::super::accepted_frames::Scope::S5A,
        work,
        observed.guarded.assembly_frame,
    );
    Ok(())
}

#[test]
fn actual_origin_genuine_headers_cover_outer_owners_and_control_transfers() {
    assert!(
        HEADERS
            >= 4 * size_of::<PendingActualRootPrefixIndicesV1>()
                + 4 * size_of::<OriginOracle>()
                + 4 * size_of::<OriginObservation>()
                + 4 * size_of::<Budget<'static>>()
                + 4 * size_of::<Work>()
    );
}
#[test]
fn origin_call_oracle_rejects_missing_and_each_equal_count_identity_substitution() {
    let expected = RootGuardedSourceCallV1 {
        source_call_ordinal: 4,
        block: 7,
        callee: SemanticCallableIdV1::from_index(2),
        destination: SemanticLocalIdV1::from_index(9),
        guarded_access: 1,
    };
    let check = |actual: Option<&RootGuardedSourceCallV1>| {
        expect_call(
            actual,
            4,
            7,
            SemanticCallableIdV1::from_index(2),
            SemanticLocalIdV1::from_index(9),
            1,
        )
    };
    check(Some(&expected)).unwrap();
    assert!(check(None).is_err());
    for field in 0..5 {
        let mut changed = expected;
        match field {
            0 => changed.source_call_ordinal ^= 1,
            1 => changed.block ^= 1,
            2 => changed.callee = SemanticCallableIdV1::from_index(3),
            3 => changed.destination = SemanticLocalIdV1::from_index(10),
            4 => changed.guarded_access ^= 1,
            _ => unreachable!(),
        }
        assert!(check(Some(&changed)).is_err());
    }
}
#[test]
fn origin_payload_oracle_rejects_missing_extra_and_neighboring_guard_index() {
    let origin = CheckedReferenceOriginV1 {
        source: CheckedReferenceSourceV1::GuardedAccess(1),
        availability: None,
    };
    expect_origin(Some(origin), Some(origin)).unwrap();
    assert!(expect_origin(None, Some(origin)).is_err());
    assert!(expect_origin(Some(origin), None).is_err());
    assert!(
        expect_origin(
            Some(CheckedReferenceOriginV1 {
                source: CheckedReferenceSourceV1::GuardedAccess(0),
                ..origin
            }),
            Some(origin)
        )
        .is_err()
    );
    assert!(
        expect_origin(
            Some(CheckedReferenceOriginV1 {
                source: CheckedReferenceSourceV1::ProjectedSharedBorrow,
                ..origin
            }),
            Some(origin)
        )
        .is_err()
    );
}
