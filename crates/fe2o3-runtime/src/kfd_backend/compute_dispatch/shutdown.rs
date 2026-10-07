use super::*;

impl KfdRuntimeBackendV1 {
    /// Returns phase timings for the latest successfully completed launch.
    pub const fn last_launch_performance_v1(&self) -> Option<KfdRuntimeLaunchPerformanceV1> {
        self.last_launch_performance
    }

    /// Returns the most recent successful authenticated H2D-ready promotion.
    ///
    /// This observation contains no allocation, queue, or native address. Its
    /// full ready-promotion duration remains included in caller-observed H2D time.
    pub const fn last_ready_promotion_performance_v1(
        &self,
    ) -> Option<KfdRuntimeReadyPromotionPerformanceV1> {
        self.last_ready_promotion_performance
    }

    /// Observes the queue-owned SDMA memory pool without changing custody.
    pub fn sdma_memory_pool_observation_v1(
        &self,
    ) -> Result<Gfx942SdmaMemoryPoolObservationV1, KfdRuntimeBackendErrorV1> {
        if self.terminal {
            return Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Terminal,
                "KFD backend is terminal",
            ));
        }
        if !self.native_available || !self.sdma_enabled {
            return Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "native KFD SDMA memory pool is unavailable",
            ));
        }
        self.queue
            .as_ref()
            .ok_or_else(|| {
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "enabled KFD SDMA pool lost its queue",
                )
            })?
            .sdma_memory_pool_observation()
            .map_err(|error| {
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Native,
                    format!("KFD SDMA memory-pool observation: {error}"),
                )
            })
    }

    /// Explicitly tears down the retained native queue after logical cleanup.
    ///
    /// Every logical stream must already be destroyed and no submission may
    /// be active. Failures after destructive native entry are terminal. A
    /// primary-custody allocation failure retains the live primary for retry;
    /// auxiliary queues already destroyed are not revisited.
    pub fn shutdown_native_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if self.has_live_generated_native_v1()
            || self.native_reconciliations.iter().any(Option::is_some)
            || !self.streams.is_empty()
            || !self.events.is_empty()
            || !self.event_submission_retain_counts.is_empty()
            || !self.submissions.is_empty()
            || !self.modules.is_empty()
            || !self.allocations.is_empty()
            || !self.pending_compute.is_empty()
            || self.terminal_pending_compute.is_some()
            || !self.pending_compute_streams.is_empty()
            || !self.allocation_custody.is_empty()
            || !self.compute_module_retain_counts.is_empty()
            || !self.compute_dependency_retain_counts.is_empty()
            || !self.stream_submission_tails.is_empty()
            || self.any_compute_active_v1()
            || !self.active_sdma.is_empty()
            || !self.published_sdma_submissions.is_empty()
            || !self.active_sdma_streams.is_empty()
            || !self.sdma_dependency_retain_counts.is_empty()
            || !self.quiescent_sdma_submissions.is_empty()
            || self.compute_completion_reservations != 0
            || self.sdma_completion_reservations != 0
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "logical runtime resources remain live",
            ));
        }
        // Multi-device shutdown may revisit a completed child after another
        // child rejects cleanup. Never re-enter native teardown after retirement.
        if self.queue_retired {
            return Ok(());
        }
        #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
        if self.cpu_queue.is_some() {
            return self.shutdown_cpu_queue_v1();
        }
        self.release_retained_persistent_control_v1()?;
        #[cfg(test)]
        if let Some(driver) = self.scripted_sdma.as_ref() {
            if !driver.is_exhausted()
                || driver.live_owner_count() != 0
                || driver.unexpected_drops() != 0
            {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "scripted directional SDMA custody or operations remain live",
                ));
            }
            self.native_available = false;
            self.sdma_enabled = false;
            self.queue_retired = true;
            return Ok(());
        }
        self.detach_recycled_dispatch()?;
        self.release_resident_data()?;
        for lane in 1..self.native_compute_lanes.len() {
            self.with_compute_lane_state_v1(lane, |backend| {
                backend.detach_recycled_dispatch()?;
                backend.release_resident_data()
            })?;
        }
        if self.sdma_enabled {
            self.trim_sdma_pool_for_shutdown_v1(|backend| {
                backend
                    .queue
                    .as_mut()
                    .expect("enabled SDMA pool retains queue")
                    .trim_sdma_memory_pool()
            })?;
        }
        for index in 0..self.native_compute_lanes.len() {
            let Some(native_lane) = self.native_compute_lanes[index] else {
                continue;
            };
            if native_lane.ordinal() == 0 {
                continue;
            }
            self.release_auxiliary_for_shutdown_v1(index, |backend| {
                backend
                    .queue
                    .as_mut()
                    .expect("auxiliary queue retains its shared owner")
                    .destroy_auxiliary_compute_lane_v1(native_lane)?;
                Ok(native_lane)
            })?;
        }
        let primary_logical_lane = self
            .native_compute_lanes
            .iter()
            .position(|lane| lane.is_some_and(|lane| lane.ordinal() == 0));
        let retained_primary = self.queue.as_ref().map_or(
            Ok(false),
            ComputeAqlQueueSessionV1::supports_retained_primary_release_v1,
        );
        let retained_primary = retained_primary.map_err(|error| {
            self.terminal_error(format!("primary KFD teardown profile: {error}"))
        })?;
        #[cfg(test)]
        super::retained_release_tests::observe_selection(retained_primary);
        if retained_primary {
            let preflight = self
                .queue
                .as_ref()
                .expect("selected primary queue")
                .preflight_primary_release_v1();
            if let Err(error) = preflight {
                return Err(
                    self.terminal_error(format!("primary KFD queue teardown preflight: {error}"))
                );
            }
            #[cfg(test)]
            let shell = if super::retained_release_tests::reject_primary_teardown_shell() {
                Err(())
            } else {
                try_uninit_box_v1::<PrimaryQueueReleaseCustodyV1>()
            };
            #[cfg(not(test))]
            let shell = try_uninit_box_v1::<PrimaryQueueReleaseCustodyV1>();
            let shell = shell.map_err(|_| {
                // Earlier dispatch/auxiliary cleanup may already have committed.
                // The primary queue is still retained and can retry shutdown.
                Self::quiescent_error(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "primary KFD teardown storage",
                )
            })?;
            let queue = self.queue.take().expect("preflight primary queue");
            self.install_and_release_primary_for_shutdown_v1(shell, queue, |owner| {
                #[cfg(all(test, feature = "hardware-qualification"))]
                {
                    super::retained_release_tests::primary_envelope::release_or_fault(owner)
                }
                #[cfg(not(all(test, feature = "hardware-qualification")))]
                {
                    owner.release_in_place()
                }
            })?;
            self.observe_destroyed_compute_lane_v1(primary_logical_lane);
        } else if let Some(queue) = self.queue.take() {
            queue.destroy().map_err(|error| {
                self.terminal_error(format!("explicit KFD queue teardown: {error}"))
            })?;
            self.observe_destroyed_compute_lane_v1(primary_logical_lane);
        }
        self.admitted_device.take();
        self.native_compute_lanes.fill(None);
        self.queue_retired = true;
        Ok(())
    }

    pub(super) fn install_and_release_primary_for_shutdown_v1(
        &mut self,
        shell: Box<MaybeUninit<PrimaryQueueReleaseCustodyV1>>,
        queue: ComputeAqlQueueSessionV1,
        release: impl FnOnce(
            &mut PrimaryQueueReleaseCustodyV1,
        ) -> Result<
            fe2o3_kfd::ComputeAqlQueueDestroyedV1,
            fe2o3_kfd::ComputeAqlQueueSessionErrorV1,
        >,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.primary_teardown = Some(fill_restore_shell_v1(
            shell,
            PrimaryQueueReleaseCustodyV1::new(queue),
        ));
        #[cfg(test)]
        super::retained_release_tests::observe_primary_host_usage(
            self.primary_teardown
                .as_ref()
                .expect("installed primary teardown"),
            false,
        );
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            release(
                self.primary_teardown
                    .as_mut()
                    .expect("installed primary teardown"),
            )
        }));
        match result {
            Ok(Ok(_)) => {
                #[cfg(test)]
                super::retained_release_tests::observe_primary_host_usage(
                    self.primary_teardown
                        .as_ref()
                        .expect("completed primary teardown"),
                    true,
                );
                self.primary_teardown.take();
                Ok(())
            }
            Ok(Err(error)) => {
                Err(self.terminal_error(format!("retained primary KFD teardown: {error}")))
            }
            Err(payload) => {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.terminal_error("retained primary KFD teardown panicked")
                }));
                std::panic::resume_unwind(payload);
            }
        }
    }

    pub(in crate::kfd_backend) fn release_auxiliary_for_shutdown_v1(
        &mut self,
        index: usize,
        release: impl FnOnce(
            &mut Self,
        )
            -> Result<ComputeAqlQueueLaneV1, fe2o3_kfd::ComputeAqlQueueSessionErrorV1>,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let destroyed = release(self).map_err(|error| {
                self.terminal_error(format!("retained auxiliary KFD queue teardown: {error}"))
            })?;
            if self.native_compute_lanes.get(index).copied().flatten() != Some(destroyed) {
                return Err(
                    self.terminal_error("retained auxiliary KFD queue slot changed after teardown")
                );
            }
            // Later primary teardown allocation may fail quiescently and allow a retry.
            self.native_compute_lanes[index] = None;
            self.observe_destroyed_compute_lane_v1(Some(index));
            Ok(())
        }));
        match result {
            Ok(result) => result,
            Err(payload) => {
                core::mem::forget(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || self.terminal_error("retained auxiliary KFD queue teardown panicked"),
                )));
                std::panic::resume_unwind(payload)
            }
        }
    }

    pub(in crate::kfd_backend) fn trim_sdma_pool_for_shutdown_v1(
        &mut self,
        trim: impl FnOnce(&mut Self) -> Result<usize, fe2o3_kfd::ComputeAqlQueueSessionErrorV1>,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| trim(self))) {
            Ok(result) => result.map(|_| ()).map_err(|error| {
                self.terminal_error(format!("KFD SDMA memory-pool trim: {error}"))
            }),
            Err(payload) => {
                core::mem::forget(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || self.terminal_error("KFD SDMA memory-pool trim panicked"),
                )));
                std::panic::resume_unwind(payload)
            }
        }
    }

    pub(in crate::kfd_backend) fn observe_destroyed_compute_lane_v1(
        &mut self,
        logical_lane: Option<usize>,
    ) {
        // An SDMA bootstrap queue may never serve a logical compute lane and
        // therefore has no matching creation event in the compute profile.
        let Some(lane) = logical_lane else {
            return;
        };
        let queue = self.profile_resource_v1(
            KfdProfileResourceKindV1::NativeQueue,
            KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + lane as u64,
        );
        self.observe_profile_v1(
            queue.map(|queue| KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue }),
        );
    }

    pub(in crate::kfd_backend) fn synchronize_recycled_dispatch_data_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_unpinned_native_lane_v1(self.selected_compute_lane)?;
        if self.recycled_dispatch.is_some() {
            let lane = self.selected_compute_lane;
            let mut dirty = Vec::new();
            dirty
                .try_reserve_exact(self.allocations.len())
                .map_err(|_| Self::capacity("KFD native-dirty synchronization roster failed"))?;
            dirty.extend(
                self.allocations
                    .ordinary_iter()
                    .filter_map(|(allocation, record)| {
                        record
                            .native_dirty
                            .iter()
                            .any(|extent| extent.compute_lane == lane)
                            .then_some(*allocation)
                    }),
            );
            for allocation in dirty {
                self.synchronize_native_allocation_lane_v1(allocation, lane)?;
            }
        }
        Ok(())
    }
}
