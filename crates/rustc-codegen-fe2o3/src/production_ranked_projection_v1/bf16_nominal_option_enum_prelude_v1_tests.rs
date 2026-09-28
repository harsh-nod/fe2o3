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
    phase: Option<EnumPhase>,
    enum_invoked: bool,
    options_complete: bool,
    enum_complete: bool,
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
        Prep::new(&mut budget, &mut owned).reserve_storage(enum_frame::<(), ()>().unwrap())
    {
        assert_eq!(owned, 0);
        return Probe {
            result: Err(saved_query_error(&error)),
            phase: None,
            enum_invoked: false,
            options_complete: false,
            enum_complete: false,
            entered: false,
            work: budget.work(),
            live: budget.storage(),
            peak: budget.peak_storage(),
            failed_work: budget.failed_work(),
            failed_storage: budget.failed_storage(),
        };
    }
    let mut pending = PendingBeforeScalarV1::new();
    let source = source(function, callables, types, &budget);
    let mut entered = false;
    let outcome = catch_unwind(AssertUnwindSafe(|| -> BResult<()> {
        let mut resources = Prep::new(&mut budget, &mut owned);
        pending.prepare(&source, &mut resources)?;
        let view = pending.view(&source, &resources)?;
        assert!(view.enum_api_invoked() && view.enum_dominance().work_units() > 0);
        entered = true;
        match mode {
            0 => Ok(()),
            1 => Err(Backend::Incomplete("enum component callback control")),
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
        enum_invoked: pending.enum_invoked,
        options_complete: pending.options.dominance.completed().is_some(),
        enum_complete: pending.enumeration.completed().is_some(),
        entered,
        work: budget.work(),
        live: budget.storage(),
        peak: budget.peak_storage(),
        failed_work: budget.failed_work(),
        failed_storage: budget.failed_storage(),
    };
    // Actual attached pending owners remain alive at this custody observation.
    before.check(&budget, owned).unwrap();
    assert_eq!(budget.storage(), FLOOR + owned);
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.failed_work(), observed.failed_work);
    assert_eq!(budget.failed_storage(), observed.failed_storage);
    observed
}
fn option_boundary(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
) -> (usize, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let source = source(function, callables, types, &budget);
    let mut owned = 0;
    let mut options = PendingOptionFirstPreludeV1::new();
    {
        let mut resources = Prep::new(&mut budget, &mut owned);
        resources
            .reserve_storage(enum_frame::<(), ()>().unwrap())
            .unwrap();
        resources.work(32).unwrap();
        options.prepare(&source, &mut resources).unwrap();
    }
    let boundary = (budget.work(), budget.storage());
    drop(options);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    boundary
}

#[test]
fn completed_option_and_enum_data_match_independent_originals() {
    let (f, c, t) = source_inputs();
    let producers = fe2o3_mir_model::semantic_option_producers_v1(&f, &c).unwrap();
    let options = SemanticOptionDominanceV1::analyze(&f, &producers).unwrap();
    let enumeration = SemanticEnumPayloadDominanceV1::analyze(&f, &t).unwrap();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let before = Custody::take(&budget).unwrap();
    let source = source(&f, &c, &t, &budget);
    let mut owned = 0;
    let mut pending = PendingBeforeScalarV1::new();
    {
        let mut resources = Prep::new(&mut budget, &mut owned);
        resources
            .reserve_storage(enum_frame::<(), ()>().unwrap())
            .unwrap();
        pending.prepare(&source, &mut resources).unwrap();
        let view = pending.view(&source, &resources).unwrap();
        assert_eq!(view.option_producers(), producers);
        assert_eq!(view.option_dominance(), &options);
        assert_eq!(view.enum_dominance(), &enumeration);
        assert!(view.enum_api_invoked() && std::ptr::eq(view.function(), &f));
        assert!(!view.enum_dominance().grants_authority());
    }
    before.check(&budget, owned).unwrap();
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

fn rebuilt(
    base: &SemanticFunctionDeclV1,
    abi: SemanticFunctionAbiV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([91; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([92; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([93; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([94; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([95; 32]),
        base.source(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
#[test]
fn empty_enum_availability_still_runs_and_completes_actual_analysis() {
    let (base, _, types) = source_inputs();
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([96; 32]),
        base.source(),
        Vec::new(),
        SemanticTerminatorV1::new(base.source(), SemanticTerminatorKindV1::Return),
    )
    .unwrap();
    let function = rebuilt(
        &base,
        base.abi().clone(),
        base.locals().to_vec(),
        vec![block],
    );
    let expected = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    for local in 0..function.locals().len() {
        for variant in 0..3 {
            assert!(
                expected
                    .availability(SemanticLocalIdV1::from_index(local as u32), variant)
                    .is_none()
            );
        }
    }
    assert!(expected.work_units() > 0);
    let result = probe(&function, &[], &types, LIMIT, LIMIT, 0);
    assert_eq!(result.result, Ok(()));
    assert!(result.enum_invoked && result.enum_complete && result.entered);
    assert_eq!(result.phase, Some(EnumPhase::BeforeScalar));
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let source = source(&function, &[], &types, &budget);
    let mut owned = 0;
    let mut pending = PendingBeforeScalarV1::new();
    {
        let mut resources = Prep::new(&mut budget, &mut owned);
        resources
            .reserve_storage(enum_frame::<(), ()>().unwrap())
            .unwrap();
        pending.prepare(&source, &mut resources).unwrap();
        assert_eq!(
            pending.view(&source, &resources).unwrap().enum_dominance(),
            &expected
        );
    }
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn success_callback_error_and_panic_keep_both_complete_owners_until_cleanup() {
    let (f, c, t) = source_inputs();
    for mode in 0..3 {
        let result = probe(&f, &c, &t, LIMIT, LIMIT, mode);
        let expected = match mode {
            0 => Ok(()),
            1 => Err(QueryError::Unavailable("enum component callback control")),
            _ => Err(QueryError::CallbackPanicked),
        };
        assert_eq!(result.result, expected);
        assert_eq!(result.phase, Some(EnumPhase::BeforeScalar));
        assert!(
            result.enum_invoked
                && result.options_complete
                && result.enum_complete
                && result.entered
        );
        assert!(
            result.live > FLOOR && result.failed_work.is_none() && result.failed_storage.is_none()
        );
    }
}
#[test]
fn exact_limits_and_one_short_keep_the_original_first_denial() {
    let (f, c, t) = source_inputs();
    let full = probe(&f, &c, &t, LIMIT, LIMIT, 0);
    assert_eq!(full.result, Ok(()));
    let exact = probe(&f, &c, &t, full.work, full.peak, 0);
    assert_eq!(exact.result, Ok(()));
    assert_eq!((exact.work, exact.peak), (full.work, full.peak));
    let short_work = probe(&f, &c, &t, full.work - 1, full.peak, 0);
    assert!(matches!(
        short_work.result,
        Err(QueryError::Resource(Resource::Work(_)))
    ));
    assert!(short_work.options_complete && short_work.enum_invoked && !short_work.entered);
    assert!(short_work.failed_work.is_some() && short_work.failed_storage.is_none());
    let short_storage = probe(&f, &c, &t, full.work, full.peak - 1, 0);
    assert!(matches!(
        short_storage.result,
        Err(QueryError::Resource(Resource::Storage(_)))
    ));
    assert!(short_storage.options_complete && short_storage.enum_invoked && !short_storage.entered);
    assert!(short_storage.failed_storage.is_some() && short_storage.failed_work.is_none());
}
#[test]
fn enum_admission_boundary_retains_completed_options_and_partial_enum_credit() {
    let (f, c, t) = source_inputs();
    let boundary = option_boundary(&f, &c, &t);
    let first = probe(&f, &c, &t, LIMIT, boundary.1, 0);
    assert!(matches!(
        first.result,
        Err(QueryError::Resource(Resource::Storage(_)))
    ));
    assert!(first.enum_invoked && first.options_complete && !first.enum_complete && !first.entered);
    assert_eq!(first.live, boundary.1);
    let full = probe(&f, &c, &t, LIMIT, LIMIT, 0);
    let partial = probe(&f, &c, &t, LIMIT, full.peak - 1, 0);
    assert!(partial.live > boundary.1 && partial.options_complete && partial.enum_invoked);
    assert!(!partial.enum_complete && !partial.entered);
    let work_boundary = probe(&f, &c, &t, boundary.0, LIMIT, 0);
    assert!(matches!(
        work_boundary.result,
        Err(QueryError::Resource(Resource::Work(_)))
    ));
    assert!(work_boundary.options_complete && work_boundary.enum_invoked && !work_boundary.entered);
}
#[test]
fn enum_failure_precedes_independently_present_allocation_failure() {
    let (base, callables, _) = source_inputs();
    let absent = SemanticTypeIdV1::from_index(10);
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([97; 32]),
        SemanticLayoutIdentityV1::from_sha256([98; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            absent,
            SemanticAbiPassModeV1::Ignore,
        ))],
        base.abi().return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let mut locals = base.locals().to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([99; 32]),
        absent,
        SemanticLocalRoleV1::Argument(0),
        base.source(),
    ));
    let function = rebuilt(&base, abi, locals, base.blocks().to_vec());
    // Both failures are independently present in this SAME source/empty types.
    // Scalar/provenance inputs come from their unchanged original APIs.
    let enum_error = SemanticEnumPayloadDominanceV1::analyze(&function, &[]).unwrap_err();
    let scalar = assertion_definition_inventory(&function).unwrap();
    let provenance = local_provenance_with_scalar_inventory_v1(
        &callables,
        &[],
        &function,
        &scalar.counts,
        &scalar.address_escaped,
    )
    .unwrap();
    let later =
        local_allocation_contracts(&[], &function, &provenance.allocation_origins).unwrap_err();
    assert_eq!(
        enum_error.detail(),
        "an enum payload type is outside the type table"
    );
    assert!(matches!(
        later,
        Backend::Unsupported("a kernel argument type is outside the semantic type table")
    ));
    let result = probe(&function, &callables, &[], LIMIT, LIMIT, 0);
    assert_eq!(
        result.result,
        Err(QueryError::Unavailable(enum_error.detail()))
    );
    assert_eq!(result.phase, Some(EnumPhase::Terminal));
    assert!(
        result.options_complete && result.enum_invoked && !result.enum_complete && !result.entered
    );
    assert!(result.live > option_boundary(&function, &callables, &[]).1);
}
#[test]
fn option_error_still_precedes_and_prevents_enum_invocation() {
    let (f, c) = nonboolean_option();
    let producers = fe2o3_mir_model::semantic_option_producers_v1(&f, &c).unwrap();
    let expected = SemanticOptionDominanceV1::analyze(&f, &producers).unwrap_err();
    assert!(SemanticEnumPayloadDominanceV1::analyze(&f, &[]).is_err());
    let result = probe(&f, &c, &[], LIMIT, LIMIT, 0);
    assert_eq!(
        result.result,
        Err(QueryError::Unavailable(expected.detail()))
    );
    assert!(!result.enum_invoked && !result.enum_complete && !result.entered);
}
#[test]
fn retry_is_terminal_without_replacing_either_completed_owner() {
    let (f, c, t) = source_inputs();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let source = source(&f, &c, &t, &budget);
    let mut owned = 0;
    let mut pending = PendingBeforeScalarV1::new();
    {
        let mut resources = Prep::new(&mut budget, &mut owned);
        resources
            .reserve_storage(enum_frame::<(), ()>().unwrap())
            .unwrap();
        pending.prepare(&source, &mut resources).unwrap();
    }
    let producers = pending.options.producers.completed().unwrap().as_ptr();
    let enumeration = std::ptr::from_ref(pending.enumeration.completed().unwrap());
    let saved = (budget.work(), budget.storage(), owned);
    {
        let mut resources = Prep::new(&mut budget, &mut owned);
        assert!(pending.prepare(&source, &mut resources).is_err());
        assert!(pending.view(&source, &resources).is_err());
    }
    assert_eq!(pending.phase, EnumPhase::Terminal);
    assert_eq!(
        pending.options.producers.completed().unwrap().as_ptr(),
        producers
    );
    assert_eq!(
        std::ptr::from_ref(pending.enumeration.completed().unwrap()),
        enumeration
    );
    assert_eq!((budget.work(), budget.storage(), owned), saved);
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}
#[test]
fn wrong_ledger_and_prior_denial_refuse_before_option_or_enum_work() {
    let (f, c, t) = source_inputs();
    let mut first_work = Work::new(LIMIT);
    let first_budget = Budget::new(&mut first_work, LIMIT);
    let wrong_source = source(&f, &c, &t, &first_budget);
    for denied in [false, true] {
        let mut work = Work::new(if denied { 0 } else { LIMIT });
        let mut budget = Budget::new(&mut work, LIMIT);
        if denied {
            assert!(budget.charge_work(1).is_err());
        }
        let denial = budget.failed_work();
        let actual_source = source(&f, &c, &t, &budget);
        let mut owned = 0;
        let mut pending = PendingBeforeScalarV1::new();
        let chosen = if denied {
            &actual_source
        } else {
            &wrong_source
        };
        assert!(
            pending
                .prepare(chosen, &mut Prep::new(&mut budget, &mut owned))
                .is_err()
        );
        assert_eq!(pending.phase, EnumPhase::Terminal);
        assert!(!pending.enum_invoked && pending.options.producers.completed().is_none());
        assert_eq!(owned, 0);
        assert_eq!(budget.failed_work(), denial);
    }
}
#[test]
fn detached_equal_source_cannot_borrow_completed_before_scalar_data() {
    let (f, c, t) = source_inputs();
    let detached = f.clone();
    assert_eq!(f, detached);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let source = source(&f, &c, &t, &budget);
    let wrong = Source {
        function: &detached,
        callables: &c,
        types: &t,
        ledger: source.ledger,
    };
    let mut owned = 0;
    let mut pending = PendingBeforeScalarV1::new();
    {
        let mut resources = Prep::new(&mut budget, &mut owned);
        resources
            .reserve_storage(enum_frame::<(), ()>().unwrap())
            .unwrap();
        pending.prepare(&source, &mut resources).unwrap();
        assert!(pending.view(&wrong, &resources).is_err());
        assert!(pending.view(&source, &resources).is_ok());
    }
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn typed_frame_is_additive_to_the_exact_parent_policy() {
    let rows = enum_frame_rows::<(), ()>().unwrap();
    assert_eq!(rows.len(), ENUM_FRAME_ROWS);
    assert_eq!(
        enum_frame::<(), ()>().unwrap(),
        super::super::frame::<(), ()>().unwrap() + rows.into_iter().sum::<usize>()
    );
    let small = enum_frame::<(), ()>().unwrap();
    let large = enum_frame::<[u8; 4096], [u8; 8192]>().unwrap();
    assert!(large >= small + 2 * 4096 + 2 * 8192);
}
#[test]
fn lexical_order_is_options_then_actual_enum_then_before_scalar() {
    let text: String = include_str!("bf16_nominal_option_enum_prelude_v1.rs")
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    let prepare = text
        .split("implPendingBeforeScalarV1{")
        .nth(1)
        .unwrap()
        .split("fnview")
        .next()
        .unwrap();
    let options = prepare.find("self.options.prepare(").unwrap();
    let invoked = prepare.find("self.enum_invoked=true;").unwrap();
    let enumeration = prepare.find("self.enumeration.prepare_into(").unwrap();
    let complete = prepare.find("self.phase=EnumPhase::BeforeScalar;").unwrap();
    assert!(options < invoked && invoked < enumeration && enumeration < complete);
    let entry = text.split("pub(incrate::production_ranked_projection_v1)fnwith_nominal_option_enum_before_scalar_v1")
        .nth(1).unwrap().split("#[cfg(test)]").next().unwrap();
    assert!(
        entry
            .find("letmutpending=PendingBeforeScalarV1::new();")
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
        "with_nominal_option_first_before_enum_v1",
        "with_nominal_rich_source_preparation",
        "with_nominal_root_cfg_preparation",
        "Budget::new",
        "Work::new",
        "unmetered(",
        "assertion_definition_inventory(",
        "local_provenance_with_",
        "project_authenticated_capabilities_v1(",
    ] {
        assert!(!entry.contains(forbidden) && !prepare.contains(forbidden));
    }
}

#[test]
fn new_header_and_wrapper_work_refuse_before_model_invocation() {
    let (f, c, t) = source_inputs();
    let header = probe(
        &f,
        &c,
        &t,
        LIMIT,
        FLOOR + enum_frame::<(), ()>().unwrap() - 1,
        0,
    );
    assert!(header.failed_storage.is_some() && header.failed_work.is_none());
    assert!(header.phase.is_none() && !header.enum_invoked && !header.entered);
    let work = probe(&f, &c, &t, 31, LIMIT, 0);
    assert!(work.failed_work.is_some() && work.failed_storage.is_none());
    assert_eq!(work.phase, Some(EnumPhase::Terminal));
    assert!(!work.options_complete && !work.enum_invoked && !work.entered);
}
