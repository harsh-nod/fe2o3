use super::*;
use crate::shared_memory::{PreparationMemoryCallV1 as Call, PreparationNativeFaultV1 as Fault};
use std::cell::Cell;

#[test]
fn primary_preparation_stage_failures_preserve_original_inputs_and_every_control_prefix() {
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
            let (mut root, trace) = setup();
            let address = &*root as *const Root as usize;
            root.preparation.1.primary_inject_stage_v1(stage, panic);
            let (root, result) = run(root, QueueRingBackingV1::AqlSpecial, false);
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
            root.preparation.1.primary_assert_failed_stage_v1(stage);
            assert_root(&root, address, &trace);
            assert_preparation_only(&root, &trace);
        }
    }
}

fn assert_preparation_only(root: &Root, trace: &Rc<RefCell<Trace>>) {
    let t = trace.borrow();
    assert_eq!(t.calls, ["retain-root"]);
    assert!(!t.poison);
    let before = t.initial_data.as_ref().unwrap();
    let after = memory(root).observation();
    assert_eq!(after.host, before.host);
    assert_eq!(after.device, before.device);
    assert_eq!(&after.calls[5..], &before.calls[5..]);
}

fn native_fault(call: Call, fault: Fault) {
    let (mut root, trace) = setup();
    let address = &*root as *const Root as usize;
    root.memory.as_mut().unwrap().fault = Some((call, fault));
    let (root, result) = run(root, QueueRingBackingV1::AqlSpecial, false);
    let payload = result.expect_err("actual preparation native boundary must reject");
    let panic_operation = match fault {
        Fault::Panic(operation) => Some(operation),
        Fault::CurrentnessPanic(_) => Some("currentness"),
        Fault::AccessPanic => Some("with_bytes_mut"),
        _ => None,
    };
    if let Fault::Projection(case) = fault
        && case.panic
    {
        case.assert_panic(&*payload);
    } else if let Some(operation) = panic_operation {
        assert_eq!(
            payload.downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", operation))
        );
    } else {
        let error = payload
            .downcast_ref::<ComputeAqlQueueSessionErrorV1>()
            .unwrap();
        let ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Memory(
            error,
        )) = error
        else {
            panic!("wrong preparation error: {error:?}");
        };
        match fault {
            Fault::Error(operation) => {
                assert!(matches!(error, MemorySessionError::Injected(at) if *at == operation))
            }
            Fault::CurrentnessError(_) => {
                assert!(matches!(error, MemorySessionError::Injected("currentness")))
            }
            Fault::ProjectionRejection => assert!(matches!(
                error,
                MemorySessionError::Model("shared allocation projection")
            )),
            Fault::Projection(_) => assert!(matches!(
                error,
                MemorySessionError::Injected("session projection")
            )),
            Fault::PartialMap(prefix, errno) => match (prefix, errno) {
                (2, _) => assert!(matches!(
                    error,
                    MemorySessionError::KernelResultMalformed(
                        "shared MAP_MEMORY_TO_GPU cumulative n_success"
                    )
                )),
                (_, true) => assert!(matches!(error, MemorySessionError::Injected("map_gpu"))),
                (0, false) => assert!(matches!(
                    error,
                    MemorySessionError::KernelResultMalformed(
                        "shared MAP_MEMORY_TO_GPU full prefix"
                    )
                )),
                _ => unreachable!("successful prefix is not a fault"),
            },
            _ => unreachable!("panic handled above"),
        }
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
    root.preparation.1.primary_assert_failed_stage_v1(stage);
    assert_root(&root, address, &trace);
    assert_preparation_only(&root, &trace);
    memory(&root).primary_assert_preparation_fault_v1(call, fault);
}

#[test]
fn primary_preparation_native_allocation_failures_retain_exact_pending_or_returned_output() {
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
        for delta in 1..=3 {
            native_fault(call, Fault::CurrentnessError(delta));
            native_fault(call, Fault::CurrentnessPanic(delta));
        }
        native_fault(call, Fault::ProjectionRejection);
    }
}

#[test]
fn primary_preparation_projection_failures_keep_exact_tokens_and_uncommitted_model() {
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
fn primary_preparation_seal_map_and_materialization_faults_preserve_original_session_tokens() {
    for i in 0..3 {
        native_fault(Call::SealCode(i), Fault::Error("protect_cpu_read_only"));
        native_fault(Call::SealCode(i), Fault::Panic("protect_cpu_read_only"));
        for delta in [1, 2] {
            native_fault(Call::SealCode(i), Fault::CurrentnessError(delta));
            native_fault(Call::SealCode(i), Fault::CurrentnessPanic(delta));
        }
    }
    for call in [
        Call::MapCode(0),
        Call::MapCode(1),
        Call::MapCode(2),
        Call::MapKernarg,
    ] {
        native_fault(call, Fault::Error("map_gpu"));
        native_fault(call, Fault::Panic("map_gpu"));
        for delta in [1, 2] {
            native_fault(call, Fault::CurrentnessError(delta));
            native_fault(call, Fault::CurrentnessPanic(delta));
        }
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
        for delta in [1, 2] {
            native_fault(call, Fault::CurrentnessError(delta));
            native_fault(call, Fault::CurrentnessPanic(delta));
        }
    }
}

struct Returned<'a> {
    token: Cpu<HostVisibleCoherentGttV1>,
    borrowed: &'a str,
    drops: Rc<Cell<usize>>,
}
impl Drop for Returned<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn primary_generic_return_keeps_borrowed_non_send_and_actual_charged_owner_through_finalization() {
    for boundary in [
        None,
        Some("runtime-created"),
        Some("doorbell"),
        Some("gate-finish"),
    ] {
        for panic in [false, true] {
            let text = String::from("original borrowed preparation");
            let drops = Rc::new(Cell::new(0));
            let calls = Cell::new(0);
            let identity = Cell::new(None);
            let (memory, trace) = setup_memory();
            let root = Root::new_with(memory, None::<Returned<'_>>);
            let address = &*root as *const Root<Option<Returned<'_>>> as usize;
            trace.borrow_mut().fault = boundary.map(|b| (b, 1, panic));
            let (root, result) = run_with(root, QueueRingBackingV1::AqlSpecial, None, |root| {
                capture_returned_preparation_v1(
                    root.memory.as_mut().unwrap(),
                    &mut root.preparation,
                    |memory| {
                        calls.set(calls.get() + 1);
                        let token = memory.allocate::<HostVisibleCoherentGttV1>(4096)?;
                        identity.set(Some(token.storage_identity()));
                        Ok(Returned {
                            token,
                            borrowed: &text,
                            drops: drops.clone(),
                        })
                    },
                )?;
                assert!(
                    capture_returned_preparation_v1(
                        root.memory.as_mut().unwrap(),
                        &mut root.preparation,
                        |_| { panic!("occupied output must not reinvoke preparation") }
                    )
                    .is_err()
                );
                Ok(())
            });
            if let Some(boundary) = boundary {
                let payload = result.expect_err("late failure must be reached");
                if panic {
                    assert_eq!(
                        payload.downcast_ref::<(&str, usize)>(),
                        Some(&(boundary, 1))
                    );
                } else {
                    assert!(payload.is::<ComputeAqlQueueSessionErrorV1>());
                }
            } else {
                assert!(result.is_ok());
            }
            assert_common(&root, address, &trace);
            assert_platform(&root, None);
            let returned = root.preparation.as_ref().unwrap();
            assert_eq!(Some(returned.token.storage_identity()), identity.get());
            assert_eq!(returned.borrowed.as_ptr(), text.as_ptr());
            assert_eq!(returned.borrowed, text);
            assert!(Rc::ptr_eq(&returned.drops, &drops));
            assert_eq!(drops.get(), 0);
            assert_eq!(calls.get(), 1);
            assert_partition(&root, vec![returned.token.storage_identity()]);
            if boundary == Some("gate-finish") || boundary.is_none() {
                assert!(root.completed.as_ref().unwrap().doorbell.is_some());
            }
            drop(root);
            assert_eq!(
                drops.get(),
                1,
                "fixture disposes its retained test root once"
            );
        }
    }
}
