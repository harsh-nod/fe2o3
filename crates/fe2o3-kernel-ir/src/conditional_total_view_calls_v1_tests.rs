//! Calls can be excluded by path premises, never by trusting a callee name.
use super::*;

fn guarded_call_fixture() -> Module {
    let mut module = input_length_guard_fixture();
    let mut helper = BasicBlock::new(BlockId(0));
    helper.terminator = Some(ret());
    module.functions.push(Function::internal_helper(
        "arbitrary_helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![helper],
    ));
    body(&mut module).blocks[4].operations.push(Operation::new(
        vec![],
        OperationKind::Call {
            callee: "arbitrary_helper".into(),
            arguments: vec![],
        },
    ));
    module
}

#[test]
fn input_premise_excludes_call_without_trusting_the_callee() {
    let module = guarded_call_fixture();
    let result = analyze(&module);
    let facts = result
        .facts()
        .expect("call is outside the readable input domain");
    assert_eq!(facts.read_count(), 1);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(37).unwrap();
    facts
        .visit_reads_v1(&mut budget, |read| {
            assert_eq!(read.parameter(), 3);
            assert_eq!(
                read.access_domain(),
                ConditionalTotalViewAddressDomainV1::GuardedOutput
            );
            assert_eq!(
                read.address_domain(),
                ConditionalTotalViewAddressDomainV1::GlobalLaunch
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn possible_calls_remain_unsupported_regardless_of_exit_or_name() {
    for change in 0..5 {
        let mut module = guarded_call_fixture();
        match change {
            0 => body(&mut module).blocks[1].terminator = Some(conditional(51, 50, 40)),
            1 => body(&mut module).blocks[1].terminator = Some(conditional(2, 40, 50)),
            2 => {
                let OperationKind::Compare { lhs, .. } =
                    &mut body(&mut module).blocks[1].operations[1].kind
                else {
                    panic!("compare");
                };
                *lhs = ValueId(13);
            }
            3 => {
                let call = body(&mut module).blocks[4].operations.pop().unwrap();
                body(&mut module).blocks[1].operations.insert(0, call);
            }
            _ => {
                let call = body(&mut module).blocks[4].operations.pop().unwrap();
                body(&mut module).blocks[2].operations.push(call);
            }
        }
        assert!(
            matches!(refused(&module), Unsupported::Call { .. }),
            "mutation {change}"
        );
    }
}

fn input_guard_before_output() -> Module {
    let mut module = guarded_call_fixture();
    let guard = std::mem::take(&mut body(&mut module).blocks[1].operations);
    body(&mut module).blocks[0].operations.extend(guard);
    body(&mut module).blocks[0].terminator = Some(conditional(51, 20, 50));
    body(&mut module).blocks[1].terminator = Some(conditional(12, 40, 30));
    module
}

#[test]
fn guarded_read_cannot_exclude_a_call_outside_the_output_domain() {
    let module = input_guard_before_output();
    // The input guard can fail when i >= output.len(), even for an empty output.
    assert!(matches!(refused(&module), Unsupported::Call { .. }));
}

#[test]
fn input_guard_does_not_shrink_a_read_before_the_output_guard() {
    let mut module = input_guard_before_output();
    let load = body(&mut module).blocks[3].operations.remove(0);
    body(&mut module).blocks[1].operations.push(load);
    let result = analyze(&module);
    let facts = result
        .facts()
        .expect("global input premise excludes the call");
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    facts
        .visit_reads_v1(&mut budget, |read| {
            assert_eq!(
                read.access_domain(),
                ConditionalTotalViewAddressDomainV1::GlobalLaunch
            );
            assert_eq!(
                read.address_domain(),
                ConditionalTotalViewAddressDomainV1::GlobalLaunch
            );
            Ok(())
        })
        .unwrap();
}

fn two_inputs_fixture() -> Module {
    let mut module = guarded_call_fixture();
    module.functions[0].signature.parameters.push(Type::slice(
        scalar(),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    ));
    body(&mut module).parameters.push(ValueId(4));
    let pointer = Type::pointer(scalar(), AddressSpace::Global, AccessMode::ReadOnly);
    let mut write = BasicBlock::new(BlockId(60));
    write.operations = vec![
        op(
            62,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(4) },
        ),
        op(
            63,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(62),
                offset: ValueId(10),
            },
        ),
        op(
            64,
            scalar(),
            OperationKind::Load {
                pointer: ValueId(63),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    let tail = body(&mut module).blocks[3].operations.split_off(1);
    write.operations.extend(tail);
    let OperationKind::Binary { rhs, .. } = &mut write.operations[3].kind else {
        panic!("sum");
    };
    *rhs = ValueId(64);
    write.terminator = Some(ret());
    body(&mut module).blocks[3].operations.extend([
        op(
            60,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(4) },
        ),
        op(
            61,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(10),
                rhs: ValueId(60),
            },
        ),
    ]);
    body(&mut module).blocks[3].terminator = Some(conditional(61, 60, 50));
    body(&mut module).blocks.push(write);
    module
}

#[test]
fn two_exact_input_premises_exclude_a_shared_failure_call() {
    let mut module = two_inputs_fixture();
    assert_eq!(analyze(&module).facts().unwrap().read_count(), 2);
    // An unrelated slice has no retained readable-domain premise.
    module.functions[0].signature.parameters.push(Type::slice(
        scalar(),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    ));
    body(&mut module).parameters.push(ValueId(5));
    let OperationKind::SliceLength { slice } = &mut body(&mut module).blocks[3].operations[1].kind
    else {
        panic!("length");
    };
    *slice = ValueId(5);
    assert!(matches!(refused(&module), Unsupported::Call { .. }));
}

#[test]
fn shared_failure_call_analysis_preserves_exact_resource_boundaries() {
    let module = two_inputs_fixture();
    let verified = verify_module_ref(&module).unwrap();
    let kernel = KernelId::new("kernel");
    let floor = 37;
    let prefix = 11;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(prefix).unwrap();
    assert!(
        derive_conditional_total_view_from_verified_v1(verified, &kernel, &mut budget)
            .unwrap()
            .facts()
            .is_some()
    );
    let used = budget.work();
    let peak = budget.peak_storage();
    assert_eq!(budget.storage(), floor);
    for (work_limit, storage_limit, success) in [
        (used, peak, true),
        (used - 1, peak, false),
        (used, peak - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(prefix).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let result = derive_conditional_total_view_from_verified_v1(verified, &kernel, &mut budget);
        if success {
            assert!(result.unwrap().facts().is_some());
            assert_eq!(budget.work(), used);
        } else {
            assert!(matches!(
                result,
                Err(ConditionalTotalViewErrorV1::Resource(_))
            ));
        }
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == identity);
        assert!(budget.work() >= prefix);
    }
}
