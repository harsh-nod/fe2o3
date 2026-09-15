use super::*;

fn query(
    request: &InertSemanticMirRequestV1,
    target: u32,
    work: &mut u64,
) -> Result<SemanticPhaseIncomingCommitmentV1, SemanticMirErrorV1> {
    SemanticPhaseIncomingCommitmentV1::reconstruct_for_defined_function(
        SemanticFunctionIdV1(target),
        &request.functions,
        &request.callables,
        work,
    )
}

#[test]
fn incoming_query_matches_existing_phase_recipe_without_attaching_authority() {
    let request = fixture::request();
    let before = request.functions.clone();
    let record = fixture::observe(&request);
    let mut work = HARD_MAX_VALIDATION_WORK_V1;
    assert_eq!(query(&request, 2, &mut work).unwrap(), record.incoming());
    assert!(work < HARD_MAX_VALIDATION_WORK_V1);
    assert_eq!(request.functions, before);
    assert!(
        request
            .functions
            .iter()
            .all(|f| f.defined_capability_contract().is_none())
    );

    let mut work = HARD_MAX_VALIDATION_WORK_V1;
    let ordinary = query(&request, 1, &mut work).unwrap();
    assert_eq!(ordinary.count(), 1);
    assert_ne!(ordinary.digest(), record.incoming().digest());
}

#[test]
fn incoming_query_commits_the_complete_multi_caller_roster() {
    let mut request = fixture::request();
    let mut work = HARD_MAX_VALIDATION_WORK_V1;
    let baseline = query(&request, 2, &mut work).unwrap();
    // This is an inert roster mutation, not admission of a second source owner.
    let mut caller = request.functions[1].clone();
    caller.identity = SemanticFunctionIdentityV1([241; 32]);
    let mut functions = request.functions.to_vec();
    functions.push(caller);
    request.functions = functions.into_boxed_slice();
    let mut work = HARD_MAX_VALIDATION_WORK_V1;
    let joined = query(&request, 2, &mut work).unwrap();
    assert_eq!(joined.count(), baseline.count() + 1);
    assert_ne!(joined.digest(), baseline.digest());
    request.functions.last_mut().unwrap().identity = SemanticFunctionIdentityV1([242; 32]);
    let mut work = HARD_MAX_VALIDATION_WORK_V1;
    let changed = query(&request, 2, &mut work).unwrap();
    assert_eq!(changed.count(), joined.count());
    assert_ne!(changed.digest(), joined.digest());
}

#[test]
fn incoming_query_rejects_missing_recursive_and_unwinding_callers() {
    let mut request = fixture::request();
    let mut work = HARD_MAX_VALIDATION_WORK_V1;
    assert!(query(&request, 0, &mut work).is_err());
    let to_caller = request
        .callables
        .iter()
        .position(|c| {
            matches!(
                c,
                SemanticCallableDeclV1::Defined {
                    function: SemanticFunctionIdV1(1)
                }
            )
        })
        .unwrap();
    let SemanticTerminatorKindV1::Call(call) = &mut request.functions[1].blocks[0].terminator.kind
    else {
        panic!("fixture's exact incoming call")
    };
    call.callee = SemanticCallableIdV1(to_caller as u32);
    let mut work = HARD_MAX_VALIDATION_WORK_V1;
    assert_eq!(
        query(&request, 1, &mut work),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );

    let mut request = fixture::request();
    let SemanticTerminatorKindV1::Call(call) = &mut request.functions[1].blocks[0].terminator.kind
    else {
        panic!("fixture's exact incoming call")
    };
    call.unwind = SemanticUnwindActionV1::Continue;
    let mut work = HARD_MAX_VALIDATION_WORK_V1;
    assert_eq!(
        query(&request, 2, &mut work),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );
}

#[test]
fn incoming_query_preserves_hard_bounds_and_exact_shared_work() {
    let request = fixture::request();
    let mut work = HARD_MAX_VALIDATION_WORK_V1;
    assert_eq!(
        query(&request, request.functions.len() as u32, &mut work),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );
    assert_eq!(work, HARD_MAX_VALIDATION_WORK_V1);
    let mut work = HARD_MAX_VALIDATION_WORK_V1 + 1;
    assert_eq!(
        query(&request, 2, &mut work),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );
    assert_eq!(work, HARD_MAX_VALIDATION_WORK_V1 + 1);

    let mut work = HARD_MAX_VALIDATION_WORK_V1;
    let expected = query(&request, 2, &mut work).unwrap();
    let used = HARD_MAX_VALIDATION_WORK_V1 - work;
    let mut exact = used;
    assert_eq!(query(&request, 2, &mut exact).unwrap(), expected);
    assert_eq!(exact, 0);
    let mut short = used - 1;
    assert!(query(&request, 2, &mut short).is_err());
    assert!(short < used);
    let mut zero = 0;
    assert!(query(&request, 2, &mut zero).is_err());
    assert_eq!(zero, 0);
}
