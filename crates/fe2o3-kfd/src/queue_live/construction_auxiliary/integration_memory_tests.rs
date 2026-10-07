use super::*;
use crate::shared_memory::{
    PreparationMemoryCallV1 as Call, PreparationNativeFaultV1 as Fault, PrimaryProjectionCaseV1,
};

fn preparation_fault(call: Call, fault: Fault) {
    let (scope, result, trace) = prefix_case(|scope, _| {
        scope
            .parent
            .original
            .as_mut()
            .unwrap()
            .primary
            .completed
            .as_mut()
            .unwrap()
            .engine
            .backend
            .session
            .fault = Some((call, fault));
    });
    let payload = result.expect_err("actual auxiliary preparation transition must reject");
    let panic = match fault {
        Fault::Panic(operation) => Some(operation),
        Fault::CurrentnessPanic(_) => Some("currentness"),
        Fault::AccessPanic => Some("with_bytes_mut"),
        _ => None,
    };
    if let Some(operation) = panic {
        assert_eq!(
            payload.downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", operation))
        );
    } else {
        let ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Memory(
            error,
        )) = error_source(&*payload)
        else {
            panic!("wrong preparation failure: {:?}", error_source(&*payload));
        };
        match fault {
            Fault::Error(operation) => assert!(
                matches!(error, MemorySessionError::Injected(actual) if *actual == operation)
            ),
            Fault::CurrentnessError(_) => {
                assert!(matches!(error, MemorySessionError::Injected("currentness")))
            }
            Fault::PartialMap(_, true) => {
                assert!(matches!(error, MemorySessionError::Injected("map_gpu")))
            }
            Fault::PartialMap(0, false) => assert!(matches!(
                error,
                MemorySessionError::KernelResultMalformed("shared MAP_MEMORY_TO_GPU full prefix")
            )),
            Fault::PartialMap(2, false) => assert!(matches!(
                error,
                MemorySessionError::KernelResultMalformed(
                    "shared MAP_MEMORY_TO_GPU cumulative n_success"
                )
            )),
            _ => panic!("unexpected fault"),
        }
    }
    // Code ordinals continue after the original primary; preparation ordinals do not.
    let stage = match call {
        Call::AllocateCode(i) => PreparationStageV1::CodeAllocate(i.checked_sub(3).unwrap()),
        Call::WriteCode(i) => PreparationStageV1::CodeMaterialize(i.checked_sub(3).unwrap()),
        Call::SealCode(i) => PreparationStageV1::CodeSeal(i.checked_sub(3).unwrap()),
        Call::MapCode(i) => PreparationStageV1::CodeMap(i.checked_sub(3).unwrap()),
        Call::AllocateKernarg => PreparationStageV1::KernargAllocate,
        Call::WriteKernarg => PreparationStageV1::KernargMaterialize,
        Call::MapKernarg => PreparationStageV1::KernargMap,
    };
    scope
        .construction
        .preparation
        .as_ref()
        .unwrap()
        .primary_assert_failed_stage_v1(stage);
    memory(&scope.primary).primary_assert_preparation_fault_v1(call, fault);
    assert!(scope.construction.ring.is_none());
    assert!(trace.borrow().calls.contains(&"auxiliary-retake-complete"));
}

#[test]
fn same_engine_auxiliary_native_preparation_allocation_failures_keep_pending_or_returned_owners() {
    for call in [
        Call::AllocateCode(3),
        Call::AllocateCode(4),
        Call::AllocateCode(5),
        Call::AllocateKernarg,
    ] {
        for operation in ["reserve_va", "alloc", "map_cpu", "prepare_cpu_mapping"] {
            preparation_fault(call, Fault::Error(operation));
            preparation_fault(call, Fault::Panic(operation));
        }
        for delta in 1..=3 {
            preparation_fault(call, Fault::CurrentnessError(delta));
            preparation_fault(call, Fault::CurrentnessPanic(delta));
        }
    }
}

#[test]
fn same_engine_auxiliary_native_preparation_map_seal_write_failures_keep_exact_prefix() {
    for call in [
        Call::MapCode(3),
        Call::MapCode(4),
        Call::MapCode(5),
        Call::MapKernarg,
    ] {
        preparation_fault(call, Fault::Error("map_gpu"));
        preparation_fault(call, Fault::Panic("map_gpu"));
        for delta in 1..=2 {
            preparation_fault(call, Fault::CurrentnessError(delta));
            preparation_fault(call, Fault::CurrentnessPanic(delta));
        }
        for (prefix, errno) in [(0, false), (0, true), (1, true), (2, false)] {
            preparation_fault(call, Fault::PartialMap(prefix, errno));
        }
    }
    for i in 3..6 {
        preparation_fault(Call::SealCode(i), Fault::Error("protect_cpu_read_only"));
        preparation_fault(Call::SealCode(i), Fault::Panic("protect_cpu_read_only"));
        preparation_fault(Call::WriteCode(i), Fault::AccessPanic);
    }
    preparation_fault(Call::WriteKernarg, Fault::AccessPanic);
}

#[test]
fn same_engine_auxiliary_control_projection_failures_preserve_actual_tokens_and_model() {
    for (boundary, ordinal, allocation) in [
        ("allocate-ring", 1, true),
        ("native-control", 1, true),
        ("native-completion", 1, true),
        ("native-executable", 1, true),
        ("native-executable", 2, true),
        ("ring-map", 1, false),
        ("map-mutable", 1, false),
        ("map-mutable", 2, false),
        ("map-executable", 1, false),
        ("map-executable", 2, false),
    ] {
        for case in PrimaryProjectionCaseV1::cases(allocation) {
            let (scope, result, trace) = prefix_case(|_, trace| {
                let prior = trace
                    .borrow()
                    .calls
                    .iter()
                    .filter(|&&c| c == boundary)
                    .count();
                trace.borrow_mut().projection_fault = Some((boundary, prior + ordinal, case));
            });
            let payload = result.expect_err("actual auxiliary control projection must reject");
            if case.panic {
                case.assert_panic(&*payload);
            } else {
                assert!(matches!(
                    error_source(&*payload),
                    ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Injected(
                        "session projection"
                    ))
                ));
            }
            let primary = scope.primary.completed.as_ref().unwrap();
            let memory = &primary.engine.backend.session;
            let foundation = &primary.engine.foundation;
            match (boundary, ordinal) {
                ("allocate-ring" | "ring-map", _) => {
                    memory.primary_assert_projection_with_foundation_v1::<AqlQueueGttV1>(foundation)
                }
                ("native-control", _) | ("map-mutable", 1) => memory
                    .primary_assert_projection_with_foundation_v1::<UserptrAqlControlGttV1>(
                        foundation,
                    ),
                ("native-completion", _) | ("map-mutable", 2) => memory
                    .primary_assert_projection_with_foundation_v1::<HostVisibleCoherentGttV1>(
                        foundation,
                    ),
                _ => memory
                    .primary_assert_projection_with_foundation_v1::<ExecutableGttV1>(foundation),
            }
            assert!(trace.borrow().calls.contains(&"auxiliary-retake-complete"));
        }
    }
}

#[test]
fn same_engine_auxiliary_native_control_failures_retain_pending_and_in_session_custody() {
    for (boundary, ordinal, operation) in [
        ("native-control", 1, "alloc_userptr"),
        ("native-completion", 1, "alloc"),
        ("native-executable", 1, "alloc"),
        ("native-executable", 2, "alloc"),
        ("ring-map", 1, "map_gpu"),
        ("map-mutable", 1, "map_gpu"),
        ("map-mutable", 2, "map_gpu"),
        ("map-executable", 1, "map_gpu"),
        ("map-executable", 2, "map_gpu"),
        ("seal", 1, "protect_cpu_read_only"),
        ("seal", 2, "protect_cpu_read_only"),
    ] {
        for panic in [false, true] {
            let (scope, result, trace) = prefix_case(|_, trace| {
                let prior = trace
                    .borrow()
                    .calls
                    .iter()
                    .filter(|&&c| c == boundary)
                    .count();
                trace.borrow_mut().native_fault = Some((boundary, prior + ordinal, panic));
            });
            let payload = result.expect_err("native control call must fail");
            if panic {
                assert_eq!(
                    payload.downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", operation))
                );
            } else {
                assert!(
                    matches!(error_source(&*payload), ComputeAqlQueueSessionErrorV1::Memory(
                    MemorySessionError::Injected(actual)) if *actual == operation)
                );
            }
            let memory = memory(&scope.primary);
            let geometry = queue_resource_plan_for_test_v1(4096);
            match (boundary, ordinal) {
                ("ring-map", _) => memory.primary_assert_control_native_fault_v1::<AqlQueueGttV1>(
                    operation, panic, 4096,
                ),
                ("native-control", _) | ("map-mutable", 1) => memory
                    .primary_assert_control_native_fault_v1::<UserptrAqlControlGttV1>(
                        operation, panic, 4096,
                    ),
                ("native-completion", _) | ("map-mutable", 2) => memory
                    .primary_assert_control_native_fault_v1::<HostVisibleCoherentGttV1>(
                        operation,
                        panic,
                        COMPLETION_SIGNAL_ARENA_BYTES_V1,
                    ),
                (_, ordinal) => memory.primary_assert_control_native_fault_v1::<ExecutableGttV1>(
                    operation,
                    panic,
                    usize::try_from(if ordinal == 1 {
                        geometry.end_of_pipe().mapping_bytes()
                    } else {
                        geometry.context_save().mapping_bytes()
                    })
                    .unwrap(),
                ),
            }
            assert!(trace.borrow().calls.contains(&"auxiliary-retake-complete"));
        }
    }
}
