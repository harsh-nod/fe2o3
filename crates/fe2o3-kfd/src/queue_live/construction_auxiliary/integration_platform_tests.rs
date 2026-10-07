use super::*;
use crate::queue_linux::primary_fixture::LocalRuntimeObservationV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    Success,
    Admission,
    Arm,
    Event,
    Install,
    Initialize(bool),
    Restore(bool),
    Cleanup(bool),
    Doorbell(bool),
    Finish(bool),
    FinishPoison,
    CrossEvent,
}

impl Case {
    fn fault(self) -> Option<(&'static str, bool)> {
        match self {
            Self::Event => Some(("event", false)),
            Self::Install => Some(("shadow-install", false)),
            Self::Initialize(panic) => Some(("shadow-init", panic)),
            Self::Restore(panic) => Some(("shadow-restore", panic)),
            Self::Cleanup(_) => Some(("shadow-restore", true)),
            Self::Doorbell(panic) => Some(("doorbell", panic)),
            Self::Finish(panic) => Some(("gate-finish", panic)),
            _ => None,
        }
    }

    fn published(self) -> bool {
        matches!(
            self,
            Self::Success | Self::Doorbell(_) | Self::Finish(_) | Self::FinishPoison
        )
    }
}

#[test]
fn same_engine_auxiliary_local_platform_matrix_retains_exact_primary_and_auxiliary_owners() {
    let cases = [
        Case::Success,
        Case::Admission,
        Case::Arm,
        Case::Event,
        Case::Install,
        Case::Initialize(false),
        Case::Initialize(true),
        Case::Restore(false),
        Case::Restore(true),
        Case::Cleanup(false),
        Case::Cleanup(true),
        Case::Doorbell(false),
        Case::Doorbell(true),
        Case::Finish(false),
        Case::Finish(true),
        Case::FinishPoison,
        Case::CrossEvent,
    ];
    for external_runtime in [false, true] {
        for case in cases {
            let gate = LocalGateV1::new();
            let mut resources = None;
            let (scope, result, trace) = prefix_case_with_setup(
                external_runtime,
                |trace| {
                    trace.borrow_mut().local_gate = Some(gate.clone());
                    resources = Some(trace.borrow().local_resources.clone());
                },
                |scope, trace| {
                    let primary = scope.primary.completed.as_ref().unwrap();
                    primary.runtime.assert_local_runtime(&gate, true);
                    primary.shadows.assert_local_published();
                    assert_eq!(trace.borrow().local_resources.live(), (1, 1, 1));
                    assert_eq!(gate.observation(), (false, false));
                    assert_eq!(
                        gate.runtime_observation(),
                        LocalRuntimeObservationV1::Enabled {
                            opener_pid: std::process::id(),
                            leases: 1
                        }
                    );
                    let mut t = trace.borrow_mut();
                    if let Some((name, panic)) = case.fault() {
                        let next = t.calls.iter().filter(|&&c| c == name).count() + 1;
                        t.fault = Some((name, next, panic));
                    }
                    match case {
                        Case::Admission => gate.poison(),
                        Case::Arm => t.local_arm_poison = true,
                        Case::FinishPoison => t.local_finish_poison = true,
                        Case::CrossEvent => {
                            t.local_event_substitute = Some(primary.event.local_event_clone())
                        }
                        Case::Cleanup(after) => t.cleanup_panic = Some(after),
                        _ => {}
                    }
                },
            );
            assert_eq!(
                result.is_ok(),
                case == Case::Success,
                "{case:?}, external={external_runtime}"
            );
            if let Err(payload) = &result {
                if let Some((name, true)) = case.fault() {
                    let nth = trace.borrow().calls.iter().filter(|&&c| c == name).count();
                    assert_eq!(payload.downcast_ref::<(&str, usize)>(), Some(&(name, nth)));
                } else if let Some((name, false)) = case.fault() {
                    assert!(
                        matches!(error_source(&**payload), ComputeAqlQueueSessionErrorV1::Contract(actual) if *actual == name)
                    );
                } else {
                    let expected = match case {
                        Case::Admission => {
                            "KFD runtime state invalid: process-global gate poisoned"
                        }
                        Case::Arm => {
                            "KFD runtime state invalid: process-global creation gate unavailable"
                        }
                        Case::FinishPoison => {
                            "KFD runtime state invalid: creation finalization unavailable"
                        }
                        Case::CrossEvent => {
                            "queue exception event invalid: event/shadow substitution"
                        }
                        _ => unreachable!(),
                    };
                    assert!(
                        matches!(error_source(&**payload), ComputeAqlQueueSessionErrorV1::Doorbell(actual) if actual == expected)
                    );
                }
            }
            let primary = scope.primary.completed.as_ref().unwrap();
            primary.runtime.assert_local_runtime(&gate, true);
            primary.shadows.assert_local_published();
            let lane = scope
                .lanes
                .first()
                .and_then(|s| s.state.as_ref())
                .or(scope.construction.completed.as_ref());
            let exception = lane.and_then(|l| l.exception.as_ref());
            let runtime = exception
                .map(|e| &e.runtime)
                .or(scope.construction.runtime.as_ref());
            assert_eq!(runtime.is_some(), case != Case::Admission);
            if let Some(runtime) = runtime {
                runtime.assert_local_runtime(&gate, case.published());
            }
            if let Some(exception) = exception {
                exception.shadows.assert_local_published();
            }
            if let Some(unpublished) = &scope.construction.unpublished {
                unpublished.assert_local_unpublished(case != Case::Cleanup(false));
            }
            let t = trace.borrow();
            assert!(t.local_event_substitute.is_none());
            let armed = !matches!(case, Case::Admission | Case::Arm);
            assert_eq!(scope.construction.creation_arm.is_some(), armed);
            assert_eq!(
                gate.observation(),
                (armed && case != Case::Success, case != Case::Success)
            );
            assert_eq!(
                gate.runtime_observation(),
                if case == Case::Success {
                    LocalRuntimeObservationV1::Enabled {
                        opener_pid: std::process::id(),
                        leases: 2,
                    }
                } else {
                    LocalRuntimeObservationV1::Poisoned
                }
            );
            assert_eq!(
                t.calls.iter().filter(|&&c| c == "create").count(),
                1 + usize::from(case.published())
            );
            assert_eq!(
                t.calls.iter().filter(|&&c| c == "publish").count(),
                1 + usize::from(case.published())
            );
            let expected = match case {
                Case::Admission | Case::Arm | Case::Event => (1, 1, 1),
                Case::Install => (1, 1, 2),
                Case::Initialize(_) | Case::Restore(_) | Case::Cleanup(true) | Case::CrossEvent => {
                    (2, 1, 2)
                }
                _ => (2, 2, 2),
            };
            let resources = resources.unwrap();
            assert_eq!(
                resources.live(),
                expected,
                "{case:?}, external={external_runtime}"
            );
            drop(t);
            drop(scope);
            assert_eq!(
                resources.live(),
                (0, 0, 0),
                "only fixture-owned mappings and files disposed"
            );
            assert_eq!(gate.observation(), (false, true));
        }
    }
}
