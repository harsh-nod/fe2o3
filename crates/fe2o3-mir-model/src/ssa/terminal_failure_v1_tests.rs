use super::*;

fn variable(index: u32) -> SsaVariableIdV1 {
    SsaVariableIdV1::new(index)
}
fn block(index: u32) -> SsaBlockIdV1 {
    SsaBlockIdV1::new(index)
}
fn edge(target: u32, role: u16) -> SsaEdgeInputV1 {
    SsaEdgeInputV1::new(SsaEdgeRoleV1::new(role), block(target), vec![])
}
fn input(events: Vec<SsaEventV1>, boundary: Option<usize>) -> SsaConstructionInputV1 {
    let mut first = SsaBlockInputV1::new(events, vec![edge(1, 1)]);
    if let Some(boundary) = boundary {
        first = first.with_terminal_failure_start(boundary);
    }
    SsaConstructionInputV1::new(
        block(0),
        2,
        vec![true, true],
        vec![variable(0), variable(1)],
        vec![
            first,
            SsaBlockInputV1::new(vec![SsaEventV1::Use(variable(0))], vec![]),
        ],
    )
}

#[test]
fn terminal_failure_move_preserves_success_definition() {
    let source = input(
        vec![
            SsaEventV1::Use(variable(1)),
            SsaEventV1::Use(variable(0)),
            SsaEventV1::Kill(variable(0)),
        ],
        Some(1),
    );
    let plan = plan_ssa_v1(&source).unwrap();
    assert_eq!(
        plan.resolved_event(block(0), 1),
        plan.resolved_event(block(1), 0)
    );
    assert!(matches!(
        plan.resolved_event(block(0), 2),
        Some(SsaResolvedEventV1::Kill {
            previous: Some(_),
            ..
        })
    ));
    plan.verify_replay(&source, SsaPlannerLimitsV1::default())
        .unwrap();
}

#[test]
fn terminal_failure_move_then_use_is_not_a_copy() {
    for last in [SsaEventV1::Use(variable(0)), SsaEventV1::Kill(variable(0))] {
        let mut events = vec![SsaEventV1::Use(variable(0)), SsaEventV1::Kill(variable(0))];
        if matches!(last, SsaEventV1::Kill(_)) {
            events.push(SsaEventV1::Use(variable(0)));
        }
        events.push(last);
        assert!(matches!(plan_ssa_v1(&input(events, Some(0))),
            Err(SsaPlannerErrorV1::UndefinedAtUse { block: id, event: 2, variable: local })
                if id == block(0) && local == variable(0)));
    }
}

#[test]
fn terminal_failure_boundary_does_not_restore_condition_moves() {
    let events = vec![
        SsaEventV1::Use(variable(0)),
        SsaEventV1::Kill(variable(0)),
        SsaEventV1::Use(variable(1)),
    ];
    assert!(matches!(plan_ssa_v1(&input(events, Some(2))),
        Err(SsaPlannerErrorV1::UndefinedAtUse { block: id, variable: local, .. })
            if id == block(1) && local == variable(0)));
}

#[test]
fn malformed_failure_boundaries_and_definitions_refuse() {
    for (events, start) in [(vec![], 1), (vec![SsaEventV1::Define(variable(0))], 0)] {
        assert_eq!(
            plan_ssa_v1(&input(events, Some(start))).unwrap_err(),
            SsaPlannerErrorV1::InvalidTerminalFailureTail {
                block: block(0),
                start
            }
        );
    }
}

#[test]
fn empty_failure_tail_is_distinct_and_replays_exactly() {
    let ordinary = input(vec![SsaEventV1::Use(variable(1))], None);
    let empty = input(vec![SsaEventV1::Use(variable(1))], Some(1));
    let entire = input(vec![SsaEventV1::Use(variable(1))], Some(0));
    let a = plan_ssa_v1(&ordinary).unwrap();
    let b = plan_ssa_v1(&empty).unwrap();
    let c = plan_ssa_v1(&entire).unwrap();
    assert_ne!(a.identity(), b.identity());
    assert_ne!(b.identity(), c.identity());
    assert!(matches!(
        b.verify_replay(&entire, SsaPlannerLimitsV1::default()),
        Err(SsaPlannerErrorV1::ReplayMismatch { .. })
    ));
}

#[test]
fn failure_kills_do_not_create_a_success_loop_merge() {
    let source = SsaConstructionInputV1::new(
        block(0),
        1,
        vec![true],
        vec![variable(0)],
        vec![
            SsaBlockInputV1::new(vec![], vec![edge(1, 1)]),
            SsaBlockInputV1::new(
                vec![SsaEventV1::Use(variable(0)), SsaEventV1::Kill(variable(0))],
                vec![edge(1, 1), edge(2, 2), edge(2, 3)],
            )
            .with_terminal_failure_start(0),
            SsaBlockInputV1::new(vec![SsaEventV1::Use(variable(0))], vec![]),
        ],
    );
    let plan = plan_ssa_v1(&source).unwrap();
    assert_eq!(plan.merge_variables(block(1)), Some([].as_slice()));
    assert_eq!(
        plan.resolved_event(block(1), 0),
        plan.resolved_event(block(2), 0)
    );
    assert!(plan.edge_arguments(SsaEdgeIdV1::new(block(1), 2)).is_some());
}

#[test]
fn failure_path_still_requires_incoming_values() {
    let source = SsaConstructionInputV1::new(
        block(0),
        1,
        vec![true],
        vec![],
        vec![
            SsaBlockInputV1::new(vec![SsaEventV1::Use(variable(0))], vec![])
                .with_terminal_failure_start(0),
        ],
    );
    assert!(matches!(
        plan_ssa_v1(&source),
        Err(SsaPlannerErrorV1::UndefinedAtUse { .. })
    ));
}

fn empty_identity(
    failure: bool,
    work: &mut WorkBudget,
) -> Result<SsaPlanIdentityV1, SsaPlannerErrorV1> {
    let mut row = SsaBlockInputV1::new(vec![], vec![]);
    if failure {
        row = row.with_terminal_failure_start(0);
    }
    let source = SsaConstructionInputV1::new(block(0), 0, vec![], vec![], vec![row]);
    compute_identity(
        &source,
        &[true],
        &[vec![]],
        &[vec![]],
        &[vec![]],
        &[],
        &[],
        &[vec![]],
        &[vec![]],
        &[vec![]],
        work,
    )
}

#[test]
fn no_failure_tail_keeps_original_identity_bytes() {
    // Original v1 domain plus 108 little-endian fixture bytes; only the reachable
    // block count at byte 24 is nonzero. This is the pre-tail encoding.
    let expected = [
        0x77, 0xfa, 0x37, 0x69, 0x0b, 0x4f, 0x43, 0x44, 0x01, 0x59, 0xaa, 0x95, 0xd0, 0xab, 0x1a,
        0x06, 0x8f, 0xf8, 0xd6, 0x12, 0x49, 0x88, 0x2d, 0x97, 0xfb, 0x58, 0x2c, 0x1e, 0xa3, 0xea,
        0x7a, 0x80,
    ];
    assert_eq!(
        empty_identity(false, &mut WorkBudget::new(7))
            .unwrap()
            .as_bytes(),
        &expected
    );
}

#[test]
fn identity_tail_scan_and_marker_have_independent_exact_work_cuts() {
    // Entry(3), reachable census(1), domain-selection scan(2), block(1),
    // optional boundary encoding(1); this empty graph has no other visits.
    for (failure, required) in [(false, 7), (true, 8)] {
        assert!(empty_identity(failure, &mut WorkBudget::new(required)).is_ok());
        assert!(matches!(
            empty_identity(failure, &mut WorkBudget::new(required - 1)),
            Err(SsaPlannerErrorV1::ResourceLimitExceeded {
                resource: SsaPlannerResourceV1::WorkUnits,
                ..
            })
        ));
    }
}

#[test]
fn unreachable_failure_boundary_does_not_change_reachable_identity() {
    let a = SsaConstructionInputV1::new(
        block(0),
        0,
        vec![],
        vec![],
        vec![
            SsaBlockInputV1::new(vec![], vec![]),
            SsaBlockInputV1::new(vec![], vec![]),
        ],
    );
    let mut b = a.clone();
    b.blocks[1] = b.blocks[1].clone().with_terminal_failure_start(0);
    assert_eq!(
        plan_ssa_v1(&a).unwrap().identity(),
        plan_ssa_v1(&b).unwrap().identity()
    );
}
