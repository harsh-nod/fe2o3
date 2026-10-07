use super::*;

impl KfdRuntimeBackendV1 {
    pub(in crate::kfd_backend) fn progress_pending_compute_v1(
        &mut self,
        mut pending: PendingComputeSubmissionV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let is_head = self
            .pending_compute_streams
            .get(&pending.launch.stream)
            .and_then(|queue| queue.front())
            .is_some_and(|head| *head == pending.id);
        if !is_head {
            self.pending_compute.insert(pending.id, pending);
            return Ok(BackendPollV1::Pending);
        }
        pending = match self.observe_peer_compute_gate_v1(pending)? {
            PeerComputeStepV1::Continue(pending) => pending,
            PeerComputeStepV1::Observed(status) => return Ok(status),
        };
        while let Some(dependency) = pending
            .explicit_success_dependencies
            .get(pending.explicit_dependency_cursor)
            .copied()
        {
            let (retained, result) = self.poll_retained_pending_dependency_v1(pending, dependency);
            pending = retained;
            let status = match result {
                Ok(status) => status,
                Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                    return self.settle_failed_compute_after_ordering_v1(pending);
                }
                Err(failure @ RuntimeBackendFailureV1::Rejected(_))
                | Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                    self.pending_compute.insert(pending.id, pending);
                    return Err(failure);
                }
            };
            match status {
                BackendPollV1::Succeeded => pending.explicit_dependency_cursor += 1,
                BackendPollV1::Pending => {
                    self.pending_compute.insert(pending.id, pending);
                    return Ok(BackendPollV1::Pending);
                }
                BackendPollV1::Failed { .. } => {
                    return self.settle_failed_compute_after_ordering_v1(pending);
                }
            }
        }
        if let Some(predecessor) = pending.ordered_predecessor
            && !self
                .submissions
                .get(&predecessor)
                .is_some_and(|record| record.status != BackendPollV1::Pending)
        {
            if let Some(lane) = self.active_compute_lane_v1(predecessor) {
                let same_physical_stream = ordered_successor_lane_matches_v1(
                    self.stream_compute_lanes
                        .get(&pending.launch.stream)
                        .copied(),
                    lane,
                );
                if same_physical_stream {
                    let publication =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            self.with_compute_lane_state_v1(lane, |backend| {
                                backend.try_publish_ordered_successor_v1(&pending, predecessor)
                            })
                        }));
                    let publication = match publication {
                        Ok(publication) => publication,
                        Err(payload) => {
                            if self.active_compute_lane_v1(pending.id).is_some() {
                                self.remove_pending_compute_from_stream_v1(
                                    pending.launch.stream,
                                    pending.id,
                                );
                                self.release_compute_dependency_retains_v1(
                                    &pending.explicit_success_dependencies,
                                );
                            } else {
                                self.pending_compute.insert(pending.id, pending);
                            }
                            super::sdma_host_write::resume_sdma_owner_panic_v1(payload, || {
                                self.poison_terminal_v1()
                            });
                        }
                    };
                    match publication {
                        Ok(true) => {
                            self.remove_pending_compute_from_stream_v1(
                                pending.launch.stream,
                                pending.id,
                            );
                            self.release_compute_dependency_retains_v1(
                                &pending.explicit_success_dependencies,
                            );
                            return Ok(BackendPollV1::Pending);
                        }
                        Ok(false) => {}
                        Err(failure) if self.active_compute_lane_v1(pending.id).is_some() => {
                            self.remove_pending_compute_from_stream_v1(
                                pending.launch.stream,
                                pending.id,
                            );
                            self.release_compute_dependency_retains_v1(
                                &pending.explicit_success_dependencies,
                            );
                            return Err(match failure {
                                failure @ RuntimeBackendFailureV1::Terminal(_) => failure,
                                _ => self.terminal_error(
                                    "indexed ordered publication returned nonterminal failure",
                                ),
                            });
                        }
                        Err(failure) => {
                            self.pending_compute.insert(pending.id, pending);
                            return Err(failure);
                        }
                    }
                }
            }
            let (retained, result) = self.poll_retained_pending_dependency_v1(pending, predecessor);
            pending = retained;
            let status = match result {
                Ok(status) => status,
                Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                    if !self.exact_submission_quiescent_v1(predecessor) {
                        self.pending_compute.insert(pending.id, pending);
                        return Ok(BackendPollV1::Pending);
                    }
                    return self.settle_failed_unpublished_compute_v1(pending, -1);
                }
                Err(failure @ RuntimeBackendFailureV1::Rejected(_))
                | Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                    self.pending_compute.insert(pending.id, pending);
                    return Err(failure);
                }
            };
            if !ordered_predecessor_completed_v1(status) {
                self.pending_compute.insert(pending.id, pending);
                return Ok(BackendPollV1::Pending);
            }
            // Stream ordering observes completion, not success. A failed
            // ordered predecessor therefore does not fail this launch unless
            // the same identity also appeared in the explicit dependency set.
        }
        let Some(ready) = self.observe_compute_quiescence_v1(pending)? else {
            return Ok(BackendPollV1::Pending);
        };
        pending = ready;
        let conversion_candidate = self
            .initialized_storage_candidates_v1(pending.launch.borrowed())
            .iter()
            .any(Option::is_some);
        let three_binding_admission =
            self.three_binding_persistent_admission_for_launch_v1(pending.launch.borrowed());
        if three_binding_requires_persistent_admission_v1(
            pending.launch.semantic_launch,
            &pending.launch.bindings,
            &self.allocations,
        ) && three_binding_admission.is_none()
            && !conversion_candidate
        {
            return self.settle_failed_unpublished_compute_v1(pending, -1);
        }
        let persistent_selected = conversion_candidate
            || self
                .persistent_full_range_admission_for_launch_v1(pending.launch.borrowed())
                .is_some()
            || three_binding_admission.is_some();
        if self.native_reconciliations.iter().flatten().any(|root| {
            persistent_selected
                || pending.launch.bindings.iter().any(|binding| {
                    binding.region.allocation == root.allocation
                        || (0..self.native_compute_lanes.len()).any(|lane| {
                            self.native_reconciliation_pins_lane_v1(lane)
                                && self.compute_lane_caches_allocation_v1(
                                    lane,
                                    binding.region.allocation,
                                )
                        })
                })
        }) {
            self.pending_compute.insert(pending.id, pending);
            return Ok(BackendPollV1::Pending);
        }
        if persistent_selected && self.has_live_generated_native_v1() {
            self.pending_compute.insert(pending.id, pending);
            return Ok(BackendPollV1::Pending);
        }
        if persistent_selected && let Some(copy) = self.persistent_compute_sdma_blocker_v1(&pending)
        {
            self.pending_compute.insert(pending.id, pending);
            let _ = self.poll_v1(copy)?;
            return Ok(BackendPollV1::Pending);
        }
        let compute_exclusion_blocker = if persistent_selected {
            self.active_compute_progress_roster_v1()
                .iter()
                .position(|active| *active)
        } else if self.persistent_compute_is_active_v1() {
            Some(0)
        } else {
            None
        };
        if let Some(blocker) = compute_exclusion_blocker {
            self.pending_compute.insert(pending.id, pending);
            let _ = self.poll_compute_lane_v1(blocker)?;
            return Ok(BackendPollV1::Pending);
        }
        let available_lane = self.free_compute_lane_v1();
        let lane = admitted_compute_lane_v1(available_lane, persistent_selected);
        let Some(lane) = lane else {
            self.pending_compute.insert(pending.id, pending);
            return Ok(BackendPollV1::Pending);
        };
        let conflicting_compute_lane = (0..self.native_compute_lanes.len()).find(|lane| {
            if *lane == 0 {
                launch_overlaps_active_compute_v1(
                    &pending.launch.bindings,
                    self.active.iter().chain(self.compute_pipeline.iter()),
                )
            } else {
                let state = &self.auxiliary_compute_lanes[*lane - 1];
                launch_overlaps_active_compute_v1(
                    &pending.launch.bindings,
                    state.active.iter().chain(state.pipeline.iter()),
                )
            }
        });
        if let Some(conflicting_lane) = conflicting_compute_lane {
            self.pending_compute.insert(pending.id, pending);
            let _ = self.poll_compute_lane_v1(conflicting_lane)?;
            return Ok(BackendPollV1::Pending);
        }
        let conflicting_copy = self.published_sdma_conflict_v1(
            pending.id,
            pending.launch.stream,
            &pending.launch.bindings,
        );
        if let Some(copy) = conflicting_copy {
            let (pending, result) = self.poll_retained_pending_dependency_v1(pending, copy);
            return match result {
                Ok(_) => {
                    self.pending_compute.insert(pending.id, pending);
                    Ok(BackendPollV1::Pending)
                }
                Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                    self.settle_failed_unpublished_compute_v1(pending, -1)
                }
                Err(failure @ RuntimeBackendFailureV1::Rejected(_))
                | Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                    self.pending_compute.insert(pending.id, pending);
                    Err(failure)
                }
            };
        }
        let staging = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if persistent_selected {
                self.release_compute_lane_cache_v1(lane)?;
            }
            for binding in &pending.launch.bindings {
                self.synchronize_native_allocation_v1(binding.region.allocation)?;
                for cached_lane in 0..self.native_compute_lanes.len() {
                    if cached_lane != lane
                        && self.compute_lane_caches_allocation_v1(
                            cached_lane,
                            binding.region.allocation,
                        )
                    {
                        self.release_compute_lane_cache_v1(cached_lane)?;
                    }
                }
            }
            for allocation in self
                .initialized_storage_candidates_v1(pending.launch.borrowed())
                .into_iter()
                .flatten()
            {
                self.convert_initialized_storage_v1(allocation)?;
            }
            Ok(())
        }));
        let staging = match staging {
            Ok(staging) => staging,
            Err(payload) => {
                self.pending_compute.insert(pending.id, pending);
                super::sdma_host_write::resume_sdma_owner_panic_v1(payload, || {
                    self.poison_terminal_v1()
                })
            }
        };
        if let Err(failure) = staging {
            return match failure {
                _failure @ (RuntimeBackendFailureV1::Rejected(_)
                | RuntimeBackendFailureV1::Quiescent(_)) => {
                    #[cfg(test)]
                    record_unpublished_compute_failure_for_test_v1(
                        pending.id,
                        pending.launch.stream,
                        "staging",
                        &_failure,
                    );
                    self.settle_failed_unpublished_compute_v1(pending, -1)
                }
                failure @ RuntimeBackendFailureV1::Terminal(_) => {
                    self.pending_compute.insert(pending.id, pending);
                    Err(failure)
                }
            };
        }
        let three_binding_admission =
            self.three_binding_persistent_admission_for_launch_v1(pending.launch.borrowed());
        if three_binding_requires_persistent_admission_v1(
            pending.launch.semantic_launch,
            &pending.launch.bindings,
            &self.allocations,
        ) && three_binding_admission.is_none()
        {
            return self.settle_failed_unpublished_compute_v1(pending, -1);
        }
        let persistent_selected = self
            .persistent_full_range_admission_for_launch_v1(pending.launch.borrowed())
            .is_some()
            || three_binding_admission.is_some();
        self.lease_compute_lane_v1(pending.launch.stream, lane);
        let publication = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.with_compute_lane_state_v1(lane, |backend| {
                if !pending.peer_gate_allows_native_checks_v1() || !pending.quiescence_complete_v1()
                {
                    return Err(
                        backend.terminal_error("KFD compute publication lost a completion gate")
                    );
                }
                let prepared = backend.prepare_launch(
                    pending.launch.borrowed(),
                    persistent_selected,
                    false,
                )?;
                backend.publish(
                    pending.id,
                    pending.dependency_depth,
                    pending.ordered_predecessor,
                    Arc::clone(&pending.launch),
                    prepared,
                )
            })
        }));
        let publication = match publication {
            Ok(publication) => publication,
            Err(payload) => {
                if self.active_compute_lane_v1(pending.id).is_some() {
                    self.remove_pending_compute_from_stream_v1(pending.launch.stream, pending.id);
                    self.release_pending_compute_dependency_retains_v1(&pending);
                } else {
                    self.pending_compute.insert(pending.id, pending);
                }
                super::sdma_host_write::resume_sdma_owner_panic_v1(payload, || {
                    self.poison_terminal_v1()
                })
            }
        };
        match publication {
            Ok(()) => {
                self.remove_pending_compute_from_stream_v1(pending.launch.stream, pending.id);
                self.release_pending_compute_dependency_retains_v1(&pending);
                Ok(BackendPollV1::Pending)
            }
            Err(failure) if self.active_compute_lane_v1(pending.id).is_some() => {
                // Publication transferred custody even if its result is an error.
                // Retire only the pending handoff; never settle or duplicate Active.
                self.remove_pending_compute_from_stream_v1(pending.launch.stream, pending.id);
                self.release_pending_compute_dependency_retains_v1(&pending);
                match failure {
                    failure @ RuntimeBackendFailureV1::Terminal(_) => Err(failure),
                    RuntimeBackendFailureV1::Rejected(_)
                    | RuntimeBackendFailureV1::Quiescent(_) => Err(self.terminal_error(
                        "KFD indexed compute publication returned a nonterminal failure",
                    )),
                }
            }
            Err(
                _failure @ (RuntimeBackendFailureV1::Rejected(_)
                | RuntimeBackendFailureV1::Quiescent(_)),
            ) => {
                #[cfg(test)]
                record_unpublished_compute_failure_for_test_v1(
                    pending.id,
                    pending.launch.stream,
                    "preparation-or-publication",
                    &_failure,
                );
                self.release_compute_lane_lease_v1(pending.launch.stream, lane);
                self.settle_failed_unpublished_compute_v1(pending, -1)
            }
            Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                self.pending_compute.insert(pending.id, pending);
                Err(failure)
            }
        }
    }

    pub(in crate::kfd_backend) fn observe_pending_compute_v1(
        &mut self,
        mut pending: PendingComputeSubmissionV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let is_head = self
            .pending_compute_streams
            .get(&pending.launch.stream)
            .and_then(|queue| queue.front())
            .is_some_and(|head| *head == pending.id);
        if !is_head {
            self.pending_compute.insert(pending.id, pending);
            return Ok(BackendPollV1::Pending);
        }
        pending = match self.observe_peer_compute_gate_v1(pending)? {
            PeerComputeStepV1::Continue(pending) => pending,
            PeerComputeStepV1::Observed(status) => return Ok(status),
        };
        while let Some(dependency) = pending
            .explicit_success_dependencies
            .get(pending.explicit_dependency_cursor)
            .copied()
        {
            let (retained, result) = self.poll_retained_pending_dependency_v1(pending, dependency);
            pending = retained;
            let status = match result {
                Ok(status) => status,
                Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                    return self.settle_failed_compute_after_ordering_v1(pending);
                }
                Err(RuntimeBackendFailureV1::Rejected(error)) => {
                    self.pending_compute.insert(pending.id, pending);
                    return Err(self.terminal_error(format!(
                        "KFD pending compute retained an exact dependency that was rejected during observation: {error}"
                    )));
                }
                Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                    self.pending_compute.insert(pending.id, pending);
                    return Err(failure);
                }
            };
            match status {
                BackendPollV1::Succeeded => pending.explicit_dependency_cursor += 1,
                BackendPollV1::Pending => {
                    self.pending_compute.insert(pending.id, pending);
                    return Ok(BackendPollV1::Pending);
                }
                BackendPollV1::Failed { .. } => {
                    return self.settle_failed_compute_after_ordering_v1(pending);
                }
            }
        }
        if let Some(predecessor) = pending.ordered_predecessor
            && !self
                .submissions
                .get(&predecessor)
                .is_some_and(|record| record.status != BackendPollV1::Pending)
        {
            let (retained, result) = self.poll_retained_pending_dependency_v1(pending, predecessor);
            pending = retained;
            match result {
                Ok(_) => {}
                Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                    if !self.exact_submission_quiescent_v1(predecessor) {
                        self.pending_compute.insert(pending.id, pending);
                        return Ok(BackendPollV1::Pending);
                    }
                    return self.settle_failed_unpublished_compute_v1(pending, -1);
                }
                Err(RuntimeBackendFailureV1::Rejected(error)) => {
                    self.pending_compute.insert(pending.id, pending);
                    return Err(self.terminal_error(format!(
                        "KFD pending compute retained an ordered predecessor that was rejected during observation: {error}"
                    )));
                }
                Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                    self.pending_compute.insert(pending.id, pending);
                    return Err(failure);
                }
            }
        }
        if let Some(pending) = self.observe_compute_quiescence_v1(pending)? {
            self.pending_compute.insert(pending.id, pending);
        }
        Ok(BackendPollV1::Pending)
    }

    pub(in crate::kfd_backend) fn try_publish_ordered_successor_v1(
        &mut self,
        pending: &PendingComputeSubmissionV1,
        predecessor: u64,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if !pending.peer_gate_allows_native_checks_v1()
            || !pending.quiescence_dependencies.is_empty()
            || self
                .initialized_storage_candidates_v1(pending.launch.borrowed())
                .iter()
                .any(Option::is_some)
            || pending.launch.bindings.iter().any(|binding| {
                binding.region.access != RuntimeAccessV1::Write
                    && self
                        .allocations
                        .get(&binding.region.allocation)
                        .is_some_and(|allocation| allocation.sdma_shadow_dirty)
            })
            || !self.compute_pipeline.has_successor_capacity()
            || three_binding_requires_persistent_admission_v1(
                pending.launch.semantic_launch,
                &pending.launch.bindings,
                &self.allocations,
            )
            || !early_pipeline_launch_is_admitted_v1(
                pending.launch.semantic_launch,
                &pending.launch.bindings,
            )
            || pending.explicit_success_dependencies.contains(&predecessor)
            || self
                .persistent_full_range_admission_for_launch_v1(pending.launch.borrowed())
                .is_some()
            || self
                .three_binding_persistent_admission_for_launch_v1(pending.launch.borrowed())
                .is_some()
        {
            return Ok(false);
        }
        let expected_shape =
            dispatch_shape_sha256_v1(&pending.launch.borrowed(), pending.launch.semantic_launch);
        let active_predecessor_matches = self
            .active
            .as_ref()
            .filter(|active| active.id == predecessor)
            .is_some_and(|active| {
                active.stream == pending.launch.stream
                    && active.ordinary_recipe.as_deref().is_some_and(|recipe| {
                        ordinary_compute_recipes_match_v1(recipe, pending.launch.as_ref())
                    })
                    && active.dispatch_shape_sha256 == expected_shape
                    && self.ordered_predecessor_execution_matches_v1(active, None)
            });
        let pipelined_predecessor_matches =
            self.compute_pipeline
                .get(predecessor)
                .is_some_and(|active| {
                    let phase = self.compute_pipeline.phase(predecessor);
                    self.ordered_predecessor_execution_matches_v1(active, phase)
                        && active.stream == pending.launch.stream
                        && active.ordinary_recipe.as_deref().is_some_and(|recipe| {
                            ordinary_compute_recipes_match_v1(recipe, pending.launch.as_ref())
                        })
                        && active.dispatch_shape_sha256 == expected_shape
                });
        if !active_predecessor_matches && !pipelined_predecessor_matches {
            return Ok(false);
        }

        // Preparation still authenticates this exact logical invocation. It
        // deliberately does not reconcile or overwrite storage because the
        // retained immutable recipe is live on this physical queue.
        let prepared = match self.prepare_launch(pending.launch.borrowed(), false, true) {
            Ok(prepared) => prepared,
            Err(RuntimeBackendFailureV1::Rejected(_) | RuntimeBackendFailureV1::Quiescent(_)) => {
                return Ok(false);
            }
            Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => return Err(failure),
        };
        let ordinary_recipe = Arc::clone(&pending.launch);
        let PreparedLaunchV1 {
            stream,
            kernel,
            storage,
            allocations,
            writebacks,
            dispatch_shape_sha256,
            profile_launch,
            profile_semantic_contract,
            profile_bindings,
            mut performance,
            ..
        } = prepared;
        let PreparedLaunchStorageV1::Materialized(data) = storage else {
            return Ok(false);
        };
        let resident_descriptors = resident_descriptors_v1(&data)?;
        let recipe_still_matches = self
            .active
            .as_ref()
            .filter(|active| active.id == predecessor)
            .or_else(|| self.compute_pipeline.get(predecessor))
            .is_some_and(|active| {
                active.stream == stream
                    && active.kernel == kernel
                    && active.ordinary_recipe.as_deref().is_some_and(|recipe| {
                        ordinary_compute_recipes_match_v1(recipe, ordinary_recipe.as_ref())
                    })
                    && active.dispatch_shape_sha256 == dispatch_shape_sha256
                    && same_resident_storage_shape_v1(
                        &active.resident_descriptors,
                        &resident_descriptors,
                    )
            });
        if !recipe_still_matches || !self.compute_pipeline.has_successor_capacity() {
            return Ok(false);
        }
        for (index, writeback) in writebacks.iter().enumerate() {
            if writebacks[..index]
                .iter()
                .any(|prior| prior.allocation == writeback.allocation)
            {
                continue;
            }
            let required = writebacks[index..]
                .iter()
                .filter(|candidate| candidate.allocation == writeback.allocation)
                .count();
            self.allocations
                .get_mut(&writeback.allocation)
                .expect("prepared writeback allocation remains retained")
                .native_dirty
                .try_reserve(required)
                .map_err(|_| Self::capacity("KFD native-dirty extent reservation failed"))?;
        }

        performance.native_binding = Duration::ZERO;
        performance.data_path = KfdRuntimeLaunchDataPathV1::ResidentReused;
        performance.user_data_materializations = 0;
        let active = ActiveSubmissionV1 {
            source_event: Default::default(),
            id: pending.id,
            stream,
            ordered_predecessor: Some(predecessor),
            deferred_ordered_predecessor_retain: true,
            kernel,
            dependency_depth: pending.dependency_depth,
            allocations,
            writebacks,
            resident_descriptors,
            ordinary_recipe: Some(ordinary_recipe),
            dispatch_shape_sha256,
            published_at: Instant::now(),
            performance,
            execution: Some(ActiveComputeExecutionV1::MaterializedSuccessorPublication(
                super::ordered_publication::OrderedPublicationV1::new(
                    PersistentPublicationProfileV1 {
                        launch: profile_launch,
                        semantic_contract: profile_semantic_contract,
                        bindings: profile_bindings,
                    },
                ),
            )),
        };
        self.publish_indexed_ordered_successor_v1(pending, active)
    }

    pub(in crate::kfd_backend) fn pending_compute_can_publish_under_deadline_v1(
        &self,
        submission: u64,
    ) -> bool {
        let Some(pending) = self.pending_compute.get(&submission) else {
            return false;
        };
        if !pending.peer_gate_allows_native_checks_v1()
            || pending.explicit_dependency_cursor != pending.explicit_success_dependencies.len()
            || !pending.quiescence_complete_v1()
            || self.native_dirty_extents != 0
            || !pending.launch.bindings.iter().all(|binding| {
                self.allocations
                    .get(&binding.region.allocation)
                    .is_some_and(|allocation| {
                        !allocation.sdma_shadow_dirty && allocation.native_dirty.is_empty()
                    })
            })
        {
            return false;
        }
        if pending.ordered_predecessor.is_some()
            && (self
                .persistent_full_range_admission_for_launch_v1(pending.launch.borrowed())
                .is_some()
                || self
                    .three_binding_persistent_admission_for_launch_v1(pending.launch.borrowed())
                    .is_some())
        {
            return false;
        }
        true
    }
}
