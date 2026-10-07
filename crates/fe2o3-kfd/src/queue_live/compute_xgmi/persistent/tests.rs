use super::super::super::tests::test_queue_key;
use super::*;
use crate::persistent_allocation::{Gfx942PersistentOperationV1, Gfx942PersistentUseRequestV1};
use crate::persistent_directional_sdma::{
    Gfx942PersistentDirectionalSdmaPairV1, promote_directional_persistent_sdma_custody_v1,
};

fn allocation(
    id: u64,
    physical: u64,
    initialized: bool,
    public: bool,
) -> Gfx942DirectionalQueuePersistentAllocationV1 {
    let lease = if public {
        crate::shared_memory::local_mapping_with_extent_for_persistent_sdma_test(id, physical)
    } else {
        crate::shared_memory::private_local_mapping_for_sdma_pool_test(id)
    };
    let buffer = Gfx942SdmaBufferV1::compute_xgmi_fixture_v1(
        lease,
        test_queue_key(id, 1),
        7,
        2048,
        if initialized { 2048 } else { 1024 },
    );
    promote_directional_persistent_sdma_custody_v1(
        buffer,
        Gfx942PersistentDirectionalSdmaPairV1 {
            host_to_device_queue_id: 41,
            device_to_host_queue_id: 42,
        },
        1,
    )
    .unwrap()
    .0
}

fn root() -> TransferRoot {
    let allocations = [
        allocation(11, 4096, true, true),
        allocation(12, 8192, true, true),
    ];
    let mut root = TransferRoot::new(
        allocations
            .each_ref()
            .map(|allocation| allocation.attachment),
        [7, 9],
        Gfx942ComputeXgmiCopyWindowV1::new(2048, 2048, 0, 0, 2048).unwrap(),
    );
    root.allocations = allocations.map(Some);
    root
}

#[test]
fn compute_xgmi_persistent_preflight_rejects_wrong_scope_identity_extent_and_initialization() {
    for case in 0..11 {
        let mut allocation = allocation(11, 4096, case != 8, case != 9);
        let queue = allocation.attachment.queue;
        match case {
            0 | 1 | 2 | 8 | 9 => {}
            3 => {
                allocation.attachment.storage_identity = self::allocation(12, 4096, true, true)
                    .attachment
                    .storage_identity
            }
            4 => allocation.attachment.pool_generation += 1,
            5 => allocation.attachment.logical_bytes -= 1,
            6 => allocation.attachment.physical_bytes *= 2,
            7 => allocation.attachment.logical_bytes = 0,
            10 => allocation
                .owner
                .quarantine_for_caller_reported_currentness_loss(),
            _ => unreachable!(),
        }
        let before = allocation.owner.ownership_snapshot_for_test_v1();
        let attachment = allocation.attachment;
        let result = admit_allocation(
            &allocation,
            if case == 1 {
                test_queue_key(99, 1)
            } else {
                queue
            },
            case != 2,
        );
        assert_eq!(result.is_ok(), case == 0, "case {case}");
        assert_eq!(allocation.attachment, attachment);
        assert_eq!(allocation.owner.ownership_snapshot_for_test_v1(), before);
    }
}

#[test]
fn compute_xgmi_persistent_requires_empty_ledger_and_exact_frontier_retirement() {
    let mut allocation = allocation(11, 4096, true, true);
    let queue = allocation.attachment.queue;
    let use_lease = allocation
        .owner
        .reserve(
            Gfx942PersistentUseRequestV1::new(Gfx942PersistentOperationV1::ComputeRead, 0, 2048)
                .unwrap(),
            None,
        )
        .unwrap();
    assert!(admit_allocation(&allocation, queue, true).is_err());
    let prepared = allocation.owner.prepare(use_lease).unwrap();
    let detached = allocation
        .owner
        .detach_local_native_for_compute(&prepared)
        .unwrap();
    assert!(admit_allocation(&allocation, queue, true).is_err());
    allocation
        .owner
        .restore_local_native_from_cancelled_compute(&prepared, detached)
        .unwrap();
    let published = allocation.owner.publish(prepared).unwrap();
    let completed = allocation.owner.complete(published).unwrap();
    let frontier = allocation.owner.settle(completed).unwrap();
    assert!(admit_allocation(&allocation, queue, true).is_err());
    allocation.owner.retire_settled_frontier(frontier).unwrap();
    assert_eq!(admit_allocation(&allocation, queue, true).unwrap(), 2048);
}

#[derive(Default)]
struct Context {
    opens: [u8; 2],
    retakes: [u8; 2],
    trace: Vec<usize>,
    poisoned: [bool; 3],
}

impl model_pair_loan::Context for Context {
    type Loan = usize;
    type Error = ComputeAqlQueueSessionErrorV1;
    fn open(&mut self, endpoint: usize) -> Result<usize, Self::Error> {
        self.trace.push(endpoint);
        match self.opens[endpoint] {
            0 => Ok(endpoint),
            1 => Err(ComputeAqlQueueSessionErrorV1::Contract("injected open")),
            _ => panic!("injected open"),
        }
    }
    fn retake(&mut self, endpoint: usize, loan: usize) -> Result<(), Self::Error> {
        assert_eq!(endpoint, loan);
        self.trace.push(endpoint + 2);
        match self.retakes[endpoint] {
            0 => Ok(()),
            1 => Err(ComputeAqlQueueSessionErrorV1::Contract("injected retake")),
            _ => panic!("injected retake"),
        }
    }
    fn poison_endpoint(&mut self, endpoint: usize) {
        self.poisoned[endpoint] = true;
    }
    fn poison_process(&mut self) {
        self.poisoned[2] = true;
    }
}

#[test]
fn compute_xgmi_persistent_roundtrip_preserves_original_owners_generations_and_unequal_pool_extents()
 {
    let mut root = root();
    let before = root
        .allocations
        .each_ref()
        .map(|a| a.as_ref().unwrap().owner.ownership_snapshot_for_test_v1());
    let mut context = Context::default();
    root.execute(&mut context, |context, core| {
        assert_eq!(context.trace, [0, 1]);
        assert!(core.buffers.iter().all(Option::is_some));
        Ok(())
    })
    .unwrap();
    assert_eq!(context.trace, [0, 1, 3, 2]);
    for (index, before) in before.into_iter().enumerate() {
        let allocation = root.allocations[index].as_ref().unwrap();
        assert_eq!(allocation.owner.ownership_snapshot_for_test_v1(), before);
        assert_eq!(allocation.attachment, root.certificates[index]);
        assert_eq!(allocation.byte_len(), 2048);
        assert_eq!(allocation.physical_byte_len(), [4096, 8192][index]);
        assert_eq!(allocation.attachment.pool_generation, 7);
    }
    assert!(root.sdma.iter().all(Option::is_none));
    assert!(root.metadata.iter().all(Option::is_none));
}

#[test]
fn compute_xgmi_persistent_every_retake_error_and_unwind_keeps_backing_out_of_outputs() {
    for source_retake in 0..3 {
        for destination_retake in 0..3 {
            if source_retake == 0 && destination_retake == 0 {
                continue;
            }
            let mut root = root();
            let before = root
                .allocations
                .each_ref()
                .map(|a| a.as_ref().unwrap().owner.ownership_snapshot_for_test_v1());
            let mut context = Context {
                retakes: [source_retake, destination_retake],
                ..Default::default()
            };
            let result = catch_unwind(AssertUnwindSafe(|| {
                root.execute(&mut context, |_, _| Ok(()))
            }));
            assert!(result.is_err() || result.unwrap().is_err());
            assert_eq!(context.trace, [0, 1, 3, 2]);
            assert_eq!(context.poisoned, [true; 3]);
            assert!(root.core.buffers.iter().all(Option::is_some));
            assert!(root.metadata.iter().all(Option::is_some));
            for (index, before) in before.iter().enumerate() {
                let owner = &root.allocations[index].as_ref().unwrap().owner;
                assert!(owner.local_native_for_sdma().is_none());
                assert!(before.same_allocation(&owner.ownership_snapshot_for_test_v1()));
            }
        }
    }
}

#[test]
fn compute_xgmi_persistent_operation_failure_or_unwind_retakes_both_and_retains_original_custody() {
    for panic in [false, true] {
        let mut root = root();
        let mut context = Context::default();
        let result = catch_unwind(AssertUnwindSafe(|| {
            root.execute(&mut context, |_, _| {
                if panic {
                    panic!("injected transfer");
                }
                Err(ComputeAqlQueueSessionErrorV1::Contract("injected transfer"))
            })
        }));
        assert!(result.is_err() || result.unwrap().is_err());
        assert_eq!(context.trace, [0, 1, 3, 2]);
        assert_eq!(context.poisoned, [true; 3]);
        for index in 0..2 {
            assert!(
                root.allocations[index]
                    .as_ref()
                    .unwrap()
                    .owner
                    .local_native_for_sdma()
                    .is_none()
            );
            assert!(root.core.buffers[index].is_some());
            assert!(root.metadata[index].is_some());
        }
    }
}

#[test]
fn compute_xgmi_persistent_restoration_failure_keeps_both_original_mappings_rooted() {
    let mut root = root();
    root.prepare().unwrap();
    root.metadata.swap(0, 1);
    assert!(root.restore().is_err());
    assert!(root.locals[0].is_some());
    assert!(root.metadata.iter().all(Option::is_some));
    assert!(
        root.core.buffers[1]
            .as_mut()
            .unwrap()
            .take_local()
            .is_some()
    );
    assert!(
        root.allocations.iter().all(|a| a
            .as_ref()
            .unwrap()
            .owner
            .local_native_for_sdma()
            .is_none())
    );
}

fn assert_async_detached(root: &TransferRoot) {
    assert!(root.allocations.iter().all(|allocation| {
        allocation
            .as_ref()
            .unwrap()
            .owner
            .local_native_for_sdma()
            .is_none()
    }));
    assert!(root.core.buffers.iter().all(Option::is_some));
    assert!(root.metadata.iter().all(Option::is_some));
    assert!(root.sdma.iter().all(Option::is_none));
    assert!(root.locals.iter().all(Option::is_none));
}

fn advance_async(root: &mut TransferRoot, phase: usize) {
    let mut context = Context::default();
    if phase >= 1 {
        root.begin(&mut context, |_, _| Ok(())).unwrap();
    }
    if phase >= 2 {
        assert!(root.poll(&mut context, |_, _| Ok(true)).unwrap());
    }
}

fn async_operation(
    root: &mut TransferRoot,
    context: &mut Context,
    phase: usize,
    operation: impl FnOnce() -> Result<(), ComputeAqlQueueSessionErrorV1>,
) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    match phase {
        0 => root.begin(context, |_, _| operation()),
        1 => root
            .poll(context, |_, _| operation().map(|()| true))
            .map(|_| ()),
        2 => root.finish(context, |_, _| operation()),
        _ => unreachable!(),
    }
}

#[test]
fn compute_xgmi_async_pending_and_ready_retake_models_without_restoring_original_owners() {
    let mut root = root();
    let before = root.allocations.each_ref().map(|allocation| {
        allocation
            .as_ref()
            .unwrap()
            .owner
            .ownership_snapshot_for_test_v1()
    });
    let mut context = Context::default();
    root.begin(&mut context, |context, _| {
        context.trace.push(4);
        Ok(())
    })
    .unwrap();
    assert_eq!(context.trace, [0, 1, 4, 3, 2]);
    assert_eq!(root.phase, Phase::Published);
    assert_async_detached(&root);
    let mut samples = 0;
    for ready in [false, false, true] {
        context.trace.clear();
        assert_eq!(
            root.poll(&mut context, |context, _| {
                samples += 1;
                context.trace.push(5);
                Ok(ready)
            })
            .unwrap(),
            ready
        );
        assert_eq!(context.trace, [0, 1, 5, 3, 2]);
        assert_eq!(
            root.phase,
            if ready {
                Phase::Ready
            } else {
                Phase::Published
            }
        );
        assert_async_detached(&root);
    }
    assert_eq!(samples, 3);
    context.trace.clear();
    assert!(
        root.poll(&mut context, |_, _| panic!("ready must not sample again"))
            .unwrap()
    );
    assert!(context.trace.is_empty());
    assert_async_detached(&root);
    root.finish(&mut context, |context, _| {
        context.trace.push(6);
        Ok(())
    })
    .unwrap();
    assert_eq!(context.trace, [0, 1, 6, 3, 2]);
    assert_eq!(root.phase, Phase::Finished);
    for (index, original) in before.into_iter().enumerate() {
        let allocation = root.allocations[index].as_ref().unwrap();
        assert_eq!(allocation.owner.ownership_snapshot_for_test_v1(), original);
        assert_eq!(allocation.attachment, root.certificates[index]);
        assert_eq!(allocation.byte_len(), 2048);
        assert_eq!(allocation.physical_byte_len(), [4096, 8192][index]);
    }
    assert!(root.metadata.iter().all(Option::is_none));
    assert!(context.poisoned.iter().all(|poisoned| !poisoned));
}

#[test]
fn compute_xgmi_async_every_phase_retake_failure_retains_detached_custody_and_forbids_retry() {
    for phase in 0..3 {
        for source in 0..3 {
            for destination in 0..3 {
                if source == 0 && destination == 0 {
                    continue;
                }
                let mut root = root();
                let originals = root.allocations.each_ref().map(|allocation| {
                    allocation
                        .as_ref()
                        .unwrap()
                        .owner
                        .ownership_snapshot_for_test_v1()
                });
                advance_async(&mut root, phase);
                let mut context = Context {
                    retakes: [source, destination],
                    ..Default::default()
                };
                let result = catch_unwind(AssertUnwindSafe(|| {
                    async_operation(&mut root, &mut context, phase, || Ok(()))
                }));
                assert!(result.is_err() || result.unwrap().is_err());
                assert_eq!(root.phase, Phase::Terminal);
                assert_eq!(context.trace, [0, 1, 3, 2]);
                assert_eq!(context.poisoned, [true; 3]);
                assert_async_detached(&root);
                for (index, original) in originals.iter().enumerate() {
                    assert!(
                        original.same_allocation(
                            &root.allocations[index]
                                .as_ref()
                                .unwrap()
                                .owner
                                .ownership_snapshot_for_test_v1()
                        )
                    );
                }
                context.trace.clear();
                assert!(
                    root.poll(&mut context, |_, _| panic!("terminal retry"))
                        .is_err()
                );
                assert!(context.trace.is_empty());
            }
        }
    }
}

#[test]
fn compute_xgmi_async_operation_errors_and_panics_settle_both_models_in_every_phase() {
    for phase in 0..3 {
        for panic in [false, true] {
            let mut root = root();
            advance_async(&mut root, phase);
            let mut context = Context {
                retakes: if panic { [2, 1] } else { [0, 0] },
                ..Default::default()
            };
            let result = catch_unwind(AssertUnwindSafe(|| {
                async_operation(&mut root, &mut context, phase, || {
                    if panic {
                        panic!("async operation panic");
                    }
                    Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "async operation error",
                    ))
                })
            }));
            if panic {
                assert_eq!(
                    result.err().unwrap().downcast_ref::<&str>(),
                    Some(&"async operation panic")
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(root.phase, Phase::Terminal);
            assert_eq!(context.trace, [0, 1, 3, 2]);
            assert_eq!(context.poisoned, [true; 3]);
            assert_async_detached(&root);
        }
    }
}

#[test]
fn compute_xgmi_async_open_failures_never_run_phase_and_keep_exact_rooted_inputs() {
    for phase in 0..3 {
        for endpoint in 0..2 {
            for fault in 1..3 {
                let mut root = root();
                advance_async(&mut root, phase);
                let before = root.allocations.each_ref().map(|allocation| {
                    allocation
                        .as_ref()
                        .unwrap()
                        .owner
                        .ownership_snapshot_for_test_v1()
                });
                let mut context = Context::default();
                context.opens[endpoint] = fault;
                let result = catch_unwind(AssertUnwindSafe(|| {
                    async_operation(&mut root, &mut context, phase, || {
                        panic!("phase must not run")
                    })
                }));
                assert!(result.is_err() || result.unwrap().is_err());
                assert_eq!(root.phase, Phase::Terminal);
                assert_eq!(
                    context.trace,
                    if endpoint == 0 {
                        vec![0]
                    } else {
                        vec![0, 1, 2]
                    }
                );
                assert_eq!(context.poisoned, [true; 3]);
                for (index, original) in before.into_iter().enumerate() {
                    assert_eq!(
                        root.allocations[index]
                            .as_ref()
                            .unwrap()
                            .owner
                            .ownership_snapshot_for_test_v1(),
                        original
                    );
                }
                if phase == 0 {
                    assert!(root.metadata.iter().all(Option::is_none));
                    assert!(root.core.buffers.iter().all(Option::is_none));
                } else {
                    assert_async_detached(&root);
                }
            }
        }
    }
}

#[test]
fn compute_xgmi_async_phase_and_output_rejection_are_effect_free() {
    let mut root = root();
    let mut context = Context::default();
    assert!(
        root.poll(&mut context, |_, _| panic!("not published"))
            .is_err()
    );
    assert!(
        root.finish(&mut context, |_, _| panic!("not ready"))
            .is_err()
    );
    assert_eq!(root.phase, Phase::Admitted);
    assert!(context.trace.is_empty());
    root.begin(&mut context, |_, _| Ok(())).unwrap();
    context.trace.clear();
    assert!(
        root.begin(&mut context, |_, _| panic!("duplicate publish"))
            .is_err()
    );
    assert!(
        root.finish(&mut context, |_, _| panic!("pending cannot remap"))
            .is_err()
    );
    assert_eq!(root.phase, Phase::Published);
    assert!(context.trace.is_empty());
    assert_async_detached(&root);
    assert!(context.poisoned.iter().all(|poisoned| !poisoned));
    for source in [false, true] {
        for destination in [false, true] {
            let outputs = [
                source.then(|| allocation(21, 4096, true, true)),
                destination.then(|| allocation(22, 8192, true, true)),
            ];
            let before = outputs.each_ref().map(|output| {
                output
                    .as_ref()
                    .map(|owner| owner.owner.ownership_snapshot_for_test_v1())
            });
            assert_eq!(
                require_empty_outputs(&outputs[0], &outputs[1]).is_ok(),
                !source && !destination
            );
            assert_eq!(
                outputs.each_ref().map(|output| output
                    .as_ref()
                    .map(|owner| owner.owner.ownership_snapshot_for_test_v1())),
                before
            );
            assert_eq!(root.phase, Phase::Published);
            assert_async_detached(&root);
        }
    }
}
