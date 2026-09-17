#[test]
fn static_publication_replay_rejects_well_typed_addresses_indices_and_producer_mutants() {
    for publish in [false, true] {
        for mutant in 0..4 {
            let mut owner = owner(publish, true);
            let RetainedProductionKirModuleV1::Legacy(module) = &mut owner.module else {
                panic!("legacy");
            };
            let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
            let zero = operations.iter().find_map(|op| {
                matches!(op.kind, OperationKind::Constant(Constant::Index(0)))
                    .then(|| op.results[0].id)
            });
            match mutant {
                0 => {
                    let gep = operations
                        .iter()
                        .position(|op| matches!(op.kind, OperationKind::GetElementPointer { .. }))
                        .unwrap();
                    let OperationKind::GetElementPointer { offset, .. } = operations[gep].kind
                    else {
                        unreachable!();
                    };
                    let definition = operations
                        .iter_mut()
                        .find(|op| op.results.iter().any(|result| result.id == offset))
                        .unwrap();
                    definition.kind = OperationKind::Constant(Constant::Index(0));
                }
                1 => {
                    let gep = operations
                        .iter()
                        .rposition(|op| matches!(op.kind, OperationKind::GetElementPointer { .. }))
                        .unwrap();
                    let OperationKind::GetElementPointer { base, .. } = operations[gep].kind else {
                        unreachable!();
                    };
                    let operation = operations
                        .iter_mut()
                        .find(|op| {
                            matches!(
                                op.kind,
                                OperationKind::Store { .. } | OperationKind::GuardedLoad { .. }
                            )
                        })
                        .unwrap();
                    match &mut operation.kind {
                        OperationKind::Store { pointer, .. }
                        | OperationKind::GuardedLoad { pointer, .. } => *pointer = base,
                        _ => unreachable!(),
                    }
                }
                2 if publish => {
                    let store = operations
                        .iter()
                        .position(|op| matches!(op.kind, OperationKind::Store { .. }))
                        .unwrap();
                    let ready = operations
                        .iter()
                        .position(|op| matches!(op.kind, OperationKind::Atomic(_)))
                        .unwrap();
                    operations.swap(store, ready);
                }
                2 => {
                    let OperationKind::GuardedLoad { pointer, .. } = operations
                        .iter()
                        .find(|op| matches!(op.kind, OperationKind::GuardedLoad { .. }))
                        .unwrap()
                        .kind
                    else {
                        unreachable!();
                    };
                    let gep = operations
                        .iter_mut()
                        .find(|op| op.results.iter().any(|result| result.id == pointer))
                        .unwrap();
                    let OperationKind::GetElementPointer { offset, .. } = &mut gep.kind else {
                        unreachable!();
                    };
                    *offset = zero.unwrap();
                }
                3 => {
                    let operation = operations
                        .iter_mut()
                        .find(|op| matches!(op.kind, OperationKind::Atomic(_)))
                        .unwrap();
                    let OperationKind::Atomic(atomic) = &mut operation.kind else {
                        unreachable!();
                    };
                    atomic.ordering = MemoryOrdering::Relaxed;
                }
                _ => unreachable!(),
            }
            verify_module(owner.module()).expect("mutant remains well typed");
            assert!(
                matches!(
                    owner.verify_equivalence(),
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                ),
                "publish={publish} mutant={mutant}"
            );
        }
    }
}

#[test]
fn static_publication_false_guard_never_reads_empty_or_uninitialized_payload() {
    use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV11;
    use fe2o3_kir_sim::{
        AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
        SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
    };
    let owner = owner(false, true);
    let target = SimulationTargetV1::amdgpu_64();
    for payload_bytes in [0, 4] {
        let request = SimulationRequestV1::new(
            owner.module().kernels[0].id.clone(),
            [128, 1, 1],
            [128, 1, 1],
            vec![
                SimulationArgumentV1::Buffer(
                    BufferArgumentV1::new(
                        ScalarType::F32,
                        AccessMode::ReadWrite,
                        4,
                        vec![0; payload_bytes],
                        vec![false; payload_bytes],
                        target,
                    )
                    .unwrap(),
                ),
                SimulationArgumentV1::Buffer(
                    BufferArgumentV1::new(
                        ScalarType::U32,
                        AccessMode::ReadWrite,
                        4,
                        2_u32.to_le_bytes().to_vec(),
                        vec![true; 4],
                        target,
                    )
                    .unwrap(),
                ),
                SimulationArgumentV1::Scalar(
                    ScalarBitsV1::new(ScalarType::U64, 0, target).unwrap(),
                ),
                SimulationArgumentV1::Scalar(
                    ScalarBitsV1::new(ScalarType::F32, 0, target).unwrap(),
                ),
            ],
        );
        for unguarded in [false, true] {
            let mut module = owner.module().clone();
            if unguarded {
                let operation = module.functions[0]
                    .body
                    .as_mut()
                    .unwrap()
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find(|op| matches!(op.kind, OperationKind::GuardedLoad { .. }))
                    .unwrap();
                let OperationKind::GuardedLoad {
                    pointer, access, ..
                } = operation.kind
                else {
                    unreachable!();
                };
                operation.kind = OperationKind::Load { pointer, access };
            }
            verify_module(&module).unwrap();
            let admitted = AdmittedSimulationModuleV1::admit_v11(
                VerifiedCanonicalKernelIrV11::from_module(module).unwrap(),
                SimulationLimitsV1::default(),
            )
            .unwrap();
            let result = admitted.simulate(&request, target, SimulationLimitsV1::default());
            assert_eq!(
                result.is_ok(),
                !unguarded,
                "payload_bytes={payload_bytes} unguarded={unguarded}: {result:?}"
            );
            if let Ok(result) = result {
                assert_eq!(result.buffer(1).unwrap().bytes(), 1_u32.to_le_bytes());
            }
        }
    }
}
