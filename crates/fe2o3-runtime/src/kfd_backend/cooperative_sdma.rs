//! Resumable DeviceLocal leaves of the router's read-all/write-all staging copy.
//!
//! Private handles use the child's ordinary async ledger, never public routes.
//! Scratch is phase-local. Quiescent cleanup failures freeze the outer copy and
//! retain only disposal custody until submission release succeeds.

use super::*;
use fe2o3_kfd::Gfx942RetainedRequestV1;

type Failure = RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LeafStepV1 {
    Allocate,
    Stream,
    Submit,
    Observe,
    Readback,
    Retire,
    Cleanup(CooperativeCopyPhaseV1),
}

pub(super) struct CooperativeSdmaLeafV1 {
    endpoint: RoutedHandleV1,
    allocation: Option<u64>,
    stream: Option<u64>,
    submission: Option<u64>,
    credit: Option<Gfx942RetainedRequestV1>,
    step: LeafStepV1,
}

impl fmt::Debug for CooperativeSdmaLeafV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CooperativeSdmaLeafV1")
            .field("endpoint", &self.endpoint)
            .field("allocation", &self.allocation)
            .field("stream", &self.stream)
            .field("submission", &self.submission)
            .field("request_accounted", &self.credit.is_some())
            .field("step", &self.step)
            .finish()
    }
}

enum LeafProgressV1 {
    Pending,
    Changed,
    Cleaned(CooperativeCopyPhaseV1),
    CleanupFailed(KfdRuntimeBackendErrorV1),
}

impl CooperativeSdmaLeafV1 {
    pub(super) fn is_quiescent(&self, child: &KfdRuntimeBackendV1) -> bool {
        matches!(self.step, LeafStepV1::Cleanup(_))
            && self.submission.is_none_or(|submission| {
                !child.active_sdma.contains_key(&submission)
                    && child
                        .submissions
                        .get(&submission)
                        .is_some_and(|record| record.status != BackendPollV1::Pending)
            })
    }

    pub(super) fn child(&self) -> usize {
        self.endpoint.child
    }

    fn allocate(&mut self, child: &mut KfdRuntimeBackendV1, byte_len: u64) -> Result<(), Failure> {
        if let Some(binding) = child.request_binding_v1()? {
            let reservation = binding.account().reserve_v1(byte_len).map_err(|error| {
                let error = native_budget::rooted_host_backing_admission_error_v1(error);
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity {
                    RuntimeBackendFailureV1::Rejected(error)
                } else {
                    child.poison_terminal_v1();
                    RuntimeBackendFailureV1::Terminal(error)
                }
            })?;
            self.credit = Some(reservation.retain());
        }
        let result = child.allocate_request_backing_v1(
            child.description.backend_device,
            RuntimeMemoryKindV1::HostVisible,
            byte_len,
            HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
        );
        match result {
            Ok(RuntimeBackendAllocationOutcomeV1::Allocated(allocation)) => {
                self.allocation = Some(allocation);
                Ok(())
            }
            Ok(RuntimeBackendAllocationOutcomeV1::SettledNoOwner(error)) => {
                self.refund(child, false)?;
                Err(RuntimeBackendFailureV1::Quiescent(error))
            }
            Err(failure @ RuntimeBackendFailureV1::Rejected(_)) => {
                self.refund(child, true)?;
                Err(failure)
            }
            Err(RuntimeBackendFailureV1::Quiescent(_)) => Err(child
                .terminal_error("cooperative scratch allocation has uncertain backing custody")),
            Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => Err(failure),
        }
    }

    fn refund(&mut self, child: &mut KfdRuntimeBackendV1, rejected: bool) -> Result<(), Failure> {
        if let Some(credit) = self.credit.take() {
            let result = if rejected {
                credit.release_after_rejection()
            } else {
                credit.release_after_disposal()
            };
            if result.is_err() {
                return Err(child.terminal_error("cooperative scratch credit settlement failed"));
            }
        }
        Ok(())
    }

    fn cleanup(&mut self, child: &mut KfdRuntimeBackendV1) -> Result<bool, Failure> {
        // Keep each handle installed until its own disposal has succeeded.
        if let Some(submission) = self.submission {
            child.release_submission_v1(submission)?;
            self.submission = None;
        } else if let Some(allocation) = self.allocation {
            child.release_allocation_v1(allocation)?;
            self.allocation = None;
            self.refund(child, false)?;
        } else if let Some(stream) = self.stream {
            child.destroy_stream_v1(stream)?;
            self.stream = None;
        } else {
            assert!(self.credit.is_none(), "scratch credit outlived its backing");
            return Ok(true);
        }
        Ok(false)
    }
}

fn progress_leaf_v1(
    child: &mut KfdRuntimeBackendV1,
    copy: &mut CooperativeCopySubmissionV1,
) -> Result<LeafProgressV1, Failure> {
    let leaf = copy
        .sdma_leaf
        .as_mut()
        .expect("selected SDMA leaf is rooted");
    if let LeafStepV1::Cleanup(next) = leaf.step {
        return match leaf.cleanup(child) {
            Ok(true) => Ok(LeafProgressV1::Cleaned(next)),
            Ok(false) => Ok(LeafProgressV1::Changed),
            // Freeze the operation as failed. Retry residual scratch disposal
            // only through submission release, never by resuming the copy.
            Err(
                RuntimeBackendFailureV1::Rejected(error)
                | RuntimeBackendFailureV1::Quiescent(error),
            ) => {
                leaf.step = LeafStepV1::Cleanup(CooperativeCopyPhaseV1::Failed);
                Ok(LeafProgressV1::CleanupFailed(error))
            }
            Err(failure) => Err(failure),
        };
    }
    let start = copy.byte_cursor;
    let end = start
        .saturating_add(COOPERATIVE_COPY_CHUNK_BYTES_V1)
        .min(copy.staging.len());
    let reading = copy.phase == CooperativeCopyPhaseV1::Read;
    let result = (|| match leaf.step {
        LeafStepV1::Allocate => leaf.allocate(child, copy.scratch_byte_len).map(|()| {
            leaf.step = LeafStepV1::Stream;
            LeafProgressV1::Changed
        }),
        LeafStepV1::Stream => child
            .create_stream_v1(child.description.backend_device)
            .map(|stream| {
                leaf.stream = Some(stream);
                leaf.step = LeafStepV1::Submit;
                LeafProgressV1::Changed
            }),
        LeafStepV1::Submit => {
            let scratch = leaf.allocation.expect("prepared leaf has scratch");
            let stream = leaf.stream.expect("prepared leaf has a private stream");
            if !child.allocations[&leaf.endpoint.local]
                .native_dirty
                .is_empty()
            {
                return Err(
                    child.terminal_error("retained cooperative endpoint became native-dirty")
                );
            }
            if !reading {
                child.write_allocation_v1(scratch, 0, &copy.staging[start..end])?;
            }
            let region = if reading {
                copy.source_region
            } else {
                copy.destination_region
            };
            let device_region = BackendMemoryRegionV1 {
                allocation: leaf.endpoint.local,
                byte_offset: region.byte_offset + start as u64,
                byte_len: (end - start) as u64,
                access: if reading {
                    RuntimeAccessV1::Read
                } else {
                    RuntimeAccessV1::Write
                },
            };
            let scratch_region = BackendMemoryRegionV1 {
                allocation: scratch,
                byte_offset: 0,
                byte_len: (end - start) as u64,
                access: if reading {
                    RuntimeAccessV1::Write
                } else {
                    RuntimeAccessV1::Read
                },
            };
            let (source, destination) = if reading {
                (device_region, scratch_region)
            } else {
                (scratch_region, device_region)
            };
            match child.copy_async_v1(stream, source, destination, &[]) {
                Ok(submission) => {
                    leaf.submission = Some(submission);
                    leaf.step = LeafStepV1::Observe;
                    Ok(LeafProgressV1::Changed)
                }
                Err(RuntimeBackendFailureV1::Quiescent(_)) => Err(child.terminal_error(
                    "cooperative child admission returned no submission settlement handle",
                )),
                Err(failure) => Err(failure),
            }
        }
        LeafStepV1::Observe => {
            let submission = leaf.submission.expect("observed leaf has child submission");
            // An accepted copy may still be Ready because of compute coexistence.
            // Drive only this exact private head, and never reconcile native-dirty data.
            if child
                .active_sdma
                .get(&submission)
                .is_some_and(|active| matches!(active.phase, ActiveSdmaPhaseV1::Ready))
            {
                let active = &child.active_sdma[&submission];
                if Some(active.stream) != leaf.stream
                    || !active.dependencies.is_empty()
                    || [active.source, active.destination]
                        .into_iter()
                        .any(|allocation| !child.allocations[&allocation].native_dirty.is_empty())
                {
                    return Err(
                        child.terminal_error("private cooperative SDMA head changed authority")
                    );
                }
                let active = child.active_sdma.remove(&submission).unwrap();
                child.progress_unpublished_sdma_copy_v1(active)?;
                return Ok(
                    if child
                        .active_sdma
                        .get(&submission)
                        .is_some_and(|active| matches!(active.phase, ActiveSdmaPhaseV1::Ready))
                    {
                        LeafProgressV1::Pending
                    } else {
                        LeafProgressV1::Changed
                    },
                );
            }
            child.poll_v1(submission).map(|status| match status {
                BackendPollV1::Pending => LeafProgressV1::Pending,
                BackendPollV1::Succeeded => {
                    leaf.step = if reading {
                        LeafStepV1::Readback
                    } else {
                        LeafStepV1::Retire
                    };
                    LeafProgressV1::Changed
                }
                BackendPollV1::Failed { .. } => {
                    leaf.step = LeafStepV1::Cleanup(CooperativeCopyPhaseV1::Failed);
                    LeafProgressV1::Changed
                }
            })
        }
        LeafStepV1::Readback => child
            .read_allocation_v1(
                leaf.allocation.expect("readback scratch remains rooted"),
                0,
                &mut copy.staging[start..end],
            )
            .map(|()| {
                leaf.step = LeafStepV1::Retire;
                LeafProgressV1::Changed
            }),
        LeafStepV1::Retire => child
            .release_submission_v1(leaf.submission.expect("completed leaf remains rooted"))
            .map(|()| {
                leaf.submission = None;
                copy.byte_cursor = end;
                leaf.step = if end == copy.staging.len() {
                    LeafStepV1::Cleanup(if reading {
                        CooperativeCopyPhaseV1::Write
                    } else {
                        CooperativeCopyPhaseV1::Succeeded
                    })
                } else {
                    LeafStepV1::Submit
                };
                LeafProgressV1::Changed
            }),
        LeafStepV1::Cleanup(_) => unreachable!(),
    })();
    match result {
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy =>
        {
            Ok(LeafProgressV1::Pending)
        }
        Err(RuntimeBackendFailureV1::Rejected(_)) | Err(RuntimeBackendFailureV1::Quiescent(_)) => {
            // Only conclusive child records may enter cleanup. Poll failures can
            // otherwise leave a published operation live despite the error class.
            if leaf
                .submission
                .is_some_and(|submission| !child.submissions.contains_key(&submission))
            {
                return Err(
                    child.terminal_error("cooperative child failure retained unsettled DMA")
                );
            }
            leaf.step = LeafStepV1::Cleanup(CooperativeCopyPhaseV1::Failed);
            Ok(LeafProgressV1::Changed)
        }
        result => result,
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(super) fn fail_cooperative_dependency_path_v1(
        &mut self,
        requested: u64,
        failed: u64,
    ) -> Result<(), Failure> {
        let mut current = requested;
        for _ in 0..MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            if current == failed {
                return Ok(());
            }
            let predecessor = match self.submissions.get(&current) {
                Some(RoutedSubmissionV1::CooperativeCopy(copy))
                    if copy.phase == CooperativeCopyPhaseV1::Dependencies
                        && copy.sdma_leaf.is_none() =>
                {
                    copy.dependencies
                        .get(copy.dependency_cursor)
                        .copied()
                        .filter(|next| *next < current)
                }
                _ => None,
            };
            let Some(predecessor) = predecessor else {
                break;
            };
            self.fail_cooperative_copy(current);
            current = predecessor;
        }
        self.terminal = true;
        Err(RuntimeBackendFailureV1::Terminal(
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Terminal,
                "cooperative failure path changed its retained dependency authority",
            ),
        ))
    }

    pub(super) fn cooperative_sdma_leaf_is_selected_v1(&self, submission: u64) -> bool {
        let RoutedSubmissionV1::CooperativeCopy(copy) = &self.submissions[&submission] else {
            unreachable!()
        };
        if copy.sdma_leaf.is_some() {
            return true;
        }
        let route = if copy.phase == CooperativeCopyPhaseV1::Read {
            copy.source
        } else {
            copy.destination
        };
        let child = &self.children[route.child];
        let record = &child.allocations[&route.local];
        child.native_available
            && record.kind == RuntimeMemoryKindV1::DeviceLocal
            && record.native_dirty.is_empty()
    }

    fn with_cooperative_leaf_v1<T>(
        &mut self,
        submission: u64,
        operation: impl FnOnce(
            &mut KfdRuntimeBackendV1,
            &mut CooperativeCopySubmissionV1,
        ) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            self.submissions.get_mut(&submission).unwrap()
        else {
            unreachable!()
        };
        let child_index = copy
            .sdma_leaf
            .as_ref()
            .expect("private leaf is installed")
            .endpoint
            .child;
        let child = &mut self.children[child_index];
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(child, copy)));
        match result {
            Ok(result @ Err(RuntimeBackendFailureV1::Terminal(_))) => {
                self.terminal = true;
                if let Some(credit) = copy.sdma_leaf.as_mut().and_then(|leaf| leaf.credit.take()) {
                    credit.quarantine();
                }
                child.poison_terminal_v1();
                result
            }
            Ok(result) => result,
            Err(payload) => {
                self.terminal = true;
                sdma_host_write::resume_sdma_owner_panic_v1(payload, || {
                    if let Some(credit) =
                        copy.sdma_leaf.as_mut().and_then(|leaf| leaf.credit.take())
                    {
                        credit.quarantine();
                    }
                    child.poison_terminal_v1();
                })
            }
        }
    }

    pub(super) fn progress_cooperative_sdma_leaf_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, Failure> {
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            self.submissions.get_mut(&submission).unwrap()
        else {
            unreachable!()
        };
        if copy.sdma_leaf.is_none() {
            assert!(
                copy.scratch_byte_len != 0,
                "native copy reserved scratch capacity"
            );
            copy.sdma_leaf = Some(CooperativeSdmaLeafV1 {
                endpoint: if copy.phase == CooperativeCopyPhaseV1::Read {
                    copy.source
                } else {
                    copy.destination
                },
                allocation: None,
                stream: None,
                submission: None,
                credit: None,
                step: LeafStepV1::Allocate,
            });
            self.note_cooperative_progress();
            return Ok(BackendPollV1::Pending);
        }
        match self.with_cooperative_leaf_v1(submission, progress_leaf_v1)? {
            LeafProgressV1::Pending => {}
            LeafProgressV1::Changed => self.note_cooperative_progress(),
            LeafProgressV1::CleanupFailed(error) => {
                self.finish_cooperative_copy(submission, CooperativeCopyPhaseV1::Failed);
                return Err(RuntimeBackendFailureV1::Quiescent(error));
            }
            LeafProgressV1::Cleaned(next) => {
                let RoutedSubmissionV1::CooperativeCopy(copy) =
                    self.submissions.get_mut(&submission).unwrap()
                else {
                    unreachable!()
                };
                copy.sdma_leaf = None;
                if next != CooperativeCopyPhaseV1::Write {
                    return Ok(self.finish_cooperative_copy(submission, next));
                }
                copy.phase = next;
                copy.byte_cursor = 0;
                self.note_cooperative_progress();
            }
        }
        Ok(BackendPollV1::Pending)
    }

    pub(super) fn cancel_cooperative_sdma_leaf_v1(
        &mut self,
        submission: u64,
    ) -> Result<bool, Failure> {
        let RoutedSubmissionV1::CooperativeCopy(copy) = &self.submissions[&submission] else {
            unreachable!()
        };
        if copy.sdma_leaf.is_none() {
            return Ok(true);
        }
        let cancelled = self.with_cooperative_leaf_v1(submission, |child, copy| {
            let leaf = copy.sdma_leaf.as_mut().unwrap();
            match leaf.step {
                LeafStepV1::Cleanup(CooperativeCopyPhaseV1::Failed) => {
                    return Err(KfdRuntimeBackendV1::quiescent_error(
                        KfdRuntimeBackendErrorKindV1::Native,
                        "cooperative copy failed before cancellation",
                    ));
                }
                LeafStepV1::Observe
                    if child.cancel_v1(leaf.submission.unwrap())?
                        != crate::BackendCancellationV1::Cancelled =>
                {
                    return Ok(false);
                }
                LeafStepV1::Retire if copy.phase == CooperativeCopyPhaseV1::Write => {
                    return Ok(false);
                }
                LeafStepV1::Cleanup(next)
                    if !matches!(
                        next,
                        CooperativeCopyPhaseV1::Write | CooperativeCopyPhaseV1::Cancelled
                    ) =>
                {
                    return Ok(false);
                }
                _ => {}
            }
            leaf.step = LeafStepV1::Cleanup(CooperativeCopyPhaseV1::Cancelled);
            // At most one retired submission, one HostVisible allocation and
            // one logical stream; cleanup never waits for a DMA completion.
            for _ in 0..4 {
                match leaf.cleanup(child) {
                    Ok(true) => return Ok(true),
                    Ok(false) => {}
                    Err(
                        RuntimeBackendFailureV1::Rejected(error)
                        | RuntimeBackendFailureV1::Quiescent(error),
                    ) => {
                        leaf.step = LeafStepV1::Cleanup(CooperativeCopyPhaseV1::Failed);
                        return Err(RuntimeBackendFailureV1::Quiescent(error));
                    }
                    Err(failure) => return Err(failure),
                }
            }
            unreachable!("bounded private leaf cleanup exceeded its three owners")
        });
        let cancelled = match cancelled {
            Err(failure @ RuntimeBackendFailureV1::Quiescent(_)) => {
                self.finish_cooperative_copy(submission, CooperativeCopyPhaseV1::Failed);
                return Err(failure);
            }
            result => result?,
        };
        if cancelled {
            let RoutedSubmissionV1::CooperativeCopy(copy) =
                self.submissions.get_mut(&submission).unwrap()
            else {
                unreachable!()
            };
            copy.sdma_leaf = None;
        }
        Ok(cancelled)
    }

    pub(super) fn release_cooperative_sdma_leaf_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), Failure> {
        let RoutedSubmissionV1::CooperativeCopy(copy) = &self.submissions[&submission] else {
            unreachable!()
        };
        if copy.sdma_leaf.is_none() {
            return Ok(());
        }
        assert!(
            copy.is_quiescent(),
            "pending copy cannot release private scratch"
        );
        self.with_cooperative_leaf_v1(submission, |child, copy| {
            let leaf = copy.sdma_leaf.as_mut().unwrap();
            for _ in 0..4 {
                if leaf.cleanup(child)? {
                    return Ok(());
                }
            }
            unreachable!("bounded residual cleanup exceeded its three owners")
        })?;
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            self.submissions.get_mut(&submission).unwrap()
        else {
            unreachable!()
        };
        copy.sdma_leaf = None;
        self.cooperative_staging_bytes = self
            .cooperative_staging_bytes
            .checked_sub(copy.scratch_byte_len)
            .expect("residual scratch stays accounted");
        copy.scratch_byte_len = 0;
        Ok(())
    }

    pub(super) fn restore_cooperative_stream_tail_v1(&mut self, submission: u64) {
        let RoutedSubmissionV1::CooperativeCopy(copy) = &self.submissions[&submission] else {
            unreachable!()
        };
        if self.cooperative_stream_tails.get(&copy.stream) == Some(&submission) {
            match copy.prior_stream_submission {
                Some(prior) => {
                    self.cooperative_stream_tails.insert(copy.stream, prior);
                }
                None => {
                    self.cooperative_stream_tails.remove(&copy.stream);
                }
            }
        }
    }

    pub(super) fn cooperative_native_custody_is_ordered_v1(
        &self,
        route: RoutedHandleV1,
        stream: u64,
        dependencies: &HashSet<u64>,
    ) -> bool {
        let Some(custody) = self.children[route.child]
            .allocation_custody
            .get(&route.local)
        else {
            return true;
        };
        custody.owners.iter().all(|owner| {
            owner.kind == RuntimeAllocationCustodyKindV1::Sdma
                && self.cooperative_allocation_owners.get(&route).is_some_and(|parents| {
                    parents.iter().any(|parent| {
                        matches!(self.submissions.get(parent), Some(RoutedSubmissionV1::CooperativeCopy(copy))
                            if (copy.stream == stream || dependencies.contains(parent))
                                && copy.sdma_leaf.as_ref().is_some_and(|leaf| {
                                    leaf.endpoint == route && leaf.submission == Some(owner.submission)
                                        && leaf.stream == Some(owner.stream)
                                }))
                    })
                })
        })
    }
}
