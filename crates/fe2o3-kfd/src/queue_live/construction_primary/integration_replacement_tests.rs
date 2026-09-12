use super::*;
use crate::shared_memory::SharedMemorySessionPhaseV1;
use crate::shared_memory::{PreparationMemoryCallV1 as Call, PreparationNativeFaultV1 as Fault};
use fe2o3_amdhsa_loader::KernelIdentityInputsV1;

type Replacement = RecycledQueuePreparationV1<'static, 3>;

#[derive(Debug, Eq, PartialEq)]
struct ProgramSnapshot {
    identity: KernelIdentityInputsV1,
    abi: Option<[u8; 32]>,
    selected_index: usize,
    selected_storage: usize,
    descriptor: (usize, usize),
    entry: (usize, usize),
}

impl ProgramSnapshot {
    fn capture(program: &ValidatedKernelEnvelope<'_>) -> Self {
        Self {
            identity: program.identity_inputs(),
            abi: program.dispatch_abi_identity(),
            selected_index: program.selected_kernel_index(),
            selected_storage: std::ptr::from_ref(program.selected_kernel()) as usize,
            descriptor: (
                program.descriptor_bytes().as_ptr() as usize,
                program.descriptor_bytes().len(),
            ),
            entry: (
                program.entry_bytes().as_ptr() as usize,
                program.entry_bytes().len(),
            ),
        }
    }
}

struct Snapshot {
    root: usize,
    destroyed: ComputeAqlQueueDestroyedV1,
    predecessor: u64,
    program_vector: (usize, usize, usize),
    programs: Vec<ProgramSnapshot>,
}

impl Snapshot {
    fn capture(root: &Root<Replacement>) -> Self {
        let (destroyed, predecessor, programs, _) = &root.preparation;
        Self {
            root: std::ptr::from_ref(root) as usize,
            destroyed: *destroyed,
            predecessor: *predecessor,
            program_vector: (
                programs.as_ptr() as usize,
                programs.len(),
                programs.capacity(),
            ),
            programs: programs.iter().map(ProgramSnapshot::capture).collect(),
        }
    }

    fn assert(&self, root: &Root<Replacement>, trace: &Rc<RefCell<Trace>>, next: Option<u64>) {
        let (destroyed, predecessor, programs, custody) = &root.preparation;
        assert_eq!(
            *destroyed, self.destroyed,
            "exact input receipt observation"
        );
        assert_eq!(*predecessor, self.predecessor);
        assert_eq!(
            (
                programs.as_ptr() as usize,
                programs.len(),
                programs.capacity()
            ),
            self.program_vector,
            "exact original program vector"
        );
        assert_eq!(
            programs
                .iter()
                .map(ProgramSnapshot::capture)
                .collect::<Vec<_>>(),
            self.programs
        );
        let dispatch = root
            .dispatch
            .as_ref()
            .or_else(|| root.completed.as_ref().and_then(|c| c.dispatch.as_ref()));
        if trace.borrow().calls.contains(&"allocate-ring") {
            assert!(
                dispatch.is_some(),
                "queue construction retains completed dispatch"
            );
        }
        assert_prepared_root(root, custody, self.root, trace, None);
        custody.primary_assert_replacement_generation_v1(next, dispatch);
    }
}

fn setup_replacement(
    predecessor: u64,
    malformed_program: bool,
) -> (Box<Root<Replacement>>, Rc<RefCell<Trace>>, Snapshot) {
    let (mut memory, trace) = setup_memory();
    let (programs, mut packets) = recipe();
    if malformed_program {
        packets[1] = Gfx942FixedDispatchPacketV1::new(
            programs.len(),
            AqlDispatchGeometryV1::new([256, 1, 1], [256, 1, 1]).unwrap(),
            0,
            vec![0; 16].into_boxed_slice(),
            vec![Gfx942DispatchBufferBindingV1::new(0, 0, 0, 4096)].into_boxed_slice(),
        );
    }
    let custody = FixedDispatchPreparationCustodyV1::new(packets, memory.roster());
    trace.borrow_mut().initial_data = Some(memory.observation());
    trace.borrow_mut().initial_preparation = Some(custody.primary_snapshot_v1());
    // This fixture receipt observes input preservation, not actual predecessor destruction.
    let destroyed = ComputeAqlQueueDestroyedV1::from_parts_for_semantic_observation_tests(43, 5);
    let root = Root::new_with(memory, (destroyed, predecessor, programs, custody));
    let snapshot = Snapshot::capture(&root);
    (root, trace, snapshot)
}

fn run_replacement(root: Box<Root<Replacement>>, ring_bytes: u32) -> RunResult<Replacement> {
    run_work(root, |root, entry| {
        root.construct_replacement(entry, ring_bytes)
    })
}

fn assert_preparation_only(root: &Root<Replacement>, trace: &Rc<RefCell<Trace>>) {
    let t = trace.borrow();
    assert_eq!(t.calls, ["plan-auxiliary-resources", "retain-root"]);
    assert!(!t.poison);
    let before = t.initial_data.as_ref().unwrap();
    let after = memory(root).observation();
    assert_eq!(after.host, before.host);
    assert_eq!(after.device, before.device);
    assert_eq!(
        &after.calls[5..],
        &before.calls[5..],
        "no cleanup or refunds"
    );
}

#[test]
fn replacement_early_validation_and_planning_retain_inputs_before_any_native_work() {
    for ring in [0, 64, 128, 192, 4097] {
        let (root, trace, snapshot) = setup_replacement(7, false);
        let (root, result) = run_replacement(root, ring);
        let payload = result.expect_err("invalid ring must reject");
        assert!(matches!(
            payload.downcast_ref::<ComputeAqlQueueSessionErrorV1>(),
            Some(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::RingCapacity {
                    requested: 3,
                    capacity: 0
                }
            ))
        ));
        snapshot.assert(&root, &trace, None);
        let t = trace.borrow();
        assert_eq!(t.calls, ["retain-root"]);
        assert!(!t.poison);
        assert_eq!(
            &memory(&root).observation(),
            t.initial_data.as_ref().unwrap()
        );
    }
    for panic in [false, true] {
        let (root, trace, snapshot) = setup_replacement(7, false);
        trace.borrow_mut().fault = Some(("plan-auxiliary-resources", 1, panic));
        let (root, result) = run_replacement(root, 4096);
        let payload = result.expect_err("planning must reject");
        if panic {
            assert_eq!(
                payload.downcast_ref::<(&str, usize)>(),
                Some(&("plan-auxiliary-resources", 1))
            );
        } else {
            assert!(matches!(
                payload.downcast_ref::<ComputeAqlQueueSessionErrorV1>(),
                Some(ComputeAqlQueueSessionErrorV1::Contract(
                    "plan-auxiliary-resources"
                ))
            ));
        }
        snapshot.assert(&root, &trace, None);
        assert_preparation_only(&root, &trace);
        assert_eq!(
            &memory(&root).observation(),
            trace.borrow().initial_data.as_ref().unwrap()
        );
    }
}

#[test]
fn replacement_invalid_generations_record_failed_custody_without_native_work() {
    for predecessor in [0, u64::MAX - 1, u64::MAX] {
        let (root, trace, snapshot) = setup_replacement(predecessor, false);
        let (root, result) = run_replacement(root, 4096);
        let payload = result.expect_err("invalid replacement generation must reject");
        let error = payload
            .downcast_ref::<ComputeAqlQueueSessionErrorV1>()
            .unwrap();
        if predecessor == 0 {
            assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::StaleDispatchGeneration
                )
            ));
        } else {
            assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::GenerationExhausted
                )
            ));
        }
        root.preparation
            .3
            .primary_assert_failed_stage_v1(PreparationStageV1::Generation);
        snapshot.assert(&root, &trace, None);
        assert_preparation_only(&root, &trace);
        assert_eq!(
            &memory(&root).observation(),
            trace.borrow().initial_data.as_ref().unwrap()
        );
    }
}

#[test]
fn replacement_invalid_recipe_retains_original_program_and_packet_rosters() {
    let (root, trace, snapshot) = setup_replacement(7, true);
    let (root, result) = run_replacement(root, 4096);
    let payload = result.expect_err("invalid recipe must reject");
    assert!(matches!(
        payload.downcast_ref::<ComputeAqlQueueSessionErrorV1>(),
        Some(ComputeAqlQueueSessionErrorV1::DispatchBinding(
            Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: 1,
                detail: "program index"
            }
        ))
    ));
    root.preparation
        .3
        .primary_assert_failed_stage_v1(PreparationStageV1::Plan);
    snapshot.assert(&root, &trace, Some(8));
    assert_preparation_only(&root, &trace);
    assert_eq!(
        &memory(&root).observation(),
        trace.borrow().initial_data.as_ref().unwrap()
    );
}

#[test]
fn replacement_preparation_stage_failures_preserve_inputs_generation_and_control_prefixes() {
    let mut stages = vec![
        PreparationStageV1::Generation,
        PreparationStageV1::Plan,
        PreparationStageV1::Capacity,
        PreparationStageV1::DataRetention,
        PreparationStageV1::KernargAllocate,
        PreparationStageV1::KernargMaterialize,
        PreparationStageV1::KernargMap,
        PreparationStageV1::KernargRetain,
        PreparationStageV1::Commit,
        PreparationStageV1::Complete,
    ];
    for i in 0..3 {
        stages.extend([
            PreparationStageV1::CodeAllocate(i),
            PreparationStageV1::CodeMaterialize(i),
            PreparationStageV1::CodeSeal(i),
            PreparationStageV1::CodeMap(i),
            PreparationStageV1::CodeRetain(i),
            PreparationStageV1::CodeResolve(i),
            PreparationStageV1::PacketResolve(i),
        ]);
    }
    for stage in stages {
        for panic in [false, true] {
            let (mut root, trace, snapshot) = setup_replacement(7, false);
            root.preparation.3.primary_inject_stage_v1(stage, panic);
            let (root, result) = run_replacement(root, 4096);
            let payload = result.expect_err("preparation stage must reject");
            if panic {
                assert_eq!(
                    payload.downcast_ref::<(&str, PreparationStageV1)>(),
                    Some(&("dispatch preparation", stage))
                );
            } else {
                assert!(matches!(
                    payload.downcast_ref::<ComputeAqlQueueSessionErrorV1>(),
                    Some(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::InvalidCode("injected preparation stage")
                    ))
                ));
            }
            root.preparation.3.primary_assert_failed_stage_v1(stage);
            snapshot.assert(&root, &trace, Some(8));
            assert_preparation_only(&root, &trace);
            let before_native = matches!(
                stage,
                PreparationStageV1::Generation
                    | PreparationStageV1::Plan
                    | PreparationStageV1::Capacity
                    | PreparationStageV1::DataRetention
                    | PreparationStageV1::CodeAllocate(0)
            );
            assert_eq!(
                memory(&root).observation().phase,
                if before_native {
                    SharedMemorySessionPhaseV1::Active
                } else {
                    SharedMemorySessionPhaseV1::Quarantined
                }
            );
        }
    }
}

fn native_fault(call: Call, fault: Fault) {
    let (mut root, trace, snapshot) = setup_replacement(7, false);
    root.memory.as_mut().unwrap().fault = Some((call, fault));
    let (root, result) = run_replacement(root, 4096);
    let payload = result.expect_err("replacement native preparation boundary must reject");
    match fault {
        Fault::Panic(operation) => assert_eq!(
            payload.downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", operation))
        ),
        Fault::AccessPanic => assert_eq!(
            payload.downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", "with_bytes_mut"))
        ),
        Fault::Projection(case) if case.panic => case.assert_panic(&*payload),
        _ => assert!(matches!(
            payload.downcast_ref::<ComputeAqlQueueSessionErrorV1>(),
            Some(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                Gfx942DispatchBindingErrorV1::Memory(_)
            ))
        )),
    }
    let stage = match call {
        Call::AllocateCode(i) => PreparationStageV1::CodeAllocate(i),
        Call::WriteCode(i) => PreparationStageV1::CodeMaterialize(i),
        Call::SealCode(i) => PreparationStageV1::CodeSeal(i),
        Call::MapCode(i) => PreparationStageV1::CodeMap(i),
        Call::AllocateKernarg => PreparationStageV1::KernargAllocate,
        Call::WriteKernarg => PreparationStageV1::KernargMaterialize,
        Call::MapKernarg => PreparationStageV1::KernargMap,
    };
    root.preparation.3.primary_assert_failed_stage_v1(stage);
    snapshot.assert(&root, &trace, Some(8));
    assert_preparation_only(&root, &trace);
    memory(&root).primary_assert_preparation_fault_v1(call, fault);
}

#[test]
fn replacement_native_preparation_failures_preserve_pending_and_returned_owners() {
    for call in [
        Call::AllocateCode(0),
        Call::AllocateCode(1),
        Call::AllocateCode(2),
        Call::AllocateKernarg,
    ] {
        for operation in ["reserve_va", "alloc", "map_cpu", "prepare_cpu_mapping"] {
            native_fault(call, Fault::Error(operation));
            native_fault(call, Fault::Panic(operation));
        }
    }
    for i in 0..3 {
        native_fault(Call::SealCode(i), Fault::Error("protect_cpu_read_only"));
        native_fault(Call::SealCode(i), Fault::Panic("protect_cpu_read_only"));
    }
    for call in [
        Call::MapCode(0),
        Call::MapCode(1),
        Call::MapCode(2),
        Call::MapKernarg,
    ] {
        native_fault(call, Fault::Error("map_gpu"));
        native_fault(call, Fault::Panic("map_gpu"));
        for prefix in 0..=2 {
            for errno in [false, true] {
                if prefix != 1 || errno {
                    native_fault(call, Fault::PartialMap(prefix, errno));
                }
            }
        }
    }
    for call in [
        Call::WriteCode(0),
        Call::WriteCode(1),
        Call::WriteCode(2),
        Call::WriteKernarg,
    ] {
        native_fault(call, Fault::AccessPanic);
    }
    for call in [
        Call::AllocateCode(0),
        Call::AllocateCode(1),
        Call::AllocateCode(2),
        Call::AllocateKernarg,
        Call::MapCode(0),
        Call::MapCode(1),
        Call::MapCode(2),
        Call::MapKernarg,
    ] {
        let allocation = matches!(call, Call::AllocateCode(_) | Call::AllocateKernarg);
        for case in crate::shared_memory::PrimaryProjectionCaseV1::cases(allocation) {
            native_fault(call, Fault::Projection(case));
        }
    }
}

#[test]
fn replacement_late_failures_retain_completed_dispatch_and_original_inputs() {
    let mut cases = vec![
        ("allocate-ring", 1),
        ("allocate-control", 1),
        ("allocate-completion", 1),
        ("allocate-executable", 1),
        ("allocate-executable", 2),
        ("gate-arm", 1),
        ("event", 1),
        ("shadow-install", 1),
        ("foundation", 1),
        ("authenticate", 1),
        ("runtime-created", 1),
        ("recover-outputs", 1),
        ("recover-id", 1),
        ("dependency", 1),
        ("doorbell", 1),
        ("gate-finish", 1),
    ];
    cases.extend((1..=8).map(|i| ("currentness", i)));
    for (name, occurrence) in cases {
        for panic in [false, true] {
            let (root, trace, snapshot) = setup_replacement(7, false);
            trace.borrow_mut().fault = Some((name, occurrence, panic));
            let (root, result) = run_replacement(root, 4096);
            let payload = result.expect_err("late replacement failure must escape");
            if panic {
                assert_eq!(
                    payload.downcast_ref::<(&str, usize)>(),
                    Some(&(name, occurrence))
                );
            } else {
                assert!(payload.is::<ComputeAqlQueueSessionErrorV1>());
            }
            assert_eq!(
                trace.borrow().calls.iter().filter(|&&s| s == name).count(),
                occurrence,
                "requested late failure boundary was reached"
            );
            snapshot.assert(&root, &trace, Some(8));
            assert!(
                root.dispatch.is_some()
                    || root
                        .completed
                        .as_ref()
                        .is_some_and(|c| c.dispatch.is_some()),
                "late failure retains transferred dispatch"
            );
            assert_eq!(trace.borrow().poison, name != "allocate-ring");
            if name == "currentness" && occurrence >= 7 {
                let completed = root
                    .completed
                    .as_ref()
                    .expect("completed bundle at closing checks");
                assert_eq!(completed.doorbell.is_some(), occurrence == 8);
                assert!(!trace.borrow().calls.contains(&"gate-finish"));
            }
        }
    }
}

#[test]
fn replacement_create_uncertainty_retains_all_published_owners_without_cleanup() {
    for mode in 1..=5 {
        let (root, trace, snapshot) = setup_replacement(7, false);
        trace.borrow_mut().create = mode;
        let (root, result) = run_replacement(root, 4096);
        assert!(result.is_err());
        snapshot.assert(&root, &trace, Some(8));
        let t = trace.borrow();
        assert!(t.poison);
        assert_eq!(t.cleanup, 0);
        assert_eq!(t.calls.iter().filter(|&&s| s == "create").count(), 1);
        assert!(!t.calls.contains(&"doorbell"));
        assert!(
            root.engine.as_ref().unwrap().resources[0]
                .authority
                .is_some()
        );
    }
}

#[test]
fn replacement_success_preserves_exact_predecessor_and_last_issuable_generation() {
    for predecessor in [7, u64::MAX - 2] {
        let (root, trace, snapshot) = setup_replacement(predecessor, false);
        let (root, result) = run_replacement(root, 4096);
        assert!(result.is_ok());
        snapshot.assert(&root, &trace, Some(predecessor + 1));
        let complete = root.completed.as_ref().unwrap();
        memory(&root)
            .primary_authenticate(&complete.engine.foundation)
            .unwrap();
        assert_eq!(complete.observation.queue_id, 7);
        assert!(complete.doorbell.is_some());
        let t = trace.borrow();
        assert!(!t.poison);
        assert_eq!(t.calls.first(), Some(&"plan-auxiliary-resources"));
        assert_eq!(t.calls.last(), Some(&"gate-finish"));
        assert_eq!(t.calls.iter().filter(|&&s| s == "create").count(), 1);
        assert!(!t.calls.contains(&"retain-root"));
    }
}

#[test]
fn replacement_public_entry_roots_inputs_before_the_shared_construction_sequence() {
    let source = include_str!("../../queue_live.rs");
    let entry = source
        .split("pub fn recreate_compute_aql_queue_with_fixed_dispatch<const N: usize>(")
        .nth(1)
        .unwrap()
        .split("\nfn recover_fixed_dispatch_data")
        .next()
        .unwrap();
    let allocation = entry.find("PrimaryQueueConstructionV1::new(").unwrap();
    let run = entry
        .find("root.run(|root, entry| root.construct_replacement(entry, ring_bytes))")
        .unwrap();
    assert!(allocation < run);
    let before_root = &entry[..allocation];
    assert!(!before_root.contains('?'));
    assert!(!entry.contains("validate_fixed_batch_ring"));
    assert!(!entry.contains("plan_aql_queue_resources"));
    assert!(!entry.contains("create_compute_aql_queue_inner"));
    assert!(entry.contains("FixedDispatchPreparationCustodyV1::new(packets, data)"));
}
