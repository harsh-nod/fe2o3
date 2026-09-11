use super::*;

#[test]
fn local_runtime_registration_rejects_foreign_binding_without_mutation() {
    let gate = LocalGateV1::new();
    let foreign = LocalGateV1::new();
    let pid = std::process::id();
    assert!(matches!(
        gate.register_runtime(pid ^ 1),
        Err(LinuxDoorbellErrorV1::ProcessChanged)
    ));
    assert_eq!(
        gate.runtime_observation(),
        LocalRuntimeObservationV1::Disabled
    );
    let registration = gate.register_runtime(pid).unwrap();
    let before = gate.runtime_observation();
    assert!(matches!(
        registration.validate_binding(&foreign, pid),
        Err(LinuxDoorbellErrorV1::Runtime("local runtime gate binding"))
    ));
    assert!(matches!(
        registration.validate_binding(&gate, pid ^ 1),
        Err(LinuxDoorbellErrorV1::ProcessChanged)
    ));
    assert_eq!(gate.runtime_observation(), before);
    assert_eq!(
        foreign.runtime_observation(),
        LocalRuntimeObservationV1::Disabled
    );
    assert!(!registration.is_queue_live());
    gate.poison();
    registration.validate_binding(&gate, pid).unwrap();
    assert!(matches!(
        gate.register_runtime(pid),
        Err(LinuxDoorbellErrorV1::Runtime(_))
    ));
    drop(registration);
    assert_eq!(
        gate.runtime_observation(),
        LocalRuntimeObservationV1::Poisoned
    );
    assert_eq!(
        foreign.runtime_observation(),
        LocalRuntimeObservationV1::Disabled
    );
}

#[test]
fn local_runtime_registrations_share_gate_with_single_queue_transition() {
    let gate = LocalGateV1::new();
    let pid = std::process::id();
    let mut primary = gate.register_runtime(pid).unwrap();
    let mut arm = gate.arm().unwrap();
    assert!(matches!(
        gate.register_runtime(pid),
        Err(LinuxDoorbellErrorV1::Runtime(_))
    ));
    assert_eq!(
        gate.runtime_observation(),
        LocalRuntimeObservationV1::Enabled {
            opener_pid: pid,
            leases: 1
        }
    );
    assert_eq!(gate.observation(), (true, false));
    primary.mark_queue_created().unwrap();
    assert!(matches!(
        primary.mark_queue_created(),
        Err(LinuxDoorbellErrorV1::Runtime(
            "runtime/queue/event ordering"
        ))
    ));
    assert!(primary.is_queue_live());
    arm.finish(pid).unwrap();
    drop(arm);
    let mut auxiliary = gate.register_runtime(pid).unwrap();
    assert!(!auxiliary.is_queue_live());
    auxiliary.mark_queue_created().unwrap();
    primary.validate_binding(&gate, pid).unwrap();
    auxiliary.validate_binding(&gate, pid).unwrap();
    assert_eq!(
        gate.runtime_observation(),
        LocalRuntimeObservationV1::Enabled {
            opener_pid: pid,
            leases: 2
        }
    );
    drop(primary);
    assert_eq!(
        gate.runtime_observation(),
        LocalRuntimeObservationV1::Poisoned
    );
    auxiliary.validate_binding(&gate, pid).unwrap();
    assert!(auxiliary.is_queue_live());
    drop(auxiliary);
    assert_eq!(gate.observation(), (false, true));
}
