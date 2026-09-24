//! Inert V21 owner execution only; no Rust custody, launch or native authority.
//! Short output slices are CPU mask controls, not production formal admission.
use fe2o3_kernel_ir as physical_global_copy_fixture_ir;
#[path = "../../fe2o3-kernel-ir/tests/fixtures/physical_global_copy_v21.rs"]
mod fixtures;
use fe2o3_kernel_ir::{
    AccessMode, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, Module, ScalarType,
    VerifiedCanonicalKernelIrModuleV21 as Owner,
};
use fe2o3_kir_sim::*;
fn owner(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(16_000_000);
    let mut budget = Budget::new(&mut work, 16_000_000);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v21(module, &mut budget).unwrap();
    (owner, receipt.retained_storage())
}
fn admitted() -> AdmittedSimulationModuleV1 {
    let (owner, retained) = owner(&fixtures::module());
    let mut work = Work::new(16_000_000);
    let mut budget = Budget::new(&mut work, 16_000_000);
    budget.reserve_storage(retained).unwrap();
    AdmittedSimulationModuleV1::admit_v21_with_verification_budget(
        &owner,
        SimulationLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
    .0
}
fn word(index: usize) -> u32 {
    (index as u32)
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(0x8000_0001)
}
fn request(
    input_len: usize,
    output_len: usize,
    grid: u64,
    uninitialized: Option<usize>,
) -> SimulationRequestV1 {
    let target = SimulationTargetV1::amdgpu_64();
    let mut input = vec![0x5a; (input_len + 4) * 4];
    for index in 0..input_len {
        input[(index + 2) * 4..(index + 3) * 4].copy_from_slice(&word(index).to_le_bytes());
    }
    let mut initialized = vec![true; input.len()];
    if let Some(index) = uninitialized {
        initialized[(index + 2) * 4..(index + 3) * 4].fill(false);
    }
    let input = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadOnly,
        4,
        input,
        initialized,
        target,
    )
    .unwrap();
    let output = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        vec![0xa5; (output_len + 4) * 4],
        vec![false; (output_len + 4) * 4],
        target,
    )
    .unwrap();
    let args = [
        (BufferBackingIdV1(7), AccessMode::ReadOnly, input_len),
        (BufferBackingIdV1(9), AccessMode::ReadWrite, output_len),
    ]
    .into_iter()
    .map(|(id, access, len)| {
        SimulationArgumentV1::BufferView(
            BufferViewArgumentV1::new(id, ScalarType::U32, access, 4, 8, len, target).unwrap(),
        )
    })
    .collect();
    SimulationRequestV1::new(
        "physical_global_copy_fixture",
        [grid, 1, 1],
        [64, 1, 1],
        args,
    )
    .with_shared_buffers(vec![
        SharedBufferV1 {
            id: BufferBackingIdV1(7),
            buffer: input,
        },
        SharedBufferV1 {
            id: BufferBackingIdV1(9),
            buffer: output,
        },
    ])
}
fn check_output(run: &SimulationExecutionV1, output_len: usize, grid: u64) {
    let out = run.shared_buffer(BufferBackingIdV1(9)).unwrap();
    for (index, bytes) in out.bytes().chunks_exact(4).enumerate() {
        let written = (2..2 + output_len.min(grid as usize)).contains(&index);
        assert_eq!(
            bytes,
            if written {
                word(index - 2).to_le_bytes()
            } else {
                [0xa5; 4]
            }
        );
        assert!(
            out.initialized()[index * 4..index * 4 + 4]
                .iter()
                .all(|v| *v == written)
        );
    }
    assert!(!run.grants_execution_authority());
}
#[test]
fn exact_owner_and_admission_ledger_keep_nonzero_floors_and_one_short_refusals() {
    let (owner, retained) = owner(&fixtures::module());
    let bytes = owner.canonical_bytes().to_vec();
    let floor = retained + 73;
    let run = |budget: &mut Budget<'_>| {
        AdmittedSimulationModuleV1::admit_v21_with_verification_budget(
            &owner,
            SimulationLimitsV1::default(),
            budget,
        )
    };
    let mut work = Work::new(16_000_000);
    work.charge_work(11).unwrap();
    let mut budget = Budget::new(&mut work, 16_000_000);
    budget.reserve_storage(floor).unwrap();
    let (view, receipt) = run(&mut budget).unwrap();
    assert_eq!(view.identity().wire_version(), 21);
    assert_eq!(view.identity().digest(), owner.identity().digest());
    assert_eq!(view.module(), owner.module());
    assert_ne!(
        view.module().functions.as_ptr(),
        owner.module().functions.as_ptr()
    );
    assert_eq!(budget.storage(), floor);
    let needed_work = budget.work();
    let needed_storage = budget.peak_storage();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    drop(view);
    budget.release_storage(receipt.retained_storage()).unwrap();
    for case in 0..3 {
        let mut work = Work::new(needed_work - usize::from(case == 1));
        work.charge_work(11).unwrap();
        let mut budget = Budget::new(&mut work, needed_storage - usize::from(case == 2));
        budget.reserve_storage(floor).unwrap();
        let result = run(&mut budget);
        assert_eq!(result.is_ok(), case == 0);
        match (case, result) {
            (
                1,
                Err(PhysicalGlobalCopySimulationAdmissionErrorV21::Resource(Resource::Work(e))),
            ) => assert_eq!(e.limit() + 1, e.actual()),
            (
                2,
                Err(PhysicalGlobalCopySimulationAdmissionErrorV21::Resource(Resource::Storage(e)))
                | Err(PhysicalGlobalCopySimulationAdmissionErrorV21::CanonicalView(
                    fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV21::Resource(
                        Resource::Storage(e),
                    ),
                ))
                | Err(PhysicalGlobalCopySimulationAdmissionErrorV21::CanonicalView(
                    fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV21::Decode(
                        fe2o3_kernel_ir::KernelIrDecodeError::Resource(Resource::Storage(e)),
                    ),
                )),
            ) => {
                assert_eq!(e.limit() + 1, e.actual());
                assert_eq!(budget.failed_storage(), Some(needed_storage));
            }
            (0, Ok(_)) => assert_eq!(budget.work(), needed_work),
            other => panic!("wrong exact refusal {other:?}"),
        }
        assert_eq!(budget.storage(), floor);
    }
    assert_eq!(owner.canonical_bytes(), bytes);
}
#[test]
fn ordinary_owner_and_canonical_byte_limit_refuse_without_erasing_accounting() {
    let (ordinary, _) = owner(&Module::new("ordinary"));
    let (physical, _) = owner(&fixtures::module());
    let mut work = Work::new(16_000_000);
    let mut budget = Budget::new(&mut work, 16_000_000);
    budget.reserve_storage(97).unwrap();
    for _ in 0..2 {
        assert!(matches!(
            AdmittedSimulationModuleV1::admit_v21_with_verification_budget(
                &ordinary,
                SimulationLimitsV1::default(),
                &mut budget
            ),
            Err(PhysicalGlobalCopySimulationAdmissionErrorV21::Profile)
        ));
    }
    assert_eq!(budget.work(), 2);
    assert_eq!(budget.storage(), 97);
    let limits = SimulationLimitsV1 {
        max_canonical_bytes: physical.canonical_bytes().len() - 1,
        ..SimulationLimitsV1::default()
    };
    assert!(matches!(
        AdmittedSimulationModuleV1::admit_v21_with_verification_budget(
            &physical,
            limits,
            &mut budget
        ),
        Err(PhysicalGlobalCopySimulationAdmissionErrorV21::Admission(
            SimulationAdmissionErrorV1::CanonicalBytesLimit { .. }
        ))
    ));
    assert_eq!(budget.storage(), 97);
    assert_eq!(budget.work(), 3);
}
#[test]
fn full_input_reads_and_zero_partial_full_output_masks_preserve_canaries() {
    let admitted = admitted();
    for grid in [64u64, 128] {
        for len in [0, 1, 63, 64, 65, 127, 128, 129] {
            let req = request(grid as usize, len, grid, None);
            let original = req.clone();
            let run = admitted
                .simulate(
                    &req,
                    SimulationTargetV1::amdgpu_64(),
                    SimulationLimitsV1::default(),
                )
                .unwrap();
            check_output(&run, len, grid);
            assert_eq!(run.steps_executed(), grid * 25);
            assert_eq!(req, original);
            assert_eq!(
                run.shared_buffer(BufferBackingIdV1(7)).unwrap().bytes(),
                original.shared_buffers[0].buffer.bytes()
            );
        }
    }
}
#[derive(Default)]
struct Events(Vec<SimulationEventV1>);
impl SimulationEventSinkV1 for Events {
    fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        assert!(self.0.len() < 16384);
        self.0.push(event.clone());
        Ok(())
    }
}
#[test]
fn actual_read_and_write_events_use_the_canonical_load_and_store_sites() {
    let admitted = admitted();
    let req = request(64, 13, 64, None);
    let mut events = Events::default();
    let run = admitted
        .simulate_observed_with_sink(
            &req,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .unwrap();
    check_output(&run, 13, 64);
    for lane in 0..64 {
        let reads: Vec<_> = events
            .0
            .iter()
            .filter(|e| {
                e.invocation.global[0] == lane
                    && matches!(e.kind, SimulationEventKindV1::MemoryRead { .. })
            })
            .collect();
        let writes: Vec<_> = events
            .0
            .iter()
            .filter(|e| {
                e.invocation.global[0] == lane
                    && matches!(e.kind, SimulationEventKindV1::MemoryWrite { .. })
            })
            .collect();
        assert_eq!(reads.len(), 1);
        assert_eq!(reads[0].site.operation, Some(13));
        assert_eq!(writes.len(), usize::from(lane < 13));
        if let Some(write) = writes.first() {
            assert_eq!(write.site.operation, Some(21));
        }
    }
}
#[test]
fn inactive_output_does_not_suppress_input_initialization_or_bounds_checks() {
    let admitted = admitted();
    for (req, uninitialized) in [
        (request(64, 0, 64, Some(63)), true),
        (request(63, 0, 64, None), false),
    ] {
        let Err(SimulationErrorV1::Execution(error)) = admitted.simulate(
            &req,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
        ) else {
            panic!("must refuse full-EXEC input")
        };
        if uninitialized {
            assert!(matches!(
                error.kind,
                SimulationExecutionErrorKindV1::UninitializedRead { .. }
            ));
        } else {
            assert!(matches!(
                error.kind,
                SimulationExecutionErrorKindV1::OutOfBounds { .. }
            ));
        }
    }
}
#[test]
fn same_backing_is_refused_before_execution_even_for_nonoverlapping_views() {
    let admitted = admitted();
    let target = SimulationTargetV1::amdgpu_64();
    for offset in [8, 264] {
        let mut req = request(64, 64, 64, None);
        // RW backing permits both views; exact same backing is still excluded.
        req.shared_buffers[0].buffer = BufferArgumentV1::new(
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            vec![0; 1024],
            vec![true; 1024],
            target,
        )
        .unwrap();
        req.arguments[1] = SimulationArgumentV1::BufferView(
            BufferViewArgumentV1::new(
                BufferBackingIdV1(7),
                ScalarType::U32,
                AccessMode::ReadWrite,
                4,
                offset,
                64,
                target,
            )
            .unwrap(),
        );
        assert!(matches!(
            {
                let SimulationArgumentV1::BufferView(input) = &req.arguments[0] else {
                    panic!("input view");
                };
                let SimulationArgumentV1::BufferView(output) = &req.arguments[1] else {
                    panic!("output view");
                };
                assert_eq!(input.byte_offset(), 8);
                assert_eq!(input.elements(), 64);
                assert_eq!(output.byte_offset(), offset);
                assert_eq!(
                    input.byte_offset() + input.elements() * 4 <= output.byte_offset(),
                    offset == 264
                );
                admitted.preflight(&req, target, SimulationLimitsV1::default())
            },
            Err(SimulationPreflightErrorV1::PhysicalGlobalCopyAliasedArgumentsV21)
        ));
    }
}
#[test]
fn permissions_alignment_target_and_full_wave_launch_remain_checked() {
    let admitted = admitted();
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    for grid in [1, 63, 65, 192] {
        assert!(
            admitted
                .preflight(&request(192, 192, grid, None), target, limits)
                .is_err()
        );
    }
    assert!(
        admitted
            .preflight(
                &request(64, 64, 64, None),
                SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
                limits
            )
            .is_err()
    );
    let mut req = request(64, 64, 64, None);
    req.shared_buffers[1].buffer = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadOnly,
        4,
        vec![0; 272],
        vec![true; 272],
        target,
    )
    .unwrap();
    assert!(matches!(
        admitted.preflight(&req, target, limits),
        Err(SimulationPreflightErrorV1::BufferAccess { .. })
    ));
    let mut req = request(64, 64, 64, None);
    req.arguments[0] = SimulationArgumentV1::BufferView(
        BufferViewArgumentV1::new(
            BufferBackingIdV1(7),
            ScalarType::U32,
            AccessMode::ReadOnly,
            1,
            1,
            64,
            target,
        )
        .unwrap(),
    );
    let Err(SimulationErrorV1::Execution(error)) = admitted.simulate(&req, target, limits) else {
        panic!("misaligned input must fail")
    };
    assert!(matches!(
        error.kind,
        SimulationExecutionErrorKindV1::MisalignedAccess { .. }
    ));
}
#[test]
fn execution_step_and_resident_one_below_limits_preserve_request() {
    let admitted = admitted();
    let req = request(64, 64, 64, None);
    let original = req.clone();
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    let plan = admitted.preflight(&req, target, limits).unwrap();
    assert!(
        admitted
            .preflight(
                &req,
                target,
                SimulationLimitsV1 {
                    max_resident_bytes: plan.resident_bytes(),
                    ..limits
                }
            )
            .is_ok()
    );
    assert!(matches!(
        admitted.preflight(
            &req,
            target,
            SimulationLimitsV1 {
                max_resident_bytes: plan.resident_bytes() - 1,
                ..limits
            }
        ),
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            ..
        })
    ));
    assert!(
        admitted
            .simulate(
                &req,
                target,
                SimulationLimitsV1 {
                    max_steps: 64 * 25,
                    ..limits
                }
            )
            .is_ok()
    );
    assert!(matches!(
        admitted.simulate(
            &req,
            target,
            SimulationLimitsV1 {
                max_steps: 64 * 25 - 1,
                ..limits
            }
        ),
        Err(SimulationErrorV1::Execution(_))
    ));
    assert_eq!(req, original);
}
#[derive(Default)]
struct Records(usize);
impl SimulationDebugSinkV1 for Records {
    fn record(&mut self, _: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        self.0 += 1;
        SimulationDebugSinkControlV1::Continue
    }
}
#[test]
fn public_debug_and_observation_routes_refuse_before_any_symbolic_or_pending_record() {
    let admitted = admitted();
    let req = request(64, 64, 64, None);
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    for capture in [
        SimulationDebugCaptureLimitsV1::disabled(),
        SimulationDebugCaptureLimitsV1::new(8, 768, 8, 4096).unwrap(),
    ] {
        let mut records = Records::default();
        let result =
            admitted.simulate_debugged_with_sink(&req, target, limits, capture, &mut records);
        assert!(matches!(
            result,
            Err(SimulationErrorV1::Preflight(
                SimulationPreflightErrorV1::PhysicalGlobalCopyPendingDebugUnavailableV21
            ))
        ));
        let result = admitted.simulate_debugged_scheduled_with_sink(
            &req,
            target,
            limits,
            SimulationScheduleRequestV1::RecordCanonical { max_decisions: 256 },
            capture,
            &mut records,
        );
        assert!(matches!(
            result,
            Err(SimulationErrorV1::Preflight(
                SimulationPreflightErrorV1::PhysicalGlobalCopyPendingDebugUnavailableV21
            ))
        ));
        let options = ObservationExecutionOptionsV1::new(capture);
        let mut events = NoopSimulationEventSinkV1;
        let result = admitted.simulate_debugged_with_observation_options(
            &req,
            target,
            limits,
            options,
            &mut events,
            &mut records,
        );
        assert!(matches!(
            result,
            Err(SimulationErrorV1::Preflight(
                SimulationPreflightErrorV1::PhysicalGlobalCopyPendingDebugUnavailableV21
            ))
        ));
        if capture.is_enabled() {
            assert!(matches!(
                admitted.preflight_with_observation_options(&req, target, limits, options),
                Err(SimulationPreflightErrorV1::PhysicalGlobalCopyPendingDebugUnavailableV21)
            ));
        }
        assert_eq!(records.0, 0);
    }
}
#[test]
fn capability_matrix_names_only_the_closed_v21_profile_and_keeps_older_rows() {
    let matrix = semantic_capability_matrix_v1();
    let rows: Vec<_> = matrix
        .top_level_rows
        .iter()
        .filter(|row| row.kir_wire_version == SimulationKirWireVersionV1::V21)
        .collect();
    assert_eq!(rows.len(), 4 * 46);
    for row in rows {
        let expected = row.profile == SimulationCapabilityProfileV1::Gfx942XnackMinus
            && matches!(
                row.operation,
                SimulationOperationSurfaceV1::PhysicalGlobalCopyDeclaration
                    | SimulationOperationSurfaceV1::PhysicalGlobalCopyStep
                    | SimulationOperationSurfaceV1::Return
            );
        assert_eq!(
            matches!(
                row.capability,
                SimulationCapabilityDispositionV1::Owned { .. }
            ),
            expected
        );
    }
    assert_eq!(
        matrix
            .top_level_rows
            .iter()
            .filter(|row| row.kir_wire_version != SimulationKirWireVersionV1::V21)
            .count(),
        9 * 4 * 46
    );
    assert!(!matrix.hardware_observed);
    assert_eq!(matrix.authority, "none");
}

#[test]
fn shifted_actual_load_site_and_register_edit_execute_without_fixture_ordinals() {
    let mut module = fixtures::module_with_extra_zero(true);
    for operation in &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations {
        if let fe2o3_kernel_ir::OperationKind::Gfx942PhysicalGlobalCopyStep(step) =
            &mut operation.kind
        {
            match step.instruction.opcode {
                fe2o3_kernel_ir::Gfx942PhysicalGlobalCopyOpcodeV1::GlobalLoadDword => {
                    step.instruction.destination = 22
                }
                fe2o3_kernel_ir::Gfx942PhysicalGlobalCopyOpcodeV1::GlobalStoreDword => {
                    step.instruction.source1 = 22
                }
                _ => {}
            }
        }
    }
    let (owner, retained) = owner(&module);
    let mut work = Work::new(16_000_000);
    let mut budget = Budget::new(&mut work, 16_000_000);
    budget.reserve_storage(retained + 73).unwrap();
    let (view, receipt) = AdmittedSimulationModuleV1::admit_v21_with_verification_budget(
        &owner,
        SimulationLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let req = request(128, 65, 128, None);
    let mut events = Events::default();
    let run = view
        .simulate_observed_with_sink(
            &req,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .unwrap();
    check_output(&run, 65, 128);
    assert_eq!(run.steps_executed(), 128 * 26);
    let reads: Vec<_> = events
        .0
        .iter()
        .filter(|e| matches!(e.kind, SimulationEventKindV1::MemoryRead { .. }))
        .collect();
    assert_eq!(reads.len(), 128);
    assert!(reads.iter().all(|e| e.site.operation == Some(14)));
    assert_eq!(view.module(), owner.module());
    drop(view);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), retained + 73);
}
