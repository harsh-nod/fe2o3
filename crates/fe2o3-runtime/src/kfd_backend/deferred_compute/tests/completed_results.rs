//! Completed handoff and typed Context composition with scripted child receipts.
//! The fixture constructs the router handoff, not native peer DMA or GPU success.

use super::super::*;
use crate::kfd_backend::kfd_backend_sdma_seam::{ScriptedRecycleOutcomeV1, ScriptedSdmaStepV1};
use crate::kfd_backend::materialized_completion::ScriptedCompletionStepV1 as Step;
use crate::{
    RuntimeAllocationIdV1, RuntimeArgumentsV1, RuntimeBindingV1, RuntimeContextV1,
    RuntimeEventIdV1, RuntimeMemoryRegionV1, RuntimeModuleIdV1, RuntimePollV1, RuntimeStreamIdV1,
    RuntimeSubmissionV1, TypedRuntimeKernelV1,
};
use std::mem::ManuallyDrop;

#[derive(Debug)]
struct Arguments(RuntimeMemoryRegionV1);

impl RuntimeArgumentsV1 for Arguments {
    const SIGNATURE_V1: [u8; 32] = [7; 32];
    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        vec![0; 16]
    }
    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        vec![RuntimeBindingV1 {
            region: self.0,
            kernarg_byte_offset: 0,
        }]
    }
}

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;

struct Fixture {
    context: ManuallyDrop<Context>,
    producer: Option<RuntimeSubmissionV1<Arguments>>,
    event: Option<RuntimeEventIdV1>,
    producer_id: u64,
    producer_route: RoutedHandleV1,
    producer_stream: Option<RuntimeStreamIdV1>,
    consumer_stream: RuntimeStreamIdV1,
    allocation: RuntimeAllocationIdV1,
    module: RuntimeModuleIdV1,
    kernel: TypedRuntimeKernelV1<Arguments>,
}

fn geometry() -> crate::RuntimeLaunchGeometryV1 {
    crate::RuntimeLaunchGeometryV1 {
        grid: [64, 1, 1],
        workgroup: [64, 1, 1],
        dynamic_shared_bytes: 0,
    }
}

impl Fixture {
    fn new() -> Self {
        Self::with_producer_access(RuntimeAccessV1::Read)
    }

    fn with_producer_access(access: RuntimeAccessV1) -> Self {
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let backend =
            KfdMultiDeviceRuntimeBackendV1::from_backends(vec![KfdRuntimeBackendV1::mock(), right])
                .unwrap();
        let mut context =
            ManuallyDrop::new(Context::open_with_version_journal_v1(backend, 16, 16).unwrap());
        let device = context.devices()[0].id();
        let producer_stream = context.create_stream(device).unwrap();
        let consumer_stream = context.create_stream(device).unwrap();
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 8)
            .unwrap();
        context.write_allocation(allocation, 0, &[29; 64]).unwrap();
        let module = context
            .load_module(device, &crate::synthetic_cov6::module())
            .unwrap();
        let kernel = context
            .resolve_kernel::<Arguments>(module, "vecadd")
            .unwrap();
        let global_allocation = *context.backend().allocations.keys().next().unwrap();
        let allocation_route = context.backend().allocations[&global_allocation];
        let backend = context.backend_mut_for_test_v1();
        let driver = ScriptedSdmaDriverV1::new([ScriptedSdmaStepV1::Recycle(
            ScriptedRecycleOutcomeV1::Success,
        )]);
        let host = driver.test_host_owner(64);
        let child = &mut backend.children[0];
        let record = child.allocations.get_mut(&allocation_route.local).unwrap();
        record.sdma_storage = KfdRuntimeSdmaStorageV1::Host(host);
        record.sdma_backed = true;
        record.sdma_initialized = true;
        child.native_available = true;
        child.sdma_enabled = true;
        child.scripted_sdma = Some(driver);
        child.scripted_materialized_preparation =
            Some((MaterializedPreparationOriginV1::NewBinding, 0));
        let mut producer = context
            .launch_producer_aware_v1(
                producer_stream,
                &kernel,
                &Arguments(RuntimeMemoryRegionV1 {
                    allocation,
                    access,
                    byte_offset: 0,
                    byte_len: 64,
                }),
                geometry(),
                &[],
            )
            .unwrap();
        let producer_id = context.backend_submission_for_test_v1(&producer).unwrap();
        let backend = context.backend_mut_for_test_v1();
        let RoutedSubmissionV1::Native { route, stream } = backend.submissions[&producer_id] else {
            unreachable!()
        };
        backend.children[route.child].scripted_materialized_completion = Some(VecDeque::from([
            (route.local, Step::PollReady),
            (route.local, Step::RecycleReady),
        ]));
        let global_kernel = *backend.kernels.keys().next().unwrap();
        let global_module = backend.kernel_modules[&global_kernel];
        let child = &backend.children[route.child];
        let launch = child
            .pending_compute
            .get(&route.local)
            .map(|pending| Arc::clone(&pending.launch))
            .or_else(|| {
                child
                    .active_compute_submission_v1(route.local)
                    .and_then(|active| active.ordinary_recipe.as_ref())
                    .map(Arc::clone)
            })
            .expect("admitted launch remains rooted across eager preparation");
        let root = Box::new(DeferredComputeV1 {
            stream,
            child: route.child,
            route: Some(route),
            status: BackendPollV1::Pending,
            quiescent: None,
            completed: None,
            identity: DeferredComputeIdentityV1::new(
                &launch,
                compute_peer::compute_identity(child, route.local)
                    .unwrap()
                    .1,
                Some(route),
            ),
            launch: Some(launch),
            collected: None,
            kernel: (global_kernel, backend.kernels[&global_kernel]),
            module: (global_module, backend.modules[&global_module]),
            allocations: vec![(global_allocation, allocation_route)],
            peers: Vec::new(),
        });
        // This is the already-admitted child handoff boundary. Completion still
        // traverses the real child settlement and router observation paths.
        backend
            .deferred_compute_retains
            .prepare(stream, &root.allocations, root.module.1)
            .unwrap();
        backend.deferred_compute_retains.acquire(producer_id, &root);
        backend
            .submissions
            .insert(producer_id, RoutedSubmissionV1::DeferredCompute(root));
        for _ in 0..8 {
            context.flush_stream(producer_stream).unwrap();
            if context.poll(&mut producer).unwrap() == RuntimePollV1::Succeeded {
                break;
            }
        }
        assert_eq!(
            context.poll(&mut producer).unwrap(),
            RuntimePollV1::Succeeded
        );
        assert_eq!(
            context
                .backend()
                .deferred_compute_v1(producer_id)
                .unwrap()
                .status,
            BackendPollV1::Succeeded
        );
        assert!(context.backend().children[route.child].exact_submission_quiescent_v1(route.local));
        if access == RuntimeAccessV1::Read {
            Self::discard_scripted_cache(&mut context);
        }
        let event = context.record_event(&producer).unwrap();
        Self {
            context,
            producer: Some(producer),
            event: Some(event),
            producer_id,
            producer_route: route,
            producer_stream: Some(producer_stream),
            consumer_stream,
            allocation,
            module,
            kernel,
        }
    }

    fn discard_scripted_cache(context: &mut Context) {
        let child = &mut context.backend_mut_for_test_v1().children[0];
        assert!(child.queue.is_none());
        assert!(!child.any_compute_active_v1());
        assert!(child.pending_compute.is_empty());
        assert_eq!(child.native_dirty_extents, 0);
        // Scripted completion has no native DATA cache to detach. End that
        // metadata-only fixture boundary without claiming native cache cleanup.
        child.recycled_dispatch = None;
    }

    fn dependency(&self) -> BackendLaunchProducerV1 {
        let event = self
            .context
            .backend()
            .events
            .iter()
            .find_map(|(id, event)| match event {
                RoutedEventV1::DeferredCompute { submission, .. }
                    if *submission == self.producer_id =>
                {
                    Some(*id)
                }
                _ => None,
            })
            .unwrap();
        BackendLaunchProducerV1 {
            event,
            producer_submission: self.producer_id,
        }
    }

    fn launch(&mut self) -> RuntimeSubmissionV1<Arguments> {
        // Clean admission may eagerly prepare. The existing scripted retry
        // keeps publication explicit and permits genuine prepared cancellation.
        self.context.backend_mut_for_test_v1().children[0].scripted_materialized_preparation =
            Some((MaterializedPreparationOriginV1::NewBinding, 0));
        let consumer = self
            .context
            .launch_producer_aware_v1(
                self.consumer_stream,
                &self.kernel,
                &Arguments(RuntimeMemoryRegionV1 {
                    allocation: self.allocation,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 64,
                }),
                geometry(),
                &[self.event.unwrap()],
            )
            .unwrap();
        let id = self
            .context
            .backend_submission_for_test_v1(&consumer)
            .unwrap();
        let backend = self.context.backend_mut_for_test_v1();
        let RoutedSubmissionV1::Native { route, .. } = backend.submissions[&id] else {
            unreachable!()
        };
        backend.children[route.child].scripted_materialized_completion = Some(VecDeque::from([
            (route.local, Step::PollPending),
            (route.local, Step::PollReady),
            (route.local, Step::RecycleReady),
        ]));
        consumer
    }

    fn clean(mut self) {
        Self::discard_scripted_cache(&mut self.context);
        if let Some(event) = self.event.take() {
            self.context.release_event(event).unwrap();
        }
        if let Some(producer) = self.producer.take() {
            self.context.release_submission(producer).unwrap();
        }
        self.context.release_allocation(self.allocation).unwrap();
        self.context.unload_module(self.module).unwrap();
        self.context.destroy_stream(self.consumer_stream).unwrap();
        if let Some(stream) = self.producer_stream.take() {
            self.context.destroy_stream(stream).unwrap();
        }
        assert!(self.context.backend().peer_launch_retains.is_empty());
        let mut backend = ManuallyDrop::into_inner(self.context).shutdown().unwrap();
        let driver = backend.children[0].scripted_sdma.as_ref().unwrap();
        assert_eq!(driver.remaining_steps(), 0);
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
        backend.shutdown_native_v1().unwrap();
    }
}

#[test]
fn completed_deferred_result_typed_reuse_retains_independently_of_public_event() {
    for destroyed_stream in [false, true] {
        let mut f = Fixture::new();
        if destroyed_stream {
            f.context
                .destroy_stream(f.producer_stream.take().unwrap())
                .unwrap();
        }
        let mut consumer = f.launch();
        let consumer_id = f.context.backend_submission_for_test_v1(&consumer).unwrap();
        assert!(
            f.context
                .backend()
                .peer_launch_retains
                .retains(f.producer_id)
        );
        f.context.release_event(f.event.take().unwrap()).unwrap();
        assert!(
            f.context
                .backend()
                .peer_launch_retains
                .retains(f.producer_id)
        );
        assert!(
            matches!(f.context.backend_mut_for_test_v1().release_submission_v1(f.producer_id),
            Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
        let RoutedSubmissionV1::Native { route, .. } =
            f.context.backend().submissions[&consumer_id]
        else {
            unreachable!()
        };
        let child = &f.context.backend().children[route.child];
        assert!(
            !child
                .compute_dependency_retain_counts
                .contains_key(&f.producer_route.local)
        );
        if let Some(pending) = child.pending_compute.get(&route.local) {
            assert!(pending.explicit_success_dependencies.is_empty());
            assert_eq!(pending.dependency_depth, 2);
        } else {
            let active = child.active_compute_submission_v1(route.local).unwrap();
            assert_eq!(active.dependency_depth, 2);
            assert!(matches!(
                active.execution,
                Some(ActiveComputeExecutionV1::MaterializedPrepared(_))
            ));
        }
        // Flush does not retry an active prepared launch. Its first poll reaches
        // publication, which must still retain the independent router result.
        f.context.flush_stream(f.consumer_stream).unwrap();
        assert_eq!(
            f.context.poll(&mut consumer).unwrap(),
            RuntimePollV1::Pending
        );
        assert!(
            f.context
                .backend()
                .peer_launch_retains
                .retains(f.producer_id)
        );
        for _ in 0..8 {
            if f.context.poll(&mut consumer).unwrap() == RuntimePollV1::Succeeded {
                break;
            }
            f.context.flush_stream(f.consumer_stream).unwrap();
        }
        assert_eq!(
            f.context.poll(&mut consumer).unwrap(),
            RuntimePollV1::Succeeded
        );
        assert!(
            !f.context
                .backend()
                .peer_launch_retains
                .retains(f.producer_id)
        );
        f.context.release_submission(consumer).unwrap();
        f.clean();
    }
}

#[test]
fn completed_deferred_result_authentication_rejects_pending_failed_and_wrong_child() {
    let mut f = Fixture::new();
    let dependency = f.dependency();
    let backend = f.context.backend_mut_for_test_v1();
    assert_eq!(
        backend
            .completed_deferred_dependency_depth_v1(dependency, 0)
            .unwrap(),
        2
    );
    for (request, child, kind) in [
        (
            BackendLaunchProducerV1 {
                producer_submission: dependency.producer_submission + 1,
                ..dependency
            },
            0,
            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
        ),
        (dependency, 1, KfdRuntimeBackendErrorKindV1::WrongDevice),
    ] {
        assert!(
            matches!(backend.completed_deferred_dependency_depth_v1(request, child),
            Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == kind)
        );
    }
    for status in [BackendPollV1::Pending, BackendPollV1::Failed { code: 3 }] {
        backend.deferred_compute_mut_v1(f.producer_id).status = status;
        assert!(
            matches!(backend.completed_deferred_dependency_depth_v1(dependency, 0),
            Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == if status == BackendPollV1::Pending { KfdRuntimeBackendErrorKindV1::Unsupported } else { KfdRuntimeBackendErrorKindV1::InvalidLaunch })
        );
    }
    backend.deferred_compute_mut_v1(f.producer_id).status = BackendPollV1::Succeeded;
    let route = f.producer_route;
    let receipt = backend
        .deferred_compute_v1(f.producer_id)
        .unwrap()
        .completed
        .unwrap();
    for wrong in [
        RoutedHandleV1 { child: 1, ..route },
        RoutedHandleV1 {
            local: route.local + 1000,
            ..route
        },
    ] {
        backend.deferred_compute_mut_v1(f.producer_id).route = Some(wrong);
        assert!(
            backend
                .completed_deferred_dependency_depth_v1(dependency, 0)
                .is_err()
        );
    }
    backend.deferred_compute_mut_v1(f.producer_id).route = Some(route);
    backend.deferred_compute_mut_v1(f.producer_id).stream = receipt.stream + 1000;
    assert!(
        backend
            .completed_deferred_dependency_depth_v1(dependency, 0)
            .is_err()
    );
    backend.deferred_compute_mut_v1(f.producer_id).stream = receipt.stream;
    backend.children[route.child]
        .submissions
        .get_mut(&route.local)
        .unwrap()
        .stream += 1;
    assert!(
        backend
            .completed_deferred_dependency_depth_v1(dependency, 0)
            .is_err()
    );
    backend.children[route.child]
        .submissions
        .get_mut(&route.local)
        .unwrap()
        .stream = receipt.local_stream;
    for status in [BackendPollV1::Pending, BackendPollV1::Failed { code: 4 }] {
        backend.children[route.child]
            .submissions
            .get_mut(&route.local)
            .unwrap()
            .status = status;
        assert!(
            backend
                .completed_deferred_dependency_depth_v1(dependency, 0)
                .is_err()
        );
    }
    backend.children[route.child]
        .submissions
        .get_mut(&route.local)
        .unwrap()
        .status = BackendPollV1::Succeeded;
    for depth in [0, MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1, usize::MAX] {
        backend.children[route.child]
            .submissions
            .get_mut(&route.local)
            .unwrap()
            .dependency_depth = depth;
        backend
            .deferred_compute_mut_v1(f.producer_id)
            .completed
            .as_mut()
            .unwrap()
            .dependency_depth = depth;
        assert!(
            backend
                .completed_deferred_dependency_depth_v1(dependency, 0)
                .is_err()
        );
    }
    backend.children[route.child]
        .submissions
        .get_mut(&route.local)
        .unwrap()
        .dependency_depth = 1;
    backend.deferred_compute_mut_v1(f.producer_id).completed = None;
    assert!(
        backend
            .completed_deferred_dependency_depth_v1(dependency, 0)
            .is_err()
    );
    backend.deferred_compute_mut_v1(f.producer_id).completed = Some(receipt);
    backend.deferred_compute_mut_v1(f.producer_id).route = None;
    assert!(
        backend
            .completed_deferred_dependency_depth_v1(dependency, 0)
            .is_err()
    );
    backend.deferred_compute_mut_v1(f.producer_id).route = Some(route);
    assert!(!backend.terminal);
    assert!(backend.peer_launch_retains.is_empty());
    f.clean();
}

#[test]
fn completed_deferred_result_preparation_rejection_keeps_original_result_reusable() {
    let mut f = Fixture::new();
    let original_count = f.context.backend().submissions.len();
    f.context.backend_mut_for_test_v1().children[0].compute_completion_reservations =
        MAX_RUNTIME_SUBMISSIONS_V1;
    assert!(
        f.context
            .launch_producer_aware_v1(
                f.consumer_stream,
                &f.kernel,
                &Arguments(RuntimeMemoryRegionV1 {
                    allocation: f.allocation,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 64
                }),
                geometry(),
                &[f.event.unwrap()],
            )
            .is_err()
    );
    assert_eq!(f.context.backend().submissions.len(), original_count);
    assert!(f.context.backend().peer_launch_retains.is_empty());
    f.context.backend_mut_for_test_v1().children[0].compute_completion_reservations = 0;
    let mut consumer = f.launch();
    assert_eq!(
        f.context.cancel(&mut consumer).unwrap(),
        crate::RuntimeCancellationV1::Cancelled
    );
    f.context.release_submission(consumer).unwrap();
    f.clean();
}

#[test]
fn completed_deferred_result_terminal_publication_keeps_independent_result_custody() {
    use crate::kfd_backend::materialized_publication::ScriptedMaterializedPublicationFaultV1 as Fault;
    for unwind in [false, true] {
        let mut f = Fixture::new();
        let mut consumer = f.launch();
        let id = f.context.backend_submission_for_test_v1(&consumer).unwrap();
        let RoutedSubmissionV1::Native { route, .. } = f.context.backend().submissions[&id] else {
            unreachable!()
        };
        assert!(matches!(
            f.context.backend().children[route.child]
                .active_compute_submission_v1(route.local)
                .unwrap()
                .execution,
            Some(ActiveComputeExecutionV1::MaterializedPrepared(_))
        ));
        f.context.release_event(f.event.take().unwrap()).unwrap();
        f.context.backend_mut_for_test_v1().children[0].scripted_materialized_publication_fault =
            Some(if unwind {
                Fault::OuterUnwindAfterPublish
            } else {
                Fault::SubmitTerminal
            });
        f.context.flush_stream(f.consumer_stream).unwrap();
        assert!(
            f.context.backend().children[0]
                .scripted_materialized_publication_fault
                .is_some()
        );
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.context.poll(&mut consumer)
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert!(f.context.backend().children[0].terminal);
        assert!(
            f.context.backend().children[0]
                .scripted_materialized_publication_fault
                .is_none()
        );
        if !unwind {
            assert!(f.context.backend().terminal);
        }
        assert!(
            f.context
                .backend()
                .peer_launch_retains
                .retains(f.producer_id)
        );
        assert!(f.context.backend().submissions.contains_key(&id));
        assert!(f.context.backend().submissions.contains_key(&f.producer_id));
        // Leave the handle unreleased; the backend retains terminal custody.
    }
}

#[test]
fn completed_deferred_result_consumer_cancellation_refunds_only_consumer_retains() {
    let mut f = Fixture::new();
    let mut consumer = f.launch();
    assert!(
        f.context
            .backend()
            .peer_launch_retains
            .retains(f.producer_id)
    );
    assert_eq!(
        f.context.cancel(&mut consumer).unwrap(),
        crate::RuntimeCancellationV1::Cancelled
    );
    assert!(
        !f.context
            .backend()
            .peer_launch_retains
            .retains(f.producer_id)
    );
    f.context.release_submission(consumer).unwrap();
    // The original completed result and its independent public event remain reusable.
    let mut consumer = f.launch();
    assert_eq!(
        f.context.cancel(&mut consumer).unwrap(),
        crate::RuntimeCancellationV1::Cancelled
    );
    f.context.release_submission(consumer).unwrap();
    f.clean();
}

#[test]
fn completed_deferred_result_typed_write_settlement_admits_read_without_readback() {
    let mut f = Fixture::with_producer_access(RuntimeAccessV1::Write);
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    let dirty = f.context.backend().children[0].native_dirty_extents;
    assert_eq!(dirty, 1);
    let mut consumer = f.launch();
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    assert!(
        f.context
            .backend()
            .peer_launch_retains
            .retains(f.producer_id)
    );
    f.context.release_event(f.event.take().unwrap()).unwrap();
    assert_eq!(
        f.context.cancel(&mut consumer).unwrap(),
        crate::RuntimeCancellationV1::Cancelled
    );
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert!(
        !f.context
            .backend()
            .peer_launch_retains
            .retains(f.producer_id)
    );
    assert_eq!(f.context.backend().children[0].native_dirty_extents, dirty);
    f.context.release_submission(consumer).unwrap();

    // The scripted completion models writer settlement, not produced bytes.
    // Discard its synthetic writeback metadata only after all assertions; no
    // GPU readback or native writeback cleanup is claimed by this test.
    let child = &mut f.context.backend_mut_for_test_v1().children[0];
    for record in child.allocations.values_mut() {
        record.native_dirty.clear();
    }
    child.native_dirty_extents = 0;
    child.recycled_dispatch = None;
    f.clean();
}
