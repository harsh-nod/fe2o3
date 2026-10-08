//! A device-local outcome exists only before this original child acquired a VM.

use super::*;
use fe2o3_kfd::{Gfx942ColdDeviceCheckV1, Gfx942ColdDeviceRefusalV1, Gfx942ColdDeviceResetV1};

mod custody;
pub(in crate::kfd_backend) use custody::DeviceCustodyV1;

#[derive(Debug)]
pub(in crate::kfd_backend) enum ColdDeviceV1 {
    Reset(Gfx942ColdDeviceResetV1),
    Refused(Gfx942ColdDeviceRefusalV1),
}

/// Private join witness; the actual reset owner stays in the original backend.
/// Neither its cached coordinates nor a backend Rejected error can mint this.
pub(crate) struct GeneratedColdDeviceFailureV1 {
    scope: GeneratedAdoptionScopeV1,
    native_device: fe2o3_runtime_model::ModelDeviceAdmissionV1,
    device_uid: u64,
}

impl GeneratedColdDeviceFailureV1 {
    pub(crate) const fn device_uid(&self) -> u64 {
        self.device_uid
    }

    // Synthetic scope tests never install a reset owner or enter native code.
    // Production validation still requires the actual retained lower owner.
    #[cfg(test)]
    pub(crate) fn synthetic_for_test(
        scope: GeneratedAdoptionScopeV1,
        native_device: fe2o3_runtime_model::ModelDeviceAdmissionV1,
    ) -> Self {
        Self {
            scope,
            native_device,
            device_uid: scope.device,
        }
    }
}

impl KfdRuntimeBackendV1 {
    // Streams and prepaid empty host metadata are permitted; no allocation,
    // queue, event, receipt, peer gate, or native continuation is permitted.
    fn cold_native_empty_v1(&self) -> bool {
        let empty = self.queue.is_none()
            && self.primary_teardown.is_none()
            && self.terminal_memory.is_none()
            && self.terminal_sdma_custody.is_none()
            && self.allocations.is_empty()
            && self.generated_shells.is_empty()
            && self.generated_submissions.is_empty()
            && self.modules.is_empty()
            && self.kernels.is_empty()
            && self.submissions.is_empty()
            && self.compute_completion_reservations == 0
            && self.sdma_completion_reservations == 0
            && self.pending_compute.is_empty()
            && self.terminal_pending_compute.is_none()
            && !self.has_admitted_peer_gate
            && self.pending_compute_streams.is_empty()
            && self.allocation_custody.is_empty()
            && self.compute_module_retain_counts.is_empty()
            && self.compute_dependency_retain_counts.is_empty()
            && self.stream_submission_tails.is_empty()
            && self.events.is_empty()
            && self.event_submission_retain_counts.is_empty()
            && self.active.is_none()
            && self.compute_pipeline.is_empty()
            && self.resident_data.is_none()
            && self.recycled_dispatch.is_none()
            && self.retained_persistent_dispatch.is_none()
            && self.auxiliary_compute_lanes.iter().all(|lane| {
                lane.owner_stream.is_none()
                    && lane.active.is_none()
                    && lane.pipeline.is_empty()
                    && lane.resident_data.is_none()
                    && lane.recycled_dispatch.is_none()
            })
            && self.native_compute_lanes.iter().all(Option::is_none)
            && self.selected_compute_lane == 0
            && self.stream_compute_lanes.is_empty()
            && self.native_dirty_extents == 0
            && self.native_reconciliations.iter().all(Option::is_none)
            && self.active_sdma.is_empty()
            && self.published_sdma_submissions.is_empty()
            && self.active_sdma_streams.is_empty()
            && self.sdma_dependency_retain_counts.is_empty()
            && self.quiescent_sdma_submissions.is_empty()
            && self.rooted_backing.is_none()
            && self.staged_context_bytes == 0
            && !self.sdma_enabled;
        #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
        let empty = empty && self.cpu_queue.is_none();
        empty
    }

    pub(in crate::kfd_backend) fn require_live_or_cold_metadata_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.admitted_device.cold().is_none() {
            return self.require_live();
        }
        if self.terminal || !self.cold_native_empty_v1() || self.admitted_device.is_some() {
            return Err(self.terminal_error("cold device custody is inconsistent"));
        }
        let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            match self.admitted_device.cold() {
                Some(ColdDeviceV1::Reset(owner)) => owner.check_process().map_err(|_| ()),
                Some(ColdDeviceV1::Refused(owner)) => {
                    let _ = owner.reason();
                    Err(())
                }
                None => Err(()),
            }
        }));
        match checked {
            Ok(Ok(())) => Ok(()),
            Ok(Err(())) => Err(self.terminal_error("original cold device process check failed")),
            Err(payload) => {
                self.poison_terminal_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    pub(crate) fn generated_cold_scope_v1(
        &mut self,
        device: u64,
        stream: u64,
    ) -> Result<GeneratedAdoptionScopeV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live_or_cold_metadata_v1()?;
        self.require_device(device)?;
        if self.streams.get(&stream) != Some(&device) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "cold stream",
            ));
        }
        Ok(GeneratedAdoptionScopeV1 {
            child: None,
            device,
            stream,
            local_stream: stream,
        })
    }

    pub(crate) fn check_generated_cold_device_v1(
        &mut self,
        scope: GeneratedAdoptionScopeV1,
        native_device: fe2o3_runtime_model::ModelDeviceAdmissionV1,
    ) -> Result<
        Option<GeneratedColdDeviceFailureV1>,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        if scope != self.generated_cold_scope_v1(scope.device, scope.stream)? {
            return Err(self.terminal_error("cold device scope substitution"));
        }
        if self.admitted_device.cold().is_none() {
            // Warm paths keep their existing currentness and global-terminal
            // behavior. This path does not classify an error returned by them.
            if self.admitted_device.is_none() || !self.cold_native_empty_v1() {
                return Ok(None);
            }
            let Some(owner) = self.admitted_device.as_ref() else {
                return Err(self.terminal_error("cold original disappeared before check"));
            };
            if owner.model_admission() != native_device
                || owner.observation().unique_id() != scope.device
            {
                return Err(self.terminal_error("cold original generation mismatch"));
            }
            // Every result roots the original before any fallible operation.
            let Some(owner) = self.admitted_device.take() else {
                return Err(self.terminal_error("cold original disappeared before transfer"));
            };
            match owner.check_before_first_native_activation_v1() {
                Gfx942ColdDeviceCheckV1::Ready(owner) => {
                    self.admitted_device = DeviceCustodyV1::Ready(owner);
                    return Ok(None);
                }
                Gfx942ColdDeviceCheckV1::ResetObserved(owner) => {
                    self.admitted_device = DeviceCustodyV1::Cold(ColdDeviceV1::Reset(owner));
                }
                Gfx942ColdDeviceCheckV1::Refused(owner) => {
                    self.admitted_device = DeviceCustodyV1::Cold(ColdDeviceV1::Refused(owner));
                    return Err(self.terminal_error("unclassified original cold device failure"));
                }
            }
        }
        self.require_live_or_cold_metadata_v1()?;
        let Some(ColdDeviceV1::Reset(owner)) = self.admitted_device.cold() else {
            return Err(self.terminal_error("cold original disappeared"));
        };
        if owner.model_admission() != native_device
            || owner.observation().unique_id() != scope.device
        {
            return Err(self.terminal_error("cold retained generation mismatch"));
        }
        Ok(Some(GeneratedColdDeviceFailureV1 {
            scope,
            native_device,
            device_uid: scope.device,
        }))
    }

    pub(crate) fn validate_generated_cold_failure_v1(
        &mut self,
        failure: &GeneratedColdDeviceFailureV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let actual = self.check_generated_cold_device_v1(failure.scope, failure.native_device)?;
        if actual.is_some_and(|actual| actual.device_uid == failure.device_uid) {
            Ok(())
        } else {
            Err(self.terminal_error("cold outcome lost its original unavailable owner"))
        }
    }
}

#[cfg(test)]
mod tests;

impl KfdMultiDeviceRuntimeBackendV1 {
    fn cold_route_empty_v1(&self, child: usize) -> bool {
        !self
            .compute_xgmi_routes
            .keys()
            .any(|&(a, b)| a == child || b == child)
            && self.compute_xgmi_children.get(child) == Some(&None)
            && [
                &self.allocations,
                &self.generated_allocations,
                &self.modules,
                &self.kernels,
            ]
            .iter()
            .all(|map| map.values().all(|route| route.child != child))
            && self
                .generated_shells
                .values()
                .all(|plan| plan.scope.child != Some(child))
            && self.generated_submissions.values().all(|submission| {
                self.generated_shells
                    .get(&submission.shell_key())
                    .is_some_and(|plan| plan.scope.child != Some(child))
            })
            && self
                .submissions
                .values()
                .all(|submission| match submission {
                    RoutedSubmissionV1::Native { route, .. } => route.child != child,
                    RoutedSubmissionV1::CooperativeCopy(copy) => {
                        copy.source.child != child && copy.destination.child != child
                    }
                    // First slice excludes the hidden retained peer ancestry even
                    // when its final consumer is on another child.
                    RoutedSubmissionV1::DeferredCompute(_) => false,
                })
            && self.events.values().all(|event| match event {
                RoutedEventV1::Native { route, .. } => route.child != child,
                RoutedEventV1::CooperativeCopy { child: owner, .. }
                | RoutedEventV1::DeferredCompute { child: owner, .. } => *owner != child,
            })
            && self
                .cooperative_allocation_owners
                .keys()
                .all(|route| route.child != child)
            && [
                &self.cooperative_stream_pending_counts,
                &self.native_stream_submission_counts,
            ]
            .iter()
            .all(|counts| {
                counts.keys().all(|stream| {
                    self.streams
                        .get(stream)
                        .is_some_and(|route| route.child != child)
                })
            })
            && self.cooperative_stream_tails.keys().all(|stream| {
                self.streams
                    .get(stream)
                    .is_some_and(|route| route.child != child)
            })
            && self.kernel_modules.iter().all(|(kernel, module)| {
                self.kernels
                    .get(kernel)
                    .is_some_and(|route| route.child != child)
                    && self
                        .modules
                        .get(module)
                        .is_some_and(|route| route.child != child)
            })
            && self.cooperative_dependency_retain_counts.is_empty()
            && self.event_submission_retain_counts.is_empty()
            && self.peer_launch_retains.is_empty()
            && self.deferred_compute_retains.is_empty()
            && self.producer_aware_native.is_empty()
    }

    pub(crate) fn generated_cold_scope_v1(
        &mut self,
        device: u64,
        stream: u64,
    ) -> Result<GeneratedAdoptionScopeV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        let child = self.child_for_device(device)?;
        let route = Self::route(&self.streams, stream, "unknown cold generated stream")?;
        if route.child != child || self.compute_xgmi_children.len() != self.children.len() {
            self.terminal = true;
            return Err(RuntimeBackendFailureV1::Terminal(
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "cold original route mismatch",
                ),
            ));
        }
        let result = self.children[child].generated_cold_scope_v1(device, route.local);
        self.latch(result)?;
        Ok(GeneratedAdoptionScopeV1 {
            child: Some(child),
            device,
            stream,
            local_stream: route.local,
        })
    }

    pub(crate) fn check_generated_cold_device_v1(
        &mut self,
        scope: GeneratedAdoptionScopeV1,
        native_device: fe2o3_runtime_model::ModelDeviceAdmissionV1,
    ) -> Result<
        Option<GeneratedColdDeviceFailureV1>,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        if self.generated_cold_scope_v1(scope.device, scope.stream)? != scope {
            self.quarantine_generated_scope_v1(scope);
            return Err(RuntimeBackendFailureV1::Terminal(
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "cold route substitution",
                ),
            ));
        }
        let Some(child) = scope.child else {
            self.quarantine_generated_scope_v1(scope);
            return Err(RuntimeBackendFailureV1::Terminal(
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "cold route lost child",
                ),
            ));
        };
        if !self.cold_route_empty_v1(child) {
            if self.children[child].admitted_device.cold().is_some() {
                self.quarantine_generated_scope_v1(scope);
                return Err(RuntimeBackendFailureV1::Terminal(
                    KfdRuntimeBackendErrorV1::new(
                        KfdRuntimeBackendErrorKindV1::Terminal,
                        "unavailable child acquired routed custody",
                    ),
                ));
            }
            return Ok(None);
        }
        let local = GeneratedAdoptionScopeV1 {
            child: None,
            stream: scope.local_stream,
            ..scope
        };
        let result = self.children[child].check_generated_cold_device_v1(local, native_device);
        Ok(self
            .latch(result)?
            .map(|failure| GeneratedColdDeviceFailureV1 { scope, ..failure }))
    }

    pub(crate) fn validate_generated_cold_failure_v1(
        &mut self,
        failure: &GeneratedColdDeviceFailureV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let actual = self.check_generated_cold_device_v1(failure.scope, failure.native_device)?;
        if actual.is_some_and(|actual| actual.device_uid == failure.device_uid) {
            Ok(())
        } else {
            self.quarantine_generated_scope_v1(failure.scope);
            Err(RuntimeBackendFailureV1::Terminal(
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "cold routed owner disappeared",
                ),
            ))
        }
    }
}
