use super::*;
mod stop_tests;
use crate::completion::{
    CompletionGraphV1, CompletionNodeIdV1, CompletionNodeStateV1, CompletionNodeV1,
    FutureIdentityV1,
};

type ActiveGraph = Option<Box<dyn graph::EngineGraphV1<MockBackend>>>;

fn id(n: u32) -> CompletionNodeIdV1 {
    CompletionNodeIdV1::new(n).unwrap()
}

fn request<B: RuntimeBackendV1>(
    context: &RuntimeContextV1<B>,
    stream: RuntimeStreamIdV1,
    tickets: Vec<RuntimeAsyncReservedTicketV1>,
) -> RuntimeGeneratedGraphRequestV1<B> {
    let identity = context.completion_stream_identity_v1(stream).unwrap();
    let nodes = (1..=tickets.len() as u32)
        .map(|n| {
            CompletionNodeV1::future(
                id(n),
                FutureIdentityV1::new(identity, [n as u8; 32]),
                (n > 1).then(|| id(n - 1)),
            )
        })
        .collect();
    let graph = CompletionGraphV1::new(identity.context(), vec![identity], nodes).unwrap();
    let mut request = RuntimeGeneratedGraphRequestV1::new(
        RuntimeGraphRequestV1::new(graph, vec![(identity, stream)]).unwrap(),
    );
    for (index, ticket) in tickets.into_iter().enumerate() {
        request.bind_reserved(id(index as u32 + 1), ticket).unwrap();
    }
    request
}

fn command(h: &mut Harness, graph: &mut ActiveGraph) {
    let command = h.receiver.try_recv().unwrap();
    assert!(!handle_command_v1(
        &mut h.context,
        &mut BTreeMap::new(),
        &mut h.registry,
        graph,
        None,
        command,
        h.config,
        Some(RuntimeAsyncProgressConfigV1::default())
    ));
}

fn advance_graph(h: &mut Harness, graph: &mut ActiveGraph) {
    if graph
        .as_mut()
        .is_some_and(|graph| graph.advance(&mut h.context, &mut h.registry, 8, 1))
    {
        *graph = None;
    }
}

fn finish(h: &mut Harness, graph: &mut ActiveGraph) {
    for _ in 0..20 {
        if graph.is_none() {
            return;
        }
        h.advance();
        advance_graph(h, graph);
    }
    panic!("generated graph did not finish");
}

#[test]
fn generated_graph_dependency_waits_for_original_decoder_and_returns_exact_receipts() {
    let mut h = Harness::new(2, 4, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let domains = [Arc::new(()), Arc::new(())];
    let (a, stream) = reserve_completion_with_domain(&mut h, drops.clone(), 0, domains[0].clone());
    let (b, _) = reserve_completion_with_domain(&mut h, drops.clone(), 0, domains[1].clone());
    let request = request(&h.context, stream, vec![a, b]);
    let future = h.handle.try_submit_generated_graph_v1(request).unwrap();
    let mut graph = None;
    command(&mut h, &mut graph);
    assert!(graph.is_some());
    advance_graph(&mut h, &mut graph);
    assert_eq!((h.registry.len(), h.registry.active_len()), (2, 1));
    for expected in ["adopt", "issue", "poll"] {
        h.advance();
        assert_eq!(
            h.state.lock().unwrap().adoption_order.last(),
            Some(&expected)
        );
        advance_graph(&mut h, &mut graph);
        assert_eq!((h.registry.len(), h.registry.active_len()), (2, 1));
        assert_eq!(drops.load(Ordering::SeqCst), 0);
    }
    h.advance(); // Settlement, decoder and gate commit, not just physical completion.
    assert_eq!((h.registry.len(), h.registry.active_len()), (1, 0));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(
        h.state
            .lock()
            .unwrap()
            .adoption_order
            .contains(&"gate_commit")
    );
    advance_graph(&mut h, &mut graph);
    assert_eq!((h.registry.len(), h.registry.active_len()), (1, 1));
    finish(&mut h, &mut graph);
    let report = ready(future).unwrap().unwrap();
    assert_eq!(
        report
            .completions
            .iter()
            .map(|(node, _)| *node)
            .collect::<Vec<_>>(),
        [id(1), id(2)]
    );
    assert!(report.errors.is_empty());
    for (index, (_, receipt)) in report.completions.iter().enumerate() {
        assert!(receipt.matches_owner(&domains[index]));
        assert!(!receipt.matches_owner(&domains[1 - index]));
    }
    assert!(
        report
            .graph
            .completion
            .entries()
            .iter()
            .all(|node| node.state() == CompletionNodeStateV1::Succeeded)
    );
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    assert_eq!(h.registry.len(), 0);
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
    h.context.destroy_stream(stream).unwrap();
}

#[test]
fn generated_graph_failed_decoder_discards_dependent_without_receipt_or_issue() {
    for mode in [23, 24] {
        let mut h = Harness::new(2, 4, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let (a, stream) = reserve_completion(&mut h, drops.clone(), mode);
        let (b, _) = reserve_completion(&mut h, drops.clone(), 0);
        let future = h
            .handle
            .try_submit_generated_graph_v1(request(&h.context, stream, vec![a, b]))
            .unwrap();
        let mut graph = None;
        command(&mut h, &mut graph);
        finish(&mut h, &mut graph);
        let report = ready(future).unwrap().unwrap();
        assert!(report.completions.is_empty());
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].0, id(1));
        match (&report.errors[0].1, mode) {
            (
                RuntimeGeneratedGraphNodeErrorV1::Readback(
                    crate::RuntimeGfx942ReadbackErrorV1::InvalidStorage,
                ),
                23,
            )
            | (
                RuntimeGeneratedGraphNodeErrorV1::Engine(
                    RuntimeAsyncEngineCallErrorV1::CommandPanicked,
                ),
                24,
            ) => {}
            other => panic!("unexpected decoder outcome: {other:?}"),
        }
        assert!(matches!(
            report.graph.completion.entries()[1].state(),
            CompletionNodeStateV1::DependencyFailed { .. }
        ));
        assert_eq!(h.state.lock().unwrap().adoption_completed.len(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 2);
        assert_eq!(h.registry.len(), 0);
        h.context.destroy_stream(stream).unwrap();
    }
}

#[test]
fn generated_graph_cancellation_discards_only_unactivated_exact_owners() {
    for activated in [false, true] {
        let mut h = Harness::new(4, 6, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let (a, stream) = reserve_completion(&mut h, drops.clone(), 0);
        let (b, _) = reserve_completion(&mut h, drops.clone(), 0);
        let (c, _) = reserve_completion(&mut h, drops.clone(), 0);
        let unrelated_drops = Arc::new(AtomicUsize::new(0));
        let (unrelated, _) = reserve_completion(&mut h, unrelated_drops.clone(), 0);
        let future = h
            .handle
            .try_submit_generated_graph_v1(request(&h.context, stream, vec![a, b, c]))
            .unwrap();
        let mut graph = None;
        command(&mut h, &mut graph);
        if activated {
            advance_graph(&mut h, &mut graph);
            h.advance();
        }
        future.control().cancel_unissued();
        advance_graph(&mut h, &mut graph);
        assert_eq!(drops.load(Ordering::SeqCst), if activated { 2 } else { 3 });
        finish(&mut h, &mut graph);
        let report = ready(future).unwrap().unwrap();
        assert_eq!(report.completions.len(), usize::from(activated));
        assert_eq!(
            h.state.lock().unwrap().adoption_completed.len(),
            usize::from(activated)
        );
        assert_eq!(h.registry.len(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 3);
        assert_eq!(unrelated_drops.load(Ordering::SeqCst), 0);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 1);
        assert!(h.registry.discard_reserved(&unrelated.key));
        drop(unrelated);
        assert_eq!(unrelated_drops.load(Ordering::SeqCst), 1);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
    }
}

#[test]
fn generated_graph_adoption_is_not_disposed_by_drain_cleanup() {
    let mut h = Harness::new(2, 4, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let (a, stream) = reserve_completion(&mut h, drops.clone(), 0);
    let (b, _) = reserve_completion(&mut h, drops.clone(), 0);
    let future = h
        .handle
        .try_submit_generated_graph_v1(request(&h.context, stream, vec![a, b]))
        .unwrap();
    let mut graph = None;
    command(&mut h, &mut graph);
    advance_graph(&mut h, &mut graph);
    drop(future);
    for _ in 0..2 {
        h.registry.retire_unpublished_v1(&mut h.context, 2);
        assert_eq!(h.registry.len(), 2);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        h.advance();
    }
    for _ in 0..20 {
        advance_graph(&mut h, &mut graph);
        h.advance();
        h.registry.retire_unpublished_v1(&mut h.context, 2);
        if graph.is_none() {
            break;
        }
    }
    assert!(graph.is_none());
    assert_eq!(h.registry.len(), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    assert_eq!(h.state.lock().unwrap().adoption_completed.len(), 2);
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
}

#[test]
fn generated_graph_rejection_returns_exact_ticket_and_preserves_parked_credit() {
    for mode in [0, 1, 2] {
        let mut h = Harness::new(2, 3, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let (ticket, stream) =
            reserve_completion(&mut h, drops.clone(), if mode == 1 { 1 } else { 0 });
        let key = ticket.key.clone();
        let request = request(&h.context, stream, vec![ticket]);
        let token = (mode == 2).then(|| h.context.reserve_graph_v1(1).unwrap());
        let future = h.handle.try_submit_generated_graph_v1(request).unwrap();
        let mut graph = None;
        if mode == 0 {
            // A real active preparation makes the owner-thread admission Busy.
            drop(
                prepare(
                    &h.handle,
                    Arc::new(AtomicUsize::new(0)),
                    drops.clone(),
                    false,
                )
                .unwrap(),
            );
            let incoming = h.receiver.try_recv().unwrap();
            h.command();
            assert!(!handle_command_v1(
                &mut h.context,
                &mut BTreeMap::new(),
                &mut h.registry,
                &mut graph,
                None,
                incoming,
                h.config,
                Some(RuntimeAsyncProgressConfigV1::default())
            ));
        } else {
            command(&mut h, &mut graph);
        }
        assert!(graph.is_none());
        let Err(RuntimeGeneratedGraphFailureV1::Rejected(mut failure)) = ready(future).unwrap()
        else {
            panic!("expected admission rejection")
        };
        let ticket = failure.request.take_reserved(id(1)).unwrap();
        assert!(Arc::ptr_eq(&key, &ticket.key));
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(
            h.handle.observer.reply_cells_in_use(),
            if mode == 0 { 2 } else { 1 }
        );
        assert!(h.registry.discard_reserved(&ticket.key));
        drop(ticket);
        if let Some(token) = token {
            h.context.close_graph_issue_v1(token).unwrap();
            h.context.release_graph_v1(token).unwrap();
        }
        if mode == 0 {
            h.advance();
            h.registry.dispose_quiescent();
        }
    }
}

#[test]
fn generated_graph_hold_requires_exact_open_token_and_blocks_release() {
    let mut h = Harness::new(1, 2, true);
    let stream = h
        .context
        .create_stream(h.context.devices()[0].id())
        .unwrap();
    let mut foreign = Harness::new(1, 2, true);
    let wrong = foreign.context.reserve_graph_v1(1).unwrap();
    let token = h.context.reserve_graph_v1(1).unwrap();
    assert!(h.context.hold_unpublished_stream_v1(stream).is_err());
    assert!(
        h.context
            .hold_unpublished_stream_with_access_v1(stream, Some(wrong))
            .is_err()
    );
    assert_eq!(
        h.context.unpublished_identity_for_test_v1(stream),
        Some(None)
    );
    let hold = h
        .context
        .hold_unpublished_stream_with_access_v1(stream, Some(token))
        .unwrap();
    h.context.close_graph_issue_v1(token).unwrap();
    assert_eq!(
        h.context.release_graph_v1(token),
        Err(RuntimeValidationErrorV1::SubmissionPending)
    );
    assert!(
        h.context
            .hold_unpublished_stream_with_access_v1(stream, Some(token))
            .is_err()
    );
    h.context.release_unpublished_hold_v1(&hold).unwrap();
    h.context.release_graph_v1(token).unwrap();
    assert!(h.context.release_unpublished_hold_v1(&hold).is_err());
    let ordinary = h.context.hold_unpublished_stream_v1(stream).unwrap();
    h.context.release_unpublished_hold_v1(&ordinary).unwrap();
    foreign.context.close_graph_issue_v1(wrong).unwrap();
    foreign.context.release_graph_v1(wrong).unwrap();
}

#[test]
fn generated_graph_immediate_and_queued_rejections_preserve_exact_tickets() {
    for mode in 0..7 {
        let mut h = Harness::new(2, 5, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let (ticket, stream) = reserve_completion(&mut h, drops.clone(), 0);
        let key = ticket.key.clone();
        let mut occupied = None;
        let mut credits = Vec::new();
        if mode == 6 {
            let (other, other_stream) = reserve_completion(&mut h, drops.clone(), 0);
            occupied = Some(
                h.handle
                    .try_submit_generated_graph_v1(request(&h.context, other_stream, vec![other]))
                    .unwrap(),
            );
        }
        let request = request(&h.context, stream, vec![ticket]);
        let mut failure = if mode == 0 {
            let future = h.handle.try_submit_generated_graph_v1(request).unwrap();
            drop(h.receiver.try_recv().unwrap());
            let Err(RuntimeGeneratedGraphFailureV1::Rejected(failure)) = ready(future).unwrap()
            else {
                panic!("queued rejection")
            };
            failure
        } else {
            match mode {
                1 => h.handle.observer.admission.close(),
                2 => h
                    .handle
                    .observer
                    .worker_thread
                    .set(thread::current().id())
                    .unwrap(),
                3 => {
                    for _ in 0..4 {
                        h.handle
                            .observer
                            .sender
                            .try_send(RuntimeAsyncEngineCommandV1::Stop)
                            .unwrap_or_else(|_| panic!("queue"));
                    }
                }
                4 => {
                    let (_, receiver) = sync_channel(1);
                    drop(core::mem::replace(&mut h.receiver, receiver));
                }
                5 => {
                    for _ in 0..4 {
                        credits.push(
                            owned::Reply::<()>::budgeted_pair(&h.handle.observer.reply_budget)
                                .unwrap(),
                        );
                    }
                }
                _ => {}
            }
            h.handle
                .try_submit_generated_graph_v1(request)
                .err()
                .expect("immediate rejection")
        };
        let expected = match mode {
            2 => RuntimeAsyncEngineCallErrorV1::ReentrantCall,
            3 => RuntimeAsyncEngineCallErrorV1::CommandQueueFull,
            5 => RuntimeAsyncEngineCallErrorV1::ReplyCapacity,
            6 => RuntimeAsyncEngineCallErrorV1::GraphCapacity,
            _ => RuntimeAsyncEngineCallErrorV1::EngineStopped,
        };
        assert!(
            matches!(failure.error, RuntimeGeneratedGraphAdmissionErrorV1::Engine(error) if error == expected)
        );
        let ticket = failure.request.take_reserved(id(1)).unwrap();
        assert!(Arc::ptr_eq(&key, &ticket.key));
        assert_eq!(
            (h.registry.len(), h.registry.active_len()),
            (if mode == 6 { 2 } else { 1 }, 0)
        );
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert!(h.state.lock().unwrap().adoption_order.is_empty());
        assert!(h.registry.discard_reserved(&ticket.key));
        drop(ticket);
        drop(credits);
        if let Some(future) = occupied {
            drop(h.receiver.try_recv().unwrap());
            let Err(RuntimeGeneratedGraphFailureV1::Rejected(mut failure)) = ready(future).unwrap()
            else {
                panic!("queued occupying graph")
            };
            let ticket = failure.request.take_reserved(id(1)).unwrap();
            assert!(h.registry.discard_reserved(&ticket.key));
            drop(ticket);
        }
        assert!(!h.handle.observer.graph_slot.load(Ordering::Acquire));
        assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
        assert_eq!(h.registry.len(), 0);
    }
}

#[test]
fn generated_graph_owned_drain_completes_accepted_prefix_or_retains_on_exhaustion() {
    for (exhausted, drop_observer) in [(false, false), (false, true), (true, false)] {
        let state = Arc::new(Mutex::new(MockState::default()));
        let trace = Arc::new(Mutex::new(OwnerTrace::default()));
        let drops = Arc::new(AtomicUsize::new(0));
        let config = RuntimeAsyncEngineConfigV1::new(8, 4, 8, 1, Duration::from_millis(1)).unwrap();
        let (engine, handle) = start_with_config(state.clone(), trace, config);
        let stream = handle
            .observer
            .try_with_context(|context| context.create_stream(context.devices()[0].id()).unwrap())
            .unwrap();
        let mut tickets = Vec::new();
        for _ in 0..2 {
            let ticket = join(preparation_with_hooks(
                &handle,
                state.clone(),
                drops.clone(),
                0,
                Some(completion_hooks()),
            ))
            .unwrap()
            .unwrap();
            tickets.push(
                join(handle.try_reserve_prepared_v1(ticket).unwrap())
                    .unwrap()
                    .unwrap(),
            );
        }
        let request = handle
            .observer
            .try_with_context(move |context| request(context, stream, tickets))
            .unwrap();
        let entered = Arc::new(Barrier::new(2));
        let released = Arc::new(Barrier::new(2));
        let (a, b) = (entered.clone(), released.clone());
        drop(
            handle
                .observer
                .enqueue_with_context(move |_| {
                    a.wait();
                    b.wait();
                })
                .unwrap(),
        );
        entered.wait();
        let future = handle.try_submit_generated_graph_v1(request).unwrap();
        let future = if drop_observer {
            drop(future);
            None
        } else {
            Some(future)
        };
        let drain = handle.begin_drain(if exhausted { 1 } else { 64 }).unwrap();
        assert!(matches!(
            handle.observer.enqueue_with_context(|_| ()),
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        ));
        released.wait();
        let report = join(drain).unwrap();
        let shutdown = engine.shutdown().unwrap();
        if exhausted {
            assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::BudgetExhausted);
            assert!(report.graph_active);
            assert_eq!(report.operations_remaining, 1);
            assert_eq!(
                shutdown.disposition,
                RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
            );
            assert_eq!(drops.load(Ordering::SeqCst), 0);
            assert!(matches!(
                join(future.unwrap()),
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            ));
        } else {
            assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
            assert!(!report.graph_active);
            assert_eq!(report.operations_remaining, 0);
            assert_eq!(
                shutdown.disposition,
                RuntimeAsyncOwnedDispositionV1::Released
            );
            assert!(shutdown.cleanup.unwrap().is_complete());
            assert_eq!(drops.load(Ordering::SeqCst), 2);
            if let Some(future) = future {
                let report = join(future).unwrap().unwrap();
                assert_eq!(report.completions.len(), 2);
                assert!(report.errors.is_empty());
            }
            assert_eq!(handle.observer.reply_cells_in_use(), 0);
        }
    }
}

#[test]
fn generated_graph_terminal_faults_and_stop_retain_active_custody() {
    for mode in [10, 11, 30, 40, 0] {
        let mut h = Harness::new(2, 4, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let (a, stream) = reserve_completion(&mut h, drops.clone(), mode);
        let (b, _) = reserve_completion(&mut h, drops.clone(), 0);
        let future = h
            .handle
            .try_submit_generated_graph_v1(request(&h.context, stream, vec![a, b]))
            .unwrap();
        let mut graph = None;
        command(&mut h, &mut graph);
        advance_graph(&mut h, &mut graph);
        h.advance();
        if mode == 0 {
            graph
                .as_mut()
                .unwrap()
                .stop(&mut h.context, &mut h.registry);
        } else {
            for _ in 0..4 {
                h.advance();
                if h.context.is_terminal() {
                    break;
                }
            }
        }
        assert!(h.context.is_terminal());
        assert_eq!(h.registry.active_len(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), usize::from(mode == 0));
        assert!(
            h.context
                .unpublished_identity_for_test_v1(stream)
                .unwrap()
                .is_some()
        );
        assert!(!h.registry.stop_observations());
        drop(graph);
        assert!(matches!(
            ready(future),
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        ));
        let before = h.state.lock().unwrap().adoption_order.clone();
        h.advance();
        h.registry.retire_unpublished_v1(&mut h.context, 4);
        let after = h.state.lock().unwrap().adoption_order.clone();
        assert_eq!(after, before);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
        core::mem::forget(h.registry); // Mirror process-retained ambiguous native custody.
    }
}

#[test]
fn generated_graph_stop_collects_already_settled_original_completion() {
    for mode in [0, 23, 24] {
        let mut h = Harness::new(2, 4, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let domain = Arc::new(());
        let (first, stream) =
            reserve_completion_with_domain(&mut h, drops.clone(), mode, domain.clone());
        let (suffix, _) = reserve_completion(&mut h, drops.clone(), 0);
        let future = h
            .handle
            .try_submit_generated_graph_v1(request(&h.context, stream, vec![first, suffix]))
            .unwrap();
        let mut active = None;
        command(&mut h, &mut active);
        advance_graph(&mut h, &mut active);
        for _ in 0..4 {
            h.advance();
        }
        assert_eq!(h.registry.active_len(), 0);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        let before = h.state.lock().unwrap().adoption_order.clone();
        active
            .as_mut()
            .unwrap()
            .stop(&mut h.context, &mut h.registry);
        assert!(
            !h.context.is_terminal(),
            "mode {mode}: settled Stop retained Context"
        );
        drop(active);
        let report = ready(future).unwrap().unwrap();
        assert_eq!(report.completions.len(), usize::from(mode == 0));
        assert_eq!(report.errors.len(), usize::from(mode != 0));
        if mode == 0 {
            assert!(report.completions[0].1.matches_owner(&domain));
        }
        assert_eq!(h.registry.len(), 0);
        assert_eq!(drops.load(Ordering::SeqCst), 2);
        let mut expected = before;
        expected.push("payload_drop"); // Only the unactivated suffix is disposed.
        assert_eq!(h.state.lock().unwrap().adoption_order, expected);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
        assert!(!h.handle.observer.graph_slot.load(Ordering::Acquire));
        assert!(h.context.cleanup().is_complete());
    }
}

#[test]
fn generated_graph_post_admission_rejection_preserves_independent_branch() {
    let mut h = Harness::new(3, 5, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let stream = h
        .context
        .create_stream(h.context.devices()[0].id())
        .unwrap();
    let mut hooks = completion_hooks();
    hooks.preflight = |context: &mut RuntimeContextV1<MockBackend>, _, _, _, _| {
        if context.backend().state.lock().unwrap().complete_on_flush {
            Err(RuntimeValidationErrorV1::Unsupported.into())
        } else {
            Ok(())
        }
    };
    let preparation =
        preparation_with_hooks(&h.handle, h.state.clone(), drops.clone(), 0, Some(hooks));
    h.command();
    h.advance();
    let reserve = h
        .handle
        .try_reserve_prepared_v1(ready(preparation).unwrap().unwrap())
        .unwrap();
    h.command();
    let a = ready(reserve).unwrap().unwrap();
    let (dependent, _) = reserve_completion(&mut h, drops.clone(), 0);
    let (independent, other) = reserve_completion(&mut h, drops.clone(), 0);
    let first = h.context.completion_stream_identity_v1(stream).unwrap();
    let second = h.context.completion_stream_identity_v1(other).unwrap();
    let graph = CompletionGraphV1::new(
        first.context(),
        vec![first, second],
        vec![
            CompletionNodeV1::future(id(1), FutureIdentityV1::new(first, [1; 32]), None),
            CompletionNodeV1::future(id(2), FutureIdentityV1::new(first, [2; 32]), Some(id(1))),
            CompletionNodeV1::future(id(3), FutureIdentityV1::new(second, [3; 32]), None),
        ],
    )
    .unwrap();
    let mut request = RuntimeGeneratedGraphRequestV1::new(
        RuntimeGraphRequestV1::new(graph, vec![(first, stream), (second, other)]).unwrap(),
    );
    for (n, ticket) in [(1, a), (2, dependent), (3, independent)] {
        request.bind_reserved(id(n), ticket).unwrap();
    }
    let future = h.handle.try_submit_generated_graph_v1(request).unwrap();
    let mut active = None;
    command(&mut h, &mut active);
    h.state.lock().unwrap().complete_on_flush = true;
    finish(&mut h, &mut active);
    let report = ready(future).unwrap().unwrap();
    assert_eq!(report.completions.len(), 1);
    assert_eq!(report.completions[0].0, id(3));
    assert!(
        matches!(report.errors.as_slice(), [(node, RuntimeGeneratedGraphNodeErrorV1::Activation(_))] if *node == id(1))
    );
    assert!(matches!(
        report.graph.completion.entries()[1].state(),
        CompletionNodeStateV1::DependencyFailed { .. }
    ));
    assert_eq!(h.state.lock().unwrap().adoption_completed.len(), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 3);
    assert_eq!(h.registry.len(), 0);
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
}

#[test]
fn generated_graph_fresh_occurrences_have_distinct_generations_and_no_residue() {
    let mut h = Harness::new(1, 3, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let mut generations = Vec::new();
    for occurrence in 1..=3 {
        let (ticket, stream) = reserve_completion(&mut h, drops.clone(), 0);
        let future = h
            .handle
            .try_submit_generated_graph_v1(request(&h.context, stream, vec![ticket]))
            .unwrap();
        let mut graph = None;
        command(&mut h, &mut graph);
        finish(&mut h, &mut graph);
        let report = ready(future).unwrap().unwrap();
        generations.push(report.graph.execution.generation());
        assert_eq!(report.completions.len(), 1);
        assert_eq!(
            h.context.unpublished_identity_for_test_v1(stream),
            Some(None)
        );
        h.context.destroy_stream(stream).unwrap();
        assert_eq!(h.registry.len(), 0);
        assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
        assert_eq!(drops.load(Ordering::SeqCst), occurrence);
    }
    assert!(generations.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn generated_graph_binding_rejections_preserve_both_original_tickets() {
    let mut h = Harness::new(2, 4, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let (first, stream) = reserve_completion(&mut h, drops.clone(), 0);
    let (second, _) = reserve_completion(&mut h, drops.clone(), 0);
    let keys = [first.key.clone(), second.key.clone()];
    let mut request = request(&h.context, stream, vec![first]);
    let failure = request.bind_reserved(id(99), second).unwrap_err();
    assert_eq!(failure.error, RuntimeGraphValidationErrorV1::UnknownNode);
    assert!(Arc::ptr_eq(&keys[1], &failure.ticket.key));
    let failure = request.bind_reserved(id(1), failure.ticket).unwrap_err();
    assert_eq!(
        failure.error,
        RuntimeGraphValidationErrorV1::DuplicateOperation
    );
    assert!(Arc::ptr_eq(&keys[1], &failure.ticket.key));
    let first = request.take_reserved(id(1)).unwrap();
    assert!(Arc::ptr_eq(&keys[0], &first.key));
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(h.handle.observer.reply_cells_in_use(), 2);
    for ticket in [first, failure.ticket] {
        assert!(h.registry.discard_reserved(&ticket.key));
    }
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    assert_eq!(h.handle.observer.reply_cells_in_use(), 0);
}

#[test]
fn generated_graph_mixes_ordinary_launch_and_copy_in_the_existing_executor() {
    let mut h = Harness::new(1, 3, true);
    let drops = Arc::new(AtomicUsize::new(0));
    let (ticket, stream) = reserve_completion(&mut h, drops.clone(), 0);
    let device = h.context.devices()[0].id();
    let module = h.context.load_module(device, &[1]).unwrap();
    let kernel = Arc::new(
        h.context
            .resolve_kernel::<EmptyArgs>(module, "mixed")
            .unwrap(),
    );
    let source = h
        .context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 8)
        .unwrap();
    let destination = h
        .context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 8)
        .unwrap();
    let identity = h.context.completion_stream_identity_v1(stream).unwrap();
    let nodes = (1..=3)
        .map(|n| {
            CompletionNodeV1::future(
                id(n),
                FutureIdentityV1::new(identity, [n as u8; 32]),
                (n > 1).then(|| id(n - 1)),
            )
        })
        .collect();
    let graph = CompletionGraphV1::new(identity.context(), vec![identity], nodes).unwrap();
    let mut request = RuntimeGraphRequestV1::new(graph, vec![(identity, stream)]).unwrap();
    request
        .bind_launch(id(1), kernel, &EmptyArgs, geometry())
        .unwrap();
    request
        .bind_copy(
            id(3),
            RuntimeMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 64,
            },
            RuntimeMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 64,
            },
        )
        .unwrap();
    let mut request = RuntimeGeneratedGraphRequestV1::new(request);
    request.bind_reserved(id(2), ticket).unwrap();
    let future = h.handle.try_submit_generated_graph_v1(request).unwrap();
    let mut active = None;
    command(&mut h, &mut active);
    for _ in 0..30 {
        for status in h.state.lock().unwrap().statuses.values_mut() {
            *status = BackendPollV1::Succeeded;
        }
        h.advance();
        advance_graph(&mut h, &mut active);
        if active.is_none() {
            break;
        }
    }
    assert!(active.is_none());
    let report = ready(future).unwrap().unwrap();
    assert_eq!(report.completions.len(), 1);
    assert_eq!(report.completions[0].0, id(2));
    assert_eq!(
        report.graph.observations,
        [
            (id(1), RuntimeCompletionStatusV1::Succeeded),
            (id(3), RuntimeCompletionStatusV1::Succeeded)
        ]
    );
    assert!(
        report
            .graph
            .completion
            .entries()
            .iter()
            .all(|entry| entry.state() == CompletionNodeStateV1::Succeeded)
    );
    assert!(report.errors.is_empty());
    assert!(report.graph.errors.is_empty());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(h.registry.len(), 0);
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn generated_graph_foreign_or_domainless_ticket_rejection_is_precommit() {
    for foreign in [false, true] {
        let mut h = Harness::new(2, 4, true);
        let mut other = Harness::new(1, 3, true);
        let drops = Arc::new(AtomicUsize::new(0));
        let (valid, stream) = reserve_completion(&mut h, drops.clone(), 0);
        let (invalid, _) = if foreign {
            reserve_completion(&mut other, drops.clone(), 0)
        } else {
            reserved(&mut h, drops.clone(), 0, true)
        };
        let keys = [valid.key.clone(), invalid.key.clone()];
        let future = h
            .handle
            .try_submit_generated_graph_v1(request(&h.context, stream, vec![valid, invalid]))
            .unwrap();
        let mut active = None;
        command(&mut h, &mut active);
        assert!(active.is_none());
        let Err(RuntimeGeneratedGraphFailureV1::Rejected(mut failure)) = ready(future).unwrap()
        else {
            panic!("invalid ticket")
        };
        match (&failure.error, foreign) {
            (
                RuntimeGeneratedGraphAdmissionErrorV1::Activation(ActivationErrorV1::Engine(
                    RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket,
                )),
                true,
            )
            | (
                RuntimeGeneratedGraphAdmissionErrorV1::Activation(ActivationErrorV1::Context(
                    RuntimeErrorV1::Validation(RuntimeValidationErrorV1::Unsupported),
                )),
                false,
            ) => {}
            other => panic!("unexpected admission error: {other:?}"),
        }
        assert_eq!(
            h.context.unpublished_identity_for_test_v1(stream),
            Some(None)
        );
        let token = h.context.reserve_graph_v1(1).unwrap();
        h.context.close_graph_issue_v1(token).unwrap();
        h.context.release_graph_v1(token).unwrap();
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        for (index, key) in keys.iter().enumerate() {
            let ticket = failure.request.take_reserved(id(index as u32 + 1)).unwrap();
            assert!(Arc::ptr_eq(key, &ticket.key));
            let registry = if foreign && index == 1 {
                &mut other.registry
            } else {
                &mut h.registry
            };
            assert!(registry.discard_reserved(&ticket.key));
        }
        assert_eq!(drops.load(Ordering::SeqCst), 2);
    }
}

#[test]
fn generated_graph_shared_direct_completion_drains_from_adoption() {
    for drop_observer in [false, true] {
        let state = Arc::new(Mutex::new(MockState::default()));
        let drops = Arc::new(AtomicUsize::new(0));
        let (engine, handle) = start(state.clone(), Arc::new(Mutex::new(OwnerTrace::default())));
        let stream = handle
            .observer
            .try_with_context(|context| context.create_stream(context.devices()[0].id()).unwrap())
            .unwrap();
        let ticket = join(preparation_with_hooks(
            &handle,
            state.clone(),
            drops.clone(),
            0,
            Some(completion_hooks()),
        ))
        .unwrap()
        .unwrap();
        let ticket = join(handle.try_reserve_prepared_v1(ticket).unwrap())
            .unwrap()
            .unwrap();
        let entered = Arc::new(Barrier::new(2));
        let released = Arc::new(Barrier::new(2));
        let (a, b) = (entered.clone(), released.clone());
        drop(
            handle
                .observer
                .enqueue_with_context(move |_| {
                    a.wait();
                    b.wait();
                })
                .unwrap(),
        );
        entered.wait();
        let activation = handle.try_activate_reserved_v1(ticket, stream).unwrap();
        let drain = handle.begin_drain(64).unwrap();
        released.wait();
        let completion = join(activation).unwrap().unwrap();
        let completion = if drop_observer {
            drop(completion);
            None
        } else {
            Some(completion)
        };
        let report = join(drain).unwrap();
        assert_eq!(report.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
        assert_eq!(report.operations_remaining, 0);
        assert_eq!(
            engine.shutdown().unwrap().disposition,
            RuntimeAsyncOwnedDispositionV1::Released
        );
        if let Some(completion) = completion {
            assert!(join(completion).unwrap().is_ok());
        }
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(
            state
                .lock()
                .unwrap()
                .adoption_order
                .contains(&"gate_commit")
        );
        assert_eq!(handle.observer.reply_cells_in_use(), 0);
    }
}
