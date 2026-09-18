//! Native canonical-owner CPU controls. These are not source-produced captures,
//! GPU executions, or physical-register/lifetime observations.

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, AssemblySourceIdentity, BasicBlock, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1, Function,
    Gfx942OrderedRegionRegistersV1, Gfx942OrderedRegionV1, IntrinsicOperation, Kernel,
    LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType,
    Signature, TargetCapability, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV16, VerifiedCanonicalKernelIrV7, VerifiedCanonicalKernelIrV9,
    VerifiedCanonicalKernelIrV10, VerifiedCanonicalKernelIrV11, VerifiedCanonicalKernelIrV12,
    WaveWidth, WorkgroupSize, gfx950_xnack_minus_target_capability,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, IndexWidthV1, ScalarBitsV1,
    SimulationAdmissionErrorV1, SimulationArgumentV1, SimulationCapabilityDispositionV1,
    SimulationCapabilityProfileV1, SimulationDebugCaptureLimitsV1,
    SimulationDebugCheckpointPhaseV1, SimulationDebugCollectionV1, SimulationDebugRecordKindV1,
    SimulationDebugRecordV1, SimulationDebugSinkControlV1, SimulationDebugSinkV1,
    SimulationDebugValueV1, SimulationEventKindV1, SimulationEventSinkErrorV1,
    SimulationEventSinkV1, SimulationEventV1, SimulationKirWireVersionV1, SimulationLimitsV1,
    SimulationOperationSurfaceV1, SimulationPreflightErrorV1, SimulationRequestV1,
    SimulationSemanticOwnerV1, SimulationTargetV1, UnsupportedFeatureV1,
    semantic_capability_matrix_v1,
};

fn module(registers: [u8; 5]) -> Module {
    let u32_type = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(
        u32_type.clone(),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let region = Gfx942OrderedRegionV1::new(
        AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
        Gfx942OrderedRegionRegistersV1::new(
            registers[0],
            registers[1],
            registers[2..].try_into().unwrap(),
        )
        .unwrap(),
        [ValueId(0), ValueId(1), ValueId(2)],
    )
    .unwrap();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(4), u32_type.clone()),
            OperationKind::Gfx942OrderedRegion(region),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), pointer.clone()),
            OperationKind::GetElementPointer {
                base: ValueId(3),
                offset: ValueId(5),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(6),
                value: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::kernel_entry(
        "region_entry",
        Signature::new(
            vec![u32_type.clone(), u32_type.clone(), u32_type, pointer],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    );
    function.required_capabilities = function.derived_capabilities();
    let mut kernel = Kernel::new(
        "region",
        "region_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = function.required_capabilities.clone();
    let mut module = Module::new("native-cpu-ordered-region-v16");
    module.required_capabilities = function.required_capabilities.clone();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn owner(module: &Module) -> VerifiedCanonicalKernelIrModuleV16 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV16::from_module_ref_with_verification_budget_v16(
            module,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    owner
}

fn request(inputs: [u32; 3], invocations: u64) -> SimulationRequestV1 {
    let target = SimulationTargetV1::amdgpu_64();
    let mut arguments: Vec<_> = inputs
        .into_iter()
        .map(|value| SimulationArgumentV1::Scalar(ScalarBitsV1::u32(value)))
        .collect();
    let length = usize::try_from(invocations).unwrap() * 4 + 8;
    arguments.push(SimulationArgumentV1::Buffer(
        BufferArgumentV1::new(
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            vec![0x5a; length],
            vec![false; length],
            target,
        )
        .unwrap(),
    ));
    SimulationRequestV1::new("region", [invocations, 1, 1], [64, 1, 1], arguments)
}

fn admit(owner: &VerifiedCanonicalKernelIrModuleV16) -> AdmittedSimulationModuleV1 {
    AdmittedSimulationModuleV1::admit_v16(owner, SimulationLimitsV1::default()).unwrap()
}

#[test]
fn borrowed_exact_v16_identity_and_normal_engine_results_preserve_inputs_and_canaries() {
    let cases = [
        ([7, 11, 13], 25),
        ([u32::MAX, 0, 1], 0),
        ([0x7fff_ffff, 0, 1], 0x8000_0000),
        ([0xa5a5_a5a5, 0x5a5a_5a5a, 2], 1),
        ([u32::MAX, u32::MAX, u32::MAX], u32::MAX),
        ([0, 0, 0], 0),
    ];
    for registers in [[32, 33, 34, 35, 36], [0, 63, 1, 62, 2], [63, 0, 62, 1, 61]] {
        let module = module(registers);
        let owner = owner(&module);
        let identity = *owner.identity();
        let source_bytes = owner.canonical_bytes().to_vec();
        let admitted = admit(&owner);
        assert_eq!(admitted.identity().wire_version(), 16);
        assert_eq!(admitted.identity().digest(), identity.digest());
        assert_eq!(
            admitted.identity().canonical_length(),
            identity.canonical_length()
        );
        assert_eq!(admitted.module(), owner.module());
        assert_ne!(
            admitted.module().functions.as_ptr(),
            owner.module().functions.as_ptr()
        );
        assert_eq!(owner.canonical_bytes(), source_bytes);
        assert!(!admitted.grants_execution_authority());
        for (inputs, expected) in cases {
            let request = request(inputs, 128);
            let original = request.clone();
            let execution = admitted
                .simulate(
                    &request,
                    SimulationTargetV1::amdgpu_64(),
                    SimulationLimitsV1::default(),
                )
                .unwrap();
            assert_eq!(request, original);
            assert_eq!(execution.steps_executed(), 128 * 5);
            let output = execution.buffer(3).unwrap();
            assert!(
                output.bytes()[..512]
                    .chunks_exact(4)
                    .all(|word| word == expected.to_le_bytes())
            );
            assert_eq!(&output.bytes()[512..], &[0x5a; 8]);
            assert!(
                output.initialized()[..512]
                    .iter()
                    .all(|initialized| *initialized)
            );
            assert_eq!(&output.initialized()[512..], &[false; 8]);
            assert!(!execution.grants_execution_authority());
        }
    }
}

#[test]
fn old_canonical_owners_cannot_smuggle_region_into_old_simulation_routes() {
    let module = module([32, 33, 34, 35, 36]);
    assert!(VerifiedCanonicalKernelIrV7::from_module(module.clone()).is_err());
    assert!(VerifiedCanonicalKernelIrV9::from_module(module.clone()).is_err());
    assert!(VerifiedCanonicalKernelIrV10::from_module(module.clone()).is_err());
    assert!(VerifiedCanonicalKernelIrV11::from_module(module.clone()).is_err());
    assert!(VerifiedCanonicalKernelIrV12::from_module(module).is_err());
}

#[test]
fn canonical_shape_verification_remains_mandatory_before_cpu_admission() {
    for case in 0..3 {
        let mut module = module([32, 33, 34, 35, 36]);
        let function = &mut module.functions[0];
        match case {
            0 => function.signature.parameters[0] = Type::Scalar(ScalarType::I32),
            1 => {
                function.body.as_mut().unwrap().blocks[0].operations[0].results[0].ty =
                    Type::Scalar(ScalarType::I32)
            }
            2 => function.body.as_mut().unwrap().blocks[0].operations[0]
                .results
                .clear(),
            _ => unreachable!(),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
        assert!(
            VerifiedCanonicalKernelIrModuleV16::from_module_ref_with_verification_budget_v16(
                &module,
                &mut budget
            )
            .is_err()
        );
    }
}

fn assert_profile_refused(
    module: &Module,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
) {
    let owner = owner(module);
    let admitted = admit(&owner);
    let error = admitted
        .preflight(request, target, SimulationLimitsV1::default())
        .unwrap_err();
    let SimulationPreflightErrorV1::Unsupported(report) = error else {
        panic!("expected profile refusal: {error:?}")
    };
    assert!(
        report
            .findings()
            .iter()
            .any(|finding| finding.feature == UnsupportedFeatureV1::OrderedRegionProfile)
    );
}

#[test]
fn partial_waves_missing_required_shape_and_wrong_layout_are_not_the_declared_profile() {
    let module = module([32, 33, 34, 35, 36]);
    for count in [1, 32, 63, 65, 127] {
        assert_profile_refused(
            &module,
            &request([7, 11, 13], count),
            SimulationTargetV1::amdgpu_64(),
        );
    }
    assert_profile_refused(
        &module,
        &request([7, 11, 13], 64),
        SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
    );
    let mut missing_required = module.clone();
    missing_required.kernels[0].workgroup_size = None;
    assert_profile_refused(
        &missing_required,
        &request([7, 11, 13], 64),
        SimulationTargetV1::amdgpu_64(),
    );
    let admitted = admit(&owner(&module));
    let mut wrong_workgroup = request([7, 11, 13], 64);
    wrong_workgroup.workgroup.0 = [32, 1, 1];
    assert!(matches!(
        admitted.preflight(
            &wrong_workgroup,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default()
        ),
        Err(SimulationPreflightErrorV1::WorkgroupMismatch { .. })
    ));
}

#[test]
fn conflicting_target_or_wave_declarations_never_reach_execution() {
    for capability in [
        gfx950_xnack_minus_target_capability(),
        TargetCapability::WaveWidth(WaveWidth::Wave32),
        TargetCapability::SubgroupSize(32),
    ] {
        for scope in 0..3 {
            let mut module = module([32, 33, 34, 35, 36]);
            match scope {
                0 => module.required_capabilities.insert(capability.clone()),
                1 => module.kernels[0]
                    .required_capabilities
                    .insert(capability.clone()),
                2 => module.functions[0]
                    .required_capabilities
                    .insert(capability.clone()),
                _ => unreachable!(),
            };
            let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
            // Canonical verification may already reject contradictory wave requirements.
            if let Ok((owner, _)) =
                VerifiedCanonicalKernelIrModuleV16::from_module_ref_with_verification_budget_v16(
                    &module,
                    &mut budget,
                )
            {
                let error = admit(&owner)
                    .preflight(
                        &request([7, 11, 13], 64),
                        SimulationTargetV1::amdgpu_64(),
                        SimulationLimitsV1::default(),
                    )
                    .unwrap_err();
                assert!(matches!(error, SimulationPreflightErrorV1::Unsupported(_)));
            }
        }
    }
}

#[derive(Default)]
struct Events(Vec<SimulationEventV1>);
impl SimulationEventSinkV1 for Events {
    fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        assert!(self.0.len() < 2048);
        self.0.push(event.clone());
        Ok(())
    }
}

#[derive(Default)]
struct DebugRecords(Vec<SimulationDebugRecordV1>);
impl SimulationDebugSinkV1 for DebugRecords {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        assert!(self.0.len() < 2048);
        self.0.push(record);
        SimulationDebugSinkControlV1::Continue
    }
}

#[test]
fn atomic_region_has_one_operation_lifecycle_and_no_physical_register_microsteps() {
    let mut module = module([32, 33, 34, 35, 36]);
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .truncate(1);
    let owner = owner(&module);
    let admitted = admit(&owner);
    let request = request([u32::MAX, 0, 1], 64);
    let mut events = Events::default();
    let execution = admitted
        .simulate_observed_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .unwrap();
    assert_eq!(execution.steps_executed(), 128);
    for global_x in 0..64 {
        let events: Vec<_> = events
            .0
            .iter()
            .filter(|event| {
                event.invocation.global[0] == global_x && event.site.operation == Some(0)
            })
            .collect();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == SimulationEventKindV1::OperationBegin)
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.kind, SimulationEventKindV1::OperationEnd { .. }))
                .count(),
            1
        );
    }
    let mut records = DebugRecords::default();
    admitted
        .simulate_debugged_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            SimulationDebugCaptureLimitsV1::new(8, 16, 8, 64).unwrap(),
            &mut records,
        )
        .unwrap();
    assert_eq!(records.0.len(), 128);
    for global_x in 0..64 {
        let checkpoints: Vec<_> = records
            .0
            .iter()
            .filter(|record| record.invocation.global[0] == global_x)
            .collect();
        assert_eq!(checkpoints.len(), 2);
        assert!(
            checkpoints
                .iter()
                .all(|record| record.site.operation == 0 && record.site.block == BlockId(0))
        );
        let SimulationDebugRecordKindV1::Checkpoint {
            phase: SimulationDebugCheckpointPhaseV1::AfterOperation,
            stack: SimulationDebugCollectionV1::Captured(frames),
            ..
        } = &checkpoints[1].kind
        else {
            panic!("expected atomic after checkpoint")
        };
        let SimulationDebugCollectionV1::Captured(values) = &frames[0].values else {
            panic!("expected SSA values")
        };
        assert!(values.iter().all(|value| value.value.0 <= 4));
        assert_eq!(
            values
                .iter()
                .find(|value| value.value == ValueId(4))
                .unwrap()
                .observed,
            SimulationDebugValueV1::Scalar(ScalarBitsV1::u32(0))
        );
    }
}

#[test]
fn canonical_byte_resident_and_step_budgets_keep_exact_boundaries() {
    let owner = owner(&module([32, 33, 34, 35, 36]));
    let mut limits = SimulationLimitsV1 {
        max_canonical_bytes: owner.canonical_bytes().len(),
        ..SimulationLimitsV1::default()
    };
    assert!(AdmittedSimulationModuleV1::admit_v16(&owner, limits).is_ok());
    limits.max_canonical_bytes -= 1;
    assert!(matches!(
        AdmittedSimulationModuleV1::admit_v16(&owner, limits),
        Err(SimulationAdmissionErrorV1::CanonicalBytesLimit { .. })
    ));
    let mut limits = SimulationLimitsV1 {
        max_resident_bytes: 1,
        ..SimulationLimitsV1::default()
    };
    let SimulationAdmissionErrorV1::ResidentBytesLimit { actual, .. } =
        AdmittedSimulationModuleV1::admit_v16(&owner, limits).unwrap_err()
    else {
        panic!("expected measured resident refusal")
    };
    limits.max_resident_bytes = actual;
    assert!(AdmittedSimulationModuleV1::admit_v16(&owner, limits).is_ok());
    limits.max_resident_bytes = actual - 1;
    assert!(matches!(
        AdmittedSimulationModuleV1::admit_v16(&owner, limits),
        Err(SimulationAdmissionErrorV1::ResidentBytesLimit { .. })
    ));
    let admitted = admit(&owner);
    let request = request([7, 11, 13], 64);
    let original = request.clone();
    let mut limits = SimulationLimitsV1 {
        max_steps: 320,
        ..SimulationLimitsV1::default()
    };
    assert!(
        admitted
            .simulate(&request, SimulationTargetV1::amdgpu_64(), limits)
            .is_ok()
    );
    limits.max_steps -= 1;
    assert!(
        admitted
            .simulate(&request, SimulationTargetV1::amdgpu_64(), limits)
            .is_err()
    );
    assert_eq!(request, original);
}

#[test]
fn atomic_value_and_debug_capture_execute_on_the_existing_bounded_stack() {
    let mut module = module([32, 33, 34, 35, 36]);
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .truncate(1);
    let admitted = admit(&owner(&module));
    std::thread::Builder::new()
        .name("ordered-region-small-stack".to_owned())
        .stack_size(256 * 1024)
        .spawn(move || {
            let mut records = DebugRecords::default();
            let execution = admitted
                .simulate_debugged_with_sink(
                    &request([7, 11, 13], 64),
                    SimulationTargetV1::amdgpu_64(),
                    SimulationLimitsV1::default(),
                    SimulationDebugCaptureLimitsV1::new(8, 16, 8, 64).unwrap(),
                    &mut records,
                )
                .unwrap();
            assert_eq!(execution.steps_executed(), 128);
            assert_eq!(records.0.len(), 128);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn declared_capability_row_owns_only_v16_gfx942_cpu_value_abstraction() {
    assert_eq!(SimulationOperationSurfaceV1::OrderedRegion as u8, 38);
    let matrix = semantic_capability_matrix_v1();
    let rows: Vec<_> = matrix
        .top_level_rows
        .iter()
        .filter(|row| row.operation == SimulationOperationSurfaceV1::OrderedRegion)
        .collect();
    assert_eq!(rows.len(), 24);
    let owned: Vec<_> = rows
        .iter()
        .filter(|row| {
            matches!(
                row.capability,
                SimulationCapabilityDispositionV1::Owned { .. }
            )
        })
        .collect();
    assert_eq!(owned.len(), 1);
    assert_eq!(owned[0].kir_wire_version, SimulationKirWireVersionV1::V16);
    assert_eq!(
        owned[0].profile,
        SimulationCapabilityProfileV1::Gfx942XnackMinus
    );
    assert!(matches!(
        owned[0].capability,
        SimulationCapabilityDispositionV1::Owned {
            owner: SimulationSemanticOwnerV1::ScalarBits,
            ..
        }
    ));
    assert_eq!(matrix.authority, "none");
    assert!(!matrix.hardware_observed && !matrix.performance_prediction);
}
