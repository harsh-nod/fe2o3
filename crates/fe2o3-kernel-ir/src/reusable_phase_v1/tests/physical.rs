use super::*;

fn project(m: &Module) -> Result<ReusablePhasePhysicalProjectionV1<'_>, ReusablePhaseCheckErrorV1> {
    let function = &m.functions[0];
    let cfg = analyze_control_flow(function).unwrap();
    project_reusable_phase_function_v1(function, &cfg, ReusablePhaseCheckLimitsV1::DEFAULT)
}

#[test]
fn phase_physical_projection_restores_one_actual_allocation_across_two_memory_phases() {
    let m = memory::memory_fixture(2);
    verify_module(&m).unwrap();
    let plan = project(&m).unwrap();
    assert!(std::ptr::eq(plan.function(), &m.functions[0]));
    assert_eq!(plan.results().len(), 27);
    assert_eq!(
        plan.results()
            .iter()
            .filter(|r| r.materializes_pointer())
            .count(),
        4
    );
    let storage = plan
        .results()
        .iter()
        .filter_map(|r| r.storage())
        .collect::<Vec<_>>();
    assert_eq!(storage.len(), 8);
    for origin in storage {
        assert_eq!(origin.allocation(), ValueId(80));
        assert_eq!(origin.element(), identity(82));
        assert_eq!(
            origin.layout(),
            ExecutionElementLayoutV1 {
                byte_size: 4,
                byte_alignment: 4
            }
        );
        assert_eq!(origin.elements(), 64);
    }
    for row in plan.results() {
        assert_eq!(row.owner(), ValueId(1));
        assert!(std::ptr::eq(
            row.operation(),
            &operations(&m)[row.operation_index()]
        ));
        assert!(
            row.operation()
                .results
                .iter()
                .any(|r| std::ptr::eq(r, row.result()))
        );
    }
    assert!(plan.retained_bytes() <= plan.usage().peak_temporary_bytes);
}

#[test]
fn phase_physical_projection_keeps_distinct_same_typed_allocations() {
    let (m, _) = fixture(2, 2);
    verify_module(&m).unwrap();
    let plan = project(&m).unwrap();
    let mut ends = 0;
    for group in plan
        .results()
        .chunk_by(|a, b| a.block() == b.block() && a.operation_index() == b.operation_index())
    {
        if !matches!(
            group[0].operation().kind,
            OperationKind::ReusablePhase(ReusablePhaseOpV1 {
                operation: ReusablePhaseOperationV1::End { storage_count: 2 },
                ..
            })
        ) {
            continue;
        }
        ends += 1;
        let allocations = group
            .iter()
            .filter_map(|r| r.storage().map(|s| s.allocation()))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            allocations,
            [ValueId(80), ValueId(82)].into_iter().collect()
        );
    }
    assert_eq!(ends, 2);
}

#[test]
fn phase_physical_projection_rejects_a_lookalike_lease_from_another_allocation() {
    let (mut m, t) = fixture(1, 2);
    let other = operations(&m)[t[0].binds[0]].results[1].id;
    p_mut(&mut m, t[0].closes[1]).operands[1] = other;
    let result = project(&m);
    assert!(
        matches!(result, Err(ReusablePhaseCheckErrorV1::WrongStorage)),
        "{result:?}"
    );
}

#[test]
fn phase_physical_projection_rejects_a_future_same_typed_lease() {
    let (mut m, t) = fixture(1, 2);
    let future = operations(&m)[t[0].binds[1]].results[1].id;
    p_mut(&mut m, t[0].closes[0]).operands[1] = future;
    let result = project(&m);
    assert!(
        matches!(result, Err(ReusablePhaseCheckErrorV1::NonDominatingUse)),
        "{result:?}"
    );
}

#[test]
fn phase_physical_projection_requires_end_before_exposing_origins() {
    let (mut m, t) = fixture(1, 1);
    operations_mut(&mut m).remove(t[0].end);
    assert!(matches!(
        project(&m),
        Err(ReusablePhaseCheckErrorV1::MissingEnd)
    ));
}

#[test]
fn phase_physical_projection_rejects_changed_original_allocation_layout() {
    let (mut m, _) = fixture(1, 1);
    let allocation = operations_mut(&mut m).iter_mut().find(|op| matches!(&op.kind,
        OperationKind::ExecutionCapability(e) if matches!(e.operation, ExecutionCapabilityOperationV1::LdsAllocate { .. }))).unwrap();
    let Type::ExecutionCapability(c) = &mut allocation.results[0].ty else {
        panic!()
    };
    let ExecutionCapabilityRoleV1::Lds { elements, .. } = &mut c.role else {
        panic!()
    };
    *elements += 1;
    assert!(matches!(
        project(&m),
        Err(ReusablePhaseCheckErrorV1::WrongStorage)
    ));
}

#[test]
fn phase_physical_projection_never_materializes_linear_loans_as_pointers() {
    let (m, _) = fixture(2, 1);
    let plan = project(&m).unwrap();
    let mut loans = 0;
    for result in plan.results() {
        if matches!(result.result().ty, Type::ReusablePhaseToken(_)) {
            loans += 1;
            assert!(!result.materializes_pointer());
        }
    }
    assert!(loans >= 8);
}

#[test]
fn phase_physical_projection_accounts_output_capacity_at_the_existing_limits() {
    let m = memory::memory_fixture(2);
    let f = &m.functions[0];
    let cfg = analyze_control_flow(f).unwrap();
    let usage = project(&m).unwrap().usage();
    let limits = ReusablePhaseCheckLimitsV1 {
        work: usage.work,
        temporary_bytes: usage.peak_temporary_bytes,
    };
    assert_eq!(
        project_reusable_phase_function_v1(f, &cfg, limits)
            .unwrap()
            .usage(),
        usage
    );
    assert!(matches!(
        project_reusable_phase_function_v1(
            f,
            &cfg,
            ReusablePhaseCheckLimitsV1 {
                work: usage.work - 1,
                ..limits
            }
        ),
        Err(ReusablePhaseCheckErrorV1::WorkLimit)
    ));
    assert!(matches!(
        project_reusable_phase_function_v1(
            f,
            &cfg,
            ReusablePhaseCheckLimitsV1 {
                temporary_bytes: usage.peak_temporary_bytes - 1,
                ..limits
            }
        ),
        Err(ReusablePhaseCheckErrorV1::StorageLimit)
    ));
}

#[test]
fn phase_physical_projection_does_not_change_canonical_bytes_or_insert_effects() {
    let m = memory::memory_fixture(2);
    let before = encode_module_v14(&m).unwrap();
    let _plan = project(&m).unwrap();
    assert_eq!(encode_module_v14(&m).unwrap(), before);
    assert_eq!(operations(&m).iter().filter(|op| matches!(&op.kind,
        OperationKind::ExecutionCapability(e) if matches!(e.operation, ExecutionCapabilityOperationV1::LdsAllocate { .. }))).count(), 1);
    assert_eq!(operations(&m).iter().filter(|op| matches!(&op.kind,
        OperationKind::ExecutionCapability(e) if matches!(e.operation,
            ExecutionCapabilityOperationV1::LdsPublish { .. } | ExecutionCapabilityOperationV1::WorkgroupBarrier { .. }))).count(), 4);
}

#[test]
fn phase_physical_projection_demands_exact_once_complete_consumer_roster() {
    let m = memory::memory_fixture(2);
    let mut plan = project(&m).unwrap();
    let sites = operations(&m)
        .iter()
        .enumerate()
        .filter_map(|(i, op)| matches!(op.kind, OperationKind::ReusablePhase(_)).then_some(i))
        .collect::<Vec<_>>();
    for site in &sites {
        let rows = plan.operation_results(BlockId(0), *site).unwrap();
        assert_eq!(rows[0].operation(), &operations(&m)[*site]);
    }
    plan.finish().unwrap();
    let mut plan = project(&m).unwrap();
    plan.operation_results(BlockId(0), sites[0]).unwrap();
    let result = plan.operation_results(BlockId(0), sites[0]);
    assert!(
        matches!(result, Err(ReusablePhaseCheckErrorV1::DuplicateConsumption)),
        "{result:?}"
    );
    assert_eq!(
        project(&m).unwrap().finish(),
        Err(ReusablePhaseCheckErrorV1::MissingProducer)
    );
}

#[test]
fn phase_physical_projection_consumer_work_does_not_restart_the_ceiling() {
    let m = memory::memory_fixture(1);
    let mut plan = project(&m).unwrap();
    let remaining = ReusablePhaseCheckLimitsV1::DEFAULT.work - plan.usage().work;
    plan.charge_adapter_work(remaining).unwrap();
    assert_eq!(
        plan.charge_adapter_work(1),
        Err(ReusablePhaseCheckErrorV1::WorkLimit)
    );
}
