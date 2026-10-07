use super::*;

impl KfdMultiDeviceRuntimeBackendV1 {
    /// `None` leaves the existing ordinary/directed admission paths unchanged.
    pub(in crate::kfd_backend) fn try_submit_deferred_compute_v1(
        &mut self,
        request: BackendProducerAwareLaunchV1<'_>,
    ) -> Result<Option<u64>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if request.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1
            || request.bindings.len() > fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1
        {
            return Err(KfdRuntimeBackendV1::capacity(
                "deferred compute admission capacity exceeded",
            ));
        }
        let has_native_peer = request.dependencies.iter().any(|dependency| {
            matches!(
            self.submissions.get(&dependency.producer_submission),
            Some(RoutedSubmissionV1::CooperativeCopy(copy))
                if copy.compute_xgmi.is_some() && !copy.is_quiescent()
                    && (copy.directed.is_none()
                        || self.compute_xgmi_children.get(copy.destination.child)
                            == Some(&Some(dependency.producer_submission))))
        });
        if !has_native_peer {
            return Ok(None);
        }
        self.require_no_deferred_stream_v1(request.stream)?;
        let stream = Self::route(
            &self.streams,
            request.stream,
            "unknown deferred compute stream",
        )?;
        let kernel = Self::route(
            &self.kernels,
            request.kernel,
            "unknown deferred compute kernel",
        )?;
        let module_id = *self.kernel_modules.get(&request.kernel).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "deferred compute kernel lost its module",
            )
        })?;
        let module = Self::route(&self.modules, module_id, "unknown deferred compute module")?;
        if kernel.child != stream.child || module.child != stream.child {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "deferred compute kernel belongs to another device",
            ));
        }
        let mut bindings = Vec::new();
        let mut allocations = Vec::new();
        bindings
            .try_reserve_exact(request.bindings.len())
            .map_err(|_| KfdRuntimeBackendV1::capacity("deferred bindings allocation failed"))?;
        allocations
            .try_reserve_exact(request.bindings.len())
            .map_err(|_| KfdRuntimeBackendV1::capacity("deferred allocation roster failed"))?;
        for binding in request.bindings {
            let allocation = Self::route(
                &self.allocations,
                binding.region.allocation,
                "unknown deferred compute allocation",
            )?;
            if allocation.child != stream.child {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::WrongDevice,
                    "deferred compute binding belongs to another device",
                ));
            }
            if self.allocation_retained_by_deferred_compute_v1(allocation) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "binding belongs to another deferred compute consumer",
                ));
            }
            if !allocations.iter().any(|(_, route)| *route == allocation) {
                allocations.push((binding.region.allocation, allocation));
            }
            bindings.push(BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: allocation.local,
                    ..binding.region
                },
                kernarg_byte_offset: binding.kernarg_byte_offset,
            });
        }
        let mut retained = Vec::new();
        let mut peers = Vec::new();
        let mut native = Vec::new();
        retained
            .try_reserve_exact(request.dependencies.len() + 1)
            .map_err(|_| KfdRuntimeBackendV1::capacity("deferred producer roster failed"))?;
        peers
            .try_reserve_exact(request.dependencies.len())
            .map_err(|_| KfdRuntimeBackendV1::capacity("deferred peer roster failed"))?;
        native
            .try_reserve_exact(request.dependencies.len())
            .map_err(|_| KfdRuntimeBackendV1::capacity("deferred native roster failed"))?;
        let mut depth = 1;
        for dependency in request.dependencies {
            if retained.contains(&dependency.producer_submission) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "deferred compute producers must be distinct",
                ));
            }
            match self.events.get(&dependency.event).copied() {
                Some(RoutedEventV1::CooperativeCopy { submission, child })
                    if submission == dependency.producer_submission && child == stream.child =>
                {
                    let Some(RoutedSubmissionV1::CooperativeCopy(copy)) =
                        self.submissions.get(&submission)
                    else {
                        return Err(self.directed_corruption_v1());
                    };
                    if copy.directed.is_some() && !self.directed_identity_is_intact_v1(submission) {
                        return Err(self.directed_corruption_v1());
                    }
                    let segmented = copy
                        .compute_xgmi
                        .as_ref()
                        .is_some_and(|root| root.is_segmented());
                    let segment_frame = if segmented {
                        Some(
                            self.compute_peer_segment_frame_v1(submission, copy.destination)
                                .ok_or_else(|| {
                                    KfdRuntimeBackendV1::rejected(
                                        KfdRuntimeBackendErrorKindV1::Unsupported,
                                        "deferred segmented input requires an exact retained destination frame",
                                    )
                                })?,
                        )
                    } else {
                        None
                    };
                    let frame = request
                        .bindings
                        .iter()
                        .find(|binding| {
                            binding.region.allocation == copy.destination_region.allocation
                        })
                        .and_then(|binding| {
                            self.compute_peer_destination_frame_v1(
                                submission,
                                copy.destination,
                                binding.region,
                            )
                        })
                        .map(compute_peer::Producer::destination_frame);
                    if copy.compute_xgmi.is_none()
                        || copy.directed.is_some()
                            && !copy.is_quiescent()
                            && self.compute_xgmi_children[copy.destination.child]
                                != Some(submission)
                        || copy.destination.child != stream.child
                        || matches!(copy.status(), BackendPollV1::Failed { .. })
                        || request
                            .bindings
                            .iter()
                            .filter(|binding| {
                                binding.region.allocation == copy.destination_region.allocation
                            })
                            .any(|binding| {
                                binding.region.access != RuntimeAccessV1::Read
                                    || binding.region.byte_len == 0
                                    || if let Some(frame) = &segment_frame {
                                        !frame.covers(self, binding.region)
                                    } else {
                                        !frame
                                            .as_ref()
                                            .is_some_and(|frame| frame.covers(self, binding.region))
                                            && (binding.region.byte_offset
                                                < copy.destination_region.byte_offset
                                                || binding
                                                    .region
                                                    .byte_offset
                                                    .checked_add(binding.region.byte_len)
                                                    .zip(
                                                        copy.destination_region
                                                            .byte_offset
                                                            .checked_add(
                                                                copy.destination_region.byte_len,
                                                            ),
                                                    )
                                                    .is_none_or(|(end, producer_end)| {
                                                        end > producer_end
                                                    }))
                                    }
                            })
                    {
                        return Err(KfdRuntimeBackendV1::rejected(
                            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                            "deferred peer destination aliases require covered read ranges",
                        ));
                    }
                    let producer_depth = segment_frame
                        .as_ref()
                        .map_or(copy.dependency_depth, |frame| frame.depth());
                    depth = depth.max(producer_depth.checked_add(1).ok_or_else(|| {
                        KfdRuntimeBackendV1::capacity("deferred dependency depth overflow")
                    })?);
                    peers.push(NativePeerProducerV1 {
                        id: submission,
                        stream: copy.stream,
                        dependency_depth: copy.dependency_depth,
                        directed: copy.directed.is_some(),
                        source: copy.source,
                        destination: copy.destination,
                        source_region: copy.source_region,
                        destination_region: copy.destination_region,
                        frame,
                        segment_frame,
                    });
                }
                Some(RoutedEventV1::Native { .. }) => {
                    let local = self
                        .exact_launch_dependency_for_child(*dependency, stream.child)?
                        .expect("native event returns an exact local producer");
                    native.push(local);
                }
                Some(RoutedEventV1::DeferredCompute { .. }) => {
                    depth = depth.max(
                        self.completed_deferred_dependency_depth_v1(*dependency, stream.child)?,
                    );
                }
                _ => {
                    return Err(KfdRuntimeBackendV1::rejected(
                        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                        "deferred producer event does not name its exact native producer",
                    ));
                }
            }
            retained.push(dependency.producer_submission);
        }
        if depth > MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1 {
            return Err(KfdRuntimeBackendV1::capacity(
                "deferred dependency depth exceeds its bound",
            ));
        }
        if self
            .cooperative_stream_tails
            .get(&request.stream)
            .is_some_and(|tail| !retained.contains(tail))
            || allocations.iter().any(|(_, route)| {
                self.cooperative_allocation_owners
                    .get(route)
                    .is_some_and(|owners| {
                        owners.iter().any(|owner| {
                            !retained.contains(owner)
                                && !peers.iter().any(|peer| {
                                    peer.frame.as_ref().is_some_and(|frame| {
                                        frame.orders_owner(self, peer.id, *route, *owner)
                                    }) || peer.segment_frame.as_ref().is_some_and(|frame| {
                                        frame.orders_owner(self, peer.id, *route, *owner)
                                    })
                                })
                        })
                    })
            })
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "deferred consumer has an unrelated cooperative predecessor",
            ));
        }
        // A busy child is a metadata-only wait for an exact peer or its
        // authenticated destination predecessor, never an unrelated native owner.
        if self.compute_xgmi_children[stream.child].is_some_and(|owner| {
            !peers.iter().any(|peer| {
                peer.id == owner
                    || peer.frame.as_ref().is_some_and(|frame| {
                        frame.owns_occupied_child(self, peer.id, stream.child, owner)
                    })
                    || peer.segment_frame.as_ref().is_some_and(|frame| {
                        frame.owns_occupied_child(self, peer.id, stream.child, owner)
                    })
            })
        }) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "deferred consumer child belongs to another native copy",
            ));
        }
        let launch = BackendLaunchV1 {
            stream: stream.local,
            kernel: kernel.local,
            explicit_kernarg: request.explicit_kernarg,
            bindings: &bindings,
            dependencies: &[],
            geometry: request.geometry,
            semantic_launch: BackendSemanticLaunchV1::Ordinary,
        };
        let result = self.children[stream.child]
            .preflight_compute_v1(launch, ComputeDependencyRosterV1::Exact(&native));
        let mut collected = self.latch(result)?;
        let result = self.children[stream.child].validate_compute_launch_base_v1(&launch);
        self.latch(result)?;
        depth = self.children[stream.child]
            .next_dependency_depth_v1(
                collected.ordered_predecessor,
                &collected.explicit_success_dependencies,
            )
            .map_err(|_| {
                KfdRuntimeBackendV1::capacity("deferred dependency depth exceeds its bound")
            })?
            .max(depth);
        collected.minimum_dependency_depth = depth;
        if let Some(prior) = collected.ordered_predecessor {
            let owner = self
                .submissions
                .iter()
                .find_map(|(id, entry)| match entry {
                    RoutedSubmissionV1::Native { route, .. }
                        if *route
                            == (RoutedHandleV1 {
                                child: stream.child,
                                local: prior,
                            }) =>
                    {
                        Some(*id)
                    }
                    RoutedSubmissionV1::DeferredCompute(root)
                        if root.route
                            == Some(RoutedHandleV1 {
                                child: stream.child,
                                local: prior,
                            })
                            && root.status == BackendPollV1::Succeeded
                            && retained.contains(id) =>
                    {
                        Some(*id)
                    }
                    _ => None,
                })
                .ok_or_else(|| {
                    KfdRuntimeBackendV1::rejected(
                        KfdRuntimeBackendErrorKindV1::Busy,
                        "deferred consumer stream has an unsupported predecessor",
                    )
                })?;
            if !retained.contains(&owner) {
                retained.push(owner);
            }
        }
        let payload = RetainedComputeLaunchV1::copy_from(
            launch,
            self.children[stream.child].launch_payload_account.as_ref(),
        )?;
        self.peer_launch_retains.prepare(&retained)?;
        self.deferred_compute_retains
            .prepare(request.stream, &allocations, module)?;
        self.reserve_native_stream_submission_v1(request.stream)?;
        Self::reserve_route(
            &mut self.submissions,
            "deferred compute route allocation failed",
        )?;
        let shell = try_uninit_box_v1().map_err(|()| {
            KfdRuntimeBackendV1::capacity("deferred compute root allocation failed")
        })?;
        let id = self.next_id()?;
        self.with_peer_launch_custody_v1(id, retained, |backend| {
            let root = Box::write(
                shell,
                DeferredComputeV1 {
                    stream: request.stream,
                    child: stream.child,
                    route: None,
                    status: BackendPollV1::Pending,
                    quiescent: None,
                    completed: None,
                    identity: DeferredComputeIdentityV1::new(&payload, depth, None),
                    launch: Some(payload),
                    collected: Some(collected),
                    kernel: (request.kernel, kernel),
                    module: (module_id, module),
                    allocations,
                    peers,
                },
            );
            backend.deferred_compute_retains.acquire(id, &root);
            backend
                .submissions
                .insert(id, RoutedSubmissionV1::DeferredCompute(root));
            backend.retain_native_stream_submission_v1(request.stream);
            Ok(id)
        })?;
        Ok(Some(id))
    }
}
