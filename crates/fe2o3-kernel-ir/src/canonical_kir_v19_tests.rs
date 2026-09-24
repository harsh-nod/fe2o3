//! Synthetic complete graphs and custody controls, never source/native evidence.
use super::*;
use crate::*;

fn fixture(diamond: bool) -> Module {
    let mut module = if diamond {
        crate::gfx942_complete_body_profile_v19::tests::diamond()
    } else {
        crate::gfx942_complete_body_profile_v19::tests::single()
    };
    // Inert fixtures advertise exactly the normal operation-derived requirements.
    // The actual source producer must independently account and derive its set.
    for function in &mut module.functions {
        function.required_capabilities = function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .flat_map(Operation::required_capabilities)
            .collect();
    }
    module.required_capabilities = module
        .functions
        .iter()
        .flat_map(|function| function.required_capabilities.iter().cloned())
        .collect();
    module
}
fn admit(module: &Module) -> VerifiedCanonicalKernelIrModuleV19 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(8_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 8_000_000);
    VerifiedCanonicalKernelIrModuleV19::from_module_ref_with_verification_budget_v19(
        module,
        &mut budget,
    )
    .unwrap()
    .0
}

#[test]
fn one_block_and_diamond_owners_retain_exact_fresh_inverse_and_unreserved_receipts() {
    for diamond in [false, true] {
        let source = fixture(diamond);
        let bytes = encode_module_v19(&source).unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(8_000_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 8_000_000);
        budget.reserve_storage(73).unwrap();
        let (from_module, receipt) =
            VerifiedCanonicalKernelIrModuleV19::from_module_ref_with_verification_budget_v19(
                &source,
                &mut budget,
            )
            .unwrap();
        assert_eq!(budget.storage(), 73);
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let floor = budget.storage();
        let (from_bytes, bytes_receipt) =
            VerifiedCanonicalKernelIrModuleV19::from_canonical_bytes_with_verification_budget_v19(
                &bytes,
                &mut budget,
            )
            .unwrap();
        assert_eq!(budget.storage(), floor);
        budget
            .reserve_storage(bytes_receipt.retained_storage())
            .unwrap();
        assert_eq!(receipt, bytes_receipt);
        assert_eq!(from_module, from_bytes);
        assert_eq!(from_module.module(), &source);
        assert_eq!(from_module.canonical_bytes(), bytes);
        assert_ne!(
            from_module.module().functions.as_ptr(),
            source.functions.as_ptr()
        );
        assert_ne!(from_bytes.canonical_bytes().as_ptr(), bytes.as_ptr());
        drop(source);
        drop(bytes);
        assert_eq!(from_module.module().functions.len(), 1);
        drop(from_module);
        budget.release_storage(receipt.retained_storage()).unwrap();
        drop(from_bytes);
        budget
            .release_storage(bytes_receipt.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), 73);
    }
}

#[test]
fn full_canonical_identity_uses_the_exact_v19_domain_and_bytes() {
    let module = fixture(true);
    let owner = admit(&module);
    let bytes = owner.canonical_bytes();
    let mut hash = Sha256::new();
    hash.update(39_u32.to_le_bytes());
    hash.update(b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V19\0");
    hash.update(1_u16.to_le_bytes());
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    assert_eq!(
        owner.identity().digest(),
        &<[u8; 32]>::from(hash.finalize())
    );
    assert_eq!(owner.identity().canonical_length(), bytes.len() as u64);
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
}

#[test]
fn complete_graphs_cannot_be_encoded_by_any_older_public_profile() {
    type Encoder = fn(&Module) -> Result<Vec<u8>, KernelIrEncodeError>;
    let old: [Encoder; 15] = [
        encode_module_v1,
        encode_module_v2,
        encode_module_v3,
        encode_module_v4,
        encode_module_v5,
        encode_module_v6,
        encode_module_v7,
        encode_module_v8,
        encode_module_v9,
        encode_module_v10,
        encode_module_v11,
        encode_module_v12,
        encode_module_v15,
        encode_module_v16,
        encode_module_v17,
    ];
    let source = fixture(false);
    for encode in old {
        assert!(encode(&source).is_err());
    }
    let bytes = encode_module_v19(&source).unwrap();
    assert!(decode_module_v12(&bytes).is_err());
    assert!(decode_module_v15(&bytes).is_err());
    assert!(decode_module_v16(&bytes).is_err());
    assert!(decode_module_v17(&bytes).is_err());
}

#[test]
fn exact_v19_rejects_v18_old_and_future_headers_without_fallback() {
    let bytes = encode_module_v19(&fixture(false)).unwrap();
    for version in [0_u16, 1, 11, 12, 13, 14, 15, 16, 17, 18, 20, u16::MAX] {
        let mut changed = bytes.clone();
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            decode_module_v19(&changed),
            Err(KernelIrDecodeError::UnknownVersion(version))
        );
    }
}

#[test]
fn exact_v19_excludes_execution_roles_and_ordered_program_payloads() {
    let mut execution = Module::new("execution");
    execution.functions.push(Function::declaration(
        "external",
        Signature::new(vec![Type::Execution(ExecutionRoleV15::Context)], vec![]),
    ));
    assert!(encode_module_v19(&execution).is_err());
    let mut bytes = encode_module_v15(&execution).unwrap();
    bytes[8..10].copy_from_slice(&19_u16.to_le_bytes());
    assert!(decode_module_v19(&bytes).is_err());

    let program = Gfx942OrderedProgramV1::new(
        AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
        Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
        [ValueId(0), ValueId(1), ValueId(2)],
        Gfx942U32ProgramV1::from_instructions(&[Gfx942ProgramInstructionV1::Move {
            destination: Gfx942ProgramDestinationV1::Output,
            source: Gfx942ProgramRoleV1::Input0,
        }])
        .unwrap(),
    )
    .unwrap();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        vec![ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32))],
        OperationKind::Gfx942OrderedProgram(program),
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    let mut old = Module::new("ordered");
    old.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32); 3],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    assert!(encode_module_v19(&old).is_err());
    let mut bytes = encode_module_v17(&old).unwrap();
    bytes[8..10].copy_from_slice(&19_u16.to_le_bytes());
    assert!(decode_module_v19(&bytes).is_err());
}

#[test]
fn wire_valid_but_semantically_changed_roles_cfg_and_tail_are_not_owner_admission() {
    for mutation in 0..4 {
        let mut source = fixture(false);
        let body = source.functions[0].body.as_mut().unwrap();
        match mutation {
            0 => {
                let OperationKind::Gfx942CompleteBodyStep(step) =
                    &mut body.blocks[0].operations[1].kind
                else {
                    panic!("step");
                };
                step.operands[0] = Some(ValueId(2));
            }
            1 => body.blocks[0].operations[1].results[0].ty = Type::BOOL,
            2 => {
                let OperationKind::Gfx942CompleteBodyDeclaration(declaration) =
                    &mut body.blocks[0].operations[0].kind
                else {
                    panic!("declaration");
                };
                declaration.instruction_count = 2;
            }
            _ => body.blocks[0].operations.pop().map(|_| ()).unwrap(),
        }
        let bytes = encode_module_v19(&source).unwrap();
        assert_eq!(decode_module_v19(&bytes).unwrap(), source);
        for from_bytes in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(8_000_000);
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 8_000_000);
            budget.reserve_storage(73).unwrap();
            let result = if from_bytes {
                VerifiedCanonicalKernelIrModuleV19::from_canonical_bytes_with_verification_budget_v19(
                    &bytes, &mut budget)
            } else {
                VerifiedCanonicalKernelIrModuleV19::from_module_ref_with_verification_budget_v19(
                    &source,
                    &mut budget,
                )
            };
            assert!(result.is_err());
            assert_eq!(budget.storage(), 73);
            assert!(budget.work() > 0);
        }
    }
}

#[test]
fn exact_and_one_short_work_storage_limits_preserve_nonzero_caller_floor() {
    let source = fixture(true);
    for from_bytes in [false, true] {
        let bytes = encode_module_v19(&source).unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(8_000_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 8_000_000);
        budget.reserve_storage(73).unwrap();
        let run = |budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>| {
            if from_bytes {
                VerifiedCanonicalKernelIrModuleV19::from_canonical_bytes_with_verification_budget_v19(&bytes, budget)
            } else {
                VerifiedCanonicalKernelIrModuleV19::from_module_ref_with_verification_budget_v19(
                    &source, budget,
                )
            }
        };
        drop(run(&mut budget).unwrap());
        let needed_work = budget.work();
        let needed_storage = budget.peak_storage();
        for case in 0..3 {
            let mut work =
                CanonicalKernelIrWorkBudgetV1::new(11 + needed_work - usize::from(case == 1));
            work.charge_work(11).unwrap();
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
                &mut work,
                needed_storage - usize::from(case == 2),
            );
            budget.reserve_storage(73).unwrap();
            let result = run(&mut budget);
            assert_eq!(result.is_ok(), case == 0);
            assert_eq!(budget.storage(), 73);
            if case == 0 {
                assert_eq!(budget.work(), 11 + needed_work);
            }
        }
    }
}

#[test]
fn declaration_step_operand_effect_and_capability_visitors_are_exact() {
    let source = fixture(false);
    let operations = &source.functions[0].body.as_ref().unwrap().blocks[0].operations;
    let declaration = &operations[0];
    assert_eq!(
        declaration.operands(),
        (0..5).map(ValueId).collect::<Vec<_>>()
    );
    let mut visited = Vec::new();
    assert_eq!(
        declaration.kind.try_visit_operands(|value| {
            visited.push(value);
            if value == ValueId(2) { Err(()) } else { Ok(()) }
        }),
        Err(())
    );
    assert_eq!(visited, [ValueId(0), ValueId(1), ValueId(2)]);
    let mut step = operations[1].clone();
    let OperationKind::Gfx942CompleteBodyStep(payload) = &mut step.kind else {
        panic!("step");
    };
    payload.instruction = Gfx942ProgramInstructionV1::Binary {
        opcode: Gfx942ProgramBinaryOpcodeV1::Xor,
        destination: Gfx942ProgramDestinationV1::Output,
        left: Gfx942ProgramRoleV1::Output,
        right: Gfx942ProgramRoleV1::Output,
    };
    payload.operands = [Some(ValueId(5)); 2];
    assert_eq!(step.operands(), [ValueId(5), ValueId(5)]);
    for operation in [declaration, &step] {
        assert!(operation.memory_effects().is_empty());
        let mut effects = 0;
        operation
            .try_visit_local_memory_effects_v1::<()>(|_| {
                effects += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(effects, 0);
        let capabilities = operation.required_capabilities();
        assert_eq!(capabilities.len(), 3);
        assert!(capabilities.contains(&TargetCapability::WaveWidth(WaveWidth::Wave64)));
        assert!(capabilities.contains(&TargetCapability::Extension {
            namespace: AMDGPU_GFX942_COMPLETE_BODY_CAPABILITY_NAMESPACE_V19.into(),
            name: AMDGPU_GFX942_COMPLETE_BODY_CAPABILITY_NAME_V19.into(),
        }));
        assert!(capabilities.contains(&TargetCapability::Extension {
            namespace: AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.into(),
            name: AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME.into(),
        }));
    }
}

#[test]
fn empty_owner_literal_budget_preserves_existing_base_carrier_accounting() {
    let module = Module::new("m");
    let bytes = encode_module_v19(&module).unwrap();
    assert_eq!(bytes.len(), 37);
    let retained = std::mem::size_of::<VerifiedCanonicalKernelIrModuleV19>() + 37 + 1;
    for (from_bytes, work_bound) in [(false, 294), (true, 233)] {
        for case in 0..3 {
            let mut work =
                CanonicalKernelIrWorkBudgetV1::new(11 + work_bound - usize::from(case == 1));
            work.charge_work(11).unwrap();
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
                &mut work,
                73 + retained - usize::from(case == 2),
            );
            budget.reserve_storage(73).unwrap();
            let result = if from_bytes {
                VerifiedCanonicalKernelIrModuleV19::from_canonical_bytes_with_verification_budget_v19(
                    &bytes, &mut budget)
            } else {
                VerifiedCanonicalKernelIrModuleV19::from_module_ref_with_verification_budget_v19(
                    &module,
                    &mut budget,
                )
            };
            assert_eq!(
                result.is_ok(),
                case == 0,
                "bytes={from_bytes} case={case}: {result:?}"
            );
            assert_eq!(budget.storage(), 73);
            if case == 0 {
                assert_eq!(budget.work(), 11 + work_bound);
                assert_eq!(budget.peak_storage(), 73 + retained);
                assert_eq!(result.unwrap().1.retained_storage(), retained);
            }
        }
    }
}
