//! Journal-enabled Context over scripted native owners, not kernel/GPU execution.

use super::*;
use crate::{RuntimeCancellationV1, RuntimeCompletionStatusV1, RuntimePollV1};
use std::mem::ManuallyDrop;

type Submission = crate::RuntimeSubmissionV1<ThreeBindingCandidateContextArgumentsV1>;

struct Fixture {
    context: ManuallyDrop<crate::RuntimeContextV1<KfdRuntimeBackendV1>>,
    streams: [crate::RuntimeStreamIdV1; 3],
    allocations: [crate::RuntimeAllocationIdV1; 5],
    native_allocations: [u64; 5],
    module: crate::RuntimeModuleIdV1,
    kernel: crate::TypedRuntimeKernelV1<ThreeBindingCandidateContextArgumentsV1>,
}

impl Fixture {
    fn new(cross_stream: bool) -> Self {
        let steps = (0..5).flat_map(|_| {
            [
                ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ]
        });
        let (mut context, stream, allocations, native_allocations) =
            scripted_persistent_context_with_steps_v1::<5>(64, steps, true);
        let device = context.devices()[0].id();
        let streams = if cross_stream {
            [
                stream,
                context.create_stream(device).unwrap(),
                context.create_stream(device).unwrap(),
            ]
        } else {
            [stream; 3]
        };
        let module = context
            .load_module(device, &synthetic_cov6::three_binding_module())
            .unwrap();
        let kernel = context
            .resolve_kernel::<ThreeBindingCandidateContextArgumentsV1>(module, "vecadd")
            .unwrap();
        Self {
            context: ManuallyDrop::new(context),
            streams,
            allocations,
            native_allocations,
            module,
            kernel,
        }
    }

    fn launch(&mut self, stage: usize, events: &[crate::RuntimeEventIdV1]) -> (Submission, u64) {
        let [a, b, x, d, y] = self.allocations;
        let args = ThreeBindingCandidateContextArgumentsV1 {
            allocations: [[a, b, x], [a, d, x], [x, b, y]][stage],
            byte_offsets: [0; 3],
            byte_lens: [64; 3],
            accesses: [
                RuntimeAccessV1::Read,
                RuntimeAccessV1::Read,
                RuntimeAccessV1::Write,
            ],
        };
        let submission = self
            .context
            .launch_producer_aware_v1(
                self.streams[stage],
                &self.kernel,
                &args,
                crate::RuntimeLaunchGeometryV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                events,
            )
            .unwrap();
        let backend = self.context.backend();
        let native = backend
            .pending_compute
            .keys()
            .chain(backend.submissions.keys())
            .copied()
            .chain(backend.active.iter().map(|active| active.id))
            .max()
            .unwrap();
        (submission, native)
    }

    fn active_bytes(&mut self, native: u64, binding: usize) -> &mut [u8] {
        let backend = self.context.backend_mut_for_test_v1();
        let active = backend.active.as_mut().expect("published fixture launch");
        assert_eq!(active.id, native);
        let Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { devices, .. }) =
            active.execution.as_mut()
        else {
            panic!("exact persistent scripted path required");
        };
        devices[binding].scripted_bytes_mut().unwrap()
    }

    fn active_owner_id(&self, native: u64, binding: usize) -> u64 {
        let active = self.context.backend().active.as_ref().unwrap();
        assert_eq!(active.id, native);
        let Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { devices, .. }) =
            active.execution.as_ref()
        else {
            panic!("exact persistent scripted path required");
        };
        devices[binding].scripted_owner_id().unwrap()
    }

    fn finish_native(&mut self, native: u64) {
        self.context
            .backend_mut_for_test_v1()
            .scripted_persistent_poll_pending_observations = 0;
        assert_eq!(
            self.context
                .backend_mut_for_test_v1()
                .poll_v1(native)
                .unwrap(),
            BackendPollV1::Succeeded
        );
    }

    fn reconcile(&mut self, submission: &mut Submission) {
        for _ in 0..16 {
            if self.context.poll(submission).unwrap() == RuntimePollV1::Succeeded {
                return;
            }
        }
        panic!("bounded Context reconciliation did not finish");
    }

    fn assert_owners(&self, count: usize) {
        let driver = self.context.backend().scripted_sdma.as_ref().unwrap();
        assert_eq!(driver.live_owner_count(), count);
        assert_eq!(driver.unexpected_drops(), 0);
    }

    fn finish(mut self, submissions: [Submission; 3], unknown_writers: usize) {
        for submission in submissions.into_iter().rev() {
            self.context.release_submission(submission).unwrap();
        }
        let backend = self.context.backend();
        assert!(backend.compute_dependency_retain_counts.is_empty());
        assert!(backend.compute_module_retain_counts.is_empty());
        assert!(backend.allocation_custody.is_empty());
        assert_eq!(backend.compute_completion_reservations, 0);
        assert_runtime_compute_pipeline_empty_v1(backend);
        assert_eq!(self.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(
            self.context.version_journal_writer_records_v1(),
            Some(unknown_writers)
        );
        self.context.unload_module(self.module).unwrap();
        // Scripted release exercises owner demotion/recycle, without native sync.
        for record in self
            .context
            .backend_mut_for_test_v1()
            .allocations
            .values_mut()
        {
            record.sdma_backed = false;
        }
        for allocation in self.allocations {
            self.context.release_allocation(allocation).unwrap();
        }
        assert_eq!(self.context.version_journal_writer_records_v1(), Some(0));
        let usage = self
            .context
            .allocation_admission_usage_v1(self.context.devices()[0].id())
            .unwrap()
            .unwrap();
        assert_eq!(usage.used, crate::RuntimeResourceVectorV1::ZERO);
        assert_eq!(usage.reserved_records, 0);
        assert_eq!(usage.retained_records, 0);
        assert_eq!(usage.quarantined_records, 0);
        assert!(!usage.poisoned);
        self.assert_owners(0);
        assert!(
            self.context
                .backend()
                .scripted_sdma
                .as_ref()
                .unwrap()
                .is_exhausted()
        );
        let mut streams = self.streams.to_vec();
        streams.sort_unstable();
        streams.dedup();
        for stream in streams {
            self.context.destroy_stream(stream).unwrap();
        }
        let mut backend = ManuallyDrop::into_inner(self.context).shutdown().unwrap();
        backend.shutdown_native_v1().unwrap();
    }
}

#[test]
fn queued_native_context_restores_exact_overwrite_before_downstream_read() {
    for cross_stream in [false, true] {
        let mut f = Fixture::new(cross_stream);
        let (a, ai) = f.launch(0, &[]);
        f.context.flush_stream(f.streams[0]).unwrap();
        let x_owner = f.active_owner_id(ai, 2);
        f.active_bytes(ai, 2).fill(0xa1);
        let ae = f.context.record_event(&a).unwrap();
        let (b, bi) = f.launch(1, &[ae]);
        let be = f.context.record_event(&b).unwrap();
        let (mut c, ci) = f.launch(2, &[be]);
        let backend = f.context.backend();
        assert_eq!(
            &*backend.pending_compute[&ci].explicit_success_dependencies,
            &[bi]
        );
        assert_eq!(
            &*backend.pending_compute[&ci].quiescence_dependencies,
            &[ai]
        );
        assert_eq!(backend.pending_compute[&ci].dependency_depth, 3);
        assert_eq!(backend.compute_dependency_retain_counts[&ai], 2);
        assert_eq!(backend.compute_dependency_retain_counts[&bi], 1);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(6));
        for event in [ae, be] {
            f.context.release_event(event).unwrap();
        }
        f.assert_owners(5);
        f.finish_native(ai);
        f.context.flush_stream(f.streams[1]).unwrap();
        assert_eq!(f.active_owner_id(bi, 2), x_owner);
        // The scripted backend does not execute arithmetic: inject distinguishable
        // completion bytes and inspect the actual persistent owner, not its shadow.
        f.active_bytes(bi, 2).fill(0xb2);
        f.finish_native(bi);
        f.context.flush_stream(f.streams[2]).unwrap();
        assert_eq!(f.active_owner_id(ci, 0), x_owner);
        assert_eq!(f.active_bytes(ci, 0), &[0xb2; 64]);
        f.active_bytes(ci, 2).fill(0xc3);
        f.finish_native(ci);
        for submission in [&a, &b, &c] {
            assert_eq!(
                f.context.query_submission(submission).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
        }
        assert_eq!(f.context.version_journal_read_records_v1(), Some(6));
        assert!(matches!(
            f.context.release_allocation(f.allocations[2]),
            Err(crate::RuntimeErrorV1::Validation(
                crate::RuntimeValidationErrorV1::ContextReserved
            ))
        ));
        f.reconcile(&mut c);
        for submission in [&a, &b, &c] {
            assert_eq!(
                f.context.query_submission(submission).unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
        }
        let backend = f.context.backend();
        assert!(backend.allocations[&f.native_allocations[2]].sdma_shadow_dirty);
        assert_eq!(
            backend.allocations[&f.native_allocations[2]].content_sha256,
            None
        );
        let performance = backend.last_launch_performance_v1().unwrap();
        assert_eq!(
            performance.data_path(),
            KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
        );
        assert_eq!(performance.user_data_materializations(), 0);
        f.finish([a, b, c], 0);
    }
}

#[test]
fn queued_native_context_cancellation_preserves_ancestor_and_never_reparents_reader() {
    for cancel_middle in [false, true] {
        let mut f = Fixture::new(true);
        let (mut a, ai) = f.launch(0, &[]);
        f.context.flush_stream(f.streams[0]).unwrap();
        let ae = f.context.record_event(&a).unwrap();
        let (mut b, bi) = f.launch(1, &[ae]);
        let be = f.context.record_event(&b).unwrap();
        let (mut c, ci) = f.launch(2, &[be]);
        for event in [ae, be] {
            f.context.release_event(event).unwrap();
        }
        f.context
            .backend_mut_for_test_v1()
            .scripted_persistent_poll_pending_observations = 64;
        if cancel_middle {
            assert_eq!(
                f.context.cancel(&mut b).unwrap(),
                RuntimeCancellationV1::Cancelled
            );
            assert!(matches!(
                f.context.poll(&mut c).unwrap(),
                RuntimePollV1::Failed { .. }
            ));
            assert!(
                !f.context
                    .backend()
                    .compute_dependency_retain_counts
                    .contains_key(&ai)
            );
        } else {
            assert_eq!(
                f.context.cancel(&mut c).unwrap(),
                RuntimeCancellationV1::Cancelled
            );
            assert_eq!(f.context.backend().compute_dependency_retain_counts[&ai], 1);
            assert!(
                !f.context
                    .backend()
                    .compute_dependency_retain_counts
                    .contains_key(&bi)
            );
        }
        assert_eq!(f.context.backend().active.as_ref().unwrap().id, ai);
        assert!(
            !f.context
                .backend()
                .submissions
                .get(&ci)
                .is_some_and(|record| record.profile_dispatch_published)
        );
        f.assert_owners(5);
        f.finish_native(ai);
        f.reconcile(&mut a);
        if !cancel_middle {
            f.context.flush_stream(f.streams[1]).unwrap();
            f.finish_native(bi);
            f.reconcile(&mut b);
        }
        f.finish([a, b, c], usize::from(cancel_middle));
    }
}

#[test]
fn queued_native_context_interior_fifo_cancel_is_too_late_until_reader_is_cancelled() {
    let mut f = Fixture::new(false);
    let (mut a, ai) = f.launch(0, &[]);
    f.context.flush_stream(f.streams[0]).unwrap();
    let ae = f.context.record_event(&a).unwrap();
    let (mut b, bi) = f.launch(1, &[ae]);
    let be = f.context.record_event(&b).unwrap();
    let (mut c, _) = f.launch(2, &[be]);
    for event in [ae, be] {
        f.context.release_event(event).unwrap();
    }
    let owners = f.native_allocations.map(|id| {
        f.context
            .backend()
            .allocation_custody
            .get(&id)
            .map(|row| row.owners.clone())
    });
    let retains = f.context.backend().compute_dependency_retain_counts.clone();
    assert_eq!(
        f.context.cancel(&mut b).unwrap(),
        RuntimeCancellationV1::TooLate
    );
    assert_eq!(f.context.version_journal_read_records_v1(), Some(6));
    assert_eq!(
        f.context.backend().compute_dependency_retain_counts,
        retains
    );
    assert_eq!(
        f.native_allocations.map(|id| f
            .context
            .backend()
            .allocation_custody
            .get(&id)
            .map(|row| row.owners.clone())),
        owners
    );
    assert_eq!(
        f.context.cancel(&mut c).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(f.context.version_journal_read_records_v1(), Some(4));
    assert_eq!(f.context.backend().compute_dependency_retain_counts[&ai], 1);
    assert!(
        !f.context
            .backend()
            .compute_dependency_retain_counts
            .contains_key(&bi)
    );
    assert_eq!(
        f.context.cancel(&mut b).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    f.finish_native(ai);
    f.reconcile(&mut a);
    f.finish([a, b, c], 0);
}
