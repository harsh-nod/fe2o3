use super::*;
use crate::shared_memory::{
    ExecutableAqlQueueProbeGttV1, PrimaryProjectionCaseV1, UserptrAqlQueueProbeGttV1,
};

#[test]
fn primary_control_projection_failures_retain_actual_input_or_output_without_model_commit() {
    for backing in [
        QueueRingBackingV1::AqlSpecial,
        QueueRingBackingV1::ExecutableProbe,
        QueueRingBackingV1::UserptrProbe,
    ] {
        for (boundary, occurrence, allocation) in [
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
            if !matches!(backing, QueueRingBackingV1::AqlSpecial)
                && !matches!(boundary, "allocate-ring" | "ring-map")
            {
                continue;
            }
            for case in PrimaryProjectionCaseV1::cases(allocation) {
                let (root, trace) = setup();
                let address = &*root as *const Root as usize;
                trace.borrow_mut().projection_fault = Some((boundary, occurrence, case));
                let (root, result) = run(root, backing, false);
                let payload = result.expect_err("selected real projection must reject");
                if case.panic {
                    case.assert_panic(&*payload);
                } else {
                    let error = payload
                        .downcast_ref::<ComputeAqlQueueSessionErrorV1>()
                        .unwrap();
                    let error =
                        if let ComputeAqlQueueSessionErrorV1::TerminalCreation { source, .. } =
                            error
                        {
                            &**source
                        } else {
                            error
                        };
                    assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Injected(
                            "session projection"
                        ))
                    ));
                }
                assert_root(&root, address, &trace);
                let memory = memory(&root);
                match (boundary, occurrence) {
                    ("allocate-ring" | "ring-map", _) => match backing {
                        QueueRingBackingV1::AqlSpecial => {
                            memory.primary_assert_projection_v1::<AqlQueueGttV1>()
                        }
                        QueueRingBackingV1::ExecutableProbe => {
                            memory.primary_assert_projection_v1::<ExecutableAqlQueueProbeGttV1>()
                        }
                        QueueRingBackingV1::UserptrProbe => {
                            memory.primary_assert_projection_v1::<UserptrAqlQueueProbeGttV1>()
                        }
                    },
                    ("native-control", _) | ("map-mutable", 1) => {
                        memory.primary_assert_projection_v1::<UserptrAqlControlGttV1>()
                    }
                    ("native-completion", _) | ("map-mutable", 2) => {
                        memory.primary_assert_projection_v1::<HostVisibleCoherentGttV1>()
                    }
                    _ => memory.primary_assert_projection_v1::<ExecutableGttV1>(),
                }
                assert!(!trace.borrow().calls.contains(&"foundation"));
                assert!(!trace.borrow().calls.contains(&"create"));
                assert_eq!(
                    trace.borrow().poison,
                    boundary != "allocate-ring"
                        || matches!(backing, QueueRingBackingV1::UserptrProbe)
                );
            }
        }
    }
}
