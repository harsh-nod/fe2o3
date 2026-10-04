//! Public producer controls over verified synthetic KIR, not ordinary-source or
//! browser acceptance. Allocation slots identify actual CPU backing incarnations.
use fe2o3_kernel_ir::*;
use fe2o3_kir_sim::*;

const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();

fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn fixture(fault: bool) -> (AdmittedSimulationModuleV1, SimulationRequestV1) {
    let scalar = Type::Scalar(ScalarType::U32);
    let private = Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let workgroup = Type::pointer(
        scalar.clone(),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let global = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        op(
            1,
            private,
            OperationKind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        op(
            2,
            workgroup,
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: scalar.clone(),
                extent: WorkgroupMemoryExtent::Static(1),
                alignment: 4,
            }),
        ),
        op(
            3,
            scalar.clone(),
            OperationKind::Constant(Constant::U32(42)),
        ),
    ];
    if !fault {
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(1),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ));
    }
    block.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(2),
            value: ValueId(3),
            access: MemoryAccess::new(AddressSpace::Workgroup, 4),
        },
    ));
    block.operations.push(op(
        4,
        scalar,
        OperationKind::Load {
            pointer: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![global], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let capabilities = entry.derived_capabilities();
    entry.required_capabilities = capabilities.clone();
    let mut kernel = Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.required_capabilities = capabilities.clone();
    kernel.workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    let mut module = Module::new("synthetic-allocation-lifecycle-v1");
    module.required_capabilities = capabilities;
    module.functions.push(entry);
    module.kernels.push(kernel);
    let admitted = AdmittedSimulationModuleV1::admit_v9(
        VerifiedCanonicalKernelIrV9::from_module(module).unwrap(),
        SimulationLimitsV1::default(),
    )
    .unwrap();
    let buffer =
        BufferArgumentV1::from_scalars(AccessMode::ReadWrite, 4, &[ScalarBitsV1::u32(7)], TARGET)
            .unwrap();
    (
        admitted,
        SimulationRequestV1::new(
            "kernel",
            [2, 1, 1],
            [1, 1, 1],
            vec![SimulationArgumentV1::Buffer(buffer)],
        ),
    )
}

fn capture_limits() -> SimulationDebugCaptureLimitsV1 {
    SimulationDebugCaptureLimitsV1::new(8, 32, 8, 128).unwrap()
}

fn options(reuse: bool) -> ObservationExecutionOptionsV1 {
    let options = ObservationExecutionOptionsV1::new(capture_limits());
    if reuse {
        options.with_allocation_reuse(
            SimulationAllocationReuseV1::exact_private_and_workgroup(128).unwrap(),
        )
    } else {
        options
    }
}

#[derive(Default)]
struct Events(Vec<SimulationEventV1>);
impl SimulationEventSinkV1 for Events {
    fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        assert!(self.0.len() < 512);
        self.0.push(event.clone());
        Ok(())
    }
}

struct Capture {
    aggregate: bool,
    records: Vec<SimulationDebugRecordV1>,
    watermarks: Vec<SimulationAllocationWatermarkV1>,
    transitions: Vec<SimulationAllocationTransitionV1>,
    lifecycle_attempts: usize,
    lifecycle_control: SimulationDebugSinkControlV1,
    record_control: SimulationDebugSinkControlV1,
}

impl Capture {
    fn new(aggregate: bool) -> Self {
        Self {
            aggregate,
            records: vec![],
            watermarks: vec![],
            transitions: vec![],
            lifecycle_attempts: 0,
            lifecycle_control: SimulationDebugSinkControlV1::Continue,
            record_control: SimulationDebugSinkControlV1::Continue,
        }
    }
}
impl SimulationDebugSinkV1 for Capture {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        assert!(
            !self.aggregate,
            "aggregate delivery must not duplicate a legacy callback"
        );
        assert!(self.records.len() < 512);
        self.records.push(record);
        self.record_control
    }
    fn wants_observation_context_v1(&self) -> bool {
        self.aggregate
    }
    fn wants_allocation_lifecycle_v1(&self) -> bool {
        self.aggregate
    }
    fn allocation_lifecycle_v1(
        &mut self,
        transition: SimulationAllocationTransitionV1,
    ) -> SimulationDebugSinkControlV1 {
        self.lifecycle_attempts += 1;
        assert!(self.lifecycle_attempts <= 32);
        if self.lifecycle_control != SimulationDebugSinkControlV1::DropAndStop {
            assert_eq!(transition.sequence(), self.transitions.len() as u64 + 1);
            self.transitions.push(transition);
        }
        self.lifecycle_control
    }
    fn record_with_observation_context_v1(
        &mut self,
        record: SimulationDebugRecordV1,
        context: SimulationDebugObservationContextV1<'_>,
    ) -> SimulationDebugSinkControlV1 {
        assert!(self.aggregate);
        assert!(self.records.len() < 512);
        let watermark = context.allocation_lifecycle();
        if let SimulationAllocationWatermarkV1::Available { through_sequence } = watermark {
            assert_eq!(
                through_sequence,
                self.transitions.len() as u64,
                "watermark must bind the prefix before this exact callback"
            );
        }
        self.records.push(record);
        self.watermarks.push(watermark);
        self.record_control
    }
}

#[test]
fn configured_scheduled_execution_preserves_exact_legacy_results_events_and_records() {
    let (module, original) = fixture(false);
    for event_policy in [EventPolicyV1::Disabled, EventPolicyV1::Enabled] {
        let mut request = original.clone();
        request.events = event_policy;
        for schedule in [
            SimulationScheduleRequestV1::RecordCanonical { max_decisions: 128 },
            SimulationScheduleRequestV1::RecordSeeded {
                seed: 71,
                max_decisions: 128,
            },
        ] {
            let mut legacy = Capture::new(false);
            let mut legacy_events = Events::default();
            let baseline = module
                .simulate_debugged_scheduled_with_sinks(
                    &request,
                    TARGET,
                    SimulationLimitsV1::default(),
                    schedule,
                    capture_limits(),
                    &mut legacy_events,
                    &mut legacy,
                )
                .unwrap();
            for reuse in [false, true] {
                let mut capture = Capture::new(true);
                let mut events = Events::default();
                let observed = module
                    .simulate_debugged_scheduled_with_observation_options(
                        &request,
                        TARGET,
                        SimulationLimitsV1::default(),
                        schedule,
                        options(reuse),
                        &mut events,
                        &mut capture,
                    )
                    .unwrap();
                assert_eq!(observed, baseline);
                assert_eq!(events.0, legacy_events.0);
                assert_eq!(capture.records, legacy.records);
                assert_eq!(capture.transitions.len(), if reuse { 9 } else { 0 });
                if !reuse {
                    assert!(
                        capture.watermarks.iter().all(|watermark| *watermark
                            == SimulationAllocationWatermarkV1::PolicyDisabled)
                    );
                }
                if event_policy == EventPolicyV1::Disabled {
                    assert_eq!(observed.events_emitted(), 0);
                    assert!(events.0.is_empty());
                }
            }
        }
    }
    assert_eq!(
        original.arguments[0],
        SimulationArgumentV1::Buffer(
            BufferArgumentV1::from_scalars(
                AccessMode::ReadWrite,
                4,
                &[ScalarBitsV1::u32(7)],
                TARGET,
            )
            .unwrap(),
        )
    );
}

#[test]
fn ordinary_options_do_not_invent_schedule_retention() {
    let (module, request) = fixture(false);
    let mut legacy = Capture::new(false);
    let baseline = module
        .simulate_debugged_with_sink(
            &request,
            TARGET,
            SimulationLimitsV1::default(),
            capture_limits(),
            &mut legacy,
        )
        .unwrap();
    for reuse in [false, true] {
        let mut capture = Capture::new(true);
        let mut events = Events::default();
        let observed = module
            .simulate_debugged_with_observation_options(
                &request,
                TARGET,
                SimulationLimitsV1::default(),
                options(reuse),
                &mut events,
                &mut capture,
            )
            .unwrap();
        assert_eq!(observed, baseline);
        assert_eq!(capture.records, legacy.records);
    }
}

#[test]
fn two_workgroups_produce_real_release_before_recreate_in_both_cells() {
    let (module, request) = fixture(false);
    let mut capture = Capture::new(true);
    module
        .simulate_debugged_with_observation_options(
            &request,
            TARGET,
            SimulationLimitsV1::default(),
            options(true),
            &mut Events::default(),
            &mut capture,
        )
        .unwrap();
    assert_eq!(capture.transitions.len(), 9);
    assert_eq!(
        capture.transitions[0].kind(),
        SimulationAllocationTransitionKindV1::Preexisting
    );
    for address_space in [AddressSpace::Private, AddressSpace::Workgroup] {
        let births: Vec<_> = capture
            .transitions
            .iter()
            .copied()
            .filter(|row| {
                row.descriptor().address_space() == address_space
                    && matches!(
                        row.kind(),
                        SimulationAllocationTransitionKindV1::Create { .. }
                    )
            })
            .collect();
        assert_eq!(births.len(), 2);
        let first = births[0].descriptor().identity();
        let second = births[1].descriptor().identity();
        assert_ne!(first.allocation(), second.allocation());
        assert_eq!(first.storage_slot(), second.storage_slot());
        assert_eq!([first.generation(), second.generation()], [1, 2]);
        assert_eq!(
            births[1].kind(),
            SimulationAllocationTransitionKindV1::Create {
                previous_allocation: Some(first.allocation()),
            }
        );
        let release = capture
            .transitions
            .iter()
            .find(|row| {
                row.kind() == SimulationAllocationTransitionKindV1::Release
                    && row.descriptor().identity() == first
            })
            .unwrap();
        assert!(births[0].sequence() < release.sequence());
        assert!(release.sequence() < births[1].sequence());
        assert_eq!(births[0].descriptor(), release.descriptor());
    }
    assert!(
        !capture.transitions.iter().any(|row| row.kind()
            == SimulationAllocationTransitionKindV1::Release
            && row.descriptor().scope() == SimulationAllocationScopeV1::Dispatch),
        "dropping the engine is not a fabricated semantic release"
    );
}

#[test]
fn lifecycle_stop_and_drop_stop_do_not_stop_records_events_or_execution() {
    let (module, mut request) = fixture(false);
    request.events = EventPolicyV1::Enabled;
    let mut baseline = Capture::new(true);
    let mut baseline_events = Events::default();
    let expected = module
        .simulate_debugged_with_observation_options(
            &request,
            TARGET,
            SimulationLimitsV1::default(),
            options(true),
            &mut baseline_events,
            &mut baseline,
        )
        .unwrap();
    for control in [
        SimulationDebugSinkControlV1::Stop,
        SimulationDebugSinkControlV1::DropAndStop,
    ] {
        let mut capture = Capture::new(true);
        capture.lifecycle_control = control;
        let mut events = Events::default();
        let execution = module
            .simulate_debugged_with_observation_options(
                &request,
                TARGET,
                SimulationLimitsV1::default(),
                options(true),
                &mut events,
                &mut capture,
            )
            .unwrap();
        assert_eq!(execution, expected);
        assert_eq!(events.0, baseline_events.0);
        assert_eq!(capture.records, baseline.records);
        assert_eq!(capture.lifecycle_attempts, 1);
        assert_eq!(
            capture.transitions.len(),
            usize::from(control == SimulationDebugSinkControlV1::Stop)
        );
        assert!(capture.watermarks.iter().all(|watermark| *watermark
            == SimulationAllocationWatermarkV1::Unavailable {
                reason: SimulationAllocationObservationUnavailableV1::ObservationStopped,
            }));
    }
}

#[test]
fn debug_record_stop_does_not_stop_allocation_lifecycle() {
    let (module, request) = fixture(false);
    let mut capture = Capture::new(true);
    capture.record_control = SimulationDebugSinkControlV1::Stop;
    let execution = module
        .simulate_debugged_with_observation_options(
            &request,
            TARGET,
            SimulationLimitsV1::default(),
            options(true),
            &mut Events::default(),
            &mut capture,
        )
        .unwrap();
    assert_eq!(execution.invocations_executed(), 2);
    assert_eq!(capture.records.len(), 1);
    assert_eq!(capture.transitions.len(), 9);
}

#[test]
fn disabled_snapshot_capture_still_delivers_explicit_lifecycle() {
    let (module, request) = fixture(false);
    let mut capture = Capture::new(true);
    let configured = ObservationExecutionOptionsV1::new(SimulationDebugCaptureLimitsV1::disabled())
        .with_allocation_reuse(
            SimulationAllocationReuseV1::exact_private_and_workgroup(128).unwrap(),
        );
    module
        .simulate_debugged_with_observation_options(
            &request,
            TARGET,
            SimulationLimitsV1::default(),
            configured,
            &mut Events::default(),
            &mut capture,
        )
        .unwrap();
    assert!(capture.records.is_empty());
    assert_eq!(capture.transitions.len(), 9);
}

#[test]
fn fault_unwind_releases_private_and_workgroup_without_terminal_snapshot() {
    let (module, request) = fixture(true);
    let mut capture = Capture::new(true);
    let result = module.simulate_debugged_with_observation_options(
        &request,
        TARGET,
        SimulationLimitsV1::default(),
        options(true),
        &mut Events::default(),
        &mut capture,
    );
    assert!(matches!(
        result,
        Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::UninitializedRead { .. },
            ..
        }))
    ));
    assert_eq!(capture.transitions.len(), 5);
    let created: Vec<_> = capture
        .transitions
        .iter()
        .copied()
        .filter(|row| {
            matches!(
                row.kind(),
                SimulationAllocationTransitionKindV1::Create { .. }
            )
        })
        .collect();
    assert_eq!(created.len(), 2);
    for birth in created {
        assert!(capture.transitions.iter().any(|row| row.kind()
            == SimulationAllocationTransitionKindV1::Release
            && row.descriptor() == birth.descriptor()
            && row.sequence() > birth.sequence()));
    }
}

#[test]
fn shared_backing_preexists_once_with_real_identity_and_no_teardown_release() {
    let (module, mut request) = fixture(false);
    let SimulationArgumentV1::Buffer(buffer) = request.arguments[0].clone() else {
        panic!("buffer");
    };
    let id = BufferBackingIdV1(9);
    request.shared_buffers.push(SharedBufferV1 { id, buffer });
    request.arguments[0] = SimulationArgumentV1::BufferView(
        BufferViewArgumentV1::new(id, ScalarType::U32, AccessMode::ReadWrite, 4, 0, 1, TARGET)
            .unwrap(),
    );
    let mut capture = Capture::new(true);
    module
        .simulate_debugged_with_observation_options(
            &request,
            TARGET,
            SimulationLimitsV1::default(),
            options(true),
            &mut Events::default(),
            &mut capture,
        )
        .unwrap();
    let preexisting: Vec<_> = capture
        .transitions
        .iter()
        .filter(|row| row.kind() == SimulationAllocationTransitionKindV1::Preexisting)
        .collect();
    assert_eq!(preexisting.len(), 1);
    let descriptor = preexisting[0].descriptor();
    assert_eq!(descriptor.address_space(), AddressSpace::Global);
    assert_eq!(descriptor.byte_len(), 4);
    assert_eq!(descriptor.creation_site(), None);
    assert_eq!(descriptor.identity().generation(), 1);
    assert!(!capture.transitions.iter().any(|row| row.kind()
        == SimulationAllocationTransitionKindV1::Release
        && row.descriptor() == descriptor));
}

#[test]
fn configured_preflight_reports_and_rejects_retained_cache_bytes() {
    let (module, request) = fixture(false);
    let limits = SimulationLimitsV1::default();
    let base = module.preflight(&request, TARGET, limits).unwrap();
    let configured = module
        .preflight_with_observation_options(&request, TARGET, limits, options(true))
        .unwrap();
    let mut descriptor_reservation = Vec::<SimulationAllocationDescriptorV1>::new();
    descriptor_reservation.try_reserve_exact(1).unwrap();
    let descriptor_bytes = (limits.max_allocations + 3)
        * descriptor_reservation.capacity()
        * std::mem::size_of::<SimulationAllocationDescriptorV1>();
    let identities_only = module
        .preflight_with_observation_options(
            &request,
            TARGET,
            limits,
            options(true).with_allocation_reuse(
                SimulationAllocationReuseV1::exact_private_and_workgroup(0).unwrap(),
            ),
        )
        .unwrap();
    assert_eq!(
        identities_only.resident_bytes(),
        base.resident_bytes() + descriptor_bytes,
    );
    assert_eq!(
        configured.resident_bytes(),
        identities_only.resident_bytes() + 128
    );
    let disabled = module
        .preflight_with_observation_options(&request, TARGET, limits, options(false))
        .unwrap();
    assert_eq!(disabled.resident_bytes(), base.resident_bytes());
    let exact = SimulationLimitsV1 {
        max_resident_bytes: configured.resident_bytes(),
        ..limits
    };
    assert_eq!(
        module
            .preflight_with_observation_options(&request, TARGET, exact, options(true))
            .unwrap()
            .resident_bytes(),
        configured.resident_bytes(),
    );
    let one_short = SimulationLimitsV1 {
        max_resident_bytes: configured.resident_bytes() - 1,
        ..limits
    };
    assert!(matches!(
        module.preflight_with_observation_options(&request, TARGET, one_short, options(true)),
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            actual,
            limit,
        }) if actual == configured.resident_bytes() as u64
            && limit == one_short.max_resident_bytes as u64
    ));
    let tight = SimulationLimitsV1 {
        max_resident_bytes: base.resident_bytes(),
        ..limits
    };
    assert!(matches!(
        module.preflight_with_observation_options(&request, TARGET, tight, options(true),),
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            ..
        })
    ));
}
