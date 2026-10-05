#[cfg(test)]
mod private_bf16_native_v12_tests {
    use super::*;
    use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work, TensorLayoutContractV1, ValueDef,
        VerifiedCanonicalKernelIrModuleV12 as Owner,
    };

    fn source(permutation: [u8; 4], trapped: bool) -> Module {
        let parameters = (0..12)
            .map(|i| {
                if i < 8 {
                    Type::Scalar(ScalarType::Bf16)
                } else {
                    Type::F32
                }
            })
            .collect::<Vec<_>>();
        let mut call = BasicBlock::new(BlockId(0));
        call.operations.push(Operation::new(
            (12..16)
                .map(|i| ValueDef::new(ValueId(i), Type::F32))
                .collect(),
            OperationKind::Call {
                callee: FunctionId::new("mfma"),
                arguments: (0..12).map(ValueId).collect(),
            },
        ));
        if trapped {
            call.operations
                .push(AmdGpuDiagnosticOperation::Trap.operation(None));
        }
        call.terminator = Some(if trapped {
            Terminator::Unreachable
        } else {
            Terminator::Return { values: vec![] }
        });
        let mut blocks = vec![call];
        if trapped {
            // A distinct reachable branch supplies the root's sole normal Return.
            let mut start = BasicBlock::new(BlockId(2));
            start.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(16), Type::BOOL),
                OperationKind::Constant(Constant::Bool(true)),
            ));
            start.terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(16),
                then_target: BlockId(0),
                then_arguments: vec![],
                else_target: BlockId(1),
                else_arguments: vec![],
            });
            let mut done = BasicBlock::new(BlockId(1));
            done.terminator = Some(Terminator::Return { values: vec![] });
            blocks = vec![start, blocks.pop().unwrap(), done];
        }
        let mut root = Function::kernel_entry(
            "entry",
            Signature::new(parameters.clone(), vec![]),
            (0..12).map(ValueId).collect(),
            blocks,
        );
        root.required_capabilities = root.derived_capabilities();
        // Calls do not derive the capability of their narrow-float arguments.
        root.required_capabilities
            .insert(TargetCapability::BFloat16);
        let mut body = BasicBlock::new(BlockId(0));
        body.operations.push(Operation::new(
            (12..16)
                .map(|i| ValueDef::new(ValueId(i), Type::F32))
                .collect(),
            OperationKind::Matrix(
                MatrixOperation::multiply_accumulate(
                    [ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
                    [ValueId(4), ValueId(5), ValueId(6), ValueId(7)],
                    [ValueId(8), ValueId(9), ValueId(10), ValueId(11)],
                )
                .with_declared_tensor_layout(
                    TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
                        .with_zero_filled_predicate_inputs(),
                ),
            ),
        ));
        body.terminator = Some(Terminator::Return {
            values: permutation
                .into_iter()
                .map(|i| ValueId(12 + u32::from(i)))
                .collect(),
        });
        let mut helper = Function::internal_helper(
            "mfma",
            Signature::new(parameters, vec![Type::F32; 4]),
            (0..12).map(ValueId).collect(),
            vec![body],
        );
        helper.required_capabilities = helper.derived_capabilities();
        let mut module = Module::new("private_bf16_native_control");
        module.functions = vec![root, helper];
        if trapped {
            module
                .functions
                .push(AmdGpuDiagnosticOperation::Trap.declaration());
        }
        let mut kernel = Kernel::new(
            "entry",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
        module
    }
    fn with_owner(
        source: &Module,
        profile: Profile,
        inspect: impl FnOnce(&Owner, &mut Budget<'_>),
    ) {
        let bound = crate::bind_production_target_v1(source, profile).unwrap();
        let mut work = Work::new(1_000_000_000_000);
        let mut budget = Budget::new(&mut work, 128 * 1024 * 1024);
        budget.reserve_storage(7).unwrap();
        let (owner, storage) =
            Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut budget)
                .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        // Caller-owned LLVM bytes are distinct from the model's selected frame.
        budget
            .reserve_storage(MAX_COMPILER_MODULE_TEXT_BYTES)
            .unwrap();
        inspect(&owner, &mut budget);
        budget
            .release_storage(MAX_COMPILER_MODULE_TEXT_BYTES)
            .unwrap();
        drop(owner);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), 7);
    }

    #[test]
    fn missing_root_bf16_signature_capability_is_not_inferred_from_helper() {
        let mut module = source([0, 1, 2, 3], false);
        assert!(
            module.functions[0]
                .required_capabilities
                .remove(&TargetCapability::BFloat16)
        );
        assert!(
            module.functions[1]
                .required_capabilities
                .contains(&TargetCapability::BFloat16)
        );
        with_owner(&module, Profile::Gfx942, |owner, budget| {
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let generic =
                lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(owner)
                    .unwrap_err();
            assert!(generic.contains(LoweringDiagnosticCode::UnsupportedCapability));
            let private =
                lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(owner, [0, 1, 2, 3], budget);
            assert!(matches!(private,
                Err(PrivateBf16NativeLlvmErrorV1::Lowering(ref error))
                    if error.contains(LoweringDiagnosticCode::UnsupportedCapability)));
            drop(private);
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(budget.check_prior_denials_v1().is_ok());
        });
    }

    #[test]
    fn actual_owner_identity_and_swap_emit_mfma_without_opening_generic_helper_path() {
        for permutation in [[0, 1, 2, 3], [1, 0, 2, 3]] {
            with_owner(
                &source(permutation, false),
                Profile::Gfx942,
                |owner, budget| {
                    let floor = budget.storage();
                    assert!(lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(owner)
                    .unwrap_err().contains(LoweringDiagnosticCode::UnsupportedMatrixOperation));
                    let text = lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(
                        owner,
                        permutation,
                        budget,
                    )
                    .unwrap();
                    assert!(text.contains(AmdgcnIntrinsic::MfmaF32M16N16K16Bf16.llvm_name()));
                    assert!(text.contains("!\"multiple_defined_bodies\""));
                    assert!(!text.contains("call void @llvm.pseudoprobe"));
                    let digest = lower_hex(owner.canonical().identity().digest());
                    assert!(text.contains(&format!("!\"sha256:{digest}\", !\"kir-version:12\"")));
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.check_prior_denials_v1().is_ok());
                    drop(text);
                },
            );
        }
    }

    #[test]
    fn exact_trap_declaration_and_actual_presence_are_required_both_ways() {
        with_owner(
            &source([0, 1, 2, 3], true),
            Profile::Gfx942,
            |owner, budget| {
                let text = lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(
                    owner,
                    [0, 1, 2, 3],
                    budget,
                )
                .unwrap();
                assert!(text.contains("llvm.trap"));
            },
        );
        let mut dead = source([0, 1, 2, 3], false);
        dead.functions
            .push(AmdGpuDiagnosticOperation::Trap.declaration());
        with_owner(&dead, Profile::Gfx942, |owner, budget| {
            assert!(matches!(
                lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(owner, [0, 1, 2, 3], budget),
                Err(PrivateBf16NativeLlvmErrorV1::Context(
                    "root Return or Trap presence"
                ))
            ));
        });
        let mut substituted = source([0, 1, 2, 3], false);
        substituted
            .functions
            .push(AmdGpuDiagnosticOperation::DebugTrap.declaration());
        with_owner(&substituted, Profile::Gfx942, |owner, budget| {
            assert!(matches!(
                lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(owner, [0, 1, 2, 3], budget),
                Err(PrivateBf16NativeLlvmErrorV1::Context(
                    "nonexact Trap declaration"
                ))
            ));
        });
    }

    #[test]
    fn actual_return_operand_role_extra_helper_operation_and_extra_call_refuse() {
        with_owner(
            &source([0, 1, 2, 3], false),
            Profile::Gfx942,
            |owner, budget| {
                assert!(matches!(
                    lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(
                        owner,
                        [1, 0, 2, 3],
                        budget
                    ),
                    Err(PrivateBf16NativeLlvmErrorV1::Context(
                        "actual optimized Return permutation"
                    ))
                ));
                assert!(
                    lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(
                        owner,
                        [0, 0, 2, 3],
                        budget
                    )
                    .is_err()
                );
            },
        );
        for mutation in 0..3 {
            let mut module = source([0, 1, 2, 3], false);
            match mutation {
                0 => {
                    let OperationKind::Matrix(matrix) =
                        &mut module.functions[1].body.as_mut().unwrap().blocks[0].operations[0]
                            .kind
                    else {
                        unreachable!()
                    };
                    let MatrixOperationKind::MultiplyAccumulate { lhs, .. } = &mut matrix.kind
                    else {
                        unreachable!()
                    };
                    lhs.swap(0, 1);
                }
                1 => module.functions[1].body.as_mut().unwrap().blocks[0]
                    .operations
                    .insert(
                        0,
                        Operation::effect_free(
                            ValueDef::new(ValueId(16), Type::F32),
                            OperationKind::Constant(Constant::F32Bits(0)),
                        ),
                    ),
                _ => {
                    let mut extra =
                        module.functions[0].body.as_ref().unwrap().blocks[0].operations[0].clone();
                    for (i, result) in extra.results.iter_mut().enumerate() {
                        result.id = ValueId(16 + i as u32);
                    }
                    module.functions[0].body.as_mut().unwrap().blocks[0]
                        .operations
                        .push(extra);
                }
            }
            with_owner(&module, Profile::Gfx942, |owner, budget| {
                assert!(matches!(
                    lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(
                        owner,
                        [0, 1, 2, 3],
                        budget
                    ),
                    Err(PrivateBf16NativeLlvmErrorV1::Context(_))
                ));
            });
        }
    }

    #[test]
    fn wrong_target_and_incomplete_physical_wave_are_not_reinterpreted() {
        with_owner(
            &source([0, 1, 2, 3], false),
            Profile::Gfx950,
            |owner, budget| {
                assert!(
                    lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(
                        owner,
                        [0, 1, 2, 3],
                        budget
                    )
                    .is_err()
                );
            },
        );
        let mut module = source([0, 1, 2, 3], false);
        module.kernels[0].domain = LaunchDomain::D1 {
            x: LaunchExtent::Static(63),
        };
        with_owner(&module, Profile::Gfx942, |owner, budget| {
            assert!(matches!(
                lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(owner, [0, 1, 2, 3], budget),
                Err(PrivateBf16NativeLlvmErrorV1::Context(
                    "exact complete wave64 geometry"
                ))
            ));
        });
    }

    #[test]
    fn varying_pre_call_control_is_not_a_helper_uniformity_summary() {
        let mut module = source([0, 1, 2, 3], false);
        let mut start = BasicBlock::new(BlockId(1));
        start.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(16), Type::INDEX),
            OperationKind::Intrinsic(fe2o3_kernel_ir::IntrinsicOperation::global_id_1d()),
        ));
        start.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(17), Type::INDEX),
            OperationKind::Constant(Constant::Index(1)),
        ));
        start.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(18), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(16),
                rhs: ValueId(17),
            },
        ));
        start.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(18),
            then_target: BlockId(0),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        let mut no_call = BasicBlock::new(BlockId(2));
        no_call.terminator = Some(Terminator::Unreachable);
        let body = module.functions[0].body.as_mut().unwrap();
        body.blocks.insert(0, start);
        body.blocks.push(no_call);
        with_owner(&module, Profile::Gfx942, |owner, budget| {
            assert!(matches!(
                lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(owner, [0, 1, 2, 3], budget),
                Err(PrivateBf16NativeLlvmErrorV1::Context(
                    "actual O Call lacks complete wave participation"
                ))
            ));
        });
    }

    #[test]
    fn private_context_rejects_a_foreign_same_bytes_owner() {
        with_owner(
            &source([0, 1, 2, 3], false),
            Profile::Gfx942,
            |first, budget| {
                let (second, storage) =
                    Owner::from_module_ref_with_verification_budget_v12(first.module(), budget)
                        .unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                let frame = bf16_native_frame_storage_v1();
                budget.reserve_storage(frame).unwrap();
                let context = derive_native_bf16_context_v1(first, [0, 1, 2, 3], budget).unwrap();
                let second_helper = &second.module().functions[1];
                let OperationKind::Matrix(second_matrix) =
                    &second_helper.body.as_ref().unwrap().blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                assert!(!context.admits(second.module(), second_helper, second_matrix));
                drop(context);
                budget.release_storage(frame).unwrap();
                drop(second);
                budget.release_storage(storage.retained_storage()).unwrap();
            },
        );
    }

    #[test]
    fn proof_frame_exact_and_one_short_original_storage_preserve_floor_and_history() {
        for short in [false, true] {
            with_owner(
                &source([0, 1, 2, 3], false),
                Profile::Gfx942,
                |owner, budget| {
                    // Owner and proof use this same accumulating account. Fill only
                    // control prefix space; do not reconstruct/reset its ledger.
                    let available = bf16_native_frame_storage_v1() - usize::from(short);
                    let padding = (128 * 1024 * 1024) - budget.storage() - available;
                    budget.reserve_storage(padding).unwrap();
                    let floor = budget.storage();
                    let id = budget.work_ledger_identity_v1();
                    let result = lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(
                        owner,
                        [0, 1, 2, 3],
                        budget,
                    );
                    assert_eq!(result.is_ok(), !short);
                    drop(result);
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work_ledger_identity_v1() == id);
                    if short {
                        let prior = budget.check_prior_denials_v1().unwrap_err();
                        let before = (
                            budget.work(),
                            budget.storage(),
                            budget.failed_work(),
                            budget.failed_storage(),
                        );
                        for _ in 0..2 {
                            assert!(
                                matches!(lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(owner,[0,1,2,3],budget),
                            Err(PrivateBf16NativeLlvmErrorV1::Resource(error)) if error==prior)
                            );
                            assert_eq!(
                                (
                                    budget.work(),
                                    budget.storage(),
                                    budget.failed_work(),
                                    budget.failed_storage()
                                ),
                                before
                            );
                        }
                    }
                    budget.release_storage(padding).unwrap();
                },
            );
        }
    }

    fn root_wave_source(permutation: [u8; 4], reconverge: bool) -> Module {
        let mut module = source(permutation, false);
        let root = &mut module.functions[0];
        let body = root.body.as_mut().unwrap();
        let wave = Operation::effect_free(
            ValueDef::new(ValueId(16), Type::Scalar(ScalarType::U32)),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::LaneId,
                WaveWidth::Wave64,
            )),
        );
        if reconverge {
            let mut entry = BasicBlock::new(BlockId(10));
            entry.operations.push(wave);
            entry.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(17), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(32)),
            ));
            entry.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(18), Type::BOOL),
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(16),
                    rhs: ValueId(17),
                },
            ));
            entry.terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(18),
                then_target: BlockId(11),
                then_arguments: vec![],
                else_target: BlockId(12),
                else_arguments: vec![],
            });
            let arm = |id| {
                let mut block = BasicBlock::new(BlockId(id));
                block.terminator = Some(Terminator::Branch {
                    target: BlockId(0),
                    arguments: vec![],
                });
                block
            };
            body.blocks = vec![entry, arm(11), arm(12), body.blocks.pop().unwrap()];
        } else {
            body.blocks[0].operations.insert(0, wave);
        }
        root.required_capabilities = root.derived_capabilities();
        root.required_capabilities
            .insert(TargetCapability::BFloat16);
        module
    }

    #[test]
    fn root_wave_and_reconverged_call_require_the_exact_private_context() {
        for permutation in [[0, 1, 2, 3], [1, 0, 2, 3]] {
            for reconverge in [false, true] {
                with_owner(
                    &root_wave_source(permutation, reconverge),
                    Profile::Gfx942,
                    |owner, budget| {
                        let floor = budget.storage();
                        let ledger = budget.work_ledger_identity_v1();
                        let generic =
                        lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(owner)
                            .unwrap_err();
                        assert!(
                            generic.contains(LoweringDiagnosticCode::UnprovenBarrierConvergence)
                        );
                        assert!(generic.to_string().contains("CallWithoutSummary"));
                        let text = lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(
                            owner,
                            permutation,
                            budget,
                        )
                        .unwrap();
                        assert!(text.contains(AmdgcnIntrinsic::MfmaF32M16N16K16Bf16.llvm_name()));
                        assert!(text.contains("llvm.amdgcn.mbcnt.lo"));
                        assert!(text.contains("!\"multiple_defined_bodies\""));
                        assert_eq!(budget.storage(), floor);
                        assert!(budget.work_ledger_identity_v1() == ledger);
                        assert!(budget.check_prior_denials_v1().is_ok());
                        drop(text);
                    },
                );
            }
        }
    }

    #[test]
    fn helper_result_dependent_root_wave_control_still_refuses() {
        let mut module = root_wave_source([0, 1, 2, 3], false);
        let root = &mut module.functions[0];
        let body = root.body.as_mut().unwrap();
        let call = &mut body.blocks[0];
        call.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(20), Type::F32),
            OperationKind::Constant(Constant::F32Bits(0)),
        ));
        call.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(21), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(12),
                rhs: ValueId(20),
            },
        ));
        call.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(21),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        let mut varying_wave = BasicBlock::new(BlockId(1));
        varying_wave.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(22), Type::Scalar(ScalarType::U32)),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::LaneId,
                WaveWidth::Wave64,
            )),
        ));
        varying_wave.terminator = Some(Terminator::Branch {
            target: BlockId(3),
            arguments: vec![],
        });
        let mut other = BasicBlock::new(BlockId(2));
        other.terminator = Some(Terminator::Branch {
            target: BlockId(3),
            arguments: vec![],
        });
        let mut done = BasicBlock::new(BlockId(3));
        done.terminator = Some(Terminator::Return { values: vec![] });
        body.blocks.extend([varying_wave, other, done]);
        root.required_capabilities = root.derived_capabilities();
        root.required_capabilities
            .insert(TargetCapability::BFloat16);
        with_owner(&module, Profile::Gfx942, |owner, budget| {
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let result =
                lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(owner, [0, 1, 2, 3], budget);
            // Lowering proves that context derivation succeeded; this refusal
            // is the unchanged per-operation convergence requirement.
            assert!(matches!(&result,
                Err(PrivateBf16NativeLlvmErrorV1::Lowering(error))
                    if error.contains(LoweringDiagnosticCode::UnprovenBarrierConvergence)
                        && error.to_string().contains("uniform control, but analysis found Varying")));
            drop(result);
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(budget.check_prior_denials_v1().is_ok());
        });
    }

    #[test]
    fn root_call_diagnostic_binding_rejects_foreign_owners_functions_and_sites() {
        use fe2o3_kernel_analysis::{Diagnostic, UnsupportedReason};
        with_owner(
            &root_wave_source([0, 1, 2, 3], false),
            Profile::Gfx942,
            |first, budget| {
                let floor = budget.storage();
                let (second, storage) =
                    Owner::from_module_ref_with_verification_budget_v12(first.module(), budget)
                        .unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                let frame = bf16_native_frame_storage_v1();
                budget.reserve_storage(frame).unwrap();
                let context = derive_native_bf16_context_v1(first, [0, 1, 2, 3], budget).unwrap();
                let diagnostic = |block, operation_index, callee: &str| Diagnostic::Unsupported {
                    block,
                    operation_index,
                    reason: UnsupportedReason::CallWithoutSummary {
                        callee: FunctionId::new(callee),
                    },
                };
                let exact = diagnostic(
                    Some(context.call_block),
                    Some(context.call_ordinal),
                    context.helper.id.as_str(),
                );
                assert!(context.admits_root_call_diagnostic(first.module(), context.root, &exact));
                let foreign_root = &second.module().functions[0];
                for (module, function) in [
                    (second.module(), foreign_root),
                    (first.module(), foreign_root),
                    (second.module(), context.root),
                    (first.module(), context.helper),
                ] {
                    assert!(!context.admits_root_call_diagnostic(module, function, &exact));
                }
                for other in [
                    diagnostic(
                        Some(BlockId(99)),
                        Some(context.call_ordinal),
                        context.helper.id.as_str(),
                    ),
                    diagnostic(
                        Some(context.call_block),
                        Some(context.call_ordinal + 1),
                        context.helper.id.as_str(),
                    ),
                    diagnostic(
                        Some(context.call_block),
                        Some(context.call_ordinal),
                        "foreign_helper",
                    ),
                    diagnostic(None, Some(context.call_ordinal), context.helper.id.as_str()),
                    diagnostic(Some(context.call_block), None, context.helper.id.as_str()),
                    Diagnostic::Unsupported {
                        block: Some(context.call_block),
                        operation_index: Some(context.call_ordinal),
                        reason: UnsupportedReason::FunctionDeclaration,
                    },
                ] {
                    assert!(!context.admits_root_call_diagnostic(
                        first.module(),
                        context.root,
                        &other
                    ));
                }
                drop(context);
                budget.release_storage(frame).unwrap();
                drop(second);
                budget.release_storage(storage.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
                assert!(budget.check_prior_denials_v1().is_ok());
            },
        );
    }

    #[test]
    fn original_work_exact_one_short_and_prior_kind_precedence_do_not_reset_account() {
        let module = source([0, 1, 2, 3], false);
        let bound = crate::bind_production_target_v1(&module, Profile::Gfx942).unwrap();
        let mut exact = 0;
        for pass in 0..3 {
            let cap = if pass == 0 {
                1_000_000_000_000
            } else {
                exact - usize::from(pass == 2)
            };
            let mut work = Work::new(cap);
            let mut budget = Budget::new(&mut work, 128 * 1024 * 1024);
            budget.reserve_storage(7).unwrap();
            let (owner, storage) =
                Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut budget)
                    .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            budget
                .reserve_storage(MAX_COMPILER_MODULE_TEXT_BYTES)
                .unwrap();
            let floor = budget.storage();
            let result = lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(
                &owner,
                [0, 1, 2, 3],
                &mut budget,
            );
            assert_eq!(result.is_ok(), pass != 2);
            drop(result);
            if pass == 0 {
                exact = budget.work();
            }
            assert_eq!(budget.storage(), floor);
            if pass == 2 {
                let prior = budget.check_prior_denials_v1().unwrap_err();
                let before = (budget.work(), budget.failed_work(), budget.failed_storage());
                assert!(
                    matches!(lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(&owner,[0,1,2,3],&mut budget),
                    Err(PrivateBf16NativeLlvmErrorV1::Resource(error)) if error==prior)
                );
                assert_eq!(
                    (budget.work(), budget.failed_work(), budget.failed_storage()),
                    before
                );
            }
            budget
                .release_storage(MAX_COMPILER_MODULE_TEXT_BYTES)
                .unwrap();
            drop(owner);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), 7);
        }
        for both in [false, true] {
            with_owner(&module, Profile::Gfx942, |owner, budget| {
                assert!(budget.reserve_storage(128 * 1024 * 1024).is_err());
                budget
                    .charge_work(1_000_000_000_000 - budget.work())
                    .unwrap();
                if both {
                    assert!(budget.charge_work(9).is_err());
                }
                let prior = budget.check_prior_denials_v1().unwrap_err();
                assert_eq!(matches!(prior, Bf16NativeResourceV1::Work(_)), both);
                let before = (
                    budget.work(),
                    budget.storage(),
                    budget.failed_work(),
                    budget.failed_storage(),
                );
                for _ in 0..2 {
                    assert!(
                        matches!(lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(owner,[0,1,2,3],budget),
                        Err(PrivateBf16NativeLlvmErrorV1::Resource(error)) if error==prior)
                    );
                    assert_eq!(
                        (
                            budget.work(),
                            budget.storage(),
                            budget.failed_work(),
                            budget.failed_storage()
                        ),
                        before
                    );
                }
            });
        }
    }
}
