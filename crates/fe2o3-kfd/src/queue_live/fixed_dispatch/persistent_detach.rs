//! Persistent compute DATA detachment after exact recycling.

use super::*;

impl ComputeAqlQueueSessionV1 {
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
        let restore_preflight =
            attachment
                .entries
                .iter()
                .zip(&data)
                .try_for_each(|(entry, data)| {
                    let PersistentComputeUseStateV1::Recycled(completed) = &entry.state else {
                        return Err(Gfx942PersistentUseErrorV1::WrongState);
                    };
                    entry
                        .allocation
                        .owner
                        .preflight_restore_completed_compute_data(
                            completed,
                            data,
                            entry.allocation.attachment.queue,
                            entry.allocation.attachment.pool_generation,
                            entry.allocation.attachment.logical_bytes,
                        )
                });
        if restore_preflight.is_err() {
            quarantine_persistent_compute_entries_v1(
                attachment.entries.each_mut(),
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Data(
                PersistentComputeTerminalDataV1::from_three(data),
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
        for (entry, data) in attachment.entries.iter_mut().zip(data) {
            let PersistentComputeUseStateV1::Recycled(completed) = &entry.state else {
                unreachable!("preflighted three-binding recycled state")
            };
            entry
                .allocation
                .owner
                .restore_completed_compute_data(
                    completed,
                    data,
                    entry.allocation.attachment.queue,
                    entry.allocation.attachment.pool_generation,
                    entry.allocation.attachment.logical_bytes,
                )
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
                    .then_some(entry.initialization.authenticated_sha256())
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
        if let Err((_error, data)) = attachment.allocation.owner.restore_completed_compute_data(
            &completed_use,
            data,
            attachment.allocation.attachment.queue,
            attachment.allocation.attachment.pool_generation,
            attachment.allocation.attachment.logical_bytes,
        ) {
            attachment.state = quarantine_persistent_compute_recycled_v1(
                &mut attachment.allocation.owner,
                completed_use,
                Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss,
            );
            attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Data(
                PersistentComputeTerminalDataV1::from_one(data),
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
                .then_some(attachment.initialization.authenticated_sha256())
                .flatten(),
            fully_initialized,
        })
    }
}
