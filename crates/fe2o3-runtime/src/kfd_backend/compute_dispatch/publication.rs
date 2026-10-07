use super::*;

impl KfdRuntimeBackendV1 {
    pub(in crate::kfd_backend) fn publish(
        &mut self,
        id: u64,
        dependency_depth: usize,
        ordered_predecessor: Option<u64>,
        ordinary_recipe: Arc<RetainedComputeLaunchV1>,
        prepared: PreparedLaunchV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        use super::materialized_publication::{
            MaterializedBindingV1, with_recycled_materialized_metadata_v1,
        };

        self.require_unpinned_native_lane_v1(self.selected_compute_lane)?;
        if matches!(
            &prepared.storage,
            PreparedLaunchStorageV1::PersistentFullRange(_)
                | PreparedLaunchStorageV1::ThreeBindingPersistent(_)
        ) {
            if matches!(
                &prepared.storage,
                PreparedLaunchStorageV1::ThreeBindingPersistent(_)
            ) {
                return self.publish_three_binding_persistent_v1(
                    id,
                    dependency_depth,
                    ordered_predecessor,
                    prepared,
                );
            }
            return self.publish_persistent_full_range_v1(
                id,
                dependency_depth,
                ordered_predecessor,
                prepared,
            );
        }
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
        let PreparedLaunchStorageV1::Materialized(data) = storage else {
            unreachable!("persistent launch publication branches before materialization")
        };
        for (index, writeback) in writebacks.iter().enumerate() {
            if writebacks[..index]
                .iter()
                .any(|prior| prior.allocation == writeback.allocation)
            {
                continue;
            }
            let required = writebacks[index..]
                .iter()
                .filter(|candidate| candidate.allocation == writeback.allocation)
                .count();
            self.allocations
                .get_mut(&writeback.allocation)
                .expect("prepared writeback allocation remains retained")
                .native_dirty
                .try_reserve(required)
                .map_err(|_| Self::capacity("KFD native-dirty extent reservation failed"))?;
        }
        let resident_descriptors = resident_descriptors_v1(&data)?;
        let user_data_count =
            u64::try_from(data.len()).expect("fixed-dispatch data count is bounded below u64");

        let cpu = false;
        #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
        let cpu = cpu || self.cpu_queue.is_some();
        #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
        if cpu {
            self.require_cpu_provider_v1()?;
            if !writebacks.is_empty() {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    "CPU receipt fixture has no device writeback authority",
                ));
            }
            self.detach_cpu_recycled_dispatch_v1()?;
        }
        let scripted = false;
        #[cfg(test)]
        let scripted = scripted
            || self.scripted_sdma.is_some()
                && (writebacks.is_empty()
                    || self.scripted_materialized_preparation.is_some()
                    || self.scripted_materialized_publication_fault.is_some());
        let native_binding_started = Instant::now();
        let creates_native_queue = self.native_compute_lanes[self.selected_compute_lane].is_none();
        let mut reused_attached = false;
        let reuse_attached = self.recycled_dispatch.as_ref().is_some_and(|recycled| {
            recycled_dispatch_reuse_is_admitted_v1(
                recycled,
                dispatch_shape_sha256,
                &resident_descriptors,
                &data,
            )
        });
        let preallocation = if scripted || cpu {
            None
        } else {
            self.preallocate_native_binding_v1(reuse_attached)?
        };
        // Pure host preparation can still reject without taking native custody.
        let programs = if reuse_attached || scripted || cpu {
            None
        } else {
            let validated_program = build_program_v1(&program, signature, &abi_rows)?;
            let mut programs = Vec::new();
            programs
                .try_reserve_exact(1)
                .map_err(|_| Self::capacity("KFD program roster allocation failed"))?;
            programs.push(validated_program);
            Some(programs)
        };
        if self.active.is_some() || !self.compute_pipeline.is_empty() {
            return Err(self.terminal_error("ordinary binding requires an idle logical lane"));
        }
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
            resident_descriptors,
            ordinary_recipe: Some(ordinary_recipe),
            dispatch_shape_sha256,
            published_at: Instant::now(),
            performance,
            execution: Some(ActiveComputeExecutionV1::MaterializedBinding(
                MaterializedBindingV1::new(PersistentPublicationProfileV1 {
                    launch: profile_launch,
                    semantic_contract: profile_semantic_contract,
                    bindings: profile_bindings,
                }),
            )),
        });
        #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
        if cpu {
            return self.submit_materialized_binding_v1();
        }
        #[cfg(test)]
        if scripted {
            return self.publish_scripted_materialized_binding_v1(data);
        }
        if self.recycled_dispatch.is_some() && !reuse_attached {
            self.detach_recycled_dispatch()?;
        }
        if reuse_attached {
            let overwrite = {
                let native_lane = self.selected_native_compute_lane_v1()?;
                let root = MaterializedBindingV1::indexed(self.active.as_mut().unwrap());
                let queue = self
                    .queue
                    .as_mut()
                    .expect("recycled dispatch retains queue");
                with_recycled_materialized_metadata_v1(&mut self.recycled_dispatch, |recycled| {
                    queue
                        .with_compute_lane_v1(native_lane, |queue| {
                            queue
                                .recycled_fixed_dispatch_generation()
                                .map_err(|error| format!("KFD recycled generation: {error}"))
                                .and_then(|generation| {
                                    root.origin =
                                        MaterializedPreparationOriginV1::RecycledAttachment {
                                            generation,
                                        };
                                    recycled
                                        .descriptors
                                        .iter()
                                        .zip(&data)
                                        .enumerate()
                                        .try_for_each(|(index, (prior, spec))| {
                                            if !resident_data_needs_host_overwrite_v1(
                                                prior,
                                                spec.content_sha256,
                                            ) {
                                                return Ok(());
                                            }
                                            queue
                                                .overwrite_recycled_fixed_dispatch_host_data(
                                                    Gfx942RecycledDispatchWriteRequestV1::new(
                                                        generation, index, 0,
                                                    ),
                                                    spec.bytes(),
                                                )
                                                .map_err(|error| {
                                                    format!("KFD recycled-data overwrite: {error}")
                                                })
                                        })
                                })
                        })
                        .map_err(|error| format!("KFD compute-lane selection: {error}"))
                        .and_then(core::convert::identity)
                })
            };
            overwrite.map_err(|detail| self.terminal_error(detail))?;
            reused_attached = true;
            performance.data_path = KfdRuntimeLaunchDataPathV1::ResidentReused;
        }

        if !reused_attached {
            let programs = programs.expect("new binding preflighted programs");
            let packet = Gfx942FixedDispatchPacketV1::new(
                0,
                geometry,
                dynamic_shared_bytes,
                kernarg,
                buffer_bindings,
            );
            if creates_native_queue && self.queue.is_none() {
                performance.user_data_materializations = user_data_count;
                if self.terminal_memory.is_some() {
                    return Err(
                        self.terminal_error("KFD materialization session is already retained")
                    );
                }
                let admission = self.take_rooted_backing_v1()?;
                let device = self.admitted_device.take().ok_or_else(|| {
                    Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Unsupported,
                        "the admitted KFD queue lifecycle has already retired",
                    )
                })?;
                let memory = match admission {
                    Some(native_budget::BackingAdmissionV1::Host(admission)) => device
                        .acquire_shared_gtt_memory_session_with_rooted_host_backing_v1(
                            self.device_backing_budget,
                            admission,
                        ),
                    Some(native_budget::BackingAdmissionV1::Native(admission)) => device
                        .acquire_shared_gtt_memory_session_with_rooted_native_backing_v1(admission),
                    Some(native_budget::BackingAdmissionV1::Composed(admission)) => {
                        device.acquire_shared_gtt_memory_session_with_composed_backing_v1(admission)
                    }
                    None => device.acquire_shared_gtt_memory_session_with_backing_budgets_v1(
                        self.device_backing_budget,
                        self.host_visible_backing_budget,
                    ),
                }
                .map_err(|error| self.terminal_error(format!("KFD VM acquisition: {error}")))?;
                self.terminal_memory = Some(memory);
                let (memory, native_data) =
                    materialize_in_retained_session_v1(&mut self.terminal_memory, |memory| {
                        materialize_initial_data_v1(memory, data, signature)
                    })
                    .map_err(|detail| self.terminal_error(detail))?;
                let queue = memory
                    .create_compute_aql_queue_with_preallocated_fixed_dispatch_v1(
                        KFD_RUNTIME_RING_BYTES_V1,
                        programs,
                        [packet],
                        native_data,
                        self.dispatch_capacity.native().clone(),
                        preallocation,
                    )
                    .map_err(|error| self.terminal_error(format!("KFD queue creation: {error}")))?;
                let primary_lane = queue.primary_compute_lane_v1();
                self.queue = Some(queue);
                self.configure_native_device_pool_v1()?;
                self.configure_native_host_pool_v1()?;
                self.native_compute_lanes[self.selected_compute_lane] = Some(primary_lane);
            } else if creates_native_queue && self.native_compute_lanes.iter().all(Option::is_none)
            {
                performance.user_data_materializations = user_data_count;
                let mut materialization_error = None;
                let error_slot = &mut materialization_error;
                let count = data.len();
                let queue = self.queue.as_mut().expect("bootstrap KFD queue exists");
                queue
                    .bind_initial_fixed_dispatch_with_preallocation_v1(
                        programs,
                        [packet],
                        count,
                        preallocation,
                        move |memory, index| {
                            materialize_initial_data_item_v1(memory, &data[index], index, signature)
                                .map_err(|detail| {
                                    *error_slot = Some(detail);
                                    fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                                        "KFD primary data materialization",
                                    )
                                })
                        },
                    )
                    .map_err(|error| {
                        self.terminal_error(
                            materialization_error
                                .unwrap_or_else(|| format!("KFD initial primary binding: {error}")),
                        )
                    })?;
                self.native_compute_lanes[self.selected_compute_lane] = Some(
                    self.queue
                        .as_ref()
                        .expect("bound bootstrap primary")
                        .primary_compute_lane_v1(),
                );
            } else if creates_native_queue {
                performance.user_data_materializations = user_data_count;
                let mut materialization_error = None;
                let lane = self
                    .queue
                    .as_mut()
                    .expect("shared KFD queue owner exists")
                    .create_auxiliary_compute_lane_with_preallocated_fixed_dispatch_v1(
                        KFD_RUNTIME_RING_BYTES_V1,
                        programs,
                        [packet],
                        preallocation,
                        |memory| {
                            materialize_initial_data_v1(memory, data, signature).map_err(|detail| {
                                materialization_error = Some(detail);
                                fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                                    "KFD auxiliary data materialization",
                                )
                            })
                        },
                    )
                    .map_err(|error| {
                        self.terminal_error(
                            materialization_error.unwrap_or_else(|| {
                                format!("KFD auxiliary queue creation: {error}")
                            }),
                        )
                    })?;
                self.native_compute_lanes[self.selected_compute_lane] = Some(lane);
            } else {
                let mut reused_resident_data = false;
                let rebound = {
                    let native_lane = self.selected_native_compute_lane_v1()?;
                    let queue = self.queue.as_mut().expect("checked queue");
                    queue
                        .with_compute_lane_v1(native_lane, |queue| {
                            let native_data = match self.resident_data.take() {
                                Some(resident)
                                    if same_resident_storage_shape_v1(
                                        &resident.descriptors,
                                        &self.active.as_ref().unwrap().resident_descriptors,
                                    ) && data.iter().all(|spec| {
                                        spec.kind == RuntimeMemoryKindV1::HostVisible
                                    }) =>
                                {
                                    reused_resident_data = true;
                                    let writer = &mut *queue;
                                    overwrite_with_custody_v1(
                                        resident.descriptors,
                                        resident.data,
                                        move |index, prior, native| {
                                            let spec = &data[index];
                                            if !resident_data_needs_host_overwrite_v1(
                                                prior,
                                                spec.content_sha256,
                                            ) {
                                                return Ok(());
                                            }
                                            writer
                                                .overwrite_detached_initialized_host_visible_fixed_dispatch_data(
                                                    index,
                                                    native,
                                                    0,
                                                    spec.bytes(),
                                                )
                                                .map_err(|error| {
                                                    format!("KFD resident-data overwrite: {error}")
                                                })
                                        },
                                        core::mem::forget,
                                    )
                                }
                                Some(resident) => with_resident_release_custody_v1(
                                    resident.descriptors,
                                    resident.data,
                                    |custody| release_resident_data_v1(queue, custody),
                                    core::mem::forget,
                                )
                                .and_then(|()| {
                                    materialize_rebound_data_v1(queue, data, signature)
                                }),
                                None => materialize_rebound_data_v1(queue, data, signature),
                            };
                            native_data.and_then(|native_data| {
                                queue
                                    .bind_fixed_dispatch_with_preallocation_v1(programs, [packet], native_data, preallocation)
                                    .map_err(|error| format!("KFD dispatch rebind: {error}"))
                            })
                        })
                        .map_err(|error| format!("KFD compute-lane selection: {error}"))
                        .and_then(core::convert::identity)
                };
                if let Err(detail) = rebound {
                    return Err(self.terminal_error(detail));
                }
                if reused_resident_data {
                    performance.data_path = KfdRuntimeLaunchDataPathV1::ResidentReused;
                } else {
                    performance.user_data_materializations = user_data_count;
                }
            }
        }
        performance.native_binding = native_binding_started.elapsed();
        self.active.as_mut().unwrap().performance = performance;
        if creates_native_queue {
            let queue = self.profile_resource_v1(
                KfdProfileResourceKindV1::NativeQueue,
                KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + self.selected_compute_lane as u64,
            );
            self.observe_profile_v1(
                queue.map(|queue| KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue }),
            );
        }

        self.submit_materialized_binding_v1()
    }

    pub(in crate::kfd_backend) fn observe_materialized_dispatch_published_v1(
        &mut self,
        id: u64,
        stream: u64,
        kernel: u64,
        dispatch_shape_sha256: [u8; 32],
        profile: PersistentPublicationProfileV1,
    ) {
        #[cfg(test)]
        if self.scripted_materialized_publication_fault == Some(
            super::materialized_publication::ScriptedMaterializedPublicationFaultV1::ProfileUnwind,
        ) {
            self.scripted_materialized_publication_fault = None;
            panic!("scripted ordinary publication profile unwind");
        }
        let profile_dispatch = self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, id);
        let profile_queue = self.profile_resource_v1(
            KfdProfileResourceKindV1::NativeQueue,
            KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + self.selected_compute_lane as u64,
        );
        let profile_stream = self.profile_resource_v1(KfdProfileResourceKindV1::Stream, stream);
        let profile_kernel = self.profile_resource_v1(KfdProfileResourceKindV1::Kernel, kernel);
        let profile_shape = self.profile_content_v1(&dispatch_shape_sha256);
        let profile_event = match profile.bindings {
            Some(Ok(bindings)) => profile_dispatch
                .zip(profile_queue)
                .zip(profile_stream)
                .zip(profile_kernel)
                .zip(profile_shape)
                .map(|((((dispatch, queue), stream), kernel), dispatch_shape)| {
                    KfdRuntimeProfileEventKindV1::DispatchPublished {
                        dispatch,
                        queue,
                        stream,
                        kernel,
                        dispatch_shape,
                        launch: profile.launch,
                        bindings,
                    }
                }),
            Some(Err(())) | None => None,
        };
        self.observe_profile_dispatch_v1(profile_event, profile.semantic_contract);
    }

    pub(in crate::kfd_backend) fn observe_persistent_dispatch_published_v1(
        &mut self,
        id: u64,
        stream: u64,
        kernel: u64,
        dispatch_shape_sha256: [u8; 32],
        profile: PersistentPublicationProfileV1,
    ) {
        #[cfg(test)]
        if self.scripted_prepared_publication_fault
            == Some(super::prepared_publication::ScriptedPreparedPublicationFaultV1::ProfileUnwind)
        {
            self.scripted_prepared_publication_fault = None;
            panic!("scripted persistent publication profile unwind");
        }
        let profile_dispatch = self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, id);
        let profile_queue = self.profile_resource_v1(
            KfdProfileResourceKindV1::NativeQueue,
            KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1,
        );
        let profile_stream = self.profile_resource_v1(KfdProfileResourceKindV1::Stream, stream);
        let profile_kernel = self.profile_resource_v1(KfdProfileResourceKindV1::Kernel, kernel);
        let profile_shape = self.profile_content_v1(&dispatch_shape_sha256);
        let profile_event = match profile.bindings {
            Some(Ok(bindings)) => profile_dispatch
                .zip(profile_queue)
                .zip(profile_stream)
                .zip(profile_kernel)
                .zip(profile_shape)
                .map(|((((dispatch, queue), stream), kernel), dispatch_shape)| {
                    KfdRuntimeProfileEventKindV1::DispatchPublished {
                        dispatch,
                        queue,
                        stream,
                        kernel,
                        dispatch_shape,
                        launch: profile.launch,
                        bindings,
                    }
                }),
            Some(Err(())) | None => None,
        };
        self.observe_profile_dispatch_v1(profile_event, profile.semantic_contract);
    }
}
