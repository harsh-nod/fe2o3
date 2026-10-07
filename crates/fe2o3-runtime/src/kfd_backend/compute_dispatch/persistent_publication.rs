use super::*;

impl KfdRuntimeBackendV1 {
    pub(in crate::kfd_backend) fn publish_three_binding_persistent_v1(
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
        let completion = self.reserve_three_binding_completion_v1(persistent.admissions)?;
        let (persistent_inputs, promotions) =
            self.take_three_binding_persistent_inputs_v1(persistent.admissions, id)?;
        let publication_profile = PersistentPublicationProfileV1 {
            launch: profile_launch,
            semantic_contract: profile_semantic_contract,
            bindings: profile_bindings,
        };
        #[cfg(test)]
        if self.scripted_sdma.is_some() {
            performance.data_path = KfdRuntimeLaunchDataPathV1::PersistentDeviceReused;
            performance.user_data_materializations = 0;
            self.active = Some(ActiveSubmissionV1 {
                source_event: Default::default(),
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
                execution: Some(
                    ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
                        admissions: persistent.admissions,
                        promotions,
                        restore_shells,
                        inputs: PreparedReceiptV1::Armed(persistent_inputs),
                        completion,
                        profile: publication_profile,
                    },
                ),
            });
            return self.publish_initial_persistent_prepared_v1();
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
                | KfdRuntimePersistentComputeInputV1::ScriptedReplay(_)
                | KfdRuntimePersistentComputeInputV1::ScriptedStorage(_) => unreachable!(),
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
        record_initial_persistent_timing_v1(
            &mut performance,
            native_binding_started.elapsed(),
            Duration::ZERO,
        );
        performance.data_path = KfdRuntimeLaunchDataPathV1::PersistentDeviceReused;
        performance.user_data_materializations = 0;
        // Index the receipt before the first consuming call or queue observer.
        self.active = Some(ActiveSubmissionV1 {
            source_event: Default::default(),
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
            execution: Some(ActiveComputeExecutionV1::ThreeBindingPersistentPrepared {
                admissions: persistent.admissions,
                promotions,
                restore_shells,
                prepared: PreparedReceiptV1::Armed(binding),
                completion,
                profile: publication_profile,
            }),
        });
        self.publish_initial_persistent_prepared_v1()
    }

    pub(in crate::kfd_backend) fn publish_persistent_full_range_v1(
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
        #[cfg(test)]
        let input_shell = if self.scripted_sdma.is_some() {
            Some(try_uninit_box_v1().map_err(|_| {
                Self::capacity("scripted initial publication input shell allocation failed")
            })?)
        } else {
            None
        };
        let admission = PersistentFullRangeComputeAdmissionV1 {
            allocation: persistent.allocation,
            access: persistent.access,
            source: persistent.source,
        };
        let completion = self.reserve_persistent_completion_v1(admission)?;
        let (persistent_input, restoration) =
            self.take_persistent_compute_input_v1(admission, id)?;
        let promotion = restoration.promotion;
        performance.ready_promotion = promotion;
        let publication_profile = PersistentPublicationProfileV1 {
            launch: profile_launch,
            semantic_contract: profile_semantic_contract,
            bindings: profile_bindings,
        };
        #[cfg(test)]
        if self.scripted_sdma.is_some() {
            if self.scripted_persistent_bind_rejections != 0 {
                self.scripted_persistent_bind_rejections -= 1;
                self.restore_persistent_bind_input_v1(persistent_input, restoration)?;
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Native,
                    "scripted persistent-compute clean bind rejection",
                ));
            }
            performance.data_path = KfdRuntimeLaunchDataPathV1::PersistentDeviceReused;
            performance.user_data_materializations = 0;
            self.active = Some(ActiveSubmissionV1 {
                source_event: Default::default(),
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
                    source: persistent.source,
                    input: PreparedReceiptV1::Armed(fill_restore_shell_v1(
                        input_shell.expect("reserved scripted input shell"),
                        persistent_input,
                    )),
                    completion,
                    profile: publication_profile,
                }),
            });
            self.install_scalar_completion_shell_v1(restoration);
            return self.publish_initial_persistent_prepared_v1();
        }
        #[cfg(not(test))]
        let KfdRuntimePersistentComputeInputV1::Native(input) = persistent_input;
        #[cfg(test)]
        let input = match persistent_input {
            KfdRuntimePersistentComputeInputV1::Native(input) => input,
            input => {
                let detail = if matches!(
                    &input,
                    KfdRuntimePersistentComputeInputV1::ScriptedStorage(_)
                ) {
                    "scripted initialized-storage publication has no native queue"
                } else {
                    "scripted persistent-compute publication has no native queue"
                };
                self.restore_persistent_bind_input_v1(input, restoration)?;
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    detail,
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
                let (detail, custody) = failure.into_parts();
                return match custody {
                    Gfx942PersistentComputeBindFailureCustodyV1::Retryable(recovered) => {
                        self.restore_persistent_bind_input_v1(
                            KfdRuntimePersistentComputeInputV1::Native(recovered),
                            restoration,
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
        record_initial_persistent_timing_v1(
            &mut performance,
            native_binding_started.elapsed(),
            Duration::ZERO,
        );
        performance.data_path = KfdRuntimeLaunchDataPathV1::PersistentDeviceReused;
        performance.user_data_materializations = 0;
        // Index the receipt before the first consuming call or queue observer.
        self.active = Some(ActiveSubmissionV1 {
            source_event: Default::default(),
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
                source: persistent.source,
                prepared: PreparedReceiptV1::Armed(binding),
                completion,
                profile: publication_profile,
            }),
        });
        self.install_scalar_completion_shell_v1(restoration);
        self.publish_initial_persistent_prepared_v1()
    }
}
