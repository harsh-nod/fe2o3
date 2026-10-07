//! Fixed-dispatch and persistent-compute orchestration for a live gfx942 queue.

use super::*;

include!("../queue_completion/source_rollback_body.rs");
include!("../queue_completion/source_publish_body.rs");
include!("dependency_source_failure_body.rs");
include!("dependency_source_output_body.rs");

#[cfg(test)]
pub(super) use {dependency_source_failure_body, dependency_source_recipe_cancel_call};

macro_rules! dependency_source_rust_expr {
    ($body:expr) => {
        $body
    };
}

// Keep source publication and rollback identical for native recipes and CPU
// receipt tests. Only the native adapter can access retained device authority.
pub(super) trait DependencySourceRecipeV1<const N: usize> {
    fn bind(
        &mut self,
        session: &mut ComputeAqlQueueSessionV1,
    ) -> Result<
        (
            Box<[CompletionPacketTemplateV1; N]>,
            DispatchEpochIdentityV1,
        ),
        Gfx942DispatchBindingErrorV1,
    >;

    fn mark_published(
        &mut self,
        session: &mut ComputeAqlQueueSessionV1,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletionBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1>;

    fn cancel(
        &mut self,
        session: &mut ComputeAqlQueueSessionV1,
        identity: DispatchEpochIdentityV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1>;
}

pub(super) struct NativeDependencySourceRecipeV1;

impl<const N: usize> DependencySourceRecipeV1<N> for NativeDependencySourceRecipeV1 {
    fn bind(
        &mut self,
        session: &mut ComputeAqlQueueSessionV1,
    ) -> Result<
        (
            Box<[CompletionPacketTemplateV1; N]>,
            DispatchEpochIdentityV1,
        ),
        Gfx942DispatchBindingErrorV1,
    > {
        session
            .dispatch
            .as_mut()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .bind_templates::<N>(session.key)
    }

    fn mark_published(
        &mut self,
        session: &mut ComputeAqlQueueSessionV1,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletionBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        session
            .dispatch
            .as_mut()
            .expect("dependency source dispatch owner remains retained")
            .mark_published(identity, completion)
    }

    fn cancel(
        &mut self,
        session: &mut ComputeAqlQueueSessionV1,
        identity: DispatchEpochIdentityV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        native_dependency_source_cancel_body!(dependency_source_rust_expr, session, identity)
    }
}

impl CheckedGfx942XnackMinusDevice {
    /// Private source-complete preparation path. There is intentionally no
    /// safe public producer for its data premises or typed kernarg images.
    #[allow(dead_code)]
    pub(crate) fn create_compute_aql_queue_with_dispatch<const N: usize>(
        self,
        ring_bytes: u32,
        kernel: fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>,
        geometry: [DispatchGeometryV1; N],
        kernargs: [TypedKernargImageV1; N],
        data: Vec<DeviceDataAllocationInputV1>,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        super::super::dispatch_binding::validate_gfx942_kernel_profiles(core::slice::from_ref(
            &kernel,
        ))?;
        validate_fixed_batch_ring::<N>(ring_bytes)?;
        let geometry_plan = plan_gfx942_aql_queue_resources(
            self.topology_snapshot(),
            self.observation().unique_id(),
            ring_bytes,
        )?;
        let memory = self.acquire_shared_gtt_memory_session()?;
        ComputeAqlQueueSessionV1::create_compute_aql_queue_inner(
            memory,
            geometry_plan,
            ring_bytes,
            QueueRingBackingV1::AqlSpecial,
            move |memory| {
                prepare_dispatch_resources(memory, kernel, geometry, kernargs, data)
                    .map(Some)
                    .map_err(ComputeAqlQueueSessionErrorV1::DispatchBinding)
            },
            None,
        )
    }
}

impl SharedGttMemorySessionV1 {
    /// Creates one long-lived compute-AQL queue from this exact KFD VM session
    /// and consumes all fixed-batch executable, kernarg, and device-storage
    /// authority into it.
    ///
    /// The operation does not expose native addresses. Inspected global-buffer
    /// access determines whether each referenced move-only storage input must
    /// carry sealed initialization authority. Every mapped storage input and
    /// inspected program is retained even when no packet in this batch selects
    /// it. Queue creation does not establish
    /// kernel numerical correctness, memory-effect refinement, or hardware
    /// execution.
    pub fn create_compute_aql_queue_with_fixed_dispatch<const N: usize>(
        self,
        ring_bytes: u32,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; N],
        data: Vec<Gfx942FixedDispatchDataV1>,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        self.create_compute_aql_queue_with_fixed_dispatch_and_capacity_v1(
            ring_bytes,
            programs,
            packets,
            data,
            Gfx942FixedDispatchCapacityV1::default(),
        )
    }

    /// Creates a queue with an immutable, explicitly accounted epoch capacity.
    /// The qualification profile admits one-packet recipes only. Native ring,
    /// signal, storage and currentness checks remain mandatory per publication.
    pub fn create_compute_aql_queue_with_fixed_dispatch_and_capacity_v1<const N: usize>(
        self,
        ring_bytes: u32,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; N],
        data: Vec<Gfx942FixedDispatchDataV1>,
        capacity: Gfx942FixedDispatchCapacityV1,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        self.create_compute_aql_queue_with_preallocated_fixed_dispatch_v1(
            ring_bytes, programs, packets, data, capacity, None,
        )
    }

    /// Consumes optional fresh epoch storage into the construction custody root.
    /// A rejected consuming call retains its inputs; this is not a retry API.
    pub fn create_compute_aql_queue_with_preallocated_fixed_dispatch_v1<const N: usize>(
        self,
        ring_bytes: u32,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; N],
        data: Vec<Gfx942FixedDispatchDataV1>,
        capacity: Gfx942FixedDispatchCapacityV1,
        preallocation: Option<Gfx942FixedDispatchPreallocationV1>,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        let mut root = PrimaryQueueConstructionV1::new(
            self,
            (
                programs,
                FixedDispatchPreparationCustodyV1::new(packets, data),
            ),
        );
        root.dispatch_capacity = capacity;
        root.prepared_generation = preallocation;
        let mut root = root.run(|root, entry| {
            root.dispatch_capacity.validate_batch::<N>()?;
            validate_fixed_batch_ring::<N>(ring_bytes)?;
            PreparedDispatchGenerationV1::validate_target(&root.prepared_generation, None)?;
            PreparedDispatchGenerationV1::ensure_preallocated::<N>(
                &mut root.prepared_generation, &root.dispatch_capacity, DispatchGenerationSeedV1::Fresh,
            )?;
            let memory = root.memory.as_mut().expect("construction memory");
            let geometry = memory.plan_aql_queue_resources(ring_bytes)?;
            super::super::dispatch_binding::prepare_public_fixed_dispatch_resources_with_capacity_in_place(
                memory,
                &root.preparation.0,
                &mut root.preparation.1,
                &root.dispatch_capacity,
                &mut root.prepared_generation,
            )?;
            root.dispatch = Some(root.preparation.1.take_completed()?);
            root.construct(
                entry,
                geometry,
                ring_bytes,
                QueueRingBackingV1::AqlSpecial,
                None,
            )
        })?;
        Ok(root
            .completed
            .take()
            .expect("validated completed queue")
            .into_session())
    }
}

impl ComputeAqlQueueSessionV1 {
    /// Private end-to-end binding of real retained dispatch resources to C2
    /// publication and C4 per-packet completion. No public caller can construct
    /// the required resource owner inputs.
    /// Publishes the entire prepared fixed batch with one ring reservation and
    /// one final doorbell store, retaining one completion signal per packet.
    pub fn submit_fixed_dispatch<const N: usize>(
        &mut self,
    ) -> Result<Gfx942DispatchBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        self.submit_fixed_dispatch_classified_v1::<N>()
            .map_err(Gfx942FixedDispatchSubmissionFailureV1::into_error)
    }

    /// Publishes one epoch of the retained immutable ordinary recipe while
    /// preserving the pre-side-effect retry boundary for a bounded scheduler.
    pub fn submit_fixed_dispatch_classified_v1<const N: usize>(
        &mut self,
    ) -> Result<Gfx942DispatchBatchV1<N>, Gfx942FixedDispatchSubmissionFailureV1> {
        if self.terminal_poisoned {
            return Err(Gfx942FixedDispatchSubmissionFailureV1::Terminal(
                Gfx942DispatchBindingErrorV1::Poisoned.into(),
            ));
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(
                Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                    Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                ),
            );
        }
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.submit_fixed_dispatch_inner_classified::<N>(FixedDispatchBindingModeV1::Ordinary)
        }));
        match operation {
            Ok(result) => result.map_err(FixedDispatchSubmissionFailureV1::into_public),
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    /// Permanently quarantines this queue after an upper-layer owner or
    /// currentness invariant becomes indeterminate while native work is live.
    pub fn poison_after_runtime_owner_failure_v1(&mut self) {
        self.poison_terminal();
        permanently_poison_process_global_kfd_runtime_gate_v1();
    }

    pub(super) fn submit_fixed_dispatch_with_dependency_events_inner_v1<const N: usize>(
        &mut self,
        lane: ComputeAqlQueueLaneV1,
    ) -> Result<Gfx942ComputeDependencySourceBatchV1<N>, Gfx942FixedDispatchSubmissionFailureV1>
    {
        self.submit_dependency_source_using_v1(
            lane,
            &mut NativeDependencySourceRecipeV1,
            |session, packets| session.submit_prepared_batch_classified(packets),
        )
    }

    pub(super) fn submit_dependency_source_using_v1<const N: usize>(
        &mut self,
        lane: ComputeAqlQueueLaneV1,
        recipe: &mut impl DependencySourceRecipeV1<N>,
        submit: impl FnOnce(
            &mut Self,
            AqlPreparedKernelDispatchBatchV2<N>,
        ) -> Result<u64, NativeAqlSubmissionFailureV1>,
    ) -> Result<Gfx942ComputeDependencySourceBatchV1<N>, Gfx942FixedDispatchSubmissionFailureV1>
    {
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.submit_fixed_dispatch_with_dependency_events_operation_v1(lane, recipe, submit)
        }));
        match operation {
            Ok(result) => self
                .terminalize_fixed_dispatch_submission_result_v1(result)
                .map_err(FixedDispatchSubmissionFailureV1::into_public),
            Err(payload) => {
                self.poison_terminal();
                poison_process_global_after_dispatch_terminal_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    fn submit_fixed_dispatch_with_dependency_events_operation_v1<const N: usize>(
        &mut self,
        lane: ComputeAqlQueueLaneV1,
        recipe: &mut impl DependencySourceRecipeV1<N>,
        submit: impl FnOnce(
            &mut Self,
            AqlPreparedKernelDispatchBatchV2<N>,
        ) -> Result<u64, NativeAqlSubmissionFailureV1>,
    ) -> Result<Gfx942ComputeDependencySourceBatchV1<N>, FixedDispatchSubmissionFailureV1> {
        if self.terminal_poisoned {
            return Err(FixedDispatchSubmissionFailureV1::Terminal(
                Gfx942DispatchBindingErrorV1::Poisoned.into(),
            ));
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
            ));
        }
        if N == 0 || N > super::completion::GFX942_MAX_COMPUTE_DEPENDENCY_READERS_V1 {
            return Err(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "dependency source packet count must be 1 through 8192",
                ),
            ));
        }
        let mut output = dependency_source_output_reserve_body!(dependency_source_rust_expr, N)?;
        let acceptance = match self.dependency_owner.reserve_acceptance_epoch() {
            Ok(acceptance) => acceptance,
            Err(error @ ComputeDependencyTargetUseErrorV1::AcceptanceEpochExhausted) => {
                return Err(FixedDispatchSubmissionFailureV1::Terminal(
                    map_dependency_target_use_error_v1(error),
                ));
            }
            Err(error) => {
                return Err(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                    map_dependency_target_use_error_v1(error),
                ));
            }
        };
        let binding = recipe.bind(self);
        let (templates, identity) =
            self.classify_fixed_dispatch_binding(FixedDispatchBindingModeV1::Ordinary, binding)?;
        let completion = self.submit_with_dependency_events_classified_v1(
            templates,
            acceptance.session_occurrence(),
            acceptance.epoch(),
            submit,
        );
        let (completion, events) = match completion {
            Ok(published) => {
                recipe
                    .mark_published(self, identity, &published.0)
                    .map_err(|error| FixedDispatchSubmissionFailureV1::Terminal(error.into()))?;
                published
            }
            Err(failure) => {
                return Err(dependency_source_failure_body!(
                    dependency_source_rust_expr,
                    self,
                    recipe,
                    identity,
                    failure
                ));
            }
        };
        Ok(Gfx942ComputeDependencySourceBatchV1 {
            batch: wrap_published(completion, identity),
            events: dependency_source_output_pack_body!(
                dependency_source_rust_expr,
                output,
                events,
                lane
            ),
        })
    }

    pub(super) fn submit_with_dependency_events_classified_v1<const N: usize>(
        &mut self,
        templates: Box<[CompletionPacketTemplateV1; N]>,
        session_occurrence: u64,
        source_acceptance_epoch: u64,
        submit: impl FnOnce(
            &mut Self,
            AqlPreparedKernelDispatchBatchV2<N>,
        ) -> Result<u64, NativeAqlSubmissionFailureV1>,
    ) -> Result<
        (
            Gfx942CompletionBatchV1<N>,
            Vec<super::completion::Gfx942ComputeEventOccurrenceV1>,
        ),
        FixedDispatchSubmissionFailureV1,
    > {
        let bound = match self.completion_owner.bind_boxed_batch(templates) {
            Ok(bound) => bound,
            Err(Gfx942CompletionErrorV1::InsufficientSignals) => {
                return Err(FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(
                    Gfx942CompletionErrorV1::InsufficientSignals.into(),
                ));
            }
            Err(error) => {
                return Err(FixedDispatchSubmissionFailureV1::Terminal(error.into()));
            }
        };
        let events = self
            .completion_owner
            .record_dependency_event_batch_for_bound_v1(
                session_occurrence,
                source_acceptance_epoch,
                &bound,
            )
            .map_err(|error| FixedDispatchSubmissionFailureV1::Terminal(error.into()))?;
        let (packets, retention) = bound.into_parts();
        match submit(self, packets) {
            Ok(last_packet_id) => {
                completion_source_publish_body!(
                    dependency_source_rust_expr,
                    self.completion_owner,
                    retention,
                    last_packet_id,
                    events
                )
            }
            Err(NativeAqlSubmissionFailureV1::RetryableBeforeSideEffect(error)) => {
                if let Err(error) = completion_source_rollback_body!(
                    dependency_source_rust_expr,
                    self.completion_owner,
                    events,
                    retention
                ) {
                    return Err(FixedDispatchSubmissionFailureV1::Terminal(error.into()));
                }
                Err(FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(
                    map_submission(error),
                ))
            }
            Err(NativeAqlSubmissionFailureV1::Terminal(error)) => Err(
                FixedDispatchSubmissionFailureV1::Terminal(map_submission(error)),
            ),
        }
    }

    pub(super) fn submit_fixed_dispatch_inner_classified<const N: usize>(
        &mut self,
        mode: FixedDispatchBindingModeV1,
    ) -> Result<Gfx942DispatchBatchV1<N>, FixedDispatchSubmissionFailureV1> {
        self.submit_fixed_dispatch_inner_classified_using(mode, |session, packets| {
            session.submit_prepared_batch_classified(packets)
        })
    }

    fn submit_fixed_dispatch_inner_classified_using<const N: usize>(
        &mut self,
        mode: FixedDispatchBindingModeV1,
        native_submit: impl FnOnce(
            &mut Self,
            AqlPreparedKernelDispatchBatchV2<N>,
        ) -> Result<u64, NativeAqlSubmissionFailureV1>,
    ) -> Result<Gfx942DispatchBatchV1<N>, FixedDispatchSubmissionFailureV1> {
        if self.terminal_poisoned {
            return Err(FixedDispatchSubmissionFailureV1::Terminal(
                Gfx942DispatchBindingErrorV1::Poisoned.into(),
            ));
        }
        let binding = self
            .dispatch
            .as_mut()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)
            .and_then(|dispatch| dispatch.bind_templates::<N>(self.key));
        let (templates, identity) = match self.classify_fixed_dispatch_binding(mode, binding) {
            Ok(binding) => binding,
            Err(error) => {
                return self.terminalize_fixed_dispatch_submission_result_v1(Err(error));
            }
        };
        let completion = self.submit_with_completions_classified_using(templates, native_submit);
        let completion = match completion {
            Ok(completion) => {
                if let Err(error) = self
                    .dispatch
                    .as_mut()
                    .expect("dispatch owner retained")
                    .mark_published(identity, &completion)
                {
                    Err(FixedDispatchSubmissionFailureV1::Terminal(error.into()))
                } else {
                    Ok(completion)
                }
            }
            Err(error) => Err(error),
        };
        let result = finish_fixed_dispatch_submission(identity, completion, |identity| {
            self.dispatch
                .as_mut()
                .expect("dispatch owner retained")
                .cancel_binding(identity)
        });
        self.terminalize_fixed_dispatch_submission_result_v1(result)
    }
}

#[path = "fixed_dispatch/readback.rs"]
mod readback;

#[path = "fixed_dispatch/completion.rs"]
mod completion;

#[path = "fixed_dispatch/fixture.rs"]
mod fixture;

#[path = "fixed_dispatch/data.rs"]
mod data;

#[path = "fixed_dispatch/persistent_detach.rs"]
mod persistent_detach;

#[path = "fixed_dispatch/persistent_recycle.rs"]
mod persistent_recycle;

#[path = "fixed_dispatch/persistent_completion.rs"]
mod persistent_completion;

#[path = "fixed_dispatch/persistent_submission.rs"]
mod persistent_submission;

#[path = "fixed_dispatch/persistent_single_bind.rs"]
mod persistent_single_bind;

#[path = "fixed_dispatch/persistent_three_bind.rs"]
mod persistent_three_bind;

#[path = "fixed_dispatch/persistent_replay.rs"]
mod persistent_replay;

#[path = "fixed_dispatch/persistent_attachment.rs"]
mod persistent_attachment;
