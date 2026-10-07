use super::*;

impl KfdRuntimeBackendV1 {
    pub(in crate::kfd_backend) fn persistent_full_range_admission_for_launch_v1(
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
        )
        .or_else(|| {
            initialized_storage::initialized_storage_full_range_admission_v1(
                launch.semantic_launch,
                launch.bindings,
                stream_device,
                allocation,
            )
        });
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

    pub(in crate::kfd_backend) fn three_binding_persistent_admission_for_launch_v1(
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

    pub(in crate::kfd_backend) fn prepare_launch(
        &mut self,
        launch: BackendLaunchV1<'_>,
        persistent_selected: bool,
        reuse_bound_recipe: bool,
    ) -> Result<PreparedLaunchV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if persistent_selected {
            self.require_default_dispatch_capacity_v1()?;
        }
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
            admission.source != PersistentFullRangeComputeSourceV1::RetainedControlReplay
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
}
