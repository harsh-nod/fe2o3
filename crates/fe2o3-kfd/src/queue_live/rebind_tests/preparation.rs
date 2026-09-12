//! Real preparation/accounting/loan composition; the facade has no Linux engine.

use super::*;
use crate::queue::QueueModelFoundationV1;
use crate::queue::dispatch_binding::{
    actual_persistent_control_test_program,
    preparation::{PreparationOwnerRefsV1, PreparationStageV1, PrimaryPreparationSnapshotV1},
    prepare_public_fixed_dispatch_resources_after_detach_in_place,
};
use crate::queue::live::rebind::LiveRebindRootV1;
use crate::shared_memory::{PreparationMemoryFixtureV1 as Memory, SharedMemorySessionPhaseV1};
use fe2o3_aql::AqlDispatchGeometryV1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Opening {
    None,
    Error,
    Panic,
    Exhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Closing {
    None,
    PreError,
    PrePanic,
    PostError,
    PostPanic,
    Regress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Validation {
    None,
    Error,
    Panic,
    Bypass,
}

struct LoanFixture {
    memory: Memory,
    foundation: QueueModelFoundationV1,
    calls: [usize; 4],
    poisoned: bool,
}

fn packet(index: usize) -> Gfx942FixedDispatchPacketV1 {
    let mut bytes = [0; 16];
    bytes[8..].copy_from_slice(&1024_u64.to_le_bytes());
    Gfx942FixedDispatchPacketV1::new(
        index,
        AqlDispatchGeometryV1::new([256, 1, 1], [256, 1, 1]).unwrap(),
        0,
        bytes.into(),
        vec![Gfx942DispatchBufferBindingV1::new(0, 0, 0, 4096)].into_boxed_slice(),
    )
}

fn fixture() -> (LoanFixture, Vec<Gfx942FixedDispatchDataV1>) {
    let mut memory = Memory::new(true);
    let mut foundation = memory.primary_transfer(&[]).unwrap();
    let loan = memory.primary_loan(&mut foundation).unwrap();
    let data = memory.roster();
    memory.primary_reclaim(&mut foundation, loan).unwrap();
    (
        LoanFixture {
            memory,
            foundation,
            calls: [0; 4],
            poisoned: false,
        },
        data,
    )
}

fn programs() -> Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'static>> {
    let image = include_bytes!(
        "../../../../fe2o3-runtime/fixtures/trusted-gfx942-inplace-transform-v1/inplace_transform.hsaco"
    );
    (1..=3)
        .map(|i| actual_persistent_control_test_program(image, [i; 32]))
        .collect()
}

fn exercise(
    predecessor: u64,
    opening: Opening,
    operation: Option<(PreparationStageV1, bool)>,
    closing: Closing,
    validation: Validation,
    suppress_operation_error: bool,
) {
    let (mut fixture, data) = fixture();
    if opening == Opening::Exhausted {
        fixture.memory.primary_expire_loan_generation_v1();
    }
    let original_session = fixture.memory.primary_session_id();
    let before = fixture.memory.observation();
    let loan_before = fixture.memory.primary_loan_state_v1(&fixture.foundation);
    let mut key = test_queue_key(510, 4);
    key.vm = fixture.foundation.identity().vms()[0].key;
    let mut session = persistent_compute_cancellation_test_session(key, None, None);
    session.observation.ring_bytes = 4096;
    session.detached_dispatch_generation = Some(predecessor);
    session.detached_data_count = data.len();
    session.detached_data_identities = fixed_dispatch_storage_identities(&data);
    session.detached_next_insertion_index = Some(data.len());
    let ledger = session.detached_data_identities.clone();
    let ledger_storage = session.detached_data_identities.as_ptr();
    let programs = programs();
    let programs_storage = (programs.as_ptr(), programs.len(), programs.capacity());
    let programs_identity = programs
        .iter()
        .map(|p| {
            (
                p.identity_inputs(),
                p.dispatch_abi_identity(),
                p.selected_kernel_index(),
            )
        })
        .collect::<Vec<_>>();
    let packets = [packet(0), packet(1), packet(2)];
    let mut original_inputs = PrimaryPreparationSnapshotV1::packets(&packets);
    original_inputs.capture_data_vector_v1(&data, data.capacity());
    let root = LiveRebindRootV1::new(programs, packets, data, Some(predecessor));
    let root_identity = &*root as *const _;
    let retained = RefCell::new(None);
    let snapshot = RefCell::new(None);
    let fixture = RefCell::new(fixture);
    let _ = take_dispatch_terminal_process_gate_record_v1();
    let result = session.settle_fixed_dispatch_rebind_with_v1(
        root,
        |_, programs, preparation, generation| {
            assert_eq!(generation, predecessor);
            preparation.primary_assert_descriptors_v1(&original_inputs, None);
            *snapshot.borrow_mut() = Some(preparation.primary_snapshot_v1());
            if let Some((stage, panic)) = operation {
                preparation.primary_inject_stage_v1(stage, panic);
            }
            let mut context = fixture.borrow_mut();
            let (result, closing) = execute_live_model_custody_v1(
                &mut *context,
                |f| {
                    f.calls[0] += 1;
                    match opening {
                        Opening::Error => Err(ComputeAqlQueueSessionErrorV1::Contract("opening")),
                        Opening::Panic => std::panic::panic_any("opening"),
                        _ => f.memory.primary_loan(&mut f.foundation).map_err(Into::into),
                    }
                },
                |f| {
                    f.calls[1] += 1;
                    let result = prepare_public_fixed_dispatch_resources_after_detach_in_place(
                        &mut f.memory,
                        programs,
                        preparation,
                        generation,
                    )
                    .map_err(Into::into);
                    if suppress_operation_error {
                        assert!(result.is_err());
                        Ok(())
                    } else {
                        result
                    }
                },
                |f, loan| {
                    f.calls[2] += 1;
                    match closing {
                        Closing::PreError => {
                            return Err(ComputeAqlQueueSessionErrorV1::Contract("closing"));
                        }
                        Closing::PrePanic => std::panic::panic_any("closing"),
                        Closing::Regress => f.memory.primary_regress_loan_revision_v1(&loan),
                        _ => {}
                    }
                    f.memory.primary_reclaim(&mut f.foundation, loan)?;
                    match closing {
                        Closing::PostError => {
                            Err(ComputeAqlQueueSessionErrorV1::Contract("closing"))
                        }
                        Closing::PostPanic => std::panic::panic_any("closing"),
                        _ => Ok(()),
                    }
                },
                |f| f.poisoned = true,
            )?;
            closing?;
            result
        },
        |_, preparation| {
            let mut f = fixture.borrow_mut();
            f.calls[3] += 1;
            if validation == Validation::Bypass {
                return Ok(());
            }
            let authorities = preparation.completed()?.device_authorities_inline_v1();
            preparation.primary_assert_snapshot_v1(
                &f.memory,
                snapshot.borrow().as_ref().unwrap(),
                None,
            );
            preparation.primary_assert_replacement_generation_v1(Some(predecessor + 1), None);
            if matches!(validation, Validation::Error | Validation::Panic) {
                f.memory
                    .primary_arm_native("currentness", validation == Validation::Panic);
            }
            f.memory
                .primary_validate_live_dispatch_memory_v1(&authorities)
                .map_err(Into::into)
        },
        |root| {
            assert_eq!(&*root as *const _, root_identity);
            *retained.borrow_mut() = Some(root);
        },
    );
    let f = fixture.borrow();
    let opened = opening == Opening::None;
    let generation_valid = predecessor < u64::MAX - 1;
    let operation_ok = opened && generation_valid && operation.is_none();
    let validation_entered =
        opened && (operation_ok || suppress_operation_error) && closing == Closing::None;
    assert_eq!(
        f.calls,
        [
            1,
            usize::from(opened),
            usize::from(opened),
            usize::from(validation_entered)
        ]
    );
    let success = operation_ok && closing == Closing::None && validation == Validation::None;
    let panics = opening == Opening::Panic
        || opened && operation.is_some_and(|(_, panic)| panic)
        || opened && matches!(closing, Closing::PrePanic | Closing::PostPanic)
        || validation_entered && validation == Validation::Panic;
    assert_eq!(result.result.is_err(), panics);
    assert_eq!(result.transport, !success);
    assert_eq!(session.terminal_poisoned, !success);
    assert_eq!(
        f.poisoned,
        opening == Opening::Panic
            || opened && (operation.is_some_and(|(_, panic)| panic) || closing != Closing::None)
    );
    // This is the outer bind producer only, not the injected fixture loan-poison callback.
    assert_eq!(take_dispatch_terminal_process_gate_record_v1(), panics);
    match result.result {
        Err(payload) => {
            if opening == Opening::Panic {
                assert_eq!(payload.downcast_ref::<&str>(), Some(&"opening"));
            } else if let Some((stage, true)) = operation {
                assert_eq!(
                    payload.downcast_ref::<(&str, PreparationStageV1)>(),
                    Some(&("dispatch preparation", stage))
                );
            } else if matches!(closing, Closing::PrePanic | Closing::PostPanic) {
                assert_eq!(payload.downcast_ref::<&str>(), Some(&"closing"));
            } else {
                assert_eq!(validation, Validation::Panic);
                assert_eq!(
                    payload.downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "currentness"))
                );
            }
        }
        Ok(result) => {
            assert_eq!(result.is_ok(), success);
            if opened && matches!(closing, Closing::PreError | Closing::PostError) {
                assert!(matches!(
                    result,
                    Err(ComputeAqlQueueSessionErrorV1::Contract("closing"))
                ));
            }
        }
    }
    let state = f.memory.primary_loan_state_v1(&f.foundation);
    assert_eq!(state.0, loan_before.0);
    assert_eq!(
        state.2,
        if opened {
            loan_before.2 + 1
        } else {
            loan_before.2
        }
    );
    assert_eq!(
        state.1,
        (opened
            && matches!(
                closing,
                Closing::PreError | Closing::PrePanic | Closing::Regress
            ))
        .then_some(loan_before.2)
    );
    if state.1.is_none() {
        f.memory.primary_authenticate(&f.foundation).unwrap();
    }
    let root = retained.into_inner();
    let mut owners = PreparationOwnerRefsV1::default();
    if success {
        assert!(root.is_none());
        let dispatch = session.dispatch.as_ref().unwrap();
        assert_eq!(
            dispatch.primary_fixture_next_generation_v1(),
            predecessor + 1
        );
        owners.dispatch(dispatch);
        assert_eq!(session.detached_data_count, 0);
        assert_eq!(session.detached_dispatch_generation, None);
        assert!(session.detached_data_identities.is_empty());
        assert_eq!(session.detached_data_identities.as_ptr(), ledger_storage);
        assert_eq!(session.detached_next_insertion_index, None);
    } else {
        let root = root.as_ref().unwrap();
        assert!(session.dispatch.is_none());
        assert_eq!(session.detached_data_count, ledger.len());
        assert_eq!(session.detached_data_identities, ledger);
        assert_eq!(session.detached_data_identities.as_ptr(), ledger_storage);
        assert_eq!(session.detached_dispatch_generation, Some(predecessor));
        assert_eq!(session.detached_next_insertion_index, Some(ledger.len()));
        assert_eq!(root.predecessor, Some(predecessor));
        assert!(root.packets.is_none() && root.data.is_none());
        let programs = root.programs.as_ref().unwrap();
        assert_eq!(
            (programs.as_ptr(), programs.len(), programs.capacity()),
            programs_storage
        );
        assert_eq!(
            programs
                .iter()
                .map(|p| (
                    p.identity_inputs(),
                    p.dispatch_abi_identity(),
                    p.selected_kernel_index()
                ))
                .collect::<Vec<_>>(),
            programs_identity
        );
        let preparation = root.preparation.as_ref().unwrap();
        preparation.primary_assert_snapshot_v1(
            &f.memory,
            snapshot.borrow().as_ref().unwrap(),
            None,
        );
        preparation.primary_assert_replacement_generation_v1(
            (opened && generation_valid).then(|| predecessor + 1),
            None,
        );
        if opened
            && generation_valid
            && let Some((stage, _)) = operation
        {
            preparation.primary_assert_failed_stage_v1(stage);
        }
        if opened && !generation_valid {
            preparation.primary_assert_failed_stage_v1(PreparationStageV1::Generation);
        }
        preparation.primary_collect_owners_v1(&mut owners);
    }
    f.memory
        .primary_assert_device_partition_v1(&owners.device_leases, &owners.device_authorities);
    f.memory.primary_assert_shared_layouts_v1(&owners.shared);
    let mut shared = owners.shared.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    shared.extend(f.memory.primary_terminal_identities());
    let expected = f.memory.primary_identities();
    assert_eq!(shared.len(), expected.len());
    for id in expected {
        assert_eq!(shared.iter().filter(|&&owner| owner == id).count(), 1);
    }
    for marker in owners.in_session {
        assert!(shared.contains(&marker));
    }
    f.memory
        .primary_assert_accounts_and_records(original_session);
    f.memory.assert_data_unchanged(&before);
    let after = f.memory.observation();
    assert_eq!(after.host, before.host);
    assert_eq!(after.device, before.device);
    assert_eq!(
        &after.calls[5..],
        &before.calls[5..],
        "no GPU unmap, free or VA release"
    );
    let native_stage_failed = opened
        && generation_valid
        && operation.is_some_and(|(stage, _)| {
            !matches!(
                stage,
                PreparationStageV1::Generation
                    | PreparationStageV1::Plan
                    | PreparationStageV1::Capacity
                    | PreparationStageV1::DataRetention
                    | PreparationStageV1::CodeAllocate(0)
            )
        });
    assert_eq!(
        after.phase,
        if native_stage_failed || validation_entered && validation == Validation::Error {
            SharedMemorySessionPhaseV1::Quarantined
        } else {
            SharedMemorySessionPhaseV1::Active
        }
    );
}

#[test]
fn ordinary_rebind_preflight_retains_exact_inputs_and_preserves_rejection_class() {
    for case in 0..8 {
        let (mut fixture, data) = fixture();
        let mut key = test_queue_key(610, 3);
        key.vm = fixture.foundation.identity().vms()[0].key;
        let mut session = persistent_compute_cancellation_test_session(key, None, None);
        session.observation.ring_bytes = 4096;
        session.detached_dispatch_generation = Some(7);
        session.detached_data_count = data.len();
        session.detached_data_identities = fixed_dispatch_storage_identities(&data);
        match case {
            0 => session.poison_terminal(),
            1 => {
                let loan = fixture
                    .memory
                    .primary_loan(&mut fixture.foundation)
                    .unwrap();
                let existing_data = fixture.memory.roster();
                let mut preparation =
                    FixedDispatchPreparationCustodyV1::new([packet(0)], existing_data);
                prepare_public_fixed_dispatch_resources_after_detach_in_place(
                    &mut fixture.memory,
                    &programs(),
                    &mut preparation,
                    11,
                )
                .unwrap();
                fixture
                    .memory
                    .primary_reclaim(&mut fixture.foundation, loan)
                    .unwrap();
                session.dispatch = Some(preparation.take_completed().unwrap());
            }
            2 => session.detached_dispatch_generation = None,
            3 => session.detached_data_count += 1,
            4 => {
                session.detached_data_count += 1;
                session
                    .detached_data_identities
                    .push(data[0].storage_identity());
            }
            5 => session.detached_data_identities.swap(0, 1),
            6 => session.observation.ring_bytes = 0,
            7 => {
                session.completion_owner.bind_barrier_probe().unwrap();
            }
            _ => unreachable!(),
        }
        let before = fixture.memory.observation();
        let metadata = (
            session.detached_data_count,
            session.detached_dispatch_generation,
            session.detached_data_identities.clone(),
            session.detached_data_identities.as_ptr(),
            session.detached_data_identities.capacity(),
            session.detached_next_insertion_index,
        );
        let state = fixture.memory.primary_loan_state_v1(&fixture.foundation);
        let original_dispatch = session.dispatch.as_ref().map(std::ptr::from_ref);
        let packets = [packet(0), packet(1), packet(2)];
        let mut original = PrimaryPreparationSnapshotV1::packets(&packets);
        original.capture_data_vector_v1(&data, data.capacity());
        let programs = programs();
        let program_storage = (programs.as_ptr(), programs.len(), programs.capacity());
        let identities = programs
            .iter()
            .map(|p| (p.identity_inputs(), p.dispatch_abi_identity()))
            .collect::<Vec<_>>();
        let root = LiveRebindRootV1::new(
            programs,
            packets,
            data,
            session.detached_dispatch_generation,
        );
        let root_identity = &*root as *const _;
        let retained = RefCell::new(None);
        let _ = take_dispatch_terminal_process_gate_record_v1();
        let settled = session.settle_fixed_dispatch_rebind_with_v1(
            root,
            |_, _, _, _| panic!("preflight entered preparation"),
            |_, _| panic!("preflight entered validation"),
            |root| {
                assert_eq!(&*root as *const _, root_identity);
                *retained.borrow_mut() = Some(root);
            },
        );
        let error = settled.result.unwrap().unwrap_err();
        match case {
            0 => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::Poisoned
                )
            )),
            1 | 2 => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                )
            )),
            3 => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::Contract(
                    "detached dispatch-data identity ledger cardinality"
                )
            )),
            4 => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::InvalidData {
                        detail: "detached dispatch-data cardinality",
                        ..
                    }
                )
            )),
            5 => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::InvalidData {
                        index: 0,
                        detail: "detached rebind storage identity"
                    }
                )
            )),
            6 => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::RingCapacity {
                        requested: 3,
                        capacity: 0
                    }
                )
            )),
            7 => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::Completion(Gfx942CompletionErrorV1::Poisoned)
            )),
            _ => unreachable!(),
        }
        let terminal = (2..=5).contains(&case);
        assert_eq!(settled.transport, terminal);
        assert_eq!(session.terminal_poisoned, terminal || case == 0);
        assert_eq!(
            (
                session.detached_data_count,
                session.detached_dispatch_generation,
                session.detached_data_identities.clone(),
                session.detached_data_identities.as_ptr(),
                session.detached_data_identities.capacity(),
                session.detached_next_insertion_index
            ),
            metadata
        );
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        assert_eq!(
            session.dispatch.as_ref().map(std::ptr::from_ref),
            original_dispatch
        );
        assert_eq!(fixture.memory.observation(), before);
        assert_eq!(
            fixture.memory.primary_loan_state_v1(&fixture.foundation),
            state
        );
        let mut root = retained.into_inner().unwrap();
        assert!(root.preparation.is_none());
        let programs = root.programs.as_ref().unwrap();
        assert_eq!(
            (programs.as_ptr(), programs.len(), programs.capacity()),
            program_storage
        );
        assert_eq!(
            programs
                .iter()
                .map(|p| (p.identity_inputs(), p.dispatch_abi_identity()))
                .collect::<Vec<_>>(),
            identities
        );
        // Reuse the descriptor oracle without running preparation.
        let preparation = FixedDispatchPreparationCustodyV1::new(
            root.packets.take().unwrap(),
            root.data.take().unwrap(),
        );
        preparation.primary_assert_descriptors_v1(&original, None);
        let mut owners = PreparationOwnerRefsV1::default();
        preparation.primary_collect_owners_v1(&mut owners);
        if let Some(dispatch) = &session.dispatch {
            owners.dispatch(dispatch);
        }
        fixture
            .memory
            .primary_assert_device_partition_v1(&owners.device_leases, &owners.device_authorities);
        fixture
            .memory
            .primary_assert_shared_layouts_v1(&owners.shared);
        fixture
            .memory
            .primary_assert_accounts_and_records(fixture.memory.primary_session_id());
    }
}

#[test]
fn ordinary_rebind_real_loan_operation_retake_matrix_preserves_custody() {
    for operation in [
        None,
        Some((PreparationStageV1::CodeResolve(0), false)),
        Some((PreparationStageV1::CodeResolve(0), true)),
    ] {
        for closing in [
            Closing::None,
            Closing::PreError,
            Closing::PrePanic,
            Closing::PostError,
            Closing::PostPanic,
            Closing::Regress,
        ] {
            exercise(
                7,
                Opening::None,
                operation,
                closing,
                Validation::None,
                false,
            );
        }
    }
}

#[test]
fn ordinary_rebind_opening_and_validation_faults_keep_original_owners() {
    for opening in [Opening::Error, Opening::Panic, Opening::Exhausted] {
        exercise(7, opening, None, Closing::None, Validation::None, false);
    }
    for validation in [Validation::Error, Validation::Panic] {
        exercise(7, Opening::None, None, Closing::None, validation, false);
    }
}

#[test]
fn ordinary_rebind_generation_and_suppressed_failure_never_install_invalid_owner() {
    for predecessor in [0, u64::MAX - 2, u64::MAX - 1, u64::MAX] {
        exercise(
            predecessor,
            Opening::None,
            None,
            Closing::None,
            Validation::None,
            false,
        );
    }
    for validation in [Validation::None, Validation::Bypass] {
        exercise(
            7,
            Opening::None,
            Some((PreparationStageV1::Complete, false)),
            Closing::None,
            validation,
            true,
        );
    }
}

#[test]
fn ordinary_rebind_all_preparation_stages_retain_original_prefixes() {
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
    assert_eq!(stages.len(), 31);
    for stage in stages {
        for panic in [false, true] {
            exercise(
                7,
                Opening::None,
                Some((stage, panic)),
                Closing::None,
                Validation::None,
                false,
            );
        }
    }
}
