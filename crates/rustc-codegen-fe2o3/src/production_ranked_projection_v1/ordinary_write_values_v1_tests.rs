#[cfg(test)]
mod ordinary_write_value_component_tests_v1 {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };

    // Unadmitted component inputs isolate placement and accounting. Actual
    // source/B/O coverage remains in the genuine Global root fixture tests.
    struct Facts<'a, 'b> {
        budget: &'a mut Budget<'b>,
        checked: bool,
    }
    impl ProjectedAssertionFactsV1 for Facts<'_, '_> {
        fn checked_control_enabled_v1(&self) -> bool {
            self.checked
        }
        fn charge_private_array_work(
            &mut self,
            amount: usize,
        ) -> Result<(), ProductionRankedProjectionErrorV1> {
            self.budget.charge_work(amount).map_err(|error| {
                ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error),
                )
            })
        }
        fn reserve_checked_control_storage_v1(
            &mut self,
            amount: usize,
        ) -> Result<(), ProductionRankedProjectionErrorV1> {
            self.budget.reserve_storage(amount).map_err(|error| {
                ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error),
                )
            })
        }
        fn private_array_access(
            &mut self,
            _: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
            _: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        ) -> Result<bool, ProductionRankedProjectionErrorV1> {
            unreachable!("component")
        }
        fn private_array_constant_index(
            &mut self,
            _: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
            _: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        ) -> Result<Option<u64>, ProductionRankedProjectionErrorV1> {
            unreachable!("component")
        }
        fn is_materialized_block(
            &mut self,
            _: usize,
        ) -> Result<bool, ProductionRankedProjectionErrorV1> {
            unreachable!("component")
        }
        fn condition(
            &mut self,
            _: usize,
            _: bool,
            _: SemanticBlockIdV1,
        ) -> Result<
            canonical_assertion_facts_v1::ProjectedAssertionConditionV1,
            ProductionRankedProjectionErrorV1,
        > {
            unreachable!("component")
        }
    }
    const SCALAR: ProductionSemanticScalarTypeV2 = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 32,
    };
    fn scalar(bits: u64) -> ProductionSemanticExpressionV2 {
        ProductionSemanticExpressionV2::Constant {
            scalar: SCALAR,
            bits,
        }
    }
    fn load() -> ProductionSemanticLoadV2 {
        ProductionSemanticLoadV2 {
            block: 1,
            operation: 0,
            scalar: SCALAR,
            allocation_origin: 1,
            view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
            indices: vec![ProductionRankedValueV1::Argument(0)].into_boxed_slice(),
        }
    }
    fn source(block: usize, operation: usize, access: AccessKindAttr) -> ProjectedAccessSourceV1 {
        ProjectedAccessSourceV1 {
            private_array_role: None,
            block,
            operation,
            access,
            memory_space: MemorySpaceAttr::Global,
            source: SemanticSourceProvenanceV1::unavailable(),
            semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                block,
                statement: Some(0),
            }),
        }
    }
    fn fixture() -> (
        Vec<ProductionRankedBlockV1>,
        Vec<ProjectedAccessSourceV1>,
        Vec<crate::production_reference_effect_join_v2::RankedGpuWriteV2>,
    ) {
        let load = load();
        let value = ProductionRankedValueIdV1::new(10);
        let blocks = vec![
            ProductionRankedBlockV1::new(
                vec![],
                ProductionRankedTerminatorV1::Branch { target: 1 },
            ),
            ProductionRankedBlockV1::new(
                vec![ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Read,
                    view: load.view,
                    indices: load.indices.to_vec(),
                }],
                ProductionRankedTerminatorV1::Branch { target: 2 },
            ),
            ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::SemanticConstant {
                        result: value,
                        value: 0,
                    },
                    ProductionRankedOperationV1::ValueAccess {
                        kind: AccessKindAttr::Write,
                        view: load.view,
                        indices: load.indices.to_vec(),
                        value: ProductionRankedValueV1::Local(value),
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            ),
        ];
        let writes = vec![
            crate::production_reference_effect_join_v2::RankedGpuWriteV2 {
                block: 2,
                operation: 1,
                allocation_origin: 1,
                view: load.view,
                indices: load.indices.to_vec(),
                value: Ok(ProductionSemanticExpressionV2::Binary {
                    operation: ProductionSemanticBinaryOpV2::Add,
                    overflow: ProductionOverflowContractV2::Wrapping,
                    scalar: SCALAR,
                    lhs: Box::new(ProductionSemanticExpressionV2::Load(load)),
                    rhs: Box::new(scalar(7)),
                }),
            },
        ];
        (
            blocks,
            vec![
                source(1, 0, AccessKindAttr::Read),
                source(2, 1, AccessKindAttr::Write),
            ],
            writes,
        )
    }

    #[test]
    fn ordinary_attachment_preserves_read_and_write_sites_and_rejects_hostile_dependencies() {
        for hostile in 0..9 {
            let (mut blocks, mut sources, mut writes) = fixture();
            match hostile {
                0 => {}
                1 => {
                    blocks[0] = ProductionRankedBlockV1::new(
                        vec![],
                        ProductionRankedTerminatorV1::AnalysisSplit {
                            control_dependencies: vec![],
                            first_block: 1,
                            second_block: 2,
                        },
                    )
                }
                2 => sources[0].memory_space = MemorySpaceAttr::Private,
                3 => {
                    let mut changed = load();
                    changed.operation = 1;
                    writes[0].value = Ok(ProductionSemanticExpressionV2::Load(changed));
                }
                4 => {
                    let mut operations = blocks[2].operations().to_vec();
                    operations[0] = ProductionRankedOperationV1::SemanticConstant {
                        result: ProductionRankedValueIdV1::new(10),
                        value: 9,
                    };
                    blocks[2] = ProductionRankedBlockV1::new(
                        operations,
                        ProductionRankedTerminatorV1::Return,
                    );
                }
                5 => writes[0].value = Err("component unresolved source expression"),
                6 => {
                    let mut changed = load();
                    changed.view = ProductionRankedValueV1::Argument(1);
                    writes[0].value = Ok(ProductionSemanticExpressionV2::Load(changed));
                }
                7 => writes.clear(),
                8 => {
                    let read = blocks[1].operations()[0].clone();
                    let mut operations = blocks[2].operations().to_vec();
                    operations.push(read);
                    blocks[1] = ProductionRankedBlockV1::new(
                        operations,
                        ProductionRankedTerminatorV1::Branch { target: 2 },
                    );
                    blocks[2] =
                        ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return);
                    sources = vec![
                        source(1, 1, AccessKindAttr::Write),
                        source(1, 2, AccessKindAttr::Read),
                    ];
                    writes[0].block = 1;
                    let mut changed = load();
                    changed.operation = 2;
                    writes[0].value = Ok(ProductionSemanticExpressionV2::Load(changed));
                }
                _ => unreachable!(),
            }
            let original_operations = blocks
                .iter()
                .map(|block| block.operations().as_ptr())
                .collect::<Vec<_>>();
            let mut work = Work::new(100_000);
            let mut budget = Budget::new(&mut work, 1_000_000);
            let mut facts = Facts {
                budget: &mut budget,
                checked: true,
            };
            let result =
                attach_projected_global_write_values_v1(&mut blocks, &sources, &writes, &mut facts);
            assert_eq!(result.is_err(), hostile != 0, "hostile {hostile}");
            if hostile == 0 {
                assert_eq!(
                    blocks
                        .iter()
                        .map(|block| block.operations().as_ptr())
                        .collect::<Vec<_>>(),
                    original_operations
                );
                assert!(
                    matches!(&blocks[2].operations()[0], ProductionRankedOperationV1::SemanticExpression {
                    result, expression, numerical_contract,
                } if result.get() == 10 && Ok(expression) == writes[0].value.as_ref()
                    && *numerical_contract == fe2o3_pliron::ProductionNumericalContractV2::exact_for_expression(expression))
                );
                assert!(matches!(
                    blocks[1].operations()[0],
                    ProductionRankedOperationV1::Access {
                        kind: AccessKindAttr::Read,
                        ..
                    }
                ));
                assert!(matches!(
                    blocks[2].operations()[1],
                    ProductionRankedOperationV1::ValueAccess { .. }
                ));
            }
        }
    }

    #[test]
    fn ordinary_load_cache_is_per_distinct_block_not_per_leaf_or_write() {
        let (blocks, sources, _) = fixture();
        let expression = ProductionSemanticExpressionV2::Binary {
            operation: ProductionSemanticBinaryOpV2::Add,
            overflow: ProductionOverflowContractV2::Wrapping,
            scalar: SCALAR,
            lhs: Box::new(ProductionSemanticExpressionV2::Load(load())),
            rhs: Box::new(ProductionSemanticExpressionV2::Load(load())),
        };
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let mut facts = Facts {
            budget: &mut budget,
            checked: true,
        };
        let mut scratch = ProjectedWriteLoadScratchV1::new(&mut facts).unwrap();
        scratch
            .check_expression_v1(
                &expression,
                (2, 0),
                &blocks,
                &sources,
                (false, 0),
                &mut facts,
            )
            .unwrap();
        scratch
            .prepare_reachability_v1(&blocks, &mut facts)
            .unwrap();
        assert_eq!(scratch.reachability.len(), 2); // 3 blocks: one full + one excluded bitset word.
        let floor = facts.budget.storage();
        let start = facts.budget.work();
        for _ in 0..5 {
            scratch
                .check_expression_v1(
                    &expression,
                    (2, 0),
                    &blocks,
                    &sources,
                    (true, 0),
                    &mut facts,
                )
                .unwrap();
        }
        // Three tree nodes, two source binary lookups (two probes each), and
        // one exact index comparison per load.
        assert_eq!(facts.budget.work() - start, 5 * (3 + 2 * (7 + 4 * 2 + 1)));
        assert_eq!(facts.budget.storage(), floor);
    }

    #[test]
    fn ordinary_load_cache_exact_and_under_component_budgets_are_shape_derived() {
        let (blocks, _, _) = fixture();
        const HISTORY: usize = 7;
        const FLOOR: usize = 13;
        // Initial fixed scratch, then 3 row visits, two allocated words,
        // full CFG DFS, 3 excluded-row probes, and entry-only excluded DFS.
        const PREPARE: usize = 3 * MAX_RANKED_BOUNDS_BLOCKS + 6 + 6 + 15 + 3 + 8;
        for (short_work, short_storage) in [(false, false), (true, false), (false, true)] {
            let limit = HISTORY + PREPARE - usize::from(short_work);
            let storage = FLOOR + std::mem::size_of::<ProjectedWriteLoadScratchV1>() + 16
                - usize::from(short_storage);
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, storage);
            budget.charge_work(HISTORY).unwrap();
            budget.reserve_storage(FLOOR).unwrap();
            let mut facts = Facts {
                budget: &mut budget,
                checked: true,
            };
            let mut scratch = ProjectedWriteLoadScratchV1::new(&mut facts).unwrap();
            scratch.excluded_rows[1] = 0;
            let result = scratch.prepare_reachability_v1(&blocks, &mut facts);
            assert_eq!(result.is_err(), short_work || short_storage);
            if !short_storage {
                assert_eq!(
                    facts.budget.work(),
                    HISTORY + PREPARE - usize::from(short_work)
                );
                assert_eq!(
                    facts.budget.storage(),
                    FLOOR + std::mem::size_of::<ProjectedWriteLoadScratchV1>() + 16
                );
            } else {
                assert_eq!(
                    facts.budget.work(),
                    HISTORY + 3 * MAX_RANKED_BOUNDS_BLOCKS + 12
                );
                assert_eq!(
                    facts.budget.storage(),
                    FLOOR + std::mem::size_of::<ProjectedWriteLoadScratchV1>()
                );
            }
            // This component stops inside the owning checked-control scope;
            // actual root fixtures independently require that scope to restore its floor.
        }
    }

    #[test]
    fn ordinary_store_indices_have_exact_and_one_short_paid_comparisons() {
        const HISTORY: usize = 7;
        const FLOOR: usize = 13;
        // One block, two operations, one source binary-search probe, one
        // constant expression and no load: census 2*2+5+4; scratch 3*M;
        // expression passes 3+1+(4*1+1) and 3+1; row preparation 2;
        // final block visit 1 and unchanged mutation prefix 6.
        const FIXED: usize =
            2 * 2 + 5 + 4 + 3 * MAX_RANKED_BOUNDS_BLOCKS + 3 + 1 + (4 + 1) + 2 + 3 + 1 + 1 + 6;
        for width in [1usize, 3, 8] {
            for short in [false, true] {
                let value = ProductionRankedValueIdV1::new(10);
                let view = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
                let indices = (0..width)
                    .map(|index| ProductionRankedValueV1::Argument(index as u32))
                    .collect::<Vec<_>>();
                let mut blocks = vec![ProductionRankedBlockV1::new(
                    vec![
                        ProductionRankedOperationV1::SemanticConstant {
                            result: value,
                            value: 0,
                        },
                        ProductionRankedOperationV1::ValueAccess {
                            kind: AccessKindAttr::Write,
                            view,
                            indices: indices.clone(),
                            value: ProductionRankedValueV1::Local(value),
                        },
                    ],
                    ProductionRankedTerminatorV1::Return,
                )];
                let sources = vec![source(0, 1, AccessKindAttr::Write)];
                let writes = vec![
                    crate::production_reference_effect_join_v2::RankedGpuWriteV2 {
                        block: 0,
                        operation: 1,
                        allocation_origin: 1,
                        view,
                        indices,
                        value: Ok(scalar(7)),
                    },
                ];
                let operation_address = blocks[0].operations().as_ptr();
                let original_store = blocks[0].operations()[1].clone();
                let exact = HISTORY + FIXED + width;
                let storage = FLOOR + std::mem::size_of::<ProjectedWriteLoadScratchV1>();
                let mut work = Work::new(exact - usize::from(short));
                let mut budget = Budget::new(&mut work, storage);
                budget.charge_work(HISTORY).unwrap();
                budget.reserve_storage(FLOOR).unwrap();
                let mut facts = Facts {
                    budget: &mut budget,
                    checked: true,
                };
                let result = attach_projected_global_write_values_v1(
                    &mut blocks,
                    &sources,
                    &writes,
                    &mut facts,
                );
                assert_eq!(result.is_err(), short, "width {width}, short {short}");
                assert_eq!(
                    facts.budget.work(),
                    if short { HISTORY + FIXED } else { exact }
                );
                assert_eq!(facts.budget.storage(), storage);
                assert_eq!(facts.budget.peak_storage(), storage);
                assert_eq!(facts.budget.failed_storage(), None);
                assert_eq!(work.failed_work(), short.then_some(exact));
                assert_eq!(blocks[0].operations().as_ptr(), operation_address);
                assert_eq!(blocks[0].operations()[1], original_store);
                assert_eq!(blocks[0].operations().len(), 2);
                assert_eq!(
                    blocks[0].terminator(),
                    &ProductionRankedTerminatorV1::Return
                );
                if short {
                    assert!(matches!(blocks[0].operations()[0],
                        ProductionRankedOperationV1::SemanticConstant { result, value: 0 }
                        if result == value));
                } else {
                    assert!(matches!(&blocks[0].operations()[0],
                        ProductionRankedOperationV1::SemanticExpression {
                            result, expression, numerical_contract,
                        } if *result == value && *expression == scalar(7)
                            && *numerical_contract == fe2o3_pliron::ProductionNumericalContractV2::exact_for_expression(expression)));
                }
                // This unadmitted component ends before the checked-control
                // owner's floor restoration; it grants no source/O authority.
            }
        }
    }

    #[test]
    fn ordinary_reservation_does_not_change_legacy_or_nonordinary_effects() {
        for checked in [false, true] {
            let mut work = Work::new(10);
            let mut budget = Budget::new(&mut work, 100);
            let mut facts = Facts {
                budget: &mut budget,
                checked,
            };
            let mut operations = Vec::new();
            let mut next = 12;
            let mut operation = ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Write,
                view: load().view,
                indices: load().indices.to_vec(),
            };
            let original = operation.clone();
            reserve_projected_global_write_value_v1(
                &mut operations,
                &mut operation,
                MemorySpaceAttr::Global,
                Some(ProjectedSemanticAccessSiteV1 {
                    block: 1,
                    statement: Some(2),
                }),
                &mut next,
                &mut facts,
            )
            .unwrap();
            assert_eq!(operations.len(), usize::from(checked));
            assert_eq!(next, 12 + u32::from(checked));
            if !checked {
                assert_eq!(operation, original);
                assert_eq!(facts.budget.work(), 0);
            }
        }
    }
}
