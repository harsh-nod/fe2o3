#![cfg(test)]
use super::*;
use crate::{RuntimeGraphDeviceCoverageV1, RuntimeGraphRequestV1};
use fe2o3_completion::{CompletionGraphV1, CompletionNodeIdV1, CompletionNodeV1, FutureIdentityV1};

#[test]
fn actual_graph_retirement_precedes_partial_manifest_return_for_all_admitted_children() {
    let mut context = RuntimeContextV1::open_with_version_journal_v1(
        KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1(),
        32,
        32,
    )
    .unwrap();
    assert_eq!(context.devices().len(), 3);
    let devices: [_; 3] = core::array::from_fn(|index| context.devices()[index].id());
    let uid = context.devices()[0].backend_device;
    let streams = devices.map(|device| context.create_stream(device).unwrap());
    assert!(matches!(
        context.create_graph_group_v1(&streams[..2], RuntimeGraphDeviceCoverageV1::AllAdmitted),
        Err(RuntimeValidationErrorV1::WrongDevice)
    ));
    let group = context
        .create_graph_group_v1(&streams, RuntimeGraphDeviceCoverageV1::AllAdmitted)
        .unwrap();
    let identities = streams.map(|s| group.stream_identity(s).unwrap());
    let id = |n| CompletionNodeIdV1::new(n).unwrap();
    let node = |n, stream, previous: Option<u32>| {
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
                node(1, identities[0], None),
                node(2, identities[0], Some(1)),
                node(3, identities[1], None),
                node(4, identities[2], None),
            ],
        )
        .unwrap(),
        group,
    )
    .unwrap();
    let decoded = [const { Cell::new(0) }; 4];
    let dropped = [const { Cell::new(0) }; 4];
    let returned = context
        .with_generated_gfx942_scope_settled_v1::<Borrowed<'_>, _>(
            4,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                scope.hooks = cold_device::cold_hooks();
                let ticket = scope
                    .admit_graph_with_v1::<()>(request, |context, node, stream| {
                        let n = (1..=4).find(|&n| id(n) == node).unwrap() as usize - 1;
                        Ok(context.bound_multi_preparation_for_test_v1(
                            context.streams[&stream].device,
                            borrowed(&decoded[n], &dropped[n]),
                        ))
                    })
                    .unwrap();
                for _ in 0..32 {
                    if scope.pending_v1() == 0 {
                        break;
                    }
                    scope.progress_v1().unwrap();
                }
                assert_eq!(scope.pending_v1(), 0);
                assert!(scope.context.graph_reservation.is_none());
                let report = scope.graph_report_v1(&ticket).unwrap().unwrap();
                report.observations.clone()
            },
        )
        .unwrap();
    assert!(!context.scope_epoch.active());
    assert!(context.graph_reservation.is_none());
    assert!(!context.has_unpublished_holds_v1());
    assert_eq!(decoded.each_ref().map(Cell::get), [0, 0, 1, 1]);
    assert_eq!(dropped.each_ref().map(Cell::get), [1, 1, 1, 1]);
    let (observations, outcome) = returned.into_parts_v1();
    assert_eq!(
        outcome,
        Err(RuntimeGfx942SettledFailureV1::DeviceUnavailableBeforeActivation { device_uid: uid })
    );
    assert!(observations.contains(&(id(1), RuntimeCompletionStatusV1::QuiescentWithoutResult)));
    assert!(observations.contains(&(id(3), RuntimeCompletionStatusV1::Succeeded)));
    assert!(observations.contains(&(id(4), RuntimeCompletionStatusV1::Succeeded)));
    assert!(!observations.iter().any(|(node, result)| *node == id(1)
        && *result == RuntimeCompletionStatusV1::Succeeded));
    assert!(context.cleanup().is_complete());
}
