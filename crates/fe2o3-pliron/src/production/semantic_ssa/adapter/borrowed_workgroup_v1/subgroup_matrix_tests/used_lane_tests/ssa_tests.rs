use super::*;

#[test]
fn checked_subgroup_component_promotes_original_getter_and_reborrow_uses() {
    for kind in [0, 1, 3, 4] {
        let body = used_body(kind, true);
        let types = types();
        let callables = [access(true, 64, true), leaf_callable(kind, true)];
        let sites = classify(&body, &types, &callables, MAX_FLOW_WORK).unwrap();
        for expected in [
            root(),
            projection(),
            SemanticTransparentBorrowSiteV1 {
                block: 2,
                statement: 2,
            },
        ] {
            assert!(
                sites.contains(&expected),
                "kind={kind} missing={expected:?} sites={sites:?}"
            );
        }
        let mut ranges = BTreeMap::new();
        let (input, _, _) = semantic_function_ssa_input_with_event_origins_v1(
            &body,
            Some(&types),
            &callables,
            &sites,
            |block, statement, events| {
                if let Some(statement) = statement {
                    ranges.insert((block, statement), events);
                }
            },
        );
        let plan = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
        for (block, statement, variable) in [(0, 0, 1), (1, 0, 3), (2, 2, 14)] {
            assert!(input.promotable()[variable]);
            assert!(
                plan.promoted_variables()
                    .contains(&SsaVariableIdV1::new(variable as u32))
            );
            let range = &ranges[&(block, statement)];
            assert_eq!(
                plan.resolved_events(SsaBlockIdV1::new(block))
                    .unwrap()
                    .iter()
                    .filter(|(event, resolved)| range.contains(&(*event as usize))
                        && matches!(resolved,SsaResolvedEventV1::Use {variable:got,..}
                    if got.get()==variable as u32))
                    .count(),
                1
            );
        }
    }
}

#[test]
fn lane_sites_do_not_survive_a_poisoned_parent_subgroup_component() {
    let body = used_body(0, true);
    let body = changed(
        &body,
        3,
        vec![],
        call(
            1,
            vec![SemanticOperandV1::Copy(place(3, 4))],
            place(16, 0),
            3,
        ),
    );
    let sites = classify(
        &body,
        &types(),
        &[access(true, 64, true), leaf_callable(0, true)],
        MAX_FLOW_WORK,
    )
    .unwrap();
    assert!(!sites.contains(&root()));
    assert!(!sites.contains(&projection()));
    assert!(!sites.contains(&SemanticTransparentBorrowSiteV1 {
        block: 2,
        statement: 2
    }));
}
