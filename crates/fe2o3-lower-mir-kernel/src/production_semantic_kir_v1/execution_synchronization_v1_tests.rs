use fe2o3_mir_model::semantic_mir_v1::*;

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}

fn place(local: u32, index: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty(index)).unwrap()
}

fn source_fixture() -> (SemanticCallableDeclV1, SemanticDirectCallV1) {
    let provenance = SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticKernelBindingIdentityV1::from_sha256([1; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([2; 32]),
        SemanticTypeIdentityV1::from_sha256([3; 32]),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([4; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([5; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([6; 32]),
    )
    .unwrap();
    let contract = SemanticExecutionCapabilityContractV1::new(
        SemanticExecutionCapabilityOperationV1::WorkgroupBarrier {
            input_workgroup: ty(1),
            output_workgroup: ty(2),
            semantics: SemanticExecutionMemorySemanticsV1::new(
                SemanticExecutionMemoryScopeV1::Workgroup,
                SemanticExecutionMemoryOrderingV1::AcquireRelease,
                SemanticExecutionMemorySpacesV1::Workgroup,
            ),
        },
        SemanticExecutionCapabilitySignatureV1::new(&[ty(1)], ty(2)).unwrap(),
        provenance,
        SemanticTypeIdentityV1::from_sha256([7; 32]),
        SemanticTypeIdentityV1::from_sha256([8; 32]),
        Some(SemanticTypeIdentityV1::from_sha256([9; 32])),
        SemanticFunctionIdentityV1::from_sha256([10; 32]),
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([11; 32]),
        SemanticLayoutIdentityV1::from_sha256([12; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![SemanticAbiValueV1::new(
            ty(1),
            SemanticAbiPassModeV1::Ignore,
        )],
        SemanticAbiValueV1::new(ty(2), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let callable = SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            contract.source_identity(),
            SemanticItemDefinitionIdentityV1::from_sha256([13; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([14; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([15; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([16; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([17; 32]),
    };
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(0),
        vec![SemanticOperandV1::Move(place(1, 1))],
        Some(SemanticCallDestinationV1::new(
            place(2, 2),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    (callable, call)
}

fn capability() -> ExecutionCapabilityOpV1 {
    let id = |byte| ExecutionTypeIdentityV1::new([byte; 32]);
    let operation = ExecutionCapabilityOperationV1::WorkgroupBarrier {
        input_workgroup: id(1),
        output_workgroup: id(2),
        semantics: ExecutionMemorySemanticsV1 {
            scope: ExecutionMemoryScopeV1::Workgroup,
            ordering: ExecutionMemoryOrderingV1::AcquireRelease,
            spaces: ExecutionMemorySpacesV1::Workgroup,
        },
    };
    let result = ExecutionCapabilityOpV1 {
        operands: vec![ValueId(1)],
        signature: ExecutionCapabilitySignatureV1::new(&[id(1)], id(2)).unwrap(),
        provenance: ExecutionCapabilityProvenanceV1 {
            root: FunctionId::new("sync87"),
            kernel_binding: [1; 32],
            frontend_unit: [2; 32],
            kernel_marker: [3; 32],
            target_brand: [4; 32],
            launch_brand: [5; 32],
            issuance: [6; 32],
        },
        workgroup_brand: Some([7; 32]),
        epoch_before: Some([8; 32]),
        epoch_after: Some([9; 32]),
        obligations: ExecutionSafetyObligationsV1::from_bits(
            fe2o3_kernel_ir::required_execution_obligations_v1(&operation),
        ),
        source: ExecutionCapabilitySourceV1 {
            function: [10; 32],
            operation: [11; 32],
            block: 12,
            occurrence: None,
        },
        operation,
    };
    assert!(result.is_complete());
    result
}

fn body(capabilities: Vec<ExecutionCapabilityOpV1>) -> FunctionBody {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = capabilities
        .into_iter()
        .map(|capability| Operation::new(vec![], OperationKind::ExecutionCapability(capability)))
        .collect();
    block.terminator = Some(Terminator::Return { values: vec![] });
    Function::kernel_entry(
        "sync87",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    )
    .body
    .unwrap()
}

#[test]
fn exact_source_barrier_and_kir_normalize_to_the_same_ranked_contract() {
    let (callable, call) = source_fixture();
    let projected = project_execution_workgroup_barrier_v1(&callable, &call)
        .unwrap()
        .unwrap();
    let kernel = fe2o3_pliron::ProductionRankedKernelV1::new(
        "sync87",
        0,
        vec![fe2o3_pliron::ProductionRankedBlockV1::new(
            vec![projected],
            fe2o3_pliron::ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    assert_eq!(
        kir_synchronization_contracts_v1(&body(vec![capability()])).unwrap(),
        ranked_synchronization_contracts_v1(&kernel).unwrap(),
    );
    assert_eq!(
        kir_contract(&capability()).unwrap(),
        Some(NormalizedSynchronizationV1 {
            execution_scope: Some(2),
            memory_scope: 2,
            ordering: 3,
            address_space: 1,
        })
    );
}

#[test]
fn shared_classifier_accepts_only_exact_scope_order_and_single_workgroup_space() {
    for scope in [
        ExecutionMemoryScopeV1::System,
        ExecutionMemoryScopeV1::Device,
        ExecutionMemoryScopeV1::Workgroup,
        ExecutionMemoryScopeV1::Subgroup,
    ] {
        for ordering in [
            ExecutionMemoryOrderingV1::Relaxed,
            ExecutionMemoryOrderingV1::Acquire,
            ExecutionMemoryOrderingV1::Release,
            ExecutionMemoryOrderingV1::AcquireRelease,
            ExecutionMemoryOrderingV1::SequentiallyConsistent,
        ] {
            for spaces in [
                ExecutionMemorySpacesV1::Global,
                ExecutionMemorySpacesV1::Workgroup,
                ExecutionMemorySpacesV1::GlobalAndWorkgroup,
            ] {
                let semantics = ExecutionMemorySemanticsV1 {
                    scope,
                    ordering,
                    spaces,
                };
                let accepted = scope == ExecutionMemoryScopeV1::Workgroup
                    && ordering == ExecutionMemoryOrderingV1::AcquireRelease
                    && spaces == ExecutionMemorySpacesV1::Workgroup;
                assert_eq!(require_workgroup_semantics(semantics).is_ok(), accepted);
                let mut changed = capability();
                let ExecutionCapabilityOperationV1::WorkgroupBarrier {
                    semantics: target, ..
                } = &mut changed.operation
                else {
                    unreachable!()
                };
                *target = semantics;
                assert_eq!(kir_contract(&changed).is_ok(), accepted);
            }
        }
    }
}

#[test]
fn source_projection_rejects_missing_or_substituted_signature_and_binding() {
    let (callable, call) = source_fixture();
    for (arguments, destination) in [
        (vec![], call.destination().cloned()),
        (
            vec![SemanticOperandV1::Copy(place(1, 2))],
            call.destination().cloned(),
        ),
        (call.arguments().to_vec(), None),
        (
            call.arguments().to_vec(),
            Some(SemanticCallDestinationV1::new(
                place(2, 1),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(1),
                ),
            )),
        ),
    ] {
        let changed = SemanticDirectCallV1::new_callable(
            call.callee(),
            arguments,
            destination,
            call.unwind(),
        )
        .unwrap();
        assert!(project_execution_workgroup_barrier_v1(&callable, &changed).is_err());
    }
    let SemanticCallableDeclV1::CompilerIntrinsic {
        binding,
        operation,
        operation_identity,
    } = &callable
    else {
        unreachable!()
    };
    let changed = SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([99; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([13; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([14; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([15; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([16; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            binding.abi().clone(),
        ),
        operation: operation.clone(),
        operation_identity: *operation_identity,
    };
    assert!(project_execution_workgroup_barrier_v1(&changed, &call).is_err());
    assert!(
        project_execution_workgroup_barrier_v1(&callable, &call)
            .unwrap()
            .is_some()
    );
}

#[test]
fn unclassified_capability_synchronization_and_incomplete_contract_still_reject() {
    let mut fence = capability();
    let ExecutionCapabilityOperationV1::WorkgroupBarrier {
        semantics,
        input_workgroup,
        output_workgroup,
    } = fence.operation
    else {
        unreachable!()
    };
    fence.operation = ExecutionCapabilityOperationV1::WorkgroupFence {
        workgroup: input_workgroup,
        result: output_workgroup,
        semantics,
    };
    assert_eq!(kir_contract(&fence).unwrap(), None);
    assert!(matches!(
        kir_synchronization_contracts_v1(&body(vec![fence])),
        Err(ProductionMirPlironTranslationErrorV1::SynchronizationMismatch),
    ));
    for field in 0..4 {
        let mut changed = capability();
        match field {
            0 => changed.source.function = [0; 32],
            1 => changed.workgroup_brand = None,
            2 => changed.epoch_after = changed.epoch_before,
            _ => changed.obligations = ExecutionSafetyObligationsV1::from_bits(0),
        }
        assert!(kir_contract(&changed).is_err());
    }
}

#[test]
fn normalization_keeps_missing_extra_and_duplicate_barriers_observable() {
    let one = kir_synchronization_contracts_v1(&body(vec![capability()])).unwrap();
    let none = kir_synchronization_contracts_v1(&body(vec![])).unwrap();
    let two = kir_synchronization_contracts_v1(&body(vec![capability(), capability()])).unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(two.len(), 2);
    assert_ne!(one, none);
    assert_ne!(one, two);
}

#[test]
fn source_projection_rejects_global_combined_spaces_and_other_sync_kinds() {
    for case in 0..3 {
        let (mut callable, call) = source_fixture();
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ..
        } = &mut callable
        else {
            unreachable!()
        };
        let semantics = SemanticExecutionMemorySemanticsV1::new(
            SemanticExecutionMemoryScopeV1::Workgroup,
            SemanticExecutionMemoryOrderingV1::AcquireRelease,
            match case {
                0 => SemanticExecutionMemorySpacesV1::Global,
                1 => SemanticExecutionMemorySpacesV1::GlobalAndWorkgroup,
                _ => SemanticExecutionMemorySpacesV1::Workgroup,
            },
        );
        let operation = if case == 2 {
            SemanticExecutionCapabilityOperationV1::WorkgroupFence {
                workgroup: ty(1),
                result: ty(2),
                semantics,
            }
        } else {
            SemanticExecutionCapabilityOperationV1::WorkgroupBarrier {
                input_workgroup: ty(1),
                output_workgroup: ty(2),
                semantics,
            }
        };
        *contract = SemanticExecutionCapabilityContractV1::new(
            operation,
            contract.signature(),
            contract.provenance(),
            contract.workgroup_brand().unwrap(),
            contract.epoch_before().unwrap(),
            if case == 2 {
                None
            } else {
                contract.epoch_after()
            },
            contract.source_identity(),
        )
        .unwrap();
        assert!(project_execution_workgroup_barrier_v1(&callable, &call).is_err());
    }
}

#[test]
fn legacy_and_unrelated_intrinsics_do_not_acquire_capability_barrier_classification() {
    let (mut callable, call) = source_fixture();
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut callable else {
        unreachable!()
    };
    *operation = SemanticCompilerIntrinsicOperationV1::WorkgroupBarrier;
    assert!(
        project_execution_workgroup_barrier_v1(&callable, &call)
            .unwrap()
            .is_none()
    );
}
