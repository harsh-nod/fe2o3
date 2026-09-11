use super::*;

pub(super) fn three_binding_persistent_compute_access_shape_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    bindings: &[BackendBindingV1],
) -> bool {
    let [a, b, c] = bindings else {
        return false;
    };
    semantic_launch == KfdRuntimeSemanticLaunchV1::Ordinary
        && [a.region.access, b.region.access, c.region.access]
            == [
                RuntimeAccessV1::Read,
                RuntimeAccessV1::Read,
                RuntimeAccessV1::Write,
            ]
}

pub(super) fn three_binding_requires_persistent_admission_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    bindings: &[BackendBindingV1],
    allocations: &HashMap<u64, AllocationRecordV1>,
) -> bool {
    // Only fully resolved host-visible rosters may use ordinary materialization.
    // Mixed or device-local candidates still require authenticated persistence.
    three_binding_persistent_compute_access_shape_v1(semantic_launch, bindings)
        && !bindings.iter().all(|binding| {
            allocations
                .get(&binding.region.allocation)
                .is_some_and(|allocation| allocation.kind == RuntimeMemoryKindV1::HostVisible)
        })
}

pub(super) fn three_binding_persistent_compute_admission_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    bindings: &[BackendBindingV1],
    stream_device: u64,
    allocations: &HashMap<u64, AllocationRecordV1>,
) -> Option<ThreeBindingPersistentComputeAdmissionV1> {
    let [a, b, c] = bindings else {
        return None;
    };
    if a.region.byte_len != b.region.byte_len || a.region.byte_len != c.region.byte_len {
        return None;
    }
    if [a.region.access, b.region.access, c.region.access]
        != [
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Write,
        ]
        || a.region.allocation == b.region.allocation
        || a.region.allocation == c.region.allocation
        || b.region.allocation == c.region.allocation
        || semantic_launch != KfdRuntimeSemanticLaunchV1::Ordinary
    {
        return None;
    }
    let admit = |index: usize, binding: &BackendBindingV1| {
        let allocation = allocations.get(&binding.region.allocation)?;
        let logical_bytes = u64::try_from(allocation.bytes.len()).ok()?;
        let full_extent = allocation.device == stream_device
            && allocation.kind == RuntimeMemoryKindV1::DeviceLocal
            && allocation.sdma_backed
            && allocation.sdma_initialized
            && allocation.native_dirty.is_empty()
            && logical_bytes != 0
            && binding.region.byte_offset == 0
            && binding.region.byte_len == logical_bytes;
        if !full_extent {
            return None;
        }
        let source = match (&allocation.sdma_storage, index) {
            (KfdRuntimeSdmaStorageV1::H2dReady(ready), 0..=2)
                if !allocation.sdma_shadow_dirty
                    && allocation.content_sha256 == Some(ready.owner.authenticated_sha256())
                    && ready.owner.byte_len() == logical_bytes
                    && ready.owner.physical_byte_len() == logical_bytes =>
            {
                PersistentFullRangeComputeSourceV1::AuthenticatedH2d
            }
            (KfdRuntimeSdmaStorageV1::PersistentReplay(input), 0..=2)
                if input.is_fully_initialized() =>
            {
                PersistentFullRangeComputeSourceV1::RetainedControlReplay
            }
            #[cfg(test)]
            (KfdRuntimeSdmaStorageV1::Device(_), 0..=2)
                if allocation.scripted_three_binding_replay =>
            {
                PersistentFullRangeComputeSourceV1::RetainedControlReplay
            }
            _ => return None,
        };
        Some(PersistentFullRangeComputeAdmissionV1 {
            allocation: binding.region.allocation,
            access: binding.region.access,
            source,
        })
    };
    Some(ThreeBindingPersistentComputeAdmissionV1 {
        bindings: [admit(0, a)?, admit(1, b)?, admit(2, c)?],
    })
}
pub(super) fn persistent_control_is_reused_v1(
    retained: Option<RetainedPersistentDispatchV1>,
    admission: Option<PersistentFullRangeComputeAdmissionV1>,
    dispatch_shape_sha256: [u8; 32],
) -> bool {
    retained
        .zip(admission)
        .is_some_and(|(retained, admission)| {
            retained.allocation == admission.allocation
                && retained.dispatch_shape_sha256 == dispatch_shape_sha256
        })
}

pub(super) fn persistent_full_range_compute_admission_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    bindings: &[BackendBindingV1],
    stream_device: u64,
    allocation: Option<&AllocationRecordV1>,
    ready: Option<PersistentComputeReadyFactsV1>,
) -> Option<PersistentFullRangeComputeAdmissionV1> {
    let [binding] = bindings else {
        return None;
    };
    let allocation = allocation?;
    let ready = ready?;
    let logical_bytes = u64::try_from(allocation.bytes.len()).ok()?;
    let max_window_bytes = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1)
        .checked_mul(u64::try_from(KFD_RUNTIME_MAX_SDMA_WINDOW_PACKETS_V1).ok()?)?;
    (semantic_launch == KfdRuntimeSemanticLaunchV1::Ordinary
        && allocation.device == stream_device
        && allocation.kind == RuntimeMemoryKindV1::DeviceLocal
        && allocation.sdma_backed
        && allocation.sdma_initialized
        && allocation.native_dirty.is_empty()
        && !allocation.sdma_shadow_dirty
        && allocation.content_sha256 == Some(ready.authenticated_sha256)
        && logical_bytes != 0
        && logical_bytes <= max_window_bytes
        && logical_bytes.is_multiple_of(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1)
        && ready.logical_bytes == logical_bytes
        && ready.physical_bytes == logical_bytes
        && binding.region.byte_offset == 0
        && binding.region.byte_len == logical_bytes)
        .then_some(PersistentFullRangeComputeAdmissionV1 {
            allocation: binding.region.allocation,
            access: binding.region.access,
            source: PersistentFullRangeComputeSourceV1::AuthenticatedH2d,
        })
}

pub(super) fn retained_persistent_full_range_compute_admission_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    bindings: &[BackendBindingV1],
    stream_device: u64,
    allocation: Option<&AllocationRecordV1>,
    retained: RetainedPersistentDispatchV1,
    dispatch_shape_sha256: [u8; 32],
) -> Option<PersistentFullRangeComputeAdmissionV1> {
    let [binding] = bindings else {
        return None;
    };
    let allocation = allocation?;
    let logical_bytes = u64::try_from(allocation.bytes.len()).ok()?;
    let initialized = match &allocation.sdma_storage {
        KfdRuntimeSdmaStorageV1::PersistentReplay(input) => input.is_fully_initialized(),
        #[cfg(test)]
        KfdRuntimeSdmaStorageV1::Device(_) => true,
        _ => return None,
    };
    (semantic_launch == KfdRuntimeSemanticLaunchV1::Ordinary
        && retained.allocation == binding.region.allocation
        && retained.dispatch_shape_sha256 == dispatch_shape_sha256
        && allocation.device == stream_device
        && allocation.kind == RuntimeMemoryKindV1::DeviceLocal
        && allocation.sdma_backed
        && allocation.sdma_initialized
        && allocation.native_dirty.is_empty()
        && logical_bytes != 0
        && binding.region.byte_offset == 0
        && binding.region.byte_len == logical_bytes
        && (binding.region.access == RuntimeAccessV1::Write || initialized))
        .then_some(PersistentFullRangeComputeAdmissionV1 {
            allocation: binding.region.allocation,
            access: binding.region.access,
            source: PersistentFullRangeComputeSourceV1::RetainedControlReplay,
        })
}

pub(super) fn admitted_compute_lane_v1(
    available_lane: Option<usize>,
    persistent_selected: bool,
) -> Option<usize> {
    if persistent_selected {
        available_lane.filter(|lane| *lane == 0)
    } else {
        available_lane
    }
}

pub(super) fn early_pipeline_access_is_admitted_v1(bindings: &[BackendBindingV1]) -> bool {
    bindings.iter().all(|binding| {
        binding.region.access != RuntimeAccessV1::ReadWrite
            && !bindings.iter().any(|other| {
                other.region.allocation == binding.region.allocation
                    && other.region.access != binding.region.access
            })
    })
}

pub(super) fn early_pipeline_launch_is_admitted_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    bindings: &[BackendBindingV1],
) -> bool {
    semantic_launch == KfdRuntimeSemanticLaunchV1::Ordinary
        && early_pipeline_access_is_admitted_v1(bindings)
}

#[cfg(test)]
pub(super) const fn explicit_dependency_succeeded_v1(status: BackendPollV1) -> bool {
    matches!(status, BackendPollV1::Succeeded)
}

pub(super) const fn ordered_predecessor_completed_v1(status: BackendPollV1) -> bool {
    !matches!(status, BackendPollV1::Pending)
}

pub(super) const fn ordered_successor_lane_matches_v1(
    stream_lane: Option<usize>,
    predecessor_lane: usize,
) -> bool {
    matches!(stream_lane, Some(lane) if lane == predecessor_lane)
}

pub(super) fn ordinary_compute_recipes_match_v1(
    left: &OwnedComputeLaunchV1,
    right: &OwnedComputeLaunchV1,
) -> bool {
    left == right
}

pub(super) fn retain_unique_native_dirty_extent_v1(
    dirty: &mut Vec<NativeDirtyExtentV1>,
    extent: NativeDirtyExtentV1,
) -> bool {
    if dirty.contains(&extent) {
        false
    } else {
        dirty.push(extent);
        true
    }
}

pub(super) fn apply_persistent_compute_effect_v1(
    record: &mut AllocationRecordV1,
    effect: Gfx942PersistentComputeEffectV1,
) {
    if effect.writes() {
        record.content_sha256 = None;
        record.last_full_host_write = None;
        record.sdma_shadow_dirty = true;
    }
}

pub(super) const fn persistent_compute_effect_v1(
    access: RuntimeAccessV1,
) -> Gfx942PersistentComputeEffectV1 {
    match access {
        RuntimeAccessV1::Read => Gfx942PersistentComputeEffectV1::Read,
        RuntimeAccessV1::Write => Gfx942PersistentComputeEffectV1::Write,
        RuntimeAccessV1::ReadWrite => Gfx942PersistentComputeEffectV1::ReadWrite,
    }
}
pub(super) fn recycled_dispatch_reuse_is_admitted_v1(
    recycled: &RecycledDispatchV1,
    dispatch_shape_sha256: [u8; 32],
    resident_descriptors: &[ResidentDataDescriptorV1],
    data: &[DataSpecV1],
) -> bool {
    recycled.dispatch_shape_sha256 == dispatch_shape_sha256
        && same_resident_storage_shape_v1(&recycled.descriptors, resident_descriptors)
        && data
            .iter()
            .all(|spec| spec.kind == RuntimeMemoryKindV1::HostVisible)
}

pub(super) fn host_visible_resident_roster_is_reusable_v1(
    descriptors: &[ResidentDataDescriptorV1],
    native_data_count: usize,
) -> bool {
    descriptors.len() == native_data_count
        && host_visible_resident_descriptors_are_reusable_v1(descriptors)
}

pub(super) fn host_visible_resident_descriptors_are_reusable_v1(
    descriptors: &[ResidentDataDescriptorV1],
) -> bool {
    !descriptors.is_empty()
        && descriptors.iter().all(|descriptor| {
            descriptor.kind == RuntimeMemoryKindV1::HostVisible && descriptor.byte_len != 0
        })
}

pub(super) fn resident_data_needs_host_overwrite_v1(
    prior: &ResidentDataDescriptorV1,
    current_host_sha256: Option<[u8; 32]>,
) -> bool {
    prior.device_may_have_modified
        || prior.host_content_sha256.is_none()
        || prior.host_content_sha256 != current_host_sha256
}

impl KfdRuntimeBackendV1 {
    pub(super) fn collect_compute_dependencies_v1(
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
            let event = self.events.get(event_handle).ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown KFD event dependency",
                )
            })?;
            if submissions.contains(&event.submission) {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "KFD compute dependencies must name distinct submissions",
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
                Some(BackendPollV1::Succeeded | BackendPollV1::Pending) => {}
                Some(BackendPollV1::Failed { .. }) => {
                    return Err(Self::rejected(
                        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                        "event dependency completed with failure",
                    ));
                }
                None => {
                    return Err(Self::rejected(
                        KfdRuntimeBackendErrorKindV1::UnknownHandle,
                        "event refers to an unknown submission",
                    ));
                }
            }
            submissions.push(event.submission);
        }
        Ok(submissions.into_boxed_slice())
    }

    pub(super) fn validate_compute_launch_v1(
        &self,
        launch: &BackendLaunchV1<'_>,
        dependencies: &[u64],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
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
            if !native_sdma_region_is_admitted_v1(
                self.allocations.get(&binding.region.allocation),
                stream_device,
                binding.region,
            ) || binding.region.byte_len == 0
            {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "KFD compute binding exceeds its retained allocation",
                ));
            }
        }
        if three_binding_requires_persistent_admission_v1(
            launch.semantic_launch,
            launch.bindings,
            &self.allocations,
        ) && self
            .three_binding_persistent_admission_for_launch_v1(*launch)
            .is_none()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "three-binding persistent-compute candidate failed exact R/R/W admission",
            ));
        }

        if launch.bindings.iter().any(|binding| {
            self.allocation_has_unordered_custody_v1(
                binding.region.allocation,
                launch.stream,
                dependencies,
                None,
            )
        }) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "overlapping cross-stream compute/copy requires an explicit event dependency",
            ));
        }
        Ok(())
    }

    pub(super) fn finish_persistent_compute_poll_and_recycle_v1(
        &mut self,
        mut active: ActiveSubmissionV1,
        allocation: u64,
        access: RuntimeAccessV1,
        poll: Result<
            Gfx942PersistentComputePollAndRecycleV1,
            Gfx942PersistentComputePollAndRecycleFailureV1,
        >,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let poll = match poll {
            Ok(poll) => poll,
            Err(Gfx942PersistentComputePollAndRecycleFailureV1::Poll(failure)) => {
                let detail = failure.error().to_string();
                let (_, custody) = failure.into_parts();
                return match custody {
                    Gfx942PersistentComputeTransitionFailureCustodyV1::Retryable(dispatch) => {
                        self.retain_terminal_sdma_custody_v1(
                            KfdRuntimeTerminalSdmaCustodyV1::PersistentComputePublished(dispatch),
                        );
                        Err(self.terminal_error(format!(
                            "KFD persistent-compute completion observation returned foreign retryable custody: {detail}"
                        )))
                    }
                    Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(custody) => {
                        self.retain_terminal_sdma_custody_v1(
                            KfdRuntimeTerminalSdmaCustodyV1::PersistentCompute(custody),
                        );
                        Err(self.terminal_error(format!(
                            "KFD persistent-compute completion observation: {detail}"
                        )))
                    }
                };
            }
            Err(Gfx942PersistentComputePollAndRecycleFailureV1::Recycle(failure)) => {
                let detail = failure.error().to_string();
                let (_, custody) = failure.into_parts();
                return match custody {
                    Gfx942PersistentComputeTransitionFailureCustodyV1::Retryable(completed) => {
                        self.retain_terminal_sdma_custody_v1(
                            KfdRuntimeTerminalSdmaCustodyV1::PersistentComputeCompleted(completed),
                        );
                        Err(self.terminal_error(format!(
                            "KFD persistent-compute completion recycle returned foreign retryable custody: {detail}"
                        )))
                    }
                    Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(custody) => {
                        self.retain_terminal_sdma_custody_v1(
                            KfdRuntimeTerminalSdmaCustodyV1::PersistentCompute(custody),
                        );
                        Err(self.terminal_error(format!(
                            "KFD persistent-compute completion recycle: {detail}"
                        )))
                    }
                };
            }
        };
        match poll {
            Gfx942PersistentComputePollAndRecycleV1::Pending(dispatch) => {
                active.execution = Some(ActiveComputeExecutionV1::Persistent {
                    allocation,
                    access,
                    dispatch,
                });
                self.active = Some(active);
                Ok(BackendPollV1::Pending)
            }
            Gfx942PersistentComputePollAndRecycleV1::Recycled {
                recycled,
                completion_observed_at,
            } => {
                active.performance.publish_to_completion =
                    completion_observed_at.saturating_duration_since(active.published_at);
                let completion_signal_recycle = completion_observed_at.elapsed();
                active.performance.completion_signal_recycle += completion_signal_recycle;
                self.finish_persistent_full_range_recycled_v1(
                    active,
                    allocation,
                    access,
                    recycled,
                    completion_observed_at,
                    completion_signal_recycle,
                )
            }
        }
    }

    pub(super) fn finish_three_binding_persistent_poll_and_recycle_v1(
        &mut self,
        mut active: ActiveSubmissionV1,
        admissions: [PersistentFullRangeComputeAdmissionV1; 3],
        restore_shells: [ThreeBindingPersistentRestoreShellV1; 3],
        poll: Result<
            Gfx942ThreeBindingPersistentComputePollAndRecycleV1,
            Gfx942ThreeBindingPersistentComputePollAndRecycleFailureV1,
        >,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let poll = match poll {
            Ok(poll) => poll,
            Err(failure) => {
                let (error, recovered) = failure.into_parts();
                if let Some(dispatch) = recovered {
                    self.retain_terminal_sdma_custody_v1(
                        KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentComputePublished(
                            dispatch,
                        ),
                    );
                }
                let detail = error.to_string();
                return Err(self.terminal_error(format!(
                    "KFD three-binding persistent completion/recycle: {detail}"
                )));
            }
        };
        match poll {
            Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Pending(dispatch) => {
                active.execution = Some(ActiveComputeExecutionV1::ThreeBindingPersistent {
                    admissions,
                    restore_shells,
                    dispatch,
                });
                self.active = Some(active);
                Ok(BackendPollV1::Pending)
            }
            Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Recycled {
                recycled,
                completion_observed_at,
            } => {
                active.performance.publish_to_completion =
                    completion_observed_at.saturating_duration_since(active.published_at);
                let completion_signal_recycle = completion_observed_at.elapsed();
                active.performance.completion_signal_recycle += completion_signal_recycle;
                let recycle_started = completion_observed_at;
                let detach = self
                    .queue
                    .as_mut()
                    .expect("three-binding recycled completion retains its queue")
                    .detach_recycled_three_binding_directional_persistent_fixed_dispatch_v1(
                        recycled,
                    );
                let completed = match detach {
                    Ok(completed) => completed,
                    Err(failure) => {
                        let (error, recovered) = failure.into_parts();
                        if let Some(recycled) = recovered {
                            self.retain_terminal_sdma_custody_v1(
                                KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentComputeRecycled(
                                    recycled,
                                ),
                            );
                        }
                        let detail = error.to_string();
                        active.performance.completion_detach_restore +=
                            completion_detach_restore_duration_v1(
                                recycle_started.elapsed(),
                                completion_signal_recycle,
                            );
                        return Err(self.terminal_error(format!(
                            "KFD three-binding persistent completion detach: {detail}"
                        )));
                    }
                };
                let completed = match completed.retire_settled_frontiers_for_replay_v1() {
                    Ok(completed) => completed,
                    Err(completed) => {
                        self.retain_terminal_sdma_custody_v1(
                            KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentComputeCompleted(
                                completed,
                            ),
                        );
                        return Err(self.terminal_error(
                            "KFD three-binding persistent frontier retirement failed",
                        ));
                    }
                };
                let effects = std::array::from_fn(|index| completed[index].1);
                let inputs =
                    completed.map(|(input, _)| KfdRuntimePersistentComputeInputV1::Native(input));
                let expected =
                    admissions.map(|admission| persistent_compute_effect_v1(admission.access));
                if effects != expected {
                    self.retain_terminal_sdma_custody_v1(
                        KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentInputs(inputs),
                    );
                    return Err(self.terminal_error(
                        "KFD three-binding persistent effects changed after admission",
                    ));
                }
                self.restore_three_binding_persistent_inputs_v1(
                    admissions,
                    active.id,
                    inputs,
                    [None; 3],
                    restore_shells,
                )?;
                for (admission, effect) in admissions.into_iter().zip(effects) {
                    let record = self
                        .allocations
                        .get_mut(&admission.allocation)
                        .expect("restored three-binding allocation remains indexed");
                    apply_persistent_compute_effect_v1(record, effect);
                    debug_assert!(record.native_dirty.is_empty());
                }
                let detach_restore = completion_detach_restore_duration_v1(
                    recycle_started.elapsed(),
                    completion_signal_recycle,
                );
                self.finish_restored_three_binding_persistent_compute_v1(active, detach_restore)
            }
        }
    }

    #[cfg(test)]
    pub(super) fn finish_scripted_three_binding_persistent_compute_v1(
        &mut self,
        active: ActiveSubmissionV1,
        admissions: [PersistentFullRangeComputeAdmissionV1; 3],
        restore_shells: [ThreeBindingPersistentRestoreShellV1; 3],
        devices: [DirectionalSdmaDeviceOwnerV1; 3],
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let slots_current = admissions.iter().all(|admission| {
            self.allocations.get(&admission.allocation).is_some_and(|record| {
                matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == active.id)
            })
        });
        if !slots_current {
            self.retain_terminal_sdma_custody_v1(
                KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentInputs(
                    devices.map(KfdRuntimePersistentComputeInputV1::ScriptedReplay),
                ),
            );
            return Err(self.terminal_error(
                "scripted three-binding persistent restoration slots changed unexpectedly",
            ));
        }
        let restore = |admission: PersistentFullRangeComputeAdmissionV1, device| {
            if admission.source == PersistentFullRangeComputeSourceV1::AuthenticatedH2d
                && admission.access == RuntimeAccessV1::Read
            {
                let authenticated_sha256 = self.allocations[&admission.allocation]
                    .content_sha256
                    .expect("scripted authenticated read retains its digest");
                let DirectionalSdmaDeviceOwnerV1::Scripted(device) = device else {
                    unreachable!("scripted completion retains scripted device custody")
                };
                KfdRuntimePersistentComputeInputV1::ScriptedReady(PersistentComputeReadyStorageV1 {
                    owner: PersistentComputeReadyOwnerV1::Scripted {
                        device,
                        authenticated_sha256,
                    },
                    promotion: None,
                })
            } else {
                KfdRuntimePersistentComputeInputV1::ScriptedReplay(device)
            }
        };
        let [device_a, device_b, device_c] = devices;
        let restored_inputs = [
            restore(admissions[0], device_a),
            restore(admissions[1], device_b),
            restore(admissions[2], device_c),
        ];
        self.restore_three_binding_persistent_inputs_v1(
            admissions,
            active.id,
            restored_inputs,
            [None; 3],
            restore_shells,
        )?;
        for admission in admissions {
            apply_persistent_compute_effect_v1(
                self.allocations
                    .get_mut(&admission.allocation)
                    .expect("restored scripted three-binding allocation"),
                persistent_compute_effect_v1(admission.access),
            );
        }
        self.finish_restored_three_binding_persistent_compute_v1(active, Duration::ZERO)
    }

    #[cfg(test)]
    pub(super) fn finish_scripted_persistent_compute_v1(
        &mut self,
        mut active: ActiveSubmissionV1,
        allocation: u64,
        access: RuntimeAccessV1,
        device: Box<DirectionalSdmaDeviceOwnerV1>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.scripted_persistent_transition_failure
            == Some(ScriptedPersistentTransitionFailureV1::Poll)
        {
            self.scripted_persistent_transition_failure = None;
            self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Device(*device));
            return Err(self.terminal_error(
                "scripted persistent-compute completion observation returned foreign retryable custody",
            ));
        }
        active.performance.publish_to_completion = active.published_at.elapsed();
        if self.scripted_persistent_transition_failure
            == Some(ScriptedPersistentTransitionFailureV1::Recycle)
        {
            self.scripted_persistent_transition_failure = None;
            self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Device(*device));
            return Err(self.terminal_error(
                "scripted persistent-compute completion recycle returned foreign retryable custody",
            ));
        }
        if self.scripted_persistent_transition_failure
            == Some(ScriptedPersistentTransitionFailureV1::Detach)
        {
            self.scripted_persistent_transition_failure = None;
            self.retain_terminal_sdma_custody_v1(KfdRuntimeTerminalSdmaCustodyV1::Device(*device));
            return Err(self.terminal_error(
                "scripted persistent-compute completion detach returned foreign retryable custody",
            ));
        }
        self.restore_persistent_compute_completion_v1(
            allocation,
            active.id,
            *device,
            persistent_compute_effect_v1(access),
        )?;
        self.finish_restored_persistent_compute_v1(active, allocation, Duration::ZERO)
    }

    pub(super) fn poll_compute_lane_v1(
        &mut self,
        lane: usize,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.with_compute_lane_state_v1(lane, |backend| {
            let ordinary_native_lane = if backend
                .active
                .as_ref()
                .and_then(|active| active.execution.as_ref())
                .is_some_and(|execution| {
                    matches!(
                        execution,
                        ActiveComputeExecutionV1::MaterializedPrepared { .. }
                            | ActiveComputeExecutionV1::Materialized(_)
                    )
                }) {
                Some(backend.selected_native_compute_lane_v1().map_err(|_| {
                    backend.terminal_error(
                        "published KFD submission lost its exact physical compute lane",
                    )
                })?)
            } else {
                None
            };
            let Some(mut active) = backend.active.take() else {
                return Err(backend
                    .terminal_error("selected KFD compute lane lost its logical frontier owner"));
            };
            let Some(execution) = active.execution.take() else {
                backend.active = Some(active);
                return Err(
                    backend.terminal_error("active KFD submission lost its execution custody")
                );
            };
            match execution {
                ActiveComputeExecutionV1::MaterializedPrepared { profile } => {
                    let native_lane = ordinary_native_lane
                        .expect("prepared materialized execution validated its native lane");
                    let publication_started = Instant::now();
                    let publication =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            backend
                                .queue
                                .as_mut()
                                .expect("prepared materialized submission retains queue")
                                .with_compute_lane_v1(native_lane, |queue| {
                                    queue.submit_fixed_dispatch_classified_v1::<1>()
                                })
                        }));
                    let publication = match publication {
                        Ok(Ok(publication)) => publication,
                        Ok(Err(error)) => {
                            active.execution = Some(
                                ActiveComputeExecutionV1::MaterializedPrepared { profile },
                            );
                            backend.active = Some(active);
                            return Err(backend.terminal_error(format!(
                                "KFD compute-lane selection before prepared publication: {error}"
                            )));
                        }
                        Err(payload) => {
                            active.execution = Some(
                                ActiveComputeExecutionV1::MaterializedPrepared { profile },
                            );
                            backend.active = Some(active);
                            let _ = backend.terminal_error(
                                "KFD prepared materialized publication unwound with logical custody",
                            );
                            std::panic::resume_unwind(payload);
                        }
                    };
                    let batch = match publication {
                        Ok(batch) => batch,
                        Err(
                            Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(_),
                        ) => {
                            active.performance.publication += publication_started.elapsed();
                            active.execution = Some(
                                ActiveComputeExecutionV1::MaterializedPrepared { profile },
                            );
                            backend.active = Some(active);
                            return Ok(BackendPollV1::Pending);
                        }
                        Err(Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                            error,
                        )) => {
                            active.execution = Some(
                                ActiveComputeExecutionV1::MaterializedPrepared { profile },
                            );
                            backend.active = Some(active);
                            return Err(backend.terminal_error(format!(
                                "KFD retained prepared dispatch was rejected before publication: {error}"
                            )));
                        }
                        Err(Gfx942FixedDispatchSubmissionFailureV1::Terminal(error)) => {
                            active.execution = Some(
                                ActiveComputeExecutionV1::MaterializedPrepared { profile },
                            );
                            backend.active = Some(active);
                            return Err(backend.terminal_error(format!(
                                "KFD prepared dispatch publication became indeterminate: {error}"
                            )));
                        }
                    };
                    active.performance.publication += publication_started.elapsed();
                    active.published_at = Instant::now();
                    active.execution = Some(ActiveComputeExecutionV1::Materialized(batch));
                    let id = active.id;
                    let stream = active.stream;
                    let kernel = active.kernel;
                    let dispatch_shape_sha256 = active.dispatch_shape_sha256;
                    backend.active = Some(active);
                    let profiling =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            backend.observe_materialized_dispatch_published_v1(
                                id,
                                stream,
                                kernel,
                                dispatch_shape_sha256,
                                profile,
                            );
                        }));
                    if let Err(payload) = profiling {
                        let _ = backend.terminal_error(
                            "KFD prepared dispatch profiling unwound after publication",
                        );
                        std::panic::resume_unwind(payload);
                    }
                    Ok(BackendPollV1::Pending)
                }
                ActiveComputeExecutionV1::Materialized(batch) => {
                    let native_lane = ordinary_native_lane
                        .expect("materialized execution validated its native lane");
                    let mut batch_owner = Some(batch);
                    let observation =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            backend
                                .queue
                                .as_mut()
                                .expect("active submission retains queue")
                                .with_compute_lane_v1(native_lane, |queue| {
                                    queue.poll_fixed_dispatch(
                                        batch_owner.take().expect(
                                            "selected lane consumes the batch exactly once",
                                        ),
                                    )
                                })
                        }));
                    let poll = match observation {
                        Ok(Ok(Ok(poll))) => poll,
                        Ok(Ok(Err(error))) => {
                            backend.active = Some(active);
                            return Err(backend
                                .terminal_error(format!("KFD completion observation: {error}")));
                        }
                        Ok(Err(error)) => {
                            if let Some(batch) = batch_owner.take() {
                                active.execution =
                                    Some(ActiveComputeExecutionV1::Materialized(batch));
                            }
                            backend.active = Some(active);
                            return Err(backend.terminal_error(format!(
                                "KFD compute-lane selection after publication: {error}"
                            )));
                        }
                        Err(payload) => {
                            if let Some(batch) = batch_owner.take() {
                                active.execution =
                                    Some(ActiveComputeExecutionV1::Materialized(batch));
                            }
                            backend.active = Some(active);
                            let _ = backend.terminal_error(
                                "KFD completion observation unwound with published custody",
                            );
                            std::panic::resume_unwind(payload);
                        }
                    };
                    match poll {
                        Gfx942DispatchPollV1::Pending(batch) => {
                            active.execution = Some(ActiveComputeExecutionV1::Materialized(batch));
                            backend.active = Some(active);
                            Ok(BackendPollV1::Pending)
                        }
                        Gfx942DispatchPollV1::Ready(completed) => {
                            backend.finish_completed(active, completed)
                        }
                    }
                }
                ActiveComputeExecutionV1::MaterializedCompleted(completed) => {
                    backend.finish_completed(active, completed)
                }
                ActiveComputeExecutionV1::PersistentPrepared {
                    allocation,
                    access,
                    prepared,
                    profile,
                } => {
                    let publication_started = Instant::now();
                    match backend
                        .queue
                        .as_mut()
                        .expect("prepared persistent submission retains its queue")
                        .submit_directional_persistent_fixed_dispatch_v1(prepared)
                    {
                        Ok(dispatch) => {
                            active.performance.publication += publication_started.elapsed();
                            active.published_at = Instant::now();
                            active.execution = Some(ActiveComputeExecutionV1::Persistent {
                                allocation,
                                access,
                                dispatch,
                            });
                            backend.observe_persistent_dispatch_published_v1(
                                active.id,
                                active.stream,
                                active.kernel,
                                active.dispatch_shape_sha256,
                                profile,
                            );
                            backend.active = Some(active);
                            Ok(BackendPollV1::Pending)
                        }
                        Err(failure) => {
                            let (_, retryable) = failure.into_parts();
                            if let Some(prepared) = retryable {
                                active.execution =
                                    Some(ActiveComputeExecutionV1::PersistentPrepared {
                                        allocation,
                                        access,
                                        prepared,
                                        profile,
                                    });
                                backend.active = Some(active);
                                Ok(BackendPollV1::Pending)
                            } else {
                                Err(backend.terminal_error(
                                    "KFD persistent-compute publication became indeterminate",
                                ))
                            }
                        }
                    }
                }
                ActiveComputeExecutionV1::Persistent {
                    allocation,
                    access,
                    dispatch,
                } => {
                    let poll = backend
                        .queue
                        .as_mut()
                        .expect("persistent submission retains its queue")
                        .poll_and_recycle_directional_persistent_fixed_dispatch_v1(dispatch);
                    backend.finish_persistent_compute_poll_and_recycle_v1(
                        active, allocation, access, poll,
                    )
                }
                ActiveComputeExecutionV1::ThreeBindingPersistentPrepared {
                    admissions,
                    promotions,
                    restore_shells,
                    prepared,
                    profile,
                } => {
                    let publication_started = Instant::now();
                    match backend
                        .queue
                        .as_mut()
                        .expect("prepared three-binding submission retains its queue")
                        .submit_three_binding_directional_persistent_fixed_dispatch_v1(prepared)
                    {
                        Ok(dispatch) => {
                            active.performance.publication += publication_started.elapsed();
                            active.published_at = Instant::now();
                            active.execution =
                                Some(ActiveComputeExecutionV1::ThreeBindingPersistent {
                                    admissions,
                                    restore_shells,
                                    dispatch,
                                });
                            backend.observe_persistent_dispatch_published_v1(
                                active.id,
                                active.stream,
                                active.kernel,
                                active.dispatch_shape_sha256,
                                profile,
                            );
                            backend.active = Some(active);
                            Ok(BackendPollV1::Pending)
                        }
                        Err(failure) => {
                            let (_, retryable) = failure.into_parts();
                            if let Some(prepared) = retryable {
                                active.execution = Some(
                                    ActiveComputeExecutionV1::ThreeBindingPersistentPrepared {
                                        admissions,
                                        promotions,
                                        restore_shells,
                                        prepared,
                                        profile,
                                    },
                                );
                                backend.active = Some(active);
                                Ok(BackendPollV1::Pending)
                            } else {
                                Err(backend.terminal_error(
                                    "KFD three-binding persistent publication became indeterminate",
                                ))
                            }
                        }
                    }
                }
                ActiveComputeExecutionV1::ThreeBindingPersistent {
                    admissions,
                    restore_shells,
                    dispatch,
                } => {
                    let poll = backend
                        .queue
                        .as_mut()
                        .expect("three-binding persistent submission retains its queue")
                        .poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1(
                            dispatch,
                        );
                    backend.finish_three_binding_persistent_poll_and_recycle_v1(
                        active,
                        admissions,
                        restore_shells,
                        poll,
                    )
                }
                #[cfg(test)]
                ActiveComputeExecutionV1::ScriptedPersistent {
                    allocation,
                    access,
                    device,
                } => backend
                    .finish_scripted_persistent_compute_v1(active, allocation, access, device),
                #[cfg(test)]
                ActiveComputeExecutionV1::ScriptedThreeBindingPersistent {
                    admissions,
                    restore_shells,
                    devices,
                } => backend.finish_scripted_three_binding_persistent_compute_v1(
                    active,
                    admissions,
                    restore_shells,
                    devices,
                ),
                #[cfg(test)]
                ActiveComputeExecutionV1::ScriptedPersistentPrepared {
                    allocation,
                    access,
                    input,
                    profile,
                } => {
                    active.published_at = Instant::now();
                    let device = match *input {
                        KfdRuntimePersistentComputeInputV1::ScriptedReady(ready) => {
                            ready.owner.normalize()
                        }
                        KfdRuntimePersistentComputeInputV1::ScriptedReplay(device) => device,
                        KfdRuntimePersistentComputeInputV1::Native(_) => {
                            unreachable!("scripted publication retained native input")
                        }
                    };
                    active.execution = Some(ActiveComputeExecutionV1::ScriptedPersistent {
                        allocation,
                        access,
                        device: Box::new(device),
                    });
                    backend.observe_persistent_dispatch_published_v1(
                        active.id,
                        active.stream,
                        active.kernel,
                        active.dispatch_shape_sha256,
                        profile,
                    );
                    backend.active = Some(active);
                    Ok(BackendPollV1::Pending)
                }
                #[cfg(test)]
                ActiveComputeExecutionV1::ScriptedMaterialized => {
                    active.performance.publish_to_completion = active.published_at.elapsed();
                    backend.finish_scripted_materialized_compute_v1(active)
                }
            }
        })
    }

    pub(super) fn poll_compute_submission_v1(
        &mut self,
        lane: usize,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let is_frontier = if lane == 0 {
            self.active
                .as_ref()
                .is_some_and(|active| active.id == submission)
        } else {
            self.auxiliary_compute_lanes[lane - 1]
                .active
                .as_ref()
                .is_some_and(|active| active.id == submission)
        };
        if is_frontier {
            return self.poll_compute_lane_v1(lane);
        }
        let status = self.with_compute_lane_state_v1(lane, |backend| {
            backend.poll_pipelined_compute_submission_v1(submission)
        })?;
        if status == BackendPollV1::Pending
            && self
                .active_compute_lane_v1(submission)
                .is_some_and(|owner| owner == lane)
        {
            // A waiter on a recycled successor must also own bounded progress
            // of the earlier logical frontier; otherwise the successor could
            // remain Pending forever after its physical retirement.
            let _ = self.poll_compute_lane_v1(lane)?;
            if let Some(record) = self.submissions.get(&submission) {
                return Ok(record.status);
            }
        }
        Ok(status)
    }

    // Recycle failures retain their exact completed token inline. Boxing that
    // token would add allocation failure to a linear-custody recovery path.
    #[allow(clippy::result_large_err)]
    fn poll_pipelined_compute_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        match self.compute_pipeline.phase(submission) {
            Some(RuntimeComputePipelinePhaseV1::PhysicallyRetired) => {
                return Ok(BackendPollV1::Pending);
            }
            Some(RuntimeComputePipelinePhaseV1::Quarantined) => {
                return Err(self.terminal_error(
                    "KFD pipelined submission is quarantined after an indeterminate transition",
                ));
            }
            Some(
                RuntimeComputePipelinePhaseV1::Published | RuntimeComputePipelinePhaseV1::Completed,
            ) => {}
            None => {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown KFD pipelined submission",
                ));
            }
        }
        let native_lane = self.selected_native_compute_lane_v1().map_err(|_| {
            self.terminal_error("pipelined KFD submission lost its exact physical compute lane")
        })?;
        let Some((identity, mut active)) = self.compute_pipeline.take_physical_owner(submission)
        else {
            return Err(
                self.terminal_error("published KFD pipeline phase lost its exact slot owner")
            );
        };
        let Some(execution) = active.execution.take() else {
            if self
                .compute_pipeline
                .restore(identity, RuntimeComputePipelinePhaseV1::Quarantined, active)
                .is_err()
            {
                std::process::abort();
            }
            return Err(self
                .terminal_error("published KFD pipeline entry lost its native execution custody"));
        };
        let completed = match execution {
            ActiveComputeExecutionV1::Materialized(batch) => {
                let mut batch_owner = Some(batch);
                let observation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.queue
                        .as_mut()
                        .expect("pipelined submission retains queue")
                        .with_compute_lane_v1(native_lane, |queue| {
                            queue.poll_fixed_dispatch(
                                batch_owner
                                    .take()
                                    .expect("selected lane consumes the batch exactly once"),
                            )
                        })
                }));
                let poll = match observation {
                    Ok(Ok(poll)) => poll,
                    Ok(Err(error)) => {
                        if let Some(batch) = batch_owner.take() {
                            active.execution = Some(ActiveComputeExecutionV1::Materialized(batch));
                        }
                        if self
                            .compute_pipeline
                            .restore(identity, RuntimeComputePipelinePhaseV1::Quarantined, active)
                            .is_err()
                        {
                            std::process::abort();
                        }
                        return Err(self.terminal_error(format!(
                            "KFD compute-lane selection after pipelined publication: {error}"
                        )));
                    }
                    Err(payload) => {
                        if let Some(batch) = batch_owner.take() {
                            active.execution = Some(ActiveComputeExecutionV1::Materialized(batch));
                        }
                        if self
                            .compute_pipeline
                            .restore(identity, RuntimeComputePipelinePhaseV1::Quarantined, active)
                            .is_err()
                        {
                            std::process::abort();
                        }
                        let _ = self.terminal_error(
                            "KFD pipelined completion observation unwound with published custody",
                        );
                        std::panic::resume_unwind(payload);
                    }
                };
                match poll {
                    Ok(Gfx942DispatchPollV1::Pending(batch)) => {
                        active.execution = Some(ActiveComputeExecutionV1::Materialized(batch));
                        if self
                            .compute_pipeline
                            .restore(identity, RuntimeComputePipelinePhaseV1::Published, active)
                            .is_err()
                        {
                            std::process::abort();
                        }
                        return Ok(BackendPollV1::Pending);
                    }
                    Ok(Gfx942DispatchPollV1::Ready(completed)) => completed,
                    Err(error) => {
                        if self
                            .compute_pipeline
                            .restore(identity, RuntimeComputePipelinePhaseV1::Quarantined, active)
                            .is_err()
                        {
                            std::process::abort();
                        }
                        return Err(self.terminal_error(format!(
                            "KFD pipelined completion observation: {error}"
                        )));
                    }
                }
            }
            ActiveComputeExecutionV1::MaterializedCompleted(completed) => completed,
            _ => {
                if self
                    .compute_pipeline
                    .restore(identity, RuntimeComputePipelinePhaseV1::Quarantined, active)
                    .is_err()
                {
                    std::process::abort();
                }
                return Err(self.terminal_error(
                    "KFD runtime pipeline retained a non-ordinary execution owner",
                ));
            }
        };
        active.performance.publish_to_completion = active.published_at.elapsed();
        let recycle_started = Instant::now();
        let mut completed_owner = Some(completed);
        let recycling = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.queue
                .as_mut()
                .expect("completed pipeline entry retains queue")
                .with_compute_lane_v1(native_lane, |queue| {
                    queue.recycle_fixed_dispatch(
                        completed_owner
                            .take()
                            .expect("selected lane consumes completed custody exactly once"),
                    )
                })
        }));
        let recycle = match recycling {
            Ok(Ok(recycle)) => recycle,
            Ok(Err(error)) => {
                if let Some(completed) = completed_owner.take() {
                    active.execution =
                        Some(ActiveComputeExecutionV1::MaterializedCompleted(completed));
                }
                if self
                    .compute_pipeline
                    .restore(identity, RuntimeComputePipelinePhaseV1::Quarantined, active)
                    .is_err()
                {
                    std::process::abort();
                }
                return Err(self.terminal_error(format!(
                    "KFD compute-lane selection after pipelined completion: {error}"
                )));
            }
            Err(payload) => {
                if let Some(completed) = completed_owner.take() {
                    active.execution =
                        Some(ActiveComputeExecutionV1::MaterializedCompleted(completed));
                }
                if self
                    .compute_pipeline
                    .restore(identity, RuntimeComputePipelinePhaseV1::Quarantined, active)
                    .is_err()
                {
                    std::process::abort();
                }
                let _ = self.terminal_error(
                    "KFD pipelined completion recycle unwound with completed custody",
                );
                std::panic::resume_unwind(payload);
            }
        };
        match recycle {
            Ok(_) => {
                active.performance.completed_readback = Duration::ZERO;
                active.performance.completion_signal_recycle = recycle_started.elapsed();
                active.performance.completion_detach_restore = Duration::ZERO;
                if self
                    .compute_pipeline
                    .restore(
                        identity,
                        RuntimeComputePipelinePhaseV1::PhysicallyRetired,
                        active,
                    )
                    .is_err()
                {
                    std::process::abort();
                }
                Ok(BackendPollV1::Pending)
            }
            Err(failure) => {
                let (error, retryable) = failure.into_parts();
                let phase = if let Some(completed) = retryable {
                    active.execution =
                        Some(ActiveComputeExecutionV1::MaterializedCompleted(completed));
                    RuntimeComputePipelinePhaseV1::Completed
                } else {
                    RuntimeComputePipelinePhaseV1::Quarantined
                };
                if self
                    .compute_pipeline
                    .restore(identity, phase, active)
                    .is_err()
                {
                    std::process::abort();
                }
                if phase == RuntimeComputePipelinePhaseV1::Completed {
                    Ok(BackendPollV1::Pending)
                } else {
                    Err(self.terminal_error(format!(
                        "KFD pipelined completion recycle became indeterminate: {error}"
                    )))
                }
            }
        }
    }

    pub(super) fn wait_published_persistent_compute_lane_v1(
        &mut self,
        lane: usize,
        deadline: Instant,
    ) -> Result<Option<BackendPollV1>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.with_compute_lane_state_v1(lane, |backend| {
            let waitable = backend
                .active
                .as_ref()
                .and_then(|active| active.execution.as_ref())
                .is_some_and(|execution| match execution {
                    ActiveComputeExecutionV1::Persistent { .. }
                    | ActiveComputeExecutionV1::ThreeBindingPersistent { .. } => true,
                    #[cfg(test)]
                    ActiveComputeExecutionV1::ScriptedPersistent { .. }
                    | ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { .. } => true,
                    _ => false,
                });
            if !waitable {
                return Ok(None);
            }
            let Some(mut active) = backend.active.take() else {
                return Ok(None);
            };
            let Some(execution) = active.execution.take() else {
                backend.active = Some(active);
                return Ok(None);
            };
            match execution {
                ActiveComputeExecutionV1::Persistent {
                    allocation,
                    access,
                    dispatch,
                } => {
                    let Some(queue) = backend.queue.as_mut() else {
                        backend.retain_terminal_sdma_custody_v1(
                            KfdRuntimeTerminalSdmaCustodyV1::PersistentComputePublished(dispatch),
                        );
                        return Err(backend
                            .terminal_error("published persistent submission lost its KFD queue"));
                    };
                    let wait = queue
                        .wait_and_recycle_directional_persistent_fixed_dispatch_until_v1(
                            dispatch, deadline,
                        )
                        .map(|wait| match wait {
                            Gfx942PersistentComputeWaitAndRecycleV1::Timeout {
                                dispatch, ..
                            } => Gfx942PersistentComputePollAndRecycleV1::Pending(dispatch),
                            Gfx942PersistentComputeWaitAndRecycleV1::Recycled {
                                recycled,
                                completion_observed_at,
                                ..
                            } => Gfx942PersistentComputePollAndRecycleV1::Recycled {
                                recycled,
                                completion_observed_at,
                            },
                        });
                    backend
                        .finish_persistent_compute_poll_and_recycle_v1(
                            active, allocation, access, wait,
                        )
                        .map(Some)
                }
                ActiveComputeExecutionV1::ThreeBindingPersistent {
                    admissions,
                    restore_shells,
                    dispatch,
                } => {
                    let Some(queue) = backend.queue.as_mut() else {
                        backend.retain_terminal_sdma_custody_v1(
                            KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentComputePublished(
                                dispatch,
                            ),
                        );
                        return Err(backend.terminal_error(
                            "published three-binding persistent submission lost its KFD queue",
                        ));
                    };
                    let wait = queue
                        .wait_and_recycle_three_binding_directional_persistent_fixed_dispatch_until_v1(
                            dispatch, deadline,
                        )
                        .map(|wait| match wait {
                            Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1::Timeout {
                                dispatch, ..
                            } => Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Pending(
                                dispatch,
                            ),
                            Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1::Recycled {
                                recycled,
                                completion_observed_at,
                                ..
                            } => Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Recycled {
                                recycled,
                                completion_observed_at,
                            },
                        });
                    backend
                        .finish_three_binding_persistent_poll_and_recycle_v1(
                            active,
                            admissions,
                            restore_shells,
                            wait,
                        )
                        .map(Some)
                }
                #[cfg(test)]
                ActiveComputeExecutionV1::ScriptedPersistent {
                    allocation,
                    access,
                    device,
                } => {
                    let mut attempts = 0_u32;
                    let mut sleep = WAIT_INITIAL_SLEEP_V1;
                    loop {
                        backend.scripted_persistent_wait_observations += 1;
                        if backend.scripted_persistent_wait_pending_observations == 0 {
                            return backend
                                .finish_scripted_persistent_compute_v1(
                                    active, allocation, access, device,
                                )
                                .map(Some);
                        }
                        backend.scripted_persistent_wait_pending_observations -= 1;
                        if !apply_wait_backoff_v1(attempts, &mut sleep, deadline) {
                            active.execution = Some(ActiveComputeExecutionV1::ScriptedPersistent {
                                allocation,
                                access,
                                device,
                            });
                            backend.active = Some(active);
                            return Ok(Some(BackendPollV1::Pending));
                        }
                        attempts = attempts.saturating_add(1);
                    }
                }
                #[cfg(test)]
                ActiveComputeExecutionV1::ScriptedThreeBindingPersistent {
                    admissions,
                    restore_shells,
                    devices,
                } => backend
                    .finish_scripted_three_binding_persistent_compute_v1(
                        active,
                        admissions,
                        restore_shells,
                        devices,
                    )
                    .map(Some),
                other => {
                    active.execution = Some(other);
                    backend.active = Some(active);
                    Ok(None)
                }
            }
        })
    }

    fn settle_failed_compute_after_ordering_v1(
        &mut self,
        pending: PendingComputeSubmissionV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        // A failed unpublished node still orders its successor after the entire
        // stream prefix. Keep its custody until that predecessor has completed.
        if let Some(predecessor) = pending.ordered_predecessor
            && !self
                .submissions
                .get(&predecessor)
                .is_some_and(|record| ordered_predecessor_completed_v1(record.status))
        {
            match self.poll_v1(predecessor) {
                Ok(BackendPollV1::Pending) => {
                    self.pending_compute.insert(pending.id, pending);
                    return Ok(BackendPollV1::Pending);
                }
                Ok(BackendPollV1::Succeeded | BackendPollV1::Failed { .. })
                | Err(RuntimeBackendFailureV1::Quiescent(_)) => {}
                Err(failure) => {
                    self.pending_compute.insert(pending.id, pending);
                    return Err(failure);
                }
            }
        }
        Ok(self.settle_unpublished_compute_v1(pending, BackendPollV1::Failed { code: -1 }))
    }

    pub(super) fn progress_pending_compute_v1(
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
        while let Some(dependency) = pending
            .explicit_success_dependencies
            .get(pending.explicit_dependency_cursor)
            .copied()
        {
            let status = match self.poll_v1(dependency) {
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
                            let _ = self.terminal_error(
                                "KFD ordered-successor publication unwound while native custody was live",
                            );
                            std::panic::resume_unwind(payload);
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
                        Err(failure) => {
                            self.pending_compute.insert(pending.id, pending);
                            return Err(failure);
                        }
                    }
                }
            }
            let status = match self.poll_v1(predecessor) {
                Ok(status) => status,
                Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                    return Ok(self.settle_unpublished_compute_v1(
                        pending,
                        BackendPollV1::Failed { code: -1 },
                    ));
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
        let persistent_selected = self
            .persistent_full_range_admission_for_launch_v1(pending.launch.borrowed())
            .is_some()
            || self
                .three_binding_persistent_admission_for_launch_v1(pending.launch.borrowed())
                .is_some();
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
            return match self.poll_v1(copy) {
                Ok(_) => {
                    self.pending_compute.insert(pending.id, pending);
                    Ok(BackendPollV1::Pending)
                }
                Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                    Ok(self
                        .settle_unpublished_compute_v1(pending, BackendPollV1::Failed { code: -1 }))
                }
                Err(failure @ RuntimeBackendFailureV1::Rejected(_))
                | Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                    self.pending_compute.insert(pending.id, pending);
                    Err(failure)
                }
            };
        }
        let staging = (|| {
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
            Ok(())
        })();
        if let Err(failure) = staging {
            return match failure {
                RuntimeBackendFailureV1::Rejected(_) | RuntimeBackendFailureV1::Quiescent(_) => {
                    Ok(self
                        .settle_unpublished_compute_v1(pending, BackendPollV1::Failed { code: -1 }))
                }
                failure @ RuntimeBackendFailureV1::Terminal(_) => {
                    self.pending_compute.insert(pending.id, pending);
                    Err(failure)
                }
            };
        }
        self.lease_compute_lane_v1(pending.launch.stream, lane);
        let publication = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.with_compute_lane_state_v1(lane, |backend| {
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
                let _ = self.terminal_error(
                    "KFD compute publication unwound while logical custody was retained",
                );
                std::panic::resume_unwind(payload);
            }
        };
        match publication {
            Ok(()) => {
                self.remove_pending_compute_from_stream_v1(pending.launch.stream, pending.id);
                self.release_pending_compute_dependency_retains_v1(&pending);
                Ok(BackendPollV1::Pending)
            }
            Err(RuntimeBackendFailureV1::Rejected(_) | RuntimeBackendFailureV1::Quiescent(_)) => {
                self.release_compute_lane_lease_v1(pending.launch.stream, lane);
                Ok(self.settle_unpublished_compute_v1(pending, BackendPollV1::Failed { code: -1 }))
            }
            Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                self.pending_compute.insert(pending.id, pending);
                Err(failure)
            }
        }
    }

    pub(super) fn observe_pending_compute_v1(
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
        while let Some(dependency) = pending
            .explicit_success_dependencies
            .get(pending.explicit_dependency_cursor)
            .copied()
        {
            let status = match self.poll_v1(dependency) {
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
            match self.poll_v1(predecessor) {
                Ok(_) => {}
                Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                    return Ok(self.settle_unpublished_compute_v1(
                        pending,
                        BackendPollV1::Failed { code: -1 },
                    ));
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
        self.pending_compute.insert(pending.id, pending);
        Ok(BackendPollV1::Pending)
    }

    pub(super) fn try_publish_ordered_successor_v1(
        &mut self,
        pending: &PendingComputeSubmissionV1,
        predecessor: u64,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if !self.compute_pipeline.has_successor_capacity()
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
                    && matches!(
                        active.execution,
                        Some(
                            ActiveComputeExecutionV1::Materialized(_)
                                | ActiveComputeExecutionV1::MaterializedCompleted(_)
                        )
                    )
            });
        let pipelined_predecessor_matches =
            self.compute_pipeline
                .get(predecessor)
                .is_some_and(|active| {
                    let phase = self.compute_pipeline.phase(predecessor);
                    let physical_owner_matches = matches!(
                        (phase, active.execution.as_ref()),
                        (
                            Some(RuntimeComputePipelinePhaseV1::Published),
                            Some(ActiveComputeExecutionV1::Materialized(_)),
                        ) | (
                            Some(RuntimeComputePipelinePhaseV1::Completed),
                            Some(ActiveComputeExecutionV1::MaterializedCompleted(_)),
                        ) | (Some(RuntimeComputePipelinePhaseV1::PhysicallyRetired), None)
                    );
                    physical_owner_matches
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

        let publication_started = Instant::now();
        let native_lane = self.selected_native_compute_lane_v1().map_err(|_| {
            self.terminal_error(
                "ordered predecessor lost its exact physical compute lane before successor publication",
            )
        })?;
        let publication = self
            .queue
            .as_mut()
            .expect("ordered predecessor retains its physical queue")
            .with_compute_lane_v1(native_lane, |queue| {
                queue.submit_fixed_dispatch_classified_v1::<1>()
            })
            .map_err(|error| self.terminal_error(format!("KFD compute-lane selection: {error}")))?;
        let batch = match publication {
            Ok(batch) => batch,
            Err(Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(_)) => {
                return Ok(false);
            }
            Err(Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error)) => {
                return Err(self.terminal_error(format!(
                    "KFD retained ordered-successor recipe was rejected before publication: {error}"
                )));
            }
            Err(Gfx942FixedDispatchSubmissionFailureV1::Terminal(error)) => {
                return Err(self.terminal_error(format!(
                    "KFD ordered-successor publication became indeterminate: {error}"
                )));
            }
        };
        performance.publication = publication_started.elapsed();
        performance.native_binding = Duration::ZERO;
        performance.data_path = KfdRuntimeLaunchDataPathV1::ResidentReused;
        performance.user_data_materializations = 0;
        let active = ActiveSubmissionV1 {
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
            execution: Some(ActiveComputeExecutionV1::Materialized(batch)),
        };
        if let Err(_active) = self.compute_pipeline.insert_published(active) {
            // Capacity and generation were checked with no intervening roster
            // mutation. Native publication already happened, so a violated
            // internal invariant must not unwind and drop its linear token.
            std::process::abort();
        }

        let profile_dispatch =
            self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, pending.id);
        let profile_queue = self.profile_resource_v1(
            KfdProfileResourceKindV1::NativeQueue,
            KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + self.selected_compute_lane as u64,
        );
        let profile_stream = self.profile_resource_v1(KfdProfileResourceKindV1::Stream, stream);
        let profile_kernel = self.profile_resource_v1(KfdProfileResourceKindV1::Kernel, kernel);
        let profile_shape = self.profile_content_v1(&dispatch_shape_sha256);
        let profile_event = match profile_bindings {
            Some(Ok(bindings)) => profile_dispatch
                .zip(profile_queue)
                .zip(profile_stream)
                .zip(profile_kernel)
                .zip(profile_shape)
                .map(|((((dispatch, queue), stream), kernel), dispatch_shape)| {
                    KfdRuntimeProfileEventKindV1::DispatchPublished {
                        dispatch,
                        queue,
                        stream,
                        kernel,
                        dispatch_shape,
                        launch: profile_launch,
                        bindings,
                    }
                }),
            Some(Err(())) | None => None,
        };
        self.observe_profile_dispatch_v1(profile_event, profile_semantic_contract);
        Ok(true)
    }

    pub(super) fn pending_compute_can_publish_under_deadline_v1(&self, submission: u64) -> bool {
        let Some(pending) = self.pending_compute.get(&submission) else {
            return false;
        };
        if pending.explicit_dependency_cursor != pending.explicit_success_dependencies.len()
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

    pub(super) fn persistent_full_range_admission_for_launch_v1(
        &self,
        launch: BackendLaunchV1<'_>,
    ) -> Option<PersistentFullRangeComputeAdmissionV1> {
        let stream_device = self.streams.get(&launch.stream).copied()?;
        let binding = launch.bindings.first()?;
        let allocation = self.allocations.get(&binding.region.allocation);
        let ready =
            allocation.and_then(|record| record.sdma_storage.persistent_compute_ready_facts_v1());
        let authenticated = persistent_full_range_compute_admission_v1(
            launch.semantic_launch,
            launch.bindings,
            stream_device,
            allocation,
            ready,
        );
        let Some(retained) = self.retained_persistent_dispatch else {
            return authenticated;
        };
        let dispatch_shape_sha256 = dispatch_shape_sha256_v1(&launch, launch.semantic_launch);
        if retained.allocation != binding.region.allocation
            || retained.dispatch_shape_sha256 != dispatch_shape_sha256
        {
            return authenticated;
        }
        authenticated.or_else(|| {
            retained_persistent_full_range_compute_admission_v1(
                launch.semantic_launch,
                launch.bindings,
                stream_device,
                allocation,
                retained,
                dispatch_shape_sha256,
            )
        })
    }

    pub(super) fn three_binding_persistent_admission_for_launch_v1(
        &self,
        launch: BackendLaunchV1<'_>,
    ) -> Option<ThreeBindingPersistentComputeAdmissionV1> {
        let stream_device = self.streams.get(&launch.stream).copied()?;
        three_binding_persistent_compute_admission_v1(
            launch.semantic_launch,
            launch.bindings,
            stream_device,
            &self.allocations,
        )
    }

    pub(super) fn prepare_launch(
        &mut self,
        launch: BackendLaunchV1<'_>,
        persistent_selected: bool,
        reuse_bound_recipe: bool,
    ) -> Result<PreparedLaunchV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let preparation_started = Instant::now();
        let dispatch_shape_sha256 = dispatch_shape_sha256_v1(&launch, launch.semantic_launch);
        let profile_launch = KfdProfileLaunchV1 {
            grid: launch.geometry.grid,
            workgroup: launch.geometry.workgroup,
            dynamic_shared_bytes: launch.geometry.dynamic_shared_bytes,
        };
        let profile_semantic_contract = self
            .profiler
            .as_ref()
            .is_some_and(KfdRuntimeProfileRecorderV1::captures_semantic_profile)
            .then(|| profile_semantic_contract_v1(launch.semantic_launch, profile_launch))
            .flatten();
        let profile_bindings = self.prepare_profile_bindings_v1(launch.bindings);
        let stream_device = *self.streams.get(&launch.stream).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD stream",
            )
        })?;
        let persistent_admission = self.persistent_full_range_admission_for_launch_v1(launch);
        let three_binding_admission = self.three_binding_persistent_admission_for_launch_v1(launch);
        if three_binding_requires_persistent_admission_v1(
            launch.semantic_launch,
            launch.bindings,
            &self.allocations,
        ) && three_binding_admission.is_none()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "three-binding persistent-compute candidate failed exact R/R/W admission",
            ));
        }
        let persistent_control_reused = persistent_selected
            && three_binding_admission.is_none()
            && persistent_control_is_reused_v1(
                self.retained_persistent_dispatch,
                persistent_admission,
                dispatch_shape_sha256,
            );
        if persistent_selected
            && persistent_admission.is_none()
            && three_binding_admission.is_none()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "persistent-compute admission changed after path selection; materialization is forbidden",
            ));
        }
        let replaces_retained_control = persistent_admission.is_some_and(|admission| {
            admission.source == PersistentFullRangeComputeSourceV1::AuthenticatedH2d
                && self.retained_persistent_dispatch.is_some_and(|retained| {
                    retained.allocation != admission.allocation
                        || retained.dispatch_shape_sha256 != dispatch_shape_sha256
                })
        });
        if persistent_selected && (replaces_retained_control || three_binding_admission.is_some()) {
            self.release_retained_persistent_control_v1()?;
        }
        if !persistent_selected && !reuse_bound_recipe {
            self.release_retained_persistent_control_v1()?;
            let mut synchronized = HashSet::new();
            for binding in launch.bindings {
                if synchronized.insert(binding.region.allocation) {
                    self.normalize_h2d_ready_v1(binding.region.allocation)?;
                    self.synchronize_native_allocation_v1(binding.region.allocation)?;
                    self.synchronize_sdma_shadow_v1(binding.region.allocation)?;
                }
            }
        }
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
        let geometry = AqlDispatchGeometryV1::new(launch.geometry.grid, launch.geometry.workgroup)
            .map_err(|error| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    format!("invalid AQL geometry: {error:?}"),
                )
            })?;
        let closure = kernel.validated.validated();
        let inspected = closure.selected_kernel();
        let arguments = inspected.explicit_arguments();
        let global_argument_count = arguments
            .iter()
            .filter(|argument| argument.value_kind() == ExplicitValueKind::GlobalBuffer)
            .count();
        if global_argument_count != launch.bindings.len() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "typed binding roster does not cover every AMDHSA global buffer",
            ));
        }

        let snapshot_started = Instant::now();
        let staged = match (persistent_admission, three_binding_admission) {
            (_, Some(admission)) if persistent_selected => {
                snapshot_three_binding_persistent_data_v1(
                    &self.allocations,
                    launch.bindings,
                    stream_device,
                    admission,
                )?
            }
            (Some(admission), None) if persistent_selected => {
                snapshot_persistent_full_range_data_v1(
                    &self.allocations,
                    launch
                        .bindings
                        .first()
                        .expect("persistent admission has one binding"),
                    stream_device,
                    admission,
                    self.retained_persistent_dispatch,
                    dispatch_shape_sha256,
                )?
            }
            _ => snapshot_bound_data_v1(&self.allocations, launch.bindings, stream_device)?,
        };
        let bound_snapshot = snapshot_started.elapsed();
        let mut buffer_bindings = Vec::new();
        let mut abi_rows = Vec::new();
        let mut allocations = HashSet::new();
        let mut writebacks = Vec::new();
        let mut seen_argument_indices = HashSet::new();
        buffer_bindings
            .try_reserve_exact(launch.bindings.len())
            .map_err(|_| Self::capacity("KFD buffer-binding preparation allocation failed"))?;
        abi_rows
            .try_reserve_exact(launch.bindings.len())
            .map_err(|_| Self::capacity("KFD dispatch-ABI preparation allocation failed"))?;
        allocations
            .try_reserve(launch.bindings.len())
            .map_err(|_| Self::capacity("KFD allocation-retention roster allocation failed"))?;
        writebacks
            .try_reserve_exact(launch.bindings.len())
            .map_err(|_| Self::capacity("KFD writeback roster allocation failed"))?;
        seen_argument_indices
            .try_reserve(launch.bindings.len())
            .map_err(|_| Self::capacity("KFD argument-roster allocation failed"))?;

        for binding in launch.bindings {
            let region = binding.region;
            let (argument_index, argument) = arguments
                .iter()
                .enumerate()
                .find(|(_, argument)| argument.offset() == u64::from(binding.kernarg_byte_offset))
                .ok_or_else(|| {
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                        "kernarg pointer patch does not match an AMDHSA global buffer",
                    )
                })?;
            if !seen_argument_indices.insert(argument_index) {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "more than one binding targets the same AMDHSA argument",
                ));
            }
            if argument.value_kind() != ExplicitValueKind::GlobalBuffer {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "kernarg pointer patch targets a non-global AMDHSA argument",
                ));
            }
            let placement = staged.placements[&region.allocation];
            let staged_offset = region
                .byte_offset
                .checked_sub(placement.allocation_offset)
                .expect("staged allocation window starts before every bound range");
            buffer_bindings.push(Gfx942DispatchBufferBindingV1::new(
                argument_index,
                placement.data_index,
                staged_offset,
                region.byte_len,
            ));
            argument.name().ok_or_else(|| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "AMDHSA global buffer has no source argument name",
                )
            })?;
            abi_rows.push(OwnedAbiRowV1 {
                explicit_argument_index: argument_index,
                offset: argument.offset(),
                pointee_alignment: argument.pointee_alignment().unwrap_or(1),
                access: map_access_v1(region.access),
            });
            allocations.insert(region.allocation);
            if region.access != RuntimeAccessV1::Read {
                writebacks.push(WritebackV1 {
                    allocation: region.allocation,
                    allocation_offset: usize::try_from(region.byte_offset).map_err(|_| {
                        Self::rejected(
                            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                            "binding offset does not fit host address space",
                        )
                    })?,
                    data_index: placement.data_index,
                    data_offset: staged_offset,
                    byte_len: region.byte_len,
                });
            }
        }

        let total_kernarg =
            usize::try_from(closure.resources().kernarg_segment_size()).map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "kernarg size does not fit host address space",
                )
            })?;
        let explicit_len = launch.explicit_kernarg.len();
        match inspected.implicit_argument_offset() {
            Some(offset)
                if usize::try_from(offset).ok() == Some(explicit_len)
                    && usize::try_from(inspected.implicit_argument_size()).ok()
                        == Some(COV6_IMPLICIT_KERNARG_BYTES_V1)
                    && explicit_len.checked_add(COV6_IMPLICIT_KERNARG_BYTES_V1)
                        == Some(total_kernarg) => {}
            None if explicit_len == total_kernarg => {}
            _ => {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "explicit kernarg does not match the inspected COV6 layout",
                ));
            }
        }
        let mut kernarg = Vec::new();
        kernarg
            .try_reserve_exact(total_kernarg)
            .map_err(|_| Self::capacity("KFD kernarg staging allocation failed"))?;
        kernarg.extend_from_slice(launch.explicit_kernarg);
        kernarg.resize(total_kernarg, 0);

        let mut authority_allocations = Vec::new();
        authority_allocations
            .try_reserve_exact(staged.data.len())
            .map_err(|_| Self::capacity("KFD authority allocation roster allocation failed"))?;
        for spec in &staged.data {
            authority_allocations.push(KfdRuntimeAuthorityAllocationV1 {
                allocation: spec.allocation,
                kind: spec.kind,
                alignment: spec.alignment,
                byte_offset: spec.allocation_offset,
                bytes: spec.bytes(),
                content_sha256: spec.content_sha256,
            });
        }
        let mut authority_abi = Vec::new();
        authority_abi
            .try_reserve_exact(abi_rows.len())
            .map_err(|_| Self::capacity("KFD authority ABI roster allocation failed"))?;
        for row in &abi_rows {
            let argument = &arguments[row.explicit_argument_index];
            authority_abi.push(KfdRuntimeAuthorityGlobalBufferV1 {
                explicit_argument_index: row.explicit_argument_index,
                name: argument
                    .name()
                    .expect("prepared global-buffer ABI row retains a source name"),
                kernarg_byte_offset: row.offset,
                pointee_alignment: row.pointee_alignment,
                access: row.access,
            });
        }
        let authority_started = Instant::now();
        let authorized = self
            .launch_gate
            .authorize_launch_v1(KfdRuntimeAuthorityRequestV1 {
                module_image: module.validated.bytes(),
                module_sha256: module.image_sha256,
                kernel_name: kernel.validated.selected_kernel().name(),
                signature: kernel.signature,
                explicit_kernarg: launch.explicit_kernarg,
                complete_kernarg_template: &kernarg,
                bindings: launch.bindings,
                dispatch_abi: &authority_abi,
                allocations: &authority_allocations,
                geometry: launch.geometry,
                semantic_launch: launch.semantic_launch,
            });
        let authority = authority_started.elapsed();
        if !authorized {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "direct KFD launch authority denied the exact invocation",
            ));
        }

        let storage = match (
            persistent_admission.filter(|_| persistent_selected),
            three_binding_admission.filter(|_| persistent_selected),
        ) {
            (_, Some(admission)) => {
                let descriptors = resident_descriptors_v1(&staged.data)?;
                PreparedLaunchStorageV1::ThreeBindingPersistent(ThreeBindingPersistentPreparedV1 {
                    admissions: admission.bindings,
                    descriptors,
                })
            }
            (Some(admission), None) => {
                let descriptors = resident_descriptors_v1(&staged.data)?;
                PreparedLaunchStorageV1::PersistentFullRange(PersistentFullRangePreparedV1 {
                    allocation: admission.allocation,
                    access: admission.access,
                    source: admission.source,
                    descriptors,
                })
            }
            (None, None) => PreparedLaunchStorageV1::Materialized(staged.data),
        };
        let preparation = preparation_started.elapsed();
        Ok(PreparedLaunchV1 {
            stream: launch.stream,
            kernel: launch.kernel,
            program: kernel.validated.clone(),
            signature: kernel.signature,
            kernarg: kernarg.into_boxed_slice(),
            geometry,
            dynamic_shared_bytes: launch.geometry.dynamic_shared_bytes,
            buffer_bindings: buffer_bindings.into_boxed_slice(),
            abi_rows,
            storage,
            allocations,
            writebacks,
            dispatch_shape_sha256,
            profile_launch,
            profile_semantic_contract,
            profile_bindings,
            performance: KfdRuntimeLaunchPerformanceV1 {
                preparation,
                bound_snapshot,
                authority,
                persistent_control_reused,
                ..KfdRuntimeLaunchPerformanceV1::default()
            },
        })
    }

    pub(super) fn publish(
        &mut self,
        id: u64,
        dependency_depth: usize,
        ordered_predecessor: Option<u64>,
        ordinary_recipe: Arc<OwnedComputeLaunchV1>,
        prepared: PreparedLaunchV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if matches!(
            &prepared.storage,
            PreparedLaunchStorageV1::PersistentFullRange(_)
                | PreparedLaunchStorageV1::ThreeBindingPersistent(_)
        ) {
            if matches!(
                &prepared.storage,
                PreparedLaunchStorageV1::ThreeBindingPersistent(_)
            ) {
                return self.publish_three_binding_persistent_v1(
                    id,
                    dependency_depth,
                    ordered_predecessor,
                    prepared,
                );
            }
            return self.publish_persistent_full_range_v1(
                id,
                dependency_depth,
                ordered_predecessor,
                prepared,
            );
        }
        let PreparedLaunchV1 {
            stream,
            kernel,
            program,
            signature,
            kernarg,
            geometry,
            dynamic_shared_bytes,
            buffer_bindings,
            abi_rows,
            storage,
            allocations,
            writebacks,
            dispatch_shape_sha256,
            profile_launch,
            profile_semantic_contract,
            profile_bindings,
            mut performance,
        } = prepared;
        let PreparedLaunchStorageV1::Materialized(data) = storage else {
            unreachable!("persistent launch publication branches before materialization")
        };
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
        let resident_descriptors = resident_descriptors_v1(&data)?;
        let user_data_count =
            u64::try_from(data.len()).expect("fixed-dispatch data count is bounded below u64");

        #[cfg(test)]
        if self.scripted_sdma.is_some() && writebacks.is_empty() {
            performance.data_path = KfdRuntimeLaunchDataPathV1::Materialized;
            performance.user_data_materializations = user_data_count;
            self.active = Some(ActiveSubmissionV1 {
                id,
                stream,
                ordered_predecessor,
                deferred_ordered_predecessor_retain: false,
                kernel,
                dependency_depth,
                allocations,
                writebacks,
                resident_descriptors,
                ordinary_recipe: Some(ordinary_recipe),
                dispatch_shape_sha256,
                published_at: Instant::now(),
                performance,
                execution: Some(ActiveComputeExecutionV1::ScriptedMaterialized),
            });
            let profile_dispatch = self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, id);
            let profile_queue = self.profile_resource_v1(
                KfdProfileResourceKindV1::NativeQueue,
                KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + self.selected_compute_lane as u64,
            );
            let profile_stream = self.profile_resource_v1(KfdProfileResourceKindV1::Stream, stream);
            let profile_kernel = self.profile_resource_v1(KfdProfileResourceKindV1::Kernel, kernel);
            let profile_shape = self.profile_content_v1(&dispatch_shape_sha256);
            let profile_event = match profile_bindings {
                Some(Ok(bindings)) => profile_dispatch
                    .zip(profile_queue)
                    .zip(profile_stream)
                    .zip(profile_kernel)
                    .zip(profile_shape)
                    .map(|((((dispatch, queue), stream), kernel), dispatch_shape)| {
                        KfdRuntimeProfileEventKindV1::DispatchPublished {
                            dispatch,
                            queue,
                            stream,
                            kernel,
                            dispatch_shape,
                            launch: profile_launch,
                            bindings,
                        }
                    }),
                Some(Err(())) | None => None,
            };
            self.observe_profile_dispatch_v1(profile_event, profile_semantic_contract);
            return Ok(());
        }

        let native_binding_started = Instant::now();
        let creates_native_queue = self.native_compute_lanes[self.selected_compute_lane].is_none();
        let mut reused_attached = false;
        let reuse_attached = self.recycled_dispatch.as_ref().is_some_and(|recycled| {
            recycled_dispatch_reuse_is_admitted_v1(
                recycled,
                dispatch_shape_sha256,
                &resident_descriptors,
                &data,
            )
        });
        if self.recycled_dispatch.is_some() && !reuse_attached {
            self.detach_recycled_dispatch()?;
        }
        if reuse_attached {
            let recycled = self
                .recycled_dispatch
                .take()
                .expect("admitted attached dispatch remains retained");
            let overwrite = {
                let native_lane = self.selected_native_compute_lane_v1()?;
                let queue = self
                    .queue
                    .as_mut()
                    .expect("recycled dispatch retains queue");
                queue
                    .with_compute_lane_v1(native_lane, |queue| {
                        queue
                            .recycled_fixed_dispatch_generation()
                            .map_err(|error| format!("KFD recycled generation: {error}"))
                            .and_then(|generation| {
                                recycled
                                    .descriptors
                                    .iter()
                                    .zip(&data)
                                    .enumerate()
                                    .try_for_each(|(index, (prior, spec))| {
                                        if !resident_data_needs_host_overwrite_v1(
                                            prior,
                                            spec.content_sha256,
                                        ) {
                                            return Ok(());
                                        }
                                        queue
                                            .overwrite_recycled_fixed_dispatch_host_data(
                                                Gfx942RecycledDispatchWriteRequestV1::new(
                                                    generation, index, 0,
                                                ),
                                                spec.bytes(),
                                            )
                                            .map_err(|error| {
                                                format!("KFD recycled-data overwrite: {error}")
                                            })
                                    })
                            })
                    })
                    .map_err(|error| format!("KFD compute-lane selection: {error}"))
                    .and_then(core::convert::identity)
            };
            if let Err(detail) = overwrite {
                return Err(self.terminal_error(detail));
            }
            reused_attached = true;
            performance.data_path = KfdRuntimeLaunchDataPathV1::ResidentReused;
        }

        if !reused_attached {
            let validated_program = build_program_v1(&program, signature, &abi_rows)?;
            let mut programs = Vec::new();
            programs
                .try_reserve_exact(1)
                .map_err(|_| Self::capacity("KFD program roster allocation failed"))?;
            programs.push(validated_program);
            let packet = Gfx942FixedDispatchPacketV1::new(
                0,
                geometry,
                dynamic_shared_bytes,
                kernarg,
                buffer_bindings,
            );
            if creates_native_queue && self.queue.is_none() {
                performance.user_data_materializations = user_data_count;
                let device = self.admitted_device.take().ok_or_else(|| {
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Unsupported,
                        "the admitted KFD queue lifecycle has already retired",
                    )
                })?;
                let mut memory = device
                    .acquire_shared_gtt_memory_session_with_backing_budgets_v1(
                        self.device_backing_budget,
                        self.host_visible_backing_budget,
                    )
                    .map_err(|error| self.terminal_error(format!("KFD VM acquisition: {error}")))?;
                let native_data = match materialize_initial_data_v1(&mut memory, data, signature) {
                    Ok(data) => data,
                    Err(detail) => {
                        self.terminal_memory = Some(memory);
                        return Err(self.terminal_error(detail));
                    }
                };
                let queue = memory
                    .create_compute_aql_queue_with_fixed_dispatch(
                        KFD_RUNTIME_RING_BYTES_V1,
                        programs,
                        [packet],
                        native_data,
                    )
                    .map_err(|error| self.terminal_error(format!("KFD queue creation: {error}")))?;
                let primary_lane = queue.primary_compute_lane_v1();
                self.queue = Some(queue);
                self.configure_native_device_pool_v1()?;
                self.native_compute_lanes[self.selected_compute_lane] = Some(primary_lane);
            } else if creates_native_queue {
                performance.user_data_materializations = user_data_count;
                let mut materialization_error = None;
                let lane = self
                    .queue
                    .as_mut()
                    .expect("shared KFD queue owner exists")
                    .create_auxiliary_compute_lane_with_fixed_dispatch(
                        KFD_RUNTIME_RING_BYTES_V1,
                        programs,
                        [packet],
                        |memory| {
                            materialize_initial_data_v1(memory, data, signature).map_err(|detail| {
                                materialization_error = Some(detail);
                                fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                                    "KFD auxiliary data materialization",
                                )
                            })
                        },
                    )
                    .map_err(|error| {
                        self.terminal_error(
                            materialization_error.unwrap_or_else(|| {
                                format!("KFD auxiliary queue creation: {error}")
                            }),
                        )
                    })?;
                self.native_compute_lanes[self.selected_compute_lane] = Some(lane);
            } else {
                let mut reused_resident_data = false;
                let rebound = {
                    let native_lane = self.selected_native_compute_lane_v1()?;
                    let queue = self.queue.as_mut().expect("checked queue");
                    queue
                        .with_compute_lane_v1(native_lane, |queue| {
                            let native_data = match self.resident_data.take() {
                                Some(mut resident)
                                    if same_resident_storage_shape_v1(
                                        &resident.descriptors,
                                        &resident_descriptors,
                                    ) && data.iter().all(|spec| {
                                        spec.kind == RuntimeMemoryKindV1::HostVisible
                                    }) =>
                                {
                                    reused_resident_data = true;
                                    let overwrite = resident
                                        .data
                                        .iter_mut()
                                        .zip(resident.descriptors.iter().zip(&data))
                                        .enumerate()
                                        .try_for_each(|(index, (native, (prior, spec)))| {
                                            if !resident_data_needs_host_overwrite_v1(
                                                prior,
                                                spec.content_sha256,
                                            ) {
                                                return Ok(());
                                            }
                                            queue
                                                .overwrite_detached_initialized_host_visible_fixed_dispatch_data(
                                                    index,
                                                    native,
                                                    0,
                                                    spec.bytes(),
                                                )
                                                .map_err(|error| {
                                                    format!("KFD resident-data overwrite: {error}")
                                                })
                                        });
                                    overwrite.map(|()| resident.data)
                                }
                                Some(resident) => release_resident_data_v1(queue, resident)
                                    .and_then(|()| {
                                        materialize_rebound_data_v1(queue, data, signature)
                                    }),
                                None => materialize_rebound_data_v1(queue, data, signature),
                            };
                            native_data.and_then(|native_data| {
                                queue
                                    .bind_fixed_dispatch(programs, [packet], native_data)
                                    .map_err(|error| format!("KFD dispatch rebind: {error}"))
                            })
                        })
                        .map_err(|error| format!("KFD compute-lane selection: {error}"))
                        .and_then(core::convert::identity)
                };
                if let Err(detail) = rebound {
                    return Err(self.terminal_error(detail));
                }
                if reused_resident_data {
                    performance.data_path = KfdRuntimeLaunchDataPathV1::ResidentReused;
                } else {
                    performance.user_data_materializations = user_data_count;
                }
            }
        }
        performance.native_binding = native_binding_started.elapsed();
        if creates_native_queue {
            let queue = self.profile_resource_v1(
                KfdProfileResourceKindV1::NativeQueue,
                KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + self.selected_compute_lane as u64,
            );
            self.observe_profile_v1(
                queue.map(|queue| KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue }),
            );
        }

        let publication_profile = PersistentPublicationProfileV1 {
            launch: profile_launch,
            semantic_contract: profile_semantic_contract,
            bindings: profile_bindings,
        };
        let publication_started = Instant::now();
        let native_lane = self.selected_native_compute_lane_v1()?;
        let batch = self
            .queue
            .as_mut()
            .expect("queue was created or rebound")
            .with_compute_lane_v1(native_lane, |queue| {
                queue.submit_fixed_dispatch_classified_v1::<1>()
            })
            .map_err(|error| self.terminal_error(format!("KFD compute-lane selection: {error}")))?;
        let batch = match batch {
            Ok(batch) => batch,
            Err(Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(_)) => {
                performance.publication += publication_started.elapsed();
                self.active = Some(ActiveSubmissionV1 {
                    id,
                    stream,
                    ordered_predecessor,
                    deferred_ordered_predecessor_retain: false,
                    kernel,
                    dependency_depth,
                    allocations,
                    writebacks,
                    resident_descriptors,
                    ordinary_recipe: Some(ordinary_recipe),
                    dispatch_shape_sha256,
                    published_at: Instant::now(),
                    performance,
                    execution: Some(ActiveComputeExecutionV1::MaterializedPrepared {
                        profile: publication_profile,
                    }),
                });
                return Ok(());
            }
            Err(Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error)) => {
                return Err(self.terminal_error(format!(
                    "KFD retained dispatch binding was rejected before publication: {error}"
                )));
            }
            Err(Gfx942FixedDispatchSubmissionFailureV1::Terminal(error)) => {
                return Err(self.terminal_error(format!(
                    "KFD dispatch publication became indeterminate: {error}"
                )));
            }
        };
        performance.publication = publication_started.elapsed();
        let published_at = Instant::now();
        self.active = Some(ActiveSubmissionV1 {
            id,
            stream,
            ordered_predecessor,
            deferred_ordered_predecessor_retain: false,
            kernel,
            dependency_depth,
            allocations,
            writebacks,
            resident_descriptors,
            ordinary_recipe: Some(ordinary_recipe),
            dispatch_shape_sha256,
            published_at,
            performance,
            execution: Some(ActiveComputeExecutionV1::Materialized(batch)),
        });
        self.observe_materialized_dispatch_published_v1(
            id,
            stream,
            kernel,
            dispatch_shape_sha256,
            publication_profile,
        );
        Ok(())
    }

    pub(super) fn observe_materialized_dispatch_published_v1(
        &mut self,
        id: u64,
        stream: u64,
        kernel: u64,
        dispatch_shape_sha256: [u8; 32],
        profile: PersistentPublicationProfileV1,
    ) {
        let profile_dispatch = self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, id);
        let profile_queue = self.profile_resource_v1(
            KfdProfileResourceKindV1::NativeQueue,
            KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + self.selected_compute_lane as u64,
        );
        let profile_stream = self.profile_resource_v1(KfdProfileResourceKindV1::Stream, stream);
        let profile_kernel = self.profile_resource_v1(KfdProfileResourceKindV1::Kernel, kernel);
        let profile_shape = self.profile_content_v1(&dispatch_shape_sha256);
        let profile_event = match profile.bindings {
            Some(Ok(bindings)) => profile_dispatch
                .zip(profile_queue)
                .zip(profile_stream)
                .zip(profile_kernel)
                .zip(profile_shape)
                .map(|((((dispatch, queue), stream), kernel), dispatch_shape)| {
                    KfdRuntimeProfileEventKindV1::DispatchPublished {
                        dispatch,
                        queue,
                        stream,
                        kernel,
                        dispatch_shape,
                        launch: profile.launch,
                        bindings,
                    }
                }),
            Some(Err(())) | None => None,
        };
        self.observe_profile_dispatch_v1(profile_event, profile.semantic_contract);
    }

    pub(super) fn observe_persistent_dispatch_published_v1(
        &mut self,
        id: u64,
        stream: u64,
        kernel: u64,
        dispatch_shape_sha256: [u8; 32],
        profile: PersistentPublicationProfileV1,
    ) {
        let profile_dispatch = self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, id);
        let profile_queue = self.profile_resource_v1(
            KfdProfileResourceKindV1::NativeQueue,
            KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1,
        );
        let profile_stream = self.profile_resource_v1(KfdProfileResourceKindV1::Stream, stream);
        let profile_kernel = self.profile_resource_v1(KfdProfileResourceKindV1::Kernel, kernel);
        let profile_shape = self.profile_content_v1(&dispatch_shape_sha256);
        let profile_event = match profile.bindings {
            Some(Ok(bindings)) => profile_dispatch
                .zip(profile_queue)
                .zip(profile_stream)
                .zip(profile_kernel)
                .zip(profile_shape)
                .map(|((((dispatch, queue), stream), kernel), dispatch_shape)| {
                    KfdRuntimeProfileEventKindV1::DispatchPublished {
                        dispatch,
                        queue,
                        stream,
                        kernel,
                        dispatch_shape,
                        launch: profile.launch,
                        bindings,
                    }
                }),
            Some(Err(())) | None => None,
        };
        self.observe_profile_dispatch_v1(profile_event, profile.semantic_contract);
    }

    pub(super) fn publish_three_binding_persistent_v1(
        &mut self,
        id: u64,
        dependency_depth: usize,
        ordered_predecessor: Option<u64>,
        prepared: PreparedLaunchV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let PreparedLaunchV1 {
            stream,
            kernel,
            program,
            signature,
            kernarg,
            geometry,
            dynamic_shared_bytes,
            buffer_bindings,
            abi_rows,
            storage,
            allocations,
            writebacks,
            dispatch_shape_sha256,
            profile_launch,
            profile_semantic_contract,
            profile_bindings,
            mut performance,
        } = prepared;
        let PreparedLaunchStorageV1::ThreeBindingPersistent(persistent) = storage else {
            unreachable!("three-binding publication requires matching prepared storage")
        };
        if self.selected_compute_lane != 0
            || self.recycled_dispatch.is_some()
            || self.resident_data.is_some()
            || persistent.descriptors.len() != 3
            || allocations.len() != 3
            || persistent
                .admissions
                .iter()
                .any(|admission| !allocations.contains(&admission.allocation))
            || writebacks.len() != 1
            || writebacks[0].allocation != persistent.admissions[2].allocation
            || persistent.admissions.map(|admission| admission.access)
                != [
                    RuntimeAccessV1::Read,
                    RuntimeAccessV1::Read,
                    RuntimeAccessV1::Write,
                ]
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "three-binding persistent publication preconditions changed",
            ));
        }
        let validated_program = build_program_v1(&program, signature, &abi_rows)?;
        let mut programs = Vec::new();
        programs
            .try_reserve_exact(1)
            .map_err(|_| Self::capacity("KFD three-binding program roster allocation failed"))?;
        programs.push(validated_program);
        let packet = Gfx942FixedDispatchPacketV1::new(
            0,
            geometry,
            dynamic_shared_bytes,
            kernarg,
            buffer_bindings,
        );
        let content_roles = [0, 1, 2].map(|ordinal| {
            Gfx942DeviceContentRoleV1::new(signature, ordinal)
                .expect("three fixed binding ordinals fit the content-role contract")
        });
        let restore_shells = self.prepare_three_binding_restore_shells_v1(persistent.admissions)?;
        let (persistent_inputs, promotions) =
            self.take_three_binding_persistent_inputs_v1(persistent.admissions, id)?;
        let mut publication_profile = Some(PersistentPublicationProfileV1 {
            launch: profile_launch,
            semantic_contract: profile_semantic_contract,
            bindings: profile_bindings,
        });
        #[cfg(test)]
        if self.scripted_sdma.is_some() {
            let devices = persistent_inputs.map(|input| match input {
                KfdRuntimePersistentComputeInputV1::ScriptedReady(ready) => ready.owner.normalize(),
                KfdRuntimePersistentComputeInputV1::ScriptedReplay(device) => device,
                KfdRuntimePersistentComputeInputV1::Native(_) => {
                    unreachable!("scripted three-binding publication retained native input")
                }
            });
            performance.data_path = KfdRuntimeLaunchDataPathV1::PersistentDeviceReused;
            performance.user_data_materializations = 0;
            let published_at = Instant::now();
            self.active = Some(ActiveSubmissionV1 {
                id,
                stream,
                ordered_predecessor,
                deferred_ordered_predecessor_retain: false,
                kernel,
                dependency_depth,
                allocations,
                writebacks,
                resident_descriptors: persistent.descriptors,
                ordinary_recipe: None,
                dispatch_shape_sha256,
                published_at,
                performance,
                execution: Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistent {
                    admissions: persistent.admissions,
                    restore_shells,
                    devices,
                }),
            });
            self.observe_persistent_dispatch_published_v1(
                id,
                stream,
                kernel,
                dispatch_shape_sha256,
                publication_profile
                    .take()
                    .expect("scripted publication retains its profile"),
            );
            return Ok(());
        }
        #[cfg(not(test))]
        let inputs = persistent_inputs.map(|input| match input {
            KfdRuntimePersistentComputeInputV1::Native(input) => input,
        });
        #[cfg(test)]
        let inputs = {
            if persistent_inputs
                .iter()
                .any(|input| !matches!(input, KfdRuntimePersistentComputeInputV1::Native(_)))
            {
                self.restore_three_binding_persistent_inputs_v1(
                    persistent.admissions,
                    id,
                    persistent_inputs,
                    promotions,
                    restore_shells,
                )?;
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    "scripted three-binding publication has no native queue",
                ));
            }
            persistent_inputs.map(|input| match input {
                KfdRuntimePersistentComputeInputV1::Native(input) => input,
                KfdRuntimePersistentComputeInputV1::ScriptedReady(_)
                | KfdRuntimePersistentComputeInputV1::ScriptedReplay(_) => unreachable!(),
            })
        };
        let native_binding_started = Instant::now();
        let binding = self
            .queue
            .as_mut()
            .expect("three-binding native inputs retain their queue")
            .bind_three_binding_directional_persistent_fixed_dispatch_v1(
                programs,
                [packet],
                Gfx942ThreeBindingPersistentComputeInputsV1::new(inputs),
                content_roles,
            );
        let binding = match binding {
            Ok(binding) => binding,
            Err(failure) => {
                let (error, custody) = failure.into_parts();
                return match custody {
                    Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1::Retryable(inputs) => {
                        self.restore_three_binding_persistent_inputs_v1(
                            persistent.admissions,
                            id,
                            inputs
                                .into_inputs()
                                .map(KfdRuntimePersistentComputeInputV1::Native),
                            promotions,
                            restore_shells,
                        )?;
                        Err(Self::rejected(
                            KfdRuntimeBackendErrorKindV1::Native,
                            format!("KFD three-binding persistent binding: {error}"),
                        ))
                    }
                    Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1::ProcessTeardown(
                        custody,
                    ) => {
                        self.retain_terminal_sdma_custody_v1(
                            KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentComputeBind(
                                custody,
                            ),
                        );
                        Err(self.terminal_error(format!(
                            "KFD three-binding persistent binding became indeterminate: {error}"
                        )))
                    }
                };
            }
        };
        let native_binding = native_binding_started.elapsed();
        let publication_started = Instant::now();
        let publication = self
            .queue
            .as_mut()
            .expect("three-binding persistent binding retains its queue")
            .submit_three_binding_directional_persistent_fixed_dispatch_v1(binding);
        record_initial_persistent_timing_v1(
            &mut performance,
            native_binding,
            publication_started.elapsed(),
        );
        performance.data_path = KfdRuntimeLaunchDataPathV1::PersistentDeviceReused;
        performance.user_data_materializations = 0;
        let execution = match publication {
            Ok(dispatch) => ActiveComputeExecutionV1::ThreeBindingPersistent {
                admissions: persistent.admissions,
                restore_shells,
                dispatch,
            },
            Err(failure) => {
                let (_, retryable) = failure.into_parts();
                let Some(prepared) = retryable else {
                    return Err(self.terminal_error(
                        "KFD three-binding persistent publication became indeterminate",
                    ));
                };
                ActiveComputeExecutionV1::ThreeBindingPersistentPrepared {
                    admissions: persistent.admissions,
                    promotions,
                    restore_shells,
                    prepared,
                    profile: publication_profile
                        .take()
                        .expect("retryable publication retains its profile"),
                }
            }
        };
        let published = matches!(
            execution,
            ActiveComputeExecutionV1::ThreeBindingPersistent { .. }
        );
        let published_at = Instant::now();
        self.retain_primary_compute_lane_v1();
        self.active = Some(ActiveSubmissionV1 {
            id,
            stream,
            ordered_predecessor,
            deferred_ordered_predecessor_retain: false,
            kernel,
            dependency_depth,
            allocations,
            writebacks,
            resident_descriptors: persistent.descriptors,
            ordinary_recipe: None,
            dispatch_shape_sha256,
            published_at,
            performance,
            execution: Some(execution),
        });
        if published {
            self.observe_persistent_dispatch_published_v1(
                id,
                stream,
                kernel,
                dispatch_shape_sha256,
                publication_profile
                    .take()
                    .expect("published dispatch retains its profile"),
            );
        }
        Ok(())
    }

    pub(super) fn publish_persistent_full_range_v1(
        &mut self,
        id: u64,
        dependency_depth: usize,
        ordered_predecessor: Option<u64>,
        prepared: PreparedLaunchV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let PreparedLaunchV1 {
            stream,
            kernel,
            program,
            signature,
            kernarg,
            geometry,
            dynamic_shared_bytes,
            buffer_bindings,
            abi_rows,
            storage,
            allocations,
            writebacks,
            dispatch_shape_sha256,
            profile_launch,
            profile_semantic_contract,
            profile_bindings,
            mut performance,
        } = prepared;
        let PreparedLaunchStorageV1::PersistentFullRange(persistent) = storage else {
            unreachable!("persistent publication requires persistent prepared storage")
        };
        if self.selected_compute_lane != 0
            || self.recycled_dispatch.is_some()
            || self.resident_data.is_some()
            || persistent.descriptors.len() != 1
            || allocations.len() != 1
            || !allocations.contains(&persistent.allocation)
            || writebacks.len() != usize::from(persistent.access != RuntimeAccessV1::Read)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "persistent-compute publication preconditions changed; materialization is forbidden",
            ));
        }
        let validated_program = build_program_v1(&program, signature, &abi_rows)?;
        let mut programs = Vec::new();
        programs
            .try_reserve_exact(1)
            .map_err(|_| Self::capacity("KFD persistent program roster allocation failed"))?;
        programs.push(validated_program);
        let packet = Gfx942FixedDispatchPacketV1::new(
            0,
            geometry,
            dynamic_shared_bytes,
            kernarg,
            buffer_bindings,
        );
        let content_role = Gfx942DeviceContentRoleV1::new(signature, 0).map_err(|error| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                format!("KFD persistent content role: {error}"),
            )
        })?;
        let (persistent_input, promotion) =
            self.take_persistent_compute_input_v1(persistent.allocation, id, persistent.source)?;
        performance.ready_promotion = promotion;
        let publication_profile = PersistentPublicationProfileV1 {
            launch: profile_launch,
            semantic_contract: profile_semantic_contract,
            bindings: profile_bindings,
        };
        #[cfg(test)]
        if self.scripted_sdma.is_some() {
            performance.data_path = KfdRuntimeLaunchDataPathV1::PersistentDeviceReused;
            performance.user_data_materializations = 0;
            if self.scripted_persistent_publication_retries != 0 {
                self.scripted_persistent_publication_retries -= 1;
                self.active = Some(ActiveSubmissionV1 {
                    id,
                    stream,
                    ordered_predecessor,
                    deferred_ordered_predecessor_retain: false,
                    kernel,
                    dependency_depth,
                    allocations,
                    writebacks,
                    resident_descriptors: persistent.descriptors,
                    ordinary_recipe: None,
                    dispatch_shape_sha256,
                    published_at: Instant::now(),
                    performance,
                    execution: Some(ActiveComputeExecutionV1::ScriptedPersistentPrepared {
                        allocation: persistent.allocation,
                        access: persistent.access,
                        input: Box::new(persistent_input),
                        profile: publication_profile,
                    }),
                });
                return Ok(());
            }
            let device = Box::new(match persistent_input {
                KfdRuntimePersistentComputeInputV1::ScriptedReady(ready) => ready.owner.normalize(),
                KfdRuntimePersistentComputeInputV1::ScriptedReplay(device) => device,
                KfdRuntimePersistentComputeInputV1::Native(_) => {
                    unreachable!("scripted publication retained native input")
                }
            });
            self.active = Some(ActiveSubmissionV1 {
                id,
                stream,
                ordered_predecessor,
                deferred_ordered_predecessor_retain: false,
                kernel,
                dependency_depth,
                allocations,
                writebacks,
                resident_descriptors: persistent.descriptors,
                ordinary_recipe: None,
                dispatch_shape_sha256,
                published_at: Instant::now(),
                performance,
                execution: Some(ActiveComputeExecutionV1::ScriptedPersistent {
                    allocation: persistent.allocation,
                    access: persistent.access,
                    device,
                }),
            });
            self.observe_persistent_dispatch_published_v1(
                id,
                stream,
                kernel,
                dispatch_shape_sha256,
                publication_profile,
            );
            return Ok(());
        }
        #[cfg(not(test))]
        let KfdRuntimePersistentComputeInputV1::Native(input) = persistent_input;
        #[cfg(test)]
        let input = match persistent_input {
            KfdRuntimePersistentComputeInputV1::Native(input) => input,
            #[cfg(test)]
            KfdRuntimePersistentComputeInputV1::ScriptedReady(ready) => {
                self.restore_h2d_ready_after_compute_rejection_v1(
                    persistent.allocation,
                    id,
                    ready,
                )?;
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    "scripted persistent-compute publication has no native queue",
                ));
            }
            #[cfg(test)]
            KfdRuntimePersistentComputeInputV1::ScriptedReplay(device) => {
                self.restore_persistent_compute_device_input_v1(
                    persistent.allocation,
                    id,
                    KfdRuntimeSdmaStorageV1::Device(Box::new(device)),
                )?;
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    "scripted persistent-compute publication has no native queue",
                ));
            }
        };

        let native_binding_started = Instant::now();
        let binding = self
            .queue
            .as_mut()
            .expect("H2D-ready native allocation retains its queue")
            .bind_directional_persistent_fixed_dispatch_v1(programs, [packet], input, content_role);
        let binding = match binding {
            Ok(binding) => binding,
            Err(failure) => {
                let detail = failure.error().to_string();
                let (_, custody) = failure.into_parts();
                return match custody {
                    Gfx942PersistentComputeBindFailureCustodyV1::Retryable(recovered) => {
                        self.restore_persistent_compute_input_v1(
                            persistent.allocation,
                            id,
                            recovered,
                            promotion,
                        )?;
                        Err(Self::rejected(
                            KfdRuntimeBackendErrorKindV1::Native,
                            format!("KFD persistent-compute binding: {detail}"),
                        ))
                    }
                    Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(custody) => {
                        self.retain_terminal_sdma_custody_v1(
                            KfdRuntimeTerminalSdmaCustodyV1::PersistentComputeBind(custody),
                        );
                        Err(self.terminal_error(format!(
                            "KFD persistent-compute binding became indeterminate: {detail}"
                        )))
                    }
                };
            }
        };
        let native_binding = native_binding_started.elapsed();
        let publication_started = Instant::now();
        let publication = self
            .queue
            .as_mut()
            .expect("persistent-compute binding retains its queue")
            .submit_directional_persistent_fixed_dispatch_v1(binding);
        record_initial_persistent_timing_v1(
            &mut performance,
            native_binding,
            publication_started.elapsed(),
        );
        let dispatch = match publication {
            Ok(dispatch) => dispatch,
            Err(failure) => {
                let detail = failure.error().to_string();
                let (_, retryable) = failure.into_parts();
                let Some(prepared) = retryable else {
                    return Err(self.terminal_error(format!(
                        "KFD persistent-compute publication became indeterminate: {detail}"
                    )));
                };
                performance.data_path = KfdRuntimeLaunchDataPathV1::PersistentDeviceReused;
                performance.user_data_materializations = 0;
                self.retain_primary_compute_lane_v1();
                self.active = Some(ActiveSubmissionV1 {
                    id,
                    stream,
                    ordered_predecessor,
                    deferred_ordered_predecessor_retain: false,
                    kernel,
                    dependency_depth,
                    allocations,
                    writebacks,
                    resident_descriptors: persistent.descriptors,
                    ordinary_recipe: None,
                    dispatch_shape_sha256,
                    published_at: Instant::now(),
                    performance,
                    execution: Some(ActiveComputeExecutionV1::PersistentPrepared {
                        allocation: persistent.allocation,
                        access: persistent.access,
                        prepared,
                        profile: publication_profile,
                    }),
                });
                return Ok(());
            }
        };
        performance.data_path = KfdRuntimeLaunchDataPathV1::PersistentDeviceReused;
        performance.user_data_materializations = 0;
        let published_at = Instant::now();
        self.retain_primary_compute_lane_v1();
        self.active = Some(ActiveSubmissionV1 {
            id,
            stream,
            ordered_predecessor,
            deferred_ordered_predecessor_retain: false,
            kernel,
            dependency_depth,
            allocations,
            writebacks,
            resident_descriptors: persistent.descriptors,
            ordinary_recipe: None,
            dispatch_shape_sha256,
            published_at,
            performance,
            execution: Some(ActiveComputeExecutionV1::Persistent {
                allocation: persistent.allocation,
                access: persistent.access,
                dispatch,
            }),
        });
        self.observe_persistent_dispatch_published_v1(
            id,
            stream,
            kernel,
            dispatch_shape_sha256,
            publication_profile,
        );
        Ok(())
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn finish_completed(
        &mut self,
        mut active: ActiveSubmissionV1,
        completed: fe2o3_kfd::Gfx942CompletedDispatchBatchV1<1>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        active.performance.publish_to_completion = active.published_at.elapsed();
        let native_lane = match self.selected_native_compute_lane_v1() {
            Ok(native_lane) => native_lane,
            Err(_) => {
                active.execution = Some(ActiveComputeExecutionV1::MaterializedCompleted(completed));
                self.active = Some(active);
                return Err(self.terminal_error(
                    "completed KFD submission lost its exact physical compute lane",
                ));
            }
        };
        let recycle_started = Instant::now();
        let mut completed_owner = Some(completed);
        let recycling = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.queue
                .as_mut()
                .expect("active submission retains queue")
                .with_compute_lane_v1(native_lane, |queue| {
                    queue.recycle_fixed_dispatch(
                        completed_owner
                            .take()
                            .expect("selected lane consumes completed custody exactly once"),
                    )
                })
        }));
        let recycle = match recycling {
            Ok(Ok(recycle)) => recycle,
            Ok(Err(error)) => {
                if let Some(completed) = completed_owner.take() {
                    active.execution =
                        Some(ActiveComputeExecutionV1::MaterializedCompleted(completed));
                }
                self.active = Some(active);
                return Err(self.terminal_error(format!(
                    "KFD compute-lane selection after completion: {error}"
                )));
            }
            Err(payload) => {
                if let Some(completed) = completed_owner.take() {
                    active.execution =
                        Some(ActiveComputeExecutionV1::MaterializedCompleted(completed));
                }
                self.active = Some(active);
                let _ =
                    self.terminal_error("KFD completion recycle unwound with completed custody");
                std::panic::resume_unwind(payload);
            }
        };
        match recycle {
            Ok(_) => {}
            Err(failure) => {
                let (error, retryable) = failure.into_parts();
                if let Some(completed) = retryable {
                    active.execution =
                        Some(ActiveComputeExecutionV1::MaterializedCompleted(completed));
                    self.active = Some(active);
                    return Ok(BackendPollV1::Pending);
                }
                self.active = Some(active);
                return Err(self.terminal_error(format!(
                    "KFD completion recycle became indeterminate: {error}"
                )));
            }
        };
        active.performance.completed_readback = Duration::ZERO;
        active.performance.completion_signal_recycle = recycle_started.elapsed();
        active.performance.completion_detach_restore = Duration::ZERO;
        let stream = active.stream;
        let commit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.commit_materialized_compute_v1(&mut active);
        }));
        if let Err(payload) = commit {
            self.active = Some(active);
            let _ = self.terminal_error(
                "KFD logical completion commit unwound while host custody was retained",
            );
            std::panic::resume_unwind(payload);
        }
        self.advance_materialized_commit_frontier_v1(stream);
        Ok(BackendPollV1::Succeeded)
    }

    fn commit_materialized_compute_v1(&mut self, active: &mut ActiveSubmissionV1) {
        let deferred_ordered_predecessor = active.deferred_ordered_predecessor_retain.then(|| {
            active
                .ordered_predecessor
                .expect("deferred ordering retain names its exact predecessor")
        });
        let compute_lane = self.selected_compute_lane;
        for writeback in &active.writebacks {
            let record = self
                .allocations
                .get_mut(&writeback.allocation)
                .expect("active allocation remains retained");
            record.content_sha256 = None;
            let extent = NativeDirtyExtentV1 {
                compute_lane,
                data_index: writeback.data_index,
                allocation_offset: writeback.allocation_offset,
                data_offset: writeback.data_offset,
                byte_len: writeback.byte_len,
            };
            if retain_unique_native_dirty_extent_v1(&mut record.native_dirty, extent) {
                self.native_dirty_extents = self
                    .native_dirty_extents
                    .checked_add(1)
                    .expect("native-dirty extent count is memory-bounded");
            }
            if let Some(descriptor) = active.resident_descriptors.get_mut(writeback.data_index) {
                descriptor.device_may_have_modified = true;
                descriptor.host_content_sha256 = None;
            }
        }
        self.recycled_dispatch = Some(RecycledDispatchV1 {
            kernel: active.kernel,
            dispatch_shape_sha256: active.dispatch_shape_sha256,
            descriptors: core::mem::take(&mut active.resident_descriptors),
        });
        let module = self
            .kernels
            .get(&active.kernel)
            .expect("active compute retains its kernel")
            .module;
        self.release_compute_custody_v1(active.id, module, active.allocations.iter().copied());
        let status = BackendPollV1::Succeeded;
        self.submissions.insert(
            active.id,
            SubmissionRecordV1 {
                stream: active.stream,
                status,
                profile_dispatch_published: true,
            },
        );
        self.compute_completion_reservations = self
            .compute_completion_reservations
            .checked_sub(1)
            .expect("published compute reserves one completion slot");
        self.last_launch_performance = Some(active.performance);
        let profile_dispatch =
            self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, active.id);
        self.observe_profile_v1(profile_dispatch.map(|dispatch| {
            KfdRuntimeProfileEventKindV1::DispatchCompleted {
                dispatch,
                host_timing: profile_host_timing_v1(active.performance),
            }
        }));
        if let Some(predecessor) = deferred_ordered_predecessor {
            self.release_compute_dependency_retains_v1(core::slice::from_ref(&predecessor));
        }
        active.execution = None;
    }

    fn advance_materialized_commit_frontier_v1(&mut self, stream: u64) {
        loop {
            let Some((phase, active)) = self.compute_pipeline.take_commit_frontier() else {
                self.release_compute_lane_lease_v1(stream, self.selected_compute_lane);
                return;
            };
            match phase {
                RuntimeComputePipelinePhaseV1::Published
                | RuntimeComputePipelinePhaseV1::Completed => {
                    self.active = Some(active);
                    return;
                }
                RuntimeComputePipelinePhaseV1::PhysicallyRetired => {
                    let mut active = active;
                    let commit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        self.commit_materialized_compute_v1(&mut active);
                    }));
                    if let Err(payload) = commit {
                        self.active = Some(active);
                        let _ = self.terminal_error(
                            "KFD pipelined logical commit unwound while host custody was retained",
                        );
                        std::panic::resume_unwind(payload);
                    }
                }
                RuntimeComputePipelinePhaseV1::Quarantined => {
                    self.active = Some(active);
                    let _ = self.terminal_error(
                        "KFD logical commit frontier reached quarantined physical custody",
                    );
                    return;
                }
            }
        }
    }

    pub(super) fn finish_persistent_full_range_recycled_v1(
        &mut self,
        mut active: ActiveSubmissionV1,
        allocation: u64,
        access: RuntimeAccessV1,
        recycled: Gfx942RecycledPersistentComputeDispatchV1,
        recycle_started: Instant,
        completion_signal_recycle: Duration,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let detach = self
            .queue
            .as_mut()
            .expect("recycled persistent completion retains its queue")
            .detach_recycled_directional_persistent_fixed_dispatch_v1(recycled);
        let completed = match detach {
            Ok(completed) => completed,
            Err(failure) => {
                let detail = failure.error().to_string();
                let (_, custody) = failure.into_parts();
                active.performance.completion_detach_restore +=
                    completion_detach_restore_duration_v1(
                        recycle_started.elapsed(),
                        completion_signal_recycle,
                    );
                return match custody {
                    Gfx942PersistentComputeTransitionFailureCustodyV1::Retryable(recycled) => {
                        self.retain_terminal_sdma_custody_v1(
                            KfdRuntimeTerminalSdmaCustodyV1::PersistentComputeRecycled(recycled),
                        );
                        Err(self.terminal_error(format!(
                            "KFD persistent-compute completion detach returned foreign retryable custody: {detail}"
                        )))
                    }
                    Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(custody) => {
                        self.retain_terminal_sdma_custody_v1(
                            KfdRuntimeTerminalSdmaCustodyV1::PersistentCompute(custody),
                        );
                        Err(self.terminal_error(format!(
                            "KFD persistent-compute completion detach: {detail}"
                        )))
                    }
                };
            }
        };
        let expected_effect = persistent_compute_effect_v1(access);
        let (input, actual_effect) = match completed.retire_settled_frontier_for_replay_v1() {
            Ok(completed) => completed,
            Err(failure) => {
                self.retain_terminal_sdma_custody_v1(
                    KfdRuntimeTerminalSdmaCustodyV1::ComputeRetirement(failure),
                );
                return Err(self.terminal_error(
                    "KFD persistent-compute completion frontier retirement failed",
                ));
            }
        };
        if actual_effect != expected_effect {
            self.retain_terminal_sdma_custody_v1(
                KfdRuntimeTerminalSdmaCustodyV1::PersistentComputeInput(input),
            );
            return Err(self
                .terminal_error("KFD persistent-compute effect changed after metadata admission"));
        }
        self.restore_persistent_compute_completion_input_v1(
            allocation,
            active.id,
            input,
            actual_effect,
        )?;
        let completion_detach_restore = completion_detach_restore_duration_v1(
            recycle_started.elapsed(),
            completion_signal_recycle,
        );
        self.finish_restored_persistent_compute_v1(active, allocation, completion_detach_restore)
    }

    #[cfg(test)]
    pub(super) fn finish_scripted_materialized_compute_v1(
        &mut self,
        mut active: ActiveSubmissionV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        debug_assert!(active.writebacks.is_empty());
        debug_assert_eq!(
            active.performance.data_path,
            KfdRuntimeLaunchDataPathV1::Materialized
        );
        active.performance.completed_readback = Duration::ZERO;
        active.performance.completion_signal_recycle = Duration::ZERO;
        active.performance.completion_detach_restore = Duration::ZERO;
        let compute_lane = self.selected_compute_lane;
        let module = self
            .kernels
            .get(&active.kernel)
            .expect("active compute retains its kernel")
            .module;
        self.release_compute_custody_v1(active.id, module, active.allocations.iter().copied());
        let status = BackendPollV1::Succeeded;
        self.submissions.insert(
            active.id,
            SubmissionRecordV1 {
                stream: active.stream,
                status,
                profile_dispatch_published: true,
            },
        );
        self.compute_completion_reservations = self
            .compute_completion_reservations
            .checked_sub(1)
            .expect("published compute reserves one completion slot");
        self.release_compute_lane_lease_v1(active.stream, compute_lane);
        self.last_launch_performance = Some(active.performance);
        let profile_dispatch =
            self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, active.id);
        self.observe_profile_v1(profile_dispatch.map(|dispatch| {
            KfdRuntimeProfileEventKindV1::DispatchCompleted {
                dispatch,
                host_timing: profile_host_timing_v1(active.performance),
            }
        }));
        active.execution = None;
        Ok(status)
    }

    pub(super) fn finish_restored_persistent_compute_v1(
        &mut self,
        mut active: ActiveSubmissionV1,
        allocation: u64,
        completion_detach_restore: Duration,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        active.performance.completed_readback = Duration::ZERO;
        active.performance.completion_detach_restore += completion_detach_restore;
        debug_assert_eq!(active.performance.user_data_materializations, 0);
        debug_assert_eq!(
            active.performance.data_path,
            KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
        );
        self.retained_persistent_dispatch = Some(RetainedPersistentDispatchV1 {
            allocation,
            dispatch_shape_sha256: active.dispatch_shape_sha256,
        });
        let compute_lane = self.selected_compute_lane;
        let module = self
            .kernels
            .get(&active.kernel)
            .expect("active compute retains its kernel")
            .module;
        self.release_compute_custody_v1(active.id, module, active.allocations.iter().copied());
        let status = BackendPollV1::Succeeded;
        self.submissions.insert(
            active.id,
            SubmissionRecordV1 {
                stream: active.stream,
                status,
                profile_dispatch_published: true,
            },
        );
        self.compute_completion_reservations = self
            .compute_completion_reservations
            .checked_sub(1)
            .expect("published compute reserves one completion slot");
        self.release_compute_lane_lease_v1(active.stream, compute_lane);
        self.last_launch_performance = Some(active.performance);
        let profile_dispatch =
            self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, active.id);
        self.observe_profile_v1(profile_dispatch.map(|dispatch| {
            KfdRuntimeProfileEventKindV1::DispatchCompleted {
                dispatch,
                host_timing: profile_host_timing_v1(active.performance),
            }
        }));
        active.execution = None;
        Ok(status)
    }

    pub(super) fn finish_restored_three_binding_persistent_compute_v1(
        &mut self,
        mut active: ActiveSubmissionV1,
        completion_detach_restore: Duration,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        active.performance.completed_readback = Duration::ZERO;
        active.performance.completion_detach_restore += completion_detach_restore;
        debug_assert_eq!(active.performance.user_data_materializations, 0);
        debug_assert_eq!(
            active.performance.data_path,
            KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
        );
        // Data persists, but this exact-three tranche rebuilds control for the
        // next launch rather than claiming retained-control replay.
        self.release_primary_detached_persistent_control_v1(
            "three-binding completion lost its detached queue control",
        )?;
        self.retained_persistent_dispatch = None;
        let compute_lane = self.selected_compute_lane;
        let module = self
            .kernels
            .get(&active.kernel)
            .expect("active compute retains its kernel")
            .module;
        self.release_compute_custody_v1(active.id, module, active.allocations.iter().copied());
        let status = BackendPollV1::Succeeded;
        self.submissions.insert(
            active.id,
            SubmissionRecordV1 {
                stream: active.stream,
                status,
                profile_dispatch_published: true,
            },
        );
        self.compute_completion_reservations = self
            .compute_completion_reservations
            .checked_sub(1)
            .expect("published compute reserves one completion slot");
        self.release_compute_lane_lease_v1(active.stream, compute_lane);
        self.last_launch_performance = Some(active.performance);
        let profile_dispatch =
            self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, active.id);
        self.observe_profile_v1(profile_dispatch.map(|dispatch| {
            KfdRuntimeProfileEventKindV1::DispatchCompleted {
                dispatch,
                host_timing: profile_host_timing_v1(active.performance),
            }
        }));
        active.execution = None;
        Ok(status)
    }

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
    /// be active. A teardown failure is terminal because the consuming KFD
    /// transition cannot return queue custody for a retry.
    pub fn shutdown_native_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if !self.streams.is_empty()
            || !self.events.is_empty()
            || !self.event_submission_retain_counts.is_empty()
            || !self.submissions.is_empty()
            || !self.modules.is_empty()
            || !self.allocations.is_empty()
            || !self.pending_compute.is_empty()
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
            let trimmed = self
                .queue
                .as_mut()
                .expect("enabled SDMA pool retains queue")
                .trim_sdma_memory_pool();
            trimmed.map_err(|error| {
                self.terminal_error(format!("KFD SDMA memory-pool trim: {error}"))
            })?;
        }
        for index in 0..self.native_compute_lanes.len() {
            let Some(native_lane) = self.native_compute_lanes[index] else {
                continue;
            };
            if native_lane.ordinal() == 0 {
                continue;
            }
            let result = self
                .queue
                .as_mut()
                .expect("auxiliary queue retains its shared owner")
                .destroy_auxiliary_compute_lane_v1(native_lane);
            if let Err(error) = result {
                return Err(
                    self.terminal_error(format!("explicit auxiliary KFD queue teardown: {error}"))
                );
            }
            self.observe_destroyed_compute_lane_v1(Some(index));
        }
        let primary_logical_lane = self
            .native_compute_lanes
            .iter()
            .position(|lane| lane.is_some_and(|lane| lane.ordinal() == 0));
        if let Some(queue) = self.queue.take() {
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

    pub(super) fn observe_destroyed_compute_lane_v1(&mut self, logical_lane: Option<usize>) {
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

    pub(super) fn synchronize_recycled_dispatch_data_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.recycled_dispatch.is_some() {
            let lane = self.selected_compute_lane;
            let mut dirty = Vec::new();
            dirty
                .try_reserve_exact(self.allocations.len())
                .map_err(|_| Self::capacity("KFD native-dirty synchronization roster failed"))?;
            dirty.extend(self.allocations.iter().filter_map(|(allocation, record)| {
                record
                    .native_dirty
                    .iter()
                    .any(|extent| extent.compute_lane == lane)
                    .then_some(*allocation)
            }));
            for allocation in dirty {
                self.synchronize_native_allocation_lane_v1(allocation, lane)?;
            }
        }
        Ok(())
    }

    pub(super) fn detach_recycled_dispatch(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.synchronize_recycled_dispatch_data_v1()?;
        let Some(recycled) = self.recycled_dispatch.take() else {
            return Ok(());
        };
        let native_lane = self.selected_native_compute_lane_v1()?;
        let result = self
            .queue
            .as_mut()
            .ok_or_else(|| "KFD recycled dispatch exists without a native queue".to_owned())
            .and_then(|queue| {
                queue
                    .with_compute_lane_v1(native_lane, |queue| {
                        queue.detach_recycled_fixed_dispatch()
                    })
                    .map_err(|error| format!("KFD compute-lane selection: {error}"))?
                    .map_err(|error| format!("KFD recycled dispatch detach: {error}"))
            });
        match result {
            Ok(detached) => {
                self.resident_data = Some(ResidentDataRosterV1 {
                    descriptors: recycled.descriptors,
                    data: detached.into_data(),
                });
                Ok(())
            }
            Err(detail) => Err(self.terminal_error(detail)),
        }
    }

    pub(super) fn release_resident_data(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let Some(resident) = self.resident_data.take() else {
            return Ok(());
        };
        let native_lane = self.selected_native_compute_lane_v1()?;
        let result = self
            .queue
            .as_mut()
            .ok_or_else(|| "KFD resident data exists without a native queue".to_owned())
            .and_then(|queue| {
                queue
                    .with_compute_lane_v1(native_lane, |queue| {
                        release_resident_data_v1(queue, resident)
                    })
                    .map_err(|error| format!("KFD compute-lane selection: {error}"))?
            });
        match result {
            Ok(()) => Ok(()),
            Err(detail) => Err(self.terminal_error(detail)),
        }
    }
}

pub(super) fn map_access_v1(access: RuntimeAccessV1) -> ArgumentAccess {
    match access {
        RuntimeAccessV1::Read => ArgumentAccess::ReadOnly,
        RuntimeAccessV1::Write => ArgumentAccess::WriteOnly,
        RuntimeAccessV1::ReadWrite => ArgumentAccess::ReadWrite,
    }
}

pub(super) fn profile_semantic_contract_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    geometry: KfdProfileLaunchV1,
) -> Option<KfdProfileSemanticContractV1> {
    match semantic_launch {
        KfdRuntimeSemanticLaunchV1::Ordinary => None,
        KfdRuntimeSemanticLaunchV1::Atomic(contract) => Some(KfdProfileSemanticContractV1::Atomic(
            KfdProfileAtomicContractV1 {
                operation: match contract.operation {
                    RuntimeAtomicOperationV1::Add => KfdProfileAtomicOperationV1::Add,
                    RuntimeAtomicOperationV1::Minimum => KfdProfileAtomicOperationV1::Minimum,
                    RuntimeAtomicOperationV1::Maximum => KfdProfileAtomicOperationV1::Maximum,
                    RuntimeAtomicOperationV1::BitwiseAnd => KfdProfileAtomicOperationV1::BitwiseAnd,
                    RuntimeAtomicOperationV1::BitwiseOr => KfdProfileAtomicOperationV1::BitwiseOr,
                    RuntimeAtomicOperationV1::BitwiseXor => KfdProfileAtomicOperationV1::BitwiseXor,
                    RuntimeAtomicOperationV1::Exchange => KfdProfileAtomicOperationV1::Exchange,
                    RuntimeAtomicOperationV1::CompareExchange => {
                        KfdProfileAtomicOperationV1::CompareExchange
                    }
                },
                scope: profile_memory_scope_v1(contract.scope),
                order: profile_memory_order_v1(contract.order),
                failure_order: contract.failure_order.map(profile_memory_order_v1),
                weak: contract.weak,
                geometry,
            },
        )),
        KfdRuntimeSemanticLaunchV1::Collective(contract) => Some(
            KfdProfileSemanticContractV1::Collective(KfdProfileCollectiveContractV1 {
                operation: match contract.operation {
                    crate::RuntimeCollectiveOperationV1::Barrier => {
                        KfdProfileCollectiveOperationV1::Barrier
                    }
                    crate::RuntimeCollectiveOperationV1::Broadcast => {
                        KfdProfileCollectiveOperationV1::Broadcast
                    }
                    crate::RuntimeCollectiveOperationV1::ReduceSum => {
                        KfdProfileCollectiveOperationV1::ReduceSum
                    }
                    crate::RuntimeCollectiveOperationV1::ReduceMinimum => {
                        KfdProfileCollectiveOperationV1::ReduceMinimum
                    }
                    crate::RuntimeCollectiveOperationV1::ReduceMaximum => {
                        KfdProfileCollectiveOperationV1::ReduceMaximum
                    }
                    crate::RuntimeCollectiveOperationV1::AllReduceSum => {
                        KfdProfileCollectiveOperationV1::AllReduceSum
                    }
                    crate::RuntimeCollectiveOperationV1::InclusiveScanSum => {
                        KfdProfileCollectiveOperationV1::InclusiveScanSum
                    }
                },
                scope: profile_memory_scope_v1(contract.scope),
                order: profile_memory_order_v1(contract.order),
                participants: contract.participants,
                geometry,
            }),
        ),
    }
}

pub(super) const fn profile_memory_scope_v1(
    scope: RuntimeMemoryScopeV1,
) -> KfdProfileMemoryScopeV1 {
    match scope {
        RuntimeMemoryScopeV1::Workgroup => KfdProfileMemoryScopeV1::Workgroup,
        RuntimeMemoryScopeV1::Device => KfdProfileMemoryScopeV1::Device,
        RuntimeMemoryScopeV1::System => KfdProfileMemoryScopeV1::System,
    }
}

pub(super) const fn profile_memory_order_v1(
    order: RuntimeMemoryOrderV1,
) -> KfdProfileMemoryOrderV1 {
    match order {
        RuntimeMemoryOrderV1::Relaxed => KfdProfileMemoryOrderV1::Relaxed,
        RuntimeMemoryOrderV1::Acquire => KfdProfileMemoryOrderV1::Acquire,
        RuntimeMemoryOrderV1::Release => KfdProfileMemoryOrderV1::Release,
        RuntimeMemoryOrderV1::AcquireRelease => KfdProfileMemoryOrderV1::AcquireRelease,
        RuntimeMemoryOrderV1::SequentiallyConsistent => {
            KfdProfileMemoryOrderV1::SequentiallyConsistent
        }
    }
}

pub(super) fn dispatch_shape_sha256_v1(
    launch: &BackendLaunchV1<'_>,
    semantic_launch: KfdRuntimeSemanticLaunchV1,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3.runtime.kfd.recycled-dispatch-shape.v1\0");
    digest.update(launch.kernel.to_le_bytes());
    for value in launch.geometry.grid {
        digest.update(value.to_le_bytes());
    }
    for value in launch.geometry.workgroup {
        digest.update(value.to_le_bytes());
    }
    digest.update(launch.geometry.dynamic_shared_bytes.to_le_bytes());
    digest.update((launch.explicit_kernarg.len() as u64).to_le_bytes());
    digest.update(launch.explicit_kernarg);
    digest.update((launch.bindings.len() as u64).to_le_bytes());
    for binding in launch.bindings {
        digest.update(binding.region.allocation.to_le_bytes());
        digest.update([match binding.region.access {
            RuntimeAccessV1::Read => 1,
            RuntimeAccessV1::Write => 2,
            RuntimeAccessV1::ReadWrite => 3,
        }]);
        digest.update(binding.region.byte_offset.to_le_bytes());
        digest.update(binding.region.byte_len.to_le_bytes());
        digest.update(binding.kernarg_byte_offset.to_le_bytes());
    }
    match semantic_launch {
        KfdRuntimeSemanticLaunchV1::Ordinary => digest.update([0]),
        KfdRuntimeSemanticLaunchV1::Atomic(contract) => {
            digest.update([1, atomic_operation_tag_v1(contract.operation)]);
            digest.update([memory_scope_tag_v1(contract.scope)]);
            digest.update([memory_order_tag_v1(contract.order)]);
            digest.update([contract
                .failure_order
                .map_or(0, |order| memory_order_tag_v1(order).saturating_add(1))]);
            digest.update([u8::from(contract.weak)]);
        }
        KfdRuntimeSemanticLaunchV1::Collective(contract) => {
            digest.update([2, collective_operation_tag_v1(contract.operation)]);
            digest.update([memory_scope_tag_v1(contract.scope)]);
            digest.update([memory_order_tag_v1(contract.order)]);
            digest.update(contract.participants.to_le_bytes());
        }
    }
    digest.finalize().into()
}

pub(super) const fn atomic_operation_tag_v1(operation: RuntimeAtomicOperationV1) -> u8 {
    match operation {
        RuntimeAtomicOperationV1::Add => 0,
        RuntimeAtomicOperationV1::Minimum => 1,
        RuntimeAtomicOperationV1::Maximum => 2,
        RuntimeAtomicOperationV1::BitwiseAnd => 3,
        RuntimeAtomicOperationV1::BitwiseOr => 4,
        RuntimeAtomicOperationV1::BitwiseXor => 5,
        RuntimeAtomicOperationV1::Exchange => 6,
        RuntimeAtomicOperationV1::CompareExchange => 7,
    }
}

pub(super) const fn collective_operation_tag_v1(
    operation: crate::RuntimeCollectiveOperationV1,
) -> u8 {
    match operation {
        crate::RuntimeCollectiveOperationV1::Barrier => 0,
        crate::RuntimeCollectiveOperationV1::Broadcast => 1,
        crate::RuntimeCollectiveOperationV1::ReduceSum => 2,
        crate::RuntimeCollectiveOperationV1::ReduceMinimum => 3,
        crate::RuntimeCollectiveOperationV1::ReduceMaximum => 4,
        crate::RuntimeCollectiveOperationV1::AllReduceSum => 5,
        crate::RuntimeCollectiveOperationV1::InclusiveScanSum => 6,
    }
}

pub(super) const fn memory_scope_tag_v1(scope: RuntimeMemoryScopeV1) -> u8 {
    match scope {
        RuntimeMemoryScopeV1::Workgroup => 0,
        RuntimeMemoryScopeV1::Device => 1,
        RuntimeMemoryScopeV1::System => 2,
    }
}

pub(super) const fn memory_order_tag_v1(order: RuntimeMemoryOrderV1) -> u8 {
    match order {
        RuntimeMemoryOrderV1::Relaxed => 0,
        RuntimeMemoryOrderV1::Acquire => 1,
        RuntimeMemoryOrderV1::Release => 2,
        RuntimeMemoryOrderV1::AcquireRelease => 3,
        RuntimeMemoryOrderV1::SequentiallyConsistent => 4,
    }
}

pub(super) const fn atomic_contract_is_legal_v1(contract: RuntimeAtomicLaunchContractV1) -> bool {
    match (contract.operation, contract.failure_order) {
        (RuntimeAtomicOperationV1::CompareExchange, Some(failure)) => {
            compare_exchange_orders_are_legal_v1(contract.order, failure)
        }
        (RuntimeAtomicOperationV1::CompareExchange, None) => false,
        (_, None) => !contract.weak,
        (_, Some(_)) => false,
    }
}

pub(super) const fn compare_exchange_orders_are_legal_v1(
    success: RuntimeMemoryOrderV1,
    failure: RuntimeMemoryOrderV1,
) -> bool {
    match success {
        RuntimeMemoryOrderV1::Relaxed => matches!(failure, RuntimeMemoryOrderV1::Relaxed),
        RuntimeMemoryOrderV1::Acquire => matches!(
            failure,
            RuntimeMemoryOrderV1::Relaxed | RuntimeMemoryOrderV1::Acquire
        ),
        RuntimeMemoryOrderV1::Release => matches!(failure, RuntimeMemoryOrderV1::Relaxed),
        RuntimeMemoryOrderV1::AcquireRelease => matches!(
            failure,
            RuntimeMemoryOrderV1::Relaxed | RuntimeMemoryOrderV1::Acquire
        ),
        RuntimeMemoryOrderV1::SequentiallyConsistent => matches!(
            failure,
            RuntimeMemoryOrderV1::Relaxed
                | RuntimeMemoryOrderV1::Acquire
                | RuntimeMemoryOrderV1::SequentiallyConsistent
        ),
    }
}

pub(super) fn complete_workgroup_geometry_v1(geometry: crate::RuntimeLaunchGeometryV1) -> bool {
    geometry
        .grid
        .into_iter()
        .zip(geometry.workgroup)
        .all(|(grid, workgroup)| {
            workgroup != 0 && grid >= workgroup && grid.is_multiple_of(workgroup)
        })
}

pub(super) fn workgroup_participants_v1(geometry: crate::RuntimeLaunchGeometryV1) -> Option<u64> {
    geometry
        .workgroup
        .into_iter()
        .try_fold(1_u64, |product, value| {
            product.checked_mul(u64::from(value))
        })
}

pub(super) const fn atomic_profile_is_admissible_v1(
    profile: KfdRuntimeAtomicExecutionProfileV1,
) -> bool {
    if matches!(profile.scope, RuntimeMemoryScopeV1::System) {
        return false;
    }
    match (profile.operation, profile.failure_order) {
        (RuntimeAtomicOperationV1::CompareExchange, Some(failure)) => {
            compare_exchange_orders_are_legal_v1(profile.order, failure)
        }
        (RuntimeAtomicOperationV1::CompareExchange, None) => false,
        (_, None) => !profile.weak,
        (_, Some(_)) => false,
    }
}

pub(super) const fn collective_profile_is_admissible_v1(
    profile: KfdRuntimeCollectiveExecutionProfileV1,
) -> bool {
    matches!(profile.scope, RuntimeMemoryScopeV1::Workgroup)
}
pub(super) fn snapshot_three_binding_persistent_data_v1(
    allocations: &HashMap<u64, AllocationRecordV1>,
    bindings: &[BackendBindingV1],
    stream_device: u64,
    admission: ThreeBindingPersistentComputeAdmissionV1,
) -> Result<StagedDataRosterV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let current = three_binding_persistent_compute_admission_v1(
        KfdRuntimeSemanticLaunchV1::Ordinary,
        bindings,
        stream_device,
        allocations,
    );
    if current != Some(admission) {
        return Err(KfdRuntimeBackendV1::rejected(
            KfdRuntimeBackendErrorKindV1::Busy,
            "three-binding persistent admission changed while snapshotting",
        ));
    }
    let mut data = Vec::with_capacity(3);
    let mut placements = HashMap::with_capacity(3);
    for (index, binding) in bindings.iter().enumerate() {
        let allocation = allocations
            .get(&binding.region.allocation)
            .expect("revalidated three-binding allocation");
        data.push(DataSpecV1 {
            allocation: binding.region.allocation,
            kind: allocation.kind,
            alignment: allocation.alignment,
            allocation_offset: 0,
            bytes: Arc::clone(&allocation.bytes),
            byte_range: 0..allocation.bytes.len(),
            content_sha256: allocation.content_sha256,
        });
        placements.insert(
            binding.region.allocation,
            StagedPlacementV1 {
                data_index: index,
                allocation_offset: 0,
            },
        );
    }
    Ok(StagedDataRosterV1 { data, placements })
}

pub(super) fn snapshot_persistent_full_range_data_v1(
    allocations: &HashMap<u64, AllocationRecordV1>,
    binding: &BackendBindingV1,
    stream_device: u64,
    admission: PersistentFullRangeComputeAdmissionV1,
    retained: Option<RetainedPersistentDispatchV1>,
    dispatch_shape_sha256: [u8; 32],
) -> Result<StagedDataRosterV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    if admission.allocation != binding.region.allocation
        || admission.access != binding.region.access
    {
        return Err(KfdRuntimeBackendV1::rejected(
            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
            "persistent-compute admission no longer matches its binding",
        ));
    }
    let allocation = allocations.get(&admission.allocation).ok_or_else(|| {
        KfdRuntimeBackendV1::rejected(
            KfdRuntimeBackendErrorKindV1::UnknownHandle,
            "persistent-compute allocation disappeared",
        )
    })?;
    let current = match admission.source {
        PersistentFullRangeComputeSourceV1::AuthenticatedH2d => allocation
            .sdma_storage
            .persistent_compute_ready_facts_v1()
            .and_then(|ready| {
                persistent_full_range_compute_admission_v1(
                    KfdRuntimeSemanticLaunchV1::Ordinary,
                    core::slice::from_ref(binding),
                    stream_device,
                    Some(allocation),
                    Some(ready),
                )
            }),
        PersistentFullRangeComputeSourceV1::RetainedControlReplay => {
            retained.and_then(|retained| {
                retained_persistent_full_range_compute_admission_v1(
                    KfdRuntimeSemanticLaunchV1::Ordinary,
                    core::slice::from_ref(binding),
                    stream_device,
                    Some(allocation),
                    retained,
                    dispatch_shape_sha256,
                )
            })
        }
    };
    current
        .filter(|actual| *actual == admission)
        .ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "persistent-compute admission changed while snapshotting",
            )
        })?;
    let mut data = Vec::new();
    data.try_reserve_exact(1)
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD persistent snapshot allocation failed"))?;
    data.push(DataSpecV1 {
        allocation: admission.allocation,
        kind: allocation.kind,
        alignment: allocation.alignment,
        allocation_offset: 0,
        bytes: Arc::clone(&allocation.bytes),
        byte_range: 0..allocation.bytes.len(),
        content_sha256: allocation.content_sha256,
    });
    let mut placements = HashMap::new();
    placements
        .try_reserve(1)
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD persistent placement allocation failed"))?;
    placements.insert(
        admission.allocation,
        StagedPlacementV1 {
            data_index: 0,
            allocation_offset: 0,
        },
    );
    Ok(StagedDataRosterV1 { data, placements })
}

pub(super) fn snapshot_bound_data_v1(
    allocations: &HashMap<u64, AllocationRecordV1>,
    bindings: &[BackendBindingV1],
    stream_device: u64,
) -> Result<StagedDataRosterV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let mut ranges = HashMap::<u64, (u64, u64)>::new();
    let mut order = Vec::<u64>::new();
    ranges
        .try_reserve(bindings.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD staged-range map allocation failed"))?;
    order
        .try_reserve_exact(bindings.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD staged-range order allocation failed"))?;

    for binding in bindings {
        let region = binding.region;
        let allocation = allocations.get(&region.allocation).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD allocation",
            )
        })?;
        if allocation.device != stream_device {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "allocation and stream belong to different devices",
            ));
        }
        if allocation.kind == RuntimeMemoryKindV1::DeviceLocal
            && region.access != RuntimeAccessV1::Read
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "device-local writeback is unavailable without an admitted copy path",
            ));
        }
        let range_end = region
            .byte_offset
            .checked_add(region.byte_len)
            .ok_or_else(|| {
                KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "binding range overflow",
                )
            })?;
        if region.byte_len == 0 || range_end > allocation.bytes.len() as u64 {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "binding lies outside its allocation",
            ));
        }
        let aligned_start = region.byte_offset & !(allocation.alignment - 1);
        if let Some((start, end)) = ranges.get_mut(&region.allocation) {
            *start = (*start).min(aligned_start);
            *end = (*end).max(range_end);
        } else {
            if order.len() == GFX942_MAX_FIXED_DISPATCH_DATA_V1 {
                return Err(KfdRuntimeBackendV1::capacity(
                    "fixed KFD dispatch data roster is full",
                ));
            }
            ranges.insert(region.allocation, (aligned_start, range_end));
            order.push(region.allocation);
        }
    }

    let mut data = Vec::new();
    let mut placements = HashMap::new();
    data.try_reserve_exact(order.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD staged-data roster allocation failed"))?;
    placements
        .try_reserve(order.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD staged-placement map allocation failed"))?;
    for allocation_id in order {
        let allocation = &allocations[&allocation_id];
        let (start, end) = ranges[&allocation_id];
        let start_index = usize::try_from(start).map_err(|_| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "staged allocation offset does not fit host address space",
            )
        })?;
        let end_index = usize::try_from(end).map_err(|_| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "staged allocation end does not fit host address space",
            )
        })?;
        let data_index = data.len();
        data.push(DataSpecV1 {
            allocation: allocation_id,
            kind: allocation.kind,
            alignment: allocation.alignment,
            allocation_offset: start,
            bytes: Arc::clone(&allocation.bytes),
            byte_range: start_index..end_index,
            content_sha256: (start_index == 0 && end_index == allocation.bytes.len())
                .then_some(allocation.content_sha256)
                .flatten(),
        });
        placements.insert(
            allocation_id,
            StagedPlacementV1 {
                data_index,
                allocation_offset: start,
            },
        );
    }
    Ok(StagedDataRosterV1 { data, placements })
}

pub(super) fn build_program_v1<'a>(
    program: &'a OwnedValidatedKernelEnvelope,
    signature: [u8; 32],
    owned_rows: &[OwnedAbiRowV1],
) -> Result<ValidatedKernelEnvelope<'a>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let arguments = program.selected_kernel().explicit_arguments();
    let mut rows = Vec::new();
    rows.try_reserve_exact(owned_rows.len()).map_err(|_| {
        KfdRuntimeBackendV1::capacity("KFD reconciled ABI roster allocation failed")
    })?;
    for row in owned_rows {
        let name = arguments[row.explicit_argument_index]
            .name()
            .expect("prepared global-buffer ABI row retains a source name");
        rows.push(KernelGlobalBufferAbiV1::new(
            row.explicit_argument_index,
            name,
            row.offset,
            row.pointee_alignment,
            row.access,
        ));
    }
    program
        .validated()
        .reconcile_dispatch_abi(signature, &rows)
        .map_err(|error| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                format!("typed AMDHSA dispatch ABI: {error:?}"),
            )
        })
}

pub(super) fn materialize_initial_data_v1(
    memory: &mut SharedGttMemorySessionV1,
    specs: Vec<DataSpecV1>,
    role_identity: [u8; 32],
) -> Result<Vec<Gfx942FixedDispatchDataV1>, String> {
    let mut data = Vec::new();
    data.try_reserve_exact(specs.len())
        .map_err(|_| "KFD native-data roster allocation failed".to_owned())?;
    for (index, spec) in specs.into_iter().enumerate() {
        let item = match spec.kind {
            RuntimeMemoryKindV1::HostVisible => memory
                .initialize_host_visible_coherent_from_slice_v1(spec.bytes())
                .map(Gfx942FixedDispatchDataV1::host_visible_initialized)
                .map_err(|error| format!("KFD host-visible initialization: {error}"))?,
            RuntimeMemoryKindV1::DeviceLocal => {
                let owned_bytes = spec.try_owned_bytes()?;
                let ordinal = u32::try_from(index)
                    .map_err(|_| "KFD device-content ordinal does not fit u32".to_owned())?;
                let role = Gfx942DeviceContentRoleV1::new(role_identity, ordinal)
                    .map_err(|error| format!("KFD device-content role: {error}"))?;
                let content = Gfx942DeviceContentDescriptorV1::from_bytes(role, &owned_bytes)
                    .map_err(|error| format!("KFD device-content descriptor: {error}"))?;
                memory
                    .initialize_gfx942_device_memory(owned_bytes, spec.alignment, content)
                    .map(Gfx942FixedDispatchDataV1::initialized)
                    .map_err(|error| format!("KFD device-local initialization: {error}"))?
            }
        };
        data.push(item);
    }
    Ok(data)
}

pub(super) fn resident_descriptors_v1(
    specs: &[DataSpecV1],
) -> Result<Vec<ResidentDataDescriptorV1>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let mut descriptors = Vec::new();
    descriptors
        .try_reserve_exact(specs.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD resident-data roster allocation failed"))?;
    for spec in specs {
        descriptors.push(ResidentDataDescriptorV1 {
            allocation: spec.allocation,
            kind: spec.kind,
            alignment: spec.alignment,
            allocation_offset: spec.allocation_offset,
            byte_len: u64::try_from(spec.bytes().len()).map_err(|_| {
                KfdRuntimeBackendV1::capacity("KFD resident-data extent does not fit u64")
            })?,
            host_content_sha256: spec.content_sha256,
            device_may_have_modified: false,
        });
    }
    Ok(descriptors)
}

pub(super) fn same_resident_storage_shape_v1(
    left: &[ResidentDataDescriptorV1],
    right: &[ResidentDataDescriptorV1],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.allocation == right.allocation
                && left.kind == right.kind
                && left.alignment == right.alignment
                && left.allocation_offset == right.allocation_offset
                && left.byte_len == right.byte_len
        })
}

pub(super) fn release_resident_data_v1(
    queue: &mut ComputeAqlQueueLaneDispatchV1<'_>,
    resident: ResidentDataRosterV1,
) -> Result<(), String> {
    for data in resident.data {
        queue
            .release_detached_fixed_dispatch_data(data)
            .map_err(|error| format!("KFD resident-data release: {error}"))?;
    }
    Ok(())
}

pub(super) fn materialize_rebound_data_v1(
    queue: &mut ComputeAqlQueueLaneDispatchV1<'_>,
    specs: Vec<DataSpecV1>,
    role_identity: [u8; 32],
) -> Result<Vec<Gfx942FixedDispatchDataV1>, String> {
    let mut data = Vec::new();
    data.try_reserve_exact(specs.len())
        .map_err(|_| "KFD rebound-data roster allocation failed".to_owned())?;
    for (index, spec) in specs.into_iter().enumerate() {
        queue
            .preflight_fixed_dispatch_data_insertion(index)
            .map_err(|error| format!("KFD dispatch-data insertion preflight: {error}"))?;
        let item = match spec.kind {
            RuntimeMemoryKindV1::HostVisible => queue
                .insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(
                    index,
                    spec.bytes(),
                )
                .map_err(|error| format!("KFD host-visible insertion: {error}"))?,
            RuntimeMemoryKindV1::DeviceLocal => {
                let owned_bytes = spec.try_owned_bytes()?;
                let ordinal = u32::try_from(index)
                    .map_err(|_| "KFD device-content ordinal does not fit u32".to_owned())?;
                let role = Gfx942DeviceContentRoleV1::new(role_identity, ordinal)
                    .map_err(|error| format!("KFD device-content role: {error}"))?;
                let content = Gfx942DeviceContentDescriptorV1::from_bytes(role, &owned_bytes)
                    .map_err(|error| format!("KFD device-content descriptor: {error}"))?;
                queue
                    .insert_initialized_fixed_dispatch_data(
                        index,
                        owned_bytes,
                        spec.alignment,
                        content,
                    )
                    .map_err(|error| format!("KFD device-local insertion: {error}"))?
            }
        };
        data.push(item);
    }
    Ok(data)
}

#[cfg(test)]
mod borrowed_initialization_tests {
    use super::*;

    #[test]
    fn borrowed_initialization_runtime_preserves_both_materializers_and_device_owned_path() {
        let source = include_str!("compute_dispatch.rs");
        for (name, next, borrowed) in [
            (
                "materialize_initial_data_v1",
                "resident_descriptors_v1",
                "initialize_host_visible_coherent_from_slice_v1(spec.bytes())",
            ),
            (
                "materialize_rebound_data_v1",
                "unused_end_marker",
                "insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(index,spec.bytes()",
            ),
        ] {
            let start = format!("pub(super) fn {name}(");
            let end = format!("pub(super) fn {next}(");
            let body = source
                .split(&start)
                .nth(1)
                .unwrap()
                .split(&end)
                .next()
                .unwrap()
                .split("#[cfg(test)]")
                .next()
                .unwrap();
            let (before_device, device) = body
                .split_once("RuntimeMemoryKindV1::DeviceLocal =>")
                .unwrap();
            let host = before_device
                .split_once("RuntimeMemoryKindV1::HostVisible =>")
                .unwrap()
                .1;
            let normalized: String = host.chars().filter(|ch| !ch.is_whitespace()).collect();
            assert!(normalized.contains(borrowed));
            for forbidden in ["try_owned_bytes", "to_vec(", "to_owned(", "Box::from("] {
                assert!(!host.contains(forbidden));
            }
            assert!(device.contains("let owned_bytes = spec.try_owned_bytes()?"));
            assert!(device.contains("Gfx942DeviceContentDescriptorV1::from_bytes"));
            assert!(!body.contains("submit_"));
        }
    }

    #[test]
    fn borrowed_initialization_runtime_views_need_no_encoded_host_allocation() {
        let bytes: Arc<[u8]> = Arc::from([0x5a; 97]);
        for range in [0..97, 3..91] {
            let spec = DataSpecV1 {
                allocation: 7,
                kind: RuntimeMemoryKindV1::HostVisible,
                alignment: 4,
                allocation_offset: range.start as u64,
                bytes: Arc::clone(&bytes),
                byte_range: range.clone(),
                content_sha256: None,
            };
            let (view, allocations) = super::super::drain_capture::tests::counted(|| spec.bytes());
            assert_eq!(allocations, 0);
            assert!(std::ptr::eq(view.as_ptr(), bytes[range.clone()].as_ptr()));
            assert_eq!(view, &bytes[range]);
            let (owned, allocations) =
                super::super::drain_capture::tests::counted(|| spec.try_owned_bytes().unwrap());
            assert!(allocations >= 1);
            assert_eq!(&*owned, view);
            assert_ne!(owned.as_ptr(), view.as_ptr());
        }
    }
}
