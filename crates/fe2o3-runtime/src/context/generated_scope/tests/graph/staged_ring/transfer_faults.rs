#![cfg(test)]
//! Actual graph/Context copies with explicit mock backend outcomes, not native qualification.
use super::*;
use crate::{RuntimeBackendV1, RuntimeReplicaUsageV1};

#[path = "transfer_faults/backend.rs"]
mod backend;
#[path = "transfer_faults/fixture.rs"]
mod fixture;
use backend::{Fault, FaultBackend};
use fixture::{Fixture, branch};

fn run(fault: Fault) {
    let mut fixture = Fixture::new(fault);
    let request = fixture.request();
    let usage = fixture.metadata.usage();
    let decoded = [const { Cell::new(0) }; 3];
    let dropped = [const { Cell::new(0) }; 3];
    let owners: [_; 3] = core::array::from_fn(|_| std::sync::Arc::new(()));
    let mut preparation_context = context();
    let devices: [_; 3] = core::array::from_fn(|i| preparation_context.devices()[i].id());
    // Synthetic generated carriers are rooted in the existing preparation
    // fixture. Only ordinary copies use real Context/backend execution here.
    let mut prepared: [_; 3] = core::array::from_fn(|i| {
        Some(preparation_context.bound_multi_preparation_for_test_v1(
            devices[i],
            Borrowed {
                ticks: Cell::new(i * 64),
                decoded: &decoded[i],
                dropped: &dropped[i],
                domain: owners[i].clone(),
                completion_order: None,
            },
        ))
    });
    let (slots, copies) = allocate_rosters(18).unwrap();
    let mut scope = RuntimeGfx942GeneratedScopeV1 {
        epoch: fixture.context.scope_epoch.begin().unwrap(),
        context: &mut fixture.context,
        slots,
        copies,
        graph: None,
        capacity: 18,
        deadline: Instant::now() + Duration::from_secs(30),
        identity: Rc::new(()),
        invariant: PhantomData,
        hooks: hooks(),
    };
    let ticket = scope
        .admit_graph_with_v1::<()>(request, |_, node, _| {
            let index = (0..3).find(|&i| branch(i)[0] == node).unwrap();
            Ok(prepared[index].take().unwrap())
        })
        .unwrap();
    assert!(prepared.iter().all(Option::is_none));
    let mut completion = scope.graph_completion_future_v1(&ticket).unwrap();
    let mut staged = [false; 3];
    let mut scratch = [[0; 16]; 3];
    let mut refused_while_siblings_live = false;
    for _ in 0..512 {
        let result = scope.progress_v1();
        if fault != Fault::Rejected && result.is_err() {
            assert!(matches!(result, Err(RuntimeGfx942ScopeErrorV1::Context(_))));
            assert!(scope.context.is_terminal());
            assert_eq!(scope.context.backend.peer_calls, 1);
            assert_eq!(scope.context.backend.local_calls, 0);
            assert_eq!(scope.context.backend.releases, 0);
            assert_eq!(scope.context.backend.rejected, 0);
            let original = scope.context.backend.ambiguous.unwrap();
            assert!(matches!(
                scope.context.backend.inner.poll_v1(original),
                Ok(BackendPollV1::Pending)
            ));
            assert_eq!(
                scope.context.replica_registry_usage_v1().unwrap(),
                RuntimeReplicaUsageV1 {
                    capacity: 3,
                    pending: 1,
                    settled: 0
                }
            );
            assert_eq!(fixture.metadata.usage(), usage);
            assert!(scope.context.graph_reservation.is_some());
            assert!(scope.context.has_unpublished_holds_v1());
            assert_eq!(decoded.each_ref().map(Cell::get), [1, 0, 0]);
            assert_eq!(dropped.each_ref().map(Cell::get), [1, 0, 0]);
            assert!(scope.pending_v1() > 0);
            assert!(scope.graph_report_v1(&ticket).unwrap().is_none());
            for i in 0..3 {
                assert!(
                    scope
                        .graph_generated_ticket_v1(&ticket, branch(i)[0])
                        .unwrap()
                        .is_none()
                );
            }
            assert!(matches!(poll(&mut completion), Poll::Ready(Err(_))));
            assert!(matches!(
                scope.progress_v1(),
                Err(RuntimeGfx942ScopeErrorV1::Unknown)
            ));
            assert_eq!(scope.context.backend.peer_calls, 1);
            assert_eq!(scope.context.backend.releases, 0);
            assert_eq!(fixture.metadata.usage(), usage);
            eprintln!("STAGED_RING_UNKNOWN_COPY_ORIGINALS_RETAINED");
            drop(scope);
            panic!("unknown ring returned after releasing owners");
        }
        result.unwrap();
        for index in 0..3 {
            let ids = branch(index);
            if !staged[index]
                && scope.graph_node_state_v1(&ticket, ids[1]).unwrap()
                    == CompletionNodeStateV1::Ready
            {
                staged[index] = scope
                    .try_stage_graph_host_write_v1(
                        &ticket,
                        ids[1],
                        ids[0],
                        fixture.sources[index],
                        &mut scratch[index],
                        |domain, bytes| {
                            assert!(domain.matches_owner(&owners[index]));
                            assert_eq!((decoded[index].get(), dropped[index].get()), (1, 1));
                            bytes.fill(0x60 + index as u8);
                            Ok::<_, ()>(Some(()))
                        },
                    )
                    .unwrap()
                    .is_some();
            }
        }
        if fault == Fault::Rejected
            && scope.context.backend.rejected == 1
            && !refused_while_siblings_live
        {
            assert!(!scope.context.is_terminal());
            assert!(scope.pending_v1() > 0);
            assert_eq!(decoded.each_ref().map(Cell::get), [1, 0, 0]);
            assert_eq!(dropped.each_ref().map(Cell::get), [1, 0, 0]);
            assert!(scope.context.has_unpublished_holds_v1());
            assert_eq!(
                scope.context.replica_registry_usage_v1().unwrap().pending,
                0
            );
            assert!(
                matches!(scope.graph_node_state_v1(&ticket, branch(0)[4]).unwrap(),
                CompletionNodeStateV1::Failed { origin, .. } if origin == branch(0)[4])
            );
            assert!(
                matches!(scope.graph_node_state_v1(&ticket, branch(0)[5]).unwrap(),
                CompletionNodeStateV1::DependencyFailed { origin, .. } if origin == branch(0)[4])
            );
            refused_while_siblings_live = true;
        }
        if scope.pending_v1() == 0 {
            break;
        }
    }
    assert_eq!(fault, Fault::Rejected);
    assert!(refused_while_siblings_live);
    assert_eq!(scope.pending_v1(), 0);
    // This notification certifies retirement, not successful copy execution.
    // The exact rejected transfer must remain visible in the original report.
    assert!(matches!(poll(&mut completion), Poll::Ready(Ok(()))));
    let report = scope.graph_report_v1(&ticket).unwrap().unwrap();
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].0, branch(0)[4]);
    assert!(matches!(
        &report.errors[0].1,
        RuntimeErrorV1::BackendRejected(_)
    ));
    assert_eq!(report.versions.len(), 18);
    assert_eq!(report.version_inputs.len(), 6);
    for index in 0..3 {
        let ids = branch(index);
        for (offset, &node) in ids.iter().enumerate() {
            let state = scope.graph_node_state_v1(&ticket, node).unwrap();
            if index == 0 && offset >= 4 {
                assert!(matches!(state, CompletionNodeStateV1::Failed { origin, .. }
                    | CompletionNodeStateV1::DependencyFailed { origin, .. } if origin == ids[4]));
            } else {
                assert_eq!(state, CompletionNodeStateV1::Succeeded);
            }
        }
        for producer in [ids[4], ids[5]] {
            let versions: Vec<_> = report
                .versions
                .iter()
                .filter(|entry| entry.version.producer() == Some(producer))
                .collect();
            assert_eq!(versions.len(), 1);
            assert_eq!(versions[0].current_at_terminal, index != 0);
        }
        let inputs: Vec<_> = report
            .version_inputs
            .iter()
            .filter(|input| input.consumer == ids[5])
            .collect();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].version.producer(), Some(ids[4]));
        assert_eq!(inputs[0].available_at_issue, index != 0);
        let original = scope
            .graph_generated_ticket_v1(&ticket, ids[0])
            .unwrap()
            .unwrap();
        assert!(matches!(
            scope.completion_v1(&original).unwrap(),
            Some(Ok(()))
        ));
    }
    assert_eq!(staged, [true; 3]);
    assert_eq!(decoded.each_ref().map(Cell::get), [1; 3]);
    assert_eq!(dropped.each_ref().map(Cell::get), [1; 3]);
    assert_eq!(scope.context.backend.peer_calls, 3);
    assert_eq!(scope.context.backend.local_calls, 2);
    assert_eq!(scope.context.backend.releases, 4);
    assert!(scope.context.backend.ambiguous.is_none());
    assert!(scope.context.submissions.is_empty());
    assert!(scope.context.graph_reservation.is_none());
    assert!(!scope.context.has_unpublished_holds_v1());
    assert_eq!(
        scope.context.replica_registry_usage_v1().unwrap(),
        RuntimeReplicaUsageV1 {
            capacity: 3,
            pending: 0,
            settled: 2
        }
    );
    drop(scope);
    assert!(!fixture.context.scope_epoch.active());
    assert_eq!(fixture.metadata.usage(), usage);
    for index in 0..3 {
        let reference = fixture
            .context
            .find_current_replica_v1(fixture.sources[index], fixture.destinations[index])
            .unwrap();
        if index == 0 {
            assert!(reference.is_none());
        } else {
            fixture
                .context
                .validate_replica_v1(reference.unwrap())
                .unwrap();
        }
        for (allocation, initial) in [
            (fixture.destinations[index], 0x3c),
            (fixture.consumers[index], 0x7e),
        ] {
            let mut actual = [0; 16];
            fixture
                .context
                .read_allocation(allocation, 0, &mut actual)
                .unwrap();
            assert_eq!(
                actual,
                [if index == 0 {
                    initial
                } else {
                    0x60 + index as u8
                }; 16]
            );
        }
    }
    assert!(fixture.context.cleanup().is_complete());
    assert!(preparation_context.cleanup().is_complete());
    drop(fixture.context);
    assert_eq!(fixture.metadata.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn rejected_ring_transfer_cancels_only_its_consumer_and_both_other_branches_settle() {
    run(Fault::Rejected);
}

#[test]
fn unknown_ring_transfer_retains_pending_replica_and_all_unsettled_branches() {
    const ENV: &str = "FE2O3_STAGED_RING_COPY_UNKNOWN";
    const TEST: &str = "context::generated_scope::tests::graph::staged_ring::transfer_faults::unknown_ring_transfer_retains_pending_replica_and_all_unsettled_branches";
    if let Some(mode) = std::env::var_os(ENV) {
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
            .unwrap();
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        run(if mode == "terminal" {
            Fault::Terminal
        } else {
            assert_eq!(mode, "quiescent");
            Fault::Quiescent
        });
        panic!("unknown ring returned after releasing owners");
    }
    for mode in ["quiescent", "terminal"] {
        crate::context::generated_scope::tests::unpublished::abort_child(
            TEST,
            ENV,
            mode,
            "STAGED_RING_UNKNOWN_COPY_ORIGINALS_RETAINED",
        );
    }
}
