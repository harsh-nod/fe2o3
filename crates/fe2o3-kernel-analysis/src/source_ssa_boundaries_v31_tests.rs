use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::{
    SsaBlockInputV1, SsaConstructionInputV1, SsaDefinitionIdV1, SsaEdgeInputV1, SsaEdgeRoleV1,
    SsaEventV1, SsaPlannerLimitsV1, plan_ssa_with_limits_v1,
};

fn block(events: Vec<SsaEventV1>, targets: &[u32]) -> SsaBlockInputV1 {
    SsaBlockInputV1::new(
        events,
        targets
            .iter()
            .enumerate()
            .map(|(ordinal, target)| {
                SsaEdgeInputV1::new(
                    SsaEdgeRoleV1::new(u16::try_from(ordinal + 1).unwrap()),
                    Block::new(*target),
                    vec![],
                )
            })
            .collect(),
    )
}

fn fixture(blocks: Vec<SsaBlockInputV1>, variables: u32) -> (Plan, Vec<Vec<Block>>) {
    let successors = blocks
        .iter()
        .map(|block| block.edges().iter().map(SsaEdgeInputV1::target).collect())
        .collect();
    let input = SsaConstructionInputV1::new(
        Block::new(0),
        variables,
        vec![true; variables as usize],
        vec![Variable::new(0)],
        blocks,
    );
    (
        plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap(),
        successors,
    )
}

fn run(
    plan: &Plan,
    successors: &[Vec<Block>],
    work: usize,
    storage: usize,
) -> (Result<Vec<(Block, Variable, Value)>>, usize, usize) {
    let mut work = Work::new(work);
    let mut budget = Budget::new(&mut work, storage);
    budget.reserve_storage(23).unwrap();
    let result = (|| {
        let (boundaries, receipt) =
            SourceSsaBoundariesV31::derive(plan, Block::new(0), successors, &mut budget)?;
        assert_eq!(budget.storage(), 23);
        budget.reserve_storage(receipt.retained_storage())?;
        assert_eq!(boundaries.entry(), Block::new(0));
        assert!(std::ptr::eq(boundaries.successors(), successors));
        let result = (|| {
            let mut values = Vec::new();
            for row in &boundaries.checked.rows {
                values.push((
                    row.block,
                    row.variable,
                    boundaries.value(plan, row.block, row.variable, &mut budget)?,
                ));
            }
            Ok(values)
        })();
        drop(boundaries);
        budget.release_storage(receipt.retained_storage())?;
        result
    })();
    let work = budget.work();
    let peak = budget.peak_storage();
    assert_eq!(budget.storage(), 23);
    (result, work, peak)
}

fn failure_run(
    plan: &Plan,
    successors: &[Vec<Block>],
    blocks: &[SourceSsaBlockEventsV299],
    work: usize,
    storage: usize,
) -> (Result<Vec<(Block, Variable, Value)>>, usize, usize) {
    let mut work = Work::new(work);
    let mut budget = Budget::new(&mut work, storage);
    budget.reserve_storage(23).unwrap();
    let result = (|| {
        let (boundaries, receipt) = SourceSsaBoundariesV31::derive_with_terminal_failures_v299(
            plan,
            Block::new(0),
            successors,
            blocks,
            &mut budget,
        )?;
        assert_eq!(budget.storage(), 23);
        budget.reserve_storage(receipt.retained_storage())?;
        let result = (|| {
            let mut values = Vec::new();
            for row in &boundaries.checked.rows {
                values.push((
                    row.block,
                    row.variable,
                    boundaries.value(plan, row.block, row.variable, &mut budget)?,
                ));
            }
            Ok(values)
        })();
        drop(boundaries);
        budget.release_storage(receipt.retained_storage())?;
        result
    })();
    let work = budget.work();
    let peak = budget.peak_storage();
    assert_eq!(budget.storage(), 23);
    (result, work, peak)
}

fn failure_fixture() -> (Plan, Vec<Vec<Block>>, Vec<SourceSsaBlockEventsV299>) {
    let a = Variable::new(0);
    let (plan, edges) = fixture(
        vec![
            block(
                vec![SsaEventV1::Use(a), SsaEventV1::Use(a), SsaEventV1::Kill(a)],
                &[1],
            )
            .with_terminal_failure_start(1),
            block(vec![SsaEventV1::Use(a)], &[]),
        ],
        1,
    );
    (
        plan,
        edges,
        vec![
            SourceSsaBlockEventsV299 {
                events: 3,
                terminal_failure_start: Some(1),
            },
            SourceSsaBlockEventsV299 {
                events: 1,
                terminal_failure_start: None,
            },
        ],
    )
}

#[test]
fn source_ssa_failure_tail_preserves_success_without_skipping_failure_uses() {
    let (plan, edges, blocks) = failure_fixture();
    assert!(matches!(
        run(&plan, &edges, 1_000_000, 1 << 20).0,
        Err(Error::Statement(_))
    ));
    let values = failure_run(&plan, &edges, &blocks, 1_000_000, 1 << 20)
        .0
        .unwrap();
    let expected = plan.entry_definitions()[0].value();
    assert_eq!(
        values,
        vec![
            (Block::new(0), Variable::new(0), expected),
            (Block::new(1), Variable::new(0), expected)
        ]
    );

    // Both original arms return. Adding an edge from a redefining arm into the
    // failure-only use must still conflict, even without any successful use.
    let a = Variable::new(0);
    let (plan, mut edges) = fixture(
        vec![
            block(vec![], &[1, 2]),
            block(vec![SsaEventV1::Use(a), SsaEventV1::Kill(a)], &[])
                .with_terminal_failure_start(0),
            block(vec![SsaEventV1::Define(a)], &[]),
        ],
        1,
    );
    let blocks = [
        SourceSsaBlockEventsV299 {
            events: 0,
            terminal_failure_start: None,
        },
        SourceSsaBlockEventsV299 {
            events: 2,
            terminal_failure_start: Some(0),
        },
        SourceSsaBlockEventsV299 {
            events: 1,
            terminal_failure_start: None,
        },
    ];
    failure_run(&plan, &edges, &blocks, 1_000_000, 1 << 20)
        .0
        .unwrap();
    edges[2].push(Block::new(1));
    assert!(matches!(
        failure_run(&plan, &edges, &blocks, 1_000_000, 1 << 20).0,
        Err(Error::Statement(_))
    ));
}

#[test]
fn source_ssa_failure_tail_rejects_incomplete_counts_cuts_and_prefix_definitions() {
    let (plan, edges, rows) = failure_fixture();
    let mut mutations = vec![rows[..1].to_vec(), vec![rows[0]; 3]];
    let mut wrong = rows.clone();
    wrong[0].terminal_failure_start = Some(4);
    mutations.push(wrong);
    let mut wrong = rows.clone();
    wrong[0].events += 1;
    mutations.push(wrong);
    let mut wrong = rows.clone();
    wrong[0].events -= 1;
    wrong[1].events += 1;
    mutations.push(wrong);
    let mut wrong = rows.clone();
    wrong[0].terminal_failure_start = None;
    mutations.push(wrong);
    for wrong in mutations {
        assert!(matches!(
            failure_run(&plan, &edges, &wrong, 1_000_000, 1 << 20).0,
            Err(Error::Statement(_))
        ));
    }
    let a = Variable::new(0);
    let (plan, edges) = fixture(vec![block(vec![SsaEventV1::Define(a)], &[])], 1);
    assert!(matches!(
        failure_run(
            &plan,
            &edges,
            &[SourceSsaBlockEventsV299 {
                events: 1,
                terminal_failure_start: Some(0)
            }],
            1_000_000,
            1 << 20
        )
        .0,
        Err(Error::Statement(_))
    ));
}

#[test]
fn source_ssa_failure_tail_keeps_empty_full_unpromoted_and_loop_boundaries() {
    let a = Variable::new(0);
    for cut in [None, Some(0), Some(1)] {
        let mut first = block(vec![SsaEventV1::Use(a)], &[1]);
        if let Some(start) = cut {
            first = first.with_terminal_failure_start(start);
        }
        let (plan, edges) = fixture(vec![first, block(vec![SsaEventV1::Use(a)], &[])], 1);
        let rows = [
            SourceSsaBlockEventsV299 {
                events: 1,
                terminal_failure_start: cut,
            },
            SourceSsaBlockEventsV299 {
                events: 1,
                terminal_failure_start: None,
            },
        ];
        assert_eq!(
            failure_run(&plan, &edges, &rows, 1_000_000, 1 << 20)
                .0
                .unwrap(),
            run(&plan, &edges, 1_000_000, 1 << 20).0.unwrap()
        );
    }
    let input = SsaConstructionInputV1::new(
        Block::new(0),
        2,
        vec![true, false],
        vec![a],
        vec![
            block(vec![], &[1]),
            block(
                vec![
                    SsaEventV1::Use(a),
                    SsaEventV1::Use(Variable::new(1)),
                    SsaEventV1::Kill(Variable::new(1)),
                    SsaEventV1::Kill(a),
                ],
                &[1, 2, 2],
            )
            .with_terminal_failure_start(1),
            block(vec![SsaEventV1::Use(a)], &[]),
        ],
    );
    let plan = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    let edges = [
        vec![Block::new(1)],
        vec![Block::new(1), Block::new(2), Block::new(2)],
        vec![],
    ];
    let rows = [
        SourceSsaBlockEventsV299 {
            events: 0,
            terminal_failure_start: None,
        },
        SourceSsaBlockEventsV299 {
            events: 4,
            terminal_failure_start: Some(1),
        },
        SourceSsaBlockEventsV299 {
            events: 1,
            terminal_failure_start: None,
        },
    ];
    let values = failure_run(&plan, &edges, &rows, 1_000_000, 1 << 20)
        .0
        .unwrap();
    assert_eq!(values.len(), 3);
    assert!(values.iter().all(
        |(_, variable, value)| *variable == a && *value == plan.entry_definitions()[0].value()
    ));
    assert_eq!(
        plan.resolved_events(Block::new(1))
            .unwrap()
            .iter()
            .map(|(at, _)| *at)
            .collect::<Vec<_>>(),
        vec![0, 3]
    );
}

#[test]
fn source_ssa_failure_tail_never_repairs_normal_moves_or_invalid_failure_uses() {
    use fe2o3_mir_model::SsaPlannerErrorV1;
    let a = Variable::new(0);
    for (events, cut, failure_block) in [
        (vec![SsaEventV1::Use(a), SsaEventV1::Kill(a)], 2, 1),
        (
            vec![SsaEventV1::Use(a), SsaEventV1::Kill(a), SsaEventV1::Use(a)],
            0,
            0,
        ),
    ] {
        let input = SsaConstructionInputV1::new(
            Block::new(0),
            1,
            vec![true],
            vec![a],
            vec![
                block(events, &[1]).with_terminal_failure_start(cut),
                block(vec![SsaEventV1::Use(a)], &[]),
            ],
        );
        // These genuinely undefined inputs refuse in the planner, before the
        // shared checker can receive an authenticated plan.
        assert!(
            matches!(plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()),
            Err(SsaPlannerErrorV1::UndefinedAtUse { block, variable, .. })
                if block == Block::new(failure_block) && variable == a)
        );
    }
}

#[test]
fn source_ssa_failure_tail_has_exact_resource_and_foreign_plan_boundaries() {
    let (plan, edges, rows) = failure_fixture();
    let measured = failure_run(&plan, &edges, &rows, 1_000_000, 1 << 20);
    measured.0.unwrap();
    let exact = failure_run(&plan, &edges, &rows, measured.1, measured.2);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (measured.1, measured.2));
    assert!(
        matches!(failure_run(&plan, &edges, &rows, measured.1 - 1, measured.2).0,
        Err(Error::Resource(Resource::Work(e))) if e.actual() == measured.1 && e.limit() == measured.1 - 1)
    );
    assert!(
        matches!(failure_run(&plan, &edges, &rows, measured.1, measured.2 - 1).0,
        Err(Error::Resource(Resource::Storage(e))) if e.actual() == measured.2 && e.limit() == measured.2 - 1)
    );
    let foreign = plan.clone();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1 << 20);
    budget.reserve_storage(23).unwrap();
    let (owner, receipt) = SourceSsaBoundariesV31::derive_with_terminal_failures_v299(
        &plan,
        Block::new(0),
        &edges,
        &rows,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        owner.value(&foreign, Block::new(1), Variable::new(0), &mut budget),
        Err(Error::ForeignPlan)
    ));
    assert_eq!(budget.storage(), floor);
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 23);
}

#[test]
fn source_ssa_failure_tail_keeps_each_redefined_variable_before_interleaved_moves() {
    for reversed in [false, true] {
        let a = Variable::new(u32::from(reversed));
        let b = Variable::new(u32::from(!reversed));
        let input = SsaConstructionInputV1::new(
            Block::new(0),
            2,
            vec![true; 2],
            vec![Variable::new(0), Variable::new(1)],
            vec![
                block(
                    vec![
                        SsaEventV1::Define(a),
                        SsaEventV1::Use(b),
                        SsaEventV1::Use(a),
                        SsaEventV1::Kill(a),
                        SsaEventV1::Use(b),
                        SsaEventV1::Kill(b),
                    ],
                    &[1],
                )
                .with_terminal_failure_start(2),
                block(vec![SsaEventV1::Use(b), SsaEventV1::Use(a)], &[]),
            ],
        );
        let plan = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
        let edges = [vec![Block::new(1)], vec![]];
        let rows = [
            SourceSsaBlockEventsV299 {
                events: 6,
                terminal_failure_start: Some(2),
            },
            SourceSsaBlockEventsV299 {
                events: 2,
                terminal_failure_start: None,
            },
        ];
        let Some(Event::Define {
            variable,
            value: a_new,
        }) = plan.resolved_event(Block::new(0), 0)
        else {
            panic!("original prefix definition");
        };
        assert_eq!(variable, a);
        let b_old = plan
            .entry_definitions()
            .iter()
            .find(|row| row.variable() == b)
            .unwrap()
            .value();
        assert_ne!(
            a_new,
            plan.entry_definitions()
                .iter()
                .find(|row| row.variable() == a)
                .unwrap()
                .value()
        );
        for (ordinal, variable, value) in [(2, a, a_new), (4, b, b_old)] {
            assert_eq!(
                plan.resolved_event(Block::new(0), ordinal),
                Some(Event::Use { variable, value })
            );
            assert_eq!(
                plan.resolved_event(Block::new(0), ordinal + 1),
                Some(Event::Kill {
                    variable,
                    previous: Some(value)
                })
            );
        }
        assert!(matches!(
            run(&plan, &edges, 1_000_000, 1 << 20).0,
            Err(Error::Statement(_))
        ));
        let values = failure_run(&plan, &edges, &rows, 1_000_000, 1 << 20)
            .0
            .unwrap();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&(Block::new(0), b, b_old)));
        assert!(values.contains(&(Block::new(1), a, a_new)));
        assert!(values.contains(&(Block::new(1), b, b_old)));
        assert_eq!(
            plan.resolved_event(Block::new(1), 0),
            Some(Event::Use {
                variable: b,
                value: b_old
            })
        );
        assert_eq!(
            plan.resolved_event(Block::new(1), 1),
            Some(Event::Use {
                variable: a,
                value: a_new
            })
        );
    }
}

#[test]
fn source_ssa_boundaries_apply_definitions_only_on_their_original_edge() {
    let variable = Variable::new(0);
    let input = SsaConstructionInputV1::new(
        Block::new(0),
        1,
        vec![true],
        vec![variable],
        vec![
            SsaBlockInputV1::new(
                vec![],
                vec![
                    SsaEdgeInputV1::new(SsaEdgeRoleV1::new(1), Block::new(1), vec![variable]),
                    SsaEdgeInputV1::new(SsaEdgeRoleV1::new(2), Block::new(1), vec![]),
                ],
            ),
            block(vec![SsaEventV1::Use(variable)], &[]),
        ],
    );
    let plan = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    let successors = vec![vec![Block::new(1), Block::new(1)], vec![]];
    let rows = run(&plan, &successors, 1_000_000, 1 << 20).0.unwrap();
    let fresh = plan.edge_definitions(Edge::new(Block::new(0), 0)).unwrap()[0].value();
    let invocation = plan.entry_definitions()[0].value();
    assert_ne!(fresh, invocation);
    assert_eq!(
        plan.edge_arguments(Edge::new(Block::new(0), 0)).unwrap()[0].value(),
        fresh
    );
    assert_eq!(
        plan.edge_arguments(Edge::new(Block::new(0), 1)).unwrap()[0].value(),
        invocation
    );
    assert!(rows.contains(&(
        Block::new(1),
        variable,
        Value::BlockArgument {
            block: Block::new(1),
            variable
        }
    )));
    let omitted = vec![vec![Block::new(1)], vec![]];
    assert!(matches!(
        run(&plan, &omitted, 1_000_000, 1 << 20).0,
        Err(Error::Statement(_))
    ));
    assert_eq!(rows, run(&plan, &successors, 1_000_000, 1 << 20).0.unwrap());
}

#[test]
fn source_ssa_boundaries_reject_a_different_owned_plan_at_query() {
    let (plan, successors) = diamond();
    let (other, _) = diamond();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1 << 20);
    let (owner, receipt) =
        SourceSsaBoundariesV31::derive(&plan, Block::new(0), &successors, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        owner.value(&other, Block::new(3), Variable::new(1), &mut budget),
        Err(Error::ForeignPlan)
    ));
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        owner
            .value(&plan, Block::new(3), Variable::new(1), &mut budget)
            .unwrap(),
        Value::BlockArgument {
            block: Block::new(3),
            variable: Variable::new(1)
        }
    );
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn source_ssa_boundary_header_oracles_name_owned_and_return_frames() {
    #[allow(dead_code)]
    struct EquationRow {
        block: Block,
        variable: Variable,
        parent: usize,
        rank: u32,
        value: Option<Value>,
    }
    #[allow(dead_code)]
    struct Rows {
        rows: Vec<EquationRow>,
    }
    #[allow(dead_code)]
    struct Owner<'a> {
        plan: &'a Plan,
        entry: Block,
        successors: &'a [Vec<Block>],
        checked: Rows,
    }
    #[allow(dead_code)]
    struct Receipt {
        retained: usize,
    }
    #[allow(dead_code)]
    struct Input<'a> {
        entry: Block,
        successors: &'a [Vec<Block>],
    }
    #[allow(dead_code)]
    struct DeriveFields<'a> {
        plan: &'a Plan,
        input: Input<'a>,
    }
    #[allow(dead_code)]
    struct QueryFields<'v, 's> {
        owner: &'v Owner<'s>,
        plan: &'v Plan,
        block: Block,
        variable: Variable,
    }
    type Scope<'a, 'w, T, C> = (
        C,
        (C, &'a mut Meter<'a, 'w>),
        std::panic::AssertUnwindSafe<(C, &'a mut Meter<'a, 'w>)>,
        std::thread::Result<Result<T>>,
        [Result<T>; 2],
        std::panic::AssertUnwindSafe<Result<T>>,
        std::thread::Result<()>,
        [Option<Box<dyn std::any::Any + Send>>; 2],
        Result<()>,
    );
    type Frame<'a, 'w> = (
        &'a Plan,
        Input<'a>,
        &'a mut Meter<'a, 'w>,
        Owner<'a>,
        Receipt,
        Result<(Owner<'a>, Receipt)>,
        Result<Rows>,
        usize,
    );
    assert_eq!(size_of::<EquationRow>(), size_of::<Row>());
    assert_eq!(size_of::<Rows>(), size_of::<Boundaries>());
    assert_eq!(
        size_of::<Owner<'_>>(),
        size_of::<SourceSsaBoundariesV31<'_>>()
    );
    assert_eq!(
        size_of::<Receipt>(),
        size_of::<SourceSsaBoundaryStorageV31>()
    );
    assert_eq!(
        size_of::<DeriveFields<'_>>(),
        size_of::<DeriveCapture<'_>>()
    );
    assert_eq!(
        std::mem::align_of::<DeriveFields<'_>>(),
        std::mem::align_of::<DeriveCapture<'_>>()
    );
    assert_eq!(
        size_of::<QueryFields<'_, '_>>(),
        size_of::<QueryCapture<'_, '_>>()
    );
    assert_eq!(
        std::mem::align_of::<QueryFields<'_, '_>>(),
        std::mem::align_of::<QueryCapture<'_, '_>>()
    );
    type DeriveScope<'a, 'w> = Scope<'a, 'w, (Owner<'a>, Receipt), DeriveFields<'a>>;
    #[allow(dead_code)]
    struct BlockEventFields {
        events: usize,
        terminal_failure_start: Option<usize>,
    }
    type FailureFields<'a> = (&'a Plan, Input<'a>, &'a [BlockEventFields]);
    type FailureScope<'a, 'w> = Scope<'a, 'w, (Owner<'a>, Receipt), FailureFields<'a>>;
    assert_eq!(
        size_of::<BlockEventFields>(),
        size_of::<SourceSsaBlockEventsV299>()
    );
    assert_eq!(
        size_of::<FailureFields<'_>>(),
        size_of::<FailureCaptureV299<'_, '_>>()
    );
    assert_eq!(
        std::mem::align_of::<FailureFields<'_>>(),
        std::mem::align_of::<FailureCaptureV299<'_, '_>>()
    );
    assert_eq!(
        boundary_failure_owner_headers_v299().unwrap(),
        size_of::<Frame<'_, '_>>()
            + std::mem::align_of::<Frame<'_, '_>>()
            + size_of::<DeriveScope<'_, '_>>()
            + std::mem::align_of::<DeriveScope<'_, '_>>()
            + size_of::<FailureScope<'_, '_>>()
            + std::mem::align_of::<FailureScope<'_, '_>>()
    );
    assert_eq!(
        boundary_owner_headers_v31().unwrap(),
        size_of::<Frame<'_, '_>>()
            + std::mem::align_of::<Frame<'_, '_>>()
            + size_of::<DeriveScope<'_, '_>>()
            + std::mem::align_of::<DeriveScope<'_, '_>>()
    );
    type Query<'a> = (
        &'a Owner<'a>,
        &'a Plan,
        Block,
        Variable,
        Result<Value>,
        Option<usize>,
        Option<Value>,
    );
    type QueryScope<'a, 'w> = Scope<'a, 'w, Value, QueryFields<'a, 'a>>;
    assert_eq!(
        boundary_query_headers_v31().unwrap(),
        size_of::<Query<'_>>()
            + size_of::<QueryScope<'_, '_>>()
            + std::mem::align_of::<QueryScope<'_, '_>>()
    );
    let expected = size_of::<Rows>()
        + size_of::<Result<Rows>>()
        + size_of::<Vec<Resolved>>()
        + size_of::<Vec<Exit>>()
        + size_of::<Input<'_>>()
        + size_of::<EquationRow>()
        + size_of::<Resolved>()
        + size_of::<Exit>()
        + size_of::<Current>()
        + size_of::<Option<Current>>()
        + size_of::<Option<&[SourceSsaBlockEventsV299]>>()
        + size_of::<SourceSsaBlockEventsV299>()
        + size_of::<std::slice::Iter<'_, SourceSsaBlockEventsV299>>()
        + 4 * size_of::<usize>()
        + 2 * size_of::<Option<usize>>()
        + size_of::<Option<Value>>()
        + size_of::<Result<()>>()
        + size_of::<&Plan>()
        + size_of::<&mut Meter<'_, '_>>()
        + size_of::<std::slice::Iter<'_, Vec<Block>>>()
        + size_of::<std::slice::Iter<'_, Block>>()
        + size_of::<std::slice::Iter<'_, (u32, Event)>>()
        + size_of::<std::slice::Iter<'_, SsaArgumentV1>>()
        + size_of::<Result<Vec<EquationRow>>>()
        + size_of::<Result<Vec<Resolved>>>()
        + size_of::<Result<Vec<Exit>>>()
        + size_of::<Result<(Vec<EquationRow>, usize)>>()
        + size_of::<Result<(Vec<Resolved>, usize)>>()
        + size_of::<Result<(Vec<Exit>, usize)>>();
    assert_eq!(headers(), expected);
}

fn diamond() -> (Plan, Vec<Vec<Block>>) {
    let a = Variable::new(0);
    let b = Variable::new(1);
    fixture(
        vec![
            block(vec![SsaEventV1::Use(a)], &[1, 2]),
            block(vec![SsaEventV1::Define(b)], &[3]),
            block(vec![SsaEventV1::Define(b)], &[3]),
            block(vec![SsaEventV1::Use(a), SsaEventV1::Use(b)], &[]),
        ],
        2,
    )
}

#[test]
fn original_mir_boundaries_keep_dominating_live_ins_distinct_from_phi_inputs() {
    let (plan, successors) = diamond();
    let rows = run(&plan, &successors, 1_000_000, 64 * 1024 * 1024)
        .0
        .unwrap();
    let a = Variable::new(0);
    let b = Variable::new(1);
    assert!(rows.contains(&(
        Block::new(3),
        a,
        Value::Definition(SsaDefinitionIdV1::new(0))
    )));
    assert!(rows.contains(&(
        Block::new(3),
        b,
        Value::BlockArgument {
            block: Block::new(3),
            variable: b
        }
    )));
    assert_eq!(plan.transport_variables(Block::new(3)).unwrap(), &[b]);
    for block in 0..4 {
        assert!(rows.contains(&(
            Block::new(block),
            a,
            Value::Definition(SsaDefinitionIdV1::new(0))
        )));
    }
}

#[test]
fn original_mir_boundaries_keep_entry_invocation_separate_from_loop_recurrence() {
    let variable = Variable::new(0);
    let (plan, successors) = fixture(
        vec![
            block(
                vec![SsaEventV1::Use(variable), SsaEventV1::Define(variable)],
                &[0, 1],
            ),
            block(vec![SsaEventV1::Use(variable)], &[]),
        ],
        1,
    );
    let rows = run(&plan, &successors, 1_000_000, 64 * 1024 * 1024)
        .0
        .unwrap();
    assert_eq!(
        plan.entry_arguments()[0].value(),
        Value::Definition(SsaDefinitionIdV1::new(0))
    );
    assert!(rows.contains(&(
        Block::new(0),
        variable,
        Value::BlockArgument {
            block: Block::new(0),
            variable
        }
    )));
    assert!(rows.contains(&(
        Block::new(1),
        variable,
        Value::Definition(SsaDefinitionIdV1::new(1))
    )));
    assert_ne!(
        plan.entry_arguments()[0].value(),
        plan.edge_arguments(Edge::new(Block::new(0), 0)).unwrap()[0].value()
    );
}

#[test]
fn original_mir_boundaries_preserve_parallel_edges_and_reject_incomplete_topology() {
    let variable = Variable::new(0);
    let (plan, successors) = fixture(
        vec![
            block(vec![], &[1, 1]),
            block(vec![SsaEventV1::Use(variable)], &[]),
        ],
        1,
    );
    let baseline = run(&plan, &successors, 1_000_000, 64 * 1024 * 1024)
        .0
        .unwrap();
    let mut missing = successors.clone();
    missing[0].pop();
    assert!(matches!(
        run(&plan, &missing, 1_000_000, 64 * 1024 * 1024).0,
        Err(Error::Statement(
            "original MIR SSA boundary equations differ"
        ))
    ));
    let mut extra = successors.clone();
    extra[0].push(Block::new(1));
    assert!(matches!(
        run(&plan, &extra, 1_000_000, 64 * 1024 * 1024).0,
        Err(Error::Statement(
            "original MIR SSA boundary equations differ"
        ))
    ));
    assert_eq!(
        baseline,
        run(&plan, &successors, 1_000_000, 64 * 1024 * 1024)
            .0
            .unwrap()
    );
}

#[test]
fn original_mir_boundaries_reject_conflicting_edge_and_use_names() {
    let (plan, mut successors) = diamond();
    // The target now exposes b before either predecessor has defined it.
    successors[0][0] = Block::new(3);
    assert!(matches!(
        run(&plan, &successors, 1_000_000, 64 * 1024 * 1024).0,
        Err(Error::Statement(
            "original MIR SSA boundary equations differ"
        ))
    ));
}

#[test]
fn original_mir_boundaries_do_not_invent_liveness_for_a_dead_value_kill() {
    let variable = Variable::new(0);
    let (plan, successors) = fixture(
        vec![
            block(vec![], &[1]),
            block(vec![SsaEventV1::Kill(variable)], &[]),
        ],
        1,
    );
    assert!(plan.live_in(Block::new(0)).unwrap().is_empty());
    assert!(plan.live_in(Block::new(1)).unwrap().is_empty());
    assert!(matches!(
        plan.resolved_events(Block::new(1)).unwrap()[0].1,
        Event::Kill {
            previous: Some(Value::Definition(_)),
            ..
        }
    ));
    assert!(
        run(&plan, &successors, 1_000_000, 64 * 1024 * 1024)
            .0
            .unwrap()
            .is_empty()
    );
}

#[test]
fn original_mir_boundaries_have_exact_and_one_short_work_and_storage() {
    let (plan, successors) = diamond();
    let measured = run(&plan, &successors, 1_000_000, 64 * 1024 * 1024);
    measured.0.unwrap();
    let exact = run(&plan, &successors, measured.1, measured.2);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (measured.1, measured.2));
    assert!(
        matches!(run(&plan, &successors, measured.1 - 1, measured.2).0,
        Err(Error::Resource(Resource::Work(error))) if error.actual() == measured.1 && error.limit() == measured.1 - 1)
    );
    assert!(
        matches!(run(&plan, &successors, measured.1, measured.2 - 1).0,
        Err(Error::Resource(Resource::Storage(error))) if error.actual() == measured.2 && error.limit() == measured.2 - 1)
    );
}

#[test]
fn original_mir_boundaries_use_bounded_indexes_for_long_pass_through_chains() {
    let mut previous = 0;
    for count in [64u32, 256, 1024] {
        let variable = Variable::new(0);
        let blocks = (0..count)
            .map(|at| {
                if at + 1 == count {
                    block(vec![SsaEventV1::Use(variable)], &[])
                } else {
                    block(vec![], &[at + 1])
                }
            })
            .collect();
        let (plan, successors) = fixture(blocks, 1);
        let first = run(&plan, &successors, 10_000_000, 64 * 1024 * 1024);
        let second = run(&plan, &successors, 10_000_000, 64 * 1024 * 1024);
        let rows = first.0.unwrap();
        assert_eq!(rows, second.0.unwrap());
        assert_eq!((first.1, first.2), (second.1, second.2));
        assert_eq!(rows.len(), count as usize);
        assert!(
            rows.iter()
                .all(|row| row.2 == Value::Definition(SsaDefinitionIdV1::new(0)))
        );
        assert!(first.1 <= count as usize * logarithm(count as usize) * 100);
        if previous != 0 {
            assert!(first.1 <= previous * 6);
        }
        previous = first.1;
    }
}
