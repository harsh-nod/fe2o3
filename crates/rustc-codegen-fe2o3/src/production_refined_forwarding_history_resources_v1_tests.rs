use super::super::super::history::AuthenticatedRefinedForwardingHistoryV1 as CheckedHistory;
use super::*;
use fe2o3_kernel_opt::{
    CanonicalPolicy6SemanticErrorV1 as P6SemanticError,
    CanonicalPolicy7SemanticErrorV1 as P7SemanticError,
    CheckedCanonicalOptimizationReceiptV1 as TransitionReceipt,
    CheckedCanonicalPolicy5ExecutionRelationV1 as Checked5,
    CheckedCanonicalPolicy6ExecutionRelationV1 as Checked6,
    CheckedCanonicalPolicy7ContinuationRelationV1 as Checked7,
    CheckedCanonicalRefinedForwardingHistoryV1 as CheckedSemantic,
    POLICY6_EXECUTION_RECORD_BYTES_V1, POLICY7_EXECUTION_HEADER_BYTES_V1,
    check_canonical_policy5_execution_relation_v1,
    decode_and_check_canonical_optimization_receipt_v1,
};
use fe2o3_pliron::{
    INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1, KirOptimizationMapErrorV12 as MapError,
    read_unauthenticated_integer_continuation_claim_v1,
};
const MAX_WORK: usize = 1_000_000_000;
const MAX_STORAGE: usize = 1_000_000_000;

struct Observation {
    error: Option<ProductionPipelineError>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
    retained: Option<usize>,
}
fn observe(
    erased: bool,
    mutation: bool,
    profile: Profile,
    replay: bool,
    work_limit: usize,
    storage_limit: usize,
) -> Observation {
    let mut observation = None;
    with_prefix(erased, profile, mutation, |prefix, parent| {
        let (native, receipt) = prepare(
            prefix,
            profile,
            Limits::default(),
            ForwardingLimits::default(),
            parent,
        )
        .unwrap();
        parent.reserve_storage(receipt.retained_storage()).unwrap();
        let claims = if replay {
            let claims = Claims::prepare(&native, parent).unwrap();
            parent.reserve_storage(claims.retained_storage()).unwrap();
            Some(claims)
        } else {
            None
        };
        let sibling = vec![0x59u8; 37];
        let floor = parent.storage() + size_of_val(&sibling) + sibling.capacity();
        let mut work = Work::new(work_limit);
        let (error, accepted, peak, failed_storage, retained) = {
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(17).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = match &claims {
                None => Claims::prepare(&native, &mut budget).map(Some),
                Some(claims) => claims.check(&native, &mut budget).map(|checked| {
                    let retained = checked.retained_storage();
                    budget.reserve_storage(retained).unwrap();
                    drop(checked);
                    budget.release_storage(retained).unwrap();
                    None
                }),
            };
            let (error, retained) = match result {
                Ok(Some(claims)) => {
                    let retained = claims.retained_storage();
                    budget.reserve_storage(retained).unwrap();
                    drop(claims);
                    budget.release_storage(retained).unwrap();
                    (None, Some(retained))
                }
                Ok(None) => (None, Some(claims.as_ref().unwrap().retained_storage())),
                Err(error) => (Some(error), None),
            };
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(sibling, [0x59; 37]);
            (
                error,
                budget.work(),
                budget.peak_storage(),
                budget.failed_storage(),
                retained,
            )
        };
        observation = Some(Observation {
            error,
            work: accepted,
            peak,
            failed_work: work.failed_work(),
            failed_storage,
            retained,
        });
        if let Some(claims) = claims {
            let retained = claims.retained_storage();
            drop(claims);
            parent.release_storage(retained).unwrap();
        }
        release(native, receipt, parent);
    });
    observation.unwrap()
}

#[test]
fn refined_forwarding_history_source_claims_exact_resources_and_final_work_denial() {
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                for replay in [false, true] {
                    let measured =
                        observe(erased, mutation, profile, replay, MAX_WORK, MAX_STORAGE);
                    assert!(measured.error.is_none(), "{:?}", measured.error);
                    assert!(measured.retained.unwrap() > 0);
                    assert_eq!(
                        (measured.failed_work, measured.failed_storage),
                        (None, None)
                    );
                    let exact = observe(
                        erased,
                        mutation,
                        profile,
                        replay,
                        measured.work,
                        measured.peak,
                    );
                    assert!(exact.error.is_none(), "{:?}", exact.error);
                    assert_eq!(exact.retained, measured.retained);
                    assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
                    assert_eq!((exact.failed_work, exact.failed_storage), (None, None));
                    let short = observe(
                        erased,
                        mutation,
                        profile,
                        replay,
                        measured.work - 1,
                        measured.peak,
                    );
                    let Some(ProductionPipelineError::InductionRefinementNativeStage(
                        InductionRefinementNativeStageErrorV1::ForwardingComposition(
                            RefinedForwardingNativeStageErrorV1::Resource(Resource::Work(limit)),
                        ),
                    )) = short.error
                    else {
                        panic!("exact final history work charge");
                    };
                    assert_eq!(
                        (limit.actual(), limit.limit()),
                        (measured.work, measured.work - 1)
                    );
                    assert_eq!(short.work, measured.work - 1);
                    assert_eq!(short.peak, measured.peak);
                    assert_eq!(short.failed_work, Some(measured.work));
                    assert_eq!(short.failed_storage, None);
                }
            }
        }
    }
}

fn same_rows<T: PartialEq + std::fmt::Debug>(a: &[T], b: &[T], budget: &mut Budget<'_>) {
    budget.charge_work(1).unwrap();
    let bytes = size_of_val(a).checked_add(size_of_val(b)).unwrap();
    budget.charge_work(bytes).unwrap();
    assert_eq!(a, b);
}

// Replay the public constituents up to, but not including, the P6 map's
// reservation. No call to Claims::check or the enclosing P6/P7 check is used.
fn replay_prefix_reference(
    native: &PreparedRefinedForwardingNativeOutputV1,
    claims: &Claims,
    budget: &mut Budget<'_>,
) -> (usize, usize, usize) {
    let floor = budget.storage();
    let header = size_of::<CheckedHistory<'_>>()
        .checked_sub(size_of::<Checked6<'_>>())
        .and_then(|n| n.checked_sub(size_of::<Checked7<'_>>()))
        .and_then(|n| n.checked_sub(size_of::<CheckedSemantic<'_>>()))
        .unwrap();
    budget.reserve_storage(header).unwrap();
    budget.charge_work(128).unwrap();
    let actual = native.owner.history();
    match actual {
        Admitted7Ref::Direct(owner) => owner.verify_equivalence(budget).unwrap(),
        Admitted7Ref::Erased(owner) => owner.verify_equivalence(budget).unwrap(),
    }
    budget
        .reserve_storage(POLICY7_EXECUTION_HEADER_BYTES_V1)
        .unwrap();
    native
        .prefix_execution
        .check_history_v1(actual, budget)
        .unwrap();
    budget
        .release_storage(POLICY7_EXECUTION_HEADER_BYTES_V1)
        .unwrap();

    let (bound, checked) = actual.portable_prefix();
    let p5 = checked.intermediate_policy5();
    let inputs = claims.inputs_for_test(native).prefix.prefix.prefix;
    let p6_header = size_of::<Checked6<'_>>()
        .checked_sub(size_of::<Checked5<'_>>())
        .and_then(|n| n.checked_sub(size_of::<TransitionReceipt<'_, '_>>()))
        .unwrap();
    budget.charge_work(3).unwrap();
    budget.reserve_storage(p6_header).unwrap();
    let claim = read_unauthenticated_integer_continuation_claim_v1(
        p5.owner(),
        checked.owner(),
        inputs.continuation.integer_record,
        budget,
    )
    .unwrap();
    budget.charge_work(2).unwrap();
    budget
        .charge_work(POLICY6_EXECUTION_RECORD_BYTES_V1)
        .unwrap();
    assert_eq!(
        inputs.continuation.composition_record,
        checked.execution().canonical_bytes()
    );
    let prefix = check_canonical_policy5_execution_relation_v1(
        bound,
        p5,
        inputs.prefix.policy4_wire,
        inputs.prefix.policy5_record,
        inputs.prefix.load_rows,
        budget,
    )
    .unwrap();
    budget
        .reserve_storage(prefix.storage().retained_storage())
        .unwrap();
    let transition = decode_and_check_canonical_optimization_receipt_v1(
        p5.owner(),
        checked.owner(),
        inputs.continuation.transition_wire,
        budget,
    )
    .unwrap();
    budget
        .reserve_storage(transition.storage().retained_storage())
        .unwrap();
    let continuation = checked.continuation();
    budget
        .charge_work(
            POLICY6_EXECUTION_RECORD_BYTES_V1 + INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1,
        )
        .unwrap();
    assert_eq!(
        claim.canonical_bytes(),
        continuation.execution().canonical_bytes()
    );
    budget.charge_work(2).unwrap();
    let bytes = p5.owner().canonical().canonical_bytes();
    let historical = continuation.native_input_audit_bytes();
    budget
        .charge_work(bytes.len().checked_add(historical.len()).unwrap())
        .unwrap();
    assert_eq!(bytes, historical);
    let a = transition.receipt().candidate();
    let b = continuation.occurrences().candidate();
    macro_rules! rows {
        ($($field:ident),+ $(,)?) => { $(same_rows(a.$field, b.$field, budget);)+ };
    }
    rows!(
        functions,
        blocks,
        segments,
        operations,
        definitions,
        definition_outputs,
        uses,
        edges,
        edge_arguments
    );
    budget
        .reserve_storage(INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1)
        .unwrap();
    continuation
        .execution()
        .check_against(
            p5.owner(),
            checked.owner(),
            continuation.report(),
            continuation.map(),
            budget,
        )
        .unwrap();
    budget
        .release_storage(INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1)
        .unwrap();

    // MapData::check_against performs all work before its sole scratch reserve;
    // check_inner has no budget access. This successful isolated public check
    // measures that opaque scratch without mirroring private Rust layouts.
    let prior_peak = budget.peak_storage();
    let map_floor = budget.storage();
    let mut map_work = Work::new(MAX_WORK);
    let (accepted, attempted) = {
        let mut map_budget = Budget::new(&mut map_work, MAX_STORAGE);
        map_budget.reserve_storage(map_floor).unwrap();
        continuation
            .map()
            .check_against(p5.owner(), checked.owner(), &mut map_budget)
            .unwrap();
        assert_eq!(map_budget.storage(), map_floor);
        assert_eq!(map_budget.failed_storage(), None);
        (
            budget.work().checked_add(map_budget.work()).unwrap(),
            map_budget.peak_storage(),
        )
    };
    assert_eq!(map_work.failed_work(), None);
    assert!(attempted > prior_peak);
    drop(transition);
    drop(prefix);
    budget
        .release_storage(budget.storage().checked_sub(floor).unwrap())
        .unwrap();
    (accepted, prior_peak, attempted)
}

fn storage_prefix_reference(
    erased: bool,
    mutation: bool,
    profile: Profile,
    replay: bool,
) -> (usize, usize, usize) {
    let mut observation = None;
    with_prefix(erased, profile, mutation, |prefix, parent| {
        let parent_floor = parent.storage();
        let parent_ledger = parent.work_ledger_identity_v1();
        let (native, receipt) = prepare(
            prefix,
            profile,
            Limits::default(),
            ForwardingLimits::default(),
            parent,
        )
        .unwrap();
        parent.reserve_storage(receipt.retained_storage()).unwrap();
        let claims = replay.then(|| {
            let claims = Claims::prepare(&native, parent).unwrap();
            parent.reserve_storage(claims.retained_storage()).unwrap();
            claims
        });
        let sibling = vec![0x59u8; 37];
        let floor = parent.storage() + size_of_val(&sibling) + sibling.capacity();
        let mut work = Work::new(MAX_WORK);
        {
            let mut budget = Budget::new(&mut work, MAX_STORAGE);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(17).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            observation = Some(match &claims {
                Some(claims) => replay_prefix_reference(&native, claims, &mut budget),
                None => {
                    native.owner.replay(&mut budget).unwrap();
                    budget
                        .reserve_storage(POLICY7_EXECUTION_HEADER_BYTES_V1)
                        .unwrap();
                    native
                        .prefix_execution
                        .check_history_v1(native.owner.history(), &mut budget)
                        .unwrap();
                    budget
                        .release_storage(POLICY7_EXECUTION_HEADER_BYTES_V1)
                        .unwrap();
                    // lower_native charges three units, then requests scratch.
                    let accepted = budget.work().checked_add(3).unwrap();
                    let scratch = dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES
                        .checked_mul(3)
                        .and_then(|n| n.checked_add(size_of::<String>() * 2))
                        .unwrap();
                    (
                        accepted,
                        budget.peak_storage(),
                        floor.checked_add(scratch).unwrap(),
                    )
                }
            });
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), None);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(sibling, [0x59; 37]);
        }
        assert_eq!(work.failed_work(), None);
        if let Some(claims) = claims {
            let retained = claims.retained_storage();
            drop(claims);
            parent.release_storage(retained).unwrap();
        }
        release(native, receipt, parent);
        assert_eq!(parent.storage(), parent_floor);
        assert!(parent.work_ledger_identity_v1() == parent_ledger);
    });
    observation.unwrap()
}

#[test]
fn refined_forwarding_history_source_storage_short_observes_exact_first_denial() {
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                for replay in [false, true] {
                    let measured =
                        observe(erased, mutation, profile, replay, MAX_WORK, MAX_STORAGE);
                    assert!(measured.error.is_none());
                    let short = observe(
                        erased,
                        mutation,
                        profile,
                        replay,
                        measured.work,
                        measured.peak - 1,
                    );
                    let limit = if replay {
                        let Some(ProductionPipelineError::CheckedOutputPolicy7Stage(
                            P7Error::Portable(error),
                        )) = short.error
                        else {
                            panic!("exact P7 portable Storage phase");
                        };
                        let P7SemanticError::Policy6(error) = *error else {
                            panic!("exact actual P6 execution Storage phase");
                        };
                        let P6SemanticError::Map(MapError::Resources(Resource::Storage(limit))) =
                            *error
                        else {
                            panic!("exact P6 map scratch Storage phase");
                        };
                        limit
                    } else {
                        let Some(ProductionPipelineError::PrivateCellNativeStage(
                            PrivateCellNativeStageErrorV1::Resource(Resource::Storage(limit)),
                        )) = short.error
                        else {
                            panic!("exact native scratch Storage phase");
                        };
                        limit
                    };
                    assert_eq!(
                        (limit.actual(), limit.limit()),
                        (measured.peak, measured.peak - 1)
                    );
                    let (accepted, prior_peak, attempted) =
                        storage_prefix_reference(erased, mutation, profile, replay);
                    assert_eq!(attempted, measured.peak);
                    assert_eq!((short.work, short.peak), (accepted, prior_peak));
                    assert_eq!(short.failed_work, None);
                    assert_eq!(short.failed_storage, Some(measured.peak));
                }
            }
        }
    }
}

#[test]
fn refined_forwarding_history_live_claim_unwind_drops_before_same_ledger_cleanup() {
    with_native(false, Profile::Gfx942, true, |native, _, budget| {
        let floor = budget.storage();
        let token = budget.work_ledger_identity_v1();
        let result: Result<()> = scoped(floor, budget, |budget| {
            let claims = Claims::prepare(native, budget)?;
            budget
                .reserve_storage(claims.retained_storage())
                .map_err(resource)?;
            let checked = claims.check(native, budget)?;
            budget
                .reserve_storage(checked.retained_storage())
                .map_err(resource)?;
            let _live = &checked;
            panic!("owned history failure while actual claims and receipts remain live");
        });
        assert!(matches!(
            result,
            Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                P7Error::Panicked
            ))
        ));
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == token);
        native.verify_equivalence(budget).unwrap();
    });
}

#[test]
fn refined_forwarding_history_existing_denials_survive_new_source_history_success() {
    with_native(true, Profile::Gfx950, false, |native, claims, parent| {
        let mut work = Work::new(MAX_WORK);
        {
            let mut budget = Budget::new(&mut work, MAX_STORAGE);
            let floor = parent.storage() + 41;
            budget.reserve_storage(floor).unwrap();
            assert!(matches!(
                budget.charge_work(MAX_WORK + 1),
                Err(Resource::Work(_))
            ));
            assert!(matches!(
                budget.reserve_storage(MAX_STORAGE + 1),
                Err(Resource::Storage(_))
            ));
            let expected = floor + MAX_STORAGE + 1;
            let token = budget.work_ledger_identity_v1();
            let checked = claims.check(native, &mut budget).unwrap();
            let retained = checked.retained_storage();
            budget.reserve_storage(retained).unwrap();
            drop(checked);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), Some(expected));
            assert!(budget.work_ledger_identity_v1() == token);
        }
        assert_eq!(work.failed_work(), Some(MAX_WORK + 1));
    });
}
