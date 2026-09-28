//! Synthetic component controls only; no constructed MIR supplies genuine authority.
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

#[derive(Clone, Copy, Debug)]
struct Probe {
    result: Result<()>,
    phase: Option<Phase>,
    completed: bool,
    entered: bool,
    work: usize,
    live: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
const FLOOR: usize = 11;
const LIMIT: usize = 1024 * 1024;
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
    let reserve = Prep::new(&mut budget, &mut owned).reserve_storage(frame::<(), ()>().unwrap());
    if let Err(error) = reserve {
        assert_eq!(owned, 0);
        return Probe {
            result: Err(saved_query_error(&error)),
            phase: None,
            completed: false,
            entered: false,
            work: budget.work(),
            live: budget.storage(),
            peak: budget.peak_storage(),
            failed_work: budget.failed_work(),
            failed_storage: budget.failed_storage(),
        };
    }
    let mut pending = PendingOptionFirstPreludeV1::new();
    let source = Source {
        function,
        callables,
        types,
        ledger: before.ledger,
    };
    let mut entered = false;
    let outcome = catch_unwind(AssertUnwindSafe(|| -> BResult<()> {
        let mut resources = Prep::new(&mut budget, &mut owned);
        pending.prepare(&source, &mut resources)?;
        let view = pending.view(&source, &resources)?;
        assert_eq!(view.producers().len(), 1);
        assert!(view.dominance().work_units() > 0);
        entered = true;
        match mode {
            0 => Ok(()),
            1 => Err(Backend::Incomplete("component callback control")),
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
    let observed = Probe {
        result,
        phase: Some(pending.phase),
        completed: pending.dominance.completed().is_some(),
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
fn source_inputs() -> (
    SemanticFunctionDeclV1,
    Vec<SemanticCallableDeclV1>,
    Vec<SemanticTypeDeclV1>,
) {
    let (function, callables) = fixture();
    let (types, _) = payload_fixture(&[(0, 2), (1, 3)], 2, false);
    (function, callables, types)
}

#[test]
fn completed_retained_data_equals_unchanged_original_apis() {
    let (function, callables, types) = source_inputs();
    // Harness-only original expectations computed before the measured component.
    let expected_producers =
        fe2o3_mir_model::semantic_option_producers_v1(&function, &callables).unwrap();
    let expected_dominance =
        SemanticOptionDominanceV1::analyze(&function, &expected_producers).unwrap();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let before = Custody::take(&budget).unwrap();
    let mut owned = 0;
    let mut pending = PendingOptionFirstPreludeV1::new();
    let source = Source {
        function: &function,
        callables: &callables,
        types: &types,
        ledger: before.ledger,
    };
    {
        let mut resources = Prep::new(&mut budget, &mut owned);
        resources
            .reserve_storage(frame::<(), ()>().unwrap())
            .unwrap();
        pending.prepare(&source, &mut resources).unwrap();
        let view = pending.view(&source, &resources).unwrap();
        assert_eq!(view.producers(), expected_producers);
        assert_eq!(view.dominance(), &expected_dominance);
        assert!(std::ptr::eq(view.function(), &function));
        assert_eq!(pending.initial.indices.len(), function.locals().len());
        assert_eq!(pending.initial.leaders.len(), function.locals().len());
        assert_eq!(pending.initial.predicates.len(), function.locals().len());
        assert_eq!(pending.initial.edges.len(), function.locals().len());
        assert!(pending.initial.edges.iter().all(Vec::is_empty));
        assert!(pending.initial.stores.is_empty() && pending.initial.loads.is_empty());
    }
    before.check(&budget, owned).unwrap();
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn success_callback_error_and_panic_keep_complete_owner_until_cleanup() {
    let (f, c, t) = source_inputs();
    for mode in 0..3 {
        let p = probe(&f, &c, &t, LIMIT, LIMIT, mode);
        let expected = match mode {
            0 => Ok(()),
            1 => Err(QueryError::Unavailable("component callback control")),
            _ => Err(QueryError::CallbackPanicked),
        };
        assert_eq!(p.result, expected);
        assert_eq!(p.phase, Some(Phase::BeforeEnum));
        assert!(p.entered && p.completed && p.live > FLOOR);
        assert_eq!(p.failed_work, None);
        assert_eq!(p.failed_storage, None);
    }
}

#[test]
fn complete_exact_limits_and_one_short_preserve_first_denial() {
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
    assert_eq!(work.phase, Some(Phase::Terminal));
    assert!(work.failed_work.is_some() && work.failed_storage.is_none() && !work.entered);
    let storage = probe(&f, &c, &t, full.work, full.peak - 1, 0);
    assert!(matches!(
        storage.result,
        Err(QueryError::Resource(Resource::Storage(_)))
    ));
    assert_eq!(storage.phase, Some(Phase::Terminal));
    assert!(storage.failed_storage.is_some() && storage.failed_work.is_none() && !storage.entered);
}

#[test]
fn every_new_initial_storage_boundary_refuses_before_later_analysis() {
    let (f, c, t) = source_inputs();
    let locals = f.locals().len();
    let mut accepted = FLOOR + frame::<(), ()>().unwrap();
    let header = probe(&f, &c, &t, LIMIT, accepted - 1, 0);
    assert!(header.failed_storage.is_some());
    assert_eq!(header.phase, None);
    for unit in [
        size_of::<Option<ProjectedDisjointIndexV1>>(),
        size_of::<Option<ProjectedGridLeaderV1>>(),
        size_of::<Option<GuardPredicateV1>>(),
        size_of::<Vec<CapabilityEdgeV1>>(),
    ] {
        accepted += locals * unit;
        let denied = probe(&f, &c, &t, LIMIT, accepted - 1, 0);
        assert!(denied.failed_storage.is_some() && !denied.entered && !denied.completed);
        assert_eq!(denied.phase, Some(Phase::Terminal));
    }
}

#[test]
fn every_new_initial_work_boundary_is_terminal() {
    let (f, c, t) = source_inputs();
    let mut boundary = 64;
    let first = probe(&f, &c, &t, boundary - 1, LIMIT, 0);
    assert!(first.failed_work.is_some() && !first.entered);
    assert_eq!(first.phase, Some(Phase::Terminal));
    boundary += c.len() * 2;
    let guard = probe(&f, &c, &t, boundary - 1, LIMIT, 0);
    assert!(guard.failed_work.is_some() && !guard.entered);
    for _ in 0..4 {
        boundary += f.locals().len();
        let denied = probe(&f, &c, &t, boundary - 1, LIMIT, 0);
        assert!(denied.failed_work.is_some() && !denied.entered && !denied.completed);
        assert_eq!(denied.phase, Some(Phase::Terminal));
    }
}

#[test]
fn retry_is_terminal_without_replacing_completed_model_allocations() {
    let (f, c, t) = source_inputs();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let ledger = (
        std::ptr::from_ref(&budget) as usize,
        budget.work_ledger_identity_v1(),
    );
    let mut owned = 0;
    let source = Source {
        function: &f,
        callables: &c,
        types: &t,
        ledger,
    };
    let mut pending = PendingOptionFirstPreludeV1::new();
    let mut resources = Prep::new(&mut budget, &mut owned);
    resources
        .reserve_storage(frame::<(), ()>().unwrap())
        .unwrap();
    pending.prepare(&source, &mut resources).unwrap();
    let producers = pending.producers.completed().unwrap().as_ptr();
    let dominance = std::ptr::from_ref(pending.dominance.completed().unwrap());
    assert!(matches!(
        pending.prepare(&source, &mut resources),
        Err(Backend::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(Resource::Accounting)
        ))
    ));
    assert_eq!(pending.phase, Phase::Terminal);
    assert_eq!(pending.producers.completed().unwrap().as_ptr(), producers);
    assert_eq!(
        std::ptr::from_ref(pending.dominance.completed().unwrap()),
        dominance
    );
    assert!(pending.view(&source, &resources).is_err());
    drop(resources);
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn wrong_ledger_is_refused_before_any_source_allocation() {
    let (f, c, t) = source_inputs();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let source = Source {
        function: &f,
        callables: &c,
        types: &t,
        ledger: (0, budget.work_ledger_identity_v1()),
    };
    let mut pending = PendingOptionFirstPreludeV1::new();
    let mut resources = Prep::new(&mut budget, &mut owned);
    assert!(matches!(
        pending.prepare(&source, &mut resources),
        Err(Backend::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(Resource::Accounting)
        ))
    ));
    assert_eq!(pending.phase, Phase::Terminal);
    assert_eq!(owned, 0);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn equal_but_detached_source_cannot_borrow_before_enum_view() {
    let (f, c, t) = source_inputs();
    let other = f.clone();
    assert_eq!(f, other);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let ledger = (
        std::ptr::from_ref(&budget) as usize,
        budget.work_ledger_identity_v1(),
    );
    let mut owned = 0;
    let mut pending = PendingOptionFirstPreludeV1::new();
    let mut resources = Prep::new(&mut budget, &mut owned);
    resources
        .reserve_storage(frame::<(), ()>().unwrap())
        .unwrap();
    let source = Source {
        function: &f,
        callables: &c,
        types: &t,
        ledger,
    };
    pending.prepare(&source, &mut resources).unwrap();
    let detached = Source {
        function: &other,
        callables: &c,
        types: &t,
        ledger,
    };
    assert!(pending.view(&detached, &resources).is_err());
    assert!(pending.view(&source, &resources).is_ok());
    drop(resources);
    drop(pending);
    budget.release_storage(owned).unwrap();
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

#[test]
fn option_failure_precedes_independently_present_enum_failure() {
    let (f, c) = nonboolean_option();
    let producers = fe2o3_mir_model::semantic_option_producers_v1(&f, &c).unwrap();
    let option = SemanticOptionDominanceV1::analyze(&f, &producers).unwrap_err();
    let later_enum = fe2o3_mir_model::SemanticEnumPayloadDominanceV1::analyze(&f, &[]).unwrap_err();
    assert_ne!(option, later_enum);
    assert_eq!(
        option.detail(),
        "an Option capability switch has no exact Some edge"
    );
    assert_eq!(
        later_enum.detail(),
        "an enum payload type is outside the type table"
    );
    let actual = probe(&f, &c, &[], LIMIT, LIMIT, 0);
    assert_eq!(actual.result, Err(QueryError::Unavailable(option.detail())));
    assert_eq!(actual.phase, Some(Phase::Terminal));
    assert!(!actual.completed && !actual.entered && actual.live > FLOOR);
}

#[test]
fn added_frame_scales_with_actual_generic_capture_and_result() {
    let small = frame::<(), ()>().unwrap();
    let large = frame::<[u8; 4096], [u8; 8192]>().unwrap();
    assert!(large >= small + 2 * 4096 + 2 * 8192);
    assert_eq!(frame_rows::<(), ()>().unwrap().len(), FRAME_ROWS);
}

#[test]
fn exact_source_order_and_closed_authentic_entry_stay_visible() {
    // Inspect the same complete token spellings across rustfmt line breaks.
    let text: String = include_str!("bf16_nominal_option_first_prelude_v1.rs")
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    let body = text
        .split("implPendingOptionFirstPreludeV1{")
        .nth(1)
        .unwrap()
        .split("fnview")
        .next()
        .unwrap();
    let guard = body
        .find("reject_retired_production_intrinsics_v1(")
        .unwrap();
    let initial = body.find("self.initial.prepare(").unwrap();
    let producers = body.find("self.producers.prepare_into(").unwrap();
    let dominance = body.find("self.dominance.prepare_into(").unwrap();
    let complete = body.find("self.phase=Phase::BeforeEnum;").unwrap();
    assert!(
        guard < initial && initial < producers && producers < dominance && dominance < complete
    );
    let entry = text.split("pub(incrate::production_ranked_projection_v1)fnwith_nominal_option_first_before_enum_v1").nth(1).unwrap()
        .split("#[cfg(test)]").next().unwrap();
    assert!(
        entry.contains("with_bf16_nominal_entry_resources_v1")
            && entry.contains("with_checked_bf16_nominal_call_v1")
    );
    assert!(
        entry.find("drop(pending);").unwrap()
            < entry.find("budget.release_storage(owned)").unwrap()
    );
    for forbidden in [
        "with_nominal_rich_source_preparation",
        "with_nominal_root_cfg_preparation",
        "Budget::new",
        "Work::new",
        "unmetered(",
    ] {
        assert!(!entry.contains(forbidden));
    }
}

#[test]
fn prior_sticky_denial_refuses_without_new_owned_credit() {
    let (f, c, t) = source_inputs();
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, LIMIT);
    assert!(budget.charge_work(1).is_err());
    let first = budget.failed_work();
    let source = Source {
        function: &f,
        callables: &c,
        types: &t,
        ledger: (
            std::ptr::from_ref(&budget) as usize,
            budget.work_ledger_identity_v1(),
        ),
    };
    let mut owned = 0;
    let mut pending = PendingOptionFirstPreludeV1::new();
    {
        let mut resources = Prep::new(&mut budget, &mut owned);
        assert!(matches!(
            pending.prepare(&source, &mut resources),
            Err(Backend::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Accounting)
            ))
        ));
    }
    assert_eq!(pending.phase, Phase::Terminal);
    assert!(pending.producers.completed().is_none() && pending.dominance.completed().is_none());
    assert_eq!(owned, 0);
    assert_eq!(budget.failed_work(), first);
}
