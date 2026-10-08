//! Only the shared CPU driver join is exercised; there is no native reset here.
#![cfg(test)]
use super::*;
use crate::RuntimeGraphDeviceCoverageV1;

#[test]
fn settled_cold_child_blocks_its_successor_and_preserves_independent_graph_branch() {
    let mut context = context();
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    let uid = context.devices()[0].backend_device;
    let streams = devices.map(|device| context.create_stream(device).unwrap());
    let group = context
        .create_graph_group_v1(&streams, RuntimeGraphDeviceCoverageV1::Selected)
        .unwrap();
    let identities = streams.map(|stream| group.stream_identity(stream).unwrap());
    let future = |n, stream, previous: Option<u32>| {
        CompletionNodeV1::future(
            id(n),
            FutureIdentityV1::new(stream, [n as u8; 32]),
            previous.map(id),
        )
    };
    let request = RuntimeGraphRequestV1::new_group_v1(
        CompletionGraphV1::new(
            group.context_identity(),
            identities.to_vec(),
            vec![
                future(1, identities[0], None),
                future(2, identities[0], Some(1)),
                future(3, identities[1], None),
            ],
        )
        .unwrap(),
        group,
    )
    .unwrap();
    let decoded = [Cell::new(0), Cell::new(0), Cell::new(0)];
    let dropped = [Cell::new(0), Cell::new(0), Cell::new(0)];
    let order = RefCell::new(Vec::new());
    let result = context.with_generated_gfx942_scope_v1::<Borrowed<'_>,_>(
        3, Instant::now() + Duration::from_secs(30), |scope| {
            scope.hooks = super::super::cold_device::cold_hooks();
            let ticket = scope.admit_graph_with_v1::<()>(request, |context,node,stream| {
                let n = (1..=3).find(|&n| id(n)==node).unwrap();
                Ok(prepare(context,stream,&decoded[n as usize-1],&dropped[n as usize-1],&order,n,2))
            }).unwrap();
            for _ in 0..32 {
                if scope.pending_v1()==0 { break; }
                scope.progress_v1().unwrap();
            }
            assert_eq!(scope.pending_v1(),0);
            assert_eq!(decoded.each_ref().map(Cell::get), [0,0,1]);
            assert_eq!(dropped.each_ref().map(Cell::get), [1,1,1]);
            assert_eq!(*order.borrow(),[3]);
            assert_eq!(scope.cold_device_failures_v1().count(),1);
            assert!(scope.context.graph_reservation.is_none());
            assert!(!scope.context.has_unpublished_holds_v1());
            let failed = scope.graph_generated_ticket_v1(&ticket,id(1)).unwrap().unwrap();
            assert!(matches!(scope.completion_v1(&failed),Err(RuntimeGfx942ScopeErrorV1::DeviceUnavailableBeforeActivation { device_uid }) if device_uid == uid));
            assert!(scope.graph_generated_ticket_v1(&ticket,id(2)).unwrap().is_none());
            let healthy = scope.graph_generated_ticket_v1(&ticket,id(3)).unwrap().unwrap();
            assert!(matches!(scope.completion_v1(&healthy),Ok(Some(Ok(())))));
            let report = scope.graph_report_v1(&ticket).unwrap().unwrap();
            assert!(report.observations.contains(&(id(1),RuntimeCompletionStatusV1::QuiescentWithoutResult)));
            assert!(report.observations.contains(&(id(3),RuntimeCompletionStatusV1::Succeeded)));
            assert!(!report.observations.iter().any(|(node,status)| *node==id(1) && *status==RuntimeCompletionStatusV1::Succeeded));
        },
    );
    assert!(
        matches!(result,Err(RuntimeGfx942ScopeErrorV1::DeviceUnavailableBeforeActivation { device_uid }) if device_uid == uid)
    );
    assert!(!context.is_terminal());
    assert!(context.cleanup().is_complete());
}
