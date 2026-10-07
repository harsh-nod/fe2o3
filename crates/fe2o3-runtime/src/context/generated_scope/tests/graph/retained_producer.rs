//! Exact graph ordering with a synthetic retained phase, not native acceptance.

use super::*;

#[test]
fn retained_producer_does_not_ready_successor_or_release_graph_credit() {
    let mut context = context();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let identity = context.completion_stream_identity_v1(stream).unwrap();
    let nodes = (1..=2)
        .map(|n| {
            CompletionNodeV1::future(
                id(n),
                FutureIdentityV1::new(identity, [n as u8; 32]),
                (n == 2).then(|| id(1)),
            )
        })
        .collect();
    let request = RuntimeGraphRequestV1::new(
        CompletionGraphV1::new(identity.context(), vec![identity], nodes).unwrap(),
        vec![(identity, stream)],
    )
    .unwrap();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let order = RefCell::new(Vec::new());
    context
        .with_generated_gfx942_scope_v1::<Borrowed<'_>, _>(
            2,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                scope.hooks = hooks();
                scope.hooks.complete = |context, prepared, _, hold| {
                    context.validate_unpublished_hold_v1(hold)?;
                    assert_eq!(context.graph_reservation, hold.graph_access());
                    if prepared.value().ticks.replace(1) == 0 {
                        return Ok(false);
                    }
                    context.release_unpublished_hold_v1(hold)?;
                    Ok(true)
                };
                let ticket = scope
                    .admit_graph_with_v1::<()>(request, |context, node, stream| {
                        let n = if node == id(1) { 1 } else { 2 };
                        Ok(prepare(context, stream, &decoded, &dropped, &order, n, 0))
                    })
                    .unwrap();
                let original_graph = scope.context.graph_reservation;
                for _ in 0..3 {
                    scope.progress_v1().unwrap();
                }
                assert_eq!(scope.slots.len(), 1);
                assert_eq!(scope.slots[0].lifecycle.phase, Phase::RetainedProducer);
                assert_eq!((decoded.get(), dropped.get()), (0, 0));
                assert!(scope.slots[0].lifecycle.outcome.is_none());
                assert_eq!(scope.context.graph_reservation, original_graph);
                assert!(scope.context.has_unpublished_holds_v1());
                assert!(
                    scope
                        .graph_generated_ticket_v1(&ticket, id(2))
                        .unwrap()
                        .is_none()
                );
                assert!(scope.graph_report_v1(&ticket).unwrap().is_none());
                scope.drain_v1().unwrap();
                assert_eq!(*order.borrow(), [1, 2]);
                assert_eq!((decoded.get(), dropped.get()), (2, 2));
                assert!(
                    scope
                        .graph_report_v1(&ticket)
                        .unwrap()
                        .unwrap()
                        .errors
                        .is_empty()
                );
                assert!(scope.context.graph_reservation.is_none());
            },
        )
        .unwrap();
    assert!(context.cleanup().is_complete());
}
