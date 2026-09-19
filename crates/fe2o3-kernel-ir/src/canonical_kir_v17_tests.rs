//! Synthetic canonical custody/codec controls, not authenticated source evidence.
use super::*;
use crate::*;

fn words(active: &[u16]) -> [u16; 16] {
    let mut result = [0; 16];
    result[..active.len()].copy_from_slice(active);
    result
}
fn program() -> Gfx942U32ProgramV1 {
    Gfx942U32ProgramV1::from_descriptors(3, words(&[0x85, 0x133, 0x19d])).unwrap()
}
fn fixture() -> Module {
    fixture_with_program(program())
}

fn fixture_with_program(program: Gfx942U32ProgramV1) -> Module {
    let region = Gfx942OrderedProgramV1::new(
        AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
        Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
        [ValueId(0), ValueId(1), ValueId(2)],
        program,
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
                OperationKind::Gfx942OrderedProgram(region),
            )],
            terminator: Some(Terminator::Return {
                values: vec![ValueId(3)],
            }),
        }],
    );
    let capabilities =
        function.body.as_ref().unwrap().blocks[0].operations[0].required_capabilities();
    function.required_capabilities = capabilities.clone();
    let mut module = Module::new("ordered-program-v17");
    module.required_capabilities = capabilities;
    module.functions.push(function);
    module
}

fn admit(module: &Module) -> VerifiedCanonicalKernelIrModuleV17 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
        module,
        &mut budget,
    )
    .unwrap()
    .0
}

#[test]
fn owner_retains_one_fresh_inverse_with_exact_bytes_identity_and_storage_transfer() {
    let source = fixture();
    let bytes = encode_module_v17(&source).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    budget.reserve_storage(7).unwrap();
    let (from_module, module_receipt) =
        VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
            &source,
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), 7);
    budget
        .reserve_storage(module_receipt.retained_storage())
        .unwrap();
    let (from_bytes, bytes_receipt) =
        VerifiedCanonicalKernelIrModuleV17::from_canonical_bytes_with_verification_budget_v17(
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
    hash.update(b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V17\0");
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
    let bytes = encode_module_v17(&module).unwrap();
    assert_eq!(bytes.len(), 37);
    let retained = std::mem::size_of::<VerifiedCanonicalKernelIrModuleV17>() + 37 + 1;
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
                VerifiedCanonicalKernelIrModuleV17::from_canonical_bytes_with_verification_budget_v17(&bytes, &mut budget)
            } else {
                VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
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
fn program_owner_budget_is_cumulative_and_exact_one_short_limits_restore_floor() {
    let source = fixture();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    budget.reserve_storage(7).unwrap();
    let baseline =
        VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
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
            VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
                &source,
                &mut budget,
            );
        assert_eq!(result.is_ok(), case == 0);
        assert_eq!(budget.storage(), 7);
        if case == 0 {
            assert_eq!(budget.work(), 11 + exact_work);
            assert!(
                VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
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
fn v17_never_relabels_old_owners_or_admits_execution_grammar() {
    let region = fixture();
    assert!(encode_module_v12(&region).is_err());
    assert!(encode_module_v15(&region).is_err());
    let bytes = encode_module_v17(&region).unwrap();
    assert!(decode_module_v12(&bytes).is_err());
    assert!(decode_module_v15(&bytes).is_err());
    for role in [
        ExecutionRoleV15::Context,
        ExecutionRoleV15::Workgroup,
        ExecutionRoleV15::MaskedTileU32 {
            lanes: 64,
            elements: 1,
        },
        ExecutionRoleV15::LaneFragmentU32 {
            lanes: 64,
            elements: 1,
        },
    ] {
        let mut execution = Module::new("execution");
        execution.functions.push(Function::declaration(
            "external",
            Signature::new(vec![Type::Execution(role)], vec![]),
        ));
        assert!(encode_module_v17(&execution).is_err());
        let mut disguised = encode_module_v15(&execution).unwrap();
        disguised[8..10].copy_from_slice(&17_u16.to_le_bytes());
        assert!(decode_module_v17(&disguised).is_err());
    }
    for version in [1_u16, 11, 12, 13, 14, 15, 16, 18] {
        let mut changed = bytes.clone();
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        assert!(decode_module_v17(&changed).is_err());
    }
    let plain = Module::new("m");
    let old = VerifiedCanonicalKernelIrV12::from_module(plain.clone()).unwrap();
    let new = admit(&plain);
    assert_ne!(old.identity().digest(), new.identity().digest());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    let (adjacent, _) =
        VerifiedCanonicalKernelIrModuleV16::from_module_ref_with_verification_budget_v16(
            &plain,
            &mut budget,
        )
        .unwrap();
    assert_eq!(adjacent.module(), new.module());
    assert_ne!(adjacent.identity().digest(), new.identity().digest());
    assert_ne!(adjacent.canonical_bytes(), new.canonical_bytes());
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
                let OperationKind::Gfx942OrderedProgram(region) = operation.kind else {
                    unreachable!()
                };
                operation.kind = OperationKind::Gfx942OrderedProgram(
                    Gfx942OrderedProgramV1::new(
                        region.source(),
                        region.registers(),
                        [ValueId(0), ValueId(1), ValueId(999)],
                        *region.program(),
                    )
                    .unwrap(),
                );
            }
            _ => {
                module.functions[0].signature.parameters[0] = Type::Scalar(ScalarType::I32);
            }
        }
        let bytes = encode_module_v17(&module).unwrap();
        assert!(decode_module_v17(&bytes).is_ok());
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
        budget.reserve_storage(7).unwrap();
        let result =
            VerifiedCanonicalKernelIrModuleV17::from_canonical_bytes_with_verification_budget_v17(
                &bytes,
                &mut budget,
            );
        assert!(
            matches!(
                result,
                Err(CanonicalKernelIrReplayAdmissionErrorV17::Verification(_))
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
    let bytes = encode_module_v17(&fixture()).unwrap();
    let mut trailing = bytes.clone();
    trailing.push(0);
    let invalid = encode_module_v17(&Module::new("")).unwrap();
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
            VerifiedCanonicalKernelIrModuleV17::from_canonical_bytes_with_verification_budget_v17(
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
fn program_effect_is_independent_nonpure_and_union_preserves_all_three_families() {
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

fn program_payload(module: &Module) -> &Gfx942OrderedProgramV1 {
    let OperationKind::Gfx942OrderedProgram(program) =
        &module.functions[0].body.as_ref().unwrap().blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    program
}

fn unique_program_tag(bytes: &[u8]) -> usize {
    let marker = [1; 32];
    let matches: Vec<_> = bytes
        .windows(marker.len())
        .enumerate()
        .filter_map(|(i, v)| (v == marker).then_some(i))
        .collect();
    assert_eq!(matches.len(), 1);
    let tag = matches[0].checked_sub(2).unwrap();
    assert_eq!(&bytes[tag..tag + 2], &[39, 0]);
    tag
}

#[test]
fn owner_does_not_borrow_mutable_source_module_or_input_bytes() {
    let mut source = fixture();
    let expected = source.clone();
    let mut bytes = encode_module_v17(&source).unwrap();
    let owner = admit(&source);
    source.id = "mutated-source".into();
    source.functions.clear();
    source.required_capabilities.clear();
    assert_eq!(owner.module(), &expected);
    assert_eq!(owner.canonical_bytes(), bytes);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    let (decoded, receipt) =
        VerifiedCanonicalKernelIrModuleV17::from_canonical_bytes_with_verification_budget_v17(
            &bytes,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let identity = *decoded.identity();
    bytes.fill(0);
    assert_eq!(decoded.module(), &expected);
    assert_eq!(decoded.identity(), &identity);
    assert_eq!(
        decoded.canonical().canonical_bytes(),
        owner.canonical_bytes()
    );
    assert!(std::ptr::eq(
        decoded.canonical_bytes().as_ptr(),
        decoded.canonical().canonical_bytes().as_ptr()
    ));
    drop(decoded);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn dead_overwritten_repeated_and_unused_steps_keep_distinct_exact_owner_identity() {
    let descriptions = [
        (1, words(&[8])),
        (2, words(&[0x10, 8])),
        (2, words(&[0x20, 8])),
        (16, [8; 16]),
    ];
    let mut identities = Vec::new();
    for (count, descriptors) in descriptions {
        let program = Gfx942U32ProgramV1::from_descriptors(count, descriptors).unwrap();
        assert_eq!(program.evaluate([19, 23, 42]), 19);
        let mut module = fixture_with_program(program);
        // The single authored unit's result is deliberately unused.
        module.functions[0].body.as_mut().unwrap().blocks[0].terminator =
            Some(Terminator::Return {
                values: vec![ValueId(0)],
            });
        let owner = admit(&module);
        assert_eq!(
            owner.module().functions[0].body.as_ref().unwrap().blocks[0]
                .operations
                .len(),
            1
        );
        assert_eq!(program_payload(owner.module()).program(), &program);
        for previous in &identities {
            assert_ne!(previous, owner.identity());
        }
        identities.push(*owner.identity());
        let bytes = owner.canonical_bytes();
        let tag = unique_program_tag(bytes);
        assert_eq!(bytes[tag + 1 + 146], count);
        for (position, descriptor) in descriptors.into_iter().enumerate() {
            assert_eq!(
                &bytes[tag + 1 + 147 + 2 * position..tag + 1 + 149 + 2 * position],
                &descriptor.to_le_bytes()
            );
        }
    }
}

#[test]
fn declared_source_and_binding_changes_alter_identity_without_authenticating_them() {
    let original = fixture();
    let original_owner = admit(&original);
    for case in 0..5 {
        let base = *program_payload(&original);
        let mut source = base.source();
        let mut registers = base.registers();
        match case {
            0 => source.frontend_unit[0] ^= 1,
            1 => source.function[0] ^= 1,
            2 => source.contract[0] ^= 1,
            3 => source.statement[0] ^= 1,
            4 => registers = Gfx942OrderedProgramRegistersV1::new(40, 41, [42, 43, 44]).unwrap(),
            _ => unreachable!(),
        }
        let mut changed = original.clone();
        changed.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
            OperationKind::Gfx942OrderedProgram(
                Gfx942OrderedProgramV1::new(source, registers, *base.inputs(), *base.program())
                    .unwrap(),
            );
        let owner = admit(&changed);
        assert_ne!(owner.identity(), original_owner.identity());
        assert_eq!(program_payload(owner.module()).source(), source);
        assert_eq!(program_payload(owner.module()).registers(), registers);
        assert_eq!(
            program_payload(owner.module())
                .program()
                .evaluate([19, 23, 42]),
            base.program().evaluate([19, 23, 42])
        );
    }
}

#[test]
fn v17_branch_rejects_old_pair_mixed_modules_and_execution_operation_tags() {
    let mut old = fixture();
    let base = *program_payload(&old);
    let old_pair = OperationKind::Gfx942OrderedRegion(
        Gfx942OrderedRegionV1::new(
            base.source(),
            Gfx942OrderedRegionRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
            *base.inputs(),
        )
        .unwrap(),
    );
    old.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind = old_pair.clone();
    assert!(encode_module_v16(&old).is_ok());
    assert!(encode_module_v17(&old).is_err());
    let mut mixed = fixture();
    mixed.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32))],
            old_pair,
        ));
    assert!(encode_module_v16(&mixed).is_err());
    assert!(encode_module_v17(&mixed).is_err());
    let bytes = encode_module_v17(&fixture()).unwrap();
    let tag = unique_program_tag(&bytes);
    for forbidden in (32..=38).chain([40, 88, 255]) {
        let mut bad = bytes.clone();
        bad[tag] = forbidden;
        assert!(matches!(
            decode_module_v17(&bad),
            Err(KernelIrDecodeError::UnknownTag { tag: actual, .. }) if actual == forbidden
        ));
    }
    // Raw tag 88 here is merely an unknown KIR opcode. MIR intrinsic 88 needs
    // separate semantic-V32 codec tests; this does not claim to test that format.
    assert!(decode_module_v16(&bytes).is_err());
    let mut disguised = encode_module_v16(&old).unwrap();
    disguised[8..10].copy_from_slice(&17_u16.to_le_bytes());
    assert!(decode_module_v17(&disguised).is_err());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    budget.reserve_storage(7).unwrap();
    for module in [&old, &mixed] {
        assert!(
            VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
                module,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 7);
    }
}

#[test]
fn canonical_owner_refuses_every_truncation_and_malformed_program_at_a_restored_floor() {
    let bytes = encode_module_v17(&fixture()).unwrap();
    let tag = unique_program_tag(&bytes);
    let payload = tag + 1;
    for end in 0..bytes.len() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
        budget.reserve_storage(7).unwrap();
        assert!(
            VerifiedCanonicalKernelIrModuleV17::from_canonical_bytes_with_verification_budget_v17(
                &bytes[..end],
                &mut budget
            )
            .is_err(),
            "end={end}"
        );
        assert_eq!(budget.storage(), 7);
    }
    for case in 0..7 {
        let mut bad = bytes.clone();
        match case {
            0 => bad[payload] = 1,
            1 => bad[payload + 146] = 0,
            2 => bad[payload + 146] = 17,
            3 => bad[payload + 147..payload + 149].copy_from_slice(&0x0030_u16.to_le_bytes()),
            4 => bad[payload + 177..payload + 179].copy_from_slice(&1_u16.to_le_bytes()),
            5 => bad[payload + 147..payload + 149].copy_from_slice(&0x0408_u16.to_le_bytes()),
            6 => bad[payload + 130] = bad[payload + 129],
            _ => unreachable!(),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
        budget.reserve_storage(7).unwrap();
        let result =
            VerifiedCanonicalKernelIrModuleV17::from_canonical_bytes_with_verification_budget_v17(
                &bad,
                &mut budget,
            );
        assert!(
            matches!(
                result,
                Err(CanonicalKernelIrReplayAdmissionErrorV17::Decode(_))
            ),
            "case={case}: {result:?}"
        );
        assert_eq!(budget.storage(), 7);
    }
}

#[test]
fn one_three_and_sixteen_step_owner_admissions_honor_both_replay_budget_routes() {
    for program in [
        Gfx942U32ProgramV1::from_descriptors(1, words(&[8])).unwrap(),
        program(),
        Gfx942U32ProgramV1::from_descriptors(16, [8; 16]).unwrap(),
    ] {
        let module = fixture_with_program(program);
        let bytes = encode_module_v17(&module).unwrap();
        for from_bytes in [false, true] {
            let run = |budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>| {
                if from_bytes {
                    VerifiedCanonicalKernelIrModuleV17::from_canonical_bytes_with_verification_budget_v17(
                        &bytes, budget,
                    )
                } else {
                    VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
                        &module, budget,
                    )
                }
            };
            let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
            budget.reserve_storage(7).unwrap();
            let baseline = run(&mut budget).unwrap();
            let exact_work = budget.work();
            let exact_peak = budget.peak_storage();
            let retained = baseline.1.retained_storage();
            drop(baseline);
            assert_eq!(budget.storage(), 7);
            for case in 0..3 {
                let mut work =
                    CanonicalKernelIrWorkBudgetV1::new(11 + exact_work - usize::from(case == 1));
                work.charge_work(11).unwrap();
                let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
                    &mut work,
                    exact_peak - usize::from(case == 2),
                );
                budget.reserve_storage(7).unwrap();
                let result = run(&mut budget);
                assert_eq!(
                    result.is_ok(),
                    case == 0,
                    "count={} bytes={from_bytes} case={case}",
                    program.count()
                );
                assert_eq!(budget.storage(), 7);
                if let Ok((owner, receipt)) = result {
                    assert_eq!(receipt.retained_storage(), retained);
                    assert_eq!(owner.canonical_bytes(), bytes);
                    assert_eq!(budget.work(), 11 + exact_work);
                    assert_eq!(budget.peak_storage(), exact_peak);
                }
            }
        }
    }
}

#[test]
fn actual_required_capabilities_include_wave64_and_exact_target_not_hardware_authority() {
    let mut module = fixture();
    let supported = module.required_capabilities.clone();
    module.required_capabilities.clear();
    module.functions[0].required_capabilities.clear();
    assert!(verify_module_with_capabilities(&module, &supported).is_ok());
    for missing in [
        TargetCapability::WaveWidth(WaveWidth::Wave64),
        TargetCapability::Extension {
            namespace: AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.to_owned(),
            name: AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME.to_owned(),
        },
        TargetCapability::Extension {
            namespace: AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAMESPACE.to_owned(),
            name: AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAME.to_owned(),
        },
    ] {
        let mut partial = supported.clone();
        assert!(partial.remove(&missing));
        assert!(verify_module_with_capabilities(&module, &partial).is_err());
    }
    assert_eq!(admit(&module).module(), &module); // Owner does not authenticate hardware availability.
}

#[test]
fn repeated_logical_inputs_remain_distinct_from_physical_binding_aliases() {
    let mut module = fixture();
    let base = *program_payload(&module);
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
        OperationKind::Gfx942OrderedProgram(
            Gfx942OrderedProgramV1::new(
                base.source(),
                base.registers(),
                [ValueId(0); 3],
                *base.program(),
            )
            .unwrap(),
        );
    let owner = admit(&module);
    assert_eq!(program_payload(owner.module()).inputs(), &[ValueId(0); 3]);
    assert_eq!(
        program_payload(owner.module()).registers().inputs(),
        [34, 35, 36]
    );
    assert_eq!(
        owner.module().functions[0].body.as_ref().unwrap().blocks[0].operations[0].operands(),
        vec![ValueId(0); 3]
    );
}
