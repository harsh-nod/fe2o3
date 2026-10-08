//! Synthetic native hooks test the shared graph failure join, not native execution.
#![cfg(test)]
use super::*;

#[test]
fn settled_rejected_generated_node_blocks_successor_but_keeps_independent_branch() {
    let mut context = context();
    let device = context.devices()[0].id();
    let streams = [
        context.create_stream(device).unwrap(),
        context.create_stream(device).unwrap(),
    ];
    let identities = streams.map(|s| context.completion_stream_identity_v1(s).unwrap());
    let future = |n, stream, previous: Option<u32>| {
        CompletionNodeV1::future(
            id(n),
            FutureIdentityV1::new(stream, [n as u8; 32]),
            previous.map(id),
        )
    };
    let request = RuntimeGraphRequestV1::new(
        CompletionGraphV1::new(
            identities[0].context(),
            identities.to_vec(),
            vec![
                future(1, identities[0], None),
                future(2, identities[0], Some(1)),
                future(3, identities[1], None),
            ],
        )
        .unwrap(),
        identities.into_iter().zip(streams).collect(),
    )
    .unwrap();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let order = RefCell::new(Vec::new());
    let result = context.with_generated_gfx942_scope_v1::<Borrowed<'_>, _>(
        3,
        Instant::now() + Duration::from_secs(30),
        |scope| {
            scope.hooks = hooks();
            scope.hooks.rejected = |context, hold| {
                context.validate_unpublished_hold_v1(hold)?;
                Ok(context
                    .streams
                    .keys()
                    .min()
                    .is_some_and(|s| *s == hold.stream()))
            };
            scope.hooks.retire_rejected = |context, prepared, _, hold| {
                assert!(hold.is_graph_scoped_v1());
                context.validate_unpublished_hold_v1(hold)?;
                assert_eq!(prepared.value().completion_order.unwrap().1, 1);
                Ok(())
            };
            let ticket = scope
                .admit_graph_with_v1::<()>(request, |context, node, stream| {
                    let n = (1..=3).find(|&n| id(n) == node).unwrap();
                    Ok(prepare(context, stream, &decoded, &dropped, &order, n, 2))
                })
                .unwrap();
            for _ in 0..32 {
                if scope.pending_v1() == 0 {
                    break;
                }
                scope.progress_v1().unwrap();
            }
            assert_eq!(scope.pending_v1(), 0);
            assert_eq!((decoded.get(), dropped.get()), (1, 3));
            assert_eq!(*order.borrow(), [3]);
            assert!(scope.context.graph_reservation.is_none());
            assert!(!scope.context.has_unpublished_holds_v1());
            let failed = scope
                .graph_generated_ticket_v1(&ticket, id(1))
                .unwrap()
                .unwrap();
            assert!(matches!(
                scope.completion_v1(&failed),
                Err(RuntimeGfx942ScopeErrorV1::RejectedBeforePublication)
            ));
            assert!(
                scope
                    .graph_generated_ticket_v1(&ticket, id(2))
                    .unwrap()
                    .is_none()
            );
            let healthy = scope
                .graph_generated_ticket_v1(&ticket, id(3))
                .unwrap()
                .unwrap();
            assert!(matches!(scope.completion_v1(&healthy), Ok(Some(Ok(())))));
            let report = scope.graph_report_v1(&ticket).unwrap().unwrap();
            assert!(
                report
                    .observations
                    .contains(&(id(1), RuntimeCompletionStatusV1::QuiescentWithoutResult))
            );
            assert!(
                report
                    .observations
                    .contains(&(id(3), RuntimeCompletionStatusV1::Succeeded))
            );
            assert!(
                !report
                    .observations
                    .iter()
                    .any(|(node, status)| *node == id(1)
                        && *status == RuntimeCompletionStatusV1::Succeeded)
            );
        },
    );
    assert!(matches!(
        result,
        Err(RuntimeGfx942ScopeErrorV1::RejectedBeforePublication)
    ));
    assert!(!context.is_terminal());
    assert!(context.cleanup().is_complete());
}
