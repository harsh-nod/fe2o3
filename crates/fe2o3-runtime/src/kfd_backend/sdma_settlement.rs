//! Local release prerequisites and callback-free indexed settlement.

use super::*;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum SdmaSettlementV1 {
    Succeeded,
    Failed,
    Quiescent,
    Cancelled,
}

impl KfdRuntimeBackendV1 {
    pub(super) fn sdma_release_custody_is_intact_v1(&self, submission: u64) -> bool {
        let active = &self.active_sdma[&submission];
        if active.id != submission
            || !matches!(
                active.phase,
                ActiveSdmaPhaseV1::Ready | ActiveSdmaPhaseV1::Quarantined
            )
            || active.source == active.destination
            || self.direct_sdma_copy_kind_for_active_v1(active).is_err()
            || active.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1
            || active.dependency_cursor > active.dependencies.len()
            || self.sdma_completion_reservations == 0
            || self.submissions.contains_key(&submission)
            || self.submissions.len() == self.submissions.capacity()
            || self.published_sdma_submissions.contains(&submission)
        {
            return false;
        }
        // The admitted roster is unique but not sorted. Check uniqueness without
        // allocating or replacing its retained backing, before decrementing counts.
        let mut sorted = [0_u64; MAX_RUNTIME_DEPENDENCIES_V1];
        let dependencies = &mut sorted[..active.dependencies.len()];
        dependencies.copy_from_slice(&active.dependencies);
        dependencies.sort_unstable();
        if dependencies.windows(2).any(|pair| pair[0] == pair[1])
            || dependencies.iter().any(|id| {
                self.sdma_dependency_retain_counts
                    .get(id)
                    .is_none_or(|count| *count == 0)
            })
        {
            return false;
        }
        let expected = RuntimeAllocationCustodyOwnerV1 {
            submission,
            stream: active.stream,
            kind: RuntimeAllocationCustodyKindV1::Sdma,
        };
        // Private admission/removal preserves ordered owner and stream indexes.
        // Validate local release prerequisites without rescanning an entire queue.
        [active.source, active.destination].into_iter().all(|id| {
            let Some(record) = self.allocations.get(&id) else {
                return false;
            };
            // Unstarted successors may release their reservation while an earlier
            // owner is still active. A handed-off copy must have restored storage.
            let storage_intact = match record.sdma_storage {
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(actual))
                | KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) => {
                    matches!(active.phase, ActiveSdmaPhaseV1::Ready) && actual != submission
                }
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous) => false,
                _ => true,
            };
            storage_intact
                && self.allocation_custody.get(&id).is_some_and(|custody| {
                    custody.owner_counts[RuntimeAllocationCustodyKindV1::Sdma.index()] != 0
                        && custody
                            .owners
                            .binary_search_by_key(&submission, |owner| owner.submission)
                            .is_ok_and(|index| {
                                custody.owners[index] == expected
                                    && index
                                        .checked_sub(1)
                                        .and_then(|prior| custody.owners.get(prior))
                                        .is_none_or(|owner| owner.submission != submission)
                                    && custody
                                        .owners
                                        .get(index + 1)
                                        .is_none_or(|owner| owner.submission != submission)
                            })
                })
        }) && self
            .active_sdma_streams
            .get(&active.stream)
            .is_some_and(|queue| {
                queue.binary_search(&submission).is_ok_and(|index| {
                    index.checked_sub(1).and_then(|prior| queue.get(prior)) != Some(&submission)
                        && queue.get(index + 1) != Some(&submission)
                })
            })
    }

    pub(super) fn settle_sdma_copy_v1(
        &mut self,
        submission: u64,
        settlement: SdmaSettlementV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        let intact = match settlement {
            SdmaSettlementV1::Succeeded => self.sdma_completion_custody_is_intact_v1(submission),
            SdmaSettlementV1::Cancelled => {
                let active = &self.active_sdma[&submission];
                matches!(active.phase, ActiveSdmaPhaseV1::Ready)
                    && active.completed_bytes == 0
                    && active.window_bytes == 0
                    && active.window_requests.is_none()
                    && self.sdma_release_custody_is_intact_v1(submission)
            }
            SdmaSettlementV1::Failed | SdmaSettlementV1::Quiescent => {
                self.sdma_release_custody_is_intact_v1(submission)
            }
        };
        let quiescent = settlement == SdmaSettlementV1::Quiescent;
        if !intact
            || self.quiescent_sdma_submissions.contains(&submission)
            || (quiescent && !self.quiescent_sdma_marker_capacity_is_reserved_v1())
        {
            return Err(self.terminal_error("SDMA completion custody changed before release"));
        }
        let active = &self.active_sdma[&submission];
        let (source, destination, stream, dependency_depth) = (
            active.source,
            active.destination,
            active.stream,
            active.dependency_depth,
        );
        Self::release_sdma_dependency_counts_v1(
            &mut self.sdma_dependency_retain_counts,
            &active.dependencies,
        );
        self.release_allocation_custody_v1(source, submission);
        self.release_allocation_custody_v1(destination, submission);
        self.release_active_sdma_stream_v1(stream, submission);
        let status = match settlement {
            SdmaSettlementV1::Succeeded => BackendPollV1::Succeeded,
            SdmaSettlementV1::Cancelled => BackendPollV1::Failed { code: -2 },
            SdmaSettlementV1::Failed | SdmaSettlementV1::Quiescent => BackendPollV1::Failed {
                code: COOPERATIVE_COPY_FAILURE_CODE_V1,
            },
        };
        self.submissions.insert(
            submission,
            SubmissionRecordV1 {
                stream,
                status,
                dependency_depth,
                profile_dispatch_published: false,
            },
        );
        if quiescent {
            self.quiescent_sdma_submissions.insert(submission);
        }
        self.sdma_completion_reservations -= 1;
        self.active_sdma.remove(&submission);
        if quiescent {
            debug_assert!(self.quiescent_sdma_marker_capacity_is_reserved_v1());
        }
        Ok(status)
    }

    pub(super) fn cancel_sdma_copy_v1(
        &mut self,
        submission: u64,
    ) -> Result<crate::BackendCancellationV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        self.require_live()?;
        let active = &self.active_sdma[&submission];
        if matches!(
            active.phase,
            ActiveSdmaPhaseV1::DirectionalPublished(_) | ActiveSdmaPhaseV1::SameDevicePublished(_)
        ) || active.completed_bytes != 0
        {
            return Ok(crate::BackendCancellationV1::TooLate);
        }
        let (stream, prior) = (active.stream, active.prior_stream_submission);
        self.settle_sdma_copy_v1(submission, SdmaSettlementV1::Cancelled)?;
        self.restore_stream_tail_before_v1(stream, submission, prior);
        Ok(crate::BackendCancellationV1::Cancelled)
    }

    pub(super) fn fail_unpublished_sdma_copy_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.settle_sdma_copy_v1(submission, SdmaSettlementV1::Failed)
    }

    pub(super) fn fail_quiescent_sdma_copy_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.settle_sdma_copy_v1(submission, SdmaSettlementV1::Quiescent)
            .map(|_| ())
    }
}
