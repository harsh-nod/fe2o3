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
    Reconcile,
    Submit,
    Observe,
    Readback,
    Retire,
    Cleanup(CooperativeCopyPhaseV1),
}

pub(super) struct CooperativeSdmaLeafV1 {
    origin: Option<PeerCopyOriginV1>,
    endpoint: RoutedHandleV1,
    allocation: Option<u64>,
    stream: Option<u64>,
    submission: Option<u64>,
    reconciliation: Option<u64>,
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
            .field("reconciliation", &self.reconciliation)
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
    pub(super) fn authenticates_compute_predecessor_v1(
        &self,
        copy: &CooperativeCopySubmissionV1,
        child: &KfdRuntimeBackendV1,
        read_origin: Option<PeerCopyOriginV1>,
        write_origin: Option<PeerCopyOriginV1>,
    ) -> bool {
        let reading = copy.phase == CooperativeCopyPhaseV1::Read;
        let (endpoint, region, base) = if reading {
            (copy.source, copy.source_region, read_origin)
        } else if copy.phase == CooperativeCopyPhaseV1::Write {
            (copy.destination, copy.destination_region, write_origin)
        } else {
            return false;
        };
        if self.endpoint != endpoint
            || !self.origin.zip(base).is_some_and(|(origin, base)| {
                origin.matches_leaf(base, endpoint, self.allocation, self.stream)
            })
        {
            return false;
        }
        let Some(id) = self.submission else {
            return true;
        };
        let Some(active) = child.active_sdma.get(&id) else {
            return child.exact_submission_quiescent_v1(id)
                && child
                    .submissions
                    .get(&id)
                    .is_some_and(|record| Some(record.stream) == self.stream);
        };
        let (device, scratch, offset, scratch_offset) = if reading {
            (
                active.source,
                active.destination,
                active.source_offset,
                active.destination_offset,
            )
        } else {
            (
                active.destination,
                active.source,
                active.destination_offset,
                active.source_offset,
            )
        };
        self.step == LeafStepV1::Observe
            && active.id == id
            && Some(active.stream) == self.stream
            && device == endpoint.local
            && Some(scratch) == self.allocation
            && scratch_offset == 0
            && region.byte_offset.checked_add(copy.byte_cursor as u64) == Some(offset)
            && active.byte_len
                == copy
                    .staging
                    .len()
                    .saturating_sub(copy.byte_cursor)
                    .min(COOPERATIVE_COPY_CHUNK_BYTES_V1) as u64
            && active
                .peer_access
                .is_some_and(|access| Some(access.origin()) == self.origin)
            && child.peer_dma_access_is_intact_v1(active)
    }

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
                self.origin = self
                    .origin
                    .map(|origin| origin.with_scratch(Some(allocation)));
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
        if let Some(root) = self.reconciliation {
            child.release_native_reconciliation_v1(root);
            self.reconciliation = None;
        } else if let Some(submission) = self.submission {
            child.release_submission_v1(submission)?;
            self.submission = None;
        } else if let Some(allocation) = self.allocation {
            child.release_allocation_v1(allocation)?;
            self.allocation = None;
            self.origin = self.origin.map(|origin| origin.with_scratch(None));
            self.refund(child, false)?;
        } else if let Some(stream) = self.stream {
            child.destroy_stream_v1(stream)?;
            self.stream = None;
            self.origin = self.origin.map(|origin| origin.with_stream(None));
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
                leaf.origin = leaf.origin.map(|origin| origin.with_stream(Some(stream)));
                leaf.step = if child.allocations[&leaf.endpoint.local]
                    .native_dirty
                    .is_empty()
                {
                    LeafStepV1::Submit
                } else {
                    LeafStepV1::Reconcile
                };
                LeafProgressV1::Changed
            }),
        LeafStepV1::Reconcile => {
            if let Some(root) = leaf.reconciliation {
                return child
                    .progress_native_reconciliation_v1(root)
                    .map(|complete| {
                        if complete {
                            leaf.reconciliation = None;
                        }
                        LeafProgressV1::Changed
                    });
            }
            leaf.reconciliation = if leaf.origin.is_some() {
                child.begin_native_reconciliation_with_peer_access_v1(
                    leaf.endpoint.local,
                    leaf.allocation.unwrap(),
                    leaf.origin,
                )?
            } else {
                child
                    .begin_native_reconciliation_v1(leaf.endpoint.local, leaf.allocation.unwrap())?
            };
            if leaf.reconciliation.is_none() {
                leaf.step = LeafStepV1::Submit;
            }
            Ok(LeafProgressV1::Changed)
        }
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
            if child.allocations[&leaf.endpoint.local].kind == RuntimeMemoryKindV1::HostVisible {
                if reading {
                    child.read_cooperative_host_range_v1(
                        leaf.endpoint.local,
                        copy.source_region.byte_offset + start as u64,
                        &mut copy.staging[start..end],
                        leaf.origin,
                    )?;
                } else {
                    if leaf.origin.is_some() {
                        child.write_cooperative_host_range_with_peer_access_v1(
                            leaf.endpoint.local,
                            copy.destination_region.byte_offset + start as u64,
                            &copy.staging[start..end],
                            leaf.origin,
                        )?;
                    } else {
                        child.write_cooperative_host_range_v1(
                            leaf.endpoint.local,
                            copy.destination_region.byte_offset + start as u64,
                            &copy.staging[start..end],
                        )?;
                    }
                }
                copy.byte_cursor = end;
                if end == copy.staging.len() {
                    leaf.step = LeafStepV1::Cleanup(if reading {
                        CooperativeCopyPhaseV1::Write
                    } else {
                        CooperativeCopyPhaseV1::Succeeded
                    });
                }
                return Ok(LeafProgressV1::Changed);
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
            match child.copy_async_with_peer_access_v1(
                stream,
                source,
                destination,
                &[],
                leaf.origin,
            ) {
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
    /// Select only a concrete private owner already past dependency gating.
    /// This is one-hop resource progress, never a new dependency or recursive poll.
    pub(super) fn directed_private_blocker_v1(
        &mut self,
        selected: u64,
    ) -> Result<Option<u64>, Failure> {
        let origin = match &self.submissions[&selected] {
            RoutedSubmissionV1::CooperativeCopy(copy)
                if matches!(
                    copy.phase,
                    CooperativeCopyPhaseV1::Read | CooperativeCopyPhaseV1::Write
                ) =>
            {
                self.peer_copy_origin_v1(selected)?
            }
            _ => None,
        };
        let RoutedSubmissionV1::CooperativeCopy(copy) = &self.submissions[&selected] else {
            unreachable!()
        };
        if !matches!(
            copy.phase,
            CooperativeCopyPhaseV1::Read | CooperativeCopyPhaseV1::Write
        ) || copy.directed.is_none()
            || copy.sdma_leaf.as_ref().is_some_and(|leaf| {
                !matches!(leaf.step, LeafStepV1::Submit | LeafStepV1::Reconcile)
                    || leaf.reconciliation.is_some()
            })
        {
            return Ok(None);
        }
        let reading = copy.phase == CooperativeCopyPhaseV1::Read;
        let endpoint = if reading {
            copy.source
        } else {
            copy.destination
        };
        let child = &self.children[endpoint.child];
        let reconciliation = child.native_reconciliation_blocker_v1(endpoint.local);
        let custody = child.allocation_custody.get(&endpoint.local);
        let dma = if reading && reconciliation.is_none() {
            if let Some(custody) = custody {
                let mut owners = custody.owners.iter().copied().filter(|owner| {
                    !child.peer_access_authorizes_owner_v1(
                        endpoint.local,
                        *owner,
                        origin,
                        PeerAccessPurposeV1::Copy,
                    )
                });
                let first = owners.next();
                if first.is_some_and(|owner| owner.kind != RuntimeAllocationCustodyKindV1::Sdma)
                    || owners.next().is_some()
                {
                    return Err(self.directed_corruption_v1());
                }
                first
            } else {
                None
            }
        } else {
            None
        };
        if reconciliation.is_none() && dma.is_none() {
            return Ok(None);
        }
        let target = reconciliation.map_or(endpoint, |(_, allocation, _)| RoutedHandleV1 {
            child: endpoint.child,
            local: allocation,
        });
        let Some(owners) = self.cooperative_allocation_owners.get(&target) else {
            return Err(self.directed_corruption_v1());
        };
        // An unrelated legacy-only roster has no directed progress contract.
        if owners.len() > MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1 {
            return Ok(None);
        }
        let mut blocker = None;
        for id in owners {
            let Some(RoutedSubmissionV1::CooperativeCopy(other)) = self.submissions.get(id) else {
                continue;
            };
            let Some(leaf) = &other.sdma_leaf else {
                continue;
            };
            // Private IDs are child-local, unlike outer submission handles.
            if leaf.endpoint.child != target.child {
                continue;
            }
            let matches_owner = reconciliation.map_or_else(
                || leaf.submission == dma.map(|owner| owner.submission),
                |(root, _, _)| leaf.reconciliation == Some(root),
            );
            if !matches_owner {
                continue;
            }
            if other.directed.is_none() {
                return Ok(None);
            }
            if !self.directed_identity_is_intact_v1(*id) || leaf.endpoint != target {
                return Err(self.directed_corruption_v1());
            }
            let owns = if let Some((root, _, scratch)) = reconciliation {
                leaf.reconciliation == Some(root)
                    && leaf.allocation == Some(scratch)
                    && leaf.step == LeafStepV1::Reconcile
                    && match other.phase {
                        CooperativeCopyPhaseV1::Read => other.source == target,
                        CooperativeCopyPhaseV1::Write => other.destination == target,
                        _ => false,
                    }
            } else {
                other.phase == CooperativeCopyPhaseV1::Read
                    && copy
                        .directed
                        .as_ref()
                        .unwrap()
                        .shares_read_source(other, endpoint)
                    && leaf.step == LeafStepV1::Observe
                    && leaf.submission.is_some_and(|submission| {
                        dma.is_some_and(|owner| Some(owner.stream) == leaf.stream)
                            && child.active_sdma.get(&submission).is_some_and(|active| {
                                active.id == submission
                                    && Some(active.stream) == leaf.stream
                                    && active.source == endpoint.local
                                    && Some(active.destination) == leaf.allocation
                                    && active.source_offset
                                        == other.source_region.byte_offset
                                            + other.byte_cursor as u64
                                    && active.destination_offset == 0
                                    && active.byte_len
                                        == (other.staging.len() - other.byte_cursor)
                                            .min(COOPERATIVE_COPY_CHUNK_BYTES_V1)
                                            as u64
                                    && active.dependencies.is_empty()
                                    && child.allocations.get(&active.source).is_some_and(|record| {
                                        record.kind == RuntimeMemoryKindV1::DeviceLocal
                                    })
                                    && child.allocations.get(&active.destination).is_some_and(
                                        |record| record.kind == RuntimeMemoryKindV1::HostVisible,
                                    )
                            })
                    })
            };
            if !owns || blocker.is_some() {
                return Err(self.directed_corruption_v1());
            }
            blocker = Some(*id);
        }
        let Some(id) = blocker else {
            return Err(self.directed_corruption_v1());
        };
        Ok((id != selected).then_some(id))
    }

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
            && (record.kind == RuntimeMemoryKindV1::DeviceLocal
                || !record.native_dirty.is_empty()
                || copy.phase == CooperativeCopyPhaseV1::Write)
    }

    fn with_cooperative_leaf_v1<T>(
        &mut self,
        submission: u64,
        operation: impl FnOnce(
            &mut KfdRuntimeBackendV1,
            &mut CooperativeCopySubmissionV1,
        ) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let RoutedSubmissionV1::CooperativeCopy(copy) = &self.submissions[&submission] else {
            unreachable!()
        };
        let leaf = copy.sdma_leaf.as_ref().expect("private leaf is installed");
        let (origin, endpoint, allocation, stream) =
            (leaf.origin, leaf.endpoint, leaf.allocation, leaf.stream);
        let current = self.peer_copy_origin_for_leg_v1(
            submission,
            origin.map_or(PeerCopyLegV1::Read, PeerCopyOriginV1::leg),
        )?;
        if match (origin, current) {
            (Some(retained), Some(current)) => {
                !retained.matches_leaf(current, endpoint, allocation, stream)
            }
            (None, None) => false,
            _ => true,
        } {
            return Err(self.directed_corruption_v1());
        }
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
        let origin = self.peer_copy_origin_v1(submission)?;
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
                origin,
                endpoint: if copy.phase == CooperativeCopyPhaseV1::Read {
                    copy.source
                } else {
                    copy.destination
                },
                allocation: None,
                stream: None,
                submission: None,
                reconciliation: None,
                credit: None,
                step: LeafStepV1::Allocate,
            });
            self.note_cooperative_progress();
            return Ok(BackendPollV1::Pending);
        }
        let leaf = copy.sdma_leaf.as_ref().unwrap();
        if match (leaf.origin, origin) {
            (Some(retained), Some(current)) => {
                !retained.matches_leaf(current, leaf.endpoint, leaf.allocation, leaf.stream)
            }
            (None, None) => false,
            _ => true,
        } {
            return Err(self.directed_corruption_v1());
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
            for _ in 0..5 {
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
            unreachable!("bounded private leaf cleanup exceeded its four owners")
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
            for _ in 0..5 {
                if leaf.cleanup(child)? {
                    return Ok(());
                }
            }
            unreachable!("bounded residual cleanup exceeded its four owners")
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
        directed: Option<&cooperative_directed::Root>,
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
                            if (copy.stream == stream || dependencies.contains(parent)
                                || directed.is_some_and(|root| root.shares_read_source(copy, route)))
                                && copy.sdma_leaf.as_ref().is_some_and(|leaf| {
                                    leaf.endpoint == route && leaf.submission == Some(owner.submission)
                                        && leaf.stream == Some(owner.stream)
                                }))
                    })
                })
        })
    }
}
