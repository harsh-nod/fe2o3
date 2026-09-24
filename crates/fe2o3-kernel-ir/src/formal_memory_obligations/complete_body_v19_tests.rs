//! Synthetic immutable-owner controls. No source or runtime binding evidence.
use super::*;
use crate::*;
fn fixture(diamond: bool) -> Module {
    let mut module = if diamond {
        crate::gfx942_complete_body_profile_v19::tests::diamond()
    } else {
        crate::gfx942_complete_body_profile_v19::tests::single()
    };
    let capabilities = module.functions[0].derived_capabilities();
    module.required_capabilities = capabilities.clone();
    module.functions[0].required_capabilities = capabilities.clone();
    module.kernels[0].required_capabilities = capabilities;
    module
}
fn owner(module: &Module) -> VerifiedCanonicalKernelIrModuleV19 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(8_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 8_000_000);
    budget.reserve_storage(73).unwrap();
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV19::from_module_ref_with_verification_budget_v19(
            module,
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), 73);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    owner
}
fn launch(count: u64) -> ExplicitLaunchExtent {
    ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [count, 1, 1],
    }
}
fn analyze(owner: &VerifiedCanonicalKernelIrModuleV19) -> FormalMemoryObligationAnalysis {
    derive_complete_body_memory_obligations_v19(
        owner,
        &owner.module().kernels[0].id,
        launch(128),
        FormalIndexWidth::Bits64,
    )
    .unwrap()
}
#[test]
fn single_and_diamond_preserve_conservative_tail_bounds_and_disjointness() {
    for diamond in [false, true] {
        let owner = owner(&fixture(diamond));
        let analysis = analyze(&owner);
        assert!(analysis.is_complete(), "{analysis:?}");
        let obligations = analysis.obligations();
        assert_eq!(
            obligations.analysis_basis(),
            FormalMemoryAnalysisBasis::CompilerDerivedIrWithUnauthenticatedLaunchInputs
        );
        assert_eq!(obligations.allocations().len(), 1);
        let [access] = obligations.accesses() else {
            panic!("one actual store");
        };
        assert_eq!(access.kind(), FormalMemoryAccessKind::Write);
        assert_eq!(
            access.location(),
            FunctionOperationLocation::new(
                BlockId(if diamond { 3 } else { 0 }),
                if diamond { 5 } else { 7 }
            )
        );
        assert_eq!(
            access.byte_offset(),
            ByteExpression::invocation_affine(0, 4)
        );
        assert_eq!(access.byte_width(), 4);
        assert_eq!(access.alignment(), 4);
        assert_eq!(
            access.invocations(),
            InvocationRange1d::from_count(128).unwrap()
        );
        // The exact direct-index guard is preserved in the executable, but the
        // existing memory analyzer conservatively requires the full launch span.
        // No Select node or conditional-domain fact is invented here.
        assert_eq!(access.domain(), FormalAccessDomainV1::LaunchEnvelope);
        assert_eq!(access.allocation().parameter_index(), 0);
        let [bounds] = obligations.bounds_requirements() else {
            panic!("runtime bounds obligation");
        };
        assert_eq!(bounds.kind(), FormalBoundsKindV1::FixedMinimumBytes(512));
        assert_eq!(bounds.minimum_byte_len(), Some(512));
        assert!(obligations.runtime_alias_requirements().is_empty());
        assert!(obligations.inter_invocation_conflicts().is_empty());
        for bytes in [0, 4, 252, 256, 260, 508, 511] {
            assert!(!bounds.is_met_by_untrusted_byte_len(bytes));
        }
        for bytes in [512, 516] {
            assert!(bounds.is_met_by_untrusted_byte_len(bytes));
        }
    }
}
#[test]
fn every_existing_generic_entry_keeps_new_operations_unmodeled() {
    for diamond in [false, true] {
        let owner = owner(&fixture(diamond));
        let module = owner.module();
        let kernel = &module.kernels[0].id;
        let typed = analyze(&owner);
        let ordinary = [
            derive_kernel_memory_obligations(
                module,
                kernel,
                ExplicitLaunchExtent1d::Exact(128),
                FormalIndexWidth::Bits64,
            ),
            derive_kernel_memory_obligations_for_launch(
                module,
                kernel,
                launch(128),
                FormalIndexWidth::Bits64,
            ),
            derive_kernel_memory_obligations_from_verified(
                owner.verified_module_ref_v1(),
                kernel,
                ExplicitLaunchExtent1d::Exact(128),
                FormalIndexWidth::Bits64,
            ),
            derive_kernel_memory_obligations_from_verified_for_launch(
                owner.verified_module_ref_v1(),
                kernel,
                launch(128),
                FormalIndexWidth::Bits64,
            ),
        ];
        for ordinary in ordinary {
            let ordinary = ordinary.unwrap();
            assert!(!ordinary.is_complete());
            assert_eq!(ordinary.obligations(), typed.obligations());
            assert_eq!(
                ordinary.incomplete_reasons().len(),
                if diamond { 4 } else { 2 }
            );
            assert!(ordinary.incomplete_reasons().iter().all(|reason| matches!(
                reason,
                FormalMemoryIncompleteReason::UnsupportedMemoryEffect { .. }
            )));
        }
    }
}
#[test]
fn arithmetic_values_do_not_become_address_facts_or_change_ordering() {
    for opcode in [
        Gfx942ProgramBinaryOpcodeV1::Add,
        Gfx942ProgramBinaryOpcodeV1::Subtract,
        Gfx942ProgramBinaryOpcodeV1::And,
        Gfx942ProgramBinaryOpcodeV1::Or,
        Gfx942ProgramBinaryOpcodeV1::Xor,
    ] {
        let mut module = fixture(false);
        let operation = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[1];
        let OperationKind::Gfx942CompleteBodyStep(step) = &mut operation.kind else {
            panic!("step");
        };
        step.instruction = Gfx942ProgramInstructionV1::Binary {
            opcode,
            destination: Gfx942ProgramDestinationV1::Output,
            left: Gfx942ProgramRoleV1::Input0,
            right: Gfx942ProgramRoleV1::Input1,
        };
        step.operands = [Some(ValueId(1)), Some(ValueId(2))];
        let owner = owner(&module);
        let before = owner.canonical_bytes().to_vec();
        assert!(analyze(&owner).is_complete());
        assert_eq!(owner.canonical_bytes(), before);
        // The formal pass neither mutates nor rewrites either ordered operation.
        assert_eq!(owner.module(), &module);
        for operation in
            &owner.module().functions[0].body.as_ref().unwrap().blocks[0].operations[..2]
        {
            assert_eq!(
                operation.compiler_ordering_effects_v12(),
                CompilerOrderingEffectSummaryV12::ordered_region()
            );
        }
    }
}
#[test]
fn malformed_step_guard_address_or_store_never_obtains_typed_owner() {
    for case in 0..6 {
        let mut module = fixture(false);
        let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
        match case {
            0 => {
                if let OperationKind::Gfx942CompleteBodyStep(step) = &mut block.operations[1].kind {
                    step.operands[0] = Some(ValueId(2));
                }
            }
            1 => block.operations[1].results[0].ty = Type::INDEX,
            2 => {
                if let OperationKind::Compare { predicate, .. } = &mut block.operations[4].kind {
                    *predicate = ComparePredicate::Equal;
                }
            }
            3 => {
                if let OperationKind::GetElementPointer { offset, .. } =
                    &mut block.operations[6].kind
                {
                    *offset = ValueId(7);
                }
            }
            4 => {
                if let OperationKind::GuardedStore { predicate, .. } = &mut block.operations[7].kind
                {
                    *predicate = ValueId(1);
                }
            }
            5 => {
                if let OperationKind::GuardedStore { access, .. } = &mut block.operations[7].kind {
                    access.volatile = true;
                }
            }
            _ => unreachable!(),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(8_000_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 8_000_000);
        budget.reserve_storage(73).unwrap();
        assert!(
            VerifiedCanonicalKernelIrModuleV19::from_module_ref_with_verification_budget_v19(
                &module,
                &mut budget
            )
            .is_err(),
            "case {case}"
        );
        assert_eq!(budget.storage(), 73);
    }
}
#[test]
fn equal_but_foreign_module_cannot_borrow_another_owners_memory_context() {
    let first = owner(&fixture(false));
    let second = owner(&fixture(false));
    assert_eq!(first, second);
    let result = super::super::derive_kernel_memory_obligations_with_v19_context(
        second.verified_module_ref_v1(),
        &second.module().kernels[0].id,
        launch(128),
        FormalIndexWidth::Bits64,
        Some(&first),
    )
    .unwrap();
    assert!(!result.is_complete());
    assert_eq!(result.incomplete_reasons().len(), 2);
}
#[test]
fn launch_and_index_inputs_remain_unauthenticated_fail_closed_inputs() {
    let owner = owner(&fixture(true));
    let kernel = &owner.module().kernels[0].id;
    for (launch, width) in [
        (ExplicitLaunchExtent::Unknown, FormalIndexWidth::Bits64),
        (launch(0), FormalIndexWidth::Bits64),
        (launch(128), FormalIndexWidth::Bits32),
        (launch(128), FormalIndexWidth::Unknown),
        (
            ExplicitLaunchExtent::Exact {
                rank: 2,
                extents: [64, 2, 1],
            },
            FormalIndexWidth::Bits64,
        ),
    ] {
        let result =
            derive_complete_body_memory_obligations_v19(&owner, kernel, launch, width).unwrap();
        assert!(!result.is_complete());
        assert!(!result.incomplete_reasons().is_empty());
    }
}
#[test]
fn missing_kernel_and_ordinary_v19_subjects_do_not_gain_profile_membership() {
    let owner = owner(&fixture(false));
    assert!(matches!(
        derive_complete_body_memory_obligations_v19(
            &owner,
            &KernelId::new("missing"),
            launch(128),
            FormalIndexWidth::Bits64
        ),
        Err(FormalMemoryObligationError::MissingKernel { .. })
    ));
    let empty = owner_from_empty();
    assert!(!contains_verified_complete_body(
        &empty,
        &KernelId::new("missing")
    ));
    assert!(matches!(
        derive_complete_body_memory_obligations_v19(
            &empty,
            &KernelId::new("missing"),
            launch(128),
            FormalIndexWidth::Bits64
        ),
        Err(FormalMemoryObligationError::MissingKernel { .. })
    ));
}
fn owner_from_empty() -> VerifiedCanonicalKernelIrModuleV19 {
    owner(&Module::new("ordinary_v19"))
}

#[test]
fn ordinary_v19_kernel_uses_generic_memory_rules_without_claiming_body_membership() {
    let mut module = Module::new("ordinary_v19");
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::kernel_entry(
        "ordinary",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    module.kernels.push(Kernel::new(
        "ordinary_kernel",
        function.id.clone(),
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module.functions.push(function);
    let owner = owner(&module);
    let kernel = &owner.module().kernels[0].id;
    assert!(!contains_verified_complete_body(&owner, kernel));
    let typed = derive_complete_body_memory_obligations_v19(
        &owner,
        kernel,
        launch(128),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    let ordinary = derive_kernel_memory_obligations_from_verified_for_launch(
        owner.verified_module_ref_v1(),
        kernel,
        launch(128),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert_eq!(typed, ordinary);
    assert!(typed.is_complete());
    assert!(typed.obligations().accesses().is_empty());
}
