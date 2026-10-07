use super::*;

impl KfdRuntimeBackendV1 {
    pub(super) fn compute_dependency_submission_v1(
        &self,
        event_handle: u64,
        expected_submission: Option<u64>,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let event = self.events.get(&event_handle).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD event dependency",
            )
        })?;
        if expected_submission.is_some_and(|expected| expected != event.submission) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "KFD event dependency does not name the expected producer",
            ));
        }
        let status = self
            .submissions
            .get(&event.submission)
            .map(|record| record.status)
            .or_else(|| {
                (self.active_compute_lane_v1(event.submission).is_some()
                    || self.pending_compute.contains_key(&event.submission)
                    || self.active_sdma.contains_key(&event.submission))
                .then_some(BackendPollV1::Pending)
            });
        match status {
            Some(BackendPollV1::Succeeded | BackendPollV1::Pending) => Ok(event.submission),
            Some(BackendPollV1::Failed { .. }) => Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "event dependency completed with failure",
            )),
            None => Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "event refers to an unknown submission",
            )),
        }
    }

    pub(in crate::kfd_backend) fn collect_exact_compute_dependencies_v1(
        &self,
        dependencies: &[BackendLaunchProducerV1],
    ) -> Result<Box<[u64]>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(Self::capacity("KFD compute dependency capacity exceeded"));
        }
        let mut submissions = Vec::new();
        submissions
            .try_reserve_exact(dependencies.len())
            .map_err(|_| Self::capacity("KFD compute dependency allocation failed"))?;
        for dependency in dependencies {
            let submission = self.compute_dependency_submission_v1(
                dependency.event,
                Some(dependency.producer_submission),
            )?;
            if submissions.contains(&submission) {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "KFD compute dependencies must name distinct submissions",
                ));
            }
            submissions.push(submission);
        }
        Ok(submissions.into_boxed_slice())
    }

    pub(in crate::kfd_backend) fn collect_compute_dependencies_v1(
        &self,
        dependencies: &[u64],
    ) -> Result<Box<[u64]>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(Self::capacity("KFD compute dependency capacity exceeded"));
        }
        let mut submissions = Vec::new();
        submissions
            .try_reserve_exact(dependencies.len())
            .map_err(|_| Self::capacity("KFD compute dependency allocation failed"))?;
        for event_handle in dependencies {
            let submission = self.compute_dependency_submission_v1(*event_handle, None)?;
            if submissions.contains(&submission) {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "KFD compute dependencies must name distinct submissions",
                ));
            }
            submissions.push(submission);
        }
        Ok(submissions.into_boxed_slice())
    }

    #[cfg(test)]
    pub(in crate::kfd_backend) fn validate_compute_launch_v1(
        &self,
        launch: &BackendLaunchV1<'_>,
        dependencies: &[u64],
        input_admission: ComputeInputAdmissionV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.validate_compute_launch_base_v1(launch)?;
        self.validate_compute_launch_with_peer_v1(
            launch,
            dependencies,
            None,
            &[],
            input_admission,
            &peer_compute_access::PeerDmaAdmissionsV1::default(),
        )
    }

    pub(in crate::kfd_backend) fn validate_compute_launch_base_v1(
        &self,
        launch: &BackendLaunchV1<'_>,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
        if self.cpu_queue.is_some() {
            self.require_cpu_provider_v1()?;
            if launch.bindings.iter().any(|binding| {
                binding.region.access != RuntimeAccessV1::Read
                    || !self
                        .allocations
                        .get(&binding.region.allocation)
                        .is_some_and(|record| {
                            record.kind == RuntimeMemoryKindV1::HostVisible
                                && matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::Synthetic)
                        })
            }) {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    "CPU receipt fixture admits only read-only synthetic host bindings",
                ));
            }
        }
        if launch.explicit_kernarg.len() > MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1 {
            return Err(Self::capacity(
                "KFD explicit kernarg exceeds the runtime admission bound",
            ));
        }
        if launch.bindings.len() > fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1 {
            return Err(Self::capacity(
                "KFD binding roster exceeds the host dispatch admission bound",
            ));
        }
        let stream_device = *self.streams.get(&launch.stream).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD stream",
            )
        })?;
        let kernel = self.kernels.get(&launch.kernel).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD kernel",
            )
        })?;
        let module = self.modules.get(&kernel.module).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "kernel module is no longer loaded",
            )
        })?;
        if module.device != stream_device {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "stream and kernel belong to different devices",
            ));
        }
        for binding in launch.bindings {
            let allocation = self.allocations.get(&binding.region.allocation);
            if allocation
                .is_some_and(|allocation| allocation.kind != RuntimeMemoryKindV1::HostVisible)
            {
                self.require_default_dispatch_capacity_v1()?;
            }
            if !native_sdma_region_is_admitted_v1(allocation, stream_device, binding.region)
                || binding.region.byte_len == 0
            {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "KFD compute binding exceeds its retained allocation",
                ));
            }
        }
        Ok(())
    }

    pub(in crate::kfd_backend) fn validate_compute_launch_with_peer_v1(
        &self,
        launch: &BackendLaunchV1<'_>,
        dependencies: &[u64],
        ordered: Option<u64>,
        quiescence: &[u64],
        input_admission: ComputeInputAdmissionV1,
        peer_dma: &peer_compute_access::PeerDmaAdmissionsV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if three_binding_requires_persistent_admission_v1(
            launch.semantic_launch,
            launch.bindings,
            &self.allocations,
        ) && self
            .three_binding_persistent_admission_for_launch_v1(*launch)
            .is_none()
            && !self.three_binding_storage_candidates_admissible_v1(*launch)
            && (input_admission != ComputeInputAdmissionV1::ExactProducers
                || !self.three_binding_producer_bindings_are_deferred_v1(
                    *launch,
                    dependencies,
                    ordered,
                    quiescence,
                    peer_dma,
                ))
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "three-binding persistent-compute candidate failed exact R/R/W admission",
            ));
        }

        if launch.bindings.iter().any(|binding| {
            self.allocation_custody
                .get(&binding.region.allocation)
                .is_some_and(|custody| {
                    custody.owners.iter().any(|owner| match owner.kind {
                        RuntimeAllocationCustodyKindV1::Compute => {
                            owner.stream != launch.stream
                                && !dependencies.contains(&owner.submission)
                                && quiescence.binary_search(&owner.submission).is_err()
                        }
                        RuntimeAllocationCustodyKindV1::Sdma => {
                            !peer_dma.authorizes(binding.region.allocation, owner.submission)
                                && !(self
                                    .active_sdma
                                    .get(&owner.submission)
                                    .is_some_and(|active| active.peer_access.is_none())
                                    && (dependencies.contains(&owner.submission)
                                        || ordered == Some(owner.submission)
                                        || quiescence.binary_search(&owner.submission).is_ok()))
                        }
                    })
                })
        }) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "overlapping cross-stream compute/copy requires an explicit event dependency",
            ));
        }
        Ok(())
    }

    pub(super) fn three_binding_producer_bindings_are_deferred_v1(
        &self,
        launch: BackendLaunchV1<'_>,
        dependencies: &[u64],
        ordered: Option<u64>,
        quiescence: &[u64],
        peer_dma: &peer_compute_access::PeerDmaAdmissionsV1,
    ) -> bool {
        let Some(&device) = self.streams.get(&launch.stream) else {
            return false;
        };
        if !three_binding_persistent_compute_shape_v1(
            launch.semantic_launch,
            launch.bindings,
            device,
            &self.allocations,
        ) {
            return false;
        }
        let candidates = self.initialized_storage_candidates_v1(launch);
        launch.bindings.iter().all(|binding| {
            let allocation = &self.allocations[&binding.region.allocation];
            if three_binding_persistent_ready_source_v1(allocation).is_some()
                || candidates.contains(&Some(binding.region.allocation))
                || matches!(allocation.sdma_storage, KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(owner)) if peer_dma.authorizes(binding.region.allocation, owner))
            {
                return true;
            }
            if let KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(owner)) =
                allocation.sdma_storage
            {
                let Some(active) = self.active_sdma.get(&owner) else {
                    return false;
                };
                // Waiting eligibility is not a ready-storage receipt. Completion must
                // restore the actual endpoint before publication rechecks admission.
                return (dependencies.contains(&owner)
                    || ordered == Some(owner)
                    || quiescence.binary_search(&owner).is_ok())
                    && (ordered != Some(owner) || active.stream == launch.stream)
                    && self.native_sdma_binding_owner_intact_v1(
                        binding.region.allocation,
                        RuntimeAllocationCustodyOwnerV1 {
                            submission: owner,
                            stream: active.stream,
                            kind: RuntimeAllocationCustodyKindV1::Sdma,
                        },
                    );
            }
            let KfdRuntimeSdmaStorageV1::ComputeInFlight(owner) = &allocation.sdma_storage else {
                return false;
            };
            // The exact R/R/W shape includes a full-allocation overwrite. An output
            // may wait for its owner too; this grants neither success nor ready backing.
            if !(dependencies.contains(owner) || ordered == Some(*owner)
                || quiescence.binary_search(owner).is_ok()) {
                return false;
            }
            let Some(active) = self.active_compute_submission_v1(*owner) else {
                return false;
            };
            if active.id != *owner || self.streams.get(&active.stream) != Some(&device)
                || ordered == Some(*owner) && active.stream != launch.stream {
                return false;
            }
            let admissions = match active.execution.as_ref() {
                Some(
                    ActiveComputeExecutionV1::ThreeBindingPersistentPrepared { admissions, .. }
                    | ActiveComputeExecutionV1::ThreeBindingPersistent { admissions, .. },
                ) => admissions,
                Some(ActiveComputeExecutionV1::ThreeBindingPersistentCompleting(root)) => &root.admissions,
                #[cfg(test)]
                Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistent {
                    admissions,
                    ..
                } | ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared { admissions, .. }) => admissions,
                _ => return false,
            };
            // These receipts establish eligibility to wait, never ready backing.
            active.allocations.len() == admissions.len()
                && active.allocations.contains(&binding.region.allocation)
                && admissions.iter().all(|admission| {
                    active.allocations.contains(&admission.allocation)
                        && self.allocation_retains_exact_owner_v1(
                            admission.allocation,
                            RuntimeAllocationCustodyOwnerV1 {
                                submission: *owner,
                                stream: active.stream,
                                kind: RuntimeAllocationCustodyKindV1::Compute,
                            },
                        )
                })
                && active.allocations.iter().all(|allocation| {
                    admissions
                        .iter()
                        .any(|admission| admission.allocation == *allocation)
                })
        })
    }
}
