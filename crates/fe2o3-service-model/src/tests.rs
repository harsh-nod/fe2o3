use alloc::{vec, vec::Vec};

use super::*;

fn digest(seed: u8) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes([seed; IDENTITY_DIGEST_BYTES_V1])
}

fn task_schema() -> TaskSchemaInputV1 {
    TaskSchemaInputV1::new(
        vec![
            task_variant(0, 10),
            task_variant(4, 30),
            task_variant(9, 50),
        ],
        digest(70),
    )
    .unwrap()
}

fn task_variant(tag: u32, seed: u8) -> TaskVariantInputV1 {
    TaskVariantInputV1 {
        canonical_tag: tag,
        variant_name_identity: digest(seed),
        payload_abi_and_layout_identity: digest(seed + 1),
        payload_lifetime_and_region_contract_id: digest(seed + 2),
        handler_algorithm_id: digest(seed + 3),
        handler_numerical_contract_id: digest(seed + 4),
        handler_contract_id: digest(seed + 5),
        handler_effect_and_capability_closure_id: digest(seed + 6),
        cancellation_contract_id: digest(seed + 7),
        unsafe_or_external_obligations_id: digest(seed + 8),
    }
}

fn scheduler() -> SchedulerModelInputV1 {
    SchedulerModelInputV1 {
        queue_model_id: digest(71),
        queue_capacity: 8,
        generation_modulus: 256,
        maximum_live_generation_span: 32,
        queue_discipline: QueueDisciplineV1::FifoBatch { maximum_batch: 4 },
        delivery_policy: DeliveryPolicyV1::AtMostOnce,
        dependency_epoch_model_id: digest(72),
        lifecycle_policy: LifecyclePolicyV1::DrainThenStop,
        cancellation_policy_id: digest(73),
        failure_model_id: digest(74),
        synchronization_contract_id: digest(75),
        progress_contract_id: None,
    }
}

fn fusion_plan() -> FusionPlanInputV1 {
    FusionPlanInputV1 {
        authoritative_dispatch_graph_id: digest(80),
        nodes_in_graph_order: vec![digest(81), digest(82), digest(83)],
        edges: vec![
            FusionEdgeInputV1 {
                source_node_index: 0,
                target_node_index: 1,
                edge_identity: digest(84),
            },
            FusionEdgeInputV1 {
                source_node_index: 1,
                target_node_index: 2,
                edge_identity: digest(85),
            },
        ],
        phases: vec![
            FusionPhaseInputV1 {
                phase_identity: digest(86),
                first_node_index: 0,
                node_count: 2,
            },
            FusionPhaseInputV1 {
                phase_identity: digest(87),
                first_node_index: 2,
                node_count: 1,
            },
        ],
        materialized_values: vec![digest(88)],
        effect_dependency_origin_map_id: digest(89),
        layout_and_region_choices_id: digest(90),
        barrier_and_convergence_contract_id: digest(91),
        numerical_order_contract_id: digest(92),
        schedule_parameters_id: digest(93),
        legality_rule_set_id: digest(94),
        transformation_receipt_schema_id: digest(95),
    }
}

fn persistent_plan() -> PersistentPlanInputV1 {
    PersistentPlanInputV1 {
        task_schema_id: TaskSchemaIdV1::from_untrusted_digest(digest(100)),
        scheduler_model_id: SchedulerModelIdV1::from_untrusted_digest(digest(101)),
        worker_roles: vec![WorkerRoleInputV1 {
            role_identity: digest(102),
            minimum_workers: 1,
            maximum_workers: 64,
            state_partition_id: digest(103),
        }],
        resident_workgroups: 8,
        resident_waves: 16,
        queue_and_state_resource_plan_id: digest(104),
        launch_and_cooperation_contract_id: digest(105),
        handler_plan_references: vec![
            HandlerPlanReferenceV1 {
                task_tag: 0,
                handler_algorithm_id: digest(13),
                fusion_plan_id: None,
            },
            HandlerPlanReferenceV1 {
                task_tag: 4,
                handler_algorithm_id: digest(33),
                fusion_plan_id: Some(FusionPlanIdV1::from_untrusted_digest(digest(108))),
            },
            HandlerPlanReferenceV1 {
                task_tag: 9,
                handler_algorithm_id: digest(53),
                fusion_plan_id: None,
            },
        ],
        drain_stop_failure_policy_id: digest(110),
        resource_contract_id: digest(111),
    }
}

#[test]
fn identity_preimages_are_deterministic_domain_separated_and_mutation_sensitive() {
    let schema = task_schema();
    let first = schema.encode_canonical_preimage();
    assert_eq!(first, schema.encode_canonical_preimage());
    assert_eq!(&first[..8], b"F2TASKS1");

    let mut mutated_variants = schema.variants().to_vec();
    mutated_variants[1].handler_contract_id = digest(112);
    let mutated = TaskSchemaInputV1::new(mutated_variants, schema.schema_failure_policy_id())
        .unwrap()
        .encode_canonical_preimage();
    assert_ne!(first, mutated);

    let scheduler_bytes = scheduler().encode_canonical_preimage().unwrap();
    assert_ne!(&first[..8], &scheduler_bytes[..8]);
    assert!(first.len() < 4_096);
    assert!(scheduler_bytes.len() < 1_024);
}

#[test]
fn task_schema_rejects_unknown_order_duplicate_tags_and_unbounded_families() {
    assert_eq!(
        TaskSchemaInputV1::new(vec![task_variant(2, 1), task_variant(1, 11)], digest(20)),
        Err(IdentityInputErrorV1::NonCanonicalTaskTagOrder)
    );
    assert_eq!(
        TaskSchemaInputV1::new(vec![task_variant(2, 1), task_variant(2, 11)], digest(20)),
        Err(IdentityInputErrorV1::DuplicateTaskTag(2))
    );
    let variants: Vec<_> = (0..=MAX_TASK_VARIANTS_V1)
        .map(|index| task_variant(index as u32, 1 + (index % 20) as u8))
        .collect();
    assert!(matches!(
        TaskSchemaInputV1::new(variants, digest(20)),
        Err(IdentityInputErrorV1::TooManyItems { .. })
    ));
    assert_eq!(
        IdentityDigestV1::from_untrusted_bytes([0; IDENTITY_DIGEST_BYTES_V1]).as_bytes(),
        &[0; IDENTITY_DIGEST_BYTES_V1]
    );
}

#[test]
fn scheduler_rejects_invalid_capacity_batch_and_generation_wrap_domain() {
    let mut model = scheduler();
    model.queue_capacity = 0;
    assert_eq!(
        model.validate(),
        Err(IdentityInputErrorV1::InvalidQueueCapacity(0))
    );
    model = scheduler();
    model.queue_discipline = QueueDisciplineV1::FifoBatch { maximum_batch: 9 };
    assert_eq!(
        model.validate(),
        Err(IdentityInputErrorV1::InvalidQueueCapacity(9))
    );
    model = scheduler();
    model.maximum_live_generation_span = model.generation_modulus;
    assert_eq!(
        model.validate(),
        Err(IdentityInputErrorV1::InvalidGenerationDomain)
    );
}

#[test]
fn fusion_plan_checks_graph_edges_and_exact_phase_partition() {
    let plan = fusion_plan();
    let bytes = plan.encode_canonical_preimage().unwrap();
    assert_eq!(&bytes[..8], b"F2FUSEP1");

    let mut invalid = plan.clone();
    invalid.edges[0].target_node_index = 9;
    assert_eq!(
        invalid.validate(),
        Err(IdentityInputErrorV1::InvalidFusionEdge)
    );

    let mut invalid = plan.clone();
    invalid.phases[1].first_node_index = 1;
    assert_eq!(
        invalid.validate(),
        Err(IdentityInputErrorV1::InvalidPhasePartition)
    );

    let mut invalid = plan;
    invalid.nodes_in_graph_order[2] = invalid.nodes_in_graph_order[0];
    assert_eq!(
        invalid.validate(),
        Err(IdentityInputErrorV1::DuplicateFusionNode)
    );
}

#[test]
fn identity_collections_reject_noncanonical_order() {
    let mut fusion = fusion_plan();
    fusion.edges.swap(0, 1);
    assert_eq!(
        fusion.validate(),
        Err(IdentityInputErrorV1::NonCanonicalFusionEdgeOrder)
    );

    let mut plan = persistent_plan();
    plan.worker_roles = vec![
        WorkerRoleInputV1 {
            role_identity: digest(120),
            minimum_workers: 1,
            maximum_workers: 2,
            state_partition_id: digest(121),
        },
        WorkerRoleInputV1 {
            role_identity: digest(119),
            minimum_workers: 1,
            maximum_workers: 2,
            state_partition_id: digest(122),
        },
    ];
    assert_eq!(
        plan.validate(),
        Err(IdentityInputErrorV1::NonCanonicalWorkerRoleOrder)
    );

    let mut plan = persistent_plan();
    plan.handler_plan_references.swap(0, 1);
    assert_eq!(
        plan.validate(),
        Err(IdentityInputErrorV1::NonCanonicalHandlerOrder)
    );
}

#[test]
fn persistent_plan_binds_every_schema_tag_once() {
    let plan = persistent_plan();
    plan.validate_against_schema(&task_schema()).unwrap();
    assert_eq!(&plan.encode_canonical_preimage().unwrap()[..8], b"F2PERST1");

    let mut missing = plan.clone();
    missing.handler_plan_references.pop();
    assert!(matches!(
        missing.validate_against_schema(&task_schema()),
        Err(IdentityInputErrorV1::MissingHandlerTag(_))
    ));

    let mut duplicate = plan;
    duplicate.handler_plan_references[2].task_tag = 4;
    assert_eq!(
        duplicate.validate(),
        Err(IdentityInputErrorV1::DuplicateHandlerTag(4))
    );

    let mut mismatch = persistent_plan();
    mismatch.handler_plan_references[1].handler_algorithm_id = digest(200);
    assert_eq!(
        mismatch.validate_against_schema(&task_schema()),
        Err(IdentityInputErrorV1::HandlerIdentityMismatch(4))
    );
}

#[test]
fn service_executable_and_run_inputs_are_bounded_descriptions_only() {
    let executable = ServiceExecutableInputV1 {
        persistent_plan_id: PersistentPlanIdV1::from_untrusted_digest(digest(120)),
        target_plan_id: digest(121),
        launch_contract_id: digest(122),
        compiler_and_toolchain_id: digest(123),
        llvm_module_id: digest(124),
        object_id: digest(125),
        hsaco_id: digest(126),
        resource_and_origin_map_id: digest(127),
    };
    assert_eq!(&executable.encode_canonical_preimage()[..8], b"F2SVEXE1");

    let run = ServiceRunInputV1 {
        executable_id: ServiceExecutableIdV1::from_untrusted_digest(digest(128)),
        physical_device_id: digest(129),
        runtime_context_id: digest(130),
        stream_or_queue_id: digest(131),
        service_epoch: 7,
        allocations: vec![
            allocation(AllocationRoleV1::Queue, 0, 132),
            allocation(AllocationRoleV1::State, 0, 133),
            allocation(AllocationRoleV1::Input, 0, 134),
            allocation(AllocationRoleV1::Output, 0, 135),
        ],
        launch_instance_id: digest(136),
    };
    assert_eq!(&run.encode_canonical_preimage().unwrap()[..8], b"F2SVRUN1");

    let mut reordered = run;
    reordered.allocations.swap(0, 1);
    assert_eq!(
        reordered.validate(),
        Err(IdentityInputErrorV1::NonCanonicalAllocationOrder)
    );
}

fn allocation(role: AllocationRoleV1, ordinal: u16, seed: u8) -> AllocationBindingInputV1 {
    AllocationBindingInputV1 {
        role,
        ordinal,
        allocation_identity: digest(seed),
        allocation_epoch: u64::from(seed),
    }
}

fn model_config() -> ServiceModelConfigV1 {
    ServiceModelConfigV1 {
        run_id: ServiceRunIdV1::from_untrusted_digest(digest(140)),
        service_epoch: 5,
        queue_identity: digest(141),
        task_schema_id: TaskSchemaIdV1::from_untrusted_digest(digest(142)),
        scheduler_model_id: SchedulerModelIdV1::from_untrusted_digest(digest(143)),
        admitted_task_tags: vec![0, 4, 9],
        queue_capacity: 2,
        generation_modulus: 16,
        maximum_live_generation_span: 4,
        delivery_policy: DeliveryPolicyV1::AtMostOnce,
        failure_model_id: digest(144),
    }
}

fn initial_state() -> ServiceStateV1 {
    ServiceStateV1 {
        config: model_config(),
        lifecycle: LifecycleStateV1::Starting,
        admission_cutoff: None,
        slots: vec![
            QueueSlotRecordV1 {
                slot_id: SlotIdV1(0),
                generation: GenerationStateV1::Current(GenerationCounterV1 {
                    logical: 0,
                    encoded: 0,
                }),
                aba: AbaStateV1::Protected {
                    oldest_live_generation: None,
                },
                state: QueueSlotStateV1::Empty { generation: 0 },
            },
            QueueSlotRecordV1 {
                slot_id: SlotIdV1(1),
                generation: GenerationStateV1::Current(GenerationCounterV1 {
                    logical: 0,
                    encoded: 0,
                }),
                aba: AbaStateV1::Protected {
                    oldest_live_generation: None,
                },
                state: QueueSlotStateV1::Empty { generation: 0 },
            },
        ],
        tasks: vec![],
        leases: vec![],
        workers: vec![WorkerRecordV1 {
            worker_id: WorkerIdV1(0),
            state: WorkerStateV1::Starting,
        }],
        dependencies: vec![],
        phase_regions: vec![PhaseRegionRecordV1 {
            region_id: RegionIdV1(0),
            state: PhaseStateV1::Inactive { epoch: 0 },
        }],
        completion_records: vec![],
        failure: None,
    }
}

fn assert_valid(state: &ServiceStateV1) {
    if let Err(report) = state.validate_global_invariants() {
        panic!("invalid fixture: {:?}", report.violations());
    }
}

fn advance(current: &ServiceStateV1, next: &ServiceStateV1) {
    assert_valid(current);
    assert_valid(next);
    current.validate_transition_to(next).unwrap();
}

#[test]
fn full_normal_trace_preserves_invariants_through_drain_and_stop() {
    let starting = initial_state();
    assert_valid(&starting);

    let mut running = starting.clone();
    running.lifecycle = LifecycleStateV1::Running;
    running.workers[0].state = WorkerStateV1::Idle;
    advance(&starting, &running);

    let key = running.config.slot_key(SlotIdV1(0), 0);
    let task_id = TaskIdV1(10);
    let lease_id = LeaseIdV1(20);
    let record_id = CompletionRecordIdV1(30);
    let mut reserved = running.clone();
    reserved.slots[0].state = QueueSlotStateV1::Reserved {
        generation: 0,
        task_id,
    };
    reserved.tasks.push(TaskRecordV1 {
        task_id,
        canonical_tag: 4,
        payload_identity: digest(145),
        submission_sequence: 1,
        dependencies: vec![],
        state: TaskStateV1::Reserved(key),
    });
    advance(&running, &reserved);

    let mut initialized = reserved.clone();
    initialized.slots[0].state = QueueSlotStateV1::Initialized {
        generation: 0,
        task_id,
    };
    initialized.tasks[0].state = TaskStateV1::Initialized(key);
    advance(&reserved, &initialized);

    let mut published = initialized.clone();
    published.slots[0].state = QueueSlotStateV1::Published {
        generation: 0,
        task_id,
    };
    published.tasks[0].state = TaskStateV1::Published(key);
    advance(&initialized, &published);

    let mut acquiring = published.clone();
    acquiring.workers[0].state = WorkerStateV1::Acquiring;
    advance(&published, &acquiring);

    let lease_key = LeaseKeyV1 {
        slot: key,
        task_id,
        acquisition_event: AcquisitionEventIdV1(40),
        worker_id: WorkerIdV1(0),
    };
    let mut acquired = acquiring.clone();
    acquired.slots[0].state = QueueSlotStateV1::Acquired {
        generation: 0,
        task_id,
        lease_id,
    };
    acquired.tasks[0].state = TaskStateV1::Acquired {
        slot: key,
        lease_id,
    };
    acquired.leases.push(LeaseRecordV1 {
        lease_id,
        state: LeaseStateV1::Issued(lease_key),
    });
    advance(&acquiring, &acquired);

    let mut executing = acquired.clone();
    executing.slots[0].state = QueueSlotStateV1::Executing {
        generation: 0,
        task_id,
        lease_id,
    };
    executing.tasks[0].state = TaskStateV1::Executing {
        slot: key,
        lease_id,
        phase_id: PhaseIdV1(1),
    };
    executing.leases[0].state = LeaseStateV1::Executing(lease_key);
    executing.workers[0].state = WorkerStateV1::Running {
        task_id,
        lease_id,
        phase_id: PhaseIdV1(1),
    };
    executing.phase_regions[0].state = PhaseStateV1::Active {
        epoch: 0,
        phase_id: PhaseIdV1(1),
        owner: PhaseOwnerV1::Worker(WorkerIdV1(0)),
    };
    advance(&acquired, &executing);

    let mut pending = executing.clone();
    pending.tasks[0].state = TaskStateV1::CompletionPending {
        slot: key,
        lease_id,
        outcome: TaskOutcomeV1::Succeeded,
    };
    advance(&executing, &pending);

    let mut completed = pending.clone();
    completed.slots[0].state = QueueSlotStateV1::Completed {
        generation: 0,
        task_id,
        outcome: TaskOutcomeV1::Succeeded,
        record_id,
    };
    completed.tasks[0].state = TaskStateV1::Completed {
        slot: key,
        record_id,
    };
    completed.leases[0].state = LeaseStateV1::Consumed {
        key: lease_key,
        outcome: TaskOutcomeV1::Succeeded,
        record_id,
    };
    completed.workers[0].state = WorkerStateV1::Publishing { task_id, lease_id };
    completed.phase_regions[0].state = PhaseStateV1::Completed {
        epoch: 0,
        phase_id: PhaseIdV1(1),
        owner: PhaseOwnerV1::Worker(WorkerIdV1(0)),
    };
    completed.completion_records.push(CompletionRecordV1 {
        record_id,
        task_id,
        slot: key,
        outcome: TaskOutcomeV1::Succeeded,
        visible: true,
    });
    advance(&pending, &completed);

    let mut reclaimable = completed.clone();
    reclaimable.slots[0].state = QueueSlotStateV1::Reclaimable {
        generation: 0,
        task_id,
        outcome: TaskOutcomeV1::Succeeded,
        record_id,
    };
    reclaimable.workers[0].state = WorkerStateV1::Idle;
    reclaimable.phase_regions[0].state = PhaseStateV1::Inactive { epoch: 1 };
    advance(&completed, &reclaimable);

    let mut reclaimed = reclaimable.clone();
    reclaimed.slots[0].state = QueueSlotStateV1::Empty { generation: 1 };
    reclaimed.slots[0].generation = GenerationStateV1::Current(GenerationCounterV1 {
        logical: 1,
        encoded: 1,
    });
    advance(&reclaimable, &reclaimed);

    let mut draining = reclaimed.clone();
    draining.lifecycle = LifecycleStateV1::Draining;
    draining.admission_cutoff = Some(1);
    advance(&reclaimed, &draining);

    let mut stopping = draining.clone();
    stopping.lifecycle = LifecycleStateV1::Stopping;
    stopping.workers[0].state = WorkerStateV1::Exiting;
    advance(&draining, &stopping);

    let mut stopped = stopping.clone();
    stopped.lifecycle = LifecycleStateV1::Stopped;
    stopped.workers[0].state = WorkerStateV1::Exited;
    advance(&stopping, &stopped);
    assert!(stopped.is_quiescent());
}

#[test]
fn cancellation_before_publication_has_one_terminal_record_and_no_lease() {
    let mut state = initial_state();
    state.lifecycle = LifecycleStateV1::Running;
    state.workers[0].state = WorkerStateV1::Idle;
    let key = state.config.slot_key(SlotIdV1(0), 0);
    state.slots[0].state = QueueSlotStateV1::Reserved {
        generation: 0,
        task_id: TaskIdV1(1),
    };
    state.tasks.push(TaskRecordV1 {
        task_id: TaskIdV1(1),
        canonical_tag: 0,
        payload_identity: digest(146),
        submission_sequence: 1,
        dependencies: vec![],
        state: TaskStateV1::Reserved(key),
    });
    assert_valid(&state);

    let mut cancelled = state.clone();
    cancelled.slots[0].state = QueueSlotStateV1::Completed {
        generation: 0,
        task_id: TaskIdV1(1),
        outcome: TaskOutcomeV1::Cancelled,
        record_id: CompletionRecordIdV1(1),
    };
    cancelled.tasks[0].state = TaskStateV1::Cancelled {
        slot: key,
        stage: CancellationStageV1::Reserved,
        record_id: CompletionRecordIdV1(1),
    };
    cancelled.completion_records.push(CompletionRecordV1 {
        record_id: CompletionRecordIdV1(1),
        task_id: TaskIdV1(1),
        slot: key,
        outcome: TaskOutcomeV1::Cancelled,
        visible: true,
    });
    advance(&state, &cancelled);
    assert!(cancelled.leases.is_empty());
}

fn assert_violation(state: &ServiceStateV1, expected: InvariantViolationV1) {
    let report = state.validate_global_invariants().unwrap_err();
    assert!(
        report.violations().contains(&expected),
        "missing {expected:?} in {:?}",
        report.violations()
    );
}

#[test]
fn invariant_mutations_reject_generation_and_aba_confusion() {
    let mut bad_encoding = initial_state();
    bad_encoding.slots[0].generation = GenerationStateV1::Current(GenerationCounterV1 {
        logical: 0,
        encoded: 1,
    });
    assert_violation(
        &bad_encoding,
        InvariantViolationV1::GenerationEncodingMismatch(SlotIdV1(0)),
    );

    let mut mismatch = initial_state();
    mismatch.slots[0].state = QueueSlotStateV1::Empty { generation: 1 };
    assert_violation(
        &mismatch,
        InvariantViolationV1::GenerationStateMismatch(SlotIdV1(0)),
    );

    let mut wrap = initial_state();
    wrap.slots[0].aba = AbaStateV1::WrapRisk {
        oldest_live_generation: 0,
    };
    assert_violation(&wrap, InvariantViolationV1::AbaWrapRisk(SlotIdV1(0)));

    let current = GenerationStateV1::Current(GenerationCounterV1 {
        logical: 15,
        encoded: 15,
    });
    let reclaimed = GenerationStateV1::Current(GenerationCounterV1 {
        logical: 16,
        encoded: 0,
    });
    assert!(current.can_transition_to(reclaimed, true, 16));
    assert!(!current.can_transition_to(reclaimed, false, 16));
    assert!(
        !GenerationStateV1::Current(GenerationCounterV1 {
            logical: u64::MAX,
            encoded: 15,
        })
        .can_transition_to(reclaimed, true, 16)
    );

    let old_reference = AbaStateV1::Protected {
        oldest_live_generation: Some(5),
    };
    assert!(old_reference.can_transition_to(
        AbaStateV1::Protected {
            oldest_live_generation: Some(7),
        },
        8,
        4,
    ));
    assert!(!old_reference.can_transition_to(
        AbaStateV1::Protected {
            oldest_live_generation: Some(4),
        },
        8,
        4,
    ));
    assert!(
        !AbaStateV1::Protected {
            oldest_live_generation: Some(9),
        }
        .is_admissible(8, 4)
    );
}

#[test]
fn invariant_rejects_partial_slot_maps_and_duplicate_task_dependencies() {
    let mut missing = initial_state();
    missing.slots.pop();
    assert_violation(&missing, InvariantViolationV1::MissingSlot(SlotIdV1(1)));

    let mut duplicate = initial_state();
    duplicate.lifecycle = LifecycleStateV1::Running;
    duplicate.workers[0].state = WorkerStateV1::Idle;
    let key = duplicate.config.slot_key(SlotIdV1(0), 0);
    duplicate.slots[0].state = QueueSlotStateV1::Reserved {
        generation: 0,
        task_id: TaskIdV1(8),
    };
    duplicate.tasks.push(TaskRecordV1 {
        task_id: TaskIdV1(8),
        canonical_tag: 0,
        payload_identity: digest(201),
        submission_sequence: 1,
        dependencies: vec![DependencyEpochV1(2), DependencyEpochV1(2)],
        state: TaskStateV1::Reserved(key),
    });
    duplicate.dependencies.push(DependencyRecordV1 {
        epoch: DependencyEpochV1(2),
        state: DependencyStateV1::Pending,
    });
    assert_violation(
        &duplicate,
        InvariantViolationV1::DuplicateTaskDependency(TaskIdV1(8)),
    );
}

#[test]
fn reclaim_requires_a_visible_matching_terminal_record() {
    let mut state = initial_state();
    state.lifecycle = LifecycleStateV1::Running;
    state.workers[0].state = WorkerStateV1::Idle;
    let key = state.config.slot_key(SlotIdV1(0), 0);
    state.slots[0].state = QueueSlotStateV1::Reclaimable {
        generation: 0,
        task_id: TaskIdV1(3),
        outcome: TaskOutcomeV1::Cancelled,
        record_id: CompletionRecordIdV1(3),
    };
    state.tasks.push(TaskRecordV1 {
        task_id: TaskIdV1(3),
        canonical_tag: 0,
        payload_identity: digest(202),
        submission_sequence: 1,
        dependencies: vec![],
        state: TaskStateV1::Cancelled {
            slot: key,
            stage: CancellationStageV1::Reserved,
            record_id: CompletionRecordIdV1(3),
        },
    });
    state.completion_records.push(CompletionRecordV1 {
        record_id: CompletionRecordIdV1(3),
        task_id: TaskIdV1(3),
        slot: key,
        outcome: TaskOutcomeV1::Cancelled,
        visible: false,
    });
    assert_violation(
        &state,
        InvariantViolationV1::CompletionMismatch(CompletionRecordIdV1(3)),
    );
    state.completion_records[0].visible = true;
    assert_valid(&state);
}

#[test]
fn invariant_mutations_reject_stale_brand_unknown_tag_and_unsatisfied_dependency() {
    let mut state = initial_state();
    state.lifecycle = LifecycleStateV1::Running;
    state.workers[0].state = WorkerStateV1::Acquiring;
    let mut stale = state.config.slot_key(SlotIdV1(0), 0);
    stale.service_epoch += 1;
    state.slots[0].state = QueueSlotStateV1::Published {
        generation: 0,
        task_id: TaskIdV1(2),
    };
    state.tasks.push(TaskRecordV1 {
        task_id: TaskIdV1(2),
        canonical_tag: 77,
        payload_identity: digest(147),
        submission_sequence: 1,
        dependencies: vec![DependencyEpochV1(9)],
        state: TaskStateV1::Published(stale),
    });
    assert_violation(&state, InvariantViolationV1::UnknownTaskTag(TaskIdV1(2)));
    assert_violation(&state, InvariantViolationV1::TaskSlotMismatch(TaskIdV1(2)));

    let key = state.config.slot_key(SlotIdV1(0), 0);
    state.tasks[0].canonical_tag = 4;
    state.tasks[0].state = TaskStateV1::Acquired {
        slot: key,
        lease_id: LeaseIdV1(5),
    };
    state.slots[0].state = QueueSlotStateV1::Acquired {
        generation: 0,
        task_id: TaskIdV1(2),
        lease_id: LeaseIdV1(5),
    };
    state.dependencies.push(DependencyRecordV1 {
        epoch: DependencyEpochV1(9),
        state: DependencyStateV1::Pending,
    });
    state.leases.push(LeaseRecordV1 {
        lease_id: LeaseIdV1(5),
        state: LeaseStateV1::Issued(LeaseKeyV1 {
            slot: key,
            task_id: TaskIdV1(2),
            acquisition_event: AcquisitionEventIdV1(1),
            worker_id: WorkerIdV1(0),
        }),
    });
    assert_violation(
        &state,
        InvariantViolationV1::DependencyUnsatisfied(TaskIdV1(2)),
    );
}

#[test]
fn stopped_is_not_valid_without_quiescence_and_visible_completion() {
    let mut state = initial_state();
    state.lifecycle = LifecycleStateV1::Stopped;
    state.admission_cutoff = Some(0);
    assert_violation(&state, InvariantViolationV1::StoppedButNotQuiescent);
}

#[test]
fn component_state_machines_reject_skips_and_cross_brand_changes() {
    assert!(!LifecycleStateV1::Running.can_transition_to(LifecycleStateV1::Stopped));
    assert!(
        !TaskStateV1::Reserved(initial_state().config.slot_key(SlotIdV1(0), 0)).can_transition_to(
            TaskStateV1::Published(initial_state().config.slot_key(SlotIdV1(0), 0))
        )
    );
    assert!(
        !QueueSlotStateV1::Empty { generation: 0 }
            .can_transition_to(QueueSlotStateV1::Empty { generation: 1 })
    );
    assert!(
        !PhaseStateV1::Completed {
            epoch: 2,
            phase_id: PhaseIdV1(1),
            owner: PhaseOwnerV1::Worker(WorkerIdV1(0)),
        }
        .can_transition_to(PhaseStateV1::Inactive { epoch: 4 })
    );
    assert!(
        !DependencyStateV1::CompletionPublished {
            producer_task: TaskIdV1(1),
        }
        .can_transition_to(DependencyStateV1::VisibleSatisfied {
            producer_task: TaskIdV1(2),
        })
    );
}

#[test]
fn transition_validation_rejects_identity_and_payload_mutation() {
    let mut running = initial_state();
    running.lifecycle = LifecycleStateV1::Running;
    running.workers[0].state = WorkerStateV1::Idle;
    assert_valid(&running);

    let mut changed_config = running.clone();
    changed_config.config.service_epoch += 1;
    assert_eq!(
        running.validate_transition_to(&changed_config),
        Err(TransitionErrorV1::ImmutableConfigurationChanged)
    );

    let key = running.config.slot_key(SlotIdV1(0), 0);
    running.slots[0].state = QueueSlotStateV1::Reserved {
        generation: 0,
        task_id: TaskIdV1(4),
    };
    running.tasks.push(TaskRecordV1 {
        task_id: TaskIdV1(4),
        canonical_tag: 0,
        payload_identity: digest(203),
        submission_sequence: 1,
        dependencies: vec![],
        state: TaskStateV1::Reserved(key),
    });
    assert_valid(&running);
    let mut changed_payload = running.clone();
    changed_payload.tasks[0].payload_identity = digest(204);
    assert_eq!(
        running.validate_transition_to(&changed_payload),
        Err(TransitionErrorV1::IllegalTaskTransition(TaskIdV1(4)))
    );
}

#[test]
fn property_classifications_never_promote_each_other() {
    let mut claims = PropertyClaimsV1::unsupported();
    claims.set(ServicePropertyV1::QueueSafe, EvidenceStatusV1::Proved);
    assert_eq!(
        claims.get(ServicePropertyV1::QueueSafe),
        EvidenceStatusV1::Proved
    );
    for property in ALL_SERVICE_PROPERTIES_V1 {
        if property != ServicePropertyV1::QueueSafe {
            assert_eq!(claims.get(property), EvidenceStatusV1::Unsupported);
        }
    }

    claims.set(
        ServicePropertyV1::ServiceProgress,
        EvidenceStatusV1::Contracted,
    );
    assert_eq!(
        claims.get(ServicePropertyV1::QuiescenceSafe),
        EvidenceStatusV1::Unsupported
    );
    assert_eq!(
        claims.get(ServicePropertyV1::TaskAccounted),
        EvidenceStatusV1::Unsupported
    );

    for selected in ALL_SERVICE_PROPERTIES_V1 {
        let mut isolated = PropertyClaimsV1::unsupported();
        isolated.set(selected, EvidenceStatusV1::Validated);
        for observed in ALL_SERVICE_PROPERTIES_V1 {
            let expected = if selected == observed {
                EvidenceStatusV1::Validated
            } else {
                EvidenceStatusV1::Unsupported
            };
            assert_eq!(isolated.get(observed), expected);
        }
    }
}
