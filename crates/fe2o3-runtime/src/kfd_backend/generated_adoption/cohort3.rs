//! Fresh-primary exact-three construction using original DATA and packet owners.

use super::*;
use crate::generated_source::GeneratedProfileV1;
use fe2o3_kfd::{Gfx942NativeFillCohortMemberV1, Gfx942NativeFillCohortV1};
use generated_shells::GeneratedControlV1;

impl KfdRuntimeBackendV1 {
    pub(crate) fn preflight_generated_cohort3_lane_v1(
        &self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        self.require_default_dispatch_capacity_v1()?;
        if self.queue.is_some()
            || self.terminal_memory.is_some()
            || self.admitted_device.is_none()
            || self.native_compute_lanes.iter().any(Option::is_some)
            || !self.stream_compute_lanes.is_empty()
            || !self.pending_compute_streams.is_empty()
            || !self.active_sdma_streams.is_empty()
            || self.persistent_compute_is_active_v1()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native cohort3 requires an unused primary queue owner",
            ));
        }
        Ok(())
    }

    pub(crate) fn adopt_generated_cohort3_data_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
        programs: [ValidatedKernelEnvelope<'_>; 3],
        buffers: [&[crate::Gfx942KfdDispatchBufferV1]; 3],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.preflight_generated_cohort3_lane_v1()?;
        if self.requires_request_witness_v1()
            && self
                .composed_request_binding
                .as_ref()
                .is_none_or(|binding| !binding.is_live())
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "generated request session is unavailable",
            ));
        }
        if plan.profile != GeneratedProfileV1::NativeFillCohort3
            || plan.count != 3
            || roster.count != 3
            || !self.validate_generated_shell_records_v1(plan)
            || !self.generated_shells.get(&plan.key).is_some_and(|record| {
                record.native.is_none()
                    && record.control.is_some()
                    && record.source_identity.matches(&roster.source_identity)
            })
            || buffers.iter().enumerate().any(|(i, buffers)| {
                buffers.len() != 1
                    || plan.members[i].is_none_or(|member| {
                        member.description.byte_len != buffers[0].bytes().len() as u64
                    })
            })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "native cohort3 original source, control or output mismatch",
            ));
        }
        let current = self
            .with_retained_preparation_device_v1(plan.binding.backend_device, |device| {
                device.model_admission()
            })?;
        if current != plan.binding.native_device {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "native cohort3 device mismatch",
            ));
        }
        let mut data = Vec::new();
        data.try_reserve_exact(3)
            .map_err(|_| Self::capacity("native cohort3 DATA roster"))?;
        if data.capacity() > fe2o3_kfd::GFX942_MAX_FIXED_DISPATCH_DATA_V1 {
            return Err(Self::capacity("native cohort3 actual DATA capacity"));
        }
        self.stream_compute_lanes
            .try_reserve(1)
            .map_err(|_| Self::capacity("native cohort3 lane lease"))?;
        self.generated_shells
            .get_mut(&plan.key)
            .expect("validated cohort3 shell")
            .native = Some(GeneratedNativeAdoptionV1 {
            phase: PhaseV1::Entering,
            lane: 0,
            native_lane: None,
            data,
            detached: detached::RetainedDetachedV1::empty(),
            returned: ReturnedDataV1::empty(),
            submission: None,
            sdma: sdma_backing::NativeCustody::empty(),
            copy: None,
        });
        self.lease_compute_lane_v1(plan.binding.backend_stream, 0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.acquire_generated_vm_v1()?;
            let initialized = {
                let memory = self.terminal_memory.as_mut().expect("rooted cohort3 VM");
                let native = self
                    .generated_shells
                    .get_mut(&plan.key)
                    .expect("rooted cohort3 shell")
                    .native
                    .as_mut()
                    .expect("cohort3 entering owner");
                buffers.into_iter().try_for_each(|buffers| {
                    initialize_generated_data_v1(memory, buffers, &mut native.data)
                })
            };
            initialized.map_err(|error| {
                self.generated_native_error_v1("cohort3 DATA initialization", error)
            })?;
            self.bind_generated_cohort3_primary_v1(plan.key, programs)?;
            self.check_generated_device_v1(plan)?;
            self.generated_shells
                .get_mut(&plan.key)
                .expect("rooted cohort3 shell")
                .native
                .as_mut()
                .expect("cohort3 native owner")
                .phase = PhaseV1::Adopted;
            Ok(())
        }));
        self.finish_generated_native_call_v1(result)
    }

    fn bind_generated_cohort3_primary_v1(
        &mut self,
        key: u64,
        programs: [ValidatedKernelEnvelope<'_>; 3],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let record = self
            .generated_shells
            .get_mut(&key)
            .expect("rooted cohort3 shell");
        let GeneratedControlV1::Cohort3(packets) = &mut record.control else {
            std::process::abort();
        };
        let native = record.native.as_mut().expect("cohort3 entering owner");
        if native.data.len() != 3 || packets.iter().any(Option::is_none) {
            std::process::abort();
        }
        // Keep the original preallocated Vec backing in the backend for refusal.
        let mut data = [native.data.pop(), native.data.pop(), native.data.pop()];
        let mut index = 0;
        let members = programs.map(|program| {
            let member = Gfx942NativeFillCohortMemberV1::new(
                program,
                packets[index]
                    .take()
                    .expect("exact original cohort3 control"),
                data[2 - index].take().expect("exact original cohort3 DATA"),
            );
            index += 1;
            member
        });
        let cohort = match Gfx942NativeFillCohortV1::admit(members) {
            Ok(cohort) => cohort,
            Err(failure) => {
                let (members, error) = failure.into_parts();
                for (index, member) in members.into_iter().enumerate() {
                    let (_, packet, data) = member.into_parts();
                    packets[index] = Some(packet);
                    native.data.push(data);
                }
                return Err(self.generated_native_error_v1("cohort3 admission", error));
            }
        };
        let queue = self
            .terminal_memory
            .take()
            .expect("original cohort3 VM")
            .create_compute_aql_queue_with_native_fill_cohort_v1(KFD_RUNTIME_RING_BYTES_V1, cohort)
            .map_err(|error| {
                self.generated_native_error_v1("cohort3 primary construction", error)
            })?;
        let lane = queue.primary_compute_lane_v1();
        self.queue = Some(queue);
        self.native_compute_lanes[0] = Some(lane);
        self.generated_shells
            .get_mut(&key)
            .expect("rooted cohort3 shell")
            .native
            .as_mut()
            .expect("cohort3 entering owner")
            .native_lane = Some(lane);
        self.observe_generated_queue_creation_v1(0);
        self.configure_native_device_pool_v1()?;
        self.configure_native_host_pool_v1()?;
        Ok(())
    }
}
