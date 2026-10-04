use fe2o3_kernel_ir::*;
use fe2o3_kernel_opt::*;

fn scalar_module() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(7)),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("inert_v12_optimizer");
    module.functions.push(Function::internal_helper(
        "helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module
}

fn carrier_modules() -> Vec<Module> {
    let vector = FixedVectorTypeV12::new(ScalarType::F32, 4, VectorLayoutV12::Contiguous);
    let access = VectorMemoryAccessV12::new(vector, MemoryAccess::new(AddressSpace::Global, 16));
    let kinds = [
        OperationKind::VectorLoad(VectorLoadOperationV12::new(ValueId(0), access)),
        OperationKind::VectorStore(VectorStoreOperationV12::new(ValueId(0), ValueId(0), access)),
        OperationKind::VectorLayoutConvert(VectorLayoutConversionV12::new(
            ValueId(0),
            VectorLayoutV12::Interleaved { factor: 2 },
        )),
        OperationKind::VerificationContract(
            VerificationContractOperationV12::WorkgroupPipelineEvent {
                contract: VerificationContractKeyV12::new(0),
                kind: WorkgroupPipelineEventKindV12::Stage,
                storage: ValueId(0),
                epoch: ValueId(0),
            },
        ),
    ];
    let mut modules = vec![];
    for kind in kinds {
        for dead in [false, true] {
            let mut module = scalar_module();
            let blocks = &mut module.functions[0].body.as_mut().unwrap().blocks;
            if dead {
                let mut block = BasicBlock::new(BlockId(9));
                block.terminator = Some(Terminator::Return { values: vec![] });
                blocks.push(block);
            }
            blocks
                .last_mut()
                .unwrap()
                .operations
                .push(Operation::new(vec![], kind.clone()));
            modules.push(module);
        }
    }
    let mut module = scalar_module();
    module.functions.push(Function::external_import(
        "unused_vector",
        Signature::new(
            vec![Type::slice(
                Type::vector(vector),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            )],
            vec![],
        ),
    ));
    modules.push(module);
    modules
}

#[test]
fn old_optimizer_entries_reject_v12_before_any_transform() {
    for module in carrier_modules() {
        let before = module.clone();
        let limits = KernelIrPlironOptimizationLimitsV2::default();
        for result in [
            optimize_production_kernel_ir_module_v2(&module),
            optimize_kernel_ir_module_v2(&module, limits),
            optimize_kernel_ir_module_at_epoch_v2(&module, 9, limits),
        ] {
            assert!(matches!(
                result,
                Err(KernelIrPlironOptimizationErrorV2::InputEncoding(
                    KernelIrEncodeError::UnsupportedInVersion { version: 10, .. },
                ))
            ));
        }
        for result in [
            optimize_production_kernel_ir_module_v3(&module),
            optimize_kernel_ir_module_v3(&module, limits),
            optimize_kernel_ir_module_at_epoch_v3(&module, 9, limits),
        ] {
            assert!(matches!(
                result,
                Err(KernelIrPlironOptimizationErrorV3::InputEncoding(
                    KernelIrEncodeError::UnsupportedInVersion { version: 11, .. },
                ))
            ));
        }
        assert_eq!(module, before);
    }
}

#[test]
fn genuine_production_reports_cannot_authorize_v12_pre_or_post_states() {
    let scalar = scalar_module();
    let v2 = optimize_production_kernel_ir_module_v2(&scalar).unwrap();
    let v3 = optimize_production_kernel_ir_module_v3(&scalar).unwrap();
    assert!(v2.report().is_production_replay_compatible());
    assert!(v3.report().is_production_replay_compatible());
    let _v2_admission =
        admit_production_kernel_ir_structural_replay_v2(&scalar, v2.module(), v2.report()).unwrap();
    let _v3_admission =
        admit_production_kernel_ir_structural_replay_v3(&scalar, v3.module(), v3.report()).unwrap();
    for module in carrier_modules() {
        assert!(matches!(
            admit_production_kernel_ir_structural_replay_v2(&module, v2.module(), v2.report()),
            Err(KernelIrPlironStructuralReplayAdmissionErrorV2::Replay(
                KernelIrPlironOptimizationErrorV2::InputEncoding(
                    KernelIrEncodeError::UnsupportedInVersion { version: 10, .. },
                ),
            )),
        ));
        assert!(matches!(
            admit_production_kernel_ir_structural_replay_v3(&module, v3.module(), v3.report()),
            Err(KernelIrPlironStructuralReplayAdmissionErrorV3::Replay(
                KernelIrPlironOptimizationErrorV3::InputEncoding(
                    KernelIrEncodeError::UnsupportedInVersion { version: 11, .. },
                ),
            )),
        ));
        assert!(matches!(
            admit_production_kernel_ir_structural_replay_v2(&scalar, &module, v2.report()),
            Err(KernelIrPlironStructuralReplayAdmissionErrorV2::OutputMismatch),
        ));
        assert!(matches!(
            admit_production_kernel_ir_structural_replay_v3(&scalar, &module, v3.report()),
            Err(KernelIrPlironStructuralReplayAdmissionErrorV3::OutputMismatch),
        ));
    }
}
