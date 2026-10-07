use super::*;
use crate::kfd_backend::multi_generated::tests::Fixture;

fn issue(
    backend: &mut KfdMultiDeviceRuntimeBackendV1,
    plan: &GeneratedShellPlanV1,
    roster: &GeneratedHostRosterV1,
) -> u64 {
    backend
        .prepare_generated_issue_with_v1(plan, |child, local| {
            Ok(child.install_generated_receipt_metadata_for_test_v1(local, roster))
        })
        .unwrap()
}

fn retire(backend: &mut KfdMultiDeviceRuntimeBackendV1, id: u64) {
    let route = backend.generated_submissions[&id];
    let child = route.shell.scope.child.unwrap();
    backend.children[child].retire_generated_receipt_metadata_for_test_v1(&route.shell.local);
    assert!(!backend.validate_generated_shell_disposal_v1(&route.shell.global));
    backend.release_submission_v1(id).unwrap();
    backend.children[child].clear_generated_receipt_metadata_for_test_v1(&route.shell.local);
    backend.dispose_generated_shells_v1(&route.shell.global);
    backend.destroy_stream_v1(route.shell.scope.stream).unwrap();
}

#[test]
fn generated_multi_issue_colliding_child_ids_keep_exact_routes_through_retired_release() {
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    let mut a = Fixture::new(&mut backend, 7);
    let mut b = Fixture::new(&mut backend, 8);
    let pa = a.install(&mut backend);
    let pb = b.install(&mut backend);
    let ia = issue(&mut backend, &pa, &a.roster);
    let ib = issue(&mut backend, &pb, &b.roster);
    assert_ne!(ia, ib);
    assert_eq!(
        backend.generated_submissions[&ia].local,
        backend.generated_submissions[&ib].local
    );
    assert!(
        backend.generated_submission_can_retire_v1(ia)
            && backend.generated_submission_can_retire_v1(ib)
    );
    assert!(backend.release_submission_v1(ia).is_err());
    assert!(
        backend
            .record_event_v1(pa.binding.backend_stream, ia)
            .is_err()
    );
    assert!(backend.cancel_v1(ia).is_err());
    assert!(backend.submissions.is_empty() && backend.events.is_empty());
    assert!(backend.shutdown_native_v1().is_err());
    retire(&mut backend, ia);
    assert!(backend.generated_submission_matches_v1(ib, backend.generated_submissions[&ib]));
    assert!(matches!(
        backend.release_submission_v1(ia),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    retire(&mut backend, ib);
    assert!(backend.generated_submissions.is_empty());
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn generated_multi_issue_rejection_and_global_capacity_precede_child_mutation() {
    for mode in 0..5 {
        let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
        let mut fixture = Fixture::new(&mut backend, 8);
        let plan = fixture.install(&mut backend);
        match mode {
            0 => backend.next_handle = 0,
            1 => backend.next_handle = u64::MAX,
            2 => backend.next_handle = plan.key,
            3 => {
                let shell = backend.generated_shells[&plan.key];
                for id in 100..100 + MAX_RUNTIME_SUBMISSIONS_V1 as u64 {
                    backend.generated_submissions.insert(
                        id,
                        MultiGeneratedSubmissionV1 {
                            global: id,
                            shell,
                            local: None,
                        },
                    );
                }
            }
            _ => backend.compute_xgmi_children[1] = Some(1),
        }
        assert!(
            backend
                .prepare_generated_issue_with_v1(&plan, |_, _| panic!("must reject before child"))
                .is_err()
        );
        assert!(
            backend
                .children
                .iter()
                .all(|child| child.generated_submissions.is_empty())
        );
        assert!(!backend.terminal);
        backend.generated_submissions.clear();
        backend.compute_xgmi_children.fill(None);
        backend.dispose_generated_shells_v1(&plan);
        backend
            .destroy_stream_v1(plan.binding.backend_stream)
            .unwrap();
    }
}

#[test]
fn generated_multi_issue_preflight_rejection_drops_only_empty_route() {
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    let mut fixture = Fixture::new(&mut backend, 8);
    let plan = fixture.install(&mut backend);
    assert!(matches!(
        backend.prepare_generated_issue_v1(&plan, &fixture.roster),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert!(backend.generated_submissions.is_empty());
    assert!(!backend.terminal);
    backend.dispose_generated_shells_v1(&plan);
    backend
        .destroy_stream_v1(plan.binding.backend_stream)
        .unwrap();
}

#[test]
fn generated_multi_issue_returned_protocol_error_and_unwind_keep_entering_custody() {
    for mode in 0..5 {
        let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
        let mut fixture = Fixture::new(&mut backend, 8);
        let plan = fixture.install(&mut backend);
        let id = backend.next_handle;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            backend.prepare_generated_issue_with_v1(&plan, |child, local| {
                let actual =
                    child.install_generated_receipt_metadata_for_test_v1(local, &fixture.roster);
                match mode {
                    0 => Ok(0),
                    1 => Ok(actual + 1),
                    2 => Err(RuntimeBackendFailureV1::Terminal(
                        KfdRuntimeBackendErrorV1::new(
                            KfdRuntimeBackendErrorKindV1::Native,
                            "injected",
                        ),
                    )),
                    3 => std::panic::panic_any(91u32),
                    _ => Err(KfdRuntimeBackendV1::rejected(
                        KfdRuntimeBackendErrorKindV1::Busy,
                        "rejection after retaining a child receipt",
                    )),
                }
            })
        }));
        if mode == 3 {
            assert_eq!(result.unwrap_err().downcast_ref::<u32>(), Some(&91));
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert!(backend.terminal && backend.children[1].terminal);
        assert!(!backend.children[0].terminal && !backend.children[2].terminal);
        assert!(backend.generated_submissions.contains_key(&id));
        assert_eq!(backend.children[1].generated_submissions.len(), 1);
        assert_eq!(backend.generated_allocations.len(), 3);
        core::mem::forget(backend);
    }
}

#[test]
fn generated_multi_issue_retained_route_corruption_never_releases_original_child() {
    for mode in 0..5 {
        let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
        let mut fixture = Fixture::new(&mut backend, 8);
        let plan = fixture.install(&mut backend);
        let id = issue(&mut backend, &plan, &fixture.roster);
        let route = backend.generated_submissions[&id];
        match mode {
            0 => backend.generated_submissions.get_mut(&id).unwrap().local = Some(u64::MAX),
            1 => backend.generated_submissions.get_mut(&id).unwrap().global += 1,
            2 => {
                backend.generated_submissions.insert(id + 1, route);
            }
            3 => {
                backend.submissions.insert(
                    id + 1,
                    RoutedSubmissionV1::Native {
                        route: RoutedHandleV1 {
                            child: 1,
                            local: route.local.unwrap(),
                        },
                        stream: plan.binding.backend_stream,
                    },
                );
            }
            _ => {
                backend
                    .streams
                    .get_mut(&plan.binding.backend_stream)
                    .unwrap()
                    .child = 0
            }
        }
        assert!(!backend.generated_submission_can_retire_v1(id));
        assert!(matches!(
            backend.release_submission_v1(id),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(backend.terminal && backend.children[1].terminal);
        assert!(!backend.children[0].terminal);
        assert_eq!(backend.children[1].generated_submissions.len(), 1);
        assert!(backend.generated_submissions.contains_key(&id));
        core::mem::forget(backend);
    }
}

#[test]
fn generated_multi_issue_wrong_plan_and_unknown_handle_preserve_sibling_custody() {
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    let mut a = Fixture::new(&mut backend, 7);
    let mut b = Fixture::new(&mut backend, 8);
    let pa = a.install(&mut backend);
    let pb = b.install(&mut backend);
    let ia = issue(&mut backend, &pa, &a.roster);
    let ib = issue(&mut backend, &pb, &b.roster);
    assert!(backend.advance_generated_issue_v1(&pa, ib).is_err());
    assert!(
        backend
            .read_generated_submission_v1(&pb, ia, &b.roster, &mut [])
            .is_err()
    );
    assert!(backend.progress_generated_submission_v1(u64::MAX).is_err());
    assert!(!backend.terminal);
    assert!(
        backend.generated_submission_can_retire_v1(ia)
            && backend.generated_submission_can_retire_v1(ib)
    );
    retire(&mut backend, ia);
    retire(&mut backend, ib);
    backend.shutdown_native_v1().unwrap();
}
