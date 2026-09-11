//! Fixed-dispatch and persistent-compute orchestration for a live gfx942 queue.

use super::*;

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
        let root = PrimaryQueueConstructionV1::new(
            self,
            (
                programs,
                FixedDispatchPreparationCustodyV1::new(packets, data),
            ),
        );
        let mut root = root.run(|root, entry| {
            validate_fixed_batch_ring::<N>(ring_bytes)?;
            let memory = root.memory.as_mut().expect("construction memory");
            let geometry = memory.plan_aql_queue_resources(ring_bytes)?;
            super::super::dispatch_binding::prepare_public_fixed_dispatch_resources_in_place(
                memory,
                &root.preparation.0,
                &mut root.preparation.1,
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
        Ok(root.completed.take().expect("validated completed queue"))
    }
}

impl ComputeAqlQueueSessionV1 {
    fn validate_persistent_bind_preparation_v1(
        &mut self,
        preparation: &FixedDispatchPreparationCustodyV1<1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        // Bind ingress reserved both empty slots. Callbacks can mutate memory,
        // not the exclusively borrowed queue's attachment or dispatch slots.
        let authorities = preparation.completed()?.device_authorities_inline_v1();
        self.engine
            .as_mut()
            .expect("checked queue engine")
            .backend
            .session
            .validate_live_queue_dispatch_memory(&authorities)
            .map_err(Into::into)
    }

    pub(super) const fn has_any_persistent_compute_attachment_v1(&self) -> bool {
        self.persistent_compute.is_some()
    }

    pub(super) fn single_persistent_compute_attachment_v1(
        &self,
    ) -> Option<&BoundedPersistentComputeAttachmentV1> {
        self.persistent_compute
            .as_ref()
            .filter(|attachment| attachment.single_entry().is_some())
    }

    #[cfg(test)]
    pub(super) fn single_persistent_compute_attachment_mut_v1(
        &mut self,
    ) -> Option<&mut BoundedPersistentComputeAttachmentV1> {
        let attachment = self.persistent_compute.as_mut()?;
        attachment.single_entry_mut()?;
        Some(attachment)
    }

    pub(super) fn take_single_persistent_compute_attachment_v1(
        &mut self,
    ) -> Option<PersistentComputeAttachmentV1> {
        match self.persistent_compute.take()?.into_single() {
            Ok(attachment) => Some(attachment),
            Err(attachment) => {
                self.persistent_compute = Some(attachment);
                None
            }
        }
    }

    pub(super) fn set_single_persistent_compute_attachment_v1(
        &mut self,
        attachment: PersistentComputeAttachmentV1,
    ) {
        debug_assert!(self.persistent_compute.is_none());
        self.persistent_compute = Some(BoundedPersistentComputeAttachmentV1::from_single(
            attachment,
        ));
    }

    pub(super) fn three_binding_persistent_compute_attachment_v1(
        &self,
    ) -> Option<&BoundedPersistentComputeAttachmentV1> {
        self.persistent_compute
            .as_ref()
            .filter(|attachment| attachment.is_three())
    }

    pub(super) fn take_three_binding_persistent_compute_attachment_v1(
        &mut self,
    ) -> Option<ThreeBindingPersistentComputeAttachmentV1> {
        match self.persistent_compute.take()?.into_three() {
            Ok(attachment) => Some(attachment),
            Err(attachment) => {
                self.persistent_compute = Some(attachment);
                None
            }
        }
    }

    pub(super) fn set_three_binding_persistent_compute_attachment_v1(
        &mut self,
        attachment: ThreeBindingPersistentComputeAttachmentV1,
    ) {
        debug_assert!(self.persistent_compute.is_none());
        self.persistent_compute =
            Some(BoundedPersistentComputeAttachmentV1::from_three(attachment));
    }

    /// Reports only the terminal persistent-compute custody stage; native
    /// identities and authorities remain retained inside the queue.
    pub fn persistent_compute_terminal_stage_v1(
        &self,
    ) -> Option<crate::persistent_compute::Gfx942PersistentComputeTerminalStageV1> {
        self.persistent_compute
            .as_ref()?
            .terminal_custody()
            .map(PersistentComputeTerminalNativeCustodyV1::stage)
    }

    pub(super) fn absorb_terminal_prepared_persistent_compute_v1(
        &mut self,
        binding: PersistentComputeBindingKeyV1,
    ) -> bool {
        let Some(mut attachment) = self.take_single_persistent_compute_attachment_v1() else {
            return false;
        };
        if attachment.binding != binding {
            self.set_single_persistent_compute_attachment_v1(attachment);
            return false;
        }
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Prepared(prepared) = state else {
            attachment.state = state;
            self.set_single_persistent_compute_attachment_v1(attachment);
            return false;
        };
        attachment.state = quarantine_persistent_compute_prepared_v1(
            &mut attachment.allocation.owner,
            prepared,
            Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
        );
        attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Attached);
        self.set_single_persistent_compute_attachment_v1(attachment);
        true
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn absorb_terminal_published_persistent_compute_v1(
        &mut self,
        binding: PersistentComputeBindingKeyV1,
        batch: Gfx942DispatchBatchV1<1>,
    ) -> Result<(), Gfx942DispatchBatchV1<1>> {
        let Some(mut attachment) = self.take_single_persistent_compute_attachment_v1() else {
            return Err(batch);
        };
        if attachment.binding != binding {
            self.set_single_persistent_compute_attachment_v1(attachment);
            return Err(batch);
        }
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Published(published) = state else {
            attachment.state = state;
            self.set_single_persistent_compute_attachment_v1(attachment);
            return Err(batch);
        };
        attachment.state = quarantine_persistent_compute_published_v1(
            &mut attachment.allocation.owner,
            published,
            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
        );
        attachment.terminal_custody =
            Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
        self.set_single_persistent_compute_attachment_v1(attachment);
        Ok(())
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn absorb_terminal_completed_persistent_compute_v1(
        &mut self,
        binding: PersistentComputeBindingKeyV1,
        completed: Gfx942CompletedDispatchBatchV1<1>,
    ) -> Result<(), Gfx942CompletedDispatchBatchV1<1>> {
        let Some(mut attachment) = self.take_single_persistent_compute_attachment_v1() else {
            return Err(completed);
        };
        if attachment.binding != binding {
            self.set_single_persistent_compute_attachment_v1(attachment);
            return Err(completed);
        }
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Completed(completed_use) = state else {
            attachment.state = state;
            self.set_single_persistent_compute_attachment_v1(attachment);
            return Err(completed);
        };
        attachment.state = quarantine_persistent_compute_completed_v1(
            &mut attachment.allocation.owner,
            completed_use,
            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
        );
        attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Completed(
            completed,
        ));
        self.set_single_persistent_compute_attachment_v1(attachment);
        Ok(())
    }

    pub(super) fn absorb_terminal_recycled_persistent_compute_v1(
        &mut self,
        binding: PersistentComputeBindingKeyV1,
        recycle: Gfx942CompletionRecycleObservationV1,
    ) -> Result<(), Gfx942CompletionRecycleObservationV1> {
        let Some(mut attachment) = self.take_single_persistent_compute_attachment_v1() else {
            return Err(recycle);
        };
        if attachment.binding != binding {
            self.set_single_persistent_compute_attachment_v1(attachment);
            return Err(recycle);
        }
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Recycled(completed_use) = state else {
            attachment.state = state;
            self.set_single_persistent_compute_attachment_v1(attachment);
            return Err(recycle);
        };
        attachment.state = quarantine_persistent_compute_recycled_v1(
            &mut attachment.allocation.owner,
            completed_use,
            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
        );
        attachment.terminal_custody =
            Some(PersistentComputeTerminalNativeCustodyV1::Recycled(recycle));
        self.set_single_persistent_compute_attachment_v1(attachment);
        Ok(())
    }

    /// Authenticates one exact full-allocation H2D window and retires its
    /// settled frontier into an initialized persistent-compute receipt.
    #[allow(clippy::result_large_err)]
    pub fn promote_full_h2d_to_persistent_compute_ready_v1(
        &mut self,
        completed: Gfx942DirectionalPersistentSdmaWindowCompletedV1,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<
        (Gfx942PersistentComputeReadyV1, Gfx942SdmaBufferV1),
        Gfx942PersistentComputeReadyFailureV1,
    > {
        let completed = preserve_persistent_compute_ready_affiliation_v1(
            completed,
            self.key,
            self.terminal_poisoned,
        )?;
        let preflight = self.require_sdma_enabled();
        let completed = preserve_persistent_compute_ready_preflight_custody_v1(
            completed,
            self.terminal_poisoned,
            preflight,
        )?;
        let direction = completed.direction();
        let host_offset = completed.host_offset();
        let device_offset = completed.device_offset();
        let copy_bytes = u64::from(completed.copy_bytes());
        let packet_count = completed.packet_count();
        let (allocation, host, frontier) = completed.into_parts();
        let valid = direction == Gfx942PersistentSdmaDirectionV1::HostToDevice
            && host_offset == 0
            && device_offset == 0
            && allocation.byte_len() == allocation.physical_byte_len()
            && copy_bytes == allocation.physical_byte_len()
            && content.byte_len() == copy_bytes
            && host.kind() == Gfx942SdmaBufferKindV1::HostVisibleCoherent
            && host.requested_bytes() == copy_bytes
            && host.physical_bytes() == copy_bytes
            && allocation.attachment.queue == self.compute_lane_session
            && self.directional_persistent_sdma_attachment_is_current(&allocation.attachment);
        if !valid {
            return Err(Gfx942PersistentComputeReadyFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute initialization requires one exact full H2D window",
                ),
                custody: Gfx942PersistentComputeReadyFailureCustodyV1::Retryable((
                    allocation, host, frontier,
                )),
            });
        }
        let observed = self.with_live_queue_memory_model(|memory| {
            memory
                .check_queue_operational_currentness()
                .map_err(ComputeAqlQueueSessionErrorV1::from)?;
            let observed = host.certified_full_host_content_sha256(copy_bytes);
            memory
                .check_queue_operational_currentness()
                .map_err(ComputeAqlQueueSessionErrorV1::from)?;
            Ok(observed)
        });
        let observed = match observed {
            Ok(observed) => observed,
            Err(error) => {
                self.poison_terminal();
                let completed =
                    Gfx942DirectionalPersistentSdmaWindowCompletedV1::from_parts_for_terminal(
                        allocation,
                        host,
                        frontier,
                        direction,
                        host_offset,
                        device_offset,
                        u32::try_from(copy_bytes).expect("copy bytes came from u32"),
                        packet_count,
                    );
                return Err(terminal_persistent_compute_ready_hash_failure_v1(
                    error, completed,
                ));
            }
        };
        if observed.is_none_or(|observed| {
            !content_descriptor_matches_sha256(content, copy_bytes, observed)
        }) {
            return Err(Gfx942PersistentComputeReadyFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute H2D content descriptor mismatch",
                ),
                custody: Gfx942PersistentComputeReadyFailureCustodyV1::Retryable((
                    allocation, host, frontier,
                )),
            });
        }
        let allocation = match allocation.retire_settled_frontier_v1(frontier) {
            Ok(allocation) => allocation,
            Err(failure) => {
                let (allocation, frontier) = failure.into_parts();
                return Err(Gfx942PersistentComputeReadyFailureV1 {
                    error: ComputeAqlQueueSessionErrorV1::Contract(
                        "persistent compute H2D frontier retirement",
                    ),
                    custody: Gfx942PersistentComputeReadyFailureCustodyV1::Retryable((
                        allocation, host, frontier,
                    )),
                });
            }
        };
        Ok((
            Gfx942PersistentComputeReadyV1 {
                allocation,
                authenticated_sha256: content.sha256(),
            },
            host,
        ))
    }

    /// Authenticates one exact full-allocation, single-packet H2D copy and
    /// retires its settled frontier into an initialized persistent-compute
    /// receipt. The consumed completion is normalized to the same sealed
    /// one-packet window representation used by the common ready transition.
    #[allow(clippy::result_large_err)]
    pub fn promote_full_single_h2d_to_persistent_compute_ready_v1(
        &mut self,
        completed: Gfx942DirectionalPersistentSdmaCompletedV1,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<
        (Gfx942PersistentComputeReadyV1, Gfx942SdmaBufferV1),
        Gfx942PersistentComputeReadyFailureV1,
    > {
        self.promote_full_h2d_to_persistent_compute_ready_v1(
            completed.into_single_packet_window_v1(),
            content,
        )
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn terminal_persistent_retained_control_replay_after_detach_v1(
        &mut self,
        mut replay: PersistentRetainedControlReplayDetachedV1,
        custody: PersistentComputeTerminalNativeCustodyV1,
        error: ComputeAqlQueueSessionErrorV1,
        commit: PersistentRetainedControlReplayCommitV1,
    ) -> Gfx942PersistentComputeBindFailureV1 {
        let disposition = match &custody {
            PersistentComputeTerminalNativeCustodyV1::Storage(_) => {
                classify_persistent_retained_control_replay_failure_v1(
                    PersistentRetainedControlReplayCustodyStageV1::Storage,
                    false,
                    false,
                    false,
                )
            }
            PersistentComputeTerminalNativeCustodyV1::Data(_) => {
                classify_persistent_retained_control_replay_failure_v1(
                    PersistentRetainedControlReplayCustodyStageV1::Data,
                    false,
                    false,
                    false,
                )
            }
            PersistentComputeTerminalNativeCustodyV1::Attached => {
                classify_persistent_retained_control_replay_failure_v1(
                    PersistentRetainedControlReplayCustodyStageV1::Attached,
                    false,
                    false,
                    false,
                )
            }
            _ => unreachable!("replay bind admits only pre-publication custody"),
        };
        debug_assert!(matches!(
            (&custody, disposition),
            (
                PersistentComputeTerminalNativeCustodyV1::Storage(_),
                PersistentRetainedControlReplayDispositionV1::TerminalStorage,
            ) | (
                PersistentComputeTerminalNativeCustodyV1::Data(_),
                PersistentRetainedControlReplayDispositionV1::TerminalData,
            ) | (
                PersistentComputeTerminalNativeCustodyV1::Attached,
                PersistentRetainedControlReplayDispositionV1::TerminalAttached,
            )
        ));
        let state = quarantine_persistent_retained_control_replay_prepared_v1(
            &mut replay.allocation.owner,
            replay.prepared,
        );
        self.dispatch = Some(replay.dispatch);
        self.set_single_persistent_compute_attachment_v1(PersistentComputeAttachmentV1 {
            allocation: replay.allocation,
            authenticated_sha256: replay.authenticated_sha256,
            fully_initialized: replay.fully_initialized,
            state,
            binding: PersistentComputeBindingKeyV1 {
                queue: self.key,
                attachment_generation: commit.attachment_generation,
            },
            storage_identity: commit.storage_identity,
            effect: commit.effect,
            predecessor_dispatch_generation: Some(commit.predecessor_generation),
            terminal_custody: Some(custody),
        });
        self.next_persistent_compute_generation = commit.next_attachment_generation;
        self.poison_terminal();
        Gfx942PersistentComputeBindFailureV1 {
            error,
            custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
            ),
        }
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn finish_persistent_retained_control_replay_before_detach_v1(
        &mut self,
        request: PersistentRetainedControlReplayRequestV1,
        error: ComputeAqlQueueSessionErrorV1,
        loan_succeeded: bool,
        commit: PersistentRetainedControlReplayCommitV1,
    ) -> Gfx942PersistentComputeBindFailureV1 {
        let PersistentRetainedControlReplayRequestV1 {
            mut input,
            prepared,
            dispatch,
            initialized_content: _,
            control_identity: _,
            predecessor_generation: _,
        } = request;
        self.dispatch = Some(dispatch);
        let cancellation = persistent_compute_input_allocation_mut_v1(&mut input)
            .owner
            .cancel_prepared(prepared);
        let cancellation_succeeded = cancellation.is_ok();
        let disposition = classify_persistent_retained_control_replay_failure_v1(
            PersistentRetainedControlReplayCustodyStageV1::Input,
            loan_succeeded,
            cancellation_succeeded,
            !self.terminal_poisoned,
        );
        match (disposition, cancellation) {
            (PersistentRetainedControlReplayDispositionV1::RetryableInput, Ok(())) => {
                persistent_retained_control_replay_input_failure_v1(error, input, true)
            }
            (PersistentRetainedControlReplayDispositionV1::TerminalInput, Ok(())) => {
                self.poison_terminal();
                persistent_retained_control_replay_input_failure_v1(error, input, false)
            }
            (PersistentRetainedControlReplayDispositionV1::TerminalAttached, Err(failure)) => {
                let (_, prepared) = failure.into_parts();
                let (mut allocation, authenticated_sha256, fully_initialized) = input.into_parts();
                let state = quarantine_persistent_retained_control_replay_prepared_v1(
                    &mut allocation.owner,
                    prepared,
                );
                self.set_single_persistent_compute_attachment_v1(PersistentComputeAttachmentV1 {
                    allocation,
                    authenticated_sha256,
                    fully_initialized,
                    state,
                    binding: PersistentComputeBindingKeyV1 {
                        queue: self.key,
                        attachment_generation: commit.attachment_generation,
                    },
                    storage_identity: commit.storage_identity,
                    effect: commit.effect,
                    predecessor_dispatch_generation: Some(commit.predecessor_generation),
                    terminal_custody: Some(PersistentComputeTerminalNativeCustodyV1::Attached),
                });
                self.next_persistent_compute_generation = commit.next_attachment_generation;
                self.poison_terminal();
                Gfx942PersistentComputeBindFailureV1 {
                    error,
                    custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                        Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
                    ),
                }
            }
            _ => unreachable!("replay failure disposition matches exact cancellation custody"),
        }
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn bind_retained_persistent_fixed_dispatch_control_replay_v1(
        &mut self,
        request: PersistentRetainedControlReplayRequestV1,
        commit: PersistentRetainedControlReplayCommitV1,
    ) -> Result<Gfx942PreparedPersistentComputeDispatchV1, Gfx942PersistentComputeBindFailureV1>
    {
        let mut request = Some(request);
        let mut phases = PersistentRetainedControlReplayCustodyV1::Empty;
        let mut pipeline_result = None;
        let fused_loan = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.with_live_queue_memory_model(|memory| {
                phases = PersistentRetainedControlReplayCustodyV1::Input(
                    request.take().expect("one replay input before opening"),
                );
                pipeline_result = Some(execute_persistent_retained_control_replay_pipeline_v1(
                    memory,
                    &mut phases,
                    |memory, phases| phases.mapped_facts(memory),
                    |_, phases| phases.detach(),
                    |_, phases| phases.construct(),
                    |memory, phases| phases.retain(memory),
                    |memory, phases| phases.audit(memory),
                ));
                Ok(())
            })
        }));
        let fused_loan = match fused_loan {
            Ok(result) => result,
            Err(payload) => {
                if let Some(request) = request.take() {
                    phases = PersistentRetainedControlReplayCustodyV1::Input(request);
                }
                let error = ComputeAqlQueueSessionErrorV1::Contract("persistent replay panicked");
                match phases.into_bind_outcome(Err(error)) {
                    PersistentRetainedControlReplayOutcomeV1::BeforeDetach { request, .. } => {
                        let (mut allocation, authenticated_sha256, fully_initialized) =
                            request.input.into_parts();
                        let state = quarantine_persistent_retained_control_replay_prepared_v1(
                            &mut allocation.owner,
                            request.prepared,
                        );
                        self.dispatch = Some(request.dispatch);
                        self.set_single_persistent_compute_attachment_v1(
                            PersistentComputeAttachmentV1 {
                                allocation,
                                authenticated_sha256,
                                fully_initialized,
                                state,
                                binding: PersistentComputeBindingKeyV1 {
                                    queue: self.key,
                                    attachment_generation: commit.attachment_generation,
                                },
                                storage_identity: commit.storage_identity,
                                effect: commit.effect,
                                predecessor_dispatch_generation: Some(
                                    commit.predecessor_generation,
                                ),
                                terminal_custody: Some(
                                    PersistentComputeTerminalNativeCustodyV1::Attached,
                                ),
                            },
                        );
                        self.next_persistent_compute_generation = commit.next_attachment_generation;
                    }
                    PersistentRetainedControlReplayOutcomeV1::AfterDetach {
                        replay,
                        custody,
                        error,
                    } => {
                        let _ = self.terminal_persistent_retained_control_replay_after_detach_v1(
                            replay, custody, error, commit,
                        );
                    }
                    PersistentRetainedControlReplayOutcomeV1::Ready(_) => {
                        unreachable!("panic is not completion")
                    }
                }
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        let outcome = pipeline_result.map(|result| phases.into_bind_outcome(result));

        let (outcome, loan_error) = match resolve_persistent_retained_control_replay_loan_v1(
            request,
            outcome,
            fused_loan,
            || {
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent replay foundation loan did not execute",
                )
            },
        ) {
            PersistentRetainedControlReplayLoanResolutionV1::Unopened { request, error } => {
                return Err(
                    self.finish_persistent_retained_control_replay_before_detach_v1(
                        request, error, false, commit,
                    ),
                );
            }
            PersistentRetainedControlReplayLoanResolutionV1::Executed {
                outcome,
                retake_error,
            } => (outcome, retake_error),
        };
        let loan_succeeded = loan_error.is_none();
        match outcome {
            PersistentRetainedControlReplayOutcomeV1::BeforeDetach { request, error } => Err(self
                .finish_persistent_retained_control_replay_before_detach_v1(
                    request,
                    loan_error.unwrap_or(error),
                    loan_succeeded,
                    commit,
                )),
            PersistentRetainedControlReplayOutcomeV1::AfterDetach {
                replay,
                custody,
                error,
            } => Err(
                self.terminal_persistent_retained_control_replay_after_detach_v1(
                    replay,
                    custody,
                    loan_error.unwrap_or(error),
                    commit,
                ),
            ),
            PersistentRetainedControlReplayOutcomeV1::Ready(replay) => {
                if let Some(error) = loan_error {
                    return Err(
                        self.terminal_persistent_retained_control_replay_after_detach_v1(
                            replay,
                            PersistentComputeTerminalNativeCustodyV1::Attached,
                            error,
                            commit,
                        ),
                    );
                }
                let binding = PersistentComputeBindingKeyV1 {
                    queue: self.key,
                    attachment_generation: commit.attachment_generation,
                };
                self.dispatch = Some(replay.dispatch);
                self.detached_data_count = 0;
                self.detached_dispatch_generation = None;
                self.detached_data_identities.clear();
                self.detached_next_insertion_index = None;
                self.set_single_persistent_compute_attachment_v1(PersistentComputeAttachmentV1 {
                    allocation: replay.allocation,
                    authenticated_sha256: replay.authenticated_sha256,
                    fully_initialized: replay.fully_initialized,
                    state: PersistentComputeUseStateV1::Prepared(replay.prepared),
                    binding,
                    storage_identity: commit.storage_identity,
                    effect: commit.effect,
                    predecessor_dispatch_generation: Some(commit.predecessor_generation),
                    terminal_custody: None,
                });
                self.next_persistent_compute_generation = commit.next_attachment_generation;
                Ok(Gfx942PreparedPersistentComputeDispatchV1 {
                    binding,
                    thread_affinity: PhantomData,
                })
            }
        }
    }

    /// Binds exactly two initialized full-extent read inputs and one distinct
    /// full-extent write output to one primary-lane packet.
    #[allow(clippy::result_large_err)]
    pub fn bind_three_binding_directional_persistent_fixed_dispatch_v1(
        &mut self,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; 1],
        inputs: Gfx942ThreeBindingPersistentComputeInputsV1,
        content_roles: [Gfx942DeviceContentRoleV1; 3],
    ) -> Result<
        Gfx942PreparedThreeBindingPersistentComputeDispatchV1,
        Gfx942ThreeBindingPersistentComputeBindFailureV1,
    > {
        let recover = |error, inputs| Gfx942ThreeBindingPersistentComputeBindFailureV1 {
            error,
            custody: Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1::Retryable(inputs),
        };
        let test_validation_only = take_three_binding_bind_validation_only_v1();
        if self.terminal_poisoned {
            return Err(Gfx942ThreeBindingPersistentComputeBindFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                custody: Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1::ProcessTeardown(
                    Gfx942ThreeBindingPersistentComputeBindTerminalCustodyV1 {
                        inputs: Some(inputs),
                    },
                ),
            });
        }
        if inputs
            .inputs
            .iter()
            .any(|input| !input.belongs_to(self.compute_lane_session))
        {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent compute input owner substitution",
                ),
                inputs,
            ));
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(recover(
                Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                inputs,
            ));
        }
        if self.key != self.compute_lane_session {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent compute requires the primary compute lane",
                ),
                inputs,
            ));
        }
        if self.dispatch.is_some() {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent compute requires released prior control",
                ),
                inputs,
            ));
        }
        let detached_generation = self.detached_dispatch_generation;
        let detached_is_empty = self.unpublished_dispatch.is_clear()
            && self.detached_data_count == 0
            && self.detached_data_identities.is_empty()
            && match detached_generation {
                None => self.detached_next_insertion_index.is_none(),
                Some(_) => self.detached_next_insertion_index == Some(0),
            };
        if !detached_is_empty {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent compute requires an empty dispatch-data roster",
                ),
                inputs,
            ));
        }
        if !test_validation_only && let Err(error) = self.require_sdma_enabled() {
            return Err(recover(error, inputs));
        }
        let competing_queues_admitted = test_validation_only
            || self.persistent_inputs_coexist_with_directional_sdma_v1(&[
                &inputs.inputs[0],
                &inputs.inputs[1],
                &inputs.inputs[2],
            ]);
        if !competing_queues_admitted {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent compute requires disjoint directional custody",
                ),
                inputs,
            ));
        }
        if !inputs.inputs[0].is_fully_initialized()
            || !inputs.inputs[1].is_fully_initialized()
            || !inputs.inputs[2].is_fully_initialized()
            || inputs.inputs[0].byte_len() != inputs.inputs[1].byte_len()
            || inputs.inputs[0].byte_len() != inputs.inputs[2].byte_len()
        {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent compute requires initialized equal-extent A, B, and C storage",
                ),
                inputs,
            ));
        }

        // Reserve the existing binder's exact data roster before any use lease
        // is created or native allocation authority is detached.
        let mut data = Vec::new();
        if data.try_reserve_exact(3).is_err() {
            return Err(recover(
                Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
                    operation: "three-binding persistent data roster",
                }
                .into(),
                inputs,
            ));
        }

        let mut entries = inputs.inputs.map(|input| {
            let (allocation, authenticated_sha256, fully_initialized) = input.into_parts();
            PersistentComputeAttachmentEntryV1 {
                allocation,
                authenticated_sha256,
                fully_initialized,
                state: PersistentComputeUseStateV1::Quarantined,
                storage_identity: None,
                effect: Gfx942PersistentComputeEffectV1::Read,
            }
        });
        for entry in &entries {
            if entry.allocation.attachment.queue != self.compute_lane_session
                || (!test_validation_only
                    && !self.directional_persistent_sdma_attachment_is_current(
                        &entry.allocation.attachment,
                    ))
                || entry.allocation.byte_len() == 0
                || entry.allocation.byte_len() != entry.allocation.physical_byte_len()
                || entry.allocation.owner.live_use_count() != 0
                || entry.allocation.owner.retained_settled_use_count() != 0
            {
                return Err(recover(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "three-binding persistent compute requires exact quiescent full extents",
                    ),
                    three_binding_entries_into_inputs_v1(entries),
                ));
            }
            if entry.allocation.owner.local_native_for_sdma().is_none() {
                return Err(recover(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "three-binding persistent compute requires attached local native custody",
                    ),
                    three_binding_entries_into_inputs_v1(entries),
                ));
            };
        }
        let layouts = std::array::from_fn(|index| {
            let lease = entries[index]
                .allocation
                .owner
                .local_native_for_sdma()
                .expect("validated three-binding local native custody");
            super::dispatch_binding::Gfx942FixedDispatchDataLayoutV1::device_local(
                lease.layout().requested_bytes(),
                lease.layout().alignment(),
            )
        });
        let identities = std::array::from_fn(|index| {
            entries[index]
                .allocation
                .owner
                .local_native_for_sdma()
                .expect("validated three-binding local native custody")
                .storage_identity()
        });
        for (entry, identity) in entries.iter_mut().zip(identities) {
            entry.storage_identity = Some(identity);
        }
        let initialized = [
            entries[0].fully_initialized,
            entries[1].fully_initialized,
            entries[2].fully_initialized,
        ];
        let storage = identities.map(Gfx942SdmaBufferStorageIdentityV1::Device);
        let control_identity = match three_binding_persistent_fixed_dispatch_control_identity_v1(
            self.key,
            &programs,
            &packets,
            layouts,
            initialized,
            content_roles,
            storage,
        ) {
            Ok(identity) => identity,
            Err(error) => {
                return Err(recover(
                    error.into(),
                    three_binding_entries_into_inputs_v1(entries),
                ));
            }
        };
        for (entry, effect) in entries.iter_mut().zip(control_identity.effects()) {
            entry.effect = match effect {
                DeviceDataEffectV1::ReadOnly => Gfx942PersistentComputeEffectV1::Read,
                DeviceDataEffectV1::WriteOnly => Gfx942PersistentComputeEffectV1::Write,
                DeviceDataEffectV1::ReadWrite => Gfx942PersistentComputeEffectV1::ReadWrite,
            };
        }
        if test_validation_only {
            #[cfg(test)]
            record_three_binding_bind_validation_effects_v1(control_identity.effects());
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "test-only three-binding validation boundary",
                ),
                three_binding_entries_into_inputs_v1(entries),
            ));
        }
        let attachment_generation = self.next_persistent_compute_generation;
        let Some(next_attachment_generation) = attachment_generation.checked_add(1) else {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent compute attachment generation exhausted",
                ),
                three_binding_entries_into_inputs_v1(entries),
            ));
        };
        for index in 0..3 {
            let operation = match entries[index].effect {
                Gfx942PersistentComputeEffectV1::Read => Gfx942PersistentOperationV1::ComputeRead,
                Gfx942PersistentComputeEffectV1::Write => Gfx942PersistentOperationV1::ComputeWrite,
                Gfx942PersistentComputeEffectV1::ReadWrite => {
                    Gfx942PersistentOperationV1::ComputeReadWrite
                }
            };
            let request = Gfx942PersistentUseRequestV1::new(
                operation,
                0,
                entries[index].allocation.byte_len(),
            )
            .expect("admitted persistent extent is nonzero");
            let reserved = match entries[index].allocation.owner.reserve(request, None) {
                Ok(reserved) => reserved,
                Err(failure) => {
                    let error = map_directional_persistent_sdma_use_error_v1(failure.error());
                    if cancel_persistent_compute_prepublication_entries_v1(entries.each_mut()) {
                        return Err(recover(
                            error,
                            three_binding_entries_into_inputs_v1(entries),
                        ));
                    }
                    self.set_three_binding_persistent_compute_attachment_v1(
                        ThreeBindingPersistentComputeAttachmentV1 {
                            entries,
                            binding: PersistentComputeBindingKeyV1 {
                                queue: self.key,
                                attachment_generation,
                            },
                            predecessor_dispatch_generation: detached_generation,
                            terminal_custody: Some(
                                PersistentComputeTerminalNativeCustodyV1::Attached,
                            ),
                        },
                    );
                    self.next_persistent_compute_generation = next_attachment_generation;
                    self.poison_terminal();
                    return Err(Gfx942ThreeBindingPersistentComputeBindFailureV1 {
                        error,
                        custody:
                            Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1::ProcessTeardown(
                                Gfx942ThreeBindingPersistentComputeBindTerminalCustodyV1 {
                                    inputs: None,
                                },
                            ),
                    });
                }
            };
            entries[index].state = PersistentComputeUseStateV1::Reserved(reserved);
            let state = core::mem::replace(
                &mut entries[index].state,
                PersistentComputeUseStateV1::Quarantined,
            );
            let PersistentComputeUseStateV1::Reserved(reserved) = state else {
                unreachable!("just-reserved three-binding use")
            };
            match entries[index].allocation.owner.prepare(reserved) {
                Ok(prepared) => {
                    entries[index].state = PersistentComputeUseStateV1::Prepared(prepared);
                }
                Err(failure) => {
                    let (failure_error, reserved) = failure.into_parts();
                    entries[index].state = PersistentComputeUseStateV1::Reserved(reserved);
                    let error = map_directional_persistent_sdma_use_error_v1(failure_error);
                    if cancel_persistent_compute_prepublication_entries_v1(entries.each_mut()) {
                        return Err(recover(
                            error,
                            three_binding_entries_into_inputs_v1(entries),
                        ));
                    }
                    self.set_three_binding_persistent_compute_attachment_v1(
                        ThreeBindingPersistentComputeAttachmentV1 {
                            entries,
                            binding: PersistentComputeBindingKeyV1 {
                                queue: self.key,
                                attachment_generation,
                            },
                            predecessor_dispatch_generation: detached_generation,
                            terminal_custody: Some(
                                PersistentComputeTerminalNativeCustodyV1::Attached,
                            ),
                        },
                    );
                    self.next_persistent_compute_generation = next_attachment_generation;
                    self.poison_terminal();
                    return Err(Gfx942ThreeBindingPersistentComputeBindFailureV1 {
                        error,
                        custody:
                            Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1::ProcessTeardown(
                                Gfx942ThreeBindingPersistentComputeBindTerminalCustodyV1 {
                                    inputs: None,
                                },
                            ),
                    });
                }
            }
        }
        let validation = validate_persistent_bind_inputs_v1(
            self,
            &entries.each_ref().map(|entry| &entry.allocation),
            |session, allocation| {
                let lease = allocation
                    .owner
                    .local_native_for_sdma()
                    .expect("validated persistent local native custody");
                session.with_live_queue_memory_model(|memory| {
                    memory
                        .mapped_gfx942_device_memory_facts(lease)
                        .map(|_| ())
                        .map_err(Into::into)
                })
            },
        );
        let validation = match validation {
            Ok(result) => result,
            Err(payload) => {
                quarantine_persistent_compute_entries_v1(
                    entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                );
                self.set_three_binding_persistent_compute_attachment_v1(
                    ThreeBindingPersistentComputeAttachmentV1 {
                        entries,
                        binding: PersistentComputeBindingKeyV1 {
                            queue: self.key,
                            attachment_generation,
                        },
                        predecessor_dispatch_generation: detached_generation,
                        terminal_custody: Some(PersistentComputeTerminalNativeCustodyV1::Attached),
                    },
                );
                self.next_persistent_compute_generation = next_attachment_generation;
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        if let Err(error) = validation {
            if cancel_persistent_compute_prepublication_entries_v1(entries.each_mut()) {
                let inputs = three_binding_entries_into_inputs_v1(entries);
                return Err(Gfx942ThreeBindingPersistentComputeBindFailureV1 {
                    error,
                    custody: if persistent_bind_retryable_v1(!self.terminal_poisoned, true) {
                        Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1::Retryable(inputs)
                    } else {
                        Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1::ProcessTeardown(
                            Gfx942ThreeBindingPersistentComputeBindTerminalCustodyV1 {
                                inputs: Some(inputs),
                            },
                        )
                    },
                });
            }
            self.set_three_binding_persistent_compute_attachment_v1(
                ThreeBindingPersistentComputeAttachmentV1 {
                    entries,
                    binding: PersistentComputeBindingKeyV1 {
                        queue: self.key,
                        attachment_generation,
                    },
                    predecessor_dispatch_generation: detached_generation,
                    terminal_custody: Some(PersistentComputeTerminalNativeCustodyV1::Attached),
                },
            );
            self.next_persistent_compute_generation = next_attachment_generation;
            self.poison_terminal();
            return Err(Gfx942ThreeBindingPersistentComputeBindFailureV1 {
                error,
                custody: Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1::ProcessTeardown(
                    Gfx942ThreeBindingPersistentComputeBindTerminalCustodyV1 { inputs: None },
                ),
            });
        }
        for index in 0..3 {
            let state = core::mem::replace(
                &mut entries[index].state,
                PersistentComputeUseStateV1::Quarantined,
            );
            let PersistentComputeUseStateV1::Prepared(prepared) = state else {
                unreachable!("validated three-binding prepared state")
            };
            let lease = match entries[index]
                .allocation
                .owner
                .detach_local_native_for_compute(&prepared)
            {
                Ok(lease) => lease,
                Err(error) => {
                    entries[index].state = PersistentComputeUseStateV1::Prepared(prepared);
                    quarantine_persistent_compute_entries_v1(
                        entries.each_mut(),
                        Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                    );
                    self.set_three_binding_persistent_compute_attachment_v1(
                        ThreeBindingPersistentComputeAttachmentV1 {
                            entries,
                            binding: PersistentComputeBindingKeyV1 {
                                queue: self.key,
                                attachment_generation,
                            },
                            predecessor_dispatch_generation: detached_generation,
                            terminal_custody: Some(if data.is_empty() {
                                PersistentComputeTerminalNativeCustodyV1::Attached
                            } else {
                                PersistentComputeTerminalNativeCustodyV1::Data(
                                    PersistentComputeTerminalDataV1::from_vec(data),
                                )
                            }),
                        },
                    );
                    self.next_persistent_compute_generation = next_attachment_generation;
                    self.poison_terminal();
                    return Err(Gfx942ThreeBindingPersistentComputeBindFailureV1 {
                        error: map_directional_persistent_sdma_use_error_v1(error),
                        custody:
                            Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1::ProcessTeardown(
                                Gfx942ThreeBindingPersistentComputeBindTerminalCustodyV1 {
                                    inputs: None,
                                },
                            ),
                    });
                }
            };
            entries[index].state = PersistentComputeUseStateV1::Prepared(prepared);
            let item = match entries[index].authenticated_sha256 {
                Some(sha256) => {
                    let content = match Gfx942DeviceContentDescriptorV1::new(
                        content_roles[index],
                        entries[index].allocation.byte_len(),
                        sha256,
                    ) {
                        Ok(content) => content,
                        Err(_) => {
                            data.push(Gfx942FixedDispatchDataV1::initialized_after_dispatch(lease));
                            quarantine_persistent_compute_entries_v1(
                                entries.each_mut(),
                                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                            );
                            self.set_three_binding_persistent_compute_attachment_v1(
                                ThreeBindingPersistentComputeAttachmentV1 {
                                    entries,
                                    binding: PersistentComputeBindingKeyV1 {
                                        queue: self.key,
                                        attachment_generation,
                                    },
                                    predecessor_dispatch_generation: detached_generation,
                                    terminal_custody: Some(
                                        PersistentComputeTerminalNativeCustodyV1::Data(
                                            PersistentComputeTerminalDataV1::from_vec(data),
                                        ),
                                    ),
                                },
                            );
                            self.next_persistent_compute_generation = next_attachment_generation;
                            self.poison_terminal();
                            return Err(Gfx942ThreeBindingPersistentComputeBindFailureV1 {
                                error: ComputeAqlQueueSessionErrorV1::Contract(
                                    "three-binding persistent content relabeling",
                                ),
                                custody:
                                    Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1::ProcessTeardown(
                                        Gfx942ThreeBindingPersistentComputeBindTerminalCustodyV1 { inputs: None },
                                    ),
                            });
                        }
                    };
                    match Gfx942InitializedDeviceMemoryV1::from_authenticated_full_transfer(
                        lease, content,
                    ) {
                        Ok(initialized) => Gfx942FixedDispatchDataV1::initialized(initialized),
                        Err(lease) => Gfx942FixedDispatchDataV1::initialized_after_dispatch(lease),
                    }
                }
                None if entries[index].fully_initialized => {
                    Gfx942FixedDispatchDataV1::initialized_after_dispatch(lease)
                }
                None => Gfx942FixedDispatchDataV1::uninitialized(lease),
            };
            data.push(item);
        }
        let mut preparation = FixedDispatchPreparationCustodyV1::new(packets, data);
        let prepared_dispatch = settle_persistent_bind_preparation_v1(
            self,
            &mut preparation,
            |session, preparation| {
                session.with_live_queue_memory_model(|memory| {
                    prepare_three_binding_persistent_fixed_dispatch_resources_v1(
                        memory,
                        &programs,
                        preparation,
                        detached_generation,
                        control_identity,
                    )
                    .map_err(Into::into)
                })
            },
            Self::validate_persistent_bind_preparation_v1,
        );
        let prepared_dispatch = match prepared_dispatch {
            Ok(result) => result,
            Err(payload) => {
                quarantine_persistent_compute_entries_v1(
                    entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                );
                self.set_three_binding_persistent_compute_attachment_v1(
                    ThreeBindingPersistentComputeAttachmentV1 {
                        entries,
                        binding: PersistentComputeBindingKeyV1 {
                            queue: self.key,
                            attachment_generation,
                        },
                        predecessor_dispatch_generation: detached_generation,
                        terminal_custody: Some(
                            PersistentComputeTerminalNativeCustodyV1::Preparation(preparation),
                        ),
                    },
                );
                self.next_persistent_compute_generation = next_attachment_generation;
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        let prepared_dispatch = match prepared_dispatch
            .and_then(|()| preparation.take_completed().map_err(Into::into))
        {
            Ok(dispatch) => dispatch,
            Err(error) => {
                quarantine_persistent_compute_entries_v1(
                    entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                );
                self.set_three_binding_persistent_compute_attachment_v1(
                    ThreeBindingPersistentComputeAttachmentV1 {
                        entries,
                        binding: PersistentComputeBindingKeyV1 {
                            queue: self.key,
                            attachment_generation,
                        },
                        predecessor_dispatch_generation: detached_generation,
                        terminal_custody: Some(
                            PersistentComputeTerminalNativeCustodyV1::Preparation(preparation),
                        ),
                    },
                );
                self.next_persistent_compute_generation = next_attachment_generation;
                self.poison_terminal();
                return Err(Gfx942ThreeBindingPersistentComputeBindFailureV1 {
                    error,
                    custody:
                        Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1::ProcessTeardown(
                            Gfx942ThreeBindingPersistentComputeBindTerminalCustodyV1 {
                                inputs: None,
                            },
                        ),
                });
            }
        };
        let binding = PersistentComputeBindingKeyV1 {
            queue: self.key,
            attachment_generation,
        };
        self.dispatch = Some(prepared_dispatch);
        self.detached_data_count = 0;
        self.detached_dispatch_generation = None;
        self.detached_data_identities.clear();
        self.detached_next_insertion_index = None;
        self.set_three_binding_persistent_compute_attachment_v1(
            ThreeBindingPersistentComputeAttachmentV1 {
                entries,
                binding,
                predecessor_dispatch_generation: detached_generation,
                terminal_custody: None,
            },
        );
        self.next_persistent_compute_generation = next_attachment_generation;
        Ok(Gfx942PreparedThreeBindingPersistentComputeDispatchV1 {
            binding,
            thread_affinity: PhantomData,
        })
    }

    /// Detaches one exactly completed and recycled fixed batch while keeping
    /// the native queue and all queue resources live.
    #[allow(clippy::result_large_err)]
    pub fn bind_directional_persistent_fixed_dispatch_v1(
        &mut self,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; 1],
        mut input: Gfx942PersistentComputeInputV1,
        content_role: Gfx942DeviceContentRoleV1,
    ) -> Result<Gfx942PreparedPersistentComputeDispatchV1, Gfx942PersistentComputeBindFailureV1>
    {
        let recover = |error, input| Gfx942PersistentComputeBindFailureV1 {
            error,
            custody: Gfx942PersistentComputeBindFailureCustodyV1::Retryable(input),
        };
        if !input.belongs_to(self.compute_lane_session) {
            return Err(recover(
                if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned.into()
                } else {
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "persistent compute input owner substitution",
                    )
                },
                input,
            ));
        }
        if self.terminal_poisoned {
            return Err(Gfx942PersistentComputeBindFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                    Gfx942PersistentComputeBindTerminalCustodyV1 { input: Some(input) },
                ),
            });
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(recover(
                Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                input,
            ));
        }
        if self.key != self.compute_lane_session {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute requires the primary compute lane",
                ),
                input,
            ));
        }
        let detached_generation = self.detached_dispatch_generation;
        let detached_is_empty = self.unpublished_dispatch.is_clear()
            && self.detached_data_count == 0
            && self.detached_data_identities.is_empty()
            && match detached_generation {
                None => self.detached_next_insertion_index.is_none(),
                Some(_) => self.detached_next_insertion_index == Some(0),
            };
        if !detached_is_empty {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute requires an empty initial or recycled dispatch roster",
                ),
                input,
            ));
        }
        if let Err(error) = self.require_sdma_enabled() {
            return Err(recover(error, input));
        }
        let competing_queues_admitted =
            self.persistent_inputs_coexist_with_directional_sdma_v1(&[&input]);
        if !competing_queues_admitted {
            return Err(recover(
                Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                input,
            ));
        }

        let authenticated_sha256 = match &input {
            Gfx942PersistentComputeInputV1::Uninitialized(_) => None,
            Gfx942PersistentComputeInputV1::Initialized(ready) => Some(ready.authenticated_sha256),
            Gfx942PersistentComputeInputV1::InitializedAfterDispatch(_) => None,
        };
        let initialized = !matches!(&input, Gfx942PersistentComputeInputV1::Uninitialized(_));
        let allocation = match &mut input {
            Gfx942PersistentComputeInputV1::Uninitialized(allocation)
            | Gfx942PersistentComputeInputV1::InitializedAfterDispatch(allocation) => allocation,
            Gfx942PersistentComputeInputV1::Initialized(ready) => &mut ready.allocation,
        };
        if allocation.attachment.queue != self.compute_lane_session
            || !self.directional_persistent_sdma_attachment_is_current(&allocation.attachment)
            || allocation.byte_len() != allocation.physical_byte_len()
            || allocation.owner.live_use_count() != 0
            || allocation.owner.retained_settled_use_count() != 0
        {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute requires exact quiescent full-extent directional custody",
                ),
                input,
            ));
        }
        let (layout, storage_identity) = {
            let Some(lease) = allocation.owner.local_native_for_sdma() else {
                return Err(recover(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "persistent compute requires attached local native custody",
                    ),
                    input,
                ));
            };
            (
                super::dispatch_binding::Gfx942FixedDispatchDataLayoutV1::device_local(
                    lease.layout().requested_bytes(),
                    lease.layout().alignment(),
                ),
                lease.storage_identity(),
            )
        };
        // Exact replay may recover initialization from the queue-retained
        // predecessor premise after identity/currentness validation. Initial
        // read admission still requires an authenticated input.
        let control_initialized = initialized || self.dispatch.is_some();
        let control_identity = match persistent_fixed_dispatch_control_identity_v1(
            self.key,
            &programs,
            &packets,
            layout,
            control_initialized,
            content_role,
            Gfx942SdmaBufferStorageIdentityV1::Device(storage_identity),
        ) {
            Ok(identity) => identity,
            Err(error) => return Err(recover(error.into(), input)),
        };
        if let Some(dispatch) = self.dispatch.as_ref() {
            let Some(predecessor_generation) = detached_generation else {
                return Err(recover(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "retained persistent control requires a recycled predecessor generation",
                    ),
                    input,
                ));
            };
            if let Err(error) =
                dispatch.validate_persistent_replay_v1(control_identity, predecessor_generation)
            {
                return Err(recover(error.into(), input));
            }
        }
        let native_effect = control_identity.effect();
        let (operation, effect) = match native_effect {
            DeviceDataEffectV1::ReadOnly => (
                Gfx942PersistentOperationV1::ComputeRead,
                Gfx942PersistentComputeEffectV1::Read,
            ),
            DeviceDataEffectV1::WriteOnly => (
                Gfx942PersistentOperationV1::ComputeWrite,
                Gfx942PersistentComputeEffectV1::Write,
            ),
            DeviceDataEffectV1::ReadWrite => (
                Gfx942PersistentOperationV1::ComputeReadWrite,
                Gfx942PersistentComputeEffectV1::ReadWrite,
            ),
        };
        let initialized_content = match authenticated_sha256 {
            Some(sha256) => match Gfx942DeviceContentDescriptorV1::new(
                content_role,
                allocation.byte_len(),
                sha256,
            ) {
                Ok(content) => Some(content),
                Err(_) => {
                    return Err(recover(
                        ComputeAqlQueueSessionErrorV1::Contract(
                            "persistent compute initialized-content relabeling",
                        ),
                        input,
                    ));
                }
            },
            None => None,
        };
        let attachment_generation = self.next_persistent_compute_generation;
        let Some(next_attachment_generation) = attachment_generation.checked_add(1) else {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute attachment generation exhausted",
                ),
                input,
            ));
        };
        let request = Gfx942PersistentUseRequestV1::new(operation, 0, allocation.byte_len())
            .expect("nonzero admitted persistent allocation extent");
        let reserved = match allocation.owner.reserve(request, None) {
            Ok(reserved) => reserved,
            Err(failure) => {
                return Err(recover(
                    map_directional_persistent_sdma_use_error_v1(failure.error()),
                    input,
                ));
            }
        };
        let prepared = match allocation.owner.prepare(reserved) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (error, reserved) = failure.into_parts();
                match classify_persistent_bind_cancellation_v1(
                    cancel_persistent_compute_reserved_v1(&mut allocation.owner, reserved),
                ) {
                    PersistentBindCancellationDispositionV1::Retryable => {
                        return Err(recover(
                            map_directional_persistent_sdma_use_error_v1(error),
                            input,
                        ));
                    }
                    PersistentBindCancellationDispositionV1::Terminal(reserved) => {
                        let (allocation, authenticated_sha256, fully_initialized) =
                            input.into_parts();
                        self.set_single_persistent_compute_attachment_v1(
                            PersistentComputeAttachmentV1 {
                                allocation,
                                authenticated_sha256,
                                fully_initialized,
                                state: PersistentComputeUseStateV1::Reserved(reserved),
                                binding: PersistentComputeBindingKeyV1 {
                                    queue: self.key,
                                    attachment_generation,
                                },
                                storage_identity,
                                effect,
                                predecessor_dispatch_generation: detached_generation,
                                terminal_custody: Some(
                                    PersistentComputeTerminalNativeCustodyV1::Attached,
                                ),
                            },
                        );
                        self.next_persistent_compute_generation = next_attachment_generation;
                        // Cancellation failed before native detach, so exact local
                        // custody is terminal without a process-global ambiguity.
                        self.poison_terminal();
                        return Err(Gfx942PersistentComputeBindFailureV1 {
                            error: map_directional_persistent_sdma_use_error_v1(error),
                            custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                                Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
                            ),
                        });
                    }
                }
            }
        };
        if let Some(dispatch) = self.dispatch.take() {
            let predecessor_generation = detached_generation
                .expect("persistent replay was preflighted with a recycled predecessor");
            return self.bind_retained_persistent_fixed_dispatch_control_replay_v1(
                PersistentRetainedControlReplayRequestV1 {
                    input,
                    prepared,
                    dispatch,
                    initialized_content,
                    control_identity,
                    predecessor_generation,
                },
                PersistentRetainedControlReplayCommitV1 {
                    attachment_generation,
                    next_attachment_generation,
                    storage_identity,
                    effect,
                    predecessor_generation,
                },
            );
        }
        let validation =
            validate_persistent_bind_inputs_v1(self, &[&*allocation], |session, allocation| {
                let lease = allocation
                    .owner
                    .local_native_for_sdma()
                    .expect("validated persistent local native custody");
                session.with_live_queue_memory_model(|memory| {
                    memory
                        .mapped_gfx942_device_memory_facts(lease)
                        .map(|_| ())
                        .map_err(Into::into)
                })
            });
        let validation = match validation {
            Ok(result) => result,
            Err(payload) => {
                let (mut allocation, authenticated_sha256, fully_initialized) = input.into_parts();
                let state = quarantine_persistent_retained_control_replay_prepared_v1(
                    &mut allocation.owner,
                    prepared,
                );
                self.set_single_persistent_compute_attachment_v1(PersistentComputeAttachmentV1 {
                    allocation,
                    authenticated_sha256,
                    fully_initialized,
                    state,
                    binding: PersistentComputeBindingKeyV1 {
                        queue: self.key,
                        attachment_generation,
                    },
                    storage_identity,
                    effect,
                    predecessor_dispatch_generation: detached_generation,
                    terminal_custody: Some(PersistentComputeTerminalNativeCustodyV1::Attached),
                });
                self.next_persistent_compute_generation = next_attachment_generation;
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        if let Err(error) = validation {
            let (allocation, authenticated_sha256, fully_initialized) = input.into_parts();
            let mut attachment = PersistentComputeAttachmentV1 {
                allocation,
                authenticated_sha256,
                fully_initialized,
                state: PersistentComputeUseStateV1::Prepared(prepared),
                binding: PersistentComputeBindingKeyV1 {
                    queue: self.key,
                    attachment_generation,
                },
                storage_identity,
                effect,
                predecessor_dispatch_generation: detached_generation,
                terminal_custody: None,
            };
            if cancel_persistent_compute_prepublication_entries_v1([&mut attachment]) {
                let input = Gfx942PersistentComputeInputV1::from_parts(
                    attachment.allocation,
                    attachment.authenticated_sha256,
                    attachment.fully_initialized,
                );
                if persistent_bind_retryable_v1(!self.terminal_poisoned, true) {
                    return Err(recover(error, input));
                }
                return Err(Gfx942PersistentComputeBindFailureV1 {
                    error,
                    custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                        Gfx942PersistentComputeBindTerminalCustodyV1 { input: Some(input) },
                    ),
                });
            }
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Attached);
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.next_persistent_compute_generation = next_attachment_generation;
            self.poison_terminal();
            return Err(Gfx942PersistentComputeBindFailureV1 {
                error,
                custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                    Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
                ),
            });
        }
        let lease = match allocation.owner.detach_local_native_for_compute(&prepared) {
            Ok(lease) => lease,
            Err(error) => match classify_persistent_bind_cancellation_v1(
                cancel_persistent_compute_prepared_v1(&mut allocation.owner, prepared),
            ) {
                PersistentBindCancellationDispositionV1::Retryable => {
                    return Err(recover(
                        map_directional_persistent_sdma_use_error_v1(error),
                        input,
                    ));
                }
                PersistentBindCancellationDispositionV1::Terminal(prepared) => {
                    let (allocation, authenticated_sha256, fully_initialized) = input.into_parts();
                    self.set_single_persistent_compute_attachment_v1(
                        PersistentComputeAttachmentV1 {
                            allocation,
                            authenticated_sha256,
                            fully_initialized,
                            state: PersistentComputeUseStateV1::Prepared(prepared),
                            binding: PersistentComputeBindingKeyV1 {
                                queue: self.key,
                                attachment_generation,
                            },
                            storage_identity,
                            effect,
                            predecessor_dispatch_generation: detached_generation,
                            terminal_custody: Some(
                                PersistentComputeTerminalNativeCustodyV1::Attached,
                            ),
                        },
                    );
                    self.next_persistent_compute_generation = next_attachment_generation;
                    self.poison_terminal();
                    return Err(Gfx942PersistentComputeBindFailureV1 {
                        error: map_directional_persistent_sdma_use_error_v1(error),
                        custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                            Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
                        ),
                    });
                }
            },
        };
        let (mut allocation, authenticated_sha256, initialized) = input.into_parts();
        let data = match initialized_content {
            Some(content) => {
                match Gfx942InitializedDeviceMemoryV1::from_authenticated_full_transfer(
                    lease, content,
                ) {
                    Ok(initialized) => Gfx942FixedDispatchDataV1::initialized(initialized),
                    Err(_lease) => {
                        let state = quarantine_persistent_compute_prepared_v1(
                            &mut allocation.owner,
                            prepared,
                            Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                        );
                        self.set_single_persistent_compute_attachment_v1(
                            PersistentComputeAttachmentV1 {
                                allocation,
                                authenticated_sha256,
                                fully_initialized: initialized,
                                state,
                                binding: PersistentComputeBindingKeyV1 {
                                    queue: self.key,
                                    attachment_generation,
                                },
                                storage_identity,
                                effect,
                                predecessor_dispatch_generation: detached_generation,
                                terminal_custody: Some(
                                    PersistentComputeTerminalNativeCustodyV1::Storage(
                                        Gfx942SdmaBufferStorageV1::Device(_lease),
                                    ),
                                ),
                            },
                        );
                        self.next_persistent_compute_generation = next_attachment_generation;
                        // Native detach rejected before moving the lease; the
                        // terminal attachment still owns exact local custody.
                        self.poison_terminal();
                        return Err(Gfx942PersistentComputeBindFailureV1 {
                            error: ComputeAqlQueueSessionErrorV1::Contract(
                                "persistent compute authenticated extent changed after preflight",
                            ),
                            custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                                Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
                            ),
                        });
                    }
                }
            }
            None if initialized => Gfx942FixedDispatchDataV1::initialized_after_dispatch(lease),
            None => Gfx942FixedDispatchDataV1::uninitialized(lease),
        };
        let mut preparation = FixedDispatchPreparationCustodyV1::new(packets, vec![data]);
        let prepared_dispatch = settle_persistent_bind_preparation_v1(
            self,
            &mut preparation,
            |session, preparation| {
                session.with_live_queue_memory_model(|memory| {
                    prepare_persistent_fixed_dispatch_resources_v1(
                        memory,
                        &programs,
                        preparation,
                        detached_generation,
                        control_identity,
                    )
                    .map_err(Into::into)
                })
            },
            Self::validate_persistent_bind_preparation_v1,
        );
        let prepared_dispatch = match prepared_dispatch {
            Ok(result) => result,
            Err(payload) => {
                let state = quarantine_persistent_retained_control_replay_prepared_v1(
                    &mut allocation.owner,
                    prepared,
                );
                self.set_single_persistent_compute_attachment_v1(PersistentComputeAttachmentV1 {
                    allocation,
                    authenticated_sha256,
                    fully_initialized: initialized,
                    state,
                    binding: PersistentComputeBindingKeyV1 {
                        queue: self.key,
                        attachment_generation,
                    },
                    storage_identity,
                    effect,
                    predecessor_dispatch_generation: detached_generation,
                    terminal_custody: Some(PersistentComputeTerminalNativeCustodyV1::Preparation(
                        preparation,
                    )),
                });
                self.next_persistent_compute_generation = next_attachment_generation;
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        };
        let prepared_dispatch = match prepared_dispatch
            .and_then(|()| preparation.take_completed().map_err(Into::into))
        {
            Ok(dispatch) => dispatch,
            Err(error) => {
                let state = quarantine_persistent_compute_prepared_v1(
                    &mut allocation.owner,
                    prepared,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                );
                self.set_single_persistent_compute_attachment_v1(PersistentComputeAttachmentV1 {
                    allocation,
                    authenticated_sha256,
                    fully_initialized: initialized,
                    state,
                    binding: PersistentComputeBindingKeyV1 {
                        queue: self.key,
                        attachment_generation,
                    },
                    storage_identity,
                    effect,
                    predecessor_dispatch_generation: detached_generation,
                    terminal_custody: Some(PersistentComputeTerminalNativeCustodyV1::Preparation(
                        preparation,
                    )),
                });
                self.next_persistent_compute_generation = next_attachment_generation;
                self.poison_terminal();
                return Err(Gfx942PersistentComputeBindFailureV1 {
                    error,
                    custody: Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(
                        Gfx942PersistentComputeBindTerminalCustodyV1 { input: None },
                    ),
                });
            }
        };
        let binding = PersistentComputeBindingKeyV1 {
            queue: self.key,
            attachment_generation,
        };
        self.dispatch = Some(prepared_dispatch);
        self.detached_data_count = 0;
        self.detached_dispatch_generation = None;
        self.detached_data_identities.clear();
        self.detached_next_insertion_index = None;
        self.set_single_persistent_compute_attachment_v1(PersistentComputeAttachmentV1 {
            allocation,
            authenticated_sha256,
            fully_initialized: initialized,
            state: PersistentComputeUseStateV1::Prepared(prepared),
            binding,
            storage_identity,
            effect,
            predecessor_dispatch_generation: detached_generation,
            terminal_custody: None,
        });
        self.next_persistent_compute_generation = next_attachment_generation;
        Ok(Gfx942PreparedPersistentComputeDispatchV1 {
            binding,
            thread_affinity: PhantomData,
        })
    }

    /// Publishes the exact prepared three-binding persistent-compute attachment.
    #[allow(clippy::result_large_err)]
    pub fn submit_three_binding_directional_persistent_fixed_dispatch_v1(
        &mut self,
        prepared_receipt: Gfx942PreparedThreeBindingPersistentComputeDispatchV1,
    ) -> Result<
        Gfx942ThreeBindingPersistentComputeDispatchV1,
        Gfx942ThreeBindingPersistentComputeExecutionFailureV1,
    > {
        self.submit_three_binding_directional_persistent_fixed_dispatch_v1_using(
            prepared_receipt,
            |session| {
                session.submit_fixed_dispatch_inner_classified::<1>(
                    FixedDispatchBindingModeV1::ExactPersistentAttachment,
                )
            },
        )
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn submit_three_binding_directional_persistent_fixed_dispatch_v1_using(
        &mut self,
        prepared_receipt: Gfx942PreparedThreeBindingPersistentComputeDispatchV1,
        submit: impl FnOnce(
            &mut Self,
        )
            -> Result<Gfx942DispatchBatchV1<1>, FixedDispatchSubmissionFailureV1>,
    ) -> Result<
        Gfx942ThreeBindingPersistentComputeDispatchV1,
        Gfx942ThreeBindingPersistentComputeExecutionFailureV1,
    > {
        let binding = prepared_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                retryable: Some(prepared_receipt),
            });
        }
        let valid = self
            .three_binding_persistent_compute_attachment_v1()
            .is_some_and(|attachment| attachment.binding == binding);
        if !valid {
            return Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                retryable: Some(prepared_receipt),
            });
        }
        let mut attachment = self
            .take_three_binding_persistent_compute_attachment_v1()
            .expect("validated three-binding attachment");
        if self.terminal_poisoned {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Attached);
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            return Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                retryable: None,
            });
        }
        if attachment
            .entries
            .iter()
            .any(|entry| !matches!(entry.state, PersistentComputeUseStateV1::Prepared(_)))
        {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Attached);
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                retryable: None,
            });
        }
        let submission = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| submit(self)));
        let submission = match submission {
            Ok(submission) => submission,
            Err(payload) => {
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                );
                attachment.terminal_custody = None;
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        match submission {
            Ok(batch) => {
                if !publish_persistent_compute_entries_v1(attachment.entries.each_mut()) {
                    quarantine_persistent_compute_entries_v1(
                        attachment.entries.each_mut(),
                        Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                    );
                    attachment.terminal_custody =
                        Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
                    self.set_three_binding_persistent_compute_attachment_v1(attachment);
                    self.poison_terminal();
                    poison_process_global_after_dispatch_terminal_v1();
                    return Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                        error: ComputeAqlQueueSessionErrorV1::Contract(
                            "three-binding persistent publication ledger transition",
                        ),
                        retryable: None,
                    });
                }
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                Ok(Gfx942ThreeBindingPersistentComputeDispatchV1 {
                    binding,
                    batch,
                    thread_affinity: PhantomData,
                })
            }
            Err(FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error)) => {
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                    error,
                    retryable: Some(Gfx942PreparedThreeBindingPersistentComputeDispatchV1 {
                        binding,
                        thread_affinity: PhantomData,
                    }),
                })
            }
            Err(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error)) => {
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                );
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Attached);
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                    error,
                    retryable: None,
                })
            }
            Err(FixedDispatchSubmissionFailureV1::Terminal(error)) => {
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                );
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Attached);
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_dispatch_terminal_v1();
                Err(Gfx942ThreeBindingPersistentComputeExecutionFailureV1 {
                    error,
                    retryable: None,
                })
            }
        }
    }

    /// Publishes the exact prepared persistent-compute attachment.
    #[allow(clippy::result_large_err)]
    pub fn submit_directional_persistent_fixed_dispatch_v1(
        &mut self,
        prepared_receipt: Gfx942PreparedPersistentComputeDispatchV1,
    ) -> Result<Gfx942PersistentComputeDispatchV1, Gfx942PersistentComputeExecutionFailureV1> {
        self.submit_directional_persistent_fixed_dispatch_v1_using(prepared_receipt, |session| {
            session.submit_fixed_dispatch_inner_classified::<1>(
                FixedDispatchBindingModeV1::ExactPersistentAttachment,
            )
        })
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn submit_directional_persistent_fixed_dispatch_v1_using(
        &mut self,
        prepared_receipt: Gfx942PreparedPersistentComputeDispatchV1,
        submit: impl FnOnce(
            &mut Self,
        )
            -> Result<Gfx942DispatchBatchV1<1>, FixedDispatchSubmissionFailureV1>,
    ) -> Result<Gfx942PersistentComputeDispatchV1, Gfx942PersistentComputeExecutionFailureV1> {
        let binding = prepared_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942PersistentComputeExecutionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                retryable: Some(prepared_receipt),
            });
        }
        if self.terminal_poisoned {
            let retryable = (!self.absorb_terminal_prepared_persistent_compute_v1(binding))
                .then_some(prepared_receipt);
            return Err(Gfx942PersistentComputeExecutionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                retryable,
            });
        }
        let valid = self
            .single_persistent_compute_attachment_v1()
            .is_some_and(|attachment| attachment.binding == binding && binding.queue == self.key);
        if !valid {
            return Err(Gfx942PersistentComputeExecutionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                retryable: Some(prepared_receipt),
            });
        }
        let mut attachment = self
            .take_single_persistent_compute_attachment_v1()
            .expect("validated persistent compute attachment");
        if !matches!(attachment.state, PersistentComputeUseStateV1::Prepared(_)) {
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeExecutionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                retryable: None,
            });
        };
        let submission = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| submit(self)));
        let submission = match submission {
            Ok(submission) => submission,
            Err(payload) => {
                quarantine_persistent_compute_entries_v1(
                    [&mut attachment],
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                );
                attachment.terminal_custody = None;
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        match submission {
            Ok(batch) => {
                if !publish_persistent_compute_entries_v1([&mut attachment]) {
                    quarantine_persistent_compute_entries_v1(
                        [&mut attachment],
                        Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                    );
                    attachment.terminal_custody =
                        Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
                    self.set_single_persistent_compute_attachment_v1(attachment);
                    self.poison_terminal();
                    return Err(Gfx942PersistentComputeExecutionFailureV1 {
                        error: ComputeAqlQueueSessionErrorV1::Contract(
                            "persistent compute publication ledger transition",
                        ),
                        retryable: None,
                    });
                }
                self.set_single_persistent_compute_attachment_v1(attachment);
                Ok(Gfx942PersistentComputeDispatchV1 {
                    binding,
                    batch,
                    thread_affinity: PhantomData,
                })
            }
            Err(FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error)) => {
                self.set_single_persistent_compute_attachment_v1(attachment);
                Err(Gfx942PersistentComputeExecutionFailureV1 {
                    error,
                    retryable: Some(Gfx942PreparedPersistentComputeDispatchV1 {
                        binding,
                        thread_affinity: PhantomData,
                    }),
                })
            }
            Err(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error)) => {
                quarantine_persistent_compute_entries_v1(
                    [&mut attachment],
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                );
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Attached);
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                Err(Gfx942PersistentComputeExecutionFailureV1 {
                    error,
                    retryable: None,
                })
            }
            Err(FixedDispatchSubmissionFailureV1::Terminal(error)) => {
                quarantine_persistent_compute_entries_v1(
                    [&mut attachment],
                    Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
                );
                attachment.terminal_custody = None;
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                Err(Gfx942PersistentComputeExecutionFailureV1 {
                    error,
                    retryable: None,
                })
            }
        }
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn terminalize_persistent_compute_poll_result_v1<T>(
        &mut self,
        result: Result<T, Gfx942PersistentComputePollFailureV1>,
    ) -> Result<T, Gfx942PersistentComputePollFailureV1> {
        if result
            .as_ref()
            .is_err_and(|failure| failure.recovered.is_none())
        {
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
        }
        result
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn terminalize_persistent_compute_recycle_result_v1<T>(
        &mut self,
        result: Result<T, Gfx942PersistentComputeRecycleFailureV1>,
    ) -> Result<T, Gfx942PersistentComputeRecycleFailureV1> {
        if result
            .as_ref()
            .is_err_and(|failure| failure.recovered.is_none())
        {
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
        }
        result
    }

    /// Cancels an exact prepared attachment before publication and restores
    /// the original initialized or uninitialized persistent input.
    #[allow(clippy::result_large_err)]
    pub fn cancel_prepared_directional_persistent_fixed_dispatch_v1(
        &mut self,
        prepared_receipt: Gfx942PreparedPersistentComputeDispatchV1,
    ) -> Result<Gfx942PersistentComputeInputV1, Gfx942PersistentComputeCancelFailureV1> {
        let binding = prepared_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(prepared_receipt),
                retained: None,
            });
        }
        if self.terminal_poisoned {
            let recovered = (!self.absorb_terminal_prepared_persistent_compute_v1(binding))
                .then_some(prepared_receipt);
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                recovered,
                retained: None,
            });
        }
        let valid = self
            .single_persistent_compute_attachment_v1()
            .is_some_and(|attachment| attachment.binding == binding && binding.queue == self.key);
        if !valid {
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: Some(prepared_receipt),
                retained: None,
            });
        }
        let mut attachment = self
            .take_single_persistent_compute_attachment_v1()
            .expect("validated persistent compute attachment");
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Prepared(prepared) = state else {
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
                retained: None,
            });
        };
        let returned = self.release_persistent_dispatch_data(false);
        let (generation, mut data) = match returned {
            Ok(returned) => returned,
            Err((error, data)) => {
                attachment.state = quarantine_persistent_compute_prepared_v1(
                    &mut attachment.allocation.owner,
                    prepared,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                );
                attachment.terminal_custody = Some(if data.is_empty() && self.dispatch.is_some() {
                    PersistentComputeTerminalNativeCustodyV1::Attached
                } else {
                    PersistentComputeTerminalNativeCustodyV1::Data(
                        PersistentComputeTerminalDataV1::from_vec(data),
                    )
                });
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                return Err(Gfx942PersistentComputeCancelFailureV1 {
                    error,
                    recovered: None,
                    retained: None,
                });
            }
        };
        let expected_generation = attachment.predecessor_dispatch_generation.unwrap_or(0);
        let exact = generation == expected_generation
            && data.len() == 1
            && data[0].sdma_storage_identity()
                == Gfx942SdmaBufferStorageIdentityV1::Device(attachment.storage_identity);
        if !exact {
            attachment.state = quarantine_persistent_compute_prepared_v1(
                &mut attachment.allocation.owner,
                prepared,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Data(
                PersistentComputeTerminalDataV1::from_vec(data),
            ));
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute cancellation returned substituted storage",
                ),
                recovered: None,
                retained: None,
            });
        }
        let data = data.pop().expect("validated one returned data authority");
        let fully_initialized = data.is_fully_initialized();
        let Gfx942SdmaBufferStorageV1::Device(lease) = data.into_sdma_storage() else {
            unreachable!("validated device storage identity")
        };
        if let Err((_error, lease)) = attachment
            .allocation
            .owner
            .restore_local_native_from_cancelled_compute(&prepared, lease)
        {
            attachment.state = quarantine_persistent_compute_prepared_v1(
                &mut attachment.allocation.owner,
                prepared,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Storage(
                Gfx942SdmaBufferStorageV1::Device(lease),
            ));
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute cancellation native restore",
                ),
                recovered: None,
                retained: None,
            });
        }
        if attachment
            .allocation
            .owner
            .cancel_prepared(prepared)
            .is_err()
        {
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Restored);
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeCancelFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute cancellation ledger transition",
                ),
                recovered: None,
                retained: None,
            });
        }
        self.detached_dispatch_generation = Some(expected_generation);
        self.detached_next_insertion_index = Some(0);
        self.detached_data_count = 0;
        self.detached_data_identities.clear();
        Ok(Gfx942PersistentComputeInputV1::from_parts(
            attachment.allocation,
            attachment.authenticated_sha256,
            fully_initialized,
        ))
    }

    /// Cancels the exact prepared three-binding attachment before publication
    /// and restores all three original persistent inputs atomically.
    #[allow(clippy::result_large_err)]
    pub fn cancel_prepared_three_binding_directional_persistent_fixed_dispatch_v1(
        &mut self,
        prepared_receipt: Gfx942PreparedThreeBindingPersistentComputeDispatchV1,
    ) -> Result<
        Gfx942ThreeBindingPersistentComputeInputsV1,
        Gfx942ThreeBindingPersistentComputeCancelFailureV1,
    > {
        let binding = prepared_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(prepared_receipt),
            });
        }
        let valid = self
            .three_binding_persistent_compute_attachment_v1()
            .is_some_and(|attachment| attachment.binding == binding);
        if !valid {
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(prepared_receipt),
            });
        }
        let mut attachment = self
            .take_three_binding_persistent_compute_attachment_v1()
            .expect("validated three-binding prepared attachment");
        if self.terminal_poisoned
            || attachment
                .entries
                .iter()
                .any(|entry| !matches!(entry.state, PersistentComputeUseStateV1::Prepared(_)))
        {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Attached);
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
            });
        }

        let (generation, data) = match self.release_persistent_dispatch_data(false) {
            Ok(returned) => returned,
            Err((error, data)) => {
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                );
                attachment.terminal_custody = Some(if data.is_empty() && self.dispatch.is_some() {
                    PersistentComputeTerminalNativeCustodyV1::Attached
                } else {
                    PersistentComputeTerminalNativeCustodyV1::Data(
                        PersistentComputeTerminalDataV1::from_vec(data),
                    )
                });
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_dispatch_terminal_v1();
                return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                    error,
                    recovered: None,
                });
            }
        };
        let expected_generation = attachment.predecessor_dispatch_generation.unwrap_or(0);
        let exact = generation == expected_generation
            && data.len() == 3
            && data.iter().zip(&attachment.entries).all(|(data, entry)| {
                entry.storage_identity.is_some_and(|identity| {
                    data.sdma_storage_identity()
                        == Gfx942SdmaBufferStorageIdentityV1::Device(identity)
                }) && data.is_fully_initialized() == entry.fully_initialized
            });
        if !exact {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Data(
                PersistentComputeTerminalDataV1::from_vec(data),
            ));
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent cancellation returned substituted storage",
                ),
                recovered: None,
            });
        }
        let data: [Gfx942FixedDispatchDataV1; 3] = data
            .try_into()
            .unwrap_or_else(|_| unreachable!("validated three-binding cancellation cardinality"));
        let fully_initialized = std::array::from_fn(|index| data[index].is_fully_initialized());
        let leases = data.map(|data| {
            let Gfx942SdmaBufferStorageV1::Device(lease) = data.into_sdma_storage() else {
                unreachable!("validated three-binding cancellation device storage")
            };
            lease
        });
        let restore_preflight =
            attachment
                .entries
                .iter()
                .zip(&leases)
                .try_for_each(|(entry, lease)| {
                    let PersistentComputeUseStateV1::Prepared(prepared) = &entry.state else {
                        return Err(Gfx942PersistentUseErrorV1::WrongState);
                    };
                    entry
                        .allocation
                        .owner
                        .preflight_restore_local_native_from_cancelled_compute(prepared, lease)
                });
        if restore_preflight.is_err() {
            let [lease0, lease1, lease2] = leases;
            let [initialized0, initialized1, initialized2] = fully_initialized;
            let restore_data = |lease, initialized| {
                if initialized {
                    Gfx942FixedDispatchDataV1::initialized_after_dispatch(lease)
                } else {
                    Gfx942FixedDispatchDataV1::uninitialized(lease)
                }
            };
            let retained = [
                restore_data(lease0, initialized0),
                restore_data(lease1, initialized1),
                restore_data(lease2, initialized2),
            ];
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Data(
                PersistentComputeTerminalDataV1::from_three(retained),
            ));
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent cancellation native restore preflight",
                ),
                recovered: None,
            });
        }
        for (entry, lease) in attachment.entries.iter_mut().zip(leases) {
            let PersistentComputeUseStateV1::Prepared(prepared) = &entry.state else {
                unreachable!("preflighted three-binding cancellation state")
            };
            entry
                .allocation
                .owner
                .restore_local_native_from_cancelled_compute(prepared, lease)
                .unwrap_or_else(|_| {
                    unreachable!("preflighted three-binding cancellation native restore")
                });
        }
        if attachment.entries.iter().any(|entry| {
            let PersistentComputeUseStateV1::Prepared(prepared) = &entry.state else {
                return true;
            };
            entry
                .allocation
                .owner
                .preflight_cancel_prepared(prepared)
                .is_err()
        }) {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Restored);
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent cancellation ledger preflight",
                ),
                recovered: None,
            });
        }
        if !cancel_persistent_compute_prepublication_entries_v1(attachment.entries.each_mut()) {
            unreachable!("preflighted three-binding cancellation ledger transition")
        }
        for (entry, initialized) in attachment.entries.iter_mut().zip(fully_initialized) {
            entry.fully_initialized = initialized;
        }
        self.detached_dispatch_generation = Some(expected_generation);
        self.detached_next_insertion_index = Some(0);
        self.detached_data_count = 0;
        self.detached_data_identities.clear();
        Ok(three_binding_entries_into_inputs_v1(attachment.entries))
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn poll_directional_persistent_fixed_dispatch_inner_v1<Completed>(
        &mut self,
        dispatch_receipt: Gfx942PersistentComputeDispatchV1,
        observe: impl FnOnce(
            &mut Self,
            Gfx942CompletionBatchV1<1>,
        ) -> Result<
            PersistentComputeCompletionObservationV1<Completed>,
            (ComputeAqlQueueSessionErrorV1, Gfx942CompletionBatchV1<1>),
        >,
        into_completed: impl FnOnce(Completed) -> Gfx942CompletedBatchV1<1>,
    ) -> Result<
        PersistentComputePollTransitionV1<
            Gfx942PersistentComputeDispatchV1,
            PersistentComputeCompletedTransitionV1<Completed>,
        >,
        Gfx942PersistentComputePollFailureV1,
    > {
        let binding = dispatch_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(dispatch_receipt),
                retained: None,
            });
        }
        if self.terminal_poisoned {
            return match self
                .absorb_terminal_published_persistent_compute_v1(binding, dispatch_receipt.batch)
            {
                Ok(()) => Err(Gfx942PersistentComputePollFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: None,
                    retained: None,
                }),
                Err(batch) => Err(Gfx942PersistentComputePollFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: Some(Gfx942PersistentComputeDispatchV1 {
                        binding,
                        batch,
                        thread_affinity: PhantomData,
                    }),
                    retained: None,
                }),
            };
        }
        let valid = self
            .single_persistent_compute_attachment_v1()
            .is_some_and(|attachment| attachment.binding == binding && binding.queue == self.key);
        if !valid {
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: Some(dispatch_receipt),
                retained: None,
            });
        }
        let mut attachment = self
            .take_single_persistent_compute_attachment_v1()
            .expect("validated persistent compute attachment");
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Published(published) = state else {
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
                retained: Some(PersistentComputeTerminalNativeCustodyV1::Published(
                    dispatch_receipt.batch,
                )),
            });
        };
        let (completion, identity) = unwrap_published(dispatch_receipt.batch);
        let completion_occurrence = completion.occurrence_v1();
        let generation_is_current = self
            .dispatch
            .as_ref()
            .is_some_and(|dispatch| dispatch.validate_published(identity, &completion).is_ok());
        if !generation_is_current {
            let batch = wrap_published(completion, identity);
            attachment.state = PersistentComputeUseStateV1::Published(published);
            quarantine_persistent_compute_entries_v1(
                [&mut attachment],
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
                retained: None,
            });
        }
        let completion_occurrence = completion_occurrence.expect("validated completion identity");
        let observed =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(self, completion)));
        let observed = match observed {
            Ok(observed) => observed,
            Err(payload) => {
                attachment.state = PersistentComputeUseStateV1::Published(published);
                quarantine_persistent_compute_entries_v1(
                    [&mut attachment],
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody = None;
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        let completed = match observed {
            Ok(PersistentComputeCompletionObservationV1::Pending(batch)) => {
                attachment.state = PersistentComputeUseStateV1::Published(published);
                self.set_single_persistent_compute_attachment_v1(attachment);
                return Ok(PersistentComputePollTransitionV1::Pending(
                    Gfx942PersistentComputeDispatchV1 {
                        binding,
                        batch: wrap_published(batch, identity),
                        thread_affinity: PhantomData,
                    },
                ));
            }
            Ok(PersistentComputeCompletionObservationV1::Ready(completed)) => completed,
            Err((error, completion)) => {
                let batch = wrap_published(completion, identity);
                attachment.state = PersistentComputeUseStateV1::Published(published);
                quarantine_persistent_compute_entries_v1(
                    [&mut attachment],
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                return Err(Gfx942PersistentComputePollFailureV1 {
                    error,
                    recovered: None,
                    retained: None,
                });
            }
        };
        if self
            .dispatch
            .as_mut()
            .expect("persistent dispatch retained")
            .mark_completed_occurrence(identity, completion_occurrence)
            .is_err()
        {
            let completed = wrap_completed(into_completed(completed), identity);
            attachment.state = PersistentComputeUseStateV1::Published(published);
            quarantine_persistent_compute_entries_v1(
                [&mut attachment],
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Completed(completed),
            );
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
                retained: None,
            });
        }
        attachment.state = PersistentComputeUseStateV1::Published(published);
        if !complete_persistent_compute_entries_v1([&mut attachment]) {
            let completed = wrap_completed(into_completed(completed), identity);
            quarantine_persistent_compute_entries_v1(
                [&mut attachment],
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Completed(completed),
            );
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputePollFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute completion ledger transition",
                ),
                recovered: None,
                retained: None,
            });
        }
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Completed(completed_use) = state else {
            unreachable!("shared completion transition produced completed state")
        };
        Ok(PersistentComputePollTransitionV1::Ready(
            PersistentComputeCompletedTransitionV1 {
                binding,
                attachment,
                completed_use,
                identity,
                completion_occurrence,
                completed,
            },
        ))
    }

    /// Polls and immediately recycles the exact three-binding dispatch.
    #[allow(clippy::result_large_err)]
    pub fn poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1(
        &mut self,
        dispatch_receipt: Gfx942ThreeBindingPersistentComputeDispatchV1,
    ) -> Result<
        Gfx942ThreeBindingPersistentComputePollAndRecycleV1,
        Gfx942ThreeBindingPersistentComputePollAndRecycleFailureV1,
    > {
        self.poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1_using(
            dispatch_receipt,
            |session, identity, completion| {
                session.dispatch.as_ref().is_some_and(|dispatch| {
                    dispatch.validate_published(identity, completion).is_ok()
                })
            },
            |session, completion| {
                session.poll_completion_batch_with_current_handoff_retaining(completion)
            },
        )
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1_using(
        &mut self,
        dispatch_receipt: Gfx942ThreeBindingPersistentComputeDispatchV1,
        validate_dispatch: impl FnOnce(
            &mut Self,
            DispatchEpochIdentityV1,
            &Gfx942CompletionBatchV1<1>,
        ) -> bool,
        observe_completion: impl FnOnce(
            &mut Self,
            Gfx942CompletionBatchV1<1>,
        ) -> Result<
            CompletionPollWithCurrentnessHandoffV1<1>,
            (ComputeAqlQueueSessionErrorV1, Gfx942CompletionBatchV1<1>),
        >,
    ) -> Result<
        Gfx942ThreeBindingPersistentComputePollAndRecycleV1,
        Gfx942ThreeBindingPersistentComputePollAndRecycleFailureV1,
    > {
        let binding = dispatch_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(dispatch_receipt),
            });
        }
        let valid = self
            .three_binding_persistent_compute_attachment_v1()
            .is_some_and(|attachment| attachment.binding == binding);
        if !valid {
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(dispatch_receipt),
            });
        }
        let mut attachment = self
            .take_three_binding_persistent_compute_attachment_v1()
            .expect("validated three-binding published attachment");
        if self.terminal_poisoned {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Published(dispatch_receipt.batch),
            );
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                recovered: None,
            });
        }
        if attachment
            .entries
            .iter()
            .any(|entry| !matches!(entry.state, PersistentComputeUseStateV1::Published(_)))
        {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Published(dispatch_receipt.batch),
            );
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
            });
        }
        let (completion, identity) = unwrap_published(dispatch_receipt.batch);
        let completion_occurrence = completion.occurrence_v1();
        if !validate_dispatch(self, identity, &completion) {
            let batch = wrap_published(completion, identity);
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
            });
        }
        let completion_occurrence = completion_occurrence.expect("validated completion occurrence");
        let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            observe_completion(self, completion)
        }));
        let observed = match observed {
            Ok(observed) => observed,
            Err(payload) => {
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody = None;
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        let handoff = match observed {
            Ok(CompletionPollWithCurrentnessHandoffV1::Pending { batch, .. }) => {
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                return Ok(
                    Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Pending(
                        Gfx942ThreeBindingPersistentComputeDispatchV1 {
                            binding,
                            batch: wrap_published(batch, identity),
                            thread_affinity: PhantomData,
                        },
                    ),
                );
            }
            Ok(CompletionPollWithCurrentnessHandoffV1::Ready { handoff, .. }) => handoff,
            Err((error, completion)) => {
                let batch = wrap_published(completion, identity);
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Published(batch));
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_dispatch_terminal_v1();
                return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                    error,
                    recovered: None,
                });
            }
        };
        let completion_observed_at = Instant::now();
        if self
            .dispatch
            .as_mut()
            .expect("three-binding persistent dispatch retained")
            .mark_completed_occurrence(identity, completion_occurrence)
            .is_err()
        {
            let completed = wrap_completed(handoff.into_completed(), identity);
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Completed(completed),
            );
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
            });
        }
        if !complete_persistent_compute_entries_v1(attachment.entries.each_mut()) {
            let completed = wrap_completed(handoff.into_completed(), identity);
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Completed(completed),
            );
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent completion ledger transition",
                ),
                recovered: None,
            });
        }
        let recycled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::recycle_completion_current_handoff_retaining(self, handoff)
        }));
        let recycle = match recycled {
            Ok(Ok(recycle)) => recycle,
            Ok(Err((error, handoff))) => {
                let completed = wrap_completed(handoff.into_completed(), identity);
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody = Some(
                    PersistentComputeTerminalNativeCustodyV1::Completed(completed),
                );
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_dispatch_terminal_v1();
                return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                    error,
                    recovered: None,
                });
            }
            Err(payload) => {
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody = None;
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        if self
            .dispatch
            .as_mut()
            .expect("three-binding persistent dispatch retained")
            .mark_recycled_occurrence(identity, completion_occurrence)
            .is_err()
        {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Recycled(recycle));
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
            });
        }
        if !recycle_persistent_compute_entries_v1(attachment.entries.each_mut()) {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Recycled(recycle));
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
            });
        }
        self.set_three_binding_persistent_compute_attachment_v1(attachment);
        Ok(
            Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Recycled {
                recycled: Gfx942RecycledThreeBindingPersistentComputeDispatchV1 {
                    binding,
                    recycle,
                    thread_affinity: PhantomData,
                },
                completion_observed_at,
            },
        )
    }

    /// Polls one published persistent-compute dispatch, retaining all custody
    /// in either returned typestate.
    #[allow(clippy::result_large_err)]
    pub fn poll_directional_persistent_fixed_dispatch_v1(
        &mut self,
        dispatch_receipt: Gfx942PersistentComputeDispatchV1,
    ) -> Result<Gfx942PersistentComputePollV1, Gfx942PersistentComputePollFailureV1> {
        let transition = self.poll_directional_persistent_fixed_dispatch_inner_v1(
            dispatch_receipt,
            |session, completion| {
                session
                    .poll_completion_batch_with_progress_retaining(completion)
                    .map(|poll| match poll {
                        Gfx942CompletionPollWithProgressV1::Pending { batch, .. } => {
                            PersistentComputeCompletionObservationV1::Pending(batch)
                        }
                        Gfx942CompletionPollWithProgressV1::Ready { completed, .. } => {
                            PersistentComputeCompletionObservationV1::Ready(completed)
                        }
                    })
            },
            |completed| completed,
        );
        let transition = self.terminalize_persistent_compute_poll_result_v1(transition)?;
        match transition {
            PersistentComputePollTransitionV1::Pending(dispatch) => {
                Ok(Gfx942PersistentComputePollV1::Pending(dispatch))
            }
            PersistentComputePollTransitionV1::Ready(PersistentComputeCompletedTransitionV1 {
                binding,
                mut attachment,
                completed_use,
                identity,
                completion_occurrence: _,
                completed,
            }) => {
                attachment.state = PersistentComputeUseStateV1::Completed(completed_use);
                self.set_single_persistent_compute_attachment_v1(attachment);
                Ok(Gfx942PersistentComputePollV1::Ready(
                    Gfx942CompletedPersistentComputeDispatchV1 {
                        binding,
                        completed: wrap_completed(completed, identity),
                        thread_affinity: PhantomData,
                    },
                ))
            }
        }
    }

    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub(super) fn finish_directional_persistent_fixed_dispatch_recycle_inner_v1<Completed>(
        &mut self,
        binding: PersistentComputeBindingKeyV1,
        mut attachment: PersistentComputeAttachmentV1,
        completed_use: Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
        identity: DispatchEpochIdentityV1,
        completion_occurrence: super::completion::CompletionBatchOccurrenceV1,
        completed: Completed,
        recycle: impl FnOnce(
            &mut Self,
            Completed,
        ) -> Result<
            Gfx942CompletionRecycleObservationV1,
            (ComputeAqlQueueSessionErrorV1, Completed),
        >,
        into_completed: impl FnOnce(Completed) -> Gfx942CompletedBatchV1<1>,
    ) -> Result<Gfx942RecycledPersistentComputeDispatchV1, Gfx942PersistentComputeRecycleFailureV1>
    {
        attachment.state = PersistentComputeUseStateV1::Completed(completed_use);
        let recycled =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| recycle(self, completed)));
        let recycled = match recycled {
            Ok(recycled) => recycled,
            Err(payload) => {
                quarantine_persistent_compute_entries_v1(
                    [&mut attachment],
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody = None;
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_persistent_unwind_v1();
                std::panic::resume_unwind(payload)
            }
        };
        let recycle = match recycled {
            Ok(recycle) => recycle,
            Err((error, completed)) => {
                let completed = wrap_completed(into_completed(completed), identity);
                quarantine_persistent_compute_entries_v1(
                    [&mut attachment],
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
                );
                attachment.terminal_custody = Some(
                    PersistentComputeTerminalNativeCustodyV1::Completed(completed),
                );
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                return Err(Gfx942PersistentComputeRecycleFailureV1 {
                    error,
                    recovered: None,
                    retained: None,
                });
            }
        };
        if self
            .dispatch
            .as_mut()
            .expect("persistent dispatch retained")
            .mark_recycled_occurrence(identity, completion_occurrence)
            .is_err()
        {
            quarantine_persistent_compute_entries_v1(
                [&mut attachment],
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Recycled(recycle));
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                recovered: None,
                retained: None,
            });
        }
        if !recycle_persistent_compute_entries_v1([&mut attachment]) {
            quarantine_persistent_compute_entries_v1(
                [&mut attachment],
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody =
                Some(PersistentComputeTerminalNativeCustodyV1::Recycled(recycle));
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
                retained: None,
            });
        }
        self.set_single_persistent_compute_attachment_v1(attachment);
        Ok(Gfx942RecycledPersistentComputeDispatchV1 {
            binding,
            recycle,
            thread_affinity: PhantomData,
        })
    }

    /// Polls one published persistent-compute dispatch and, on Ready, recycles
    /// its exact completion signal without reopening the just-closed
    /// currentness envelope. Pending preserves the ordinary two-check poll.
    #[allow(clippy::result_large_err)]
    pub fn poll_and_recycle_directional_persistent_fixed_dispatch_v1(
        &mut self,
        dispatch_receipt: Gfx942PersistentComputeDispatchV1,
    ) -> Result<
        Gfx942PersistentComputePollAndRecycleV1,
        Gfx942PersistentComputePollAndRecycleFailureV1,
    > {
        let transition = execute_persistent_compute_poll_and_recycle_v1(
            self,
            |session| {
                session.poll_directional_persistent_fixed_dispatch_inner_v1(
                    dispatch_receipt,
                    |session, completion| {
                        session
                            .poll_completion_batch_with_current_handoff_retaining(completion)
                            .map(|poll| match poll {
                                CompletionPollWithCurrentnessHandoffV1::Pending {
                                    batch, ..
                                } => PersistentComputeCompletionObservationV1::Pending(batch),
                                CompletionPollWithCurrentnessHandoffV1::Ready {
                                    handoff, ..
                                } => PersistentComputeCompletionObservationV1::Ready(handoff),
                            })
                    },
                    CompletionCurrentnessHandoffV1::into_completed,
                )
            },
            |_| Instant::now(),
            |session, completed| {
                let PersistentComputeCompletedTransitionV1 {
                    binding,
                    attachment,
                    completed_use,
                    identity,
                    completion_occurrence,
                    completed: handoff,
                } = completed;
                session.finish_directional_persistent_fixed_dispatch_recycle_inner_v1(
                    binding,
                    attachment,
                    completed_use,
                    identity,
                    completion_occurrence,
                    handoff,
                    Self::recycle_completion_current_handoff_retaining,
                    CompletionCurrentnessHandoffV1::into_completed,
                )
            },
        );
        match transition {
            Ok(PersistentComputePollAndRecycleTransitionV1::Pending(dispatch)) => {
                Ok(Gfx942PersistentComputePollAndRecycleV1::Pending(dispatch))
            }
            Ok(PersistentComputePollAndRecycleTransitionV1::Recycled {
                recycled,
                completion_observed_at,
            }) => Ok(Gfx942PersistentComputePollAndRecycleV1::Recycled {
                recycled,
                completion_observed_at,
            }),
            Err(PersistentComputePollAndRecycleTransitionFailureV1::Poll(failure)) => {
                let failure = self
                    .terminalize_persistent_compute_poll_result_v1::<()>(Err(failure))
                    .expect_err("poll transition already failed");
                Err(Gfx942PersistentComputePollAndRecycleFailureV1::Poll(
                    failure,
                ))
            }
            Err(PersistentComputePollAndRecycleTransitionFailureV1::Recycle(failure)) => {
                let failure = self
                    .terminalize_persistent_compute_recycle_result_v1::<()>(Err(failure))
                    .expect_err("recycle transition already failed");
                Err(Gfx942PersistentComputePollAndRecycleFailureV1::Recycle(
                    failure,
                ))
            }
        }
    }

    /// Waits until one published persistent-compute dispatch completes or the
    /// monotonic deadline expires, recycling its signal immediately on Ready.
    ///
    /// The first completion observation is unconditional, including when
    /// `deadline` has already elapsed. A clean timeout returns the exact
    /// Published dispatch without resetting its signal or advancing either
    /// retirement ledger.
    #[allow(clippy::result_large_err)]
    pub fn wait_and_recycle_directional_persistent_fixed_dispatch_until_v1(
        &mut self,
        dispatch_receipt: Gfx942PersistentComputeDispatchV1,
        deadline: Instant,
    ) -> Result<
        Gfx942PersistentComputeWaitAndRecycleV1,
        Gfx942PersistentComputePollAndRecycleFailureV1,
    > {
        let mut wait = MonotonicWaitV1::until(deadline);
        let transition = execute_persistent_compute_wait_and_recycle_v1(
            self,
            dispatch_receipt,
            |session, dispatch| {
                session
                    .poll_and_recycle_directional_persistent_fixed_dispatch_v1(dispatch)
                    .map(|transition| match transition {
                        Gfx942PersistentComputePollAndRecycleV1::Pending(dispatch) => {
                            PersistentComputePollAndRecycleTransitionV1::Pending(dispatch)
                        }
                        Gfx942PersistentComputePollAndRecycleV1::Recycled {
                            recycled,
                            completion_observed_at,
                        } => PersistentComputePollAndRecycleTransitionV1::Recycled {
                            recycled,
                            completion_observed_at,
                        },
                    })
            },
            |_| {
                if wait.expired() {
                    true
                } else {
                    wait.pause();
                    wait.expired()
                }
            },
        )?;
        Ok(match transition {
            PersistentComputeWaitAndRecycleTransitionV1::Timeout {
                pending: dispatch,
                observations,
            } => Gfx942PersistentComputeWaitAndRecycleV1::Timeout {
                dispatch,
                observations,
            },
            PersistentComputeWaitAndRecycleTransitionV1::Recycled {
                recycled,
                completion_observed_at,
                observations,
            } => Gfx942PersistentComputeWaitAndRecycleV1::Recycled {
                recycled,
                completion_observed_at,
                observations,
            },
        })
    }

    /// Waits for the exact three-binding dispatch until a monotonic deadline,
    /// preserving Published custody on a clean timeout.
    #[allow(clippy::result_large_err)]
    pub fn wait_and_recycle_three_binding_directional_persistent_fixed_dispatch_until_v1(
        &mut self,
        dispatch_receipt: Gfx942ThreeBindingPersistentComputeDispatchV1,
        deadline: Instant,
    ) -> Result<
        Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1,
        Gfx942ThreeBindingPersistentComputePollAndRecycleFailureV1,
    > {
        self.wait_and_recycle_three_binding_directional_persistent_fixed_dispatch_until_v1_using(
            dispatch_receipt,
            deadline,
            |session, dispatch| {
                session.poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1(
                    dispatch,
                )
            },
        )
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn wait_and_recycle_three_binding_directional_persistent_fixed_dispatch_until_v1_using(
        &mut self,
        dispatch_receipt: Gfx942ThreeBindingPersistentComputeDispatchV1,
        deadline: Instant,
        mut poll: impl FnMut(
            &mut Self,
            Gfx942ThreeBindingPersistentComputeDispatchV1,
        ) -> Result<
            Gfx942ThreeBindingPersistentComputePollAndRecycleV1,
            Gfx942ThreeBindingPersistentComputePollAndRecycleFailureV1,
        >,
    ) -> Result<
        Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1,
        Gfx942ThreeBindingPersistentComputePollAndRecycleFailureV1,
    > {
        let mut wait = MonotonicWaitV1::until(deadline);
        let transition = execute_persistent_compute_wait_and_recycle_v1(
            self,
            dispatch_receipt,
            |session, dispatch| {
                poll(session, dispatch).map(|transition| match transition {
                    Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Pending(dispatch) => {
                        PersistentComputePollAndRecycleTransitionV1::Pending(dispatch)
                    }
                    Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Recycled {
                        recycled,
                        completion_observed_at,
                    } => PersistentComputePollAndRecycleTransitionV1::Recycled {
                        recycled,
                        completion_observed_at,
                    },
                })
            },
            |_| {
                if wait.expired() {
                    true
                } else {
                    wait.pause();
                    wait.expired()
                }
            },
        )?;
        Ok(match transition {
            PersistentComputeWaitAndRecycleTransitionV1::Timeout {
                pending: dispatch,
                observations,
            } => Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1::Timeout {
                dispatch,
                observations,
            },
            PersistentComputeWaitAndRecycleTransitionV1::Recycled {
                recycled,
                completion_observed_at,
                observations,
            } => Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1::Recycled {
                recycled,
                completion_observed_at,
                observations,
            },
        })
    }

    /// Recycles the exact completion signal after device completion.
    #[allow(clippy::result_large_err)]
    pub fn recycle_directional_persistent_fixed_dispatch_v1(
        &mut self,
        completed_receipt: Gfx942CompletedPersistentComputeDispatchV1,
    ) -> Result<Gfx942RecycledPersistentComputeDispatchV1, Gfx942PersistentComputeRecycleFailureV1>
    {
        let binding = completed_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942PersistentComputeRecycleFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(completed_receipt),
                retained: None,
            });
        }
        if self.terminal_poisoned {
            return match self.absorb_terminal_completed_persistent_compute_v1(
                binding,
                completed_receipt.completed,
            ) {
                Ok(()) => Err(Gfx942PersistentComputeRecycleFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: None,
                    retained: None,
                }),
                Err(completed) => Err(Gfx942PersistentComputeRecycleFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: Some(Gfx942CompletedPersistentComputeDispatchV1 {
                        binding,
                        completed,
                        thread_affinity: PhantomData,
                    }),
                    retained: None,
                }),
            };
        }
        let valid = self
            .single_persistent_compute_attachment_v1()
            .is_some_and(|attachment| attachment.binding == binding && binding.queue == self.key);
        if !valid {
            return Err(Gfx942PersistentComputeRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: Some(completed_receipt),
                retained: None,
            });
        }
        let mut attachment = self
            .take_single_persistent_compute_attachment_v1()
            .expect("validated persistent compute attachment");
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Completed(completed_use) = state else {
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return self.terminalize_persistent_compute_recycle_result_v1(Err(
                Gfx942PersistentComputeRecycleFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                    recovered: None,
                    retained: Some(PersistentComputeTerminalNativeCustodyV1::Completed(
                        completed_receipt.completed,
                    )),
                },
            ));
        };
        let (completion, identity) = unwrap_completed(completed_receipt.completed);
        let completion_occurrence = completion.occurrence_v1();
        let generation_is_current = self
            .dispatch
            .as_ref()
            .is_some_and(|dispatch| dispatch.validate_completed(identity, &completion).is_ok());
        if !generation_is_current {
            let completed = wrap_completed(completion, identity);
            attachment.state = quarantine_persistent_compute_completed_v1(
                &mut attachment.allocation.owner,
                completed_use,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
            );
            attachment.terminal_custody = Some(
                PersistentComputeTerminalNativeCustodyV1::Completed(completed),
            );
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return self.terminalize_persistent_compute_recycle_result_v1(Err(
                Gfx942PersistentComputeRecycleFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                    recovered: None,
                    retained: None,
                },
            ));
        }
        let result = self.finish_directional_persistent_fixed_dispatch_recycle_inner_v1(
            binding,
            attachment,
            completed_use,
            identity,
            completion_occurrence.expect("validated completion occurrence"),
            completion,
            Self::recycle_completion_batch_retaining,
            |completed| completed,
        );
        self.terminalize_persistent_compute_recycle_result_v1(result)
    }

    /// Detaches and restores all three owners after exact completion recycle.
    #[allow(clippy::result_large_err)]
    pub fn detach_recycled_three_binding_directional_persistent_fixed_dispatch_v1(
        &mut self,
        recycled_receipt: Gfx942RecycledThreeBindingPersistentComputeDispatchV1,
    ) -> Result<
        Gfx942ThreeBindingPersistentComputeCompletedV1,
        Gfx942ThreeBindingPersistentComputeDetachFailureV1,
    > {
        let binding = recycled_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(recycled_receipt),
            });
        }
        let valid = self
            .three_binding_persistent_compute_attachment_v1()
            .is_some_and(|attachment| attachment.binding == binding);
        if !valid {
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(recycled_receipt),
            });
        }
        let mut attachment = self
            .take_three_binding_persistent_compute_attachment_v1()
            .expect("validated three-binding recycled attachment");
        if self.terminal_poisoned
            || attachment
                .entries
                .iter()
                .any(|entry| !matches!(entry.state, PersistentComputeUseStateV1::Recycled(_)))
        {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Recycled(
                recycled_receipt.recycle,
            ));
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
            });
        }
        let release_preflight = self
            .completion_owner
            .ensure_releasable()
            .map_err(ComputeAqlQueueSessionErrorV1::from)
            .and_then(|()| {
                (self.detached_data_count == 0
                    && self.detached_dispatch_generation.is_none()
                    && self.detached_data_identities.is_empty()
                    && self.detached_next_insertion_index.is_none())
                .then_some(())
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent detach requires an empty detached-data ledger",
                ))
            });
        if let Err(error) = release_preflight {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Recycled(
                recycled_receipt.recycle,
            ));
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error,
                recovered: None,
            });
        }
        let (generation, data) = match self.detach_persistent_dispatch_data_retaining_control_v1() {
            Ok(returned) => returned,
            Err((error, data)) => {
                quarantine_persistent_compute_entries_v1(
                    attachment.entries.each_mut(),
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                );
                attachment.terminal_custody = Some(if data.is_empty() {
                    PersistentComputeTerminalNativeCustodyV1::Attached
                } else {
                    PersistentComputeTerminalNativeCustodyV1::Data(
                        PersistentComputeTerminalDataV1::from_vec(data),
                    )
                });
                self.set_three_binding_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                poison_process_global_after_dispatch_terminal_v1();
                return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                    error,
                    recovered: None,
                });
            }
        };
        let exact = generation != 0
            && recycled_receipt.recycle.packet_count() == 1
            && data.len() == 3
            && data.iter().zip(&attachment.entries).all(|(data, entry)| {
                entry.storage_identity.is_some_and(|identity| {
                    data.sdma_storage_identity()
                        == Gfx942SdmaBufferStorageIdentityV1::Device(identity)
                })
            });
        if !exact {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Data(
                PersistentComputeTerminalDataV1::from_vec(data),
            ));
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent detach returned substituted storage",
                ),
                recovered: None,
            });
        }
        let data: [Gfx942FixedDispatchDataV1; 3] = data
            .try_into()
            .unwrap_or_else(|_| unreachable!("validated three-binding data cardinality"));
        let fully_initialized = std::array::from_fn(|index| data[index].is_fully_initialized());
        let leases = data.map(|data| {
            let Gfx942SdmaBufferStorageV1::Device(lease) = data.into_sdma_storage() else {
                unreachable!("validated three-binding device storage")
            };
            lease
        });
        let restore_preflight =
            attachment
                .entries
                .iter()
                .zip(&leases)
                .try_for_each(|(entry, lease)| {
                    let PersistentComputeUseStateV1::Recycled(completed) = &entry.state else {
                        return Err(Gfx942PersistentUseErrorV1::WrongState);
                    };
                    entry
                        .allocation
                        .owner
                        .preflight_restore_local_native_from_compute(completed, lease)
                });
        if restore_preflight.is_err() {
            let [lease0, lease1, lease2] = leases;
            let [initialized0, initialized1, initialized2] = fully_initialized;
            let restore_data = |lease, initialized| {
                if initialized {
                    Gfx942FixedDispatchDataV1::initialized_after_dispatch(lease)
                } else {
                    Gfx942FixedDispatchDataV1::uninitialized(lease)
                }
            };
            let retained = [
                restore_data(lease0, initialized0),
                restore_data(lease1, initialized1),
                restore_data(lease2, initialized2),
            ];
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Data(
                PersistentComputeTerminalDataV1::from_three(retained),
            ));
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent native restore preflight",
                ),
                recovered: None,
            });
        }
        for (entry, lease) in attachment.entries.iter_mut().zip(leases) {
            let PersistentComputeUseStateV1::Recycled(completed) = &entry.state else {
                unreachable!("preflighted three-binding recycled state")
            };
            entry
                .allocation
                .owner
                .restore_local_native_from_compute(completed, lease)
                .unwrap_or_else(|_| unreachable!("preflighted three-binding native restore"));
        }
        let settle_preflight = attachment.entries.iter().try_for_each(|entry| {
            let PersistentComputeUseStateV1::Recycled(completed) = &entry.state else {
                return Err(Gfx942PersistentUseErrorV1::WrongState);
            };
            entry.allocation.owner.preflight_settle(completed)
        });
        if settle_preflight.is_err() {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Restored);
            self.set_three_binding_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
            return Err(Gfx942ThreeBindingPersistentComputeTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "three-binding persistent settlement preflight",
                ),
                recovered: None,
            });
        }
        let settle_entry = |mut entry: PersistentComputeAttachmentEntryV1, initialized| {
            let state =
                core::mem::replace(&mut entry.state, PersistentComputeUseStateV1::Quarantined);
            let PersistentComputeUseStateV1::Recycled(completed_use) = state else {
                unreachable!("preflighted three-binding recycled state")
            };
            let frontier = entry
                .allocation
                .owner
                .settle(completed_use)
                .unwrap_or_else(|_| unreachable!("preflighted three-binding settlement"));
            Gfx942PersistentComputeCompletedV1 {
                allocation: entry.allocation,
                frontier,
                effect: entry.effect,
                authenticated_sha256: (!entry.effect.writes())
                    .then_some(entry.authenticated_sha256)
                    .flatten(),
                fully_initialized: initialized,
            }
        };
        let [entry0, entry1, entry2] = attachment.entries;
        let [initialized0, initialized1, initialized2] = fully_initialized;
        let completed = [
            settle_entry(entry0, initialized0),
            settle_entry(entry1, initialized1),
            settle_entry(entry2, initialized2),
        ];
        self.detached_dispatch_generation = Some(generation);
        self.detached_data_count = 0;
        self.detached_data_identities.clear();
        self.detached_next_insertion_index = Some(0);
        Ok(Gfx942ThreeBindingPersistentComputeCompletedV1 { completed })
    }

    /// Detaches the recycled batch, restores the exact mapped HBM authority to
    /// its persistent owner, and settles the compute ledger use.
    #[allow(clippy::result_large_err)]
    pub fn detach_recycled_directional_persistent_fixed_dispatch_v1(
        &mut self,
        recycled_receipt: Gfx942RecycledPersistentComputeDispatchV1,
    ) -> Result<Gfx942PersistentComputeCompletedV1, Gfx942PersistentComputeDetachFailureV1> {
        let binding = recycled_receipt.binding;
        if binding.queue != self.key {
            return Err(Gfx942PersistentComputeDetachFailureV1 {
                error: if self.terminal_poisoned {
                    Gfx942DispatchBindingErrorV1::Poisoned
                } else {
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                }
                .into(),
                recovered: Some(recycled_receipt),
                retained: None,
            });
        }
        if self.terminal_poisoned {
            return match self
                .absorb_terminal_recycled_persistent_compute_v1(binding, recycled_receipt.recycle)
            {
                Ok(()) => Err(Gfx942PersistentComputeDetachFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: None,
                    retained: None,
                }),
                Err(recycle) => Err(Gfx942PersistentComputeDetachFailureV1 {
                    error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                    recovered: Some(Gfx942RecycledPersistentComputeDispatchV1 {
                        binding,
                        recycle,
                        thread_affinity: PhantomData,
                    }),
                    retained: None,
                }),
            };
        }
        let valid = self
            .single_persistent_compute_attachment_v1()
            .is_some_and(|attachment| attachment.binding == binding && binding.queue == self.key);
        if !valid {
            return Err(Gfx942PersistentComputeDetachFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: Some(recycled_receipt),
                retained: None,
            });
        }
        let mut attachment = self
            .take_single_persistent_compute_attachment_v1()
            .expect("validated persistent compute attachment");
        let state = core::mem::replace(
            &mut attachment.state,
            PersistentComputeUseStateV1::Quarantined,
        );
        let PersistentComputeUseStateV1::Recycled(completed_use) = state else {
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeDetachFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                recovered: None,
                retained: Some(PersistentComputeTerminalNativeCustodyV1::Recycled(
                    recycled_receipt.recycle,
                )),
            });
        };
        let release_preflight = self
            .completion_owner
            .ensure_releasable()
            .map_err(ComputeAqlQueueSessionErrorV1::from)
            .and_then(|()| {
                (self.detached_data_count == 0
                    && self.detached_dispatch_generation.is_none()
                    && self.detached_data_identities.is_empty()
                    && self.detached_next_insertion_index.is_none())
                .then_some(())
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute detach requires an empty detached-data ledger",
                ))
            });
        if let Err(error) = release_preflight {
            attachment.state = quarantine_persistent_compute_recycled_v1(
                &mut attachment.allocation.owner,
                completed_use,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Attached);
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeDetachFailureV1 {
                error,
                recovered: None,
                retained: None,
            });
        }
        let (generation, mut data) = match self
            .detach_persistent_dispatch_data_retaining_control_v1()
        {
            Ok(returned) => returned,
            Err((error, data)) => {
                attachment.state = quarantine_persistent_compute_recycled_v1(
                    &mut attachment.allocation.owner,
                    completed_use,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                );
                attachment.terminal_custody = Some(if data.is_empty() && self.dispatch.is_some() {
                    PersistentComputeTerminalNativeCustodyV1::Attached
                } else {
                    PersistentComputeTerminalNativeCustodyV1::Data(
                        PersistentComputeTerminalDataV1::from_vec(data),
                    )
                });
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                return Err(Gfx942PersistentComputeDetachFailureV1 {
                    error,
                    recovered: None,
                    retained: None,
                });
            }
        };
        let exact = generation != 0
            && recycled_receipt.recycle.packet_count() == 1
            && data.len() == 1
            && data[0].sdma_storage_identity()
                == Gfx942SdmaBufferStorageIdentityV1::Device(attachment.storage_identity);
        if !exact {
            attachment.state = quarantine_persistent_compute_recycled_v1(
                &mut attachment.allocation.owner,
                completed_use,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Data(
                PersistentComputeTerminalDataV1::from_vec(data),
            ));
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeDetachFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "persistent compute detach returned substituted storage",
                ),
                recovered: None,
                retained: None,
            });
        }
        let data = data.pop().expect("validated one detached data authority");
        let fully_initialized = data.is_fully_initialized();
        let Gfx942SdmaBufferStorageV1::Device(lease) = data.into_sdma_storage() else {
            unreachable!("validated device storage identity")
        };
        if let Err((_error, lease)) = attachment
            .allocation
            .owner
            .restore_local_native_from_compute(&completed_use, lease)
        {
            attachment.state = quarantine_persistent_compute_recycled_v1(
                &mut attachment.allocation.owner,
                completed_use,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Storage(
                Gfx942SdmaBufferStorageV1::Device(lease),
            ));
            self.set_single_persistent_compute_attachment_v1(attachment);
            self.poison_terminal();
            return Err(Gfx942PersistentComputeDetachFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("persistent compute native restore"),
                recovered: None,
                retained: None,
            });
        }
        let frontier = match attachment.allocation.owner.settle(completed_use) {
            Ok(frontier) => frontier,
            Err(failure) => {
                let (_, completed_use) = failure.into_parts();
                attachment.state = quarantine_persistent_compute_recycled_v1(
                    &mut attachment.allocation.owner,
                    completed_use,
                    Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
                );
                attachment.terminal_custody =
                    Some(PersistentComputeTerminalNativeCustodyV1::Restored);
                self.set_single_persistent_compute_attachment_v1(attachment);
                self.poison_terminal();
                return Err(Gfx942PersistentComputeDetachFailureV1 {
                    error: ComputeAqlQueueSessionErrorV1::Contract("persistent compute settlement"),
                    recovered: None,
                    retained: None,
                });
            }
        };
        self.detached_dispatch_generation = Some(generation);
        self.detached_data_count = 0;
        self.detached_data_identities.clear();
        self.detached_next_insertion_index = Some(0);
        Ok(Gfx942PersistentComputeCompletedV1 {
            allocation: attachment.allocation,
            frontier,
            effect: attachment.effect,
            authenticated_sha256: (!attachment.effect.writes())
                .then_some(attachment.authenticated_sha256)
                .flatten(),
            fully_initialized,
        })
    }

    pub fn detach_recycled_fixed_dispatch(
        &mut self,
    ) -> Result<Gfx942DetachedFixedDispatchV1, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        self.detach_recycled_fixed_dispatch_inner()
    }

    /// Consumes a detached persistent dispatch's immutable code/kernarg
    /// control without changing the queue's detached-generation ledger.
    pub fn release_retained_persistent_fixed_dispatch_control_v1(
        &mut self,
    ) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if !self.unpublished_dispatch.is_clear() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let Some(dispatch) = self.dispatch.as_ref() else {
            return Ok(false);
        };
        if !dispatch.persistent_data_is_detached_v1() {
            return Ok(false);
        }
        let Some(generation) = self.detached_dispatch_generation else {
            self.poison_terminal();
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "retained persistent control lost its detached generation",
            ));
        };
        if let Err(error) = dispatch.validate_detached_persistent_control_release_v1(generation) {
            self.poison_terminal();
            return Err(error.into());
        }
        let full_currentness = self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))
            .and_then(|engine| {
                engine
                    .backend
                    .session
                    .check_queue_currentness()
                    .map_err(Into::into)
            });
        if let Err(error) = full_currentness {
            self.poison_terminal();
            return Err(error);
        }
        let mut dispatch = Some(
            self.dispatch
                .take()
                .expect("validated detached persistent control remains retained"),
        );
        let envelope = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.with_live_queue_memory_model_custody(|memory| {
                dispatch
                    .take()
                    .expect("custody operation executes at most once")
                    .release_detached_persistent_control_v1(memory, generation)
            })
        }));
        let envelope = match envelope {
            Ok(envelope) => envelope,
            Err(payload) => {
                self.dispatch = dispatch;
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        };
        match envelope {
            Ok((Ok(()), Ok(()))) => Ok(true),
            Ok((Err(error), Ok(()))) => {
                self.poison_terminal();
                Err(error.into())
            }
            Ok((_, Err(error))) => {
                self.poison_terminal();
                Err(error)
            }
            Err(error) => {
                self.dispatch = dispatch;
                Err(error)
            }
        }
    }

    pub(super) fn detach_recycled_fixed_dispatch_inner(
        &mut self,
    ) -> Result<Gfx942DetachedFixedDispatchV1, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if !self.unpublished_dispatch.is_clear() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        if self.detached_data_count != 0
            || self.detached_dispatch_generation.is_some()
            || !self.detached_data_identities.is_empty()
            || self.detached_next_insertion_index.is_some()
        {
            self.poison_terminal();
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch-data ledger was not empty",
            ));
        }
        self.completion_owner.ensure_releasable()?;
        let dispatch = self
            .dispatch
            .take()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let returned = self.with_live_queue_memory_model(|memory| {
            dispatch
                .release_non_data_after_recycle(memory)
                .map_err(Into::into)
        });
        let returned = match returned {
            Ok(returned) => returned,
            Err(error) => {
                self.poison_terminal();
                return Err(error);
            }
        };
        let generation = returned.generation();
        let data = recover_fixed_dispatch_data(returned);
        if generation == 0 {
            self.poison_terminal();
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch generation was zero",
            ));
        }
        self.detached_data_count = data.len();
        self.detached_dispatch_generation = Some(generation);
        self.detached_data_identities = fixed_dispatch_storage_identities(&data);
        self.detached_next_insertion_index = None;
        Ok(Gfx942DetachedFixedDispatchV1 { generation, data })
    }

    /// Binds a new fixed batch to the same live native queue.
    ///
    /// The queue must have no attached batch. The complete detached data set is
    /// rebound, and its device-local subset is revalidated against the retained
    /// KFD session before the new owner is installed. Every mapped storage input
    /// and inspected program is retained even when no packet in this batch
    /// selects it. This does not publish.
    pub fn bind_fixed_dispatch<const N: usize>(
        &mut self,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; N],
        data: Vec<Gfx942FixedDispatchDataV1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.dispatch.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        if !(self.unpublished_dispatch.is_clear() && self.detached_dispatch_generation.is_some()
            || self.unpublished_dispatch.is_detached()
                && self.detached_dispatch_generation.is_none())
        {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let data_identities = fixed_dispatch_storage_identities(&data);
        if self.detached_data_identities.len() != self.detached_data_count {
            self.poison_terminal();
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch-data identity ledger cardinality",
            ));
        }
        if data.len() != self.detached_data_count {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: data.len().min(self.detached_data_count),
                detail: "detached dispatch-data cardinality",
            }
            .into());
        }
        if let Some(index) =
            first_ordered_identity_mismatch(&self.detached_data_identities, &data_identities)
        {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index,
                detail: "detached rebind storage identity",
            }
            .into());
        }
        let preflight = self
            .completion_owner
            .ensure_releasable()
            .map_err(ComputeAqlQueueSessionErrorV1::from)
            .and_then(|()| {
                validate_fixed_batch_ring::<N>(self.observation.ring_bytes).map_err(Into::into)
            });
        if let Err(error) = preflight {
            if self.unpublished_dispatch.is_detached() {
                self.poison_terminal();
            }
            return Err(error);
        }
        if self.unpublished_dispatch.is_detached() {
            return self.bind_after_pristine_abort_v1(programs, packets, data);
        }
        let predecessor_generation = self
            .detached_dispatch_generation
            .expect("checked detached dispatch generation");
        let prepared = self.with_live_queue_memory_model(|memory| {
            prepare_public_fixed_dispatch_resources_after_detach(
                memory,
                programs,
                packets,
                data,
                predecessor_generation,
            )
            .map_err(Into::into)
        });
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.poison_terminal();
                return Err(error);
            }
        };
        let validation = {
            let device_authorities = prepared.device_authorities_inline_v1();
            self.engine
                .as_mut()
                .expect("checked queue engine")
                .backend
                .session
                .validate_live_queue_dispatch_memory(&device_authorities)
        };
        if let Err(error) = validation {
            self.poison_terminal();
            return Err(error.into());
        }
        self.dispatch = Some(prepared);
        self.detached_data_count = 0;
        self.detached_dispatch_generation = None;
        self.detached_data_identities.clear();
        self.detached_next_insertion_index = None;
        Ok(())
    }

    /// Allocates and maps one uninitialized device-local extent while no fixed
    /// batch is attached.
    pub fn allocate_uninitialized_fixed_dispatch_data(
        &mut self,
        requested_bytes: u64,
        alignment: u64,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        let result = self.with_live_queue_memory_model(|memory| {
            memory
                .allocate_gfx942_device_memory(requested_bytes, alignment)
                .and_then(|lease| memory.map_gfx942_device_memory(lease))
                .map_err(Into::into)
        });
        match result {
            Ok(lease) => {
                let data = Gfx942FixedDispatchDataV1::uninitialized(lease);
                self.record_new_detached_data(&data);
                Ok(data)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Allocates, writes, verifies, CPU-unmaps, and GPU-maps one fully
    /// initialized device-local extent while no fixed batch is attached.
    pub fn initialize_fixed_dispatch_data(
        &mut self,
        bytes: Box<[u8]>,
        alignment: u64,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        let result = self.with_live_queue_memory_model(|memory| {
            memory
                .initialize_gfx942_device_memory(bytes, alignment, content)
                .map_err(Into::into)
        });
        match result {
            Ok(memory) => {
                let data = Gfx942FixedDispatchDataV1::initialized(memory);
                self.record_new_detached_data(&data);
                Ok(data)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Allocates and inserts one initialized device-local extent at an exact
    /// detached data ordinal without replacing an existing allocation.
    ///
    /// The insertion ordinal is validated before allocation. It is intended
    /// for service ledgers that keep device-local entries before coherent host
    /// entries while changing the detached allocation cardinality.
    pub fn insert_initialized_fixed_dispatch_data(
        &mut self,
        data_index: usize,
        bytes: Box<[u8]>,
        alignment: u64,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        self.require_new_detached_data_index(data_index)?;
        let result = self.with_live_queue_memory_model(|memory| {
            memory
                .initialize_gfx942_device_memory(bytes, alignment, content)
                .map_err(Into::into)
        });
        match result {
            Ok(memory) => {
                let data = Gfx942FixedDispatchDataV1::initialized(memory);
                self.record_new_detached_data_at(&data, data_index);
                Ok(data)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Validates an exact detached-data insertion without allocating, mapping,
    /// or changing queue state.
    ///
    /// Service layers can use this before mutating their own allocation ledger,
    /// so a full lower data roster or an invalid ordinal remains a retry-safe
    /// rejection with unchanged custody.
    pub fn preflight_fixed_dispatch_data_insertion(
        &self,
        data_index: usize,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        self.require_new_detached_data_index(data_index)
    }

    /// Overwrites one initialized coherent extent retained from the immediately
    /// preceding completed dispatch while the queue is unbound.
    ///
    /// The exact detached storage identity and bounds are checked before the
    /// mapped bytes are changed. Native handles and GPU addresses remain private.
    pub fn overwrite_detached_initialized_host_visible_fixed_dispatch_data(
        &mut self,
        data_index: usize,
        data: &mut Gfx942FixedDispatchDataV1,
        offset: u64,
        source: &[u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        let expected_identity = *self.detached_data_identities.get(data_index).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidData {
                index: data_index,
                detail: "detached overwrite ordinal",
            },
        )?;
        if data.storage_identity() != expected_identity {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: data_index,
                detail: "detached overwrite storage identity",
            }
            .into());
        }
        let end = offset
            .checked_add(u64::try_from(source.len()).map_err(|_| {
                Gfx942DispatchBindingErrorV1::InvalidData {
                    index: data_index,
                    detail: "detached overwrite source length",
                }
            })?)
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidData {
                index: data_index,
                detail: "detached overwrite range overflow",
            })?;
        if source.is_empty() || end > data.layout().requested_bytes() {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: data_index,
                detail: "detached overwrite range",
            }
            .into());
        }
        let token = data.initialized_host_visible_token_mut().ok_or(
            Gfx942DispatchBindingErrorV1::InvalidData {
                index: data_index,
                detail: "detached overwrite requires initialized coherent storage",
            },
        )?;
        let result = self.with_live_queue_memory_model(|memory| {
            memory
                .overwrite_mapped_host_visible_subrange(token, offset, source)
                .map_err(Into::into)
        });
        if let Err(error) = result {
            self.poison_terminal();
            return Err(error);
        }
        Ok(())
    }

    /// Allocates, initializes, and inserts one coherent host-visible extent at
    /// an exact detached data ordinal.
    pub fn insert_initialized_host_visible_fixed_dispatch_data(
        &mut self,
        data_index: usize,
        bytes: Box<[u8]>,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(data_index, &bytes)
    }

    /// Synchronously copies a complete borrowed extent at an exact detached
    /// ordinal. The source borrow does not escape; this does not publish work.
    pub fn insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(
        &mut self,
        data_index: usize,
        bytes: &[u8],
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        self.require_new_detached_data_index(data_index)?;
        let result = self.with_live_queue_memory_model(|memory| {
            memory
                .initialize_host_visible_coherent_from_slice_v1(bytes)
                .map_err(Into::into)
        });
        match result {
            Ok(memory) => {
                let data = Gfx942FixedDispatchDataV1::host_visible_initialized(memory);
                self.record_new_detached_data_at(&data, data_index);
                Ok(data)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Allocates and initializes one coherent host-visible extent at the exact
    /// ordinal vacated by the immediately preceding detached release.
    pub fn initialize_host_visible_fixed_dispatch_data(
        &mut self,
        bytes: Box<[u8]>,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.initialize_host_visible_fixed_dispatch_data_from_slice_v1(&bytes)
    }

    /// Synchronously initializes the exact vacated detached ordinal from a
    /// borrowed extent, preserving the existing release/replacement ledger.
    pub fn initialize_host_visible_fixed_dispatch_data_from_slice_v1(
        &mut self,
        bytes: &[u8],
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        if self.detached_next_insertion_index.is_none() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let result = self.with_live_queue_memory_model(|memory| {
            memory
                .initialize_host_visible_coherent_from_slice_v1(bytes)
                .map_err(Into::into)
        });
        match result {
            Ok(memory) => {
                let data = Gfx942FixedDispatchDataV1::host_visible_initialized(memory);
                self.record_new_detached_data(&data);
                Ok(data)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Allocates, maps, and inserts one uninitialized coherent host-visible
    /// extent at an exact detached data ordinal.
    pub fn insert_host_visible_fixed_dispatch_data(
        &mut self,
        data_index: usize,
        requested_bytes: usize,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        self.require_new_detached_data_index(data_index)?;
        let result = self.with_live_queue_memory_model(|memory| {
            let allocation = memory.allocate_host_visible_coherent(requested_bytes)?;
            memory.map_to_gpu(allocation).map_err(Into::into)
        });
        match result {
            Ok(memory) => {
                let data = Gfx942FixedDispatchDataV1::host_visible_uninitialized(memory);
                self.record_new_detached_data_at(&data, data_index);
                Ok(data)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Allocates and maps one uninitialized coherent host-visible extent at the
    /// exact ordinal vacated by the immediately preceding detached release.
    pub fn allocate_host_visible_fixed_dispatch_data(
        &mut self,
        requested_bytes: usize,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        self.require_detached_allocation_capacity()?;
        if self.detached_next_insertion_index.is_none() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let result = self.with_live_queue_memory_model(|memory| {
            let allocation = memory.allocate_host_visible_coherent(requested_bytes)?;
            memory.map_to_gpu(allocation).map_err(Into::into)
        });
        match result {
            Ok(memory) => {
                let data = Gfx942FixedDispatchDataV1::host_visible_uninitialized(memory);
                self.record_new_detached_data(&data);
                Ok(data)
            }
            Err(error) => {
                self.poison_terminal();
                Err(error)
            }
        }
    }

    /// Unmaps and releases detached fixed-dispatch storage exactly once.
    pub fn release_detached_fixed_dispatch_data(
        &mut self,
        data: Gfx942FixedDispatchDataV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()?;
        let identity = data.storage_identity();
        let mut matching = self
            .detached_data_identities
            .iter()
            .enumerate()
            .filter(|(_, retained)| **retained == identity);
        let Some((identity_index, _)) = matching.next() else {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: self.detached_data_count,
                detail: "detached release storage identity",
            }
            .into());
        };
        if matching.next().is_some() {
            self.poison_terminal();
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "duplicate detached storage identity",
            ));
        }
        let result = self.with_live_queue_memory_model(|memory| {
            memory.release_fixed_dispatch_data(data).map_err(Into::into)
        });
        if let Err(error) = result {
            self.poison_terminal();
            return Err(error);
        }
        self.detached_data_count = self.detached_data_count.checked_sub(1).ok_or(
            ComputeAqlQueueSessionErrorV1::Contract("detached dispatch-data ledger underflow"),
        )?;
        self.detached_data_identities.remove(identity_index);
        self.detached_next_insertion_index = Some(identity_index);
        Ok(())
    }

    pub(super) fn require_unbound_fixed_dispatch(
        &self,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.dispatch.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        if !(self.unpublished_dispatch.is_clear() && self.detached_dispatch_generation.is_some()
            || self.unpublished_dispatch.is_detached()
                && self.detached_dispatch_generation.is_none())
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        if self.detached_data_count > super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch-data ledger bound",
            ));
        }
        if self.detached_data_identities.len() != self.detached_data_count
            || self
                .detached_next_insertion_index
                .is_some_and(|index| index > self.detached_data_identities.len())
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "detached dispatch-data identity ledger",
            ));
        }
        self.completion_owner.ensure_releasable()?;
        Ok(())
    }

    pub(super) fn require_new_detached_data_index(
        &self,
        data_index: usize,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        validate_new_detached_data_index(self.detached_data_count, data_index).map_err(Into::into)
    }

    pub(super) fn record_new_detached_data(&mut self, data: &Gfx942FixedDispatchDataV1) {
        insert_detached_identity(
            &mut self.detached_data_identities,
            &mut self.detached_next_insertion_index,
            data.storage_identity(),
        );
        self.detached_data_count += 1;
    }

    pub(super) fn record_new_detached_data_at(
        &mut self,
        data: &Gfx942FixedDispatchDataV1,
        data_index: usize,
    ) {
        insert_detached_identity_at(
            &mut self.detached_data_identities,
            &mut self.detached_next_insertion_index,
            data.storage_identity(),
            data_index,
        );
        self.detached_data_count += 1;
    }

    pub(super) fn require_detached_allocation_capacity(
        &self,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.detached_data_count >= super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 {
            return Err(Gfx942DispatchBindingErrorV1::DataLeaseCount {
                requested: self.detached_data_count + 1,
                maximum: super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1,
            }
            .into());
        }
        Ok(())
    }

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
            Ok(Ok(batch)) => Ok(batch),
            Ok(Err(FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error))) => {
                Err(Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error))
            }
            Ok(Err(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error))) => {
                Err(Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error))
            }
            Ok(Err(FixedDispatchSubmissionFailureV1::Terminal(error))) => {
                Err(Gfx942FixedDispatchSubmissionFailureV1::Terminal(error))
            }
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
    ) -> Result<Gfx942ComputeDependencySourceBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.submit_fixed_dispatch_with_dependency_events_operation_v1::<N>(lane)
        }));
        match operation {
            Ok(result) => result,
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    pub(super) fn submit_fixed_dispatch_with_dependency_events_operation_v1<const N: usize>(
        &mut self,
        lane: ComputeAqlQueueLaneV1,
    ) -> Result<Gfx942ComputeDependencySourceBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        if N == 0 || N > super::completion::GFX942_MAX_COMPUTE_DEPENDENCY_READERS_V1 {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "dependency source packet count must be 1 through 8192",
            ));
        }
        let acceptance = match self.dependency_owner.reserve_acceptance_epoch() {
            Ok(acceptance) => acceptance,
            Err(error @ ComputeDependencyTargetUseErrorV1::AcceptanceEpochExhausted) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(map_dependency_target_use_error_v1(error));
            }
            Err(error) => return Err(map_dependency_target_use_error_v1(error)),
        };
        let binding = self
            .dispatch
            .as_mut()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)
            .and_then(|dispatch| dispatch.bind_templates::<N>(self.key));
        let (templates, identity) = self
            .classify_fixed_dispatch_binding(FixedDispatchBindingModeV1::Ordinary, binding)
            .map_err(FixedDispatchSubmissionFailureV1::into_error)?;
        let completion = self.submit_with_dependency_events_classified_v1(
            templates,
            acceptance.session_occurrence(),
            acceptance.epoch(),
        );
        let (completion, events) = match completion {
            Ok(published) => {
                if let Err(error) = self
                    .dispatch
                    .as_mut()
                    .expect("dependency source dispatch owner remains retained")
                    .mark_published(identity, &published.0)
                {
                    self.poison_terminal();
                    permanently_poison_process_global_kfd_runtime_gate_v1();
                    return Err(error.into());
                }
                published
            }
            Err(FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error)) => {
                if self
                    .dispatch
                    .as_mut()
                    .expect("dependency source dispatch owner remains retained")
                    .cancel_binding(identity)
                    .is_err()
                {
                    self.poison_terminal();
                    permanently_poison_process_global_kfd_runtime_gate_v1();
                    return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
                }
                return Err(error);
            }
            Err(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error))
            | Err(FixedDispatchSubmissionFailureV1::Terminal(error)) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                return Err(error);
            }
        };
        Ok(Gfx942ComputeDependencySourceBatchV1 {
            batch: wrap_published(completion, identity),
            events: events
                .into_iter()
                .map(|event| Gfx942ComputeDependencyEventV1 { lane, event })
                .collect(),
        })
    }

    pub(super) fn submit_with_dependency_events_classified_v1<const N: usize>(
        &mut self,
        templates: [CompletionPacketTemplateV1; N],
        session_occurrence: u64,
        source_acceptance_epoch: u64,
    ) -> Result<
        (
            Gfx942CompletionBatchV1<N>,
            Vec<super::completion::Gfx942ComputeEventOccurrenceV1>,
        ),
        FixedDispatchSubmissionFailureV1,
    > {
        let bound = match self.completion_owner.bind_batch(templates) {
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
        match self.submit_prepared_batch_classified(packets) {
            Ok(last_packet_id) => {
                let batch = self
                    .completion_owner
                    .mark_published_retaining(retention, last_packet_id)
                    .map_err(|(error, _retention)| {
                        FixedDispatchSubmissionFailureV1::Terminal(error.into())
                    })?;
                let events = self
                    .completion_owner
                    .bind_dependency_event_batch_v1(events, &batch)
                    .map_err(|(error, _events)| {
                        FixedDispatchSubmissionFailureV1::Terminal(error.into())
                    })?;
                Ok((batch, events))
            }
            Err(NativeAqlSubmissionFailureV1::RetryableBeforeSideEffect(error)) => {
                if self
                    .completion_owner
                    .release_dependency_event_batch_v1(events)
                    .is_err()
                    || self.completion_owner.cancel_bound(retention).is_err()
                {
                    return Err(FixedDispatchSubmissionFailureV1::Terminal(
                        Gfx942CompletionErrorV1::StaleEventOccurrence.into(),
                    ));
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
        let completion = self.submit_with_completions_classified(templates);
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

    #[cfg(test)]
    pub(super) fn submit_fixed_dispatch_inner_classified_with_test_owner(
        &mut self,
        owner: &mut super::dispatch_binding::TestOnlyDispatchGenerationOwnerV1,
        template: impl FnOnce(u64) -> CompletionPacketTemplateV1,
        native_submit: impl FnOnce(
            &mut Self,
            AqlPreparedKernelDispatchBatchV2<1>,
        ) -> Result<u64, NativeAqlSubmissionFailureV1>,
    ) -> Result<Gfx942DispatchBatchV1<1>, FixedDispatchSubmissionFailureV1> {
        let generation = match owner
            .bind_one()
            .map_err(|error| FixedDispatchSubmissionFailureV1::Terminal(error.into()))
        {
            Ok(generation) => generation,
            Err(error) => {
                return self.terminalize_fixed_dispatch_submission_result_v1(Err(error));
            }
        };
        let identity = DispatchEpochIdentityV1::for_test(self.key, generation);
        let completion =
            self.submit_with_completions_classified_using([template(generation)], native_submit);
        let result = finish_fixed_dispatch_submission(identity, completion, |identity| {
            owner.cancel_binding(identity.dispatch_generation())
        });
        self.terminalize_fixed_dispatch_submission_result_v1(result)
    }

    #[cfg(test)]
    pub(super) fn submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
        &mut self,
        owner: &mut super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1,
        template: impl FnOnce(u64) -> CompletionPacketTemplateV1,
        native_submit: impl FnOnce(
            &mut Self,
            AqlPreparedKernelDispatchBatchV2<1>,
        ) -> Result<u64, NativeAqlSubmissionFailureV1>,
    ) -> Result<Gfx942DispatchBatchV1<1>, FixedDispatchSubmissionFailureV1> {
        if self.terminal_poisoned {
            return Err(FixedDispatchSubmissionFailureV1::Terminal(
                Gfx942DispatchBindingErrorV1::Poisoned.into(),
            ));
        }
        let template = template(owner.next_generation());
        let identity = match owner
            .reserve_one(self.key, template)
            .map_err(|error| FixedDispatchSubmissionFailureV1::Terminal(error.into()))
        {
            Ok(identity) => identity,
            Err(error) => {
                return self.terminalize_fixed_dispatch_submission_result_v1(Err(error));
            }
        };
        let completion = self.submit_with_completions_classified_using([template], native_submit);
        let completion = match completion {
            Ok(completion) => {
                if let Err(error) = owner.mark_published(identity, &completion) {
                    Err(FixedDispatchSubmissionFailureV1::Terminal(error.into()))
                } else {
                    Ok(completion)
                }
            }
            Err(error) => Err(error),
        };
        let result = finish_fixed_dispatch_submission(identity, completion, |identity| {
            owner.cancel(identity)
        });
        self.terminalize_fixed_dispatch_submission_result_v1(result)
    }

    #[cfg(test)]
    pub(super) fn complete_fixed_dispatch_with_multi_inflight_test_owner(
        &mut self,
        owner: &mut super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1,
        batch: Gfx942DispatchBatchV1<1>,
    ) -> Result<Gfx942CompletedDispatchBatchV1<1>, Gfx942DispatchBindingErrorV1> {
        let (completion, identity) = unwrap_published(batch);
        let occurrence = owner.validate_published(identity, &completion)?;
        let completed = self
            .completion_owner
            .complete_one_without_native_for_test(completion);
        owner.mark_completed(identity, occurrence)?;
        Ok(wrap_completed(completed, identity))
    }

    #[cfg(test)]
    pub(super) fn recycle_fixed_dispatch_with_multi_inflight_test_owner(
        &mut self,
        owner: &mut super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1,
        completed: Gfx942CompletedDispatchBatchV1<1>,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942DispatchBindingErrorV1> {
        let (completion, identity) = unwrap_completed(completed);
        let occurrence = owner.validate_completed(identity, &completion)?;
        let recycled = self
            .completion_owner
            .recycle_one_without_native_for_test(completion);
        owner.mark_recycled(identity, occurrence)?;
        Ok(recycled)
    }

    pub(super) fn terminalize_fixed_dispatch_submission_result_v1<T>(
        &mut self,
        result: Result<T, FixedDispatchSubmissionFailureV1>,
    ) -> Result<T, FixedDispatchSubmissionFailureV1> {
        if matches!(&result, Err(FixedDispatchSubmissionFailureV1::Terminal(_))) {
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
        }
        result
    }

    pub(super) fn terminalize_fixed_dispatch_observation_result_v1<T>(
        &mut self,
        result: Result<T, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<T, ComputeAqlQueueSessionErrorV1> {
        if result.is_err() {
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
        }
        result
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn terminalize_fixed_dispatch_recycle_result_v1<const N: usize>(
        &mut self,
        result: Result<
            Gfx942CompletionRecycleObservationV1,
            Gfx942FixedDispatchRecycleFailureV1<N>,
        >,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942FixedDispatchRecycleFailureV1<N>> {
        if result
            .as_ref()
            .is_err_and(|failure| failure.retryable_completed.is_none())
        {
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
        }
        result
    }

    pub(super) fn classify_fixed_dispatch_binding<T>(
        &mut self,
        mode: FixedDispatchBindingModeV1,
        binding: Result<T, Gfx942DispatchBindingErrorV1>,
    ) -> Result<T, FixedDispatchSubmissionFailureV1> {
        binding.map_err(|error| {
            let error = error.into();
            match mode {
                FixedDispatchBindingModeV1::Ordinary => {
                    FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error)
                }
                FixedDispatchBindingModeV1::ExactPersistentAttachment => {
                    self.poison_terminal();
                    FixedDispatchSubmissionFailureV1::Terminal(error)
                }
            }
        })
    }

    /// Polls every packet signal once and returns linear pending or completed custody.
    pub fn poll_fixed_dispatch<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
    ) -> Result<Gfx942DispatchPollV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.poll_fixed_dispatch_with_progress_inner(batch)
        }));
        let poll = match operation {
            Ok(result) => self.terminalize_fixed_dispatch_observation_result_v1(result)?,
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        };
        match poll {
            Gfx942DispatchPollWithProgressV1::Pending { batch, .. } => {
                Ok(Gfx942DispatchPollV1::Pending(batch))
            }
            Gfx942DispatchPollWithProgressV1::Ready { completed, .. } => {
                Ok(Gfx942DispatchPollV1::Ready(completed))
            }
        }
    }

    /// Polls every packet signal once and returns custody plus same-scan progress.
    pub fn poll_fixed_dispatch_with_progress<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
    ) -> Result<Gfx942DispatchPollWithProgressV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.poll_fixed_dispatch_with_progress_inner(batch)
        }));
        match operation {
            Ok(result) => self.terminalize_fixed_dispatch_observation_result_v1(result),
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    pub(super) fn poll_fixed_dispatch_with_progress_inner<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
    ) -> Result<Gfx942DispatchPollWithProgressV1<N>, ComputeAqlQueueSessionErrorV1> {
        let (completion, identity) = unwrap_published(batch);
        if self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .validate_published(identity, &completion)
            .is_err()
        {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
        }
        match self.poll_completion_batch_with_progress(completion) {
            Ok(poll) => {
                if let Gfx942CompletionPollWithProgressV1::Ready { completed, .. } = &poll
                    && self
                        .dispatch
                        .as_mut()
                        .expect("dispatch owner retained")
                        .mark_completed(identity, completed)
                        .is_err()
                {
                    self.poison_terminal();
                    return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
                }
                Ok(wrap_poll_with_progress(poll, identity))
            }
            Err(error) => {
                if let Some(dispatch) = self.dispatch.as_mut() {
                    dispatch.poison();
                }
                Err(error)
            }
        }
    }

    /// Performs a bounded wait for every signal in the exact published batch.
    pub fn wait_fixed_dispatch<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
        polls: u32,
    ) -> Result<Gfx942CompletedDispatchBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.wait_fixed_dispatch_inner(batch, polls)
        }));
        match operation {
            Ok(result) => self.terminalize_fixed_dispatch_observation_result_v1(result),
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    pub(super) fn wait_fixed_dispatch_inner<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
        polls: u32,
    ) -> Result<Gfx942CompletedDispatchBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        let (completion, identity) = unwrap_published(batch);
        if self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .validate_published(identity, &completion)
            .is_err()
        {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
        }
        match self.wait_completion_batch(completion, polls) {
            Ok(completion) => {
                if self
                    .dispatch
                    .as_mut()
                    .expect("dispatch owner retained")
                    .mark_completed(identity, &completion)
                    .is_err()
                {
                    self.poison_terminal();
                    return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
                }
                Ok(wrap_completed(completion, identity))
            }
            Err(error) => {
                if let Some(dispatch) = self.dispatch.as_mut() {
                    dispatch.poison();
                }
                Err(error)
            }
        }
    }

    /// Waits for the exact published batch until a monotonic relative deadline.
    ///
    /// This is the preferred blocking API. It performs a short latency spin,
    /// then yields and sleeps with bounded backoff. The poll-count method is
    /// retained for compatibility with callers that require an observation
    /// budget rather than a time budget.
    pub fn wait_fixed_dispatch_for<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
        timeout_milliseconds: u32,
    ) -> Result<Gfx942CompletedDispatchBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.wait_fixed_dispatch_for_inner(batch, timeout_milliseconds)
        }));
        match operation {
            Ok(result) => self.terminalize_fixed_dispatch_observation_result_v1(result),
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    pub(super) fn wait_fixed_dispatch_for_inner<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
        timeout_milliseconds: u32,
    ) -> Result<Gfx942CompletedDispatchBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        let (completion, identity) = unwrap_published(batch);
        if self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .validate_published(identity, &completion)
            .is_err()
        {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
        }
        let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_milliseconds));
        match self.wait_completion_batch_until(completion, deadline) {
            Ok(completion) => {
                if self
                    .dispatch
                    .as_mut()
                    .expect("dispatch owner retained")
                    .mark_completed(identity, &completion)
                    .is_err()
                {
                    self.poison_terminal();
                    return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
                }
                Ok(wrap_completed(completion, identity))
            }
            Err(error) => {
                if let Some(dispatch) = self.dispatch.as_mut() {
                    dispatch.poison();
                }
                Err(error)
            }
        }
    }

    /// Recycles all completed signal slots and returns the queue to prepared state.
    #[allow(clippy::result_large_err)]
    pub fn recycle_fixed_dispatch<const N: usize>(
        &mut self,
        completed: Gfx942CompletedDispatchBatchV1<N>,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942FixedDispatchRecycleFailureV1<N>> {
        if self.terminal_poisoned {
            return Err(Gfx942FixedDispatchRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                retryable_completed: None,
            });
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942FixedDispatchRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                retryable_completed: Some(completed),
            });
        }
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.recycle_fixed_dispatch_inner(completed)
        }));
        match operation {
            Ok(result) => self.terminalize_fixed_dispatch_recycle_result_v1(result),
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn recycle_fixed_dispatch_inner<const N: usize>(
        &mut self,
        completed: Gfx942CompletedDispatchBatchV1<N>,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942FixedDispatchRecycleFailureV1<N>> {
        let (completion, identity) = unwrap_completed(completed);
        let completion_occurrence = completion.occurrence_v1();
        let completion_occurrence = match completion_occurrence {
            Ok(completion_occurrence) => completion_occurrence,
            Err(error) => {
                self.poison_terminal();
                return Err(Gfx942FixedDispatchRecycleFailureV1 {
                    error: error.into(),
                    retryable_completed: None,
                });
            }
        };
        if self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)
            .and_then(|dispatch| dispatch.validate_completed(identity, &completion))
            .is_err()
        {
            self.poison_terminal();
            return Err(Gfx942FixedDispatchRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                retryable_completed: None,
            });
        }
        let observation = match self.recycle_completion_batch_retaining(completion) {
            Ok(observation) => observation,
            Err((error, completion)) => {
                if matches!(
                    error,
                    ComputeAqlQueueSessionErrorV1::Completion(
                        Gfx942CompletionErrorV1::SignalPinned { .. }
                    )
                ) {
                    return Err(Gfx942FixedDispatchRecycleFailureV1 {
                        error,
                        retryable_completed: Some(wrap_completed(completion, identity)),
                    });
                }
                if let Some(dispatch) = self.dispatch.as_mut() {
                    dispatch.poison();
                }
                return Err(Gfx942FixedDispatchRecycleFailureV1 {
                    error,
                    retryable_completed: None,
                });
            }
        };
        if self
            .dispatch
            .as_mut()
            .expect("dispatch owner retained")
            .mark_recycled_occurrence(identity, completion_occurrence)
            .is_err()
        {
            self.poison_terminal();
            return Err(Gfx942FixedDispatchRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                retryable_completed: None,
            });
        }
        Ok(observation)
    }

    /// Returns the exact dispatch generation only while its completion signals
    /// have been observed and recycled and the same batch remains attached.
    pub fn recycled_fixed_dispatch_generation(&self) -> Result<u64, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        self.dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .ensure_returnable()
            .map_err(Into::into)
    }

    /// Copies one inspected writable subrange from coherent host-visible data.
    ///
    /// The exact attached dispatch must have completed and recycled. Device-local
    /// storage, read-only or unwritten bytes, stale generations, invalid bounds,
    /// and requests intersecting more than one admitted writable range fail
    /// before any mapped bytes are exposed.
    pub fn read_recycled_fixed_dispatch_data(
        &mut self,
        request: Gfx942CompletedDispatchReadRequestV1,
    ) -> Result<Gfx942CompletedDispatchReadbackV1, ComputeAqlQueueSessionErrorV1> {
        admit_generic_recycled_dispatch_access(
            self.terminal_poisoned,
            self.has_any_persistent_compute_attachment_v1(),
            GenericRecycledDispatchAccessV1::Read,
        )?;
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let dispatch = self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let memory = &mut self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session;
        let result = dispatch.read_completed_host_visible(memory, request);
        if matches!(result, Err(Gfx942DispatchBindingErrorV1::Memory(_))) {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }

    /// Copies one inspected writable coherent subrange into caller-owned bytes.
    ///
    /// This has the same generation, effect, kind, and bounds checks as
    /// [`Self::read_recycled_fixed_dispatch_data`] but avoids an intermediate
    /// owned readback allocation when the caller already owns the destination.
    pub fn read_recycled_fixed_dispatch_data_into(
        &mut self,
        request: Gfx942CompletedDispatchReadRequestV1,
        destination: &mut [u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        admit_generic_recycled_dispatch_access(
            self.terminal_poisoned,
            self.has_any_persistent_compute_attachment_v1(),
            GenericRecycledDispatchAccessV1::ReadInto,
        )?;
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let dispatch = self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let memory = &mut self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session;
        let result = dispatch.read_completed_host_visible_into(memory, request, destination);
        if matches!(result, Err(Gfx942DispatchBindingErrorV1::Memory(_))) {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }

    /// Copies one exact admitted enclosing snapshot from coherent host-visible data.
    ///
    /// The exact attached dispatch must have completed and recycled. Admission
    /// requires a retained fully initialized range strictly enclosing one
    /// isolated inspected writable binding. Subranges, stale generations,
    /// device-local storage, and undeclared ranges fail before bytes are exposed.
    pub fn read_recycled_fixed_dispatch_snapshot(
        &mut self,
        request: Gfx942CompletedDispatchSnapshotRequestV1,
    ) -> Result<Gfx942CompletedDispatchReadbackV1, ComputeAqlQueueSessionErrorV1> {
        admit_generic_recycled_dispatch_access(
            self.terminal_poisoned,
            self.has_any_persistent_compute_attachment_v1(),
            GenericRecycledDispatchAccessV1::Snapshot,
        )?;
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let dispatch = self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let memory = &mut self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session;
        let result = dispatch.read_completed_host_visible_snapshot(memory, request);
        if matches!(result, Err(Gfx942DispatchBindingErrorV1::Memory(_))) {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }

    /// Overwrites one initialized coherent range while the attached dispatch
    /// is exactly completed, recycled, and ready for another generation.
    pub fn overwrite_recycled_fixed_dispatch_host_data(
        &mut self,
        request: Gfx942RecycledDispatchWriteRequestV1,
        source: &[u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        admit_generic_recycled_dispatch_access(
            self.terminal_poisoned,
            self.has_any_persistent_compute_attachment_v1(),
            GenericRecycledDispatchAccessV1::Overwrite,
        )?;
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        let dispatch = self
            .dispatch
            .as_mut()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let memory = &mut self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session;
        let result = dispatch.overwrite_recycled_host_visible(memory, request, source);
        if matches!(result, Err(Gfx942DispatchBindingErrorV1::Memory(_))) {
            self.poison_terminal();
        }
        result.map_err(Into::into)
    }
}
