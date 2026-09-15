use super::*;

#[test]
fn math_bound_opacity_preserves_shared_leaves_but_rejects_nested_owned_siblings() {
    let source = fixture::full_source(false, false);
    let original = &source.functions()[0];
    assert_eq!(source.types().len(), 12);
    assert_eq!(MathConsumerBorrowV1::for_callable(source.types(), &source.callables()[7])
        .unwrap().pair(), (ty(10), ty(9)));
    for (fields, offsets) in [([10, 9], [0, 8]), ([9, 10], [0, 16])] {
        // Private shape controls, not admitted source or producer authority.
        let mut types = source.types().to_vec();
        for (index, fields, offsets) in [
            (12, fields.to_vec(), offsets.to_vec()), (13, vec![12], vec![0]),
        ] {
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([230 + index as u8; 32]),
                SemanticLayoutIdentityV1::from_sha256([230 + index as u8; 32]),
                SemanticTypeLayoutV1::aggregate(Some(24), 8,
                    SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap()).unwrap(),
                SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(fields.into_iter().map(ty).collect()).unwrap()),
            ));
        }
        let mut locals = original.locals().to_vec();
        for index in [12, 13] {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([230 + index as u8; 32]),
                ty(index), SemanticLocalRoleV1::Temporary, original.source()));
        }
        let body = SemanticFunctionDeclV1::new(original.identity(), original.role(),
            original.item_definition_identity(), original.monomorphization_identity(),
            original.generic_type_arguments_identity(), original.const_generic_arguments_identity(),
            original.source(), original.abi().clone(), locals, original.entry(),
            original.blocks().to_vec()).unwrap();
        let routes = Routes::new(&body, Some(&types), source.callables(),
            &mut budget(MAX_FLOW_WORK)).unwrap();
        assert_eq!(routes.reference(ty(10)), Some(ty(10)));
        assert_eq!(routes.owned(ty(10)), Some(&ty(9)));
        for opaque in [9, 12, 13] { assert_eq!(routes.owned(ty(opaque)), None); }
    }
    let captured = fixture::captured_source(false, false, false, false);
    let routes = Routes::new(&captured.functions()[0], Some(captured.types()),
        captured.callables(), &mut budget(MAX_FLOW_WORK)).unwrap();
    assert_eq!(routes.owned(ty(12)), Some(&ty(9)), "ordinary &bound carrier survives");
}

#[test]
fn math_bound_registry_charges_first_copy_and_repeated_membership_before_mutation() {
    let source = fixture::full_source(false, false);
    let terminal = source.callables()[7].clone();
    // First insertion: set header 3, zero copied entries, lookup 1, entry 2.
    // Repeated insertion: no clone, lookup 2, conservative entry allowance 2.
    for (callables, before, requested) in [
        (vec![terminal.clone()], 1 + 16 + 2, 6),
        (vec![terminal.clone(), terminal], 2 + 16 + 2 + 6 + 16 + 2, 4),
    ] {
        for remaining in 0..requested {
            let limit = before + remaining;
            let Err(ProductionSemanticSsaErrorV1::BorrowFlowWork {
                remaining_work_units, requested_work_units, phase_work_units, error, ..
            }) = Routes::new(&source.functions()[0], Some(source.types()), &callables,
                &mut budget(limit)) else { panic!("short registration budget accepted") };
            assert_eq!(remaining_work_units, remaining);
            assert_eq!(requested_work_units, requested);
            assert_eq!(phase_work_units.iter().sum::<usize>(), before);
            assert_eq!(*error, ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits, required: limit + 1, limit });
        }
        let mut full = budget(MAX_FLOW_WORK);
        Routes::new(&source.functions()[0], Some(source.types()), &callables, &mut full).unwrap();
        let required = MAX_FLOW_WORK - full.remaining;
        let mut exact = budget(required);
        let routes = Routes::new(&source.functions()[0], Some(source.types()), &callables,
            &mut exact).unwrap();
        assert_eq!(exact.remaining, 0);
        assert_eq!(routes.owned(ty(10)), Some(&ty(9)));
        assert!(Routes::new(&source.functions()[0], Some(source.types()), &callables,
            &mut budget(required - 1)).is_err());
    }
}

#[test]
fn absent_math_registry_keeps_the_empty_route_constructor_cost() {
    let source = fixture::full_source(false, false);
    for count in [0, 7] {
        let mut exact = budget(count);
        let routes = Routes::new(&source.functions()[0], Some(source.types()),
            &source.callables()[..count], &mut exact).unwrap();
        assert_eq!(exact.remaining, 0);
        assert!(routes.routes.is_empty());
    }
}
