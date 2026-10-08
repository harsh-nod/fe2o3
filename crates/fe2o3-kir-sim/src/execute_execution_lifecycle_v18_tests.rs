use super::*;

fn invocation() -> SimulationInvocationV1 {
    SimulationInvocationV1 {
        global: [7, 5, 1],
        workgroup: [1, 1, 0],
        local: [3, 1, 1],
        workgroup_size: [4, 4, 2],
        workgroup_count: [3, 2, 1],
        launch_extent: [12, 8, 2],
    }
}
fn issue() -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(10), Type::Execution(Role::Context)),
        OperationKind::Execution(Op::ContextIssue),
    )
}
fn derive(id: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::Execution(Role::Workgroup)),
        OperationKind::Execution(Op::WorkgroupDerive {
            context: ValueId(10),
        }),
    )
}
fn end(id: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Execution(Op::ScopeEnd {
            workgroup: ValueId(id),
            discarded: vec![],
        }),
    )
}
fn apply(values: &mut RuntimeValues<'_>, op: &Operation) -> Result<(), Failure> {
    transition(values, op, invocation(), 3, FunctionRole::KernelEntry, 8)
}
fn issued() -> RuntimeValues<'static> {
    let mut values = RuntimeValues::with_capacity(8);
    apply(&mut values, &issue()).unwrap();
    values
}

#[test]
fn lifecycle_tokens_reuse_bounded_cells_with_distinct_generations() {
    let mut values = issued();
    let capacity = values.capacity();
    for generation in 1..=256 {
        apply(&mut values, &derive(11)).unwrap();
        assert_eq!(values.len(), 2);
        let group = token(&values, ValueId(11), invocation(), 3).unwrap();
        assert_eq!(group.generation, generation);
        assert_eq!(group.invocation, invocation());
        assert_eq!(
            runtime_type(values.get_ref(&ValueId(11)).unwrap()),
            Type::Execution(Role::Workgroup)
        );
        apply(&mut values, &end(11)).unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values.capacity(), capacity);
    }
    assert!(std::mem::size_of::<RuntimeValue>() <= 160);
    assert!(v12_observation_probe_cells_fit());
}

#[test]
fn lifecycle_tokens_reject_foreign_owner_geometry_and_forged_role() {
    let issued = issued();
    for index in 0..8 {
        let mut values = issued.clone();
        let RuntimeValue::Execution(value) = values.get_mut(&ValueId(10)).unwrap() else {
            panic!()
        };
        match index {
            0 => value.function += 1,
            1 => value.invocation.global[0] += 1,
            2 => value.invocation.workgroup[0] += 1,
            3 => value.invocation.local[0] += 1,
            4 => value.invocation.workgroup_size[0] += 1,
            5 => value.invocation.workgroup_count[0] += 1,
            6 => value.invocation.launch_extent[0] += 1,
            7 => value.producer = ValueId(9),
            _ => unreachable!(),
        }
        let before = values.clone();
        assert_eq!(apply(&mut values, &derive(11)), Err(Failure::Invalid));
        assert_eq!(values, before);
    }
    let mut forged = issued.clone();
    forged.insert(ValueId(10), RuntimeValue::Scalar(ScalarBitsV1::u32(10)));
    assert_eq!(apply(&mut forged, &derive(11)), Err(Failure::Invalid));
    let mut values = issued.clone();
    assert_eq!(
        transition(
            &mut values,
            &derive(11),
            invocation(),
            3,
            FunctionRole::InternalHelper,
            8
        ),
        Err(Failure::Invalid)
    );
    assert_eq!(values, issued);
}

#[test]
fn lifecycle_rejects_stale_wrong_scope_double_end_and_nested_borrow() {
    let mut values = issued();
    apply(&mut values, &derive(11)).unwrap();
    let stale = values.get(&ValueId(11)).unwrap().clone();
    let active = values.clone();
    assert_eq!(apply(&mut values, &derive(12)), Err(Failure::Invalid));
    assert_eq!(apply(&mut values, &end(12)), Err(Failure::Invalid));
    assert_eq!(values, active);
    apply(&mut values, &end(11)).unwrap();
    assert_eq!(apply(&mut values, &end(11)), Err(Failure::Invalid));
    apply(&mut values, &derive(11)).unwrap();
    values.insert(ValueId(11), stale);
    let before = values.clone();
    assert_eq!(apply(&mut values, &end(11)), Err(Failure::Invalid));
    assert_eq!(values, before);
}

#[test]
fn lifecycle_limit_and_overflow_fail_before_mutation() {
    let mut values = issued();
    let before = values.clone();
    assert_eq!(
        transition(
            &mut values,
            &derive(11),
            invocation(),
            3,
            FunctionRole::KernelEntry,
            1
        ),
        Err(Failure::SsaLimit)
    );
    assert_eq!(values, before);
    assert_eq!(apply(&mut values, &issue()), Err(Failure::Invalid));
    let RuntimeValue::Execution(value) = values.get_mut(&ValueId(10)).unwrap() else {
        panic!()
    };
    value.generation = u64::MAX;
    let before = values.clone();
    assert_eq!(apply(&mut values, &derive(11)), Err(Failure::Invalid));
    assert_eq!(values, before);
    let mut malformed = derive(11);
    malformed.results[0].ty = Type::Scalar(ScalarType::U32);
    assert!(!supports_operation(&malformed));
    assert_eq!(apply(&mut values, &malformed), Err(Failure::Invalid));
    let mut discarded_operation = end(11);
    let OperationKind::Execution(Op::ScopeEnd { discarded, .. }) = &mut discarded_operation.kind
    else {
        panic!()
    };
    discarded.push(ValueId(12));
    assert!(!supports_operation(&discarded_operation));
    assert_eq!(
        apply(&mut values, &discarded_operation),
        Err(Failure::Invalid)
    );
}

#[test]
fn lifecycle_debug_does_not_fabricate_legacy_scalar_values() {
    let mut values = issued();
    apply(&mut values, &derive(11)).unwrap();
    for id in [10, 11] {
        assert_eq!(debug_value(values.get_ref(&ValueId(id)).unwrap()), None);
    }
}
