fn checked_output_guarded_owner_v375(
    module: &Module,
) -> fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work, VerifiedCanonicalKernelIrModuleV12 as Owner,
    };
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let (owner, _) = Owner::from_module_ref_with_verification_budget_v12(module, &mut budget)
        .expect("actual native output must independently verify");
    assert_eq!(budget.storage(), 0);
    owner
}

#[test]
fn checked_output_guarded_formal_uses_fresh_shifted_output_locations() {
    let mut fixture = generated_matrix_tail_fixture(2);
    let original = checked_output_guarded_owner_v375(&fixture.module);
    let historical = derive_checked_output_guarded_obligations_v1(&original, 1_000).unwrap();
    guarded_fixture_operations_mut(&mut fixture).insert(
        0,
        Operation::effect_free(
            ValueDef::new(ValueId(100), Type::INDEX),
            OperationKind::Constant(Constant::Index(19)),
        ),
    );
    let output = checked_output_guarded_owner_v375(&fixture.module);
    let fresh = derive_checked_output_guarded_obligations_v1(&output, 1_000).unwrap();
    assert_eq!(fresh.len(), 1);
    assert_eq!(fresh[0].accesses().len(), 2);
    assert_ne!(historical, fresh);
    for (before, after) in historical[0].accesses().iter().zip(fresh[0].accesses()) {
        assert_eq!(before.location().block, after.location().block);
        assert_eq!(
            before.location().operation_index + 1,
            after.location().operation_index
        );
    }
    assert_eq!(
        fresh,
        derive_checked_output_guarded_obligations_v1(&output, 1_000).unwrap()
    );
}

#[test]
fn checked_output_guarded_formal_refuses_changed_predicate_slice_and_fallback() {
    for case in 0..4 {
        let mut fixture = generated_matrix_tail_fixture(1);
        let operations = guarded_fixture_operations_mut(&mut fixture);
        match case {
            0 => {
                let always_true = operations[3].results[0].id;
                let OperationKind::Select { condition, .. } = &mut operations[9].kind else {
                    panic!("fixture select");
                };
                *condition = always_true;
                let OperationKind::GuardedLoad { predicate, .. } = &mut operations[11].kind else {
                    panic!("fixture load");
                };
                *predicate = always_true;
            }
            1 => {
                let OperationKind::SliceLength { slice } = &mut operations[1].kind else {
                    panic!("fixture slice length");
                };
                *slice = ValueId(1);
            }
            2 => {
                let OperationKind::Select { false_value, .. } = &mut operations[9].kind else {
                    panic!("fixture select");
                };
                *false_value = ValueId(2);
            }
            _ => {
                let OperationKind::Compare { predicate, .. } = &mut operations[7].kind else {
                    panic!("fixture compare");
                };
                *predicate = ComparePredicate::Equal;
            }
        }
        let output = checked_output_guarded_owner_v375(&fixture.module);
        assert!(matches!(
            derive_checked_output_guarded_obligations_v1(&output, 1_000),
            Err(crate::ProductionFormalMemoryErrorV1::GuardedAccessDischarge { .. })
        ));
    }
}

#[test]
fn checked_output_guarded_formal_does_not_discharge_other_incomplete_reasons() {
    let mut fixture = generated_matrix_tail_fixture(1);
    let operations = guarded_fixture_operations_mut(&mut fixture);
    let OperationKind::GuardedLoad {
        pointer, access, ..
    } = operations[11].kind.clone()
    else {
        panic!("fixture load");
    };
    operations[11].kind = OperationKind::Load { pointer, access };
    let output = checked_output_guarded_owner_v375(&fixture.module);
    let error = derive_checked_output_guarded_obligations_v1(&output, 1_000).unwrap_err();
    let crate::ProductionFormalMemoryErrorV1::Incomplete { reasons } = error else {
        panic!("ordinary unsupported index must not use guarded-load discharge: {error}");
    };
    assert!(reasons.iter().any(|reason| matches!(
        reason,
        FormalMemoryIncompleteReason::UnsupportedIndexExpression { .. }
    )));
}

#[test]
fn checked_output_guarded_formal_preserves_full_kernel_roster_and_bounds() {
    let mut fixture = generated_matrix_tail_fixture(1);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Return { values: vec![] });
    fixture.module.functions.insert(
        0,
        Function::kernel_entry("plain", Signature::new(vec![], vec![]), vec![], vec![entry]),
    );
    fixture.module.kernels.insert(
        0,
        Kernel::new(
            "plain",
            "plain",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ),
    );
    let output = checked_output_guarded_owner_v375(&fixture.module);
    let fresh = derive_checked_output_guarded_obligations_v1(&output, 1_000).unwrap();
    assert_eq!(fresh.len(), output.module().kernels.len());
    for (kernel, obligations) in output.module().kernels.iter().zip(&fresh) {
        assert_eq!(&kernel.id, obligations.kernel());
        assert_eq!(&kernel.entry, obligations.entry());
        assert!(obligations.inter_invocation_conflicts().is_empty());
    }
    assert!(fresh[0].accesses().is_empty());
    assert_eq!(fresh[1].accesses().len(), 1);
    let analysis = fe2o3_kernel_ir::derive_kernel_memory_obligations_for_launch(
        output.module(),
        &output.module().kernels[1].id,
        fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [64, 1, 1],
        },
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert_eq!(&fresh[1], analysis.obligations());
}

#[test]
fn checked_output_guarded_formal_rejects_empty_roster_and_proof_exhaustion() {
    let empty = checked_output_guarded_owner_v375(&Module::new("empty"));
    assert!(matches!(
        derive_checked_output_guarded_obligations_v1(&empty, 1_000),
        Err(crate::ProductionFormalMemoryErrorV1::KernelCount { actual: 0 })
    ));
    let fixture = generated_matrix_tail_fixture(1);
    let output = checked_output_guarded_owner_v375(&fixture.module);
    for limit in [0, 1, usize::MAX] {
        assert!(matches!(
            derive_checked_output_guarded_obligations_v1(&output, limit),
            Err(crate::ProductionFormalMemoryErrorV1::GuardedAccessDischarge { .. })
        ));
    }
}
