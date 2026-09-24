//! Inert canonical graph CPU controls, not source authentication, native code
//! execution, physical register observations or production artifact authority.
#[path = "canonical_v19/fixtures.rs"]
mod fixtures;

use fe2o3_kernel_ir::{
    AccessMode, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, Gfx942CompleteBodyStepVNext as Step,
    Gfx942ProgramBinaryOpcodeV1 as Opcode, Gfx942ProgramDestinationV1 as Destination,
    Gfx942ProgramInstructionV1 as Instruction, Gfx942ProgramRoleV1 as Role, Module, Operation,
    OperationKind, ScalarType, TargetCapability, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV19 as Owner, WaveWidth, gfx950_xnack_minus_target_capability,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1 as Admitted, BufferArgumentV1, BufferBackingIdV1,
    BufferViewArgumentV1, IndexWidthV1, ScalarBitsV1, SharedBufferV1,
    SimulationAdmissionErrorV1 as AdmissionError, SimulationArgumentV1,
    SimulationCapabilityDispositionV1 as Disposition, SimulationCapabilityProfileV1 as Profile,
    SimulationDebugCaptureLimitsV1, SimulationDebugCheckpointPhaseV1 as Phase,
    SimulationDebugCollectionV1 as Collection, SimulationDebugRecordKindV1 as RecordKind,
    SimulationDebugRecordV1, SimulationDebugSinkControlV1, SimulationDebugSinkV1,
    SimulationDebugValueV1, SimulationEventKindV1, SimulationEventSinkErrorV1,
    SimulationEventSinkV1, SimulationEventV1, SimulationKirWireVersionV1 as Wire,
    SimulationLimitsV1 as Limits, SimulationOperationSurfaceV1 as Surface,
    SimulationPreflightErrorV1 as PreflightError, SimulationRequestV1 as Request,
    SimulationTargetV1 as Target, SimulationUnsupportedReasonCodeV1 as Reason,
    UnsupportedFeatureV1, semantic_capability_matrix_v1,
};

fn owner(module: &Module) -> Owner {
    let mut work = Work::new(4_000_000);
    let mut budget = Budget::new(&mut work, 4_000_000);
    budget.reserve_storage(97).unwrap();
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v19(module, &mut budget).unwrap();
    assert_eq!(budget.storage(), 97);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    owner
}
fn admit(owner: &Owner) -> Admitted {
    Admitted::admit_v19(owner, Limits::default()).unwrap()
}
fn request(inputs: [u32; 3], selector: u32, length: usize, grid: u64) -> Request {
    let target = Target::amdgpu_64();
    let backing = BufferBackingIdV1(7);
    // The slice excludes both two-word canaries even at zero logical length.
    let bytes = (length + 4) * 4;
    let buffer = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        vec![0x5a; bytes],
        vec![false; bytes],
        target,
    )
    .unwrap();
    let view = BufferViewArgumentV1::new(
        backing,
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        8,
        length,
        target,
    )
    .unwrap();
    let mut args = vec![SimulationArgumentV1::BufferView(view)];
    args.extend(
        inputs
            .into_iter()
            .chain([selector])
            .map(|value| SimulationArgumentV1::Scalar(ScalarBitsV1::u32(value))),
    );
    Request::new("synthetic_kernel", [grid, 1, 1], [64, 1, 1], args).with_shared_buffers(vec![
        SharedBufferV1 {
            id: backing,
            buffer,
        },
    ])
}
fn assert_output(
    execution: &fe2o3_kir_sim::SimulationExecutionV1,
    expected: u32,
    length: usize,
    grid: u64,
) {
    let output = execution.shared_buffer(BufferBackingIdV1(7)).unwrap();
    let written = length.min(grid as usize);
    for (index, word) in output.bytes().chunks_exact(4).enumerate() {
        let write = (2..2 + written).contains(&index);
        assert_eq!(
            word,
            if write {
                expected.to_le_bytes()
            } else {
                [0x5a; 4]
            },
            "output word {index}"
        );
        assert!(
            output.initialized()[index * 4..index * 4 + 4]
                .iter()
                .all(|initialized| *initialized == write)
        );
    }
    assert!(!execution.grants_execution_authority());
}

#[test]
fn exact_v19_custody_and_independent_simulation_view_preserve_source_and_request() {
    for module in [fixtures::single(), fixtures::diamond()] {
        let owner = owner(&module);
        let bytes = owner.canonical_bytes().to_vec();
        let admitted = admit(&owner);
        assert_eq!(admitted.identity().wire_version(), 19);
        assert_eq!(admitted.identity().digest(), owner.identity().digest());
        assert_eq!(
            admitted.identity().canonical_length(),
            owner.identity().canonical_length()
        );
        assert_eq!(admitted.module(), owner.module());
        assert_ne!(
            admitted.module().functions.as_ptr(),
            owner.module().functions.as_ptr()
        );
        assert!(!admitted.grants_execution_authority());
        let request = request([19, 23, 42], 0, 65, 128);
        let original = request.clone();
        let execution = admitted
            .simulate(&request, Target::amdgpu_64(), Limits::default())
            .unwrap();
        assert_output(&execution, 19, 65, 128);
        assert_eq!(request, original);
        assert_eq!(owner.canonical_bytes(), bytes);
    }
}

#[derive(Default)]
struct Events(Vec<SimulationEventV1>);
impl SimulationEventSinkV1 for Events {
    fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        assert!(self.0.len() < 4096);
        self.0.push(event.clone());
        Ok(())
    }
}
#[derive(Default)]
struct Records(Vec<SimulationDebugRecordV1>);
impl SimulationDebugSinkV1 for Records {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        assert!(self.0.len() < 4096);
        self.0.push(record);
        SimulationDebugSinkControlV1::Continue
    }
}

#[test]
fn selector_takes_actual_cfg_edges_and_merges_actual_predecessor_values() {
    let admitted = admit(&owner(&fixtures::diamond()));
    for selector in [0, 1, u32::MAX] {
        let mut events = Events::default();
        let request = request([19, 23, 42], selector, 63, 64);
        let execution = admitted
            .simulate_observed_with_sink(
                &request,
                Target::amdgpu_64(),
                Limits::default(),
                &mut events,
            )
            .unwrap();
        assert_output(&execution, if selector == 0 { 19 } else { 42 }, 63, 64);
        assert_eq!(execution.steps_executed(), 64 * 14);
        for lane in 0..64 {
            let entered: Vec<_> = events
                .0
                .iter()
                .filter(|event| {
                    event.invocation.global[0] == lane
                        && event.kind == SimulationEventKindV1::BlockEnter
                })
                .map(|event| event.site.block)
                .collect();
            assert_eq!(
                entered,
                vec![
                    BlockId(0),
                    BlockId(if selector == 0 { 1 } else { 2 }),
                    BlockId(3)
                ]
            );
        }
    }
}

fn binary_module(opcode: Opcode) -> Module {
    let mut module = fixtures::single();
    let operation = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[1];
    operation.kind = OperationKind::Gfx942CompleteBodyStep(Step {
        authored_block: 0,
        authored_instruction: 0,
        instruction: Instruction::Binary {
            opcode,
            destination: Destination::Output,
            left: Role::Input0,
            right: Role::Input1,
        },
        operands: [Some(ValueId(1)), Some(ValueId(2))],
    });
    module
}
#[test]
fn all_six_instruction_kinds_use_existing_scalar_semantics_with_wrapping_edges() {
    for kind in 0..6 {
        let module = if kind == 0 {
            fixtures::single()
        } else {
            binary_module(
                [
                    Opcode::Add,
                    Opcode::Subtract,
                    Opcode::And,
                    Opcode::Or,
                    Opcode::Xor,
                ][kind - 1],
            )
        };
        let admitted = admit(&owner(&module));
        for [a, b] in [
            [0, 0],
            [u32::MAX, 1],
            [0, 1],
            [0x8000_0000, 0x8000_0000],
            [0xa5a5_a5a5, 0x5a5a_5a5a],
            [19, 23],
        ] {
            let expected = match kind {
                0 => a,
                1 => a.wrapping_add(b),
                2 => a.wrapping_sub(b),
                3 => a & b,
                4 => a | b,
                5 => a ^ b,
                _ => unreachable!(),
            };
            let execution = admitted
                .simulate(
                    &request([a, b, 42], 0, 64, 64),
                    Target::amdgpu_64(),
                    Limits::default(),
                )
                .unwrap();
            assert_eq!(execution.steps_executed(), 64 * 9);
            assert_output(&execution, expected, 64, 64);
        }
    }
}

fn self_write_module() -> Module {
    let mut module = fixtures::single();
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let OperationKind::Gfx942CompleteBodyDeclaration(declaration) = &mut block.operations[0].kind
    else {
        panic!("fixture declaration");
    };
    declaration.instruction_count = 3;
    block.operations.insert(
        1,
        Operation::new(
            vec![ValueDef::new(ValueId(30), Type::Scalar(ScalarType::U32))],
            OperationKind::Gfx942CompleteBodyStep(Step {
                authored_block: 0,
                authored_instruction: 0,
                instruction: Instruction::Move {
                    destination: Destination::Scratch,
                    source: Role::Input0,
                },
                operands: [Some(ValueId(1)), None],
            }),
        ),
    );
    block.operations.insert(
        2,
        Operation::new(
            vec![ValueDef::new(ValueId(31), Type::Scalar(ScalarType::U32))],
            OperationKind::Gfx942CompleteBodyStep(Step {
                authored_block: 0,
                authored_instruction: 1,
                instruction: Instruction::Binary {
                    opcode: Opcode::Add,
                    destination: Destination::Scratch,
                    left: Role::Scratch,
                    right: Role::Input1,
                },
                operands: [Some(ValueId(30)), Some(ValueId(2))],
            }),
        ),
    );
    let OperationKind::Gfx942CompleteBodyStep(step) = &mut block.operations[3].kind else {
        panic!("fixture step");
    };
    step.authored_instruction = 2;
    step.instruction = Instruction::Move {
        destination: Destination::Output,
        source: Role::Scratch,
    };
    step.operands = [Some(ValueId(31)), None];
    module
}
#[test]
fn actual_ssa_steps_and_resultless_declaration_are_visible_on_existing_bounded_debug_stack() {
    let admitted = admit(&owner(&self_write_module()));
    std::thread::Builder::new()
        .name("complete-body-small-stack".to_owned())
        .stack_size(256 * 1024)
        .spawn(move || {
            let mut records = Records::default();
            let execution = admitted
                .simulate_debugged_with_sink(
                    &request([u32::MAX, 1, 42], 0, 1, 64),
                    Target::amdgpu_64(),
                    Limits::default(),
                    SimulationDebugCaptureLimitsV1::new(8, 48, 8, 64).unwrap(),
                    &mut records,
                )
                .unwrap();
            assert_output(&execution, 0, 1, 64);
            assert_eq!(execution.steps_executed(), 64 * 11);
            for ordinal in 0..4 {
                let selected: Vec<_> = records
                    .0
                    .iter()
                    .filter(|record| {
                        record.invocation.global[0] == 0
                            && record.site.block == BlockId(0)
                            && record.site.operation == ordinal
                    })
                    .collect();
                assert_eq!(selected.len(), 2);
                for (index, record) in selected.into_iter().enumerate() {
                    let RecordKind::Checkpoint {
                        phase,
                        stack: Collection::Captured(frames),
                        ..
                    } = &record.kind
                    else {
                        panic!("expected ordinary SSA checkpoint");
                    };
                    assert_eq!(
                        *phase,
                        if index == 0 {
                            Phase::BeforeOperation
                        } else {
                            Phase::AfterOperation
                        }
                    );
                    let Collection::Captured(values) = &frames[0].values else {
                        panic!("captured SSA");
                    };
                    let definitions = [(30, u32::MAX), (31, 0), (5, 0)];
                    for (step, (id, expected)) in definitions.into_iter().enumerate() {
                        let visible = values.iter().find(|value| value.value == ValueId(id));
                        let defined = ordinal as usize > step + 1
                            || (ordinal as usize == step + 1 && index == 1);
                        if defined {
                            assert_eq!(
                                visible.unwrap().observed,
                                SimulationDebugValueV1::Scalar(ScalarBitsV1::u32(expected))
                            );
                        } else {
                            assert!(visible.is_none());
                        }
                    }
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn guarded_tail_uses_logical_slice_length_and_preserves_both_backing_canaries() {
    for module in [fixtures::single(), fixtures::diamond()] {
        let admitted = admit(&owner(&module));
        for length in [0, 1, 63, 64, 65, 127, 128, 129] {
            let execution = admitted
                .simulate(
                    &request([19, 23, 42], 0, length, 128),
                    Target::amdgpu_64(),
                    Limits::default(),
                )
                .unwrap();
            assert_output(&execution, 19, length, 128);
        }
    }
}

#[test]
fn no_older_canonical_encoder_or_decoder_accepts_new_operations_or_v19_bytes() {
    let module = fixtures::diamond();
    let owner = owner(&module);
    for encode in [
        fe2o3_kernel_ir::encode_module_v7,
        fe2o3_kernel_ir::encode_module_v9,
        fe2o3_kernel_ir::encode_module_v10,
        fe2o3_kernel_ir::encode_module_v11,
        fe2o3_kernel_ir::encode_module_v12,
        fe2o3_kernel_ir::encode_module_v16,
        fe2o3_kernel_ir::encode_module_v17,
    ] {
        assert!(encode(&module).is_err());
    }
    for decode in [
        fe2o3_kernel_ir::decode_module_v7,
        fe2o3_kernel_ir::decode_module_v9,
        fe2o3_kernel_ir::decode_module_v10,
        fe2o3_kernel_ir::decode_module_v11,
        fe2o3_kernel_ir::decode_module_v12,
        fe2o3_kernel_ir::decode_module_v16,
        fe2o3_kernel_ir::decode_module_v17,
    ] {
        assert!(decode(owner.canonical_bytes()).is_err());
    }
}

#[test]
fn canonical_verification_rejects_stale_role_ssa_bad_arity_and_wrong_edge_before_admission() {
    for case in 0..4 {
        let mut module = self_write_module();
        let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
        match case {
            0 => {
                if let OperationKind::Gfx942CompleteBodyStep(step) = &mut block.operations[3].kind {
                    step.operands[0] = Some(ValueId(30)); // valid dominance but stale role definition
                }
            }
            1 => block.operations[1].results.clear(),
            2 => {
                if let OperationKind::Gfx942CompleteBodyStep(step) = &mut block.operations[2].kind {
                    step.operands[1] = None;
                }
            }
            3 => {
                module = fixtures::diamond();
                if let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
                    &mut module.functions[0].body.as_mut().unwrap().blocks[0].terminator
                {
                    then_arguments[0] = ValueId(1); // dominates but not actual Scratch
                }
            }
            _ => unreachable!(),
        }
        let mut work = Work::new(4_000_000);
        let mut budget = Budget::new(&mut work, 4_000_000);
        budget.reserve_storage(97).unwrap();
        assert!(Owner::from_module_ref_with_verification_budget_v19(&module, &mut budget).is_err());
        assert_eq!(budget.storage(), 97);
    }
}
fn assert_profile_refused(admitted: &Admitted, request: &Request, target: Target) {
    let error = admitted
        .preflight(request, target, Limits::default())
        .unwrap_err();
    let PreflightError::Unsupported(report) = error else {
        panic!("expected profile refusal: {error:?}");
    };
    assert!(
        report
            .findings()
            .iter()
            .any(|finding| finding.feature == UnsupportedFeatureV1::CompleteBodyProfile)
    );
}
#[test]
fn partial_waves_wrong_workgroup_and_index_layout_are_refused() {
    let admitted = admit(&owner(&fixtures::single()));
    for grid in [1, 32, 63, 65, 127] {
        assert_profile_refused(
            &admitted,
            &request([1, 2, 3], 0, 64, grid),
            Target::amdgpu_64(),
        );
    }
    assert_profile_refused(
        &admitted,
        &request([1, 2, 3], 0, 64, 64),
        Target::little_endian(IndexWidthV1::Bits32),
    );
    let mut wrong = request([1, 2, 3], 0, 64, 64);
    wrong.workgroup.0 = [32, 1, 1];
    assert!(matches!(
        admitted.preflight(&wrong, Target::amdgpu_64(), Limits::default()),
        Err(PreflightError::WorkgroupMismatch { .. })
    ));
}
#[test]
fn missing_or_conflicting_scope_capabilities_never_reach_execution() {
    for scope in 0..3 {
        for capability in [
            None,
            Some(gfx950_xnack_minus_target_capability()),
            Some(TargetCapability::WaveWidth(WaveWidth::Wave32)),
            Some(TargetCapability::SubgroupSize(32)),
        ] {
            let mut module = fixtures::single();
            let caps = match scope {
                0 => &mut module.required_capabilities,
                1 => &mut module.kernels[0].required_capabilities,
                2 => &mut module.functions[0].required_capabilities,
                _ => unreachable!(),
            };
            if let Some(capability) = capability {
                caps.insert(capability);
            } else {
                caps.clear();
            }
            let mut work = Work::new(4_000_000);
            let mut budget = Budget::new(&mut work, 4_000_000);
            if let Ok((owner, _)) =
                Owner::from_module_ref_with_verification_budget_v19(&module, &mut budget)
            {
                assert_profile_refused(
                    &admit(&owner),
                    &request([1, 2, 3], 0, 64, 64),
                    Target::amdgpu_64(),
                );
            }
        }
    }
}

#[test]
fn canonical_byte_resident_and_actual_cfg_step_limits_have_exact_boundaries() {
    let owner = owner(&fixtures::diamond());
    let mut limits = Limits {
        max_canonical_bytes: owner.canonical_bytes().len(),
        ..Limits::default()
    };
    assert!(Admitted::admit_v19(&owner, limits).is_ok());
    limits.max_canonical_bytes -= 1;
    assert!(matches!(
        Admitted::admit_v19(&owner, limits),
        Err(AdmissionError::CanonicalBytesLimit { .. })
    ));
    let mut limits = Limits {
        max_resident_bytes: 1,
        ..Limits::default()
    };
    let AdmissionError::ResidentBytesLimit { actual, .. } =
        Admitted::admit_v19(&owner, limits).unwrap_err()
    else {
        panic!("expected resident refusal");
    };
    limits.max_resident_bytes = actual;
    assert!(Admitted::admit_v19(&owner, limits).is_ok());
    limits.max_resident_bytes -= 1;
    assert!(matches!(
        Admitted::admit_v19(&owner, limits),
        Err(AdmissionError::ResidentBytesLimit { .. })
    ));
    let admitted = admit(&owner);
    let request = request([1, 2, 3], 1, 64, 64);
    let original = request.clone();
    let mut limits = Limits {
        max_steps: 64 * 14,
        ..Limits::default()
    };
    assert!(
        admitted
            .simulate(&request, Target::amdgpu_64(), limits)
            .is_ok()
    );
    limits.max_steps -= 1;
    assert!(
        admitted
            .simulate(&request, Target::amdgpu_64(), limits)
            .is_err()
    );
    assert_eq!(request, original);
}

#[test]
fn capability_rows_activate_only_v19_gfx942_and_keep_previous_ids_and_baseline() {
    assert_eq!(Surface::OrderedRegion as u8, 38);
    assert_eq!(Surface::OrderedProgram as u8, 39);
    assert_eq!(Surface::CompleteBodyDeclaration as u8, 40);
    assert_eq!(Surface::CompleteBodyStep as u8, 41);
    let matrix = semantic_capability_matrix_v1();
    for surface in [Surface::CompleteBodyDeclaration, Surface::CompleteBodyStep] {
        let rows: Vec<_> = matrix
            .top_level_rows
            .iter()
            .filter(|row| row.kir_wire_version != Wire::V21 && row.operation == surface)
            .collect();
        assert_eq!(rows.len(), 4 * 9); // four targets across all nine exact wire profiles
        for row in rows {
            if row.kir_wire_version == Wire::V19 && row.profile == Profile::Gfx942XnackMinus {
                assert!(matches!(row.capability, Disposition::Owned { .. }));
            } else {
                assert_eq!(
                    row.capability,
                    Disposition::Unsupported {
                        reason: Reason::CompleteBodyProfile
                    }
                );
            }
        }
    }
    for row in matrix
        .top_level_rows
        .iter()
        .filter(|row| row.kir_wire_version == Wire::V19 && (row.operation as u8) < 40)
    {
        let baseline = matrix
            .top_level_rows
            .iter()
            .find(|baseline| {
                baseline.kir_wire_version == Wire::V12
                    && baseline.profile == row.profile
                    && baseline.operation == row.operation
            })
            .unwrap();
        assert_eq!(row.capability, baseline.capability);
    }
    assert_eq!(matrix.authority, "none");
    assert!(!matrix.hardware_observed && !matrix.performance_prediction);
}

#[test]
fn maximum_sixteen_steps_keep_dead_and_self_writes_as_ordinary_operation_events() {
    let mut module = fixtures::single();
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let OperationKind::Gfx942CompleteBodyDeclaration(declaration) = &mut block.operations[0].kind
    else {
        panic!("fixture declaration");
    };
    declaration.instruction_count = 16;
    for index in 0..15 {
        let (source, value) = match index {
            0 => (Role::Input1, ValueId(2)), // Deliberate authored dead write.
            1 => (Role::Input0, ValueId(1)),
            _ => (Role::Scratch, ValueId(39 + index as u32)),
        };
        block.operations.insert(
            index + 1,
            Operation::new(
                vec![ValueDef::new(
                    ValueId(40 + index as u32),
                    Type::Scalar(ScalarType::U32),
                )],
                OperationKind::Gfx942CompleteBodyStep(Step {
                    authored_block: 0,
                    authored_instruction: index as u8,
                    instruction: Instruction::Move {
                        destination: Destination::Scratch,
                        source,
                    },
                    operands: [Some(value), None],
                }),
            ),
        );
    }
    let OperationKind::Gfx942CompleteBodyStep(step) = &mut block.operations[16].kind else {
        panic!("fixture output step");
    };
    step.authored_instruction = 15;
    step.instruction = Instruction::Move {
        destination: Destination::Output,
        source: Role::Scratch,
    };
    step.operands = [Some(ValueId(54)), None];
    let admitted = admit(&owner(&module));
    let mut events = Events::default();
    let execution = admitted
        .simulate_observed_with_sink(
            &request([19, 23, 42], 0, 64, 64),
            Target::amdgpu_64(),
            Limits::default(),
            &mut events,
        )
        .unwrap();
    assert_eq!(execution.steps_executed(), 64 * 24);
    assert_output(&execution, 19, 64, 64);
    for ordinal in 1..=16 {
        assert_eq!(
            events
                .0
                .iter()
                .filter(|event| event.invocation.global[0] == 0
                    && event.site.operation == Some(ordinal)
                    && event.kind == SimulationEventKindV1::OperationBegin)
                .count(),
            1
        );
    }
}
