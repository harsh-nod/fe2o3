//! Checked, indexed release of an accepted compute recipe that never published.

use super::*;

impl KfdRuntimeBackendV1 {
    pub(super) fn pending_compute_stream_membership_intact_v1(
        &self,
        stream: u64,
        submission: u64,
    ) -> bool {
        self.pending_compute_streams
            .get(&stream)
            .is_some_and(|queue| {
                queue.binary_search(&submission).is_ok_and(|index| {
                    index
                        .checked_sub(1)
                        .and_then(|prior| queue.get(prior))
                        .is_none_or(|id| *id < submission)
                        && queue.get(index + 1).is_none_or(|id| *id > submission)
                })
            })
    }

    fn pending_compute_release_custody_is_intact_v1(&self, submission: u64) -> bool {
        let pending = &self.pending_compute[&submission];
        let bindings = &pending.launch.bindings;
        if pending.id != submission
            || submission == 0
            || bindings.len() > fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1
            || pending.explicit_success_dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1
            || pending.quiescence_dependencies.len() > self.compute_quiescence_limit_v1()
            || pending.explicit_dependency_cursor > pending.explicit_success_dependencies.len()
            || pending.quiescence_cursor > pending.quiescence_dependencies.len()
            || self.compute_completion_reservations == 0
            || self.submissions.contains_key(&submission)
            || self.active_sdma.contains_key(&submission)
            || self.active_compute_lane_v1(submission).is_some()
            || !pending
                .peer_access
                .valid_for(pending.peer_gate, submission, bindings)
            || pending.peer_gate.is_some_and(|gate| {
                gate.action(submission, false, false) == PeerComputeActionV1::Invalid
            })
            || self
                .compute_module_retain_counts
                .get(&pending.module)
                .is_none_or(|count| *count == 0)
            || self
                .kernels
                .get(&pending.launch.kernel)
                .is_none_or(|kernel| kernel.module != pending.module)
            || self.modules.get(&pending.module).is_none_or(|module| {
                self.streams.get(&pending.launch.stream) != Some(&module.device)
            })
        {
            return false;
        }
        // Admission preserves first-binding order and retains each allocation
        // once. Authenticate that projection before releasing any of its members.
        let mut retained = pending.retained_allocations.iter();
        for (index, binding) in bindings.iter().enumerate() {
            let allocation = binding.region.allocation;
            if bindings[..index]
                .iter()
                .any(|prior| prior.region.allocation == allocation)
            {
                continue;
            }
            if retained.next() != Some(&allocation) || !self.allocations.contains_key(&allocation) {
                return false;
            }
            let expected = RuntimeAllocationCustodyOwnerV1 {
                submission,
                stream: pending.launch.stream,
                kind: RuntimeAllocationCustodyKindV1::Compute,
            };
            // Private admission/removal maintains monotone indexes. Check this
            // owner's local release prerequisites, not every unrelated owner.
            if !self
                .allocation_custody
                .get(&allocation)
                .is_some_and(|custody| {
                    custody.owner_counts[RuntimeAllocationCustodyKindV1::Compute.index()] != 0
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
            {
                return false;
            }
        }
        if retained.next().is_some() {
            return false;
        }
        let mut sorted = [0; MAX_RUNTIME_DEPENDENCIES_V1];
        let explicit = &mut sorted[..pending.explicit_success_dependencies.len()];
        explicit.copy_from_slice(&pending.explicit_success_dependencies);
        explicit.sort_unstable();
        let retained_dependency = |id: u64| {
            id != 0
                && id < submission
                && self
                    .compute_dependency_retain_counts
                    .get(&id)
                    .is_some_and(|count| *count != 0)
        };
        if explicit.windows(2).any(|pair| pair[0] == pair[1])
            || explicit.iter().any(|id| !retained_dependency(*id))
            || pending.ordered_predecessor.is_some_and(|id| {
                !retained_dependency(id)
                    || self.native_dependency_stream_v1(id) != Some(pending.launch.stream)
            })
            || pending
                .quiescence_dependencies
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || pending.quiescence_dependencies.iter().any(|id| {
                !retained_dependency(*id)
                    || explicit.binary_search(id).is_ok()
                    || pending.ordered_predecessor == Some(*id)
            })
        {
            return false;
        }
        self.pending_compute_stream_membership_intact_v1(pending.launch.stream, submission)
    }

    pub(super) fn settle_failed_unpublished_compute_v1(
        &mut self,
        pending: PendingComputeSubmissionV1,
        code: i64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        // Progress callers temporarily remove this recipe. Keep it outside any
        // fallible reindex allocation, and retain it inline if reindexing fails.
        let submission = pending.id;
        if self.pending_compute.contains_key(&submission)
            || self.pending_compute.try_reserve(1).is_err()
        {
            if self.terminal_pending_compute.is_some() {
                std::process::abort();
            }
            self.terminal_pending_compute = Some(pending);
            return Err(
                self.terminal_error("unpublished compute settlement cannot restore its index")
            );
        }
        self.pending_compute.insert(submission, pending);
        self.settle_indexed_unpublished_compute_v1(submission, code)
    }

    pub(super) fn settle_indexed_unpublished_compute_v1(
        &mut self,
        submission: u64,
        code: i64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if !self.pending_compute_release_custody_is_intact_v1(submission) {
            return Err(self.terminal_error("unpublished compute custody changed before release"));
        }
        if self.submissions.try_reserve(1).is_err() {
            return Err(
                self.terminal_error("unpublished compute settlement cannot reserve its result")
            );
        }
        let pending = &self.pending_compute[&submission];
        let (stream, module, depth, count) = (
            pending.launch.stream,
            pending.module,
            pending.dependency_depth,
            pending.retained_allocations.len(),
        );
        Self::release_compute_dependency_counts_v1(
            &mut self.compute_dependency_retain_counts,
            &pending.explicit_success_dependencies,
        );
        Self::release_compute_dependency_counts_v1(
            &mut self.compute_dependency_retain_counts,
            &pending.quiescence_dependencies,
        );
        if let Some(prior) = pending.ordered_predecessor
            && !pending.explicit_success_dependencies.contains(&prior)
        {
            Self::release_compute_dependency_counts_v1(
                &mut self.compute_dependency_retain_counts,
                &[prior],
            );
        }
        for index in 0..count {
            let allocation = self.pending_compute[&submission].retained_allocations[index];
            self.release_allocation_custody_v1(allocation, submission);
        }
        self.release_compute_module_retain_v1(module);
        self.remove_pending_compute_from_stream_v1(stream, submission);
        let status = BackendPollV1::Failed { code };
        self.submissions.insert(
            submission,
            SubmissionRecordV1 {
                origin: SubmissionOriginV1::Ordinary,
                stream,
                status,
                dependency_depth: depth,
                profile_dispatch_published: false,
            },
        );
        self.compute_completion_reservations -= 1;
        self.pending_compute.remove(&submission);
        Ok(status)
    }
}
