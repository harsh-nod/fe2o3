use super::super::callable_facts_cache_v1::{self as cache, Cache, Facts};
use super::*;
#[path = "../global_statement_index_v1/fixture.rs"]
mod global_fixture;

fn budget(limit: usize) -> Budget {
    Budget {
        remaining: limit,
        limit,
        profile: FlowWorkProfile::default(),
    }
}

fn id(index: u32) -> SemanticCallableIdV1 {
    SemanticCallableIdV1::from_index(index)
}

fn signature(facts: &Facts<'_>, call: &SemanticDirectCallV1) -> bool {
    facts.contract.is_some_and(|contract| {
        call.arguments()
            .iter()
            .map(SemanticOperandV1::ty)
            .eq(contract.signature().arguments())
            && call.destination().map(|d| d.place().ty()) == Some(contract.signature().output())
    })
}

fn summary(
    facts: &Facts<'_>,
    call: &SemanticDirectCallV1,
    arg: usize,
    owned: SemanticTypeIdV1,
) -> [bool; 7] {
    [
        facts.context.is_some_and(|f| f.accepts(call, arg, owned)),
        facts
            .workgroup_context
            .is_some_and(|f| f.accepts(call, arg, owned)),
        facts
            .global_matrix
            .as_deref()
            .is_some_and(|f| f.accepts(call, arg, owned)),
        facts
            .allocation
            .is_some_and(|f| f.accepts(call, arg, owned)),
        facts
            .math_consumer
            .is_some_and(|f| f.accepts(call, arg, owned)),
        signature(facts, call),
        facts.contract.is_some(),
    ]
}

fn direct_call(arguments: Vec<SemanticOperandV1>, output: u32) -> SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) = call(0, arguments, place(4, output), 1) else {
        unreachable!()
    };
    call
}

#[test]
fn callable_cache_preserves_cold_classification_and_all_call_specific_decisions() {
    let types = global_fixture::types();
    let body = direct_function(direct_statements(), false);
    let callables = [
        borrowed_callable(5, true),
        borrowed_callable(8, true),
        borrowed_callable(5, false),
        global_fixture::callable(0, 62),
        global_fixture::callable(15, 63),
    ];
    let mut owner = Cache::new(&body, Some(&types), &callables);
    let mut work = budget(MAX_FLOW_WORK);
    for index in [0, 3, 1, 4, 2, 17, 0, 4, 17] {
        let original_contract = callables.get(index as usize).and_then(contract);
        let expected = cache::cold(Some(&types), callables.get(index as usize), &mut work).unwrap();
        let cached = owner
            .get(&body, Some(&types), &callables, id(index), &mut work)
            .unwrap();
        assert_eq!(cached.contract.copied(), original_contract);
        assert_eq!(cached.contract.copied(), expected.contract.copied());
        assert_eq!(
            cached.global_matrix.as_deref().map(|f| f.pairs()),
            expected.global_matrix.as_deref().map(|f| f.pairs())
        );
        for args in [
            vec![],
            vec![SemanticOperandV1::Copy(place(3, 5))],
            vec![SemanticOperandV1::Move(place(3, 5))],
            vec![SemanticOperandV1::Copy(place(3, 8))],
            vec![
                SemanticOperandV1::Copy(place(3, 5)),
                SemanticOperandV1::Copy(place(2, 5)),
            ],
        ] {
            for output in [4, 7, 20] {
                let call = direct_call(args.clone(), output);
                for arg in 0..3 {
                    for owned in [ty(4), ty(6), ty(21)] {
                        assert_eq!(
                            summary(cached, &call, arg, owned),
                            summary(&expected, &call, arg, owned)
                        );
                    }
                }
            }
        }
    }
    let first = owner
        .get(&body, Some(&types), &callables, id(0), &mut work)
        .unwrap();
    assert!(signature(
        first,
        &direct_call(vec![SemanticOperandV1::Copy(place(3, 5))], 7)
    ));
    assert!(!signature(
        first,
        &direct_call(vec![SemanticOperandV1::Copy(place(3, 8))], 7)
    ));
    assert!(!signature(
        first,
        &direct_call(vec![SemanticOperandV1::Copy(place(3, 5))], 4)
    ));
    assert!(
        owner
            .get(&body, Some(&types), &callables, id(2), &mut work)
            .unwrap()
            .contract
            .is_none()
    );
}

#[test]
fn callable_cache_typed_global_acceptance_still_rechecks_each_actual_signature() {
    let types = global_fixture::types();
    let body = direct_function(direct_statements(), false);
    let callables = [global_fixture::callable(0, 62)];
    let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = &callables[0] else {
        unreachable!()
    };
    let arguments = binding
        .abi()
        .source_input_types()
        .iter()
        .enumerate()
        .map(|(index, ty)| SemanticOperandV1::Copy(global_fixture::place(index as u32 + 100, *ty)))
        .collect::<Vec<_>>();
    let make_call = |arguments| {
        SemanticDirectCallV1::new_callable(
            id(0),
            arguments,
            Some(SemanticCallDestinationV1::new(
                global_fixture::place(104, binding.abi().source_output_type()),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(1),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap()
    };
    let actual = make_call(arguments.clone());
    let mut owner = Cache::new(&body, Some(&types), &callables);
    let mut work = budget(MAX_FLOW_WORK);
    let facts = owner
        .get(&body, Some(&types), &callables, id(0), &mut work)
        .unwrap();
    let fact = *facts.global_matrix.as_deref().unwrap();
    assert!(fact.accepts(&actual, 0, fact.pairs()[0].1));
    for mutation in 0..3 {
        let mut changed = arguments.clone();
        match mutation {
            0 => {
                changed.pop();
            }
            1 => {
                changed.swap(0, 1);
            }
            _ => {
                changed[0] = SemanticOperandV1::Copy(global_fixture::place(100, ty(0)));
            }
        }
        let actual = make_call(changed);
        let cached = owner
            .get(&body, Some(&types), &callables, id(0), &mut work)
            .unwrap();
        assert!(
            !cached
                .global_matrix
                .as_deref()
                .unwrap()
                .accepts(&actual, 0, fact.pairs()[0].1)
        );
    }
    assert!(
        owner
            .get(&body, Some(&types), &callables, id(0), &mut work)
            .unwrap()
            .global_matrix
            .as_deref()
            .unwrap()
            .accepts(&make_call(arguments), 0, fact.pairs()[0].1)
    );
}

#[test]
fn callable_cache_is_bound_to_exact_body_type_and_callable_owners() {
    let body = direct_function(direct_statements(), false);
    let body_clone = body.clone();
    let types = global_fixture::types();
    let types_clone = types.clone();
    let callables = [borrowed_callable(5, true)];
    let callable_clone = callables.clone();
    for mutation in 0..4 {
        let mut owner = Cache::new(&body, Some(&types), &callables);
        let mut work = budget(MAX_FLOW_WORK);
        owner
            .get(&body, Some(&types), &callables, id(0), &mut work)
            .unwrap();
        let before = work.remaining;
        let failure = owner.get(
            if mutation == 0 { &body_clone } else { &body },
            if mutation == 1 {
                Some(&types_clone)
            } else if mutation == 2 {
                None
            } else {
                Some(&types)
            },
            if mutation == 3 {
                &callable_clone
            } else {
                &callables
            },
            id(0),
            &mut work,
        );
        assert!(matches!(
            failure,
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        ));
        assert_eq!(before, work.remaining);
        assert_eq!(owner.test_state(), (1, true, true));
        assert!(matches!(
            owner.get(
                &body,
                Some(&types),
                &callables,
                id(0),
                &mut budget(MAX_FLOW_WORK)
            ),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        ));
    }
    let wrong = [borrowed_callable(5, false)];
    let mut other = Cache::new(&body_clone, None, &wrong);
    assert!(
        other
            .get(&body_clone, None, &wrong, id(0), &mut budget(MAX_FLOW_WORK))
            .unwrap()
            .contract
            .is_none()
    );
}

#[test]
fn callable_cache_first_use_storage_and_existing_work_fail_closed_without_refund() {
    let body = direct_function(direct_statements(), false);
    let callables = [borrowed_callable(5, true)];
    let run = |limit: usize| {
        let mut owner = Cache::new(&body, None, &callables);
        let mut work = budget(limit);
        work.charge(37)
            .and_then(|()| {
                owner
                    .get(&body, None, &callables, id(0), &mut work)
                    .map(|_| ())
            })
            .map(|()| (owner, work))
    };
    let (_, full) = run(MAX_FLOW_WORK).unwrap();
    let used = MAX_FLOW_WORK - full.remaining;
    assert!(used > 37 + 16);
    let (owner, exact) = run(used).unwrap();
    assert_eq!(exact.remaining, 0);
    assert_eq!(owner.test_state(), (1, true, false));
    for limit in 37..used {
        let mut owner = Cache::new(&body, None, &callables);
        let mut work = budget(limit);
        work.charge(37).unwrap();
        let error = owner
            .get(&body, None, &callables, id(0), &mut work)
            .err()
            .unwrap();
        assert!(matches!(
            flow_work_profile_v1::original_error_for_test(error),
            ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits,
                ..
            }
        ));
        assert_eq!(owner.test_state().0, 0);
        assert!(owner.test_state().2);
        assert!(work.remaining <= limit - 37);
        let remaining = work.remaining;
        assert!(matches!(
            owner.get(&body, None, &callables, id(0), &mut work),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        ));
        assert_eq!(work.remaining, remaining);
    }
}

#[test]
fn callable_cache_repeated_hits_remove_classification_not_just_its_debit() {
    let body = direct_function(direct_statements(), false);
    let callables = [borrowed_callable(5, true)];
    let mut owner = Cache::new(&body, None, &callables);
    assert_eq!(owner.test_state(), (0, false, false));
    let mut work = budget(MAX_FLOW_WORK);
    let first = owner
        .get(&body, None, &callables, id(0), &mut work)
        .unwrap() as *const Facts<'_>;
    let before = work.remaining;
    for _ in 0..4096 {
        let stored = owner
            .get(&body, None, &callables, id(0), &mut work)
            .unwrap() as *const Facts<'_>;
        assert_eq!(stored, first);
    }
    let hit_work = before - work.remaining;
    let mut cold_work = budget(MAX_FLOW_WORK);
    for _ in 0..4096 {
        cache::cold(None, Some(&callables[0]), &mut cold_work).unwrap();
    }
    assert_eq!(MAX_FLOW_WORK - cold_work.remaining, 4096 * 16);
    assert!(hit_work < (MAX_FLOW_WORK - cold_work.remaining) / 2);
    assert_eq!(owner.test_state(), (1, true, false));
    eprintln!(
        "callable-cache classifier-loop cached_work={hit_work} cold_work={} rows=1 capacity={} owner_bytes={} row_bytes={}",
        MAX_FLOW_WORK - cold_work.remaining,
        owner.capacity(),
        std::mem::size_of::<Cache<'_>>(),
        std::mem::size_of::<(u32, Facts<'_>)>()
    );
    let mut measure = budget(MAX_FLOW_WORK);
    owner
        .get(&body, None, &callables, id(0), &mut measure)
        .unwrap();
    let hit = MAX_FLOW_WORK - measure.remaining;
    assert!(hit < 16);
    let mut exact = budget(hit);
    owner
        .get(&body, None, &callables, id(0), &mut exact)
        .unwrap();
    assert_eq!(exact.remaining, 0);
    let mut short = budget(hit - 1);
    let error = owner
        .get(&body, None, &callables, id(0), &mut short)
        .err()
        .unwrap();
    assert!(
        matches!(flow_work_profile_v1::original_error_for_test(error),
        ProductionSemanticSsaErrorV1::AggregateResourceLimit { required, limit, .. }
            if required == hit && limit == hit - 1)
    );
    assert_eq!(owner.test_state(), (1, true, true));
}

#[test]
fn callable_cache_sorted_index_keeps_interleaved_and_negative_entries_exact() {
    let body = direct_function(direct_statements(), false);
    let callables = (0..16)
        .map(|index| borrowed_callable(if index % 2 == 0 { 5 } else { 8 }, index % 3 != 0))
        .collect::<Vec<_>>();
    let mut owner = Cache::new(&body, None, &callables);
    let mut work = budget(MAX_FLOW_WORK);
    for index in [
        9, 1, 7, 3, 15, 0, 12, 11, 5, 14, 2, 6, 8, 4, 10, 13, 0, 9, 15, 3,
    ] {
        let expected = contract(&callables[index as usize]);
        assert_eq!(
            owner
                .get(&body, None, &callables, id(index), &mut work)
                .unwrap()
                .contract
                .copied(),
            expected
        );
    }
    assert_eq!(owner.test_state(), (16, true, false));
    let capacity = owner.capacity();
    for index in (0..16).rev() {
        let before = work.remaining;
        assert_eq!(
            owner
                .get(&body, None, &callables, id(index), &mut work)
                .unwrap()
                .contract
                .copied(),
            contract(&callables[index as usize])
        );
        assert!(before - work.remaining <= 6);
        assert_eq!(owner.capacity(), capacity);
    }
}

#[test]
fn callable_cache_exhaustion_after_capacity_growth_never_publishes_a_partial_row() {
    let body = direct_function(direct_statements(), false);
    let callables = vec![borrowed_callable(5, true); 10];
    let mut full_owner = Cache::new(&body, None, &callables);
    let mut full = budget(MAX_FLOW_WORK);
    full_owner
        .get(&body, None, &callables, id(9), &mut full)
        .unwrap();
    let first_work = MAX_FLOW_WORK - full.remaining;
    assert_eq!(full_owner.capacity(), 1);
    full_owner
        .get(&body, None, &callables, id(1), &mut full)
        .unwrap();
    let both_work = MAX_FLOW_WORK - full.remaining;
    assert_eq!(full_owner.capacity(), 2);
    let mut short_owner = Cache::new(&body, None, &callables);
    let mut short = budget(both_work - 1);
    short_owner
        .get(&body, None, &callables, id(9), &mut short)
        .unwrap();
    assert_eq!(short.remaining, both_work - 1 - first_work);
    let before = short.remaining;
    let error = short_owner
        .get(&body, None, &callables, id(1), &mut short)
        .err()
        .unwrap();
    assert!(
        matches!(flow_work_profile_v1::original_error_for_test(error),
        ProductionSemanticSsaErrorV1::AggregateResourceLimit { required, limit, .. }
            if required == both_work && limit == both_work - 1)
    );
    assert!(short.remaining < before);
    assert_eq!(short_owner.test_state(), (1, true, true));
    assert_eq!(short_owner.capacity(), 2);
    let remaining = short.remaining;
    assert!(matches!(
        short_owner.get(&body, None, &callables, id(9), &mut short),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
    assert_eq!(remaining, short.remaining);
}

fn repeated(count: usize, mutation: u8) -> SemanticFunctionDeclV1 {
    let template = direct_function(direct_statements(), false);
    let mut blocks = Vec::new();
    for index in 0..count {
        let mut tag = [0; 32];
        tag[..8].copy_from_slice(&(index as u64 + 1).to_le_bytes());
        let mut args = vec![SemanticOperandV1::Copy(place(3, 5))];
        let mut output = place(4, 7);
        if index == count / 2 {
            match mutation {
                1 => args.push(SemanticOperandV1::Copy(place(2, 5))),
                2 => args[0] = SemanticOperandV1::Copy(place(3, 8)),
                3 => output = place(3, 5),
                4 => {
                    args[0] = SemanticOperandV1::Copy(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(3),
                            vec![
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Dereference,
                                    ty(4),
                                )
                                .unwrap(),
                            ],
                            ty(4),
                        )
                        .unwrap(),
                    )
                }
                5 => args[0] = SemanticOperandV1::Move(place(3, 5)),
                6 => {
                    args[0] = SemanticOperandV1::Copy(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(4),
                            vec![
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(
                                        3,
                                    )),
                                    ty(7),
                                )
                                .unwrap(),
                            ],
                            ty(7),
                        )
                        .unwrap(),
                    )
                }
                _ => {}
            }
        }
        let callee = if mutation == 7 && index == count / 2 {
            1
        } else {
            0
        };
        blocks.push(
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256(tag),
                source(),
                if index == 0 {
                    direct_statements()
                } else {
                    vec![]
                },
                SemanticTerminatorV1::new(source(), call(callee, args, output, index as u32 + 1)),
            )
            .unwrap(),
        );
    }
    blocks.push(block(250, vec![], SemanticTerminatorKindV1::Return));
    function(
        175,
        template.abi().clone(),
        template.locals().to_vec(),
        blocks,
    )
}

#[test]
fn callable_cache_live_dispatch_preserves_late_signature_escape_and_destination_failures() {
    let callables = [borrowed_callable(5, true), borrowed_callable(5, false)];
    let expected = BTreeSet::from([SemanticTransparentBorrowSiteV1 {
        block: 0,
        statement: 1,
    }]);
    for mutation in 0..8 {
        let body = repeated(64, mutation);
        let actual = sites(&body, &callables, &[], MAX_FLOW_WORK, None).unwrap();
        // A scalar classifier does not decide original Move reuse. The separate
        // source SSA planner remains responsible for the mutation5 lifetime.
        if mutation == 0 || mutation == 5 {
            assert_eq!(actual, expected);
        } else {
            assert!(actual.is_empty(), "mutation {mutation}");
        }
    }
}

#[test]
fn callable_cache_four_large_repeated_call_scales_keep_the_fixed_request_limit() {
    let callable = borrowed_callable(5, true);
    let expected = BTreeSet::from([SemanticTransparentBorrowSiteV1 {
        block: 0,
        statement: 1,
    }]);
    // Synthetic dispatch scales, not the four actual GPTOSS source bodies.
    // Actual64 prefixes and the unchanged required source cases are in README.
    for count in [512, 1024, 2048, 4096] {
        let body = repeated(count, 0);
        let run = |limit| sites(&body, std::slice::from_ref(&callable), &[], limit, None);
        assert_eq!(run(MAX_FLOW_WORK).unwrap(), expected);
        let (mut low, mut high) = (0, MAX_FLOW_WORK);
        while low < high {
            let mid = low + (high - low) / 2;
            if run(mid).is_ok() {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        assert_eq!(run(low).unwrap(), expected);
        let error = flow_work_profile_v1::original_error_for_test(run(low - 1).unwrap_err());
        assert!(
            matches!(error, ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits, required, limit,
        } if required == low && limit == low - 1)
        );
        eprintln!(
            "callable-cache synthetic calls={count} complete_work={low} limit={MAX_FLOW_WORK}"
        );
    }
}
