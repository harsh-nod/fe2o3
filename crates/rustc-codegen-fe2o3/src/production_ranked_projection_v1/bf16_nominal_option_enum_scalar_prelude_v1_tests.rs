//! Synthetic component controls; no constructor below grants source authority.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::*;

fn payload_edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn payload_targets(cases: &[(u128, u32)], otherwise: u32) -> SemanticSwitchTargetsV1 {
    SemanticSwitchTargetsV1::new(
        cases
            .iter()
            .map(|(value, target)| {
                SemanticSwitchTargetV1::new(
                    *value,
                    payload_edge(SemanticEdgeRoleV1::SwitchValue, *target),
                )
            })
            .collect(),
        payload_edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise),
    )
    .unwrap()
}

pub(super) fn payload_fixture(
    cases: &[(u128, u32)],
    otherwise: u32,
    second_definition: bool,
) -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let unit = SemanticTypeIdV1::from_index(0);
    let scalar = SemanticTypeIdV1::from_index(1);
    let enumeration = SemanticTypeIdV1::from_index(2);
    let types = vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([201; 32]),
            SemanticLayoutIdentityV1::from_sha256([202; 32]),
            SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([203; 32]),
            SemanticLayoutIdentityV1::from_sha256([204; 32]),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([205; 32]),
            SemanticLayoutIdentityV1::from_sha256([206; 32]),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Enum {
                discriminant: scalar,
                variants: (0..3)
                    .map(|variant| {
                        SemanticEnumVariantV1::new(
                            variant,
                            SemanticAggregateTypeV1::new(vec![]).unwrap(),
                        )
                    })
                    .collect(),
            },
        ),
    ];
    let source = SemanticSourceProvenanceV1::unavailable();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let assign = |local, ty, kind| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, ty),
                SemanticRvalueV1::new(ty, kind),
            )),
        )
    };
    let define = || {
        assign(
            1,
            enumeration,
            SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::EnumVariant(0), vec![])
                .unwrap(),
        )
    };
    let mut statements = vec![define()];
    if second_definition {
        statements.push(define());
    }
    statements.push(assign(
        2,
        scalar,
        SemanticRvalueKindV1::Discriminant(place(1, enumeration)),
    ));
    let mut blocks = vec![
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([207; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(
                source,
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(2, scalar)),
                    targets: payload_targets(cases, otherwise),
                },
            ),
        )
        .unwrap(),
    ];
    for tag in [208, 209, 210] {
        blocks.push(
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([tag; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        );
    }
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([211; 32]),
        SemanticLayoutIdentityV1::from_sha256([212; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([213; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([214; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([215; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([216; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([217; 32]),
        source,
        abi,
        [unit, enumeration, scalar]
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([218 + index as u8; 32]),
                    ty,
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    (types, function)
}

fn fixture() -> (SemanticFunctionDeclV1, Vec<SemanticCallableDeclV1>) {
    let (_, base) = payload_fixture(&[(0, 2), (1, 3)], 2, false);
    let source = SemanticSourceProvenanceV1::unavailable();
    let option = SemanticTypeIdV1::from_index(2);
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(0),
        Vec::new(),
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), Vec::new(), option).unwrap(),
            edge(SemanticEdgeRoleV1::CallReturn, 1),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([40; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([41; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([42; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([43; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([44; 32]),
        source,
        base.abi().clone(),
        base.locals().to_vec(),
        SemanticBlockIdV1::from_index(0),
        vec![
            block(45, Vec::new(), SemanticTerminatorKindV1::Call(call)),
            block(
                46,
                vec![base.blocks()[0].statements()[1].clone()],
                base.blocks()[0].terminator().kind().clone(),
            ),
            block(47, Vec::new(), SemanticTerminatorKindV1::Return),
            block(48, Vec::new(), SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap();
    let callables = vec![SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([50; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([51; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([52; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([53; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([54; 32]),
            source,
            base.abi().clone(),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
            disjoint_slice: option,
            index_witness: option,
            element: option,
            raw_index: option,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([55; 32]),
    }];
    (function, callables)
}

fn nonboolean_option() -> (SemanticFunctionDeclV1, Vec<SemanticCallableDeclV1>) {
    let (base, callables) = fixture();
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let mut blocks = base.blocks().to_vec();
    let SemanticTerminatorKindV1::SwitchInt { discriminant, .. } = blocks[1].terminator().kind()
    else {
        panic!("fixture switch");
    };
    blocks[1] = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([81; 32]),
        provenance,
        blocks[1].statements().to_vec(),
        SemanticTerminatorV1::new(
            provenance,
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: discriminant.clone(),
                targets: payload_targets(&[(2, 3)], 2),
            },
        ),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([82; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([83; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([84; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([85; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([86; 32]),
        provenance,
        base.abi().clone(),
        base.locals().to_vec(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    (function, callables)
}

#[derive(Debug)]
struct Probe {
    result: Result<()>,
    phase: Option<ScalarPhase>,
    option_complete: bool,
    enum_invoked: bool,
    enum_complete: bool,
    scalar_invoked: bool,
    scalar_complete: bool,
    entered: bool,
    work: usize,
    live: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
const FLOOR: usize = 11;
const LIMIT: usize = 1024 * 1024;
fn source_inputs() -> (
    SemanticFunctionDeclV1,
    Vec<SemanticCallableDeclV1>,
    Vec<SemanticTypeDeclV1>,
) {
    let (function, callables) = fixture();
    let (types, _) = payload_fixture(&[(0, 2), (1, 3)], 2, false);
    (function, callables, types)
}
fn source<'a>(
    function: &'a SemanticFunctionDeclV1,
    callables: &'a [SemanticCallableDeclV1],
    types: &'a [SemanticTypeDeclV1],
    budget: &Budget<'_>,
) -> Source<'a> {
    Source {
        function,
        callables,
        types,
        ledger: (
            std::ptr::from_ref(budget) as usize,
            budget.work_ledger_identity_v1(),
        ),
    }
}
fn probe(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
    work_limit: usize,
    storage_limit: usize,
    mode: usize,
) -> Probe {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let before = Custody::take(&budget).unwrap();
    let mut owned = 0usize;
    if let Err(error) =
        Prep::new(&mut budget, &mut owned).reserve_storage(scalar_frame::<(), ()>().unwrap())
    {
        assert_eq!(owned, 0);
        return Probe {
            result: Err(saved_query_error(&error)),
            phase: None,
            option_complete: false,
            enum_invoked: false,
            enum_complete: false,
            scalar_invoked: false,
            scalar_complete: false,
            entered: false,
            work: budget.work(),
            live: budget.storage(),
            peak: budget.peak_storage(),
            failed_work: budget.failed_work(),
            failed_storage: budget.failed_storage(),
        };
    }
    let mut pending = PendingBeforeProvenanceV1::new();
    let source = source(function, callables, types, &budget);
    let mut entered = false;
    let outcome = catch_unwind(AssertUnwindSafe(|| -> BResult<()> {
        let mut resources = Prep::new(&mut budget, &mut owned);
        pending.prepare(&source, &mut resources)?;
        let view = pending.view(&source, &resources)?;
        assert!(view.enum_api_invoked() && view.scalar_api_invoked());
        assert_eq!(
            view.scalar_inventory().counts.len(),
            function.locals().len()
        );
        entered = true;
        match mode {
            0 => Ok(()),
            1 => Err(Backend::Incomplete("scalar checkpoint callback control")),
            _ => std::panic::panic_any(()),
        }
    }));
    let result = match outcome {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            let mapped = saved_query_error(&error);
            pending.failure = Some(error);
            Err(mapped)
        }
        Err(payload) => {
            drop(payload);
            Err(QueryError::CallbackPanicked)
        }
    };
    let scalar_complete = pending
        .scalar
        .completed_for(function, &Prep::new(&mut budget, &mut owned))
        .is_ok();
    let observed = Probe {
        result,
        phase: Some(pending.phase),
        option_complete: pending.earlier.options.dominance.completed().is_some(),
        enum_invoked: pending.earlier.enum_invoked,
        enum_complete: pending.earlier.enumeration.completed().is_some(),
        scalar_invoked: pending.scalar_invoked,
        scalar_complete,
        entered,
        work: budget.work(),
        live: budget.storage(),
        peak: budget.peak_storage(),
        failed_work: budget.failed_work(),
        failed_storage: budget.failed_storage(),
    };
    before.check(&budget, owned).unwrap();
    assert_eq!(budget.storage(), FLOOR + owned);
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.failed_work(), observed.failed_work);
    assert_eq!(budget.failed_storage(), observed.failed_storage);
    observed
}
fn scalar_boundary(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
) -> (usize, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let source = source(function, callables, types, &budget);
    let mut owned = 0;
    let mut earlier = PendingBeforeScalarV1::new();
    {
        let mut resources = Prep::new(&mut budget, &mut owned);
        resources
            .reserve_storage(scalar_frame::<(), ()>().unwrap())
            .unwrap();
        resources.work(32).unwrap();
        earlier.prepare(&source, &mut resources).unwrap();
    }
    let boundary = (budget.work(), budget.storage());
    drop(earlier);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    boundary
}
fn exact_scalar(left: &AssertionDefinitionInventoryV1, right: &AssertionDefinitionInventoryV1) {
    assert_eq!(left.counts, right.counts);
    assert_eq!(left.blocks, right.blocks);
    assert_eq!(left.assignments, right.assignments);
    assert_eq!(left.address_escaped, right.address_escaped);
}

#[test]
fn all_option_enum_scalar_data_match_independent_originals() {
    let (f, c, t) = source_inputs();
    let producers = fe2o3_mir_model::semantic_option_producers_v1(&f, &c).unwrap();
    let options = SemanticOptionDominanceV1::analyze(&f, &producers).unwrap();
    let enumeration = SemanticEnumPayloadDominanceV1::analyze(&f, &t).unwrap();
    let scalar = assertion_definition_inventory(&f).unwrap();
    assert!(scalar.counts.iter().any(|&n| n > 0));
    assert!(scalar.assignments.iter().any(Option::is_some));
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let before = Custody::take(&budget).unwrap();
    let source = source(&f, &c, &t, &budget);
    let mut owned = 0;
    let mut pending = PendingBeforeProvenanceV1::new();
    {
        let mut resources = Prep::new(&mut budget, &mut owned);
        resources
            .reserve_storage(scalar_frame::<(), ()>().unwrap())
            .unwrap();
        pending.prepare(&source, &mut resources).unwrap();
        let view = pending.view(&source, &resources).unwrap();
        assert_eq!(view.option_producers(), producers);
        assert_eq!(view.option_dominance(), &options);
        assert_eq!(view.enum_dominance(), &enumeration);
        exact_scalar(view.scalar_inventory(), &scalar);
        assert!(view.enum_api_invoked() && view.scalar_api_invoked());
        assert!(std::ptr::eq(view.function(), &f));
    }
    before.check(&budget, owned).unwrap();
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
#[test]
fn success_error_and_panic_retain_all_complete_owners_before_refund() {
    let (f, c, t) = source_inputs();
    for mode in 0..3 {
        let result = probe(&f, &c, &t, LIMIT, LIMIT, mode);
        assert_eq!(
            result.result,
            match mode {
                0 => Ok(()),
                1 => Err(QueryError::Unavailable(
                    "scalar checkpoint callback control"
                )),
                _ => Err(QueryError::CallbackPanicked),
            }
        );
        assert_eq!(result.phase, Some(ScalarPhase::BeforeProvenance));
        assert!(
            result.option_complete
                && result.enum_invoked
                && result.enum_complete
                && result.scalar_invoked
                && result.scalar_complete
                && result.entered
        );
        assert!(
            result.live > FLOOR && result.failed_work.is_none() && result.failed_storage.is_none()
        );
    }
}
#[test]
fn exact_and_one_short_resources_preserve_first_denial_and_partial_scalar() {
    let (f, c, t) = source_inputs();
    let full = probe(&f, &c, &t, LIMIT, LIMIT, 0);
    assert_eq!(full.result, Ok(()));
    let exact = probe(&f, &c, &t, full.work, full.peak, 0);
    assert_eq!(exact.result, Ok(()));
    assert_eq!((exact.work, exact.peak), (full.work, full.peak));
    let work = probe(&f, &c, &t, full.work - 1, full.peak, 0);
    assert!(matches!(
        work.result,
        Err(QueryError::Resource(Resource::Work(_)))
    ));
    assert!(work.option_complete && work.enum_complete && work.scalar_invoked && !work.entered);
    assert!(work.failed_work.is_some() && work.failed_storage.is_none());
    let storage = probe(&f, &c, &t, full.work, full.peak - 1, 0);
    assert!(matches!(
        storage.result,
        Err(QueryError::Resource(Resource::Storage(_)))
    ));
    assert!(
        storage.option_complete
            && storage.enum_complete
            && storage.scalar_invoked
            && !storage.entered
    );
    assert!(storage.failed_storage.is_some() && storage.failed_work.is_none());
}
#[test]
fn scalar_boundary_keeps_earlier_owners_and_accepted_partial_credits() {
    let (f, c, t) = source_inputs();
    let boundary = scalar_boundary(&f, &c, &t);
    for result in [
        probe(&f, &c, &t, boundary.0, LIMIT, 0),
        probe(&f, &c, &t, LIMIT, boundary.1, 0),
    ] {
        assert!(result.result.is_err());
        assert!(result.option_complete && result.enum_complete && result.scalar_invoked);
        assert!(!result.scalar_complete && !result.entered);
        assert_eq!(result.live, boundary.1);
    }
    let full = probe(&f, &c, &t, LIMIT, LIMIT, 0);
    let partial = probe(&f, &c, &t, LIMIT, full.peak - 1, 0);
    assert!(partial.live > boundary.1 && partial.scalar_invoked && !partial.scalar_complete);
    // Every later work refusal retains completed earlier owners; the independent
    // retained-scalar controls cover its attached current-block/array internals.
    for limit in boundary.0..full.work {
        let denied = probe(&f, &c, &t, limit, LIMIT, 0);
        assert!(matches!(
            denied.result,
            Err(QueryError::Resource(Resource::Work(_)))
        ));
        assert!(denied.option_complete && denied.enum_complete && denied.scalar_invoked);
        assert!(!denied.scalar_complete && !denied.entered);
    }
}
#[test]
fn original_option_error_prevents_enum_and_scalar_invocation() {
    let (f, c) = nonboolean_option();
    let producers = fe2o3_mir_model::semantic_option_producers_v1(&f, &c).unwrap();
    let expected = SemanticOptionDominanceV1::analyze(&f, &producers).unwrap_err();
    assert!(SemanticEnumPayloadDominanceV1::analyze(&f, &[]).is_err());
    let result = probe(&f, &c, &[], LIMIT, LIMIT, 0);
    assert_eq!(
        result.result,
        Err(QueryError::Unavailable(expected.detail()))
    );
    assert!(!result.enum_invoked && !result.scalar_invoked && !result.entered);
}
#[test]
fn original_enum_error_prevents_scalar_invocation() {
    let (f, c, _) = source_inputs();
    let expected = SemanticEnumPayloadDominanceV1::analyze(&f, &[]).unwrap_err();
    assert!(assertion_definition_inventory(&f).is_ok());
    let result = probe(&f, &c, &[], LIMIT, LIMIT, 0);
    assert_eq!(
        result.result,
        Err(QueryError::Unavailable(expected.detail()))
    );
    assert!(result.option_complete && result.enum_invoked && !result.enum_complete);
    assert!(!result.scalar_invoked && !result.entered);
}
#[test]
fn occupied_retry_preserves_completed_data_and_consumes_no_more_credit() {
    let (f, c, t) = source_inputs();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let source = source(&f, &c, &t, &budget);
    let mut owned = 0;
    let mut pending = PendingBeforeProvenanceV1::new();
    let counts;
    {
        let mut resources = Prep::new(&mut budget, &mut owned);
        resources
            .reserve_storage(scalar_frame::<(), ()>().unwrap())
            .unwrap();
        pending.prepare(&source, &mut resources).unwrap();
        counts = pending
            .scalar
            .completed_for(&f, &resources)
            .unwrap()
            .counts
            .as_ptr();
    }
    let saved = (budget.work(), budget.storage(), owned);
    {
        let mut resources = Prep::new(&mut budget, &mut owned);
        assert!(pending.prepare(&source, &mut resources).is_err());
        assert!(pending.view(&source, &resources).is_err());
        assert_eq!(
            pending
                .scalar
                .completed_for(&f, &resources)
                .unwrap()
                .counts
                .as_ptr(),
            counts
        );
    }
    assert_eq!(pending.phase, ScalarPhase::Terminal);
    assert_eq!((budget.work(), budget.storage(), owned), saved);
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn wrong_ledger_and_prior_denial_prevent_all_new_analysis() {
    let (f, c, t) = source_inputs();
    let mut other_work = Work::new(LIMIT);
    let other_budget = Budget::new(&mut other_work, LIMIT);
    let wrong = source(&f, &c, &t, &other_budget);
    for denied in [false, true] {
        let mut work = Work::new(if denied { 0 } else { LIMIT });
        let mut budget = Budget::new(&mut work, LIMIT);
        if denied {
            assert!(budget.charge_work(1).is_err());
        }
        let first = budget.failed_work();
        let actual = source(&f, &c, &t, &budget);
        let mut owned = 0;
        let mut pending = PendingBeforeProvenanceV1::new();
        assert!(
            pending
                .prepare(
                    if denied { &actual } else { &wrong },
                    &mut Prep::new(&mut budget, &mut owned)
                )
                .is_err()
        );
        assert_eq!(pending.phase, ScalarPhase::Terminal);
        assert!(!pending.earlier.enum_invoked && !pending.scalar_invoked);
        assert!(pending.earlier.options.producers.completed().is_none());
        assert_eq!(owned, 0);
        assert_eq!(budget.failed_work(), first);
    }
}
#[test]
fn detached_equal_source_and_tables_cannot_borrow_completed_checkpoint() {
    let (f, c, t) = source_inputs();
    let detached = f.clone();
    let other_c = c.clone();
    let other_t = t.clone();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let source = source(&f, &c, &t, &budget);
    let mut owned = 0;
    let mut pending = PendingBeforeProvenanceV1::new();
    {
        let mut resources = Prep::new(&mut budget, &mut owned);
        resources
            .reserve_storage(scalar_frame::<(), ()>().unwrap())
            .unwrap();
        pending.prepare(&source, &mut resources).unwrap();
        for wrong in [
            Source {
                function: &detached,
                callables: &c,
                types: &t,
                ledger: source.ledger,
            },
            Source {
                function: &f,
                callables: &other_c,
                types: &t,
                ledger: source.ledger,
            },
            Source {
                function: &f,
                callables: &c,
                types: &other_t,
                ledger: source.ledger,
            },
        ] {
            assert!(pending.view(&wrong, &resources).is_err());
        }
        assert!(pending.view(&source, &resources).is_ok());
    }
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn scalar_frame_is_checked_and_additive_to_unchanged_enum_policy() {
    let rows = scalar_frame_rows::<(), ()>().unwrap();
    assert_eq!(rows.len(), SCALAR_FRAME_ROWS);
    assert_eq!(
        scalar_frame::<(), ()>().unwrap(),
        super::super::enum_frame::<(), ()>().unwrap() + rows.into_iter().sum::<usize>()
    );
    assert!(
        scalar_frame::<[u8; 4096], [u8; 8192]>().unwrap()
            >= scalar_frame::<(), ()>().unwrap() + 2 * 4096 + 2 * 8192
    );
}
#[test]
fn new_header_and_wrapper_refuse_before_earlier_analysis() {
    let (f, c, t) = source_inputs();
    let header = probe(
        &f,
        &c,
        &t,
        LIMIT,
        FLOOR + scalar_frame::<(), ()>().unwrap() - 1,
        0,
    );
    assert!(header.failed_storage.is_some() && header.phase.is_none());
    assert!(!header.enum_invoked && !header.scalar_invoked && !header.entered);
    let work = probe(&f, &c, &t, 31, LIMIT, 0);
    assert!(work.failed_work.is_some() && work.failed_storage.is_none());
    assert!(!work.option_complete && !work.enum_invoked && !work.scalar_invoked);
}
#[test]
fn exact_private_order_and_external_pending_custody_remain_visible() {
    let text: String = include_str!("bf16_nominal_option_enum_scalar_prelude_v1.rs")
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    let prepare = text
        .split("implPendingBeforeProvenanceV1{")
        .nth(1)
        .unwrap()
        .split("fnview")
        .next()
        .unwrap();
    let earlier = prepare.find("self.earlier.prepare(").unwrap();
    let invoked = prepare.find("self.scalar_invoked=true;").unwrap();
    let scalar = prepare.find("self.scalar.prepare_into(").unwrap();
    let complete = prepare
        .find("self.phase=ScalarPhase::BeforeProvenance;")
        .unwrap();
    assert!(earlier < invoked && invoked < scalar && scalar < complete);
    let entry = text.split("pub(incrate::production_ranked_projection_v1)fnwith_nominal_option_enum_scalar_before_provenance_v1")
        .nth(1).unwrap().split("#[cfg(test)]").next().unwrap();
    assert!(
        entry
            .find("letmutpending=PendingBeforeProvenanceV1::new();")
            .unwrap()
            < entry
                .find("owner.with_checked_bf16_nominal_call_v1(")
                .unwrap()
    );
    assert!(entry.contains("with_bf16_nominal_entry_resources_v1"));
    assert!(
        entry.find("drop(pending);").unwrap()
            < entry.find("budget.release_storage(owned)").unwrap()
    );
    for forbidden in [
        "with_nominal_option_enum_before_scalar_v1(",
        "with_nominal_option_first_before_enum_v1(",
        "with_nominal_rich_source_preparation",
        "with_nominal_root_cfg_preparation",
        "Budget::new",
        "Work::new",
        "unmetered(",
        "local_provenance_with_",
        "project_authenticated_capabilities_v1(",
    ] {
        assert!(!prepare.contains(forbidden) && !entry.contains(forbidden));
    }
}

#[test]
fn earlier_definition_error_wins_over_same_source_scalar_error() {
    let (base, callables, types) = source_inputs();
    let mut blocks = base.blocks().to_vec();
    let block = &blocks[1];
    let SemanticStatementKindV1::Assign(original) = block.statements()[0].kind() else {
        panic!("fixture discriminant");
    };
    let mut statements = block.statements().to_vec();
    statements.push(SemanticStatementV1::new(
        base.source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(99),
                vec![],
                original.destination().ty(),
            )
            .unwrap(),
            original.value().clone(),
        )),
    ));
    blocks[1] = SemanticBasicBlockV1::new(
        block.identity(),
        block.source(),
        statements,
        block.terminator().clone(),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        base.identity(),
        base.role(),
        base.item_definition_identity(),
        base.monomorphization_identity(),
        base.generic_type_arguments_identity(),
        base.const_generic_arguments_identity(),
        base.source(),
        base.abi().clone(),
        base.locals().to_vec(),
        base.entry(),
        blocks,
    )
    .unwrap();
    let producers = fe2o3_mir_model::semantic_option_producers_v1(&function, &callables).unwrap();
    let earlier = SemanticOptionDominanceV1::analyze(&function, &producers).unwrap_err();
    let scalar = assertion_definition_inventory(&function).err().unwrap();
    assert_eq!(
        earlier.detail(),
        "a semantic definition is outside the local table"
    );
    assert!(matches!(
        scalar,
        Backend::Unsupported("an assertion proof assignment is outside the semantic local table")
    ));
    let result = probe(&function, &callables, &types, LIMIT, LIMIT, 0);
    assert_eq!(
        result.result,
        Err(QueryError::Unavailable(earlier.detail()))
    );
    assert!(!result.enum_invoked && !result.scalar_invoked && !result.entered);
}
