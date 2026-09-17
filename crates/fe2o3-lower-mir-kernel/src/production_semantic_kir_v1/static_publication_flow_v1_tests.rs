fn ranked_consumer() -> (
    ProductionRankedKernelLoweringInputV1,
    Vec<ProductionRankedAccessSourceV1>,
) {
    let id = ProductionRankedValueIdV1::new;
    let value = |index| ProductionRankedValueV1::Local(id(index));
    let mut operations = vec![
        ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: 1,
            global_extents: [256, 1, 1],
            workgroup_extents: [128, 1, 1],
            subgroup_size: 64,
            full_physical_workgroups: true,
        },
        ProductionRankedOperationV1::ViewInSpace {
            result: id(0),
            element_width: 32,
            writable: true,
            shape: vec![dialect_kernel::DYNAMIC_EXTENT],
            dynamic_extents: vec![value(1)],
            memory_space: dialect_kernel::MemorySpaceAttr::Global,
            allocation_origin: 1,
            noalias_class: 1,
        },
        ProductionRankedOperationV1::ViewInSpace {
            result: id(1),
            element_width: 32,
            writable: true,
            shape: vec![dialect_kernel::DYNAMIC_EXTENT],
            dynamic_extents: vec![value(1)],
            memory_space: dialect_kernel::MemorySpaceAttr::Global,
            allocation_origin: 2,
            noalias_class: 2,
        },
        ProductionRankedOperationV1::IndexConstant {
            result: id(2),
            value: 0,
        },
        ProductionRankedOperationV1::PublicationAtomicStoreU32 {
            view: value(1),
            index: value(2),
            value: 1,
        },
        ProductionRankedOperationV1::PublicationAtomicLoadU32 {
            result: id(3),
            view: value(1),
            index: value(2),
        },
        ProductionRankedOperationV1::PublicationReadGuard {
            result: id(4),
            success: id(5),
            index: value(2),
            physical_extent: value(1),
            acquired: value(3),
        },
        ProductionRankedOperationV1::PredicatedAccess {
            kind: AccessKindAttr::Read,
            view: value(0),
            index: value(4),
            success: value(5),
        },
    ];
    // Use a complete live handshake; the source-correlation fixture selects
    // only the consumer span and is not itself source custody authority.
    let mut consumer = operations.split_off(4);
    let mut flags = operations.remove(2);
    let mut payload = operations.remove(1);
    if let ProductionRankedOperationV1::ViewInSpace { result, .. } = &mut payload {
        *result = id(3);
    }
    if let ProductionRankedOperationV1::ViewInSpace { result, .. } = &mut flags {
        *result = id(4);
    }
    operations.truncate(1);
    operations.extend([
        ProductionRankedOperationV1::InvocationIndex {
            result: id(0),
            dimension: 0,
            launch_extent: 256,
        },
        ProductionRankedOperationV1::IndexConstant {
            result: id(1),
            value: 128,
        },
        ProductionRankedOperationV1::IndexBinary {
            result: id(2),
            kind: dialect_kernel::IndexBinaryKindAttr::Remainder,
            lhs: value(0),
            rhs: value(1),
        },
        payload,
        flags,
    ]);
    consumer[0] = ProductionRankedOperationV1::PublicationAtomicStoreU32 {
        view: value(4),
        index: value(2),
        value: 1,
    };
    consumer[1] = ProductionRankedOperationV1::PublicationAtomicLoadU32 {
        result: id(5),
        view: value(4),
        index: value(2),
    };
    consumer[2] = ProductionRankedOperationV1::PublicationReadGuard {
        result: id(6),
        success: id(7),
        index: value(2),
        physical_extent: value(1),
        acquired: value(5),
    };
    consumer[3] = ProductionRankedOperationV1::PredicatedAccess {
        kind: AccessKindAttr::Read,
        view: value(3),
        index: value(6),
        success: value(7),
    };
    let producer = vec![
        ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Write,
            view: value(3),
            indices: vec![value(2)],
        },
        ProductionRankedOperationV1::PublicationAtomicStoreU32 {
            view: value(4),
            index: value(2),
            value: 2,
        },
    ];
    let kernel = ProductionRankedKernelV1::new(
        "static_publication_component",
        0,
        vec![
            ProductionRankedBlockV1::new(
                operations,
                ProductionRankedTerminatorV1::IndexLessThan {
                    lhs: value(0),
                    rhs: value(1),
                    true_block: 1,
                    false_block: 2,
                },
            ),
            ProductionRankedBlockV1::new(producer, ProductionRankedTerminatorV1::Return),
            ProductionRankedBlockV1::new(consumer, ProductionRankedTerminatorV1::Return),
        ],
    )
    .unwrap();
    let lowering = compile_ranked_kernel_for_gfx942_lowering_v1(
        ProductionConstructionV1::ranked_kernel("publication_component", kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
        [2],
    )
    .unwrap();
    let sources = [0, 1, 3]
        .into_iter()
        .enumerate()
        .map(|(ordinal, operation)| {
            ProductionRankedAccessSourceV1::new(0, None, ordinal as u32, 2, operation)
        })
        .collect();
    (lowering, sources)
}

#[test]
fn static_publication_conditional_witness_retains_both_mandatory_atomics() {
    let owner = owner(false, true);
    let (lowering, sources) = ranked_consumer();
    let body = owner.module().functions[0].body.as_ref().unwrap();
    let mut setup = UnsupportedIndexCorrelationBudgetV1 { remaining: 100_000 };
    let kir = build_kir_correlation_index(body, 1024, &mut setup).unwrap();
    let function = SemanticFunctionIdV1::from_index(0);
    let sites =
        index_semantic_access_sites(owner.correspondence(), function, function, &kir, &mut setup)
            .unwrap();
    let ranked = index_ranked_correlation(&lowering, &sources, 1024, &mut setup).unwrap();
    let context = StaticPublicationCorrelationV1 {
        semantic: Some(owner.semantic_ssa.source_semantic()),
        semantic_function: function,
        kir: &kir,
        sites: &sites,
        ranked: &ranked,
        lowering: &lowering,
    };
    let mut witnesses = BTreeMap::new();
    for consumer in &kir.memory_consumers {
        let site = sites[&(consumer.location, consumer.operation_access_ordinal)];
        let witness = context
            .authenticate_read(site, site, *consumer, 3, &mut setup)
            .unwrap();
        assert_eq!(witness.is_some(), site.ordinal == 2);
        if let Some(witness) = witness {
            witnesses.insert(witness.key().unwrap(), witness);
        }
    }
    assert_eq!(witnesses.len(), 1);
    let locations = kir
        .memory_consumers
        .iter()
        .map(|consumer| {
            let site = sites[&(consumer.location, consumer.operation_access_ordinal)];
            let source = ranked.sources_by_site[&site];
            (
                site,
                consumer.location,
                consumer.operation_access_ordinal,
                (source.ranked_block, source.ranked_operation),
            )
        })
        .collect::<Vec<_>>();
    validate_effect_control_flow_v1(body, lowering.kernel(), &locations, &witnesses, &mut setup)
        .unwrap();
    let events = BTreeMap::from([(
        0,
        (0..4)
            .map(|ordinal| {
                (
                    u64::from(ordinal),
                    SemanticAccessSiteV1 {
                        block: 0,
                        statement: None,
                        ordinal,
                    },
                )
            })
            .collect(),
    )]);
    let read = AuthenticatedConditionalReadV1 {
        location: FunctionOperationLocation::new(BlockId(0), 0),
        operation_access_ordinal: 2,
        site: SemanticAccessSiteV1 {
            block: 0,
            statement: None,
            ordinal: 2,
        },
    };
    let optional = BTreeMap::from([(read.key().unwrap(), read)]);
    let successors = BTreeMap::from([(0, vec![])]);
    let flow = effect_flow_signature_v1(0, &events, &successors, &optional, &mut setup).unwrap();
    let event = |ordinal| SemanticAccessSiteV1 {
        block: 0,
        statement: None,
        ordinal,
    };
    assert_eq!(flow.entry_effects, BTreeSet::from([event(0)]));
    assert_eq!(
        flow.next_effects,
        BTreeSet::from([
            (event(0), event(1)),
            (event(1), event(2)),
            (event(1), event(3)),
            (event(2), event(3)),
        ])
    );
}

#[test]
fn static_publication_witness_rejects_missing_extra_or_relocated_consumers_and_exhaustion() {
    let owner = owner(false, true);
    let (lowering, sources) = ranked_consumer();
    for mutation in 0..7 {
        let mut setup = UnsupportedIndexCorrelationBudgetV1 { remaining: 100_000 };
        let mut kir = build_kir_correlation_index(
            owner.module().functions[0].body.as_ref().unwrap(),
            1024,
            &mut setup,
        )
        .unwrap();
        let function = SemanticFunctionIdV1::from_index(0);
        let mut sites = index_semantic_access_sites(
            owner.correspondence(),
            function,
            function,
            &kir,
            &mut setup,
        )
        .unwrap();
        let mut ranked = index_ranked_correlation(&lowering, &sources, 1024, &mut setup).unwrap();
        let read = *kir
            .memory_consumers
            .iter()
            .find(|consumer| sites[&(consumer.location, 0)].ordinal == 2)
            .unwrap();
        let site = sites[&(read.location, 0)];
        match mutation {
            0 | 1 => {}
            2 => {
                kir.memory_consumers.remove(0);
            }
            3 => {
                let duplicate = kir.memory_consumers[0];
                kir.memory_consumers.push(duplicate);
            }
            4 => {
                let request = kir.memory_consumers[0];
                sites.get_mut(&(request.location, 0)).unwrap().ordinal = 1;
            }
            5 => {
                ranked
                    .sources_by_site
                    .get_mut(&site)
                    .unwrap()
                    .ranked_operation = 5;
            }
            6 => {}
            _ => unreachable!(),
        }
        let context = StaticPublicationCorrelationV1 {
            semantic: Some(owner.semantic_ssa.source_semantic()),
            semantic_function: function,
            kir: &kir,
            sites: &sites,
            ranked: &ranked,
            lowering: &lowering,
        };
        let mut budget = UnsupportedIndexCorrelationBudgetV1 {
            remaining: if mutation == 6 { 0 } else { 100_000 },
        };
        assert!(
            context
                .authenticate_read(
                    site,
                    site,
                    read,
                    match mutation {
                        0 => 2,
                        1 => 4,
                        _ => 3,
                    },
                    &mut budget
                )
                .is_err(),
            "mutation {mutation}"
        );
    }
}
