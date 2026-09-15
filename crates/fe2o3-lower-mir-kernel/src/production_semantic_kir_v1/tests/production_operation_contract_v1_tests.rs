mod operation_contract_tests_v1 {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    include!("production_operation_contract_legacy_oracle_v1_tests.rs");

    fn body(operations: Vec<Operation>) -> FunctionBody {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = operations;
        block.terminator = Some(Terminator::Return { values: vec![] });
        Function::kernel_entry(
            "contract_component",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        )
        .body
        .unwrap()
    }
    fn ranked(
        operations: Vec<ProductionRankedOperationV1>,
    ) -> fe2o3_pliron::ProductionRankedKernelV1 {
        fe2o3_pliron::ProductionRankedKernelV1::new(
            "contract_component",
            0,
            vec![fe2o3_pliron::ProductionRankedBlockV1::new(
                operations,
                fe2o3_pliron::ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap()
    }
    fn barrier() -> Operation {
        Operation::new(
            vec![],
            OperationKind::WorkgroupBarrier(fe2o3_kernel_ir::WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
                convergence: fe2o3_kernel_ir::Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        )
    }
    fn ranked_barrier() -> ProductionRankedOperationV1 {
        ProductionRankedOperationV1::Barrier {
            execution_scope: dialect_gpu::HierarchyAttr::Workgroup,
            memory_scope: dialect_gpu::MemoryScopeAttr::Workgroup,
            address_space: dialect_gpu::AddressSpaceAttr::Workgroup,
            order: dialect_gpu::MemoryOrderAttr::AcquireRelease,
        }
    }

    #[test]
    fn contract_factor_matches_frozen_legacy_payload_refusals_and_sorting() {
        for scope in [
            SynchronizationScope::Invocation,
            SynchronizationScope::Subgroup,
            SynchronizationScope::Workgroup,
            SynchronizationScope::Device,
            SynchronizationScope::System,
        ] {
            for ordering in [
                MemoryOrdering::Relaxed,
                MemoryOrdering::Acquire,
                MemoryOrdering::Release,
                MemoryOrdering::AcquireRelease,
                MemoryOrdering::SequentiallyConsistent,
            ] {
                for spaces in [
                    vec![],
                    vec![AddressSpace::Private],
                    vec![AddressSpace::Workgroup],
                    vec![AddressSpace::Global],
                    vec![AddressSpace::Constant],
                    vec![AddressSpace::Generic],
                    vec![AddressSpace::Global, AddressSpace::Workgroup],
                ] {
                    let fixture = body(vec![
                        barrier(),
                        Operation::new(
                            vec![],
                            OperationKind::Barrier(fe2o3_kernel_ir::Barrier {
                                execution_scope: scope,
                                memory_scope: scope,
                                semantics: BarrierSemantics::new(ordering, spaces.clone()),
                            }),
                        ),
                        Operation::new(
                            vec![],
                            OperationKind::Fence(fe2o3_kernel_ir::Fence {
                                memory_scope: scope,
                                semantics: BarrierSemantics::new(ordering, spaces),
                            }),
                        ),
                        barrier(),
                    ]);
                    assert_eq!(
                        format!("{:?}", kir_synchronization_contracts_v1(&fixture)),
                        format!(
                            "{:?}",
                            frozen_operation_contract_legacy_v1::kir_synchronization_contracts_v1(
                                &fixture
                            )
                        )
                    );
                }
            }
        }
        let publish = Operation::new(
            vec![],
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
                Gfx950LdsTransposeOperationKindV1::Publish {
                    format: Gfx950LdsTransposeFormatV1::Fp8E4M3,
                    storage: ValueId(0),
                },
            )),
        );
        let fixture = body(vec![publish, barrier()]);
        assert_eq!(
            format!("{:?}", kir_synchronization_contracts_v1(&fixture)),
            format!(
                "{:?}",
                frozen_operation_contract_legacy_v1::kir_synchronization_contracts_v1(&fixture)
            )
        );
        let ranked = ranked(vec![
            ranked_barrier(),
            ProductionRankedOperationV1::Fence {
                memory_scope: dialect_gpu::MemoryScopeAttr::Device,
                address_space: dialect_gpu::AddressSpaceAttr::Global,
                order: dialect_gpu::MemoryOrderAttr::Release,
            },
            ranked_barrier(),
        ]);
        assert_eq!(
            format!("{:?}", ranked_synchronization_contracts_v1(&ranked)),
            format!(
                "{:?}",
                frozen_operation_contract_legacy_v1::ranked_synchronization_contracts_v1(&ranked)
            )
        );
    }

    #[test]
    fn tensor_factor_preserves_full_records_and_legacy_rejections() {
        let base = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64();
        let mut layouts = vec![
            base,
            TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64_lds_xor4(),
            TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_f32_m16n16k128_wave64(),
            TensorLayoutContractV1::gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64(),
            TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_fp8_e4m3_f32_m16n16k128_wave64(),
        ];
        let mut changed = base;
        changed.subgroup_width = 32;
        layouts.push(changed);
        let mut changed = base;
        changed.a.shape[0] += 1;
        layouts.push(changed);
        let mut changed = base;
        changed.b.fragment_elements += 1;
        layouts.push(changed);
        let mut changed = base;
        changed.accumulator.shape[1] += 1;
        layouts.push(changed);
        let mut changed = base;
        changed.a.mapping = fe2o3_kernel_ir::TensorSymbolicMapV1::Opaque(9);
        layouts.push(changed);
        let mut changed = base;
        changed.tail_mask = fe2o3_kernel_ir::TensorTailMaskV1::Missing;
        layouts.push(changed);
        let mut changed = base;
        changed.a.role = fe2o3_kernel_ir::TensorOperandRoleV1::B;
        layouts.push(changed);
        let mut changed = base;
        changed.b.element = fe2o3_kernel_ir::MatrixElement::F32;
        layouts.push(changed);
        let mut changed = base;
        changed.b.multiplicity = fe2o3_kernel_ir::TensorMultiplicityV1::Broadcast { factor: 7 };
        layouts.push(changed);
        let mut changed = base;
        changed.accumulator.packing = fe2o3_kernel_ir::TensorElementPackingV1::Unsupported(9);
        layouts.push(changed);
        for layout in layouts {
            let mut matrix = MatrixOperation::multiply_accumulate(
                [ValueId(0); 4],
                [ValueId(1); 4],
                [ValueId(2); 4],
            );
            matrix.tensor_layout = Some(layout);
            let fixture = body(vec![Operation::new(vec![], OperationKind::Matrix(matrix))]);
            assert_eq!(
                format!("{:?}", kir_tensor_contracts_v1(&fixture)),
                format!(
                    "{:?}",
                    frozen_operation_contract_legacy_v1::kir_tensor_contracts_v1(&fixture)
                )
            );
            let extracted = kir_tensor_contracts_v1(&fixture).unwrap()[0];
            assert_eq!(extracted.contract, layout);
            let mut work = Work::new(10_000);
            let mut budget = Budget::new(&mut work, 0);
            let expected = NormalizedTensorV1 {
                contract: base,
                active_lanes: 64,
                convergence: 1,
            };
            assert_eq!(
                source_output_contracts_equal_v1(&mut [extracted], &mut [expected], &mut budget)
                    .unwrap(),
                layout == base
            );
        }
        for (active_lanes, convergence) in [(32, 1), (64, 2)] {
            let expected = NormalizedTensorV1 {
                contract: base,
                active_lanes: 64,
                convergence: 1,
            };
            let changed = NormalizedTensorV1 {
                contract: base,
                active_lanes,
                convergence,
            };
            let mut work = Work::new(10_000);
            let mut budget = Budget::new(&mut work, 0);
            assert!(
                !source_output_contracts_equal_v1(&mut [expected], &mut [changed], &mut budget)
                    .unwrap()
            );
        }
        for convergence in [
            dialect_kernel::TensorConvergenceAttr::UniformSubgroup,
            dialect_kernel::TensorConvergenceAttr::UniformWorkgroup,
            dialect_kernel::TensorConvergenceAttr::Divergent,
            dialect_kernel::TensorConvergenceAttr::Opaque,
        ] {
            let fixture = ranked(vec![ProductionRankedOperationV1::TensorLayout {
                contract: base,
                convergence,
                active_lanes: 64,
                binding: None,
            }]);
            assert_eq!(
                format!("{:?}", ranked_tensor_contracts_v1(&fixture)),
                format!(
                    "{:?}",
                    frozen_operation_contract_legacy_v1::ranked_tensor_contracts_v1(&fixture)
                )
            );
        }
    }

    #[test]
    fn tensor_kind_target_eligibility_does_not_relabel_instruction_profiles() {
        let bf16 =
            MatrixOperation::multiply_accumulate([ValueId(0); 4], [ValueId(1); 4], [ValueId(2); 4])
                .with_declared_tensor_layout(
                    TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
                );
        let fp4 = MatrixOperation::scaled_multiply_accumulate_fp4_e2m1(
            [ValueId(0); 8],
            [ValueId(1); 8],
            [ValueId(2); 4],
        );
        let fp4 = fp4.with_declared_tensor_layout(
            TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_f32_m16n16k128_wave64(),
        );
        let fp8 = MatrixOperation::scaled_multiply_accumulate_fp8_e4m3(
            [ValueId(0); 8],
            [ValueId(1); 8],
            [ValueId(2); 4],
        );
        let fp8 = fp8.with_declared_tensor_layout(
            TensorLayoutContractV1::gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64(),
        );
        for (matrix, scaled) in [
            (bf16.clone(), false),
            (fp4.clone(), true),
            (fp8.clone(), true),
        ] {
            let operation = Operation::new(vec![], OperationKind::Matrix(matrix));
            assert!(source_output_tensor_kind_v1(
                &operation,
                SourceOutputContractTargetV1::Gfx950
            ));
            assert_eq!(
                source_output_tensor_kind_v1(&operation, SourceOutputContractTargetV1::Gfx942),
                !scaled
            );
        }
        let mut wrong = bf16;
        wrong.tensor_layout = fp4.tensor_layout;
        assert!(!source_output_tensor_kind_v1(
            &Operation::new(vec![], OperationKind::Matrix(wrong)),
            SourceOutputContractTargetV1::Gfx950
        ));
        let mut wrong = fp4;
        wrong.tensor_layout = fp8.tensor_layout;
        assert!(!source_output_tensor_kind_v1(
            &Operation::new(vec![], OperationKind::Matrix(wrong)),
            SourceOutputContractTargetV1::Gfx950
        ));
    }

    #[test]
    fn exact_target_contract_refuses_missing_foreign_duplicate_and_wrong_wave() {
        use fe2o3_kernel_ir::TargetCapability as C;
        let target = fe2o3_kernel_ir::gfx942_xnack_minus_target_capability();
        let other = fe2o3_kernel_ir::gfx950_xnack_minus_target_capability();
        let wave = C::WaveWidth(fe2o3_kernel_ir::WaveWidth::Wave64);
        let foreign = C::Extension {
            namespace: fe2o3_kernel_ir::AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.to_owned(),
            name: "gfx942:xnack+".to_owned(),
        };
        for (capabilities, accepted) in [
            (vec![target.clone(), wave.clone()], true),
            (vec![other.clone(), wave.clone()], true),
            (vec![target.clone()], false),
            (vec![wave.clone()], false),
            (vec![foreign, wave.clone()], false),
            (vec![target.clone(), other, wave], false),
            (
                vec![target, C::WaveWidth(fe2o3_kernel_ir::WaveWidth::Wave32)],
                false,
            ),
        ] {
            let mut work = Work::new(10_000);
            let mut budget = Budget::new(&mut work, 0);
            assert_eq!(
                source_output_contract_target_v1(&capabilities.into_iter().collect(), &mut budget)
                    .is_ok(),
                accepted
            );
            assert_eq!(budget.storage(), 0);
        }
    }

    #[test]
    fn paid_multiset_comparison_preserves_duplicates_but_grants_no_order() {
        let a = kir_synchronization_contract_v1(&barrier())
            .unwrap()
            .unwrap();
        let b = NormalizedSynchronizationV1 {
            execution_scope: None,
            ..a
        };
        let mut work = Work::new(10_000);
        let mut budget = Budget::new(&mut work, 0);
        assert!(
            source_output_contracts_equal_v1(&mut [a, b, a], &mut [b, a, a], &mut budget).unwrap()
        );
        assert!(
            !source_output_contracts_equal_v1(&mut [a, b, a], &mut [b, b, a], &mut budget).unwrap()
        );
        assert!(
            !source_output_contracts_equal_v1(&mut [a, b, a], &mut [b, a], &mut budget).unwrap()
        );
    }

    #[test]
    fn paid_contract_pair_has_independent_exact_under_and_storage_boundaries() {
        let fixture = body(vec![barrier()]);
        let ranked = ranked(vec![ranked_barrier()]);
        let record = std::mem::size_of::<NormalizedSynchronizationV1>();
        let headers = 2 * std::mem::size_of::<Vec<NormalizedSynchronizationV1>>();
        // KIR 1+8+1+2, ranked 1+8+2, compare 1+(1+record).
        let exact = 25 + record;
        for (available, storage) in [
            (exact, headers + 8 * record),
            (exact - 1, headers + 8 * record),
            (exact, headers + 8 * record - 1),
        ] {
            let mut work = Work::new(7 + available);
            let mut budget = Budget::new(&mut work, 23 + storage);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(23).unwrap();
            let result = source_output_global_scratch_scope_v1(&mut budget, |budget| {
                budget
                    .reserve_storage(headers)
                    .map_err(ProductionSourceOutputErrorV1::Resource)?;
                let mut a = Vec::new();
                let mut b = Vec::new();
                visit_kir_synchronization_contracts_v1(
                    &fixture,
                    &mut SourceOutputOperationContractSinkV1 {
                        values: &mut a,
                        budget,
                    },
                )?;
                visit_ranked_synchronization_contracts_v1(
                    &ranked,
                    &mut SourceOutputOperationContractSinkV1 {
                        values: &mut b,
                        budget,
                    },
                )?;
                assert!(source_output_contracts_equal_v1(&mut a, &mut b, budget)?);
                Ok(())
            });
            assert_eq!(
                result.is_ok(),
                available == exact && storage == headers + 8 * record
            );
            assert_eq!(budget.storage(), 23);
            if available == exact - 1 {
                assert_eq!(budget.work(), 7 + 24);
                assert_eq!(work.failed_work(), Some(7 + exact));
            }
        }
    }

    fn growth_and_cleanup<T: Copy>(value: T) {
        let header = std::mem::size_of::<Vec<T>>();
        let record = std::mem::size_of::<T>();
        for unwind in [false, true] {
            let mut work = Work::new(1000);
            let mut budget = Budget::new(&mut work, 23 + header + 12 * record);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(23).unwrap();
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                source_output_global_scratch_scope_v1(&mut budget, |budget| {
                    budget
                        .reserve_storage(header)
                        .map_err(ProductionSourceOutputErrorV1::Resource)?;
                    let mut values = Vec::new();
                    for _ in 0..5 {
                        SourceOutputOperationContractSinkV1 {
                            values: &mut values,
                            budget,
                        }
                        .emit(value)?;
                    }
                    assert_eq!(budget.work(), 18);
                    assert_eq!(budget.peak_storage(), 23 + header + 12 * record);
                    assert!(!unwind, "paid contract scratch unwind");
                    Err::<(), _>(ProductionSourceOutputErrorV1::Invalid(
                        "paid contract scratch refusal",
                    ))
                })
            }));
            assert_eq!(outcome.is_err(), unwind);
            assert_eq!(budget.storage(), 23);
            budget.charge_work(1).unwrap();
            budget.reserve_storage(1).unwrap();
            budget.release_storage(1).unwrap();
            assert_eq!(budget.work(), 19);
            assert_eq!(work.failed_work(), None);
        }
    }

    #[test]
    fn both_record_arenas_charge_relocation_and_restore_floor_on_error_and_unwind() {
        growth_and_cleanup(
            kir_synchronization_contract_v1(&barrier())
                .unwrap()
                .unwrap(),
        );
        growth_and_cleanup(NormalizedTensorV1 {
            contract: TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
            active_lanes: 64,
            convergence: 1,
        });
    }
}
