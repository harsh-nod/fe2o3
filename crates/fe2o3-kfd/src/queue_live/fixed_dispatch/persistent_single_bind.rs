//! Single-binding persistent compute admission and custody.

use super::*;

impl ComputeAqlQueueSessionV1 {
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
        if !self.terminal_poisoned
            && self.dispatch_capacity.profile() != Gfx942FixedDispatchCapacityProfileV1::Default64
        {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "scaled capacity excludes persistent compute",
                ),
                input,
            ));
        }
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
            Gfx942PersistentComputeInputV1::InitializedStorage(_) => None,
        };
        let initialized = !matches!(&input, Gfx942PersistentComputeInputV1::Uninitialized(_));
        let allocation = match &mut input {
            Gfx942PersistentComputeInputV1::Uninitialized(allocation) => allocation,
            Gfx942PersistentComputeInputV1::InitializedAfterDispatch(ready) => {
                &mut ready.allocation
            }
            Gfx942PersistentComputeInputV1::InitializedStorage(ready) => &mut ready.allocation,
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
                super::super::dispatch_binding::Gfx942FixedDispatchDataLayoutV1::device_local(
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
                        let (allocation, initialization) = input.into_parts();
                        self.set_single_persistent_compute_attachment_v1(
                            PersistentComputeAttachmentV1 {
                                allocation,
                                initialization,
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
                let (mut allocation, initialization) = input.into_parts();
                let state = quarantine_persistent_retained_control_replay_prepared_v1(
                    &mut allocation.owner,
                    prepared,
                );
                self.set_single_persistent_compute_attachment_v1(PersistentComputeAttachmentV1 {
                    allocation,
                    initialization,
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
            let (allocation, initialization) = input.into_parts();
            let mut attachment = PersistentComputeAttachmentV1 {
                allocation,
                initialization,
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
                    attachment.initialization,
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
                    let (allocation, initialization) = input.into_parts();
                    self.set_single_persistent_compute_attachment_v1(
                        PersistentComputeAttachmentV1 {
                            allocation,
                            initialization,
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
        let (mut allocation, initialization) = input.into_parts();
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
                                initialization,
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
            None if initialized => Gfx942FixedDispatchDataV1::initialized_storage(lease),
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
                    initialization,
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
                    initialization,
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
            initialization,
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
}
