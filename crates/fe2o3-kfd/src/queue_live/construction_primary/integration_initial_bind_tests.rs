//! Initial binding on the original completed constructor, not an engine-less facade.

use super::*;
use crate::queue::dispatch_binding::preparation::PreparationOwnerRefsV1;
use crate::queue::live::initial_bind::{
    InitialBindingCustodyV1, InitialBindingParentV1, InitialPreparationResultV1,
    bind_initial_with_v1, initial_dispatch_state_admitted_v1,
};

struct InitializerDrop;

#[test]
fn scaled_initial_binding_preserves_bootstrap_capacity_and_bounds_inputs_before_effects() {
    let (memory, trace) = setup_memory();
    trace.borrow_mut().local_gate = Some(LocalGateV1::new());
    let (capacity, account) = super::super::capacity_cases::capacity();
    let mut constructor = Root::<()>::new_with(memory, ());
    constructor.dispatch_capacity = capacity.clone();
    let (mut constructor, result) = run_with(
        constructor,
        QueueRingBackingV1::AqlSpecial,
        None,
        |_| Ok(()),
    );
    assert!(result.is_ok());
    let mut parent = Some(parent_from_completed(constructor.completed.take().unwrap()));
    let (programs, packets) = recipe();
    let before = parent
        .as_ref()
        .unwrap()
        .engine
        .backend
        .session
        .observation();
    let root = InitialBindingCustodyV1::new(programs, packets, |_: &mut Memory, _| {
        panic!("multi-packet initializer must not run")
    });
    let mut retained = None;
    assert!(bind_initial_with_v1(&mut parent, root, 1, |root| retained = Some(root)).is_err());
    assert!(retained.unwrap().terminal_parent.is_none());
    assert_eq!(
        parent
            .as_ref()
            .unwrap()
            .engine
            .backend
            .session
            .observation(),
        before
    );
    assert_eq!(account.usage().retained_records, 0);

    let (programs, [packet, _, _]) = recipe();
    let root = InitialBindingCustodyV1::new(programs, [packet], |memory: &mut Memory, _| {
        assert_eq!(account.usage().retained_records, 1);
        assert!(
            account
                .reserve(fe2o3_resource_accounting::ResourceVectorV1::ZERO.with(
                    fe2o3_resource_accounting::ResourceKindV1::ControlResidentBytes,
                    1,
                ))
                .is_err()
        );
        Ok(memory.host(true))
    });
    bind_initial_with_v1(&mut parent, root, 1, |_| panic!("valid initial binding")).unwrap();
    parent
        .as_mut()
        .unwrap()
        .dispatch
        .as_mut()
        .unwrap()
        .primary_fixture_exercise_capacity_v1(&capacity);
    assert_eq!(account.usage().retained_records, 1);
    drop(parent);
    assert_eq!(account.usage().retained_records, 0);
}

impl Drop for InitializerDrop {
    fn drop(&mut self) {
        step("initial-initializer-drop").unwrap();
    }
}

#[test]
fn scaled_initial_credit_exhaustion_preserves_parent_before_currentness_and_data() {
    use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceVectorV1};
    let (memory, trace) = setup_memory();
    trace.borrow_mut().local_gate = Some(LocalGateV1::new());
    let account = ResourceCreditAccountV1::new(ResourceVectorV1::ZERO, 1).unwrap();
    let mut constructor = Root::<()>::new_with(memory, ());
    constructor.dispatch_capacity =
        Gfx942FixedDispatchCapacityV1::qualification_1024(account.clone());
    let (mut constructor, result) = run_with(
        constructor,
        QueueRingBackingV1::AqlSpecial,
        None,
        |_| Ok(()),
    );
    assert!(result.is_ok());
    let mut parent = Some(parent_from_completed(constructor.completed.take().unwrap()));
    let (programs, [packet, _, _]) = recipe();
    let before = parent
        .as_ref()
        .unwrap()
        .engine
        .backend
        .session
        .observation();
    let mut calls = trace.borrow().calls.clone();
    // Borrowed owner validation precedes the credit reservation, not native work.
    calls.push("release-validate-owners");
    let root = InitialBindingCustodyV1::new(programs, [packet], |_: &mut Memory, _| {
        panic!("credit exhaustion cannot enter initializer")
    });
    let mut retained = None;
    let result = bind_initial_with_v1(&mut parent, root, 1, |root| retained = Some(root));
    assert!(matches!(
        result,
        Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
            Gfx942DispatchBindingErrorV1::HostAllocationCapacity { .. }
        ))
    ));
    assert!(!parent.as_ref().unwrap().poisoned);
    assert_eq!(
        parent
            .as_ref()
            .unwrap()
            .engine
            .backend
            .session
            .observation(),
        before
    );
    assert_eq!(trace.borrow().calls, calls);
    let retained = retained.unwrap();
    assert!(retained.terminal_parent.is_none());
    assert!(retained.preparation.is_none());
    assert!(retained.prepared_generation.is_none());
    assert!(retained.data.is_empty());
    assert!(!trace.borrow().calls.contains(&"initial-validation"));
    assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn scaled_initial_post_entry_failures_retain_preallocated_epoch_storage() {
    for stage in ["initial-currentness", "initial-data", "generation"] {
        for panic in [false, true] {
            let (memory, trace) = setup_memory();
            trace.borrow_mut().local_gate = Some(LocalGateV1::new());
            let (capacity, account) = super::super::capacity_cases::capacity();
            let mut constructor = Root::<()>::new_with(memory, ());
            constructor.dispatch_capacity = capacity;
            let (mut constructor, result) = run_with(
                constructor,
                QueueRingBackingV1::AqlSpecial,
                None,
                |_| Ok(()),
            );
            assert!(result.is_ok());
            let mut parent = Some(parent_from_completed(constructor.completed.take().unwrap()));
            let (programs, [packet, _, _]) = recipe();
            let mut root =
                InitialBindingCustodyV1::new(programs, [packet], |memory: &mut Memory, _| {
                    assert_eq!(account.usage().retained_records, 1);
                    step("initial-data")?;
                    Ok(memory.host(true))
                });
            if stage == "generation" {
                root.preparation_fault = Some((PreparationStageV1::Generation, panic));
            } else {
                trace.borrow_mut().fault = Some((stage, 1, panic));
            }
            let retained = RefCell::new(None);
            let result = catch_unwind(AssertUnwindSafe(|| {
                bind_initial_with_v1(&mut parent, root, 1, |root| {
                    *retained.borrow_mut() = Some(root)
                })
            }));
            assert_eq!(result.is_err(), panic);
            assert!(!matches!(result, Ok(Ok(()))));
            assert!(parent.is_none());
            let retained = retained.into_inner().unwrap();
            assert!(retained.terminal_parent.as_ref().unwrap().poisoned);
            assert_eq!(
                retained.prepared_generation.is_some(),
                stage != "generation"
            );
            assert_eq!(retained.preparation.is_some(), stage == "generation");
            assert_eq!(account.usage().retained_records, 1);
            trace.borrow_mut().fault = None;
            // CPU fixture teardown is not native terminal-custody recovery.
            drop(retained);
            assert_eq!(
                account.usage().used,
                fe2o3_resource_accounting::ResourceVectorV1::ZERO
            );
        }
    }
}

impl InitialBindingParentV1 for Option<Parent> {
    type Memory = Memory;
    type TerminalParent = Parent;

    fn dispatch_capacity(&self) -> Gfx942FixedDispatchCapacityV1 {
        self.as_ref().unwrap().dispatch_capacity.clone()
    }

    fn preflight<const N: usize>(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let parent = self
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract("retained parent"))?;
        if parent.poisoned
            || !initial_dispatch_state_admitted_v1(
                &parent.unpublished,
                parent.dispatch.is_some(),
                parent.detached_generation,
                parent.detached_count,
                parent.detached_identities.len(),
                parent.detached_next,
            )
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        validate_fixed_batch_ring::<N>(4096)?;
        parent.completion.ensure_releasable()?;
        parent
            .dependency
            .ensure_idle()
            .map_err(map_dependency_target_use_error_v1)?;
        preflight_primary_owners_v1::<Fixture>(
            &parent.engine,
            parent.key,
            parent.signals.as_ref(),
            parent.exception.as_ref(),
            parent.doorbell.as_ref(),
        )
    }

    fn currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        step("initial-currentness")?;
        self.as_mut()
            .unwrap()
            .engine
            .prepare_operation()
            .map_err(map_native)
    }

    fn with_preparation(
        &mut self,
        work: impl FnOnce(&mut Memory) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    ) -> InitialPreparationResultV1 {
        execute_live_model_custody_v1(
            self.as_mut().unwrap(),
            |parent| {
                step("initial-loan")?;
                let engine = &mut parent.engine;
                engine
                    .backend
                    .session
                    .primary_loan(&mut engine.foundation)
                    .map_err(Into::into)
            },
            |parent| work(&mut parent.engine.backend.session),
            |parent, loan| {
                step("initial-retake-before")?;
                let engine = &mut parent.engine;
                if trace()
                    .borrow()
                    .fault
                    .is_some_and(|(name, _, _)| name == "initial-reclaim-regress")
                {
                    engine
                        .backend
                        .session
                        .primary_regress_loan_revision_v1(&loan);
                }
                engine
                    .backend
                    .session
                    .primary_reclaim(&mut engine.foundation, loan)?;
                step("initial-retake-after")
            },
            |parent| parent.poison_release(),
        )
    }

    fn validate<const N: usize>(
        &mut self,
        preparation: &FixedDispatchPreparationCustodyV1<N>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let parent = self.as_mut().unwrap();
        assert!(parent.dispatch.is_none());
        step("initial-validation")?;
        parent
            .engine
            .backend
            .session
            .primary_validate_live_dispatch_memory_v1(
                &preparation.completed()?.device_authorities_inline_v1(),
            )
            .map_err(Into::into)
    }

    fn install(&mut self, dispatch: DispatchResourceOwnerV1) {
        self.as_mut().unwrap().dispatch = Some(dispatch);
    }

    fn take_terminal_parent(&mut self) -> Parent {
        let mut parent = self.take().unwrap();
        parent.poison_release();
        parent
    }
}

fn exercise(
    fault: Option<(&'static str, usize, bool)>,
    preparation_fault: Option<(PreparationStageV1, bool)>,
    sdma: bool,
) {
    let (parent, t, gate) = if sdma {
        sdma_cases::with_sdma(false)
    } else {
        constructed(false)
    };
    let baseline = parent.engine.backend.session.primary_identities();
    let original_data = parent.engine.backend.session.observation();
    let original_drops = t.borrow().drops;
    let original_resources = original_resource_ids(&parent);
    let original_signal = Memory::primary_token_identity(parent.signals.as_ref().unwrap());
    let original_key = parent.key;
    let platform = |p: &Parent| {
        let e = p.exception.as_ref().unwrap();
        (
            e.runtime.identity,
            e.event.identity,
            e.shadows.identity,
            p.doorbell.as_ref().unwrap().identity,
        )
    };
    let original_platform = platform(&parent);
    let mut parent = Some(parent);
    if fault.is_some_and(|(name, _, _)| name == "initial-loan-exhausted") {
        parent
            .as_mut()
            .unwrap()
            .engine
            .backend
            .session
            .primary_expire_loan_generation_v1();
    }
    let p = parent.as_ref().unwrap();
    let original_loan = p
        .engine
        .backend
        .session
        .primary_loan_state_v1(&p.engine.foundation);
    let initialized = RefCell::new(Vec::with_capacity(3));
    let initialized_ref = &initialized;
    let drop_probe = InitializerDrop;
    let initializer = move |memory: &mut Memory, index: usize| {
        let _ = &drop_probe;
        step("initial-data")?;
        assert_eq!(index, initialized_ref.borrow().len());
        let data = if index == 1 {
            memory.device(true)
        } else {
            memory.host(true)
        };
        initialized_ref.borrow_mut().push(data.storage_identity());
        Ok(data)
    };
    let (programs, packets) = recipe();
    let mut root = InitialBindingCustodyV1::new(programs, packets, initializer);
    root.preparation_fault = preparation_fault;
    let address = &*root as *const _ as usize;
    let retained = RefCell::new(None);
    t.borrow_mut().fault = fault;
    let _ = take_dispatch_terminal_process_gate_record_v1();
    let result = catch_unwind(AssertUnwindSafe(|| {
        bind_initial_with_v1(&mut parent, root, 3, |root| {
            assert_eq!(&*root as *const _ as usize, address);
            *retained.borrow_mut() = Some(root);
        })
    }));
    let success = fault.is_none() && preparation_fault.is_none();
    let panic = preparation_fault.is_some_and(|(_, panic)| panic)
        || fault.is_some_and(|(_, _, panic)| panic);
    assert_eq!(matches!(result, Ok(Ok(()))), success);
    assert_eq!(result.is_err(), panic);
    if let Ok(Err(error)) = &result {
        match fault.map(|(name, _, _)| name) {
            Some("initial-loan-exhausted") => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Model(
                    "fixture live foundation loan"
                ))
            )),
            Some("initial-reclaim-regress") => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Model(
                    "fixture live foundation reclaim"
                ))
            )),
            Some(name) => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::Contract(actual) if *actual == name
            )),
            None => assert!(matches!(
                error,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::InvalidCode("injected preparation stage")
                )
            )),
        }
    }
    if let Err(payload) = &result {
        if let Some((stage, true)) = preparation_fault {
            assert_eq!(
                payload.downcast_ref::<(&str, PreparationStageV1)>(),
                Some(&("dispatch preparation", stage))
            );
        } else {
            let (name, occurrence, _) = fault.unwrap();
            assert_eq!(
                payload.downcast_ref::<(&str, usize)>(),
                Some(&(name, occurrence))
            );
        }
    }
    assert_eq!(take_dispatch_terminal_process_gate_record_v1(), !success);
    assert_eq!(parent.is_some(), success);
    let retained = retained.into_inner();
    assert_eq!(retained.is_some(), !success);
    let p = parent
        .as_ref()
        .or_else(|| retained.as_ref().unwrap().terminal_parent.as_ref())
        .unwrap();
    assert_eq!(p.poisoned, !success);
    assert_eq!(p.key, original_key);
    assert_eq!(original_resource_ids(p), original_resources);
    assert_eq!(
        Memory::primary_token_identity(p.signals.as_ref().unwrap()),
        original_signal
    );
    assert_eq!(platform(p), original_platform);
    assert_eq!(p.sdma.is_some(), sdma);
    assert!(p.unpublished.is_clear());
    assert_eq!(
        (p.detached_count, p.detached_generation, p.detached_next),
        (0, None, None)
    );
    assert!(p.detached_identities.is_empty());
    let memory = &p.engine.backend.session;
    let before_loan = fault.is_some_and(|(name, _, _)| {
        matches!(
            name,
            "initial-currentness" | "initial-loan" | "initial-loan-exhausted"
        )
    });
    let retained_loan = fault.is_some_and(|(name, _, _)| {
        matches!(name, "initial-retake-before" | "initial-reclaim-regress")
    });
    assert_eq!(
        memory.primary_loan_state_v1(&p.engine.foundation),
        (
            original_loan.0,
            retained_loan.then_some(original_loan.2),
            if before_loan {
                original_loan.2
            } else {
                original_loan.2 + 1
            },
        )
    );
    memory.primary_assert_accounts_and_records(t.borrow().session);
    memory.assert_original_records_unchanged(&original_data);
    let mut expected_device = original_data.device.unwrap();
    if initialized.borrow().len() >= 2 {
        expected_device.used_backing_bytes += 4096;
        expected_device.used_allocation_records += 1;
        expected_device.retained_records += 1;
    }
    assert_eq!(memory.observation().device, Some(expected_device));
    assert_eq!(t.borrow().drops, original_drops);
    assert!(
        !t.borrow()
            .calls
            .iter()
            .any(|name| matches!(*name, "create" | "publish" | "destroy"))
    );
    let mut owners = PreparationOwnerRefsV1::default();
    if let Some(root) = &retained {
        owners.data(&root.data);
        if let Some(preparation) = &root.preparation {
            preparation.primary_collect_owners_v1(&mut owners);
            preparation.primary_assert_replacement_generation_v1(Some(1), None);
            if let Some((stage, _)) = preparation_fault {
                preparation.primary_assert_failed_stage_v1(stage);
            }
        } else {
            assert_eq!(
                fixed_dispatch_storage_identities(&root.data),
                *initialized.borrow()
            );
        }
        assert!(p.dispatch.is_none());
    } else {
        let dispatch = p.dispatch.as_ref().unwrap();
        assert_eq!(dispatch.primary_fixture_next_generation_v1(), 1);
        owners.dispatch(dispatch);
    }
    memory.primary_assert_shared_layouts_v1(&owners.shared);
    memory.primary_assert_device_partition_v1(&owners.device_leases, &owners.device_authorities);
    let mut identities = baseline;
    identities.extend(owners.shared.iter().map(|(id, _)| *id));
    identities.extend(memory.primary_terminal_identities());
    let expected = memory.primary_identities();
    assert_eq!(identities.len(), expected.len());
    assert_eq!(
        identities
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        identities.len()
    );
    assert_eq!(
        identities
            .into_iter()
            .collect::<std::collections::HashSet<_>>(),
        expected.into_iter().collect()
    );
    assert_eq!(gate.observation(), (false, false));
    if success {
        let p = parent.as_mut().unwrap();
        p.engine
            .backend
            .session
            .primary_authenticate(&p.engine.foundation)
            .unwrap();
        assert!(
            parent.preflight::<3>().is_err(),
            "initial entry is one-shot"
        );
        t.borrow_mut().fault = None;
        let mut release = PrimaryReleaseStateV1::<Fixture>::new();
        release.release_in_place(parent.as_mut().unwrap()).unwrap();
        parent
            .as_ref()
            .unwrap()
            .engine
            .backend
            .session
            .primary_assert_all_released_v1();
        if let Some(sdma) = release.sdma.as_mut() {
            sdma.cleanup_local_mappings();
        }
    } else {
        let before = p.engine.backend.session.observation();
        let calls = t.borrow().calls.clone();
        assert!(parent.preflight::<3>().is_err());
        assert_eq!(p.engine.backend.session.observation(), before);
        assert_eq!(t.borrow().calls, calls);
    }
}

#[test]
fn constructed_initial_binding_uses_original_primary_with_and_without_sdma() {
    exercise(None, None, false);
    exercise(None, None, true);
}

#[test]
fn constructed_initial_binding_keeps_every_materialized_prefix_on_error_and_panic() {
    for index in 1..=3 {
        for panic in [false, true] {
            exercise(Some(("initial-data", index, panic)), None, false);
        }
    }
}

#[test]
fn constructed_initial_binding_keeps_every_preparation_prefix_on_error_and_panic() {
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
    for index in 0..3 {
        stages.extend([
            PreparationStageV1::CodeAllocate(index),
            PreparationStageV1::CodeMaterialize(index),
            PreparationStageV1::CodeSeal(index),
            PreparationStageV1::CodeMap(index),
            PreparationStageV1::CodeRetain(index),
            PreparationStageV1::CodeResolve(index),
            PreparationStageV1::PacketResolve(index),
        ]);
    }
    for stage in stages {
        for panic in [false, true] {
            exercise(None, Some((stage, panic)), false);
        }
    }
}

#[test]
fn constructed_initial_binding_retains_original_parent_across_model_and_validation_failures() {
    for name in [
        "initial-currentness",
        "initial-loan",
        "initial-retake-before",
        "initial-retake-after",
        "initial-validation",
    ] {
        for panic in [false, true] {
            exercise(Some((name, 1, panic)), None, false);
        }
    }
    exercise(Some(("initial-loan-exhausted", 1, false)), None, false);
    exercise(Some(("initial-reclaim-regress", 1, false)), None, false);
    exercise(Some(("initial-initializer-drop", 1, true)), None, false);
    for name in ["initial-retake-before", "initial-retake-after"] {
        for operation_panic in [false, true] {
            for retake_panic in [false, true] {
                exercise(
                    Some((name, 1, retake_panic)),
                    Some((PreparationStageV1::CodeResolve(0), operation_panic)),
                    false,
                );
            }
        }
    }
}

#[test]
fn constructed_initial_binding_rejects_nonfresh_ledgers_before_any_effect() {
    for case in 0..5 {
        let (mut p, t, _) = constructed(case == 0);
        match case {
            0 => assert!(p.dispatch.is_some()),
            1 => p.detached_generation = Some(0),
            2 => p.detached_generation = Some(7),
            3 => p.detached_count = 1,
            4 => p.detached_next = Some(0),
            _ => unreachable!(),
        }
        let before = p.engine.backend.session.observation();
        let original_ids = original_resource_ids(&p);
        let original_drops = t.borrow().drops;
        let mut parent = Some(p);
        let (programs, packets) = recipe();
        let root = InitialBindingCustodyV1::new(programs, packets, |_: &mut Memory, _: usize| {
            panic!("rejected initial bind must not initialize")
        });
        let retained = RefCell::new(None);
        let _ = take_dispatch_terminal_process_gate_record_v1();
        assert!(
            bind_initial_with_v1(&mut parent, root, 3, |root| {
                *retained.borrow_mut() = Some(root);
            })
            .is_err()
        );
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        let retained = retained.into_inner().unwrap();
        assert!(
            retained.terminal_parent.is_none()
                && retained.data.is_empty()
                && retained.preparation.is_none()
        );
        let p = parent.as_ref().unwrap();
        assert!(!p.poisoned);
        assert_eq!(original_resource_ids(p), original_ids);
        assert_eq!(p.engine.backend.session.observation(), before);
        assert!(t.borrow().calls.is_empty());
        assert_eq!(t.borrow().drops, original_drops);
    }
}

#[test]
fn constructed_initial_binding_bounds_inputs_before_allocation_or_currentness() {
    let (p, t, _) = constructed(false);
    let before = p.engine.backend.session.observation();
    let mut parent = Some(p);
    let (programs, packets) = recipe();
    let root = InitialBindingCustodyV1::new(programs, packets, |_: &mut Memory, _: usize| {
        panic!("oversized initial bind must not initialize")
    });
    let retained = RefCell::new(None);
    assert!(
        bind_initial_with_v1(&mut parent, root, usize::MAX, |root| {
            *retained.borrow_mut() = Some(root);
        })
        .is_err()
    );
    let retained = retained.into_inner().unwrap();
    assert!(retained.terminal_parent.is_none() && retained.data.capacity() == 0);
    assert_eq!(
        parent
            .as_ref()
            .unwrap()
            .engine
            .backend
            .session
            .observation(),
        before
    );
    assert_eq!(t.borrow().calls, ["release-validate-owners"]);
}
