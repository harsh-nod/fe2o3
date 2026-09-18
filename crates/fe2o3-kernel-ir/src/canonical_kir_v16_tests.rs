use super::*;
use crate::*;

fn fixture() -> Module {
    let region = Gfx942OrderedRegionV1::new(
        AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
        Gfx942OrderedRegionRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
        [ValueId(0), ValueId(1), ValueId(2)],
    )
    .unwrap();
    let mut function = Function::internal_helper(
        "region",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32); 3],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![BasicBlock {
            id: BlockId(0),
            parameters: vec![],
            operations: vec![Operation::new(
                vec![ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32))],
                OperationKind::Gfx942OrderedRegion(region),
            )],
            terminator: Some(Terminator::Return {
                values: vec![ValueId(3)],
            }),
        }],
    );
    let capabilities =
        function.body.as_ref().unwrap().blocks[0].operations[0].required_capabilities();
    function.required_capabilities = capabilities.clone();
    let mut module = Module::new("ordered-region-v16");
    module.required_capabilities = capabilities;
    module.functions.push(function);
    module
}

fn admit(module: &Module) -> VerifiedCanonicalKernelIrModuleV16 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    VerifiedCanonicalKernelIrModuleV16::from_module_ref_with_verification_budget_v16(
        module,
        &mut budget,
    )
    .unwrap()
    .0
}

#[test]
fn owner_retains_one_fresh_inverse_with_exact_bytes_identity_and_storage_transfer() {
    let source = fixture();
    let bytes = encode_module_v16(&source).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    budget.reserve_storage(7).unwrap();
    let (from_module, module_receipt) =
        VerifiedCanonicalKernelIrModuleV16::from_module_ref_with_verification_budget_v16(
            &source,
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), 7);
    budget
        .reserve_storage(module_receipt.retained_storage())
        .unwrap();
    let (from_bytes, bytes_receipt) =
        VerifiedCanonicalKernelIrModuleV16::from_canonical_bytes_with_verification_budget_v16(
            &bytes,
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), 7 + module_receipt.retained_storage());
    budget
        .reserve_storage(bytes_receipt.retained_storage())
        .unwrap();
    assert_eq!(from_module, from_bytes);
    assert_eq!(module_receipt, bytes_receipt);
    assert_eq!(from_module.module(), &source);
    assert_ne!(
        from_module.module().functions.as_ptr(),
        source.functions.as_ptr()
    );
    assert_ne!(from_bytes.canonical_bytes().as_ptr(), bytes.as_ptr());
    assert_eq!(from_module.canonical_bytes(), bytes);
    assert_eq!(
        from_module.identity().canonical_length(),
        bytes.len() as u64
    );
    let mut hash = Sha256::new();
    hash.update(39_u32.to_le_bytes());
    hash.update(b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V16\0");
    hash.update(1_u16.to_le_bytes());
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(&bytes);
    assert_eq!(
        from_module.identity().digest(),
        &<[u8; 32]>::from(hash.finalize())
    );
    drop(source);
    drop(bytes);
    assert_eq!(from_module.module().functions.len(), 1);
    drop(from_module);
    budget
        .release_storage(module_receipt.retained_storage())
        .unwrap();
    drop(from_bytes);
    budget
        .release_storage(bytes_receipt.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 7);
}

#[test]
fn independent_empty_owner_work_and_storage_boundaries_match_v12_accounting() {
    let module = Module::new("m");
    let bytes = encode_module_v16(&module).unwrap();
    assert_eq!(bytes.len(), 37);
    let retained = std::mem::size_of::<VerifiedCanonicalKernelIrModuleV16>() + 37 + 1;
    // Exact same fixed-width domain length and schema work as V12: module294,
    // bytes233. This is independently literal, not obtained from a success probe.
    for (from_bytes, exact_work) in [(false, 294), (true, 233)] {
        for case in 0..3 {
            let mut work =
                CanonicalKernelIrWorkBudgetV1::new(11 + exact_work - usize::from(case == 1));
            work.charge_work(11).unwrap();
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
                &mut work,
                7 + retained - usize::from(case == 2),
            );
            budget.reserve_storage(7).unwrap();
            let result = if from_bytes {
                VerifiedCanonicalKernelIrModuleV16::from_canonical_bytes_with_verification_budget_v16(&bytes, &mut budget)
            } else {
                VerifiedCanonicalKernelIrModuleV16::from_module_ref_with_verification_budget_v16(
                    &module,
                    &mut budget,
                )
            };
            assert_eq!(
                result.is_ok(),
                case == 0,
                "bytes={from_bytes} case={case}: {result:?}"
            );
            assert_eq!(budget.storage(), 7);
            if case == 0 {
                assert_eq!(budget.work(), 11 + exact_work);
                assert_eq!(budget.peak_storage(), 7 + retained);
                assert_eq!(result.unwrap().1.retained_storage(), retained);
            }
        }
    }
}

#[test]
fn region_owner_budget_is_cumulative_and_exact_one_short_limits_restore_floor() {
    let source = fixture();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    budget.reserve_storage(7).unwrap();
    let baseline =
        VerifiedCanonicalKernelIrModuleV16::from_module_ref_with_verification_budget_v16(
            &source,
            &mut budget,
        )
        .unwrap();
    let exact_work = budget.work();
    let exact_peak = budget.peak_storage();
    drop(baseline);
    for case in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(11 + exact_work - usize::from(case == 1));
        work.charge_work(11).unwrap();
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            exact_peak - usize::from(case == 2),
        );
        budget.reserve_storage(7).unwrap();
        let result =
            VerifiedCanonicalKernelIrModuleV16::from_module_ref_with_verification_budget_v16(
                &source,
                &mut budget,
            );
        assert_eq!(result.is_ok(), case == 0);
        assert_eq!(budget.storage(), 7);
        if case == 0 {
            assert_eq!(budget.work(), 11 + exact_work);
            assert!(
                VerifiedCanonicalKernelIrModuleV16::from_module_ref_with_verification_budget_v16(
                    &source,
                    &mut budget
                )
                .is_err()
            );
            assert_eq!(budget.storage(), 7);
        }
    }
}

#[test]
fn v16_never_relabels_old_owners_or_admits_execution_grammar() {
    let region = fixture();
    assert!(encode_module_v12(&region).is_err());
    assert!(encode_module_v15(&region).is_err());
    let bytes = encode_module_v16(&region).unwrap();
    assert!(decode_module_v12(&bytes).is_err());
    assert!(decode_module_v15(&bytes).is_err());
    let mut execution = Module::new("execution");
    execution.functions.push(Function::declaration(
        "external",
        Signature::new(vec![Type::Execution(ExecutionRoleV15::Context)], vec![]),
    ));
    assert!(encode_module_v16(&execution).is_err());
    let mut disguised = encode_module_v15(&execution).unwrap();
    disguised[8..10].copy_from_slice(&16_u16.to_le_bytes());
    assert!(decode_module_v16(&disguised).is_err());
    for version in [1_u16, 11, 12, 13, 14, 15, 17] {
        let mut changed = bytes.clone();
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        assert!(decode_module_v16(&changed).is_err());
    }
    let plain = Module::new("m");
    let old = VerifiedCanonicalKernelIrV12::from_module(plain.clone()).unwrap();
    let new = admit(&plain);
    assert_ne!(old.identity().digest(), new.identity().digest());
}

#[test]
fn semantic_definition_input_and_result_failures_are_not_wire_admission() {
    for case in 0..4 {
        let mut module = fixture();
        let operation = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0];
        match case {
            0 => operation.results[0].ty = Type::Scalar(ScalarType::I32),
            1 => operation.results[0].id = ValueId(0),
            2 => {
                let OperationKind::Gfx942OrderedRegion(region) = operation.kind else {
                    unreachable!()
                };
                operation.kind = OperationKind::Gfx942OrderedRegion(
                    Gfx942OrderedRegionV1::new(
                        region.source(),
                        region.registers(),
                        [ValueId(0), ValueId(1), ValueId(999)],
                    )
                    .unwrap(),
                );
            }
            _ => {
                module.functions[0].signature.parameters[0] = Type::Scalar(ScalarType::I32);
            }
        }
        let bytes = encode_module_v16(&module).unwrap();
        assert!(decode_module_v16(&bytes).is_ok());
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
        budget.reserve_storage(7).unwrap();
        let result =
            VerifiedCanonicalKernelIrModuleV16::from_canonical_bytes_with_verification_budget_v16(
                &bytes,
                &mut budget,
            );
        assert!(
            matches!(
                result,
                Err(CanonicalKernelIrReplayAdmissionErrorV16::Verification(_))
            ),
            "case={case}: {result:?}"
        );
        assert_eq!(budget.storage(), 7);
    }
}

#[test]
fn target_capability_is_derived_without_treating_owner_or_declaration_as_target_authority() {
    let mut module = fixture();
    let supported = module.required_capabilities.clone();
    module.required_capabilities.clear();
    module.functions[0].required_capabilities.clear();
    // Ordinary canonical shape admission is target independent; it does not
    // authenticate a declared capability or pretend one was supplied by a target.
    assert_eq!(admit(&module).module(), &module);
    assert!(verify_module_with_capabilities(&module, &Default::default()).is_err());
    assert!(verify_module_with_capabilities(&module, &supported).is_ok());
    let operation = &module.functions[0].body.as_ref().unwrap().blocks[0].operations[0];
    assert_eq!(operation.required_capabilities(), supported);
}

#[test]
fn rejection_preserves_existing_failure_history_and_exact_input() {
    let bytes = encode_module_v16(&fixture()).unwrap();
    let mut trailing = bytes.clone();
    trailing.push(0);
    let invalid = encode_module_v16(&Module::new("")).unwrap();
    let old = encode_module_v12(&Module::new("m")).unwrap();
    for input in [
        &bytes[..bytes.len() - 1],
        trailing.as_slice(),
        invalid.as_slice(),
        old.as_slice(),
    ] {
        let snapshot = input.to_vec();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
        budget.reserve_storage(7).unwrap();
        assert!(budget.charge_work(usize::MAX).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let failed_storage = budget.failed_storage();
        assert!(
            VerifiedCanonicalKernelIrModuleV16::from_canonical_bytes_with_verification_budget_v16(
                input,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 7);
        assert_eq!(budget.failed_storage(), failed_storage);
        assert_eq!(input, snapshot);
    }
}

#[test]
fn region_effect_is_independent_nonpure_and_union_preserves_all_three_families() {
    let module = fixture();
    let operation = &module.functions[0].body.as_ref().unwrap().blocks[0].operations[0];
    assert!(operation.memory_effects().is_empty());
    assert!(operation.has_complete_effect_summary());
    assert!(!operation.combined_effect_summary_v12().is_pure());
    assert_eq!(
        operation.operands(),
        vec![ValueId(0), ValueId(1), ValueId(2)]
    );
    let region = operation.compiler_ordering_effects_v12();
    assert!(region.has_ordered_region());
    assert!(!region.has_ordered_execution());
    assert!(!region.has_ordered_verification_contract());
    let families = [
        CompilerOrderingEffectSummaryV12::ordered_verification_contract(),
        CompilerOrderingEffectSummaryV12::ordered_execution(),
        region,
    ];
    let expected = [
        None,
        Some(CompilerOrderingEffectV12::OrderedVerificationContract),
        Some(CompilerOrderingEffectV12::OrderedExecution),
        Some(CompilerOrderingEffectV12::OrderedVerificationContractAndExecution),
        Some(CompilerOrderingEffectV12::OrderedRegion),
        Some(CompilerOrderingEffectV12::OrderedVerificationContractAndRegion),
        Some(CompilerOrderingEffectV12::OrderedExecutionAndRegion),
        Some(CompilerOrderingEffectV12::OrderedVerificationContractAndExecutionAndRegion),
    ];
    for (mask, expected) in expected.into_iter().enumerate() {
        let mut summary = CompilerOrderingEffectSummaryV12::empty();
        for (index, family) in families.into_iter().enumerate() {
            if mask & (1 << index) != 0 {
                summary = summary.union(family);
            }
        }
        assert_eq!(summary.effect(), expected);
        assert_eq!(summary.is_empty(), mask == 0);
        assert_eq!(summary.union(summary), summary);
    }
}
