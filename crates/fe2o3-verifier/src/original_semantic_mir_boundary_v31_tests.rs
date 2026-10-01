use super::super::{Resource, vector};
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::{
    SsaBlockInputV1, SsaConstructionInputV1, SsaDefinitionIdV1, SsaEdgeIdV1 as Edge,
    SsaEdgeInputV1, SsaEdgeRoleV1, SsaEventV1, SsaPlannerLimitsV1, SsaResolvedEventV1 as Event,
    plan_ssa_with_limits_v1,
};

fn logarithm(count: usize) -> usize {
    (usize::BITS - count.max(1).leading_zeros()) as usize + 1
}

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
    let result = (|| {
        budget.reserve_storage(super::super::super::super::SOURCE_LIMIT)?;
        let mut writer = Writer::new(&mut budget)?;
        let boundaries = Boundaries::derive(
            plan,
            ControlInput {
                entry: Block::new(0),
                successors,
            },
            &mut writer,
        )?;
        let count = (0..successors.len()).try_fold(0usize, |count, ordinal| {
            writer.budget.charge_work(1)?;
            let block = Block::new(u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?);
            count
                .checked_add(plan.live_in(block).map_or(0, <[_]>::len))
                .ok_or_else(|| Error::Resource(Resource::Arithmetic))
        })?;
        let mut values = vector(count, &mut writer)?;
        for ordinal in 0..successors.len() {
            writer.budget.charge_work(1)?;
            let block = Block::new(u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?);
            for &variable in plan.live_in(block).unwrap_or(&[]) {
                values.push((
                    block,
                    variable,
                    boundaries.value(block, variable, &mut writer)?,
                ));
            }
        }
        Ok(values)
    })();
    let work = budget.work();
    let peak = budget.peak_storage();
    budget.release_storage(budget.storage()).unwrap();
    assert_eq!(budget.storage(), 0);
    (result, work, peak)
}

#[test]
fn original_mir_shared_boundary_adapter_frames_have_independent_field_oracles() {
    #[allow(dead_code)]
    struct Fields<'a> {
        plan: &'a Plan,
        checked: Checked<'a>,
    }
    #[allow(dead_code)]
    struct Input<'a> {
        entry: Block,
        successors: &'a [Vec<Block>],
    }
    assert_eq!(size_of::<Fields<'_>>(), size_of::<Boundaries<'_>>());
    assert_eq!(
        std::mem::align_of::<Fields<'_>>(),
        std::mem::align_of::<Boundaries<'_>>()
    );
    assert_eq!(
        headers(),
        size_of::<Fields<'_>>()
            + size_of::<Result<Fields<'_>>>()
            + size_of::<Input<'_>>()
            + size_of::<Storage>()
            + size_of::<std::result::Result<(Checked<'_>, Storage), BoundaryError>>()
            + size_of::<std::result::Result<Value, BoundaryError>>()
            + size_of::<Result<Value>>()
            + size_of::<Result<()>>()
            + size_of::<&Plan>()
            + size_of::<&mut Writer<'_, '_>>()
            + size_of::<Block>()
            + size_of::<Variable>()
    );
    assert!(matches!(
        error(BoundaryError::Resource(Resource::Accounting)),
        Error::Resource(Resource::Accounting)
    ));
    assert!(matches!(
        error(BoundaryError::ForeignPlan),
        Error::Statement("original MIR SSA boundary has a foreign plan")
    ));
    assert!(matches!(
        error(BoundaryError::Panicked),
        Error::Statement("original MIR SSA boundary derivation panicked")
    ));
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
