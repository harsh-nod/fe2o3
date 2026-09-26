use super::super::storage_tests_v1::*;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
    StorageLayoutLimitsV1,
};

fn admit(module: &Module) -> VerifiedCanonicalKernelIrModuleV18 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        module,
        StorageLayoutLimitsV1 {
            rows: 64,
            edges: 256,
            containment_depth: 32,
            object_bytes: 4096,
        },
        &mut budget,
    )
    .unwrap()
    .0
}

fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_memory_access_records: 128,
        max_call_depth: 8,
        max_ssa_values: 64,
        ..SimulationLimitsV1::default()
    }
}

fn bits(result: &SimulationExecutionV18) -> u32 {
    u32::from_le_bytes(result.buffer(0).unwrap().bytes().try_into().unwrap())
}

#[derive(Default)]
struct DebugRecords {
    records: Vec<SimulationDebugRecordV1>,
    origins: Vec<crate::SimulationDebugOriginContextV1>,
    stop: bool,
}

impl SimulationDebugSinkV1 for DebugRecords {
    fn wants_operation_origin_v1(&self) -> bool {
        true
    }
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        self.records.push(record);
        if self.stop {
            SimulationDebugSinkControlV1::Stop
        } else {
            SimulationDebugSinkControlV1::Continue
        }
    }
    fn record_with_operation_origin_v1(
        &mut self,
        record: SimulationDebugRecordV1,
        origin: crate::SimulationDebugOriginContextV1,
    ) -> SimulationDebugSinkControlV1 {
        self.origins.push(origin);
        self.record(record)
    }
}

fn capture() -> SimulationDebugCaptureLimitsV1 {
    SimulationDebugCaptureLimitsV1::new(8, 64, 16, 128).unwrap()
}

#[test]
fn canonical_storage_empty_and_nonempty_owners_execute_the_actual_graph() {
    for graph in [
        module(vec![], vec![constant(1, 0x1234_5678), output(1)]),
        simple(AddressSpace::Private),
        simple(AddressSpace::Workgroup),
    ] {
        let owner = admit(&graph);
        let source_bytes = owner.canonical_bytes().to_vec();
        let source_pointer = owner.module() as *const Module;
        let request = request();
        let original = request.clone();
        let result = crate::simulate_canonical_storage_v18(
            &owner,
            &request,
            SimulationTargetV1::amdgpu_64(),
            limits(),
        )
        .unwrap();
        assert_eq!(bits(&result), 0x1234_5678);
        assert_eq!(result.identity(), owner.identity());
        assert_eq!(result.invocations_executed(), 1);
        assert_eq!(result.workgroups_visited(), 1);
        assert_eq!(result.scheduled_slots_visited(), 1);
        assert!(result.schedule_coverage().is_complete());
        assert!(!result.grants_execution_authority());
        assert!(result.completion.schedule_transcript_identity.is_none());
        assert_eq!(request, original);
        assert_eq!(owner.canonical_bytes(), source_bytes);
        assert_eq!(owner.module() as *const Module, source_pointer);
        assert!(result.buffer(1).is_none());
        let (arguments, backings) = result.into_outputs();
        assert!(backings.is_empty());
        assert_ne!(arguments, request.arguments);
    }
}

#[test]
fn canonical_storage_debug_and_event_observations_use_shared_live_state() {
    let graph = simple(AddressSpace::Private);
    let owner = admit(&graph);
    let mut original_events = Events::default();
    let original = run(&graph, &mut original_events).unwrap();
    let mut events = Events::default();
    let mut debug = DebugRecords::default();
    let result = crate::simulate_canonical_storage_with_sinks_v18(
        &owner,
        &request(),
        None,
        SimulationTargetV1::amdgpu_64(),
        limits(),
        capture(),
        &mut events,
        &mut debug,
    )
    .unwrap();
    assert_eq!(events.0, original_events.0);
    assert_eq!(result.arguments(), original.arguments);
    assert_eq!(result.steps_executed(), original.steps_executed);
    assert_eq!(result.events_emitted(), events.0.len() as u64);
    assert!(!debug.records.is_empty());
    assert!(
        debug
            .origins
            .iter()
            .any(|origin| matches!(origin, crate::SimulationDebugOriginContextV1::Available(_)))
    );
    assert!(debug.records.iter().any(|record| matches!(
        &record.kind,
        SimulationDebugRecordKindV1::Memory {
            address_space: AddressSpace::Global,
            value: SimulationDebugValueV1::Scalar(value), ..
        } if value.bits() == 0x1234_5678
    )));
    for record in &debug.records {
        assert_eq!(record.site.function_ordinal, 0);
        assert_eq!(record.site.block, BlockId(0));
    }
    let mut stopped = DebugRecords {
        stop: true,
        ..DebugRecords::default()
    };
    let stopped_result = crate::simulate_canonical_storage_debugged_with_sink_v18(
        &owner,
        &request(),
        SimulationTargetV1::amdgpu_64(),
        limits(),
        capture(),
        &mut stopped,
    )
    .unwrap();
    assert_eq!(stopped.records.len(), 1);
    assert_eq!(bits(&stopped_result), bits(&result));
    assert_eq!(stopped_result.steps_executed(), result.steps_executed());
}

#[test]
fn canonical_storage_exact_canonical_size_and_early_limits_do_not_deliver_callbacks() {
    let owner = admit(&simple(AddressSpace::Private));
    let mut events = Events::default();
    let mut debug = DebugRecords::default();
    let mut limits = limits();
    limits.max_canonical_bytes = owner.canonical_bytes().len();
    assert!(
        crate::simulate_canonical_storage_v18(
            &owner,
            &request(),
            SimulationTargetV1::amdgpu_64(),
            limits,
        )
        .is_ok()
    );
    limits.max_canonical_bytes -= 1;
    assert!(matches!(
        crate::simulate_canonical_storage_with_sinks_v18(
            &owner,
            &request(),
            None,
            SimulationTargetV1::amdgpu_64(),
            limits,
            capture(),
            &mut events,
            &mut debug,
        ),
        Err(SimulationErrorV1::Preflight(
            SimulationPreflightErrorV1::ResourceLimit {
                resource: "canonical bytes",
                ..
            }
        ))
    ));
    limits.max_canonical_bytes = 0;
    assert!(matches!(
        crate::simulate_canonical_storage_with_sinks_v18(
            &owner,
            &request(),
            None,
            SimulationTargetV1::amdgpu_64(),
            limits,
            capture(),
            &mut events,
            &mut debug,
        ),
        Err(SimulationErrorV1::Preflight(
            SimulationPreflightErrorV1::InvalidLimits(_)
        ))
    ));
    assert!(events.0.is_empty());
    assert!(debug.records.is_empty());
}

#[test]
fn canonical_storage_independent_step_boundary_preserves_request_on_failure() {
    let owner = admit(&module(rows(), vec![allocate(1, 0, AddressSpace::Private)]));
    let request = request();
    let before = request.clone();
    // Allocate: dispatch, row lookup, result binding. Return: one dispatch.
    for steps in [4, 3] {
        let mut limits = limits();
        limits.max_steps = steps;
        let result = crate::simulate_canonical_storage_v18(
            &owner,
            &request,
            SimulationTargetV1::amdgpu_64(),
            limits,
        );
        if steps == 4 {
            assert_eq!(result.unwrap().steps_executed(), 4);
        } else {
            assert!(matches!(
                result,
                Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                    kind: SimulationExecutionErrorKindV1::StepLimit { limit: 3 },
                    ..
                }))
            ));
        }
        assert_eq!(request, before);
    }
}

#[test]
fn canonical_storage_retention_includes_actual_owner_bytes_and_output_wrapper() {
    let owner = admit(&simple(AddressSpace::Private));
    let view = owner.verified_storage_module_ref_v1();
    let structural = crate::resident::storage_module_retained_bytes_v1(&view).unwrap();
    assert_eq!(
        retained_bytes(&owner).unwrap() - structural,
        size_of::<VerifiedCanonicalKernelIrModuleV18>() - size_of::<Module>()
            + owner.canonical_bytes().len()
            + size_of::<SimulationExecutionV18>()
    );
    let mut limits = limits();
    limits.max_resident_bytes = retained_bytes(&owner).unwrap() - 1;
    let mut events = Events::default();
    let mut debug = DebugRecords::default();
    assert!(matches!(
        crate::simulate_canonical_storage_with_sinks_v18(
            &owner,
            &request(),
            None,
            SimulationTargetV1::amdgpu_64(),
            limits,
            capture(),
            &mut events,
            &mut debug,
        ),
        Err(SimulationErrorV1::Preflight(
            SimulationPreflightErrorV1::ResourceLimit {
                resource: "resident bytes",
                ..
            }
        ))
    ));
    assert!(events.0.is_empty());
    assert!(debug.records.is_empty());
}

#[test]
fn canonical_storage_whole_execution_has_deterministic_resident_boundary() {
    let owner = admit(&simple(AddressSpace::Private));
    let request = request();
    let original = request.clone();
    let run = |bytes| {
        crate::simulate_canonical_storage_v18(
            &owner,
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1 {
                max_resident_bytes: bytes,
                ..limits()
            },
        )
    };
    let mut low = 1;
    let mut high = limits().max_resident_bytes;
    assert!(run(high).is_ok());
    while low < high {
        let middle = low + (high - low) / 2;
        if run(middle).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    for _ in 0..2 {
        assert_eq!(bits(&run(low).unwrap()), 0x1234_5678);
        assert!(matches!(
            run(low - 1),
            Err(SimulationErrorV1::Preflight(
                SimulationPreflightErrorV1::ResourceLimit {
                    resource: "resident bytes",
                    ..
                }
            )) | Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                kind: SimulationExecutionErrorKindV1::StorageResidentLimit { .. },
                ..
            }))
        ));
    }
    assert_eq!(request, original);
}

#[test]
fn canonical_storage_dynamic_lds_remains_explicit_and_layout_checked() {
    let mut graph = simple(AddressSpace::Workgroup);
    let OperationKind::WorkgroupMemory(memory) =
        &mut graph.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    memory.extent = WorkgroupMemoryExtent::Dynamic;
    let owner = admit(&graph);
    assert!(
        crate::simulate_canonical_storage_v18(
            &owner,
            &request(),
            SimulationTargetV1::amdgpu_64(),
            limits(),
        )
        .is_err()
    );
    for bytes in [8, 16, 12] {
        let mut events = Events::default();
        let dynamic = DynamicWorkgroupMemoryRequestV1::new(bytes);
        let result = crate::simulate_canonical_storage_with_sinks_v18(
            &owner,
            &request(),
            Some(dynamic),
            SimulationTargetV1::amdgpu_64(),
            limits(),
            SimulationDebugCaptureLimitsV1::disabled(),
            &mut events,
            &mut NoopSimulationDebugSinkV1,
        );
        if bytes == 12 {
            assert!(matches!(result, Err(SimulationErrorV1::Preflight(_))));
            assert!(events.0.is_empty());
        } else {
            let result = result.unwrap();
            assert_eq!(bits(&result), 0x1234_5678);
            assert_eq!(result.dynamic_workgroup_memory(), Some(dynamic));
            assert!(events.0.iter().any(|event| matches!(event.kind,
                SimulationEventKindV1::AllocationCreated {
                    address_space: AddressSpace::Workgroup, bytes: actual, ..
                } if actual == bytes as usize
            )));
        }
    }
}

#[test]
fn canonical_storage_target_refusal_occurs_before_any_execution() {
    let mut graph = simple(AddressSpace::Private);
    graph.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(value(
            100,
            Type::Scalar(ScalarType::Index),
            OperationKind::Constant(Constant::Index(u64::MAX)),
        ));
    let owner = admit(&graph);
    assert!(
        crate::simulate_canonical_storage_v18(
            &owner,
            &request(),
            SimulationTargetV1::amdgpu_64(),
            limits(),
        )
        .is_ok()
    );
    let mut events = Events::default();
    let mut debug = DebugRecords::default();
    assert!(matches!(
        crate::simulate_canonical_storage_with_sinks_v18(
            &owner,
            &request(),
            None,
            SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
            limits(),
            capture(),
            &mut events,
            &mut debug,
        ),
        Err(SimulationErrorV1::Preflight(
            SimulationPreflightErrorV1::Unsupported(_)
        ))
    ));
    assert!(events.0.is_empty());
    assert!(debug.records.is_empty());
}
