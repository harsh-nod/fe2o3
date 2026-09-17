fn publication_deferred_effect(
    role: StaticPublicationRoleV1,
) -> ProjectedStaticPublicationEffectV1 {
    ProjectedStaticPublicationEffectV1 {
        role,
        payload: ProductionRankedValueIdV1::new(0),
        flags: ProductionRankedValueIdV1::new(1),
        index: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(2)),
        extent: ProductionRankedValueV1::Argument(0),
        read_results: None,
    }
}

#[test]
fn publication_deferred_values_preserve_consumer_order_and_skip_unreachable() {
    let mut blocks = vec![
        ProjectedSemanticBlockV1 { items: vec![] },
        ProjectedSemanticBlockV1 {
            items: vec![ProjectedBlockItemV1::StaticPublication(
                publication_deferred_effect(StaticPublicationRoleV1::Producer),
            )],
        },
        ProjectedSemanticBlockV1 {
            items: vec![ProjectedBlockItemV1::StaticPublication(
                publication_deferred_effect(StaticPublicationRoleV1::Consumer),
            )],
        },
        ProjectedSemanticBlockV1 {
            items: vec![ProjectedBlockItemV1::StaticPublication(
                publication_deferred_effect(StaticPublicationRoleV1::Consumer),
            )],
        },
    ];
    let mut next = 18;
    let pipeline =
        prepare_pipeline_values_v1(&mut blocks, &[true, true, false, true], &mut next).unwrap();
    assert!(pipeline.is_empty());
    assert_eq!(next, 21);
    let ProjectedBlockItemV1::StaticPublication(skipped) = &blocks[2].items[0] else {
        panic!()
    };
    assert!(skipped.read_results.is_none());
    let ProjectedBlockItemV1::StaticPublication(producer) = blocks[1].items.remove(0) else {
        panic!()
    };
    assert_eq!(producer.operation_count(), 2);
    let producer = producer.materialize().unwrap();
    assert!(matches!(
        producer.as_slice(),
        [
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Write,
                ..
            },
            ProductionRankedOperationV1::PublicationAtomicStoreU32 { value: 2, .. }
        ]
    ));
    let ProjectedBlockItemV1::StaticPublication(consumer) = blocks[3].items.remove(0) else {
        panic!()
    };
    assert_eq!(consumer.operation_count(), 4);
    assert_eq!(
        consumer
            .read_results
            .unwrap()
            .map(ProductionRankedValueIdV1::get),
        [18, 19, 20]
    );
    let consumer = consumer.materialize().unwrap();
    let [
        ProductionRankedOperationV1::PublicationAtomicStoreU32 {
            view: flags,
            index: request_index,
            value: 1,
        },
        ProductionRankedOperationV1::PublicationAtomicLoadU32 {
            result: acquired,
            view: load_flags,
            index: load_index,
        },
        ProductionRankedOperationV1::PublicationReadGuard {
            result,
            success,
            index,
            physical_extent,
            acquired: observed,
        },
        ProductionRankedOperationV1::PredicatedAccess {
            kind: AccessKindAttr::Read,
            view: payload,
            index: read_index,
            success: read_success,
        },
    ] = consumer.as_slice()
    else {
        panic!()
    };
    assert_eq!(flags, load_flags);
    assert_eq!(request_index, load_index);
    assert_eq!(request_index, index);
    assert_eq!(*physical_extent, ProductionRankedValueV1::Argument(0));
    assert_eq!(*observed, ProductionRankedValueV1::Local(*acquired));
    assert_eq!(*read_index, ProductionRankedValueV1::Local(*result));
    assert_eq!(*read_success, ProductionRankedValueV1::Local(*success));
    assert_eq!(
        *payload,
        ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0))
    );
    assert_eq!(producer.len() + consumer.len(), 6); // Five memory effects plus the pure guard.
}

#[test]
fn publication_deferred_values_reject_missing_or_repeated_preparation() {
    let mut consumer = publication_deferred_effect(StaticPublicationRoleV1::Consumer);
    assert!(consumer.clone().materialize().is_err());
    let mut next = 7;
    consumer.prepare_values(&mut next).unwrap();
    assert!(consumer.prepare_values(&mut next).is_err());
    assert_eq!(next, 10);
    let mut producer = publication_deferred_effect(StaticPublicationRoleV1::Producer);
    producer.read_results = consumer.read_results;
    assert!(producer.materialize().is_err());
}
