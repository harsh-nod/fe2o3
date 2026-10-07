//! Persistent compute attachment and terminal retention.

use super::*;

impl ComputeAqlQueueSessionV1 {
    pub(in super::super) fn validate_persistent_bind_preparation_v1<const N: usize>(
        &mut self,
        preparation: &FixedDispatchPreparationCustodyV1<N>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        // The completed owner remains rooted until full live-memory validation succeeds.
        let authorities = preparation.completed()?.device_authorities_inline_v1();
        self.engine
            .as_mut()
            .expect("checked queue engine")
            .backend
            .session
            .validate_live_queue_dispatch_memory(&authorities)
            .map_err(Into::into)
    }

    pub(in super::super) const fn has_any_persistent_compute_attachment_v1(&self) -> bool {
        self.persistent_compute.is_some()
    }

    pub(in super::super) fn single_persistent_compute_attachment_v1(
        &self,
    ) -> Option<&BoundedPersistentComputeAttachmentV1> {
        self.persistent_compute
            .as_ref()
            .filter(|attachment| attachment.single_entry().is_some())
    }

    #[cfg(test)]
    pub(in super::super) fn single_persistent_compute_attachment_mut_v1(
        &mut self,
    ) -> Option<&mut BoundedPersistentComputeAttachmentV1> {
        let attachment = self.persistent_compute.as_mut()?;
        attachment.single_entry_mut()?;
        Some(attachment)
    }

    pub(in super::super) fn take_single_persistent_compute_attachment_v1(
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

    pub(in super::super) fn set_single_persistent_compute_attachment_v1(
        &mut self,
        attachment: PersistentComputeAttachmentV1,
    ) {
        debug_assert!(self.persistent_compute.is_none());
        self.persistent_compute = Some(BoundedPersistentComputeAttachmentV1::from_single(
            attachment,
        ));
    }

    pub(in super::super) fn three_binding_persistent_compute_attachment_v1(
        &self,
    ) -> Option<&BoundedPersistentComputeAttachmentV1> {
        self.persistent_compute
            .as_ref()
            .filter(|attachment| attachment.is_three())
    }

    pub(in super::super) fn take_three_binding_persistent_compute_attachment_v1(
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

    pub(in super::super) fn set_three_binding_persistent_compute_attachment_v1(
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
            .and_then(PersistentComputeTerminalNativeCustodyV1::stage)
    }

    pub(in super::super) fn absorb_terminal_prepared_persistent_compute_v1(
        &mut self,
        binding: PersistentComputeBindingKeyV1,
    ) -> bool {
        if self.persistent_compute.as_ref().is_some_and(|attachment| {
            attachment.binding == binding
                && attachment.is_single()
                && attachment.terminal_custody.is_some()
        }) {
            return true;
        }
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
    pub(in super::super) fn absorb_terminal_published_persistent_compute_v1(
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
    pub(in super::super) fn absorb_terminal_completed_persistent_compute_v1(
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

    pub(in super::super) fn absorb_terminal_recycled_persistent_compute_v1(
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
}
