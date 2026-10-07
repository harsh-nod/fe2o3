//! Three-binding persistent compute admission and custody.

use super::*;

impl ComputeAqlQueueSessionV1 {
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
        if !self.terminal_poisoned
            && self.dispatch_capacity.profile() != Gfx942FixedDispatchCapacityProfileV1::Default64
        {
            return Err(recover(
                ComputeAqlQueueSessionErrorV1::Contract(
                    "scaled capacity excludes persistent compute",
                ),
                inputs,
            ));
        }
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
            let (allocation, initialization) = input.into_parts();
            PersistentComputeAttachmentEntryV1 {
                allocation,
                initialization,
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
            super::super::dispatch_binding::Gfx942FixedDispatchDataLayoutV1::device_local(
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
            entries[0].initialization.is_fully_initialized(),
            entries[1].initialization.is_fully_initialized(),
            entries[2].initialization.is_fully_initialized(),
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
            let item = match entries[index].initialization.authenticated_sha256() {
                Some(sha256) => {
                    let content = match Gfx942DeviceContentDescriptorV1::new(
                        content_roles[index],
                        entries[index].allocation.byte_len(),
                        sha256,
                    ) {
                        Ok(content) => content,
                        Err(_) => {
                            data.push(Gfx942FixedDispatchDataV1::initialized_storage(lease));
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
                        Err(lease) => Gfx942FixedDispatchDataV1::initialized_storage(lease),
                    }
                }
                None if entries[index].initialization.is_fully_initialized() => {
                    Gfx942FixedDispatchDataV1::initialized_storage(lease)
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
}
