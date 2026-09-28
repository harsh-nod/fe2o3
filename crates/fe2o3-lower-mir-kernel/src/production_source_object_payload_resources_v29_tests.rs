use super::*;

#[test]
fn each_inert_opcode_has_an_independent_twelve_work_validation_boundary() {
    for (payload, operation) in payload_cases() {
        for limit in [11, 12] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            let result = payload.check_operation(&operation, &mut budget);
            assert_eq!(result.is_ok(), limit == 12);
            assert_eq!(budget.storage(), 0);
        }
    }
}

fn inspect_atomic_publication(
    lifecycle: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let original = emitted
        .iter()
        .flatten()
        .find_map(|row| row.scoped_memory_anchors.as_ref())
        .unwrap();
    let payload = payload_cases()
        .into_iter()
        .find(|(row, _)| matches!(row.operation, ScopedObjectOperationV29::CopyObject { .. }))
        .unwrap()
        .0;
    // Two endpoints each check original and view paths (12), two capacity preparations (10),
    // and one atomic publication (3). No production cost helper is called.
    let required_work = 25;
    let required_storage = 4 * std::mem::size_of::<ScopedObjectPayloadV29>()
        + 4 * std::mem::size_of::<ScopedMemoryAnchorV29>();
    for (work_limit, storage_limit, success) in [
        (required_work, required_storage, true),
        (required_work - 1, required_storage, false),
        (required_work, required_storage - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut meter = ArgumentBudgetV1::new(&mut work, storage_limit);
        let mut subject = original.subject;
        subject.ledger = meter.work_ledger_identity_v1();
        let mut anchors = fresh_anchors(subject, original.placement);
        let result = anchors.append_object(BlockId(1), 0, None, payload, &mut meter);
        assert_eq!(result.is_ok(), success);
        assert_eq!(
            (anchors.rows.len(), anchors.objects.len()),
            if success { (1, 1) } else { (0, 0) }
        );
        if success {
            assert_eq!(meter.work(), required_work);
            assert_eq!(meter.storage(), required_storage);
            assert_eq!(anchors.retained_storage().unwrap(), required_storage);
        } else {
            // The raw ledger retains denial history; scope owners enforce poisoning.
            if work_limit < required_work {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error))) if error.actual() == required_work && error.limit() == work_limit)
                );
                assert_eq!(meter.work(), required_work - 3);
                assert_eq!(meter.storage(), required_storage);
                assert_eq!(meter.failed_storage(), None);
            } else {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(error))) if error.actual() == required_storage && error.limit() == storage_limit)
                );
                assert_eq!(meter.work(), required_work - 3);
                assert_eq!(
                    meter.storage(),
                    4 * std::mem::size_of::<ScopedObjectPayloadV29>()
                );
                assert_eq!(meter.failed_storage(), Some(required_storage));
            }
            assert_eq!(anchors.retained_storage().unwrap(), meter.storage());
            assert!(
                anchors
                    .append_object(BlockId(1), 0, None, payload, &mut meter)
                    .is_err()
            );
            assert!(anchors.rows.is_empty() && anchors.objects.is_empty());
        }
    }
    observe(lifecycle, instances, emitted, slots, budget)
}

#[test]
fn object_payload_and_anchor_publish_together_at_independent_resource_boundaries() {
    let (result, _, _) = run(
        false,
        ScopedFixture::CheckedSlot,
        inspect_atomic_publication,
        LIMIT,
        LIMIT,
    );
    assert!(is_stopped(&result), "{result:?}");
}

#[test]
fn typed_anchor_owner_and_recorder_headers_match_independent_live_shapes() {
    assert_eq!(
        std::mem::size_of::<ScopedObjectCheckpointV29>(),
        3 * std::mem::size_of::<usize>()
    );
    assert_eq!(
        std::mem::size_of::<Option<ScopedObjectCheckpointV29>>(),
        std::mem::size_of::<Option<(usize, usize, usize)>>()
    );
    #[allow(dead_code)]
    enum AnchorKind {
        Object(usize),
        Access {
            pointer: ValueId,
            payload: Option<ScopedMemoryPayloadV29>,
        },
        FailureRead {
            event: usize,
            local: u32,
        },
        Kill {
            event: usize,
            local: u32,
            cause: ScopedMemoryKillV29,
        },
    }
    #[allow(dead_code)]
    struct Anchor {
        block: BlockId,
        position: usize,
        source: Option<ScopedMemoryFrameV29>,
        kind: AnchorKind,
    }
    #[allow(dead_code)]
    struct Anchors {
        subject: ScopedInitializationSubjectV29,
        placement: SemanticEmissionPlacementV1,
        rows: Vec<Anchor>,
        payloads: Vec<ScopedObjectPayloadV29>,
        components: Vec<ScopedObjectComponentV29>,
    }
    #[allow(dead_code)]
    struct Recorder {
        anchors: Anchors,
        block: Option<BlockId>,
        frame: Option<ScopedMemoryFrameV29>,
        read: Option<(ScopedMemoryReadV29, bool)>,
        index: Option<ScopedMemoryIndexReadV29>,
        store: Option<(ValueId, ScopedMemoryStoreSourceV29)>,
        object: Option<ScopedObjectRoleV29>,
        last_load: Option<usize>,
    }
    assert_eq!(
        std::mem::size_of::<ScopedMemoryAnchorKindV29>(),
        std::mem::size_of::<AnchorKind>()
    );
    assert_eq!(
        std::mem::size_of::<ScopedMemoryAnchorV29>(),
        std::mem::size_of::<Anchor>()
    );
    assert_eq!(
        std::mem::size_of::<ScopedMemoryAnchorsV29>(),
        std::mem::size_of::<Anchors>()
    );
    assert_eq!(
        std::mem::size_of::<ScopedMemoryRecorderV29>(),
        std::mem::size_of::<Recorder>()
    );
}

fn inspect_path_boundaries(
    lifecycle: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let original = emitted
        .iter()
        .flatten()
        .find_map(|row| row.scoped_memory_anchors.as_ref())
        .unwrap();
    let component = ScopedObjectComponentV29::View {
        projection: ScopedObjectViewProjectionV29::ArrayElement,
        ty: SemanticTypeIdV1::from_index(0),
    };
    // Independent fixed path bound, row visit cost, capacity preparation and
    // allocation header. Do not derive expected costs from production helpers.
    assert_eq!(MAX_SSA_VALUE_COMPONENTS_V1, 256);
    for count in [1, 256] {
        let components = vec![component; count];
        let required_work = 2 + count + 2 + 3;
        let required_storage = count.max(4) * std::mem::size_of::<ScopedObjectComponentV29>();
        for (work_limit, storage_limit, success) in [
            (required_work, required_storage, true),
            (required_work - 1, required_storage, false),
            (required_work, required_storage - 1, false),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut meter = ArgumentBudgetV1::new(&mut work, storage_limit);
            let mut subject = original.subject;
            subject.ledger = meter.work_ledger_identity_v1();
            let mut anchors = fresh_anchors(subject, original.placement);
            let result = anchors.append_object_path(&components, &mut meter);
            assert_eq!(result.is_ok(), success);
            assert!(anchors.rows.is_empty() && anchors.objects.is_empty());
            if success {
                assert_eq!(result.unwrap(), ScopedObjectPathV29 { first: 0, count });
                assert_eq!(anchors.object_components, components);
                assert_eq!(meter.work(), required_work);
                assert_eq!(meter.storage(), required_storage);
            } else {
                assert!(anchors.object_components.is_empty());
                assert_eq!(anchors.object_components.capacity(), 0);
                assert_eq!(meter.storage(), 0);
                if work_limit < required_work {
                    assert!(
                        matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(error))) if error.actual() == required_work && error.limit() == work_limit)
                    );
                    assert_eq!(meter.work(), required_work - 3);
                    assert_eq!(meter.failed_storage(), None);
                } else {
                    assert!(
                        matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(error))) if error.actual() == required_storage && error.limit() == storage_limit)
                    );
                    assert_eq!(meter.work(), required_work);
                    assert_eq!(meter.failed_storage(), Some(required_storage));
                }
            }
        }
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut meter = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let mut subject = original.subject;
    subject.ledger = meter.work_ledger_identity_v1();
    let mut anchors = fresh_anchors(subject, original.placement);
    assert!(
        anchors
            .append_object_path(&vec![component; 257], &mut meter)
            .is_err()
    );
    assert_eq!(meter.work(), 259);
    assert_eq!(meter.storage(), 0);
    assert!(anchors.object_components.is_empty());
    for path in [
        ScopedObjectPathV29 { first: 1, count: 0 },
        ScopedObjectPathV29 {
            first: 0,
            count: 257,
        },
        ScopedObjectPathV29 {
            first: usize::MAX,
            count: 1,
        },
    ] {
        assert!(anchors.object_path(path, &mut meter).is_err());
    }
    assert_eq!(meter.storage(), 0);
    observe(lifecycle, instances, emitted, slots, budget)
}

#[test]
fn original_and_view_path_publication_has_independent_exact_and_one_short_limits() {
    let (result, _, _) = run(
        false,
        ScopedFixture::CheckedSlot,
        inspect_path_boundaries,
        LIMIT,
        LIMIT,
    );
    assert!(is_stopped(&result), "{result:?}");
}
