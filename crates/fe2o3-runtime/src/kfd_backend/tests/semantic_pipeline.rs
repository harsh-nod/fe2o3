use super::*;

#[test]
fn profiler_projection_retains_exact_atomic_and_collective_contracts() {
    let geometry = KfdProfileLaunchV1 {
        grid: [64, 1, 1],
        workgroup: [64, 1, 1],
        dynamic_shared_bytes: 0,
    };
    let atomic = RuntimeAtomicLaunchContractV1 {
        operation: RuntimeAtomicOperationV1::CompareExchange,
        scope: RuntimeMemoryScopeV1::Device,
        order: RuntimeMemoryOrderV1::SequentiallyConsistent,
        failure_order: Some(RuntimeMemoryOrderV1::Acquire),
        weak: true,
        geometry: semantic_geometry_v1(),
    };
    assert_eq!(
        profile_semantic_contract_v1(KfdRuntimeSemanticLaunchV1::Atomic(atomic), geometry),
        Some(KfdProfileSemanticContractV1::Atomic(
            KfdProfileAtomicContractV1 {
                operation: KfdProfileAtomicOperationV1::CompareExchange,
                scope: KfdProfileMemoryScopeV1::Device,
                order: KfdProfileMemoryOrderV1::SequentiallyConsistent,
                failure_order: Some(KfdProfileMemoryOrderV1::Acquire),
                weak: true,
                geometry,
            }
        ))
    );

    assert_eq!(
        profile_semantic_contract_v1(
            KfdRuntimeSemanticLaunchV1::Collective(collective_contract_v1()),
            geometry,
        ),
        Some(KfdProfileSemanticContractV1::Collective(
            KfdProfileCollectiveContractV1 {
                operation: KfdProfileCollectiveOperationV1::ReduceSum,
                scope: KfdProfileMemoryScopeV1::Workgroup,
                order: KfdProfileMemoryOrderV1::AcquireRelease,
                participants: 64,
                geometry,
            }
        ))
    );
    assert_eq!(
        profile_semantic_contract_v1(KfdRuntimeSemanticLaunchV1::Ordinary, geometry),
        None
    );
}

#[test]
fn semantic_profiles_control_both_capability_layers_fail_closed() {
    let overbound = KfdRuntimeLaunchGateV1::Semantic(Box::new(TestOverboundSemanticAuthorityV1));
    assert!(!overbound.advertises_atomics_v1());
    assert!(!overbound.supports_atomic_v1(atomic_contract_v1()));

    let panicking =
        KfdRuntimeLaunchGateV1::Semantic(Box::new(TestPanickingSemanticProfileAuthorityV1));
    assert!(!panicking.advertises_atomics_v1());
    assert!(!panicking.advertises_collectives_v1());
    assert!(!panicking.supports_atomic_v1(atomic_contract_v1()));
    assert!(!panicking.supports_collective_v1(collective_contract_v1()));

    let mut ordinary = KfdRuntimeBackendV1::mock();
    assert!(!ordinary.description.capabilities.atomics);
    assert!(!ordinary.description.capabilities.collectives);
    ordinary.native_available = true;
    assert!(!ordinary.execution_capabilities_v1(7).atomics);
    assert!(!ordinary.execution_capabilities_v1(7).collectives);

    let mut semantic = KfdRuntimeBackendV1::mock_with_semantic_authority_v1();
    assert!(semantic.description.capabilities.atomics);
    assert!(semantic.description.capabilities.collectives);
    assert_eq!(
        semantic.execution_capabilities_v1(7),
        RuntimeExecutionCapabilitiesV1::default()
    );
    semantic.native_available = true;
    assert!(semantic.execution_capabilities_v1(7).atomics);
    assert!(semantic.execution_capabilities_v1(7).collectives);
    assert_eq!(
        semantic.execution_capabilities_v1(8),
        RuntimeExecutionCapabilitiesV1::default()
    );
    semantic.native_available = false;
    semantic.shutdown_native_v1().unwrap();
    ordinary.native_available = false;
    ordinary.shutdown_native_v1().unwrap();
}

#[test]
fn semantic_rejections_precede_scheduler_custody_and_handle_allocation() {
    let mut backend = KfdRuntimeBackendV1::mock_with_semantic_authority_v1();
    let stream = backend.create_stream_v1(7).unwrap();
    backend.native_available = true;
    let before_handle = backend.next_handle;
    let unsupported_atomic = RuntimeAtomicLaunchContractV1 {
        operation: RuntimeAtomicOperationV1::Exchange,
        ..atomic_contract_v1()
    };
    let launch = |semantic_launch| BackendLaunchV1 {
        stream,
        kernel: 999,
        explicit_kernarg: &[],
        bindings: &[],
        dependencies: &[],
        geometry: semantic_geometry_v1(),
        semantic_launch,
    };
    assert!(matches!(
        backend.submit_atomic_v1(launch(KfdRuntimeSemanticLaunchV1::Atomic(
            unsupported_atomic,
        ))),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
    ));
    let system_atomic = RuntimeAtomicLaunchContractV1 {
        scope: RuntimeMemoryScopeV1::System,
        ..atomic_contract_v1()
    };
    assert!(matches!(
        backend.submit_atomic_v1(launch(KfdRuntimeSemanticLaunchV1::Atomic(system_atomic))),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
    ));
    let bad_collective = RuntimeCollectiveLaunchContractV1 {
        participants: 63,
        ..collective_contract_v1()
    };
    assert!(matches!(
        backend.submit_collective_v1(launch(KfdRuntimeSemanticLaunchV1::Collective(
            bad_collective,
        ))),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
    ));
    assert!(matches!(
        backend.submit_atomic_v1(launch(KfdRuntimeSemanticLaunchV1::Collective(
            collective_contract_v1(),
        ))),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    assert_eq!(backend.next_handle, before_handle);
    assert!(backend.pending_compute.is_empty());
    assert!(backend.allocation_custody.is_empty());
    assert!(backend.compute_module_retain_counts.is_empty());
    assert_eq!(backend.compute_completion_reservations, 0);
    backend.native_available = false;
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn semantic_contract_is_part_of_recycled_dispatch_identity() {
    let geometry = semantic_geometry_v1();
    let ordinary = BackendLaunchV1 {
        stream: 1,
        kernel: 2,
        explicit_kernarg: &[3],
        bindings: &[],
        dependencies: &[],
        geometry,
        semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
    };
    let atomic = BackendLaunchV1 {
        semantic_launch: KfdRuntimeSemanticLaunchV1::Atomic(atomic_contract_v1()),
        ..ordinary
    };
    let collective = BackendLaunchV1 {
        semantic_launch: KfdRuntimeSemanticLaunchV1::Collective(collective_contract_v1()),
        ..ordinary
    };
    assert_ne!(
        dispatch_shape_sha256_v1(&ordinary, ordinary.semantic_launch),
        dispatch_shape_sha256_v1(&atomic, atomic.semantic_launch)
    );
    assert_ne!(
        dispatch_shape_sha256_v1(&atomic, atomic.semantic_launch),
        dispatch_shape_sha256_v1(&collective, collective.semantic_launch)
    );
}

#[test]
fn ordinary_pipeline_recipe_digest_rejects_every_mutable_launch_axis() {
    let geometry = crate::RuntimeLaunchGeometryV1 {
        grid: [8, 1, 1],
        workgroup: [4, 1, 1],
        dynamic_shared_bytes: 16,
    };
    let binding = BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: 3,
            access: RuntimeAccessV1::Read,
            byte_offset: 8,
            byte_len: 16,
        },
        kernarg_byte_offset: 24,
    };
    let digest = |kernel, kernarg: &[u8], binding, geometry| {
        let bindings = [binding];
        let launch = BackendLaunchV1 {
            stream: 1,
            kernel,
            explicit_kernarg: kernarg,
            bindings: &bindings,
            dependencies: &[],
            geometry,
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        };
        dispatch_shape_sha256_v1(&launch, launch.semantic_launch)
    };
    let expected = digest(2, &[1, 2], binding, geometry);
    assert_ne!(expected, digest(9, &[1, 2], binding, geometry));
    assert_ne!(expected, digest(2, &[1, 3], binding, geometry));
    for changed in [
        BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: 4,
                ..binding.region
            },
            ..binding
        },
        BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                access: RuntimeAccessV1::Write,
                ..binding.region
            },
            ..binding
        },
        BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                byte_offset: 9,
                ..binding.region
            },
            ..binding
        },
        BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                byte_len: 17,
                ..binding.region
            },
            ..binding
        },
        BackendBindingV1 {
            kernarg_byte_offset: 25,
            ..binding
        },
    ] {
        assert_ne!(expected, digest(2, &[1, 2], changed, geometry));
    }
    for changed in [
        crate::RuntimeLaunchGeometryV1 {
            grid: [9, 1, 1],
            ..geometry
        },
        crate::RuntimeLaunchGeometryV1 {
            workgroup: [2, 1, 1],
            ..geometry
        },
        crate::RuntimeLaunchGeometryV1 {
            dynamic_shared_bytes: 17,
            ..geometry
        },
    ] {
        assert_ne!(expected, digest(2, &[1, 2], binding, changed));
    }

    let descriptor = ResidentDataDescriptorV1 {
        allocation: 3,
        kind: RuntimeMemoryKindV1::HostVisible,
        alignment: 8,
        allocation_offset: 0,
        byte_len: 32,
        host_content_sha256: None,
        device_may_have_modified: false,
    };
    for changed in [
        ResidentDataDescriptorV1 {
            allocation: 4,
            ..descriptor
        },
        ResidentDataDescriptorV1 {
            kind: RuntimeMemoryKindV1::DeviceLocal,
            ..descriptor
        },
        ResidentDataDescriptorV1 {
            alignment: 16,
            ..descriptor
        },
        ResidentDataDescriptorV1 {
            allocation_offset: 8,
            ..descriptor
        },
        ResidentDataDescriptorV1 {
            byte_len: 64,
            ..descriptor
        },
    ] {
        assert!(!same_resident_storage_shape_v1(&[descriptor], &[changed]));
    }
}

#[test]
fn ordinary_pipeline_recipe_requires_exact_field_equality_not_only_a_digest() {
    let base = OwnedComputeLaunchV1 {
        stream: 1,
        kernel: 2,
        explicit_kernarg: vec![3, 4].into_boxed_slice(),
        bindings: vec![BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: 5,
                access: RuntimeAccessV1::Read,
                byte_offset: 8,
                byte_len: 16,
            },
            kernarg_byte_offset: 24,
        }]
        .into_boxed_slice(),
        geometry: crate::RuntimeLaunchGeometryV1 {
            grid: [8, 1, 1],
            workgroup: [4, 1, 1],
            dynamic_shared_bytes: 16,
        },
        semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
    };
    assert!(ordinary_compute_recipes_match_v1(&base, &base));
    let mut changed = Vec::new();
    let mut recipe = base.unaccounted_copy_for_test();
    recipe.stream = 9;
    changed.push(recipe);
    let mut recipe = base.unaccounted_copy_for_test();
    recipe.kernel = 9;
    changed.push(recipe);
    let mut recipe = base.unaccounted_copy_for_test();
    recipe.explicit_kernarg[1] = 9;
    changed.push(recipe);
    for binding in [
        BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: 9,
                ..base.bindings[0].region
            },
            ..base.bindings[0]
        },
        BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                access: RuntimeAccessV1::Write,
                ..base.bindings[0].region
            },
            ..base.bindings[0]
        },
        BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                byte_offset: 9,
                ..base.bindings[0].region
            },
            ..base.bindings[0]
        },
        BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                byte_len: 17,
                ..base.bindings[0].region
            },
            ..base.bindings[0]
        },
        BackendBindingV1 {
            kernarg_byte_offset: 25,
            ..base.bindings[0]
        },
    ] {
        let mut recipe = base.unaccounted_copy_for_test();
        recipe.bindings[0] = binding;
        changed.push(recipe);
    }
    for geometry in [
        crate::RuntimeLaunchGeometryV1 {
            grid: [9, 1, 1],
            ..base.geometry
        },
        crate::RuntimeLaunchGeometryV1 {
            workgroup: [2, 1, 1],
            ..base.geometry
        },
        crate::RuntimeLaunchGeometryV1 {
            dynamic_shared_bytes: 17,
            ..base.geometry
        },
    ] {
        let mut recipe = base.unaccounted_copy_for_test();
        recipe.geometry = geometry;
        changed.push(recipe);
    }
    let mut recipe = base.unaccounted_copy_for_test();
    recipe.semantic_launch = KfdRuntimeSemanticLaunchV1::Atomic(atomic_contract_v1());
    changed.push(recipe);
    assert!(
        changed
            .iter()
            .all(|changed| !ordinary_compute_recipes_match_v1(&base, changed))
    );
}

#[test]
fn later_chunk_rejection_is_quiescent_after_prior_device_publication() {
    let rejected = || {
        RuntimeBackendFailureV1::Rejected(KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::Native,
            "injected recovered rejection",
        ))
    };
    assert!(matches!(
        classify_sdma_chunk_failure_v1(0, rejected()),
        RuntimeBackendFailureV1::Rejected(_)
    ));
    assert!(matches!(
        classify_sdma_chunk_failure_v1(1, rejected()),
        RuntimeBackendFailureV1::Quiescent(_)
    ));
}

#[test]
fn generated_readiness_observes_both_active_and_pipeline_lanes_without_progress() {
    let mut backend = KfdRuntimeBackendV1::mock();
    for pipelined in [false, true] {
        if pipelined {
            backend
                .compute_pipeline
                .insert_published(pipelined_active_for_test_v1(10))
                .unwrap();
            backend.auxiliary_compute_lanes[0]
                .pipeline
                .insert_published(pipelined_active_for_test_v1(11))
                .unwrap();
        } else {
            backend.active = Some(pipelined_active_for_test_v1(10));
            backend.auxiliary_compute_lanes[0].active = Some(pipelined_active_for_test_v1(11));
        }
        for _ in 0..3 {
            assert!(!backend.generated_lane_ready_v1().unwrap());
            assert!(!backend.terminal);
            assert_eq!(backend.active_compute_lane_v1(10), Some(0));
            assert_eq!(backend.active_compute_lane_v1(11), Some(1));
        }
        if pipelined {
            backend.compute_pipeline.take_commit_frontier().unwrap();
        } else {
            backend.active = None;
        }
        assert!(backend.generated_lane_ready_v1().unwrap());
        assert_eq!(backend.active_compute_lane_v1(11), Some(1));
        if pipelined {
            backend.auxiliary_compute_lanes[0]
                .pipeline
                .take_commit_frontier()
                .unwrap();
        } else {
            backend.auxiliary_compute_lanes[0].active = None;
        }
    }
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn host_visible_write_cache_retention_requires_full_write_and_global_compute_quiescence() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let host = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 16, 8)
        .unwrap();
    let device = backend
        .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
        .unwrap();
    assert!(backend.can_retain_host_visible_write_cache_v1(host, true));
    assert!(!backend.can_retain_host_visible_write_cache_v1(host, false));
    assert!(!backend.can_retain_host_visible_write_cache_v1(device, true));
    assert!(!backend.can_retain_host_visible_write_cache_v1(u64::MAX, true));
    backend.active = Some(pipelined_active_for_test_v1(10));
    assert!(!backend.can_retain_host_visible_write_cache_v1(host, true));
    backend.active = None;
    backend.auxiliary_compute_lanes[0].active = Some(pipelined_active_for_test_v1(11));
    assert!(!backend.can_retain_host_visible_write_cache_v1(host, true));
    backend.auxiliary_compute_lanes[0].active = None;
    assert!(
        backend
            .compute_pipeline
            .insert_published(pipelined_active_for_test_v1(12))
            .is_ok()
    );
    assert!(!backend.can_retain_host_visible_write_cache_v1(host, true));
    backend.compute_pipeline = RuntimeComputePipelineV1::vacant();
    assert!(
        backend.auxiliary_compute_lanes[0]
            .pipeline
            .insert_published(pipelined_active_for_test_v1(13))
            .is_ok()
    );
    assert!(!backend.can_retain_host_visible_write_cache_v1(host, true));
    backend.auxiliary_compute_lanes[0].pipeline = RuntimeComputePipelineV1::vacant();
    backend.retained_persistent_dispatch = Some(RetainedPersistentDispatchV1 {
        allocation: device,
        dispatch_shape_sha256: [1; 32],
    });
    assert!(!backend.can_retain_host_visible_write_cache_v1(host, true));
    backend.retained_persistent_dispatch = None;
    assert!(backend.can_retain_host_visible_write_cache_v1(host, true));
    backend.write_allocation_v1(host, 0, &[0x60; 16]).unwrap();
    backend.write_allocation_v1(host, 0, &[0x60; 16]).unwrap();
    backend.release_allocation_v1(device).unwrap();
    backend.release_allocation_v1(host).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn host_visible_write_cache_retention_rejects_mixed_empty_or_incomplete_rosters() {
    let descriptor = ResidentDataDescriptorV1 {
        allocation: 1,
        kind: RuntimeMemoryKindV1::HostVisible,
        alignment: 8,
        allocation_offset: 0,
        byte_len: 16,
        host_content_sha256: Some([1; 32]),
        device_may_have_modified: false,
    };
    let descriptors = [descriptor; 3];
    assert!(host_visible_resident_roster_is_reusable_v1(&descriptors, 3));
    for count in [0, 1, 2, 4] {
        assert!(!host_visible_resident_roster_is_reusable_v1(
            &descriptors,
            count
        ));
    }
    assert!(!host_visible_resident_roster_is_reusable_v1(&[], 0));
    for mask in 1..8 {
        let mut mixed = descriptors;
        for (index, descriptor) in mixed.iter_mut().enumerate() {
            if mask & (1 << index) != 0 {
                descriptor.kind = RuntimeMemoryKindV1::DeviceLocal;
            }
        }
        assert!(!host_visible_resident_roster_is_reusable_v1(&mixed, 3));
    }
    let empty = ResidentDataDescriptorV1 {
        byte_len: 0,
        ..descriptor
    };
    assert!(!host_visible_resident_roster_is_reusable_v1(&[empty], 1));
}

#[test]
fn host_visible_write_cache_retention_keeps_old_native_digest_until_checked_overwrite() {
    let prior = ResidentDataDescriptorV1 {
        allocation: 1,
        kind: RuntimeMemoryKindV1::HostVisible,
        alignment: 8,
        allocation_offset: 0,
        byte_len: 16,
        host_content_sha256: Some([1; 32]),
        device_may_have_modified: false,
    };
    assert!(!resident_data_needs_host_overwrite_v1(
        &prior,
        Some([1; 32])
    ));
    assert!(resident_data_needs_host_overwrite_v1(&prior, Some([2; 32])));
    assert!(resident_data_needs_host_overwrite_v1(&prior, None));
    let written = ResidentDataDescriptorV1 {
        device_may_have_modified: true,
        ..prior
    };
    assert!(resident_data_needs_host_overwrite_v1(
        &written,
        Some([1; 32])
    ));
    let unknown = ResidentDataDescriptorV1 {
        host_content_sha256: None,
        ..prior
    };
    assert!(resident_data_needs_host_overwrite_v1(&unknown, None));
    assert!(resident_data_needs_host_overwrite_v1(
        &unknown,
        Some([1; 32])
    ));
    assert_eq!(prior.host_content_sha256, Some([1; 32]));
}

#[test]
fn host_visible_recycled_control_retention_policy_requires_one_nonempty_host_owner() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let descriptor = ResidentDataDescriptorV1 {
        allocation: 1,
        kind: RuntimeMemoryKindV1::HostVisible,
        alignment: 8,
        allocation_offset: 0,
        byte_len: 16,
        host_content_sha256: Some([1; 32]),
        device_may_have_modified: true,
    };
    assert!(!backend.can_retain_host_visible_recycled_control_v1());
    backend.recycled_dispatch = Some(RecycledDispatchV1 {
        kernel: 1,
        dispatch_shape_sha256: [2; 32],
        descriptors: vec![descriptor],
    });
    assert!(backend.can_retain_host_visible_recycled_control_v1());
    backend.resident_data = Some(ResidentDataRosterV1 {
        descriptors: vec![],
        data: vec![],
    });
    assert!(!backend.can_retain_host_visible_recycled_control_v1());
    backend.resident_data = None;
    for invalid in [
        vec![],
        vec![ResidentDataDescriptorV1 {
            byte_len: 0,
            ..descriptor
        }],
        vec![
            descriptor,
            ResidentDataDescriptorV1 {
                kind: RuntimeMemoryKindV1::DeviceLocal,
                ..descriptor
            },
        ],
    ] {
        backend.recycled_dispatch.as_mut().unwrap().descriptors = invalid;
        assert!(!backend.can_retain_host_visible_recycled_control_v1());
    }
    // This policy fixture never creates a native control owner.
    backend.recycled_dispatch = None;
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn host_visible_recycled_control_retention_wiring_preserves_checked_reconciliation() {
    let source = include_str!("../../kfd_backend.rs");
    let prepare = source
        .split("fn prepare_compute_caches_for_host_write_v1(")
        .nth(1)
        .unwrap()
        .split("fn can_retain_host_visible_recycled_control_v1(")
        .next()
        .unwrap();
    let mut remaining = prepare;
    for fragment in [
        "can_retain_host_visible_write_cache_v1(allocation, full_write)",
        "can_retain_host_visible_recycled_control_v1()",
        "queue.recycled_fixed_dispatch_generation()",
        "validation.map_err(|detail| backend.terminal_error(detail))?;",
        "backend.synchronize_recycled_dispatch_data_v1()?;",
        "return Ok(());",
        "backend.detach_recycled_dispatch()?;",
    ] {
        remaining = remaining.split_once(fragment).unwrap().1;
    }
    let dispatch = include_str!("../compute_dispatch/resident_release.rs");
    let detach = dispatch
        .split("fn detach_recycled_dispatch(")
        .nth(1)
        .unwrap()
        .split("fn release_resident_data(")
        .next()
        .unwrap();
    let normalized: String = detach.chars().filter(|ch| !ch.is_whitespace()).collect();
    let mut remaining = normalized.as_str();
    for fragment in [
        "self.synchronize_recycled_dispatch_data_v1()?;",
        "ifself.resident_data.is_some()",
        "letrecycled=&mutself.recycled_dispatch;",
        ".with_compute_lane_v1(native_lane,|queue|{",
        "queue.detach_recycled_fixed_dispatch()",
        "*resident=Some(ResidentDataRosterV1{",
        "recycled.take()",
        "data:detached.into_data()",
        "Ok(())",
    ] {
        remaining = remaining.split_once(fragment).unwrap().1;
    }
}

#[test]
fn runtime_compute_pipeline_capacity_is_fixed_and_slot_identity_is_aba_safe() {
    let mut pipeline = RuntimeComputePipelineV1::vacant();
    for id in 2..=RUNTIME_COMPUTE_PIPELINE_CAPACITY_V1 as u64 {
        assert!(
            pipeline
                .insert_published(pipelined_active_for_test_v1(id))
                .is_ok()
        );
    }
    assert_eq!(pipeline.len(), RUNTIME_COMPUTE_PIPELINE_CAPACITY_V1 - 1);
    assert!(!pipeline.has_successor_capacity());
    assert!(
        pipeline
            .insert_published(pipelined_active_for_test_v1(65))
            .is_err()
    );

    let stale = pipeline.identity_for_submission_v1(2).unwrap();
    let (phase, retired) = pipeline.take_commit_frontier().unwrap();
    assert_eq!(phase, RuntimeComputePipelinePhaseV1::Published);
    assert_eq!(retired.id, 2);
    let fresh = pipeline
        .insert_published(pipelined_active_for_test_v1(66))
        .unwrap();
    assert_ne!(stale, fresh);
    assert!(
        pipeline
            .restore(
                stale,
                RuntimeComputePipelinePhaseV1::PhysicallyRetired,
                pipelined_active_for_test_v1(67),
            )
            .is_err()
    );
    let (fresh, owner) = pipeline.take_physical_owner(66).unwrap();
    let substituted = pipeline
        .restore(
            fresh,
            RuntimeComputePipelinePhaseV1::Published,
            pipelined_active_for_test_v1(67),
        )
        .expect_err("a fresh slot capability is bound to one submission");
    assert_eq!(substituted.id, 67);
    pipeline
        .restore(fresh, RuntimeComputePipelinePhaseV1::Published, owner)
        .unwrap();

    let mut duplicate = RuntimeComputePipelineV1::vacant();
    duplicate
        .insert_published(pipelined_active_for_test_v1(2))
        .unwrap();
    assert!(
        duplicate
            .insert_published(pipelined_active_for_test_v1(2))
            .is_err()
    );
}

#[test]
fn runtime_compute_pipeline_identity_exhaustion_never_wraps() {
    let mut exhausted_slots = RuntimeComputePipelineV1::vacant();
    exhausted_slots.exhaust_vacant_identities_for_test_v1();
    assert!(!exhausted_slots.has_successor_capacity());
    assert!(
        exhausted_slots
            .insert_published(pipelined_active_for_test_v1(2))
            .is_err()
    );

    let mut exhausted_epochs = RuntimeComputePipelineV1::vacant();
    exhausted_epochs.exhaust_logical_epochs_for_test_v1();
    assert!(!exhausted_epochs.has_successor_capacity());
    assert!(
        exhausted_epochs
            .insert_published(pipelined_active_for_test_v1(2))
            .is_err()
    );
}

#[test]
fn runtime_compute_pipeline_recycles_out_of_order_but_commits_contiguously() {
    let mut pipeline = RuntimeComputePipelineV1::vacant();
    for id in 2..=4 {
        pipeline
            .insert_published(pipelined_active_for_test_v1(id))
            .unwrap();
    }
    let (four, active) = pipeline.take_physical_owner(4).unwrap();
    pipeline
        .restore(
            four,
            RuntimeComputePipelinePhaseV1::PhysicallyRetired,
            active,
        )
        .unwrap();

    let (phase, active) = pipeline.take_commit_frontier().unwrap();
    assert_eq!(phase, RuntimeComputePipelinePhaseV1::Published);
    assert_eq!(active.id, 2);
    let (three, active) = pipeline.take_physical_owner(3).unwrap();
    pipeline
        .restore(
            three,
            RuntimeComputePipelinePhaseV1::PhysicallyRetired,
            active,
        )
        .unwrap();
    let (phase, active) = pipeline.take_commit_frontier().unwrap();
    assert_eq!(phase, RuntimeComputePipelinePhaseV1::PhysicallyRetired);
    assert_eq!(active.id, 3);
    let (phase, active) = pipeline.take_commit_frontier().unwrap();
    assert_eq!(phase, RuntimeComputePipelinePhaseV1::PhysicallyRetired);
    assert_eq!(active.id, 4);
    assert!(pipeline.is_empty());
}
