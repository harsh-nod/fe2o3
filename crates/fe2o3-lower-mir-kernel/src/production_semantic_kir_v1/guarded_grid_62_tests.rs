mod representation_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAggregateLayoutV1, SemanticAggregateTypeV1, SemanticLayoutIdentityV1,
        SemanticTypeIdentityV1, SemanticTypeLayoutV1,
    };

    // Scalar/noop scaffolding only supplies a real work owner. It is not Grid
    // issuance; the actual source callbacks exercise the opaque receipt path.
    fn with_graph(extra: usize, test: impl FnOnce(&mut CapabilitySsaGraphV1<'_>)) {
        let owner = super::super::resource_tests::noop_semantic_owner(&["grid_representation"]);
        let body = &owner.semantic().functions()[0];
        let plan = fe2o3_pliron::plan_semantic_function_ssa_v1(
            SemanticFunctionIdV1::from_index(0),
            body,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let base = (0..1024)
            .find(|limit| CapabilitySsaGraphV1::new(body, plan.plan(), *limit).is_ok())
            .expect("bounded constructor work for the noop fixture");
        let mut graph = CapabilitySsaGraphV1::new(body, plan.plan(), base + extra).unwrap();
        test(&mut graph);
    }
    fn aggregate(fields: Vec<SemanticTypeIdV1>, alignment: u64) -> SemanticTypeDeclV1 {
        let offsets = vec![0; fields.len()];
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([41; 32]),
            SemanticLayoutIdentityV1::from_sha256([42; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                alignment,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
        )
    }
    fn marker_types() -> Vec<SemanticTypeDeclV1> {
        vec![
            aggregate(vec![SemanticTypeIdV1::from_index(1); 3], 1),
            aggregate(vec![], 1),
        ]
    }
    fn work_error(error: ProductionSemanticKirErrorV1) {
        assert!(
            matches!(
                error,
                ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    ..
                }
            ),
            "unexpected error: {error:?}"
        );
    }
    fn value(id: u32, ty: Type) -> SemanticValueBindingV1 {
        SemanticValueBindingV1::Value {
            id: ValueId(id),
            ty,
        }
    }

    #[test]
    fn guarded_grid_kir_representation_precharges_exact_flat_payload() {
        let types = marker_types();
        let work = 16
            + 4
            + 64
            + 9 * std::mem::size_of::<SemanticValueBindingV1>()
                .div_ceil(std::mem::size_of::<usize>());
        with_graph(work, |g| {
            precharge_leader_representation(&types, SemanticTypeIdV1::from_index(0), g).unwrap();
            work_error(g.charge(1).unwrap_err());
        });
        with_graph(work - 1, |g| {
            work_error(
                precharge_leader_representation(&types, SemanticTypeIdV1::from_index(0), g)
                    .unwrap_err(),
            )
        });
    }
    #[test]
    fn guarded_grid_kir_representation_rejects_nested_aligned_and_missing_fields() {
        for mutation in 0..5 {
            let mut types = marker_types();
            match mutation {
                0 => types[0] = aggregate(vec![SemanticTypeIdV1::from_index(1); 2], 1),
                1 => types[1] = aggregate(vec![SemanticTypeIdV1::from_index(1)], 1),
                2 => types[1] = aggregate(vec![], 2),
                3 => {
                    types.pop();
                }
                _ => types[0] = aggregate(vec![], 1),
            }
            with_graph(10_000, |g| {
                assert!(
                    matches!(
                        precharge_leader_representation(&types, SemanticTypeIdV1::from_index(0), g),
                        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                    ),
                    "mutation {mutation}"
                )
            });
        }
    }
    #[test]
    fn guarded_grid_kir_plain_equality_keeps_scalar_identity_and_type() {
        with_graph(100, |g| {
            let a = value(1, Type::INDEX);
            assert!(same_plain(&a, &a, g, 0).unwrap());
            assert!(!same_plain(&a, &value(2, Type::INDEX), g, 0).unwrap());
            assert!(!same_plain(&a, &value(1, Type::F32), g, 0).unwrap());
            assert!(
                !same_plain(
                    &SemanticValueBindingV1::Unit,
                    &SemanticValueBindingV1::Aggregate(vec![]),
                    g,
                    0
                )
                .unwrap()
            );
        });
    }
    #[test]
    fn guarded_grid_kir_plain_equality_cannot_mint_or_compare_capabilities() {
        with_graph(100, |g| {
            let witness = SemanticValueBindingV1::IndexWitness {
                id: ValueId(1),
                index_space: SemanticDisjointIndexSpaceV1::GridExclusive,
                disjoint: true,
                availability: None,
            };
            assert!(!same_plain(&witness, &witness, g, 0).unwrap());
            assert!(!zero_plain(&witness, g, 0).unwrap());
            assert!(!zero_plain(&value(0, Type::INDEX), g, 0).unwrap());
        });
    }
    #[test]
    fn guarded_grid_kir_plain_walks_share_budget_without_refund() {
        let a = SemanticValueBindingV1::Aggregate(vec![SemanticValueBindingV1::Unit; 3]);
        with_graph(8, |g| {
            assert!(same_plain(&a, &a, g, 0).unwrap());
            assert!(zero_plain(&a, g, 0).unwrap());
            work_error(same_plain(&a, &a, g, 0).unwrap_err());
        });
        with_graph(3, |g| {
            work_error(same_plain(&a, &a, g, 0).unwrap_err());
            work_error(g.charge(1).unwrap_err());
        });
    }
    #[test]
    fn guarded_grid_kir_plain_walks_reject_deep_structure() {
        let mut a = SemanticValueBindingV1::Unit;
        for _ in 0..34 {
            a = SemanticValueBindingV1::Aggregate(vec![a]);
        }
        with_graph(100, |g| {
            assert!(matches!(
                same_plain(&a, &a, g, 0),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
            assert!(matches!(
                zero_plain(&a, g, 0),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
        });
    }
}
